use crate::{
    error::BotError,
    config::MonitoringConfig,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use std::time::{Duration, Instant};

pub struct Metrics {
    config: MonitoringConfig,
    is_running: Arc<RwLock<bool>>,
    metrics: Arc<RwLock<HashMap<String, MetricValue>>>,
    health_checks: Arc<RwLock<Vec<HealthCheck>>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
}

#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Timestamp(DateTime<Utc>),
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub last_check: DateTime<Utc>,
    pub error_message: Option<String>,
    pub response_time: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub id: String,
    pub level: AlertLevel,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
    pub acknowledged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

impl Metrics {
    pub fn new(config: &MonitoringConfig) -> Result<Self, BotError> {
        Ok(Self {
            config: config.clone(),
            is_running: Arc::new(RwLock::new(false)),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            health_checks: Arc::new(RwLock::new(Vec::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    pub async fn start(&self) -> Result<(), BotError> {
        if !self.config.enabled {
            return Ok(());
        }
        
        info!("Starting metrics server on port {}", self.config.metrics_port);
        *self.is_running.write().await = true;
        
        // Start metrics collection loop
        let metrics = self.metrics.clone();
        let is_running = self.is_running.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            Self::metrics_collection_loop(metrics, is_running, config).await;
        });
        
        // Start health check loop
        let health_checks = self.health_checks.clone();
        let is_running = self.is_running.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            Self::health_check_loop(health_checks, is_running, config).await;
        });
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<(), BotError> {
        info!("Stopping metrics server");
        *self.is_running.write().await = false;
        Ok(())
    }
    
    // Metrics recording methods
    pub async fn increment_counter(&self, name: &str, value: u64) {
        let mut metrics = self.metrics.write().await;
        let current = metrics.get(name);
        
        match current {
            Some(MetricValue::Counter(current_value)) => {
                metrics.insert(name.to_string(), MetricValue::Counter(current_value + value));
            }
            _ => {
                metrics.insert(name.to_string(), MetricValue::Counter(value));
            }
        }
    }
    
    pub async fn set_gauge(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        metrics.insert(name.to_string(), MetricValue::Gauge(value));
    }
    
    pub async fn record_histogram(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.write().await;
        let current = metrics.get(name);
        
        match current {
            Some(MetricValue::Histogram(values)) => {
                let mut new_values = values.clone();
                new_values.push(value);
                // Keep only last 1000 values
                if new_values.len() > 1000 {
                    new_values.drain(0..new_values.len() - 1000);
                }
                metrics.insert(name.to_string(), MetricValue::Histogram(new_values));
            }
            _ => {
                metrics.insert(name.to_string(), MetricValue::Histogram(vec![value]));
            }
        }
    }
    
    pub async fn set_timestamp(&self, name: &str, timestamp: DateTime<Utc>) {
        let mut metrics = self.metrics.write().await;
        metrics.insert(name.to_string(), MetricValue::Timestamp(timestamp));
    }
    
    // Trading-specific metrics
    pub async fn record_successful_trade(&self, amount_sol: f64) {
        self.increment_counter("trades_successful", 1).await;
        self.increment_counter("total_volume_sol", amount_sol as u64).await;
        self.record_histogram("trade_amounts", amount_sol).await;
    }
    
    pub async fn record_failed_trade(&self, amount_sol: f64) {
        self.increment_counter("trades_failed", 1).await;
        self.record_histogram("failed_trade_amounts", amount_sol).await;
    }
    
    pub async fn record_successful_snipe(&self, amount_sol: f64) {
        self.increment_counter("snipes_successful", 1).await;
        self.record_histogram("snipe_amounts", amount_sol).await;
    }
    
    pub async fn record_failed_snipe(&self, amount_sol: f64) {
        self.increment_counter("snipes_failed", 1).await;
        self.record_histogram("failed_snipe_amounts", amount_sol).await;
    }
    
    pub async fn record_snipe_sale(&self, amount_sol: f64) {
        self.increment_counter("snipes_sold", 1).await;
        self.record_histogram("snipe_sale_amounts", amount_sol).await;
    }
    
    pub async fn record_successful_copy_trade(&self, amount_sol: f64) {
        self.increment_counter("copy_trades_successful", 1).await;
        self.record_histogram("copy_trade_amounts", amount_sol).await;
    }
    
    pub async fn record_failed_copy_trade(&self, amount_sol: f64) {
        self.increment_counter("copy_trades_failed", 1).await;
        self.record_histogram("failed_copy_trade_amounts", amount_sol).await;
    }
    
    pub async fn record_successful_bundle(&self, transaction_count: usize) {
        self.increment_counter("bundles_successful", 1).await;
        self.record_histogram("bundle_transaction_counts", transaction_count as f64).await;
    }
    
    pub async fn record_failed_bundle(&self, transaction_count: usize) {
        self.increment_counter("bundles_failed", 1).await;
        self.record_histogram("failed_bundle_transaction_counts", transaction_count as f64).await;
    }
    
    // Status update methods
    pub async fn update_sol_balance(&self, balance: f64) {
        self.set_gauge("sol_balance", balance).await;
    }
    
    pub async fn update_active_snipes(&self, count: usize) {
        self.set_gauge("active_snipes", count as f64).await;
    }
    
    pub async fn update_active_copy_trades(&self, count: usize) {
        self.set_gauge("active_copy_trades", count as f64).await;
    }
    
    pub async fn update_tracked_traders(&self, count: usize) {
        self.set_gauge("tracked_traders", count as f64).await;
    }
    
    pub async fn update_pending_bundles(&self, count: usize) {
        self.set_gauge("pending_bundles", count as f64).await;
    }
    
    pub async fn update_active_bundles(&self, count: usize) {
        self.set_gauge("active_bundles", count as f64).await;
    }
    
    pub async fn update_bundle_history_count(&self, count: usize) {
        self.set_gauge("bundle_history_count", count as f64).await;
    }
    
    // Health check methods
    pub async fn record_health_check_success(&self) {
        self.increment_counter("health_checks_successful", 1).await;
        self.set_timestamp("last_health_check_success", Utc::now()).await;
    }
    
    pub async fn record_health_check_failure(&self) {
        self.increment_counter("health_checks_failed", 1).await;
        self.set_timestamp("last_health_check_failure", Utc::now()).await;
    }
    
    // Alert methods
    pub async fn create_alert(&self, level: AlertLevel, message: &str) {
        let alert = Alert {
            id: uuid::Uuid::new_v4().to_string(),
            level,
            message: message.to_string(),
            timestamp: Utc::now(),
            acknowledged: false,
            acknowledged_at: None,
        };
        
        let mut alerts = self.alerts.write().await;
        alerts.push(alert.clone());
        
        // Keep only last 1000 alerts
        if alerts.len() > 1000 {
            alerts.drain(0..alerts.len() - 1000);
        }
        
        // Send webhook if configured
        if let Some(webhook_url) = &self.config.alert_webhook {
            if let Err(e) = self.send_webhook_alert(&alert, webhook_url).await {
                warn!("Failed to send webhook alert: {}", e);
            }
        }
        
        info!("Alert created: {:?} - {}", level, message);
    }
    
    async fn send_webhook_alert(&self, alert: &Alert, webhook_url: &str) -> Result<(), BotError> {
        let payload = serde_json::json!({
            "level": format!("{:?}", alert.level),
            "message": alert.message,
            "timestamp": alert.timestamp.to_rfc3339(),
            "id": alert.id
        });
        
        let client = reqwest::Client::new();
        let response = client
            .post(webhook_url)
            .json(&payload)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(BotError::Network(format!("Webhook failed with status: {}", response.status())));
        }
        
        Ok(())
    }
    
    // Metrics collection loop
    async fn metrics_collection_loop(
        metrics: Arc<RwLock<HashMap<String, MetricValue>>>,
        is_running: Arc<RwLock<bool>>,
        config: MonitoringConfig,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(60)); // Collect every minute
        
        while *is_running.read().await {
            interval.tick().await;
            
            // Update system metrics
            if let Err(e) = Self::update_system_metrics(&metrics).await {
                warn!("Failed to update system metrics: {}", e);
            }
            
            // Clean up old metrics
            Self::cleanup_old_metrics(&metrics).await;
        }
    }
    
    async fn update_system_metrics(metrics: &Arc<RwLock<HashMap<String, MetricValue>>>) -> Result<(), BotError> {
        let mut metrics = metrics.write().await;
        
        // Update timestamp
        metrics.insert("last_metrics_update".to_string(), MetricValue::Timestamp(Utc::now()));
        
        // Update uptime
        if let Some(start_time) = metrics.get("start_time") {
            if let MetricValue::Timestamp(start) = start_time {
                let uptime = Utc::now() - *start;
                metrics.insert("uptime_seconds".to_string(), MetricValue::Gauge(uptime.num_seconds() as f64));
            }
        }
        
        Ok(())
    }
    
    async fn cleanup_old_metrics(metrics: &Arc<RwLock<HashMap<String, MetricValue>>>) {
        let mut metrics = metrics.write().await;
        let now = Utc::now();
        
        // Remove metrics older than 24 hours
        let cutoff = now - chrono::Duration::hours(24);
        
        let mut to_remove = Vec::new();
        for (key, value) in metrics.iter() {
            if let MetricValue::Timestamp(timestamp) = value {
                if *timestamp < cutoff {
                    to_remove.push(key.clone());
                }
            }
        }
        
        for key in to_remove {
            metrics.remove(&key);
        }
    }
    
    // Health check loop
    async fn health_check_loop(
        health_checks: Arc<RwLock<Vec<HealthCheck>>>,
        is_running: Arc<RwLock<bool>>,
        config: MonitoringConfig,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(config.health_check_interval_secs));
        
        while *is_running.read().await {
            interval.tick().await;
            
            // Perform health checks
            if let Err(e) = Self::perform_health_checks(&health_checks).await {
                warn!("Health check failed: {}", e);
            }
        }
    }
    
    async fn perform_health_checks(health_checks: &Arc<RwLock<Vec<HealthCheck>>>) -> Result<(), BotError> {
        let mut checks = health_checks.write().await;
        
        // Add basic system health checks
        let system_check = HealthCheck {
            name: "system".to_string(),
            status: HealthStatus::Healthy,
            last_check: Utc::now(),
            error_message: None,
            response_time: Duration::from_millis(1),
        };
        
        checks.push(system_check);
        
        // Keep only last 100 health checks
        if checks.len() > 100 {
            checks.drain(0..checks.len() - 100);
        }
        
        Ok(())
    }
    
    // Metrics retrieval methods
    pub async fn get_metrics(&self) -> HashMap<String, MetricValue> {
        self.metrics.read().await.clone()
    }
    
    pub async fn get_metric(&self, name: &str) -> Option<MetricValue> {
        self.metrics.read().await.get(name).cloned()
    }
    
    pub async fn get_health_checks(&self) -> Vec<HealthCheck> {
        self.health_checks.read().await.clone()
    }
    
    pub async fn get_alerts(&self, level: Option<AlertLevel>) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        
        if let Some(level) = level {
            alerts.iter()
                .filter(|alert| alert.level == level)
                .cloned()
                .collect()
        } else {
            alerts.clone()
        }
    }
    
    // Metrics export for Prometheus format
    pub async fn export_prometheus(&self) -> String {
        let metrics = self.metrics.read().await;
        let mut output = String::new();
        
        output.push_str("# HELP heaven_trading_bot_metrics Heaven Trading Bot Metrics\n");
        output.push_str("# TYPE heaven_trading_bot_metrics counter\n");
        
        for (name, value) in metrics.iter() {
            match value {
                MetricValue::Counter(count) => {
                    output.push_str(&format!("heaven_trading_bot_{} {}\n", name, count));
                }
                MetricValue::Gauge(gauge) => {
                    output.push_str(&format!("heaven_trading_bot_{} {}\n", name, gauge));
                }
                MetricValue::Histogram(values) => {
                    if let Some(max) = values.iter().max_by(|a, b| a.partial_cmp(b).unwrap()) {
                        output.push_str(&format!("heaven_trading_bot_{}_max {}\n", name, max));
                    }
                    if let Some(min) = values.iter().min_by(|a, b| a.partial_cmp(b).unwrap()) {
                        output.push_str(&format!("heaven_trading_bot_{}_min {}\n", name, min));
                    }
                    let avg = values.iter().sum::<f64>() / values.len() as f64;
                    output.push_str(&format!("heaven_trading_bot_{}_avg {}\n", name, avg));
                }
                MetricValue::Timestamp(timestamp) => {
                    output.push_str(&format!("heaven_trading_bot_{} {}\n", name, timestamp.timestamp()));
                }
            }
        }
        
        output
    }
    
    // Performance monitoring
    pub async fn record_performance_metric(&self, name: &str, duration: Duration) {
        self.record_histogram(&format!("{}_duration_ms", name), duration.as_millis() as f64).await;
    }
    
    pub async fn start_performance_timer(&self, name: &str) -> PerformanceTimer {
        PerformanceTimer {
            name: name.to_string(),
            start_time: Instant::now(),
            metrics: self.clone(),
        }
    }
}

