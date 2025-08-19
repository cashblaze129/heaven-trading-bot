use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// Token and Launch Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLaunch {
    pub token_mint: String,
    pub token_name: String,
    pub token_symbol: String,
    pub launch_time: DateTime<Utc>,
    pub initial_price: f64,
    pub price: f64,
    pub market_cap: f64,
    pub liquidity_sol: f64,
    pub volume_24h: f64,
    pub token_type: String, // "creator" or "community"
    pub has_flywheel: bool,
    pub flywheel_activity: f64,
    pub creator_address: Option<String>,
    pub social_links: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub mint: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub supply: u64,
    pub price: f64,
    pub market_cap: f64,
    pub volume_24h: f64,
    pub liquidity_sol: f64,
    pub price_change_24h: f64,
    pub last_updated: DateTime<Utc>,
}

// Trading Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub token_mint: String,
    pub trade_type: String, // "buy" or "sell"
    pub amount_sol: f64,
    pub token_amount: f64,
    pub price: f64,
    pub slippage: f64,
    pub strategy: SniperStrategy,
    pub timestamp: DateTime<Utc>,
    pub status: String,
    pub transaction_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeQuote {
    pub token_amount: f64,
    pub sol_amount: f64,
    pub price: f64,
    pub slippage: f64,
    pub fee: f64,
    pub fee_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    pub success: bool,
    pub signature: Option<String>,
    pub error: Option<String>,
    pub gas_used: Option<u64>,
    pub block_number: Option<u64>,
}

// Sniper Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SniperStrategy {
    CreatorToken,
    CommunityToken,
    HighVolume,
    LowMarketCap,
    FlywheelActive,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnipeTarget {
    pub token_mint: String,
    pub strategy: SniperStrategy,
    pub target_price: f64,
    pub max_amount: f64,
    pub priority: SnipePriority,
    pub conditions: Vec<SnipeCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnipePriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnipeCondition {
    MarketCapBelow(f64),
    VolumeAbove(f64),
    PriceBelow(f64),
    LiquidityAbove(f64),
    HasFlywheel,
    CreatorVerified,
    Custom(String),
}

// Copy Trading Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trader {
    pub address: String,
    pub name: String,
    pub total_trades: u64,
    pub successful_trades: u64,
    pub total_profit: f64,
    pub win_rate: f64,
    pub average_profit: f64,
    pub total_volume: f64,
    pub last_trade_time: DateTime<Utc>,
    pub is_verified: bool,
    pub risk_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTrade {
    pub id: String,
    pub original_trade_id: String,
    pub trader_address: String,
    pub trader_name: String,
    pub token_mint: String,
    pub trade_type: String,
    pub amount_sol: f64,
    pub token_amount: f64,
    pub price: f64,
    pub slippage: f64,
    pub timestamp: DateTime<Utc>,
    pub status: String,
    pub transaction_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTradeResult {
    pub success: bool,
    pub signature: Option<String>,
    pub error: Option<String>,
}

// Bundler Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub id: String,
    pub transactions: Vec<BundleTransaction>,
    pub created_at: DateTime<Utc>,
    pub target_block: Option<u64>,
    pub priority_fee: u64,
    pub status: String,
    pub bundle_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleTransaction {
    pub id: String,
    pub instructions: Vec<solana_sdk::instruction::Instruction>,
    pub signers: Vec<String>,
    pub fee_payer: String,
    pub compute_units: u32,
    pub priority_fee: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleResult {
    pub bundle_id: String,
    pub bundle_signature: String,
    pub success: bool,
    pub error: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub total_transactions: usize,
    pub priority_fee: u64,
}

// Heaven AMM Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    pub token_a: TokenInfo,
    pub token_b: TokenInfo,
    pub liquidity: f64,
    pub fee_rate: f64,
    pub protocol_fee_rate: f64,
    pub creator_fee_rate: f64,
    pub last_swap_time: DateTime<Utc>,
    pub total_volume: f64,
    pub total_fees: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    pub version: u8,
    pub admin: String,
    pub fee_collector: String,
    pub light_token_mint: String,
    pub max_slippage: f64,
    pub min_liquidity: f64,
    pub emergency_pause: bool,
}

// Fee Structure Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeStructure {
    pub protocol_fee: f64,
    pub creator_fee: f64,
    pub total_fee: f64,
    pub fee_type: FeeType,
    pub market_cap_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeeType {
    Below100k,    // 1% protocol fee
    CommunityAbove100k, // 0.25% protocol + 0.1% creator
    CreatorAbove100k,   // 0.5% protocol + 1% creator
}

// Flywheel Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlywheelInfo {
    pub address: String,
    pub token_mint: String,
    pub total_buybacks: f64,
    pub total_burned: f64,
    pub last_activity: DateTime<Utc>,
    pub is_active: bool,
    pub buyback_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuybackEvent {
    pub id: String,
    pub flywheel_address: String,
    pub token_mint: String,
    pub amount_sol: f64,
    pub tokens_bought: f64,
    pub tokens_burned: f64,
    pub timestamp: DateTime<Utc>,
    pub transaction_signature: String,
}

// Market Data Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub token_mint: String,
    pub price: f64,
    pub volume_24h: f64,
    pub market_cap: f64,
    pub price_change_1h: f64,
    pub price_change_24h: f64,
    pub price_change_7d: f64,
    pub liquidity: f64,
    pub holders: u64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceAlert {
    pub id: String,
    pub token_mint: String,
    pub target_price: f64,
    pub alert_type: AlertType,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub triggered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    PriceAbove(f64),
    PriceBelow(f64),
    VolumeAbove(f64),
    MarketCapAbove(f64),
    MarketCapBelow(f64),
}

// Risk Management Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskProfile {
    pub max_position_size: f64,
    pub max_daily_loss: f64,
    pub max_slippage: f64,
    pub stop_loss_percentage: f64,
    pub take_profit_percentage: f64,
    pub max_concurrent_trades: usize,
    pub blacklisted_tokens: Vec<String>,
    pub whitelisted_tokens: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub token_mint: String,
    pub entry_price: f64,
    pub current_price: f64,
    pub amount_sol: f64,
    pub token_amount: f64,
    pub unrealized_pnl: f64,
    pub entry_time: DateTime<Utc>,
    pub strategy: SniperStrategy,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
}

// Performance Tracking Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub average_win: f64,
    pub average_loss: f64,
    pub profit_factor: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub total_volume: f64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub date: DateTime<Utc>,
    pub trades_count: u64,
    pub pnl: f64,
    pub volume: f64,
    pub fees_paid: f64,
    pub best_trade: Option<Trade>,
    pub worst_trade: Option<Trade>,
}

// Configuration Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub enabled: bool,
    pub max_position_size: f64,
    pub max_daily_trades: usize,
    pub max_daily_loss: f64,
    pub slippage_tolerance: f64,
    pub gas_optimization: bool,
    pub auto_rebalance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub webhook_url: Option<String>,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub email_enabled: bool,
    pub email_address: Option<String>,
    pub notification_level: NotificationLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationLevel {
    Critical,
    Warning,
    Info,
    Debug,
}

// Database Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseRecord {
    pub id: String,
    pub table_name: String,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// API Response Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub has_next: bool,
    pub has_prev: bool,
}

// Event Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotEvent {
    pub id: String,
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
    pub severity: EventSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    TradeExecuted,
    TradeFailed,
    NewTokenLaunch,
    PriceAlert,
    RiskLimitExceeded,
    BotStarted,
    BotStopped,
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}
