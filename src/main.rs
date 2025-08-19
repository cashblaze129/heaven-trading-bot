use clap::{Parser, Subcommand};
use heaven_trading_bot::{
    bot::HeavenTradingBot,
    config::BotConfig,
    error::BotError,
    sniper::SniperBot,
    copy_trader::CopyTraderBot,
    bundler::BundlerBot,
};
use tracing::{info, error};

#[derive(Parser)]
#[command(name = "heaven-trading-bot")]
#[command(about = "Advanced trading bot for Heaven.xyz with sniper, copy trading, and bundler capabilities")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the main trading bot
    Start {
        /// Path to config file
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },
    /// Run sniper bot only
    Sniper {
        /// Path to config file
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },
    /// Run copy trading bot only
    CopyTrade {
        /// Path to config file
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },
    /// Run bundler bot only
    Bundler {
        /// Path to config file
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), BotError> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Start { config } => {
            info!("Starting Heaven Trading Bot...");
            let bot_config = BotConfig::from_file(&config)?;
            let mut bot = HeavenTradingBot::new(bot_config)?;
            bot.start().await?;
        }
        Commands::Sniper { config } => {
            info!("Starting Sniper Bot...");
            let bot_config = BotConfig::from_file(&config)?;
            let mut sniper = SniperBot::new(bot_config)?;
            sniper.start().await?;
        }
        Commands::CopyTrade { config } => {
            info!("Starting Copy Trading Bot...");
            let bot_config = BotConfig::from_file(&config)?;
            let mut copy_trader = CopyTraderBot::new(bot_config)?;
            copy_trader.start().await?;
        }
        Commands::Bundler { config } => {
            info!("Starting Bundler Bot...");
            let bot_config = BotConfig::from_file(&config)?;
            let mut bundler = BundlerBot::new(bot_config)?;
            bundler.start().await?;
        }
    }
    
    Ok(())
}
