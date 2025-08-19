use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Solana RPC error: {0}")]
    SolanaRpc(String),
    
    #[error("Heaven SDK error: {0}")]
    HeavenSdk(String),
    
    #[error("Transaction error: {0}")]
    Transaction(String),
    
    #[error("Token error: {0}")]
    Token(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),
    
    #[error("Pool not found: {0}")]
    PoolNotFound(String),
    
    #[error("Invalid quote: {0}")]
    InvalidQuote(String),
    
    #[error("Slippage exceeded: {0}")]
    SlippageExceeded(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),
    
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<solana_client::client_error::ClientError> for BotError {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        BotError::SolanaRpc(err.to_string())
    }
}

impl From<solana_sdk::transaction::TransactionError> for BotError {
    fn from(err: solana_sdk::transaction::TransactionError) -> Self {
        BotError::Transaction(err.to_string())
    }
}

impl From<sqlx::Error> for BotError {
    fn from(err: sqlx::Error) -> Self {
        BotError::Database(err.to_string())
    }
}

impl From<reqwest::Error> for BotError {
    fn from(err: reqwest::Error) -> Self {
        BotError::Network(err.to_string())
    }
}

impl From<serde_json::Error> for BotError {
    fn from(err: serde_json::Error) -> Self {
        BotError::Config(err.to_string())
    }
}

impl From<std::io::Error> for BotError {
    fn from(err: std::io::Error) -> Self {
        BotError::Config(err.to_string())
    }
}
