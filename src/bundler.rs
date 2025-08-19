use crate::{
    config::BotConfig,
    error::BotError,
    heaven_client::HeavenClient,
    database::Database,
    monitoring::Metrics,
    types::{Bundle, BundleTransaction, BundleResult},
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    signature::Keypair,
    pubkey::Pubkey,
    transaction::Transaction,
    instruction::Instruction,
    compute_budget::ComputeBudgetInstruction,
    commitment_config::CommitmentConfig,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use std::time::{Duration, Instant};

pub struct BundlerBot {
    config: BotConfig,
    rpc_client: Arc<RpcClient>,
    heaven_client: Arc<HeavenClient>,
    database: Arc<Database>,
    metrics: Arc<Metrics>,
    wallet: Arc<Keypair>,
    is_running: Arc<RwLock<bool>>,
    pending_bundles: Arc<RwLock<Vec<Bundle>>>,
    active_bundles: Arc<RwLock<HashMap<String, Bundle>>>,
    bundle_history: Arc<RwLock<Vec<BundleResult>>>,
    last_bundle_time: Arc<RwLock<Instant>>,
}

impl BundlerBot {
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
            pending_bundles: Arc::new(RwLock::new(Vec::new())),
            active_bundles: Arc::new(RwLock::new(HashMap::new())),
            bundle_history: Arc<RwLock::new(Vec::new())),
            last_bundle_time: Arc<RwLock::new(Instant::now()),
        })
    }
    
    pub async fn start(&mut self) -> Result<(), BotError> {
        info!("Starting Bundler Bot...");
        *self.is_running.write().await = true;
        
        // Start the main bundler loop
        self.main_bundler_loop().await?;
        
        Ok(())
    }
    
    pub async fn stop(&mut self) -> Result<(), BotError> {
        info!("Stopping Bundler Bot...");
        *self.is_running.write().await = false;
        Ok(())
    }
    
    async fn main_bundler_loop(&self) -> Result<(), BotError> {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_millis(self.config.bundler.max_bundle_time_ms)
        );
        
        while *self.is_running.read().await {
            interval.tick().await;
            
            // Process pending bundles
            if let Err(e) = self.process_pending_bundles().await {
                warn!("Failed to process pending bundles: {}", e);
            }
            
            // Monitor active bundles
            if let Err(e) = self.monitor_active_bundles().await {
                warn!("Failed to monitor active bundles: {}", e);
            }
            
            // Clean up completed bundles
            if let Err(e) = self.cleanup_completed_bundles().await {
                warn!("Failed to cleanup completed bundles: {}", e);
            }
            
            // Update bundler metrics
            self.update_bundler_metrics().await;
        }
        
        Ok(())
    }
    
    pub async fn add_transaction_to_bundle(&self, transaction: BundleTransaction) -> Result<(), BotError> {
        let mut pending_bundles = self.pending_bundles.write().await;
        
        // Find an existing bundle or create a new one
        if let Some(bundle) = pending_bundles.last_mut() {
            if bundle.transactions.len() < self.config.bundler.max_bundle_size {
                bundle.transactions.push(transaction);
                debug!("Added transaction to existing bundle: {}", bundle.id);
                return Ok(());
            }
        }
        
        // Create new bundle
        let new_bundle = Bundle {
            id: uuid::Uuid::new_v4().to_string(),
            transactions: vec![transaction],
            created_at: Utc::now(),
            target_block: self.config.bundler.target_block,
            priority_fee: self.calculate_priority_fee().await?,
            status: "pending".to_string(),
            bundle_signature: None,
        };
        
        pending_bundles.push(new_bundle);
        debug!("Created new bundle: {}", new_bundle.id);
        
        Ok(())
    }
    
    async fn process_pending_bundles(&self) -> Result<(), BotError> {
        let mut pending_bundles = self.pending_bundles.write().await;
        let mut to_remove = Vec::new();
        
        for (index, bundle) in pending_bundles.iter_mut().enumerate() {
            // Check if bundle is ready to submit
            if self.should_submit_bundle(bundle).await {
                info!("Submitting bundle: {} with {} transactions", bundle.id, bundle.transactions.len());
                
                // Submit bundle
                match self.submit_bundle(bundle).await {
                    Ok(result) => {
                        // Move to active bundles
                        let mut active_bundles = self.active_bundles.write().await;
                        bundle.status = "submitted".to_string();
                        bundle.bundle_signature = Some(result.bundle_signature.clone());
                        active_bundles.insert(bundle.id.clone(), bundle.clone());
                        
                        // Record bundle result
                        let mut bundle_history = self.bundle_history.write().await;
                        bundle_history.push(result);
                        
                        to_remove.push(index);
                        
                        info!("Bundle {} submitted successfully", bundle.id);
                    }
                    Err(e) => {
                        error!("Failed to submit bundle {}: {}", bundle.id, e);
                        bundle.status = "failed".to_string();
                        to_remove.push(index);
                    }
                }
            }
        }
        
        // Remove processed bundles (in reverse order to maintain indices)
        for &index in to_remove.iter().rev() {
            pending_bundles.remove(index);
        }
        
        Ok(())
    }
    
    async fn should_submit_bundle(&self, bundle: &Bundle) -> bool {
        // Check if bundle is full
        if bundle.transactions.len() >= self.config.bundler.max_bundle_size {
            return true;
        }
        
        // Check if bundle has been waiting too long
        let bundle_age = Utc::now() - bundle.created_at;
        let max_wait_time = Duration::from_millis(self.config.bundler.max_bundle_time_ms);
        
        if bundle_age > chrono::Duration::from_std(max_wait_time).unwrap() {
            return true;
        }
        
        // Check if we have a target block
        if let Some(target_block) = bundle.target_block {
            let current_slot = self.rpc_client.get_slot()?;
            if current_slot >= target_block {
                return true;
            }
        }
        
        false
    }
    
    async fn submit_bundle(&self, bundle: &Bundle) -> Result<BundleResult, BotError> {
        // Create bundle transaction
        let bundle_transaction = self.create_bundle_transaction(bundle).await?;
        
        // Submit with priority fee
        let signature = self.submit_with_priority_fee(&bundle_transaction, bundle.priority_fee).await?;
        
        // Wait for confirmation
        let status = self.wait_for_bundle_confirmation(&signature).await?;
        
        Ok(BundleResult {
            bundle_id: bundle.id.clone(),
            bundle_signature: signature.to_string(),
            success: status.is_ok(),
            error: if status.is_err() { 
                Some(format!("{:?}", status.unwrap_err())) 
            } else { 
                None 
            },
            submitted_at: Utc::now(),
            confirmed_at: Some(Utc::now()),
            total_transactions: bundle.transactions.len(),
            priority_fee: bundle.priority_fee,
        })
    }
    
    async fn create_bundle_transaction(&self, bundle: &Bundle) -> Result<Transaction, BotError> {
        let mut all_instructions = Vec::new();
        
        // Add compute budget instructions for the entire bundle
        all_instructions.push(
            ComputeBudgetInstruction::set_compute_unit_limit(
                self.config.heaven.compute_unit_limit * bundle.transactions.len() as u32
            )
        );
        
        all_instructions.push(
            ComputeBudgetInstruction::set_compute_unit_price(bundle.priority_fee)
        );
        
        // Add all transaction instructions
        for bundle_tx in &bundle.transactions {
            all_instructions.extend(bundle_tx.instructions.clone());
        }
        
        // Create and sign transaction
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &all_instructions,
            Some(&self.wallet.pubkey()),
            &[&*self.wallet],
            recent_blockhash,
        );
        
        Ok(transaction)
    }
    
    async fn submit_with_priority_fee(&self, transaction: &Transaction, priority_fee: u64) -> Result<solana_sdk::signature::Signature, BotError> {
        // Submit transaction with retry logic
        let mut retries = 0;
        let max_retries = 3;
        
        while retries < max_retries {
            match self.rpc_client.send_transaction_with_config(
                transaction,
                solana_client::rpc_config::RpcSendTransactionConfig {
                    skip_preflight: false,
                    preflight_commitment: Some(CommitmentConfig::confirmed()),
                    max_retries: Some(1),
                    min_context_slot: None,
                },
            ) {
                Ok(signature) => {
                    info!("Bundle submitted with signature: {}", signature);
                    return Ok(signature);
                }
                Err(e) => {
                    retries += 1;
                    if retries >= max_retries {
                        return Err(BotError::Transaction(format!("Failed to submit bundle after {} retries: {}", max_retries, e)));
                    }
                    
                    warn!("Bundle submission failed, retrying ({}/{}): {}", retries, max_retries, e);
                    tokio::time::sleep(Duration::from_millis(100 * retries as u64)).await;
                }
            }
        }
        
        Err(BotError::Transaction("Max retries exceeded for bundle submission".to_string()))
    }
    
    async fn wait_for_bundle_confirmation(&self, signature: &solana_sdk::signature::Signature) -> Result<(), BotError> {
        let mut attempts = 0;
        let max_attempts = 30; // 30 seconds timeout
        
        while attempts < max_attempts {
            match self.rpc_client.get_transaction_status(signature) {
                Ok(status) => {
                    if status.is_ok() {
                        info!("Bundle confirmed: {}", signature);
                        return Ok(());
                    } else if status.is_err() {
                        return Err(BotError::Transaction(format!("Bundle failed: {:?}", status.unwrap_err())));
                    }
                }
                Err(e) => {
                    debug!("Bundle status check failed: {}, retrying...", e);
                }
            }
            
            attempts += 1;
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
        
        Err(BotError::Transaction("Bundle confirmation timeout".to_string()))
    }
    
    async fn monitor_active_bundles(&self) -> Result<(), BotError> {
        let mut active_bundles = self.active_bundles.write().await;
        let mut to_remove = Vec::new();
        
        for (bundle_id, bundle) in active_bundles.iter_mut() {
            // Check if bundle is confirmed
            if let Some(signature) = &bundle.bundle_signature {
                if let Ok(status) = self.rpc_client.get_transaction_status(&signature.parse()?) {
                    if status.is_ok() {
                        bundle.status = "confirmed".to_string();
                        to_remove.push(bundle_id.clone());
                        
                        info!("Bundle {} confirmed", bundle_id);
                        self.metrics.record_successful_bundle(bundle.transactions.len()).await;
                    } else if status.is_err() {
                        bundle.status = "failed".to_string();
                        to_remove.push(bundle_id.clone());
                        
                        error!("Bundle {} failed: {:?}", bundle_id, status.unwrap_err());
                        self.metrics.record_failed_bundle(bundle.transactions.len()).await;
                    }
                }
            }
        }
        
        // Remove completed bundles
        for bundle_id in to_remove {
            active_bundles.remove(&bundle_id);
        }
        
        Ok(())
    }
    
    async fn cleanup_completed_bundles(&self) -> Result<(), BotError> {
        let mut bundle_history = self.bundle_history.write().await;
        
        // Keep only last 1000 bundle results
        if bundle_history.len() > 1000 {
            bundle_history.drain(0..bundle_history.len() - 1000);
        }
        
        Ok(())
    }
    
    async fn calculate_priority_fee(&self) -> Result<u64, BotError> {
        // Get current network conditions
        let recent_prioritization_fees = self.rpc_client.get_recent_prioritization_fees(&[])?;
        
        // Calculate optimal priority fee based on network conditions
        let base_fee = self.config.heaven.compute_unit_price;
        let multiplier = self.config.bundler.priority_fee_multiplier;
        
        // Adjust based on network congestion
        let network_fee = if let Some(fees) = recent_prioritization_fees.first() {
            fees.prioritization_fee
        } else {
            base_fee
        };
        
        let optimal_fee = (base_fee as f64 * multiplier).max(network_fee as f64) as u64;
        
        Ok(optimal_fee)
    }
    
    async fn update_bundler_metrics(&self) {
        let pending_count = self.pending_bundles.read().await.len();
        let active_count = self.active_bundles.read().await.len();
        let history_count = self.bundle_history.read().await.len();
        
        self.metrics.update_pending_bundles(pending_count).await;
        self.metrics.update_active_bundles(active_count).await;
        self.metrics.update_bundle_history_count(history_count).await;
    }
    
    pub async fn get_bundler_status(&self) -> BundlerStatus {
        let pending_bundles = self.pending_bundles.read().await;
        let active_bundles = self.active_bundles.read().await;
        let bundle_history = self.bundle_history.read().await;
        
        BundlerStatus {
            is_running: *self.is_running.read().await,
            pending_bundles: pending_bundles.len(),
            active_bundles: active_bundles.len(),
            total_bundles: bundle_history.len(),
            max_bundle_size: self.config.bundler.max_bundle_size,
            priority_fee_multiplier: self.config.bundler.priority_fee_multiplier,
        }
    }
    
    pub async fn get_bundle_stats(&self) -> BundleStats {
        let bundle_history = self.bundle_history.read().await;
        
        let total_bundles = bundle_history.len();
        let successful_bundles = bundle_history.iter().filter(|b| b.success).count();
        let failed_bundles = total_bundles - successful_bundles;
        let success_rate = if total_bundles > 0 {
            successful_bundles as f64 / total_bundles as f64
        } else {
            0.0
        };
        
        let total_transactions: usize = bundle_history.iter().map(|b| b.total_transactions).sum();
        let average_bundle_size = if total_bundles > 0 {
            total_transactions as f64 / total_bundles as f64
        } else {
            0.0
        };
        
        BundleStats {
            total_bundles,
            successful_bundles,
            failed_bundles,
            success_rate,
            total_transactions,
            average_bundle_size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BundlerStatus {
    pub is_running: bool,
    pub pending_bundles: usize,
    pub active_bundles: usize,
    pub total_bundles: usize,
    pub max_bundle_size: usize,
    pub priority_fee_multiplier: f64,
}

#[derive(Debug, Clone)]
pub struct BundleStats {
    pub total_bundles: usize,
    pub successful_bundles: usize,
    pub failed_bundles: usize,
    pub success_rate: f64,
    pub total_transactions: usize,
    pub average_bundle_size: f64,
}
