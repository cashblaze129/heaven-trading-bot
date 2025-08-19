use crate::{
    error::BotError,
    types::{Trade, TokenLaunch, CopyTrade, Trader, Bundle, BundleResult},
};
use sqlx::{sqlite::SqlitePool, Row};
use chrono::{DateTime, Utc};
use tracing::{info, warn, error};

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(config: &crate::config::DatabaseConfig) -> Result<Self, BotError> {
        let pool = SqlitePool::connect(&config.url).await?;
        
        // Initialize database schema
        let db = Self { pool };
        db.initialize_schema().await?;
        
        Ok(db)
    }
    
    async fn initialize_schema(&self) -> Result<(), BotError> {
        // Create tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                id TEXT PRIMARY KEY,
                token_mint TEXT NOT NULL,
                trade_type TEXT NOT NULL,
                amount_sol REAL NOT NULL,
                token_amount REAL NOT NULL,
                price REAL NOT NULL,
                slippage REAL NOT NULL,
                strategy TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                status TEXT NOT NULL,
                transaction_signature TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#
        ).execute(&self.pool).await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS token_launches (
                id TEXT PRIMARY KEY,
                token_mint TEXT NOT NULL,
                token_name TEXT NOT NULL,
                token_symbol TEXT NOT NULL,
                launch_time TEXT NOT NULL,
                initial_price REAL NOT NULL,
                price REAL NOT NULL,
                market_cap REAL NOT NULL,
                liquidity_sol REAL NOT NULL,
                volume_24h REAL NOT NULL,
                token_type TEXT NOT NULL,
                has_flywheel BOOLEAN NOT NULL,
                flywheel_activity REAL NOT NULL,
                creator_address TEXT,
                social_links TEXT NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#
        ).execute(&self.pool).await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS copy_trades (
                id TEXT PRIMARY KEY,
                original_trade_id TEXT NOT NULL,
                trader_address TEXT NOT NULL,
                trader_name TEXT NOT NULL,
                token_mint TEXT NOT NULL,
                trade_type TEXT NOT NULL,
                amount_sol REAL NOT NULL,
                token_amount REAL NOT NULL,
                price REAL NOT NULL,
                slippage REAL NOT NULL,
                timestamp TEXT NOT NULL,
                status TEXT NOT NULL,
                transaction_signature TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#
        ).execute(&self.pool).await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS traders (
                address TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                total_trades INTEGER NOT NULL,
                successful_trades INTEGER NOT NULL,
                total_profit REAL NOT NULL,
                win_rate REAL NOT NULL,
                average_profit REAL NOT NULL,
                total_volume REAL NOT NULL,
                last_trade_time TEXT NOT NULL,
                is_verified BOOLEAN NOT NULL,
                risk_score REAL NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#
        ).execute(&self.pool).await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bundles (
                id TEXT PRIMARY KEY,
                transactions_count INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                target_block INTEGER,
                priority_fee INTEGER NOT NULL,
                status TEXT NOT NULL,
                bundle_signature TEXT,
                created_at_db TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#
        ).execute(&self.pool).await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bundle_results (
                id TEXT PRIMARY KEY,
                bundle_id TEXT NOT NULL,
                bundle_signature TEXT NOT NULL,
                success BOOLEAN NOT NULL,
                error TEXT,
                submitted_at TEXT NOT NULL,
                confirmed_at TEXT,
                total_transactions INTEGER NOT NULL,
                priority_fee INTEGER NOT NULL,
                created_at TEXT NOT NULL
            )
            "#
        ).execute(&self.pool).await?;
        
        // Create indexes for better performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_token_mint ON trades(token_mint)").execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_timestamp ON trades(timestamp)").execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status)").execute(&self.pool).await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_launches_token_mint ON token_launches(token_mint)").execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_launches_launch_time ON token_launches(launch_time)").execute(&self.pool).await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_copy_trades_trader ON copy_trades(trader_address)").execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_copy_trades_status ON copy_trades(status)").execute(&self.pool).await?;
        
        info!("Database schema initialized successfully");
        Ok(())
    }
    
    // Trade operations
    pub async fn record_trade(&self, trade: &Trade) -> Result<(), BotError> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO trades (
                id, token_mint, trade_type, amount_sol, token_amount, price,
                slippage, strategy, timestamp, status, transaction_signature,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&trade.id)
        .bind(&trade.token_mint)
        .bind(&trade.trade_type)
        .bind(trade.amount_sol)
        .bind(trade.token_amount)
        .bind(trade.price)
        .bind(trade.slippage)
        .bind(format!("{:?}", trade.strategy))
        .bind(trade.timestamp.to_rfc3339())
        .bind(&trade.status)
        .bind(&trade.transaction_signature)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        
        info!("Trade recorded: {}", trade.id);
        Ok(())
    }
    
    pub async fn get_trade(&self, trade_id: &str) -> Result<Option<Trade>, BotError> {
        let row = sqlx::query(
            "SELECT * FROM trades WHERE id = ?"
        )
        .bind(trade_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            Ok(Some(self.row_to_trade(&row)?))
        } else {
            Ok(None)
        }
    }
    
    pub async fn get_trades_by_token(&self, token_mint: &str, limit: Option<u32>) -> Result<Vec<Trade>, BotError> {
        let limit = limit.unwrap_or(100);
        
        let rows = sqlx::query(
            "SELECT * FROM trades WHERE token_mint = ? ORDER BY timestamp DESC LIMIT ?"
        )
        .bind(token_mint)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        let mut trades = Vec::new();
        for row in rows {
            trades.push(self.row_to_trade(&row)?);
        }
        
        Ok(trades)
    }
    
    pub async fn get_total_trades(&self) -> Result<u64, BotError> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM trades").fetch_one(&self.pool).await?;
        Ok(row.get::<i64, _>("count") as u64)
    }
    
    pub async fn get_daily_pnl(&self) -> Result<f64, BotError> {
        let today = Utc::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        
        let row = sqlx::query(
            "SELECT SUM(CASE WHEN trade_type = 'sell' THEN amount_sol ELSE -amount_sol END) as pnl FROM trades WHERE DATE(timestamp) = ?"
        )
        .bind(&today_str)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(row.get::<Option<f64>, _>("pnl").unwrap_or(0.0))
    }
    
    // Token launch operations
    pub async fn record_token_launch(&self, launch: &TokenLaunch) -> Result<(), BotError> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO token_launches (
                id, token_mint, token_name, token_symbol, launch_time,
                initial_price, price, market_cap, liquidity_sol, volume_24h,
                token_type, has_flywheel, flywheel_activity, creator_address,
                social_links, description, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&launch.token_mint)
        .bind(&launch.token_mint)
        .bind(&launch.token_name)
        .bind(&launch.token_symbol)
        .bind(launch.launch_time.to_rfc3339())
        .bind(launch.initial_price)
        .bind(launch.price)
        .bind(launch.market_cap)
        .bind(launch.liquidity_sol)
        .bind(launch.volume_24h)
        .bind(&launch.token_type)
        .bind(launch.has_flywheel)
        .bind(launch.flywheel_activity)
        .bind(&launch.creator_address)
        .bind(serde_json::to_string(&launch.social_links)?)
        .bind(&launch.description)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        
        info!("Token launch recorded: {}", launch.token_mint);
        Ok(())
    }
    
    pub async fn get_recent_launches(&self, limit: Option<u32>) -> Result<Vec<TokenLaunch>, BotError> {
        let limit = limit.unwrap_or(50);
        
        let rows = sqlx::query(
            "SELECT * FROM token_launches ORDER BY launch_time DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        let mut launches = Vec::new();
        for row in rows {
            launches.push(self.row_to_token_launch(&row)?);
        }
        
        Ok(launches)
    }
    
    // Copy trade operations
    pub async fn record_copy_trade(&self, copy_trade: &CopyTrade) -> Result<(), BotError> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO copy_trades (
                id, original_trade_id, trader_address, trader_name, token_mint,
                trade_type, amount_sol, token_amount, price, slippage,
                timestamp, status, transaction_signature, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&copy_trade.id)
        .bind(&copy_trade.original_trade_id)
        .bind(&copy_trade.trader_address)
        .bind(&copy_trade.trader_name)
        .bind(&copy_trade.token_mint)
        .bind(&copy_trade.trade_type)
        .bind(copy_trade.amount_sol)
        .bind(copy_trade.token_amount)
        .bind(copy_trade.price)
        .bind(copy_trade.slippage)
        .bind(copy_trade.timestamp.to_rfc3339())
        .bind(&copy_trade.status)
        .bind(&copy_trade.transaction_signature)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        
        info!("Copy trade recorded: {}", copy_trade.id);
        Ok(())
    }
    
    // Trader operations
    pub async fn record_trader(&self, trader: &Trader) -> Result<(), BotError> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO traders (
                address, name, total_trades, successful_trades, total_profit,
                win_rate, average_profit, total_volume, last_trade_time,
                is_verified, risk_score, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&trader.address)
        .bind(&trader.name)
        .bind(trader.total_trades)
        .bind(trader.successful_trades)
        .bind(trader.total_profit)
        .bind(trader.win_rate)
        .bind(trader.average_profit)
        .bind(trader.total_volume)
        .bind(trader.last_trade_time.to_rfc3339())
        .bind(trader.is_verified)
        .bind(trader.risk_score)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_tracked_traders(&self) -> Result<Vec<Trader>, BotError> {
        let rows = sqlx::query(
            "SELECT * FROM traders ORDER BY total_profit DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut traders = Vec::new();
        for row in rows {
            traders.push(self.row_to_trader(&row)?);
        }
        
        Ok(traders)
    }
    
    pub async fn get_trader(&self, address: &str) -> Result<Option<Trader>, BotError> {
        let row = sqlx::query(
            "SELECT * FROM traders WHERE address = ?"
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row {
            Ok(Some(self.row_to_trader(&row)?))
        } else {
            Ok(None)
        }
    }
    
    // Bundle operations
    pub async fn record_bundle(&self, bundle: &Bundle) -> Result<(), BotError> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO bundles (
                id, transactions_count, created_at, target_block, priority_fee,
                status, bundle_signature, created_at_db, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&bundle.id)
        .bind(bundle.transactions.len() as i64)
        .bind(bundle.created_at.to_rfc3339())
        .bind(bundle.target_block.map(|b| b as i64))
        .bind(bundle.priority_fee as i64)
        .bind(&bundle.status)
        .bind(&bundle.bundle_signature)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    pub async fn record_bundle_result(&self, result: &BundleResult) -> Result<(), BotError> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO bundle_results (
                id, bundle_id, bundle_signature, success, error,
                submitted_at, confirmed_at, total_transactions, priority_fee, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&result.bundle_id)
        .bind(&result.bundle_signature)
        .bind(result.success)
        .bind(&result.error)
        .bind(result.submitted_at.to_rfc3339())
        .bind(result.confirmed_at.map(|t| t.to_rfc3339()))
        .bind(result.total_transactions as i64)
        .bind(result.priority_fee as i64)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // Helper methods to convert database rows to types
    fn row_to_trade(&self, row: &sqlx::sqlite::SqliteRow) -> Result<Trade, BotError> {
        let strategy_str: String = row.get("strategy");
        let strategy = match strategy_str.as_str() {
            "CreatorToken" => crate::types::SniperStrategy::CreatorToken,
            "CommunityToken" => crate::types::SniperStrategy::CommunityToken,
            "HighVolume" => crate::types::SniperStrategy::HighVolume,
            "LowMarketCap" => crate::types::SniperStrategy::LowMarketCap,
            "FlywheelActive" => crate::types::SniperStrategy::FlywheelActive,
            _ => crate::types::SniperStrategy::Custom(strategy_str),
        };
        
        Ok(Trade {
            id: row.get("id"),
            token_mint: row.get("token_mint"),
            trade_type: row.get("trade_type"),
            amount_sol: row.get("amount_sol"),
            token_amount: row.get("token_amount"),
            price: row.get("price"),
            slippage: row.get("slippage"),
            strategy,
            timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))?.with_timezone(&Utc),
            status: row.get("status"),
            transaction_signature: row.get("transaction_signature"),
        })
    }
    
    fn row_to_token_launch(&self, row: &sqlx::sqlite::SqliteRow) -> Result<TokenLaunch, BotError> {
        let social_links: String = row.get("social_links");
        let social_links: Vec<String> = serde_json::from_str(&social_links)?;
        
        Ok(TokenLaunch {
            token_mint: row.get("token_mint"),
            token_name: row.get("token_name"),
            token_symbol: row.get("token_symbol"),
            launch_time: DateTime::parse_from_rfc3339(&row.get::<String, _>("launch_time"))?.with_timezone(&Utc),
            initial_price: row.get("initial_price"),
            price: row.get("price"),
            market_cap: row.get("market_cap"),
            liquidity_sol: row.get("liquidity_sol"),
            volume_24h: row.get("volume_24h"),
            token_type: row.get("token_type"),
            has_flywheel: row.get("has_flywheel"),
            flywheel_activity: row.get("flywheel_activity"),
            creator_address: row.get("creator_address"),
            social_links,
            description: row.get("description"),
        })
    }
    
    fn row_to_trader(&self, row: &sqlx::sqlite::SqliteRow) -> Result<Trader, BotError> {
        Ok(Trader {
            address: row.get("address"),
            name: row.get("name"),
            total_trades: row.get("total_trades"),
            successful_trades: row.get("successful_trades"),
            total_profit: row.get("total_profit"),
            win_rate: row.get("win_rate"),
            average_profit: row.get("average_profit"),
            total_volume: row.get("total_volume"),
            last_trade_time: DateTime::parse_from_rfc3339(&row.get::<String, _>("last_trade_time"))?.with_timezone(&Utc),
            is_verified: row.get("is_verified"),
            risk_score: row.get("risk_score"),
        })
    }
    
    // Database maintenance
    pub async fn cleanup_old_records(&self, days: u32) -> Result<(), BotError> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();
        
        // Clean up old trades
        let result = sqlx::query("DELETE FROM trades WHERE timestamp < ?")
            .bind(&cutoff_str)
            .execute(&self.pool)
            .await?;
        
        info!("Cleaned up {} old trades", result.rows_affected());
        
        // Clean up old token launches
        let result = sqlx::query("DELETE FROM token_launches WHERE launch_time < ?")
            .bind(&cutoff_str)
            .execute(&self.pool)
            .await?;
        
        info!("Cleaned up {} old token launches", result.rows_affected());
        
        Ok(())
    }
    
    pub async fn get_database_stats(&self) -> Result<DatabaseStats, BotError> {
        let trades_count: i64 = sqlx::query("SELECT COUNT(*) FROM trades").fetch_one(&self.pool).await?.get(0);
        let launches_count: i64 = sqlx::query("SELECT COUNT(*) FROM token_launches").fetch_one(&self.pool).await?.get(0);
        let copy_trades_count: i64 = sqlx::query("SELECT COUNT(*) FROM copy_trades").fetch_one(&self.pool).await?.get(0);
        let traders_count: i64 = sqlx::query("SELECT COUNT(*) FROM traders").fetch_one(&self.pool).await?.get(0);
        let bundles_count: i64 = sqlx::query("SELECT COUNT(*) FROM bundles").fetch_one(&self.pool).await?.get(0);
        
        Ok(DatabaseStats {
            trades_count: trades_count as u64,
            launches_count: launches_count as u64,
            copy_trades_count: copy_trades_count as u64,
            traders_count: traders_count as u64,
            bundles_count: bundles_count as u64,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub trades_count: u64,
    pub launches_count: u64,
    pub copy_trades_count: u64,
    pub traders_count: u64,
    pub bundles_count: u64,
}
