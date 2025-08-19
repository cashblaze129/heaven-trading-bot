pub mod error;
pub mod config;
pub mod types;
pub mod heaven_client;
pub mod database;
pub mod monitoring;
pub mod bot;
pub mod sniper;
pub mod copy_trader;
pub mod bundler;

// Re-export main types for convenience
pub use bot::HeavenTradingBot;
pub use sniper::SniperBot;
pub use copy_trader::CopyTraderBot;
pub use bundler::BundlerBot;
pub use config::BotConfig;
pub use error::BotError;
pub use types::*;
