use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use crate::error::BotError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub solana: SolanaConfig,
    pub heaven: HeavenConfig,
    pub sniper: SniperConfig,
    pub copy_trader: CopyTraderConfig,
    pub bundler: BundlerConfig,
    pub trading: TradingConfig,
    pub database: DatabaseConfig,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub commitment: String,
    pub wallet_path: String,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeavenConfig {
    pub program_id: String,
    pub protocol_config_version: u8,
    pub chainlink_sol_usd_feed: String,
    pub light_token_mint: String,
    pub max_slippage: f64,
    pub compute_unit_limit: u32,
    pub compute_unit_price: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SniperConfig {
    pub enabled: bool,
    pub max_sol_per_trade: f64,
    pub min_liquidity_sol: f64,
    pub max_slippage: f64,
    pub gas_optimization: bool,
    pub frontrun_protection: bool,
    pub auto_approve: bool,
    pub blacklisted_tokens: Vec<String>,
    pub whitelisted_tokens: Vec<String>,
    pub min_market_cap: f64,
    pub max_market_cap: f64,
    pub volume_threshold: f64,
    pub launch_detection_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTraderConfig {
    pub enabled: bool,
    pub max_sol_per_trade: f64,
    pub copy_percentage: f64,
    pub max_traders: usize,
    pub min_trader_balance: f64,
    pub min_trader_profit: f64,
    pub blacklisted_traders: Vec<String>,
    pub whitelisted_traders: Vec<String>,
    pub delay_ms: u64,
    pub auto_approve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlerConfig {
    pub enabled: bool,
    pub max_bundle_size: usize,
    pub max_bundle_time_ms: u64,
    pub priority_fee_multiplier: f64,
    pub target_block: Option<u64>,
    pub auto_submit: bool,
    pub bundle_validation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub max_concurrent_trades: usize,
    pub trade_timeout_secs: u64,
    pub profit_taking_percentage: f64,
    pub stop_loss_percentage: f64,
    pub max_daily_trades: usize,
    pub max_daily_loss_sol: f64,
    pub risk_per_trade: f64,
    pub auto_rebalance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub metrics_port: u16,
    pub health_check_interval_secs: u64,
    pub alert_webhook: Option<String>,
    pub log_level: String,
}

impl BotConfig {
    pub fn from_file(path: &str) -> Result<Self, BotError> {
        let config_content = std::fs::read_to_string(path)?;
        let config: BotConfig = toml::from_str(&config_content)?;
        Ok(config)
    }
    
    pub fn validate(&self) -> Result<(), BotError> {
        // Validate Solana config
        if self.solana.rpc_url.is_empty() {
            return Err(BotError::Validation("Solana RPC URL cannot be empty".to_string()));
        }
        
        // Validate Heaven config
        if self.heaven.program_id.is_empty() {
            return Err(BotError::Validation("Heaven program ID cannot be empty".to_string()));
        }
        
        // Validate trading config
        if self.trading.max_concurrent_trades == 0 {
            return Err(BotError::Validation("Max concurrent trades must be greater than 0".to_string()));
        }
        
        // Validate sniper config
        if self.sniper.enabled && self.sniper.max_sol_per_trade <= 0.0 {
            return Err(BotError::Validation("Max SOL per trade must be greater than 0".to_string()));
        }
        
        Ok(())
    }
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            solana: SolanaConfig {
                rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
                ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                wallet_path: "~/.config/solana/id.json".to_string(),
                max_retries: 3,
                retry_delay_ms: 1000,
            },
            heaven: HeavenConfig {
                program_id: "heaven_program_id_here".to_string(),
                protocol_config_version: 1,
                chainlink_sol_usd_feed: "GvDMxPzN1sCj7L26YDK2HnjMRmcCVK6yGVSHx7KC8dL3".to_string(),
                light_token_mint: "88aUGeGXFNaEyzL48fkzSPWUPhJr3gWrMDD8EH8tCb1".to_string(),
                max_slippage: 0.05,
                compute_unit_limit: 200_000,
                compute_unit_price: 1_000_000,
            },
            sniper: SniperConfig {
                enabled: true,
                max_sol_per_trade: 0.1,
                min_liquidity_sol: 0.01,
                max_slippage: 0.1,
                gas_optimization: true,
                frontrun_protection: true,
                auto_approve: false,
                blacklisted_tokens: vec![],
                whitelisted_tokens: vec![],
                min_market_cap: 0.0,
                max_market_cap: 1_000_000.0,
                volume_threshold: 1000.0,
                launch_detection_delay_ms: 100,
            },
            copy_trader: CopyTraderConfig {
                enabled: true,
                max_sol_per_trade: 0.05,
                copy_percentage: 0.1,
                max_traders: 10,
                min_trader_balance: 1.0,
                min_trader_profit: 0.05,
                blacklisted_traders: vec![],
                whitelisted_traders: vec![],
                delay_ms: 500,
                auto_approve: false,
            },
            bundler: BundlerConfig {
                enabled: true,
                max_bundle_size: 10,
                max_bundle_time_ms: 1000,
                priority_fee_multiplier: 1.5,
                target_block: None,
                auto_submit: true,
                bundle_validation: true,
            },
            trading: TradingConfig {
                max_concurrent_trades: 5,
                trade_timeout_secs: 30,
                profit_taking_percentage: 0.2,
                stop_loss_percentage: 0.1,
                max_daily_trades: 100,
                max_daily_loss_sol: 1.0,
                risk_per_trade: 0.02,
                auto_rebalance: true,
            },
            database: DatabaseConfig {
                url: "sqlite:trading_bot.db".to_string(),
                max_connections: 10,
                connection_timeout_secs: 30,
            },
            monitoring: MonitoringConfig {
                enabled: true,
                metrics_port: 8080,
                health_check_interval_secs: 60,
                alert_webhook: None,
                log_level: "info".to_string(),
            },
        }
    }
}
