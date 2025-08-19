use crate::{
    config::BotConfig,
    error::BotError,
    sniper::SniperBot,
    copy_trader::CopyTraderBot,
    bundler::BundlerBot,
    heaven_client::HeavenClient,
    database::Database,
    monitoring::Metrics,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    signature::{Keypair, read_keypair_file},
    pubkey::Pubkey,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

pub struct HeavenTradingBot {
    config: BotConfig,
    rpc_client: Arc<RpcClient>,
    heaven_client: Arc<HeavenClient>,
    database: Arc<Database>,
    metrics: Arc<Metrics>,
    sniper_bot: Option<Arc<SniperBot>>,
    copy_trader_bot: Option<Arc<CopyTraderBot>>,
    bundler_bot: Option<Arc<BundlerBot>>,
    wallet: Arc<Keypair>,
    is_running: Arc<RwLock<bool>>,
}

impl HeavenTradingBot {
    pub fn new(config: BotConfig) -> Result<Self, BotError> {
        // Validate configuration
        config.validate()?;
        
        // Initialize Solana RPC client
        let rpc_client = Arc::new(RpcClient::new(config.solana.rpc_url.clone()));
        
        // Load wallet
        let wallet_path = shellexpand::tilde(&config.solana.wallet_path).to_string();
        let wallet = Arc::new(read_keypair_file(&wallet_path)
            .map_err(|e| BotError::Config(format!("Failed to load wallet: {}", e)))?);
        
        // Initialize Heaven client
        let heaven_client = Arc::new(HeavenClient::new(
            rpc_client.clone(),
            wallet.clone(),
            config.heaven.clone(),
        )?);
        
        // Initialize database
        let database = Arc::new(Database::new(&config.database)?);
        
        // Initialize metrics
        let metrics = Arc::new(Metrics::new(&config.monitoring)?);
        
        // Initialize component bots
        let sniper_bot = if config.sniper.enabled {
            Some(Arc::new(SniperBot::new(
                config.clone(),
                rpc_client.clone(),
                heaven_client.clone(),
                database.clone(),
                metrics.clone(),
                wallet.clone(),
            )?))
        } else {
            None
        };
        
        let copy_trader_bot = if config.copy_trader.enabled {
            Some(Arc::new(CopyTraderBot::new(
                config.clone(),
                rpc_client.clone(),
                heaven_client.clone(),
                database.clone(),
                metrics.clone(),
                wallet.clone(),
            )?))
        } else {
            None
        };
        
        let bundler_bot = if config.bundler.enabled {
            Some(Arc::new(BundlerBot::new(
                config.clone(),
                rpc_client.clone(),
                heaven_client.clone(),
                database.clone(),
                metrics.clone(),
                wallet.clone(),
            )?))
        } else {
            None
        };
        
        Ok(Self {
            config,
            rpc_client,
            heaven_client,
            database,
            metrics,
            sniper_bot,
            copy_trader_bot,
            bundler_bot,
            wallet,
            is_running: Arc::new(RwLock::new(false)),
        })
    }
    
    pub async fn start(&mut self) -> Result<(), BotError> {
        info!("Starting Heaven Trading Bot...");
        
        // Set running state
        *self.is_running.write().await = true;
        
        // Start metrics server
        if self.config.monitoring.enabled {
            self.metrics.start().await?;
        }
        
        // Start all component bots
        let mut handles = Vec::new();
        
        if let Some(sniper_bot) = &self.sniper_bot {
            let sniper = sniper_bot.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = sniper.start().await {
                    error!("Sniper bot error: {}", e);
                }
            });
            handles.push(handle);
        }
        
        if let Some(copy_trader_bot) = &self.copy_trader_bot {
            let copy_trader = copy_trader_bot.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = copy_trader.start().await {
                    error!("Copy trader bot error: {}", e);
                }
            });
            handles.push(handle);
        }
        
        if let Some(bundler_bot) = &self.bundler_bot {
            let bundler = bundler_bot.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = bundler.start().await {
                    error!("Bundler bot error: {}", e);
                }
            });
            handles.push(handle);
        }
        
        // Start main trading loop
        let main_handle = tokio::spawn({
            let is_running = self.is_running.clone();
            let config = self.config.clone();
            let heaven_client = self.heaven_client.clone();
            let database = self.database.clone();
            let metrics = self.metrics.clone();
            
            async move {
                Self::main_trading_loop(is_running, config, heaven_client, database, metrics).await
            }
        });
        handles.push(main_handle);
        
        // Wait for all components to complete
        for handle in handles {
            if let Err(e) = handle.await {
                error!("Component error: {:?}", e);
            }
        }
        
        Ok(())
    }
    
    pub async fn stop(&mut self) -> Result<(), BotError> {
        info!("Stopping Heaven Trading Bot...");
        *self.is_running.write().await = false;
        
        // Stop metrics server
        if self.config.monitoring.enabled {
            self.metrics.stop().await?;
        }
        
        Ok(())
    }
    
    async fn main_trading_loop(
        is_running: Arc<RwLock<bool>>,
        config: BotConfig,
        heaven_client: Arc<HeavenClient>,
        database: Arc<Database>,
        metrics: Arc<Metrics>,
    ) -> Result<(), BotError> {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(config.monitoring.health_check_interval_secs)
        );
        
        while *is_running.read().await {
            interval.tick().await;
            
            // Health check
            if let Err(e) = Self::health_check(&heaven_client).await {
                warn!("Health check failed: {}", e);
                metrics.record_health_check_failure().await;
            } else {
                metrics.record_health_check_success().await;
            }
            
            // Update metrics
            if let Ok(balance) = heaven_client.get_sol_balance().await {
                metrics.update_sol_balance(balance).await;
            }
            
            // Check for new opportunities
            if let Err(e) = Self::scan_for_opportunities(&heaven_client, &database).await {
                warn!("Opportunity scan failed: {}", e);
            }
        }
        
        Ok(())
    }
    
    async fn health_check(heaven_client: &HeavenClient) -> Result<(), BotError> {
        // Check if we can connect to Heaven
        heaven_client.ping().await?;
        
        // Check if we have sufficient balance
        let balance = heaven_client.get_sol_balance().await?;
        if balance < 0.01 {
            return Err(BotError::InsufficientBalance("Low SOL balance".to_string()));
        }
        
        Ok(())
    }
    
    async fn scan_for_opportunities(
        heaven_client: &HeavenClient,
        database: &Database,
    ) -> Result<(), BotError> {
        // Scan for new token launches
        let new_launches = heaven_client.scan_new_launches().await?;
        
        for launch in new_launches {
            // Store launch information
            database.record_token_launch(&launch).await?;
            
            // Check if it meets our criteria
            if Self::should_trade_launch(&launch).await {
                info!("New trading opportunity: {}", launch.token_mint);
                // Trigger trading logic
            }
        }
        
        Ok(())
    }
    
    async fn should_trade_launch(launch: &crate::types::TokenLaunch) -> bool {
        // Implement trading criteria logic
        // This could include:
        // - Market cap thresholds
        // - Liquidity requirements
        // - Creator vs Community categorization
        // - Volume thresholds
        // - Blacklist/whitelist checks
        
        true // Placeholder
    }
    
    pub async fn get_status(&self) -> BotStatus {
        BotStatus {
            is_running: *self.is_running.read().await,
            sniper_enabled: self.sniper_bot.is_some(),
            copy_trader_enabled: self.copy_trader_bot.is_some(),
            bundler_enabled: self.bundler_bot.is_some(),
            sol_balance: self.heaven_client.get_sol_balance().await.unwrap_or(0.0),
            total_trades: self.database.get_total_trades().await.unwrap_or(0),
            daily_pnl: self.database.get_daily_pnl().await.unwrap_or(0.0),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BotStatus {
    pub is_running: bool,
    pub sniper_enabled: bool,
    pub copy_trader_enabled: bool,
    pub bundler_enabled: bool,
    pub sol_balance: f64,
    pub total_trades: u64,
    pub daily_pnl: f64,
}
