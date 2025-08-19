use crate::{
    config::BotConfig,
    error::BotError,
    heaven_client::HeavenClient,
    database::Database,
    monitoring::Metrics,
    types::{Trade, Trader, CopyTrade},
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    signature::Keypair,
    pubkey::Pubkey,
    transaction::Transaction,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

pub struct CopyTraderBot {
    config: BotConfig,
    rpc_client: Arc<RpcClient>,
    heaven_client: Arc<HeavenClient>,
    database: Arc<Database>,
    metrics: Arc<Metrics>,
    wallet: Arc<Keypair>,
    is_running: Arc<RwLock<bool>>,
    tracked_traders: Arc<RwLock<HashMap<String, Trader>>>,
    active_copy_trades: Arc<RwLock<HashMap<String, CopyTrade>>>,
    trader_performance: Arc<RwLock<HashMap<String, TraderPerformance>>>,
}

#[derive(Debug, Clone)]
struct TraderPerformance {
    total_trades: u64,
    successful_trades: u64,
    total_profit: f64,
    win_rate: f64,
    average_profit: f64,
    last_trade_time: DateTime<Utc>,
}

impl CopyTraderBot {
    pub fn new(
        config: BotConfig,
        rpc_client: Arc<RpcClient>,
        heaven_client: Arc<HeavenClient>,
        database: Arc<Database>,
        metrics: Arc<Metrics>,
        wallet: Arc<Keypair>,
    ) -> Result<Self, BotError> {
        Ok(Self {
            config,
            rpc_client,
            heaven_client,
            database,
            metrics,
            wallet,
            is_running: Arc::new(RwLock::new(false)),
            tracked_traders: Arc::new(RwLock::new(HashMap::new())),
            active_copy_trades: Arc::new(RwLock::new(HashMap::new())),
            trader_performance: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    pub async fn start(&mut self) -> Result<(), BotError> {
        info!("Starting Copy Trading Bot...");
        *self.is_running.write().await = true;
        
        // Initialize tracked traders
        self.initialize_tracked_traders().await?;
        
        // Start the main copy trading loop
        self.main_copy_trading_loop().await?;
        
        Ok(())
    }
    
    pub async fn stop(&mut self) -> Result<(), BotError> {
        info!("Stopping Copy Trading Bot...");
        *self.is_running.write().await = false;
        Ok(())
    }
    
    async fn main_copy_trading_loop(&self) -> Result<(), BotError> {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_millis(self.config.copy_trader.delay_ms)
        );
        
        while *self.is_running.read().await {
            interval.tick().await;
            
            // Scan for new trades from tracked traders
            if let Err(e) = self.scan_trader_activity().await {
                warn!("Failed to scan trader activity: {}", e);
                continue;
            }
            
            // Process active copy trades
            if let Err(e) = self.process_copy_trades().await {
                warn!("Failed to process copy trades: {}", e);
            }
            
            // Update trader performance metrics
            if let Err(e) = self.update_trader_performance().await {
                warn!("Failed to update trader performance: {}", e);
            }
            
            // Update copy trading metrics
            self.update_copy_trading_metrics().await;
        }
        
        Ok(())
    }
    
    async fn initialize_tracked_traders(&self) -> Result<(), BotError> {
        info!("Initializing tracked traders...");
        
        // Load traders from database
        let traders = self.database.get_tracked_traders().await?;
        
        for trader in traders {
            if self.should_track_trader(&trader).await {
                self.tracked_traders.write().await.insert(
                    trader.address.clone(),
                    trader.clone(),
                );
                
                // Initialize performance tracking
                self.trader_performance.write().await.insert(
                    trader.address.clone(),
                    TraderPerformance {
                        total_trades: 0,
                        successful_trades: 0,
                        total_profit: 0.0,
                        win_rate: 0.0,
                        average_profit: 0.0,
                        last_trade_time: Utc::now(),
                    },
                );
                
                info!("Tracking trader: {} ({} trades)", trader.name, trader.total_trades);
            }
        }
        
        Ok(())
    }
    
    async fn should_track_trader(&self, trader: &Trader) -> bool {
        // Check blacklist/whitelist
        if !self.config.copy_trader.whitelisted_traders.is_empty() {
            if !self.config.copy_trader.whitelisted_traders.contains(&trader.address) {
                return false;
            }
        }
        
        if self.config.copy_trader.blacklisted_traders.contains(&trader.address) {
            return false;
        }
        
        // Check minimum requirements
        trader.total_trades >= 10 && // At least 10 trades
        trader.win_rate >= self.config.copy_trader.min_trader_profit && // Minimum win rate
        trader.total_volume >= self.config.copy_trader.min_trader_balance // Minimum volume
    }
    
    async fn scan_trader_activity(&self) -> Result<(), BotError> {
        let tracked_traders = self.tracked_traders.read().await;
        
        for (address, trader) in tracked_traders.iter() {
            // Get recent trades from this trader
            let recent_trades = self.heaven_client.get_trader_trades(address).await?;
            
            for trade in recent_trades {
                // Check if this is a new trade we haven't seen
                if self.is_new_trade(&trade).await {
                    debug!("New trade detected from trader {}: {}", trader.name, trade.token_mint);
                    
                    // Evaluate if we should copy this trade
                    if self.should_copy_trade(&trade, trader).await {
                        info!("Copying trade from {}: {} {}", trader.name, trade.trade_type, trade.token_mint);
                        
                        if let Err(e) = self.execute_copy_trade(&trade, trader).await {
                            error!("Failed to copy trade: {}", e);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn is_new_trade(&self, trade: &Trade) -> bool {
        // Check if we've already seen this trade
        let active_trades = self.active_copy_trades.read().await;
        !active_trades.contains_key(&trade.id)
    }
    
    async fn should_copy_trade(&self, trade: &Trade, trader: &Trader) -> bool {
        // Check if we're at max traders limit
        let active_trades = self.active_copy_trades.read().await;
        if active_trades.len() >= self.config.copy_trader.max_traders {
            return false;
        }
        
        // Check if we have sufficient balance
        let balance = self.heaven_client.get_sol_balance().await.unwrap_or(0.0);
        let copy_amount = trade.amount_sol * self.config.copy_trader.copy_percentage;
        
        if balance < copy_amount {
            return false;
        }
        
        // Check if this trade type is allowed
        match trade.trade_type.as_str() {
            "buy" => true, // Always allow buys
            "sell" => {
                // Only copy sells if we have the token
                self.heaven_client.get_token_balance(&trade.token_mint).await.unwrap_or(0.0) > 0.0
            }
            _ => false,
        }
    }
    
    async fn execute_copy_trade(&self, original_trade: &Trade, trader: &Trader) -> Result<(), BotError> {
        // Calculate copy trade amount
        let copy_amount = original_trade.amount_sol * self.config.copy_trader.copy_percentage;
        
        // Create copy trade
        let copy_trade = CopyTrade {
            id: uuid::Uuid::new_v4().to_string(),
            original_trade_id: original_trade.id.clone(),
            trader_address: trader.address.clone(),
            trader_name: trader.name.clone(),
            token_mint: original_trade.token_mint.clone(),
            trade_type: original_trade.trade_type.clone(),
            amount_sol: copy_amount,
            token_amount: 0.0, // Will be calculated
            price: original_trade.price,
            slippage: self.config.copy_trader.max_slippage,
            timestamp: Utc::now(),
            status: "pending".to_string(),
            transaction_signature: None,
        };
    
        // Execute the copy trade
        let result = self.execute_copy_trade_transaction(&copy_trade).await?;
        
        if result.success {
            // Record successful copy trade
            self.active_copy_trades.write().await.insert(
                copy_trade.id.clone(),
                copy_trade.clone(),
            );
            
            // Record in database
            self.database.record_copy_trade(&copy_trade).await?;
            
            // Update metrics
            self.metrics.record_successful_copy_trade(copy_amount).await;
            
            info!("Successfully copied trade from {}: {} SOL", trader.name, copy_amount);
        } else {
            error!("Failed to copy trade from {}: {}", trader.name, result.error.unwrap_or_default());
            self.metrics.record_failed_copy_trade(copy_amount).await;
        }
        
        Ok(())
    }
    
    async fn execute_copy_trade_transaction(&self, copy_trade: &CopyTrade) -> Result<CopyTradeResult, BotError> {
        // Create and execute the trade based on type
        match copy_trade.trade_type.as_str() {
            "buy" => self.execute_copy_buy(copy_trade).await,
            "sell" => self.execute_copy_sell(copy_trade).await,
            _ => Err(BotError::Validation("Invalid trade type".to_string())),
        }
    }
    
    async fn execute_copy_buy(&self, copy_trade: &CopyTrade) -> Result<CopyTradeResult, BotError> {
        // Get buy quote from Heaven AMM
        let quote = self.heaven_client.get_buy_quote(
            &copy_trade.token_mint,
            copy_trade.amount_sol,
            copy_trade.slippage,
        ).await?;
        
        // Create buy instruction
        let buy_ix = self.heaven_client.create_buy_instruction(
            &copy_trade.token_mint,
            copy_trade.amount_sol,
            quote.token_amount,
            &self.wallet.pubkey(),
        ).await?;
        
        // Execute transaction
        let transaction = self.create_signed_transaction(&[buy_ix]).await?;
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        
        // Check transaction status
        let status = self.rpc_client.get_transaction_status(&signature)?;
        
        Ok(CopyTradeResult {
            success: status.is_ok(),
            signature: Some(signature.to_string()),
            error: if status.is_err() { 
                Some(format!("{:?}", status.unwrap_err())) 
            } else { 
                None 
            },
        })
    }
    
    async fn execute_copy_sell(&self, copy_trade: &CopyTrade) -> Result<CopyTradeResult, BotError> {
        // Get current token balance
        let token_balance = self.heaven_client.get_token_balance(&copy_trade.token_mint).await?;
        
        // Get sell quote
        let quote = self.heaven_client.get_sell_quote(
            &copy_trade.token_mint,
            token_balance,
            copy_trade.slippage,
        ).await?;
        
        // Create sell instruction
        let sell_ix = self.heaven_client.create_sell_instruction(
            &copy_trade.token_mint,
            token_balance,
            quote.sol_amount,
            &self.wallet.pubkey(),
        ).await?;
        
        // Execute transaction
        let transaction = self.create_signed_transaction(&[sell_ix]).await?;
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        
        // Check transaction status
        let status = self.rpc_client.get_transaction_status(&signature)?;
        
        Ok(CopyTradeResult {
            success: status.is_ok(),
            signature: Some(signature.to_string()),
            error: if status.is_err() { 
                Some(format!("{:?}", status.unwrap_err())) 
            } else { 
                None 
            },
        })
    }
    
    async fn create_signed_transaction(&self, instructions: &[solana_sdk::instruction::Instruction]) -> Result<Transaction, BotError> {
        // This would create the actual transaction with all necessary instructions
        // For now, returning a placeholder
        unimplemented!("Transaction creation not yet implemented")
    }
    
    async fn process_copy_trades(&self) -> Result<(), BotError> {
        let mut active_trades = self.active_copy_trades.write().await;
        let mut to_remove = Vec::new();
        
        for (trade_id, copy_trade) in active_trades.iter_mut() {
            // Check if trade is complete
            if copy_trade.status == "completed" || copy_trade.status == "failed" {
                to_remove.push(trade_id.clone());
                continue;
            }
            
            // Check if we should close the position
            if self.should_close_copy_trade(copy_trade).await {
                if let Err(e) = self.close_copy_trade(copy_trade).await {
                    warn!("Failed to close copy trade {}: {}", trade_id, e);
                } else {
                    copy_trade.status = "completed".to_string();
                    to_remove.push(trade_id.clone());
                }
            }
        }
        
        // Remove completed trades
        for trade_id in to_remove {
            active_trades.remove(&trade_id);
        }
        
        Ok(())
    }
    
    async fn should_close_copy_trade(&self, copy_trade: &CopyTrade) -> bool {
        // Check if original trader has closed their position
        if let Ok(original_trade) = self.database.get_trade(&copy_trade.original_trade_id).await {
            if original_trade.status == "closed" || original_trade.status == "sold" {
                return true;
            }
        }
        
        // Check profit/loss thresholds
        if let Ok(current_price) = self.heaven_client.get_token_price(&copy_trade.token_mint).await {
            let price_change = (current_price - copy_trade.price) / copy_trade.price;
            
            // Close if profit target reached or stop loss hit
            price_change >= self.config.trading.profit_taking_percentage ||
            price_change <= -self.config.trading.stop_loss_percentage
        } else {
            false
        }
    }
    
    async fn close_copy_trade(&self, copy_trade: &CopyTrade) -> Result<(), BotError> {
        info!("Closing copy trade: {} {}", copy_trade.trade_type, copy_trade.token_mint);
        
        // Execute opposite trade to close position
        match copy_trade.trade_type.as_str() {
            "buy" => {
                // Sell to close long position
                let sell_trade = CopyTrade {
                    id: uuid::Uuid::new_v4().to_string(),
                    original_trade_id: copy_trade.id.clone(),
                    trader_address: copy_trade.trader_address.clone(),
                    trader_name: copy_trade.trader_name.clone(),
                    token_mint: copy_trade.token_mint.clone(),
                    trade_type: "sell".to_string(),
                    amount_sol: 0.0,
                    token_amount: copy_trade.token_amount,
                    price: copy_trade.price,
                    slippage: copy_trade.slippage,
                    timestamp: Utc::now(),
                    status: "pending".to_string(),
                    transaction_signature: None,
                };
                
                self.execute_copy_trade_transaction(&sell_trade).await?;
            }
            "sell" => {
                // Buy to close short position (if supported)
                // For now, just mark as completed
                copy_trade.status = "completed".to_string();
            }
            _ => {}
        }
        
        Ok(())
    }
    
    async fn update_trader_performance(&self) -> Result<(), BotError> {
        let mut performance = self.trader_performance.write().await;
        
        for (address, perf) in performance.iter_mut() {
            // Get updated trader data
            if let Ok(trader) = self.database.get_trader(address).await {
                perf.total_trades = trader.total_trades;
                perf.successful_trades = trader.successful_trades;
                perf.total_profit = trader.total_profit;
                perf.win_rate = trader.win_rate;
                perf.average_profit = trader.average_profit;
                perf.last_trade_time = trader.last_trade_time;
            }
        }
        
        Ok(())
    }
    
    async fn update_copy_trading_metrics(&self) {
        let active_trades = self.active_copy_trades.read().await;
        let tracked_traders = self.tracked_traders.read().await;
        
        self.metrics.update_active_copy_trades(active_trades.len()).await;
        self.metrics.update_tracked_traders(tracked_traders.len()).await;
    }
    
    pub async fn get_copy_trader_status(&self) -> CopyTraderStatus {
        let active_trades = self.active_copy_trades.read().await;
        let tracked_traders = self.tracked_traders.read().await;
        
        CopyTraderStatus {
            is_running: *self.is_running.read().await,
            active_copy_trades: active_trades.len(),
            tracked_traders: tracked_traders.len(),
            max_traders: self.config.copy_trader.max_traders,
            copy_percentage: self.config.copy_trader.copy_percentage,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CopyTradeResult {
    pub success: bool,
    pub signature: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CopyTraderStatus {
    pub is_running: bool,
    pub active_copy_trades: usize,
    pub tracked_traders: usize,
    pub max_traders: usize,
    pub copy_percentage: f64,
}