#[derive(Clone)]
pub struct PerformanceTimer {
    name: String,
    start_time: Instant,
    metrics: Arc<Metrics>,
}

impl PerformanceTimer {
    pub async fn finish(self) {
        let duration = self.start_time.elapsed();
        self.metrics.record_performance_metric(&self.name, duration).await;
    }
}

impl Drop for PerformanceTimer {
    fn drop(&mut self) {
        // If timer wasn't finished, record it anyway
        let duration = self.start_time.elapsed();
        let metrics = self.metrics.clone();
        let name = self.name.clone();
        
        tokio::spawn(async move {
            metrics.record_performance_metric(&name, duration).await;
        });
    }
}

// Health check implementation
impl Metrics {
    pub async fn check_database_health(&self) -> HealthCheck {
        let start_time = Instant::now();
        let status = HealthStatus::Healthy;
        let error_message = None;
        
        // This would actually check database connectivity
        // For now, just return healthy status
        
        HealthCheck {
            name: "database".to_string(),
            status,
            last_check: Utc::now(),
            error_message,
            response_time: start_time.elapsed(),
        }
    }
    
    pub async fn check_heaven_health(&self) -> HealthCheck {
        let start_time = Instant::now();
        let status = HealthStatus::Healthy;
        let error_message = None;
        
        // This would actually check Heaven connectivity
        // For now, just return healthy status
        
        HealthCheck {
            name: "heaven".to_string(),
            status,
            last_check: Utc::now(),
            error_message,
            response_time: start_time.elapsed(),
        }
    }
    
    pub async fn check_solana_health(&self) -> HealthCheck {
        let start_time = Instant::now();
        let status = HealthStatus::Healthy;
        let error_message = None;
        
        // This would actually check Solana RPC connectivity
        // For now, just return healthy status
        
        HealthCheck {
            name: "solana".to_string(),
            status,
            last_check: Utc::now(),
            error_message,
            response_time: start_time.elapsed(),
        }
    }
}
