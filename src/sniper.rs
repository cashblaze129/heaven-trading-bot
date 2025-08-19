use crate::{
    config::BotConfig,
    error::BotError,
    heaven_client::HeavenClient,
    database::Database,
    monitoring::Metrics,
    types::{TokenLaunch, Trade, SniperStrategy},
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    signature::Keypair,
    pubkey::Pubkey,
    transaction::Transaction,
    instruction::Instruction,
    compute_budget::ComputeBudgetInstruction,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

pub struct SniperBot {
    config: BotConfig,
    rpc_client: Arc<RpcClient>,
    heaven_client: Arc<HeavenClient>,
    database: Arc<Database>,
    metrics: Arc<Metrics>,
    wallet: Arc<Keypair>,
    is_running: Arc<RwLock<bool>>,
    active_snipes: Arc<RwLock<HashMap<String, ActiveSnipe>>>,
    strategies: Vec<SniperStrategy>,
    last_scan_time: Arc<RwLock<DateTime<Utc>>>,
}

#[derive(Debug, Clone)]
struct ActiveSnipe {
    token_mint: String,
    strategy: SniperStrategy,
    entry_price: f64,
    entry_time: DateTime<Utc>,
    trade_amount: f64,
    status: SnipeStatus,
}

#[derive(Debug, Clone, PartialEq)]
enum SnipeStatus {
    Pending,
    Executed,
    Failed,
    Sold,
}

impl SniperBot {
    pub fn new(
        config: BotConfig,
        rpc_client: Arc<RpcClient>,
        heaven_client: Arc<HeavenClient>,
        database: Arc<Database>,
        metrics: Arc<Metrics>,
        wallet: Arc<Keypair>,
    ) -> Result<Self, BotError> {
        // Initialize sniper strategies
        let strategies = Self::initialize_strategies(&config)?;
        
        Ok(Self {
            config,
            rpc_client,
            heaven_client,
            database,
            metrics,
            wallet,
            is_running: Arc::new(RwLock::false()),
            active_snipes: Arc::new(RwLock::new(HashMap::new())),
            strategies,
            last_scan_time: Arc<RwLock::new(Utc::now()),
        })
    }
    
    pub async fn start(&mut self) -> Result<(), BotError> {
        info!("Starting Sniper Bot...");
        *self.is_running.write().await = true;
        
        // Start the main sniper loop
        self.main_sniper_loop().await?;
        
        Ok(())
    }
    
    pub async fn stop(&mut self) -> Result<(), BotError> {
        info!("Stopping Sniper Bot...");
        *self.is_running.write().await = false;
        Ok(())
    }
    
    async fn main_sniper_loop(&self) -> Result<(), BotError> {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_millis(self.config.sniper.launch_detection_delay_ms)
        );
        
        while *self.is_running.read().await {
            interval.tick().await;
            
            // Scan for new launches
            if let Err(e) = self.scan_new_launches().await {
                warn!("Failed to scan for new launches: {}", e);
                continue;
            }
            
            // Process active snipes
            if let Err(e) = self.process_active_snipes().await {
                warn!("Failed to process active snipes: {}", e);
            }
            
            // Update metrics
            self.update_sniper_metrics().await;
        }
        
        Ok(())
    }
    
    async fn scan_new_launches(&self) -> Result<(), BotError> {
        let new_launches = self.heaven_client.scan_new_launches().await?;
        let mut last_scan = self.last_scan_time.write().await;
        
        for launch in new_launches {
            // Check if this is a new launch since our last scan
            if launch.launch_time > *last_scan {
                debug!("New launch detected: {}", launch.token_mint);
                
                // Evaluate launch against our strategies
                if let Some(strategy) = self.evaluate_launch(&launch).await {
                    info!("Launch {} matches strategy: {:?}", launch.token_mint, strategy);
                    
                    // Execute snipe
                    if let Err(e) = self.execute_snipe(&launch, &strategy).await {
                        error!("Failed to execute snipe for {}: {}", launch.token_mint, e);
                    }
                }
            }
        }
        
        *last_scan = Utc::now();
        Ok(())
    }
    
    async fn evaluate_launch(&self, launch: &TokenLaunch) -> Option<SniperStrategy> {
        for strategy in &self.strategies {
            if self.matches_strategy(launch, strategy).await {
                return Some(strategy.clone());
            }
        }
        None
    }
    
    async fn matches_strategy(&self, launch: &TokenLaunch, strategy: &SniperStrategy) -> bool {
        // Check blacklist/whitelist
        if !self.config.sniper.whitelisted_tokens.is_empty() {
            if !self.config.sniper.whitelisted_tokens.contains(&launch.token_mint) {
                return false;
            }
        }
        
        if self.config.sniper.blacklisted_tokens.contains(&launch.token_mint) {
            return false;
        }
        
        // Check market cap requirements
        if launch.market_cap < self.config.sniper.min_market_cap {
            return false;
        }
        
        if launch.market_cap > self.config.sniper.max_market_cap {
            return false;
        }
        
        // Check liquidity requirements
        if launch.liquidity_sol < self.config.sniper.min_liquidity_sol {
            return false;
        }
        
        // Check volume threshold
        if launch.volume_24h < self.config.sniper.volume_threshold {
            return false;
        }
        
        // Strategy-specific checks
        match strategy {
            SniperStrategy::CreatorToken => {
                launch.token_type == "creator" && launch.has_flywheel
            }
            SniperStrategy::CommunityToken => {
                launch.token_type == "community"
            }
            SniperStrategy::HighVolume => {
                launch.volume_24h > self.config.sniper.volume_threshold * 10.0
            }
            SniperStrategy::LowMarketCap => {
                launch.market_cap < 10000.0 // $10k threshold
            }
            SniperStrategy::FlywheelActive => {
                launch.has_flywheel && launch.flywheel_activity > 0.0
            }
        }
    }
    
    async fn execute_snipe(&self, launch: &TokenLaunch, strategy: &SniperStrategy) -> Result<(), BotError> {
        info!("Executing snipe for {} with strategy {:?}", launch.token_mint, strategy);
        
        // Calculate trade amount based on strategy and risk
        let trade_amount = self.calculate_trade_amount(strategy, launch).await?;
        
        // Check if we have sufficient balance
        let balance = self.heaven_client.get_sol_balance().await?;
        if balance < trade_amount {
            return Err(BotError::InsufficientBalance(
                format!("Insufficient SOL for snipe: {:.4} < {:.4}", balance, trade_amount)
            ));
        }
        
        // Create and execute the trade
        let trade = self.create_snipe_trade(launch, trade_amount, strategy).await?;
        
        // Execute the trade
        let result = self.execute_trade(&trade).await?;
        
        if result.success {
            // Record successful snipe
            let active_snipe = ActiveSnipe {
                token_mint: launch.token_mint.clone(),
                strategy: strategy.clone(),
                entry_price: launch.price,
                entry_time: Utc::now(),
                trade_amount,
                status: SnipeStatus::Executed,
            };
            
            self.active_snipes.write().await.insert(
                launch.token_mint.clone(),
                active_snipe,
            );
            
            // Record trade in database
            self.database.record_trade(&trade).await?;
            
            // Update metrics
            self.metrics.record_successful_snipe(trade_amount).await;
            
            info!("Successfully sniped {} for {:.4} SOL", launch.token_mint, trade_amount);
        } else {
            error!("Snipe failed for {}: {}", launch.token_mint, result.error.unwrap_or_default());
            self.metrics.record_failed_snipe(trade_amount).await;
        }
        
        Ok(())
    }
    
    async fn calculate_trade_amount(&self, strategy: &SniperStrategy, launch: &TokenLaunch) -> Result<f64, BotError> {
        let base_amount = self.config.sniper.max_sol_per_trade;
        
        // Adjust based on strategy
        let multiplier = match strategy {
            SniperStrategy::CreatorToken => 1.0, // Full amount for creator tokens
            SniperStrategy::CommunityToken => 0.7, // 70% for community tokens
            SniperStrategy::HighVolume => 1.2, // 120% for high volume
            SniperStrategy::LowMarketCap => 0.8, // 80% for low market cap
            SniperStrategy::FlywheelActive => 1.1, // 110% for active flywheel
        };
        
        // Adjust based on risk
        let risk_multiplier = if launch.market_cap < 1000.0 { 0.5 } else { 1.0 };
        
        let adjusted_amount = base_amount * multiplier * risk_multiplier;
        
        // Ensure we don't exceed max amount
        Ok(adjusted_amount.min(self.config.sniper.max_sol_per_trade))
    }
    
    async fn create_snipe_trade(&self, launch: &TokenLaunch, amount: f64, strategy: &SniperStrategy) -> Result<Trade, BotError> {
        // Get quote from Heaven AMM
        let quote = self.heaven_client.get_buy_quote(
            &launch.token_mint,
            amount,
            self.config.sniper.max_slippage,
        ).await?;
        
        // Create trade instruction
        let trade_ix = self.heaven_client.create_buy_instruction(
            &launch.token_mint,
            amount,
            quote.token_amount,
            &self.wallet.pubkey(),
        ).await?;
        
        // Add compute budget instructions for gas optimization
        let mut instructions = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(self.config.heaven.compute_unit_limit),
            ComputeBudgetInstruction::set_compute_unit_price(self.config.heaven.compute_unit_price),
        ];
        
        instructions.push(trade_ix);
        
        // Create trade object
        Ok(Trade {
            id: uuid::Uuid::new_v4().to_string(),
            token_mint: launch.token_mint.clone(),
            trade_type: "buy".to_string(),
            amount_sol: amount,
            token_amount: quote.token_amount,
            price: quote.price,
            slippage: quote.slippage,
            strategy: strategy.clone(),
            timestamp: Utc::now(),
            status: "pending".to_string(),
            transaction_signature: None,
        })
    }
    
    async fn execute_trade(&self, trade: &Trade) -> Result<TradeResult, BotError> {
        // Create and sign transaction
        let transaction = self.create_signed_transaction(&trade).await?;
        
        // Submit transaction
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        
        // Check transaction status
        let status = self.rpc_client.get_transaction_status(&signature)?;
        
        Ok(TradeResult {
            success: status.is_ok(),
            signature: Some(signature.to_string()),
            error: if status.is_err() { 
                Some(format!("{:?}", status.unwrap_err())) 
            } else { 
                None 
            },
        })
    }
    
    async fn create_signed_transaction(&self, trade: &Trade) -> Result<Transaction, BotError> {
        // This would create the actual transaction with all necessary instructions
        // For now, returning a placeholder
        unimplemented!("Transaction creation not yet implemented")
    }
    
    async fn process_active_snipes(&self) -> Result<(), BotError> {
        let mut active_snipes = self.active_snipes.write().await;
        let mut to_remove = Vec::new();
        
        for (token_mint, snipe) in active_snipes.iter_mut() {
            match snipe.status {
                SnipeStatus::Executed => {
                    // Check if we should sell
                    if self.should_sell_snipe(snipe).await {
                        if let Err(e) = self.sell_snipe(snipe).await {
                            warn!("Failed to sell snipe {}: {}", token_mint, e);
                        } else {
                            snipe.status = SnipeStatus::Sold;
                            to_remove.push(token_mint.clone());
                        }
                    }
                }
                SnipeStatus::Failed | SnipeStatus::Sold => {
                    to_remove.push(token_mint.clone());
                }
                _ => {}
            }
        }
        
        // Remove completed snipes
        for token_mint in to_remove {
            active_snipes.remove(&token_mint);
        }
        
        Ok(())
    }
    
    async fn should_sell_snipe(&self, snipe: &ActiveSnipe) -> bool {
        // Get current price
        if let Ok(current_price) = self.heaven_client.get_token_price(&snipe.token_mint).await {
            let price_change = (current_price - snipe.entry_price) / snipe.entry_price;
            
            // Sell if profit target reached or stop loss hit
            price_change >= self.config.trading.profit_taking_percentage ||
            price_change <= -self.config.trading.stop_loss_percentage
        } else {
            false
        }
    }
    
    async fn sell_snipe(&self, snipe: &ActiveSnipe) -> Result<(), BotError> {
        info!("Selling snipe for {} at {:.4} SOL", snipe.token_mint, snipe.trade_amount);
        
        // Get current token balance
        let token_balance = self.heaven_client.get_token_balance(&snipe.token_mint).await?;
        
        // Create sell trade
        let sell_trade = Trade {
            id: uuid::Uuid::new_v4().to_string(),
            token_mint: snipe.token_mint.clone(),
            trade_type: "sell".to_string(),
            amount_sol: 0.0, // Will be calculated
            token_amount: token_balance,
            price: 0.0, // Will be calculated
            slippage: self.config.sniper.max_slippage,
            strategy: snipe.strategy.clone(),
            timestamp: Utc::now(),
            status: "pending".to_string(),
            transaction_signature: None,
        };
        
        // Execute sell
        let result = self.execute_trade(&sell_trade).await?;
        
        if result.success {
            info!("Successfully sold snipe for {}", snipe.token_mint);
            self.metrics.record_snipe_sale(snipe.trade_amount).await;
        } else {
            error!("Failed to sell snipe for {}: {}", snipe.token_mint, result.error.unwrap_or_default());
        }
        
        Ok(())
    }
    
    async fn update_sniper_metrics(&self) {
        let active_count = self.active_snipes.read().await.len();
        self.metrics.update_active_snipes(active_count).await;
    }
    
    fn initialize_strategies(config: &BotConfig) -> Result<Vec<SniperStrategy>, BotError> {
        let mut strategies = Vec::new();
        
        // Add default strategies based on config
        strategies.push(SniperStrategy::CreatorToken);
        strategies.push(SniperStrategy::CommunityToken);
        strategies.push(SniperStrategy::HighVolume);
        strategies.push(SniperStrategy::LowMarketCap);
        strategies.push(SniperStrategy::FlywheelActive);
        
        Ok(strategies)
    }
    
    pub async fn get_sniper_status(&self) -> SniperStatus {
        let active_snipes = self.active_snipes.read().await;
        
        SniperStatus {
            is_running: *self.is_running.read().await,
            active_snipes: active_snipes.len(),
            total_strategies: self.strategies.len(),
            last_scan: *self.last_scan_time.read().await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TradeResult {
    pub success: bool,
    pub signature: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SniperStatus {
    pub is_running: bool,
    pub active_snipes: usize,
    pub total_strategies: usize,
    pub last_scan: DateTime<Utc>,
}
