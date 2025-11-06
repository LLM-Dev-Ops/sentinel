//! # Sentinel Alerting
//!
//! Alert delivery and notification system for LLM-Sentinel.
//!
//! This crate provides:
//! - Alert delivery via RabbitMQ
//! - Webhook notifications
//! - Alert deduplication
//! - Retry logic with exponential backoff
//! - Alert routing by severity

#![warn(missing_debug_implementations, rust_2018_idioms, unreachable_pub)]

pub mod deduplication;
pub mod rabbitmq;
pub mod webhook;

use async_trait::async_trait;
use llm_sentinel_core::{events::AnomalyEvent, Result};
use serde::{Deserialize, Serialize};

/// Trait for alert delivery systems
#[async_trait]
pub trait Alerter: Send + Sync {
    /// Send a single alert
    async fn send(&self, alert: &AnomalyEvent) -> Result<()>;

    /// Send multiple alerts in batch
    async fn send_batch(&self, alerts: &[AnomalyEvent]) -> Result<()> {
        for alert in alerts {
            self.send(alert).await?;
        }
        Ok(())
    }

    /// Health check
    async fn health_check(&self) -> Result<()>;

    /// Get alerter name for logging
    fn name(&self) -> &str;
}

/// Alert metadata for tracking delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMetadata {
    /// Unique alert ID
    pub alert_id: String,
    /// Number of delivery attempts
    pub attempts: u32,
    /// Last delivery attempt timestamp
    pub last_attempt: chrono::DateTime<chrono::Utc>,
    /// Delivery status
    pub status: AlertStatus,
}

/// Alert delivery status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertStatus {
    /// Pending delivery
    Pending,
    /// Successfully delivered
    Delivered,
    /// Failed after retries
    Failed,
    /// Deduplicated (not sent)
    Deduplicated,
}

/// Alert delivery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Enable alert deduplication
    pub enable_deduplication: bool,
    /// Deduplication window in seconds
    pub deduplication_window_secs: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Initial retry delay in milliseconds
    pub retry_delay_ms: u64,
    /// Backoff multiplier for retries
    pub backoff_multiplier: f64,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enable_deduplication: true,
            deduplication_window_secs: 300, // 5 minutes
            max_retries: 3,
            retry_delay_ms: 1000,
            backoff_multiplier: 2.0,
        }
    }
}

/// Re-export commonly used types
pub mod prelude {
    pub use crate::deduplication::{AlertDeduplicator, DeduplicationConfig};
    pub use crate::rabbitmq::{RabbitMqAlerter, RabbitMqConfig};
    pub use crate::webhook::{WebhookAlerter, WebhookConfig};
    pub use crate::{AlertConfig, AlertStatus, Alerter};
}
