//! # Sentinel Storage
//!
//! Data persistence layer for LLM-Sentinel.
//!
//! This crate provides:
//! - Time-series storage (InfluxDB)
//! - In-memory caching (Moka)
//! - Distributed caching (Redis)
//! - Query interfaces for metrics and anomalies

#![warn(missing_debug_implementations, rust_2018_idioms, unreachable_pub)]

pub mod cache;
pub mod influxdb;
pub mod query;

use async_trait::async_trait;
use llm_sentinel_core::{
    events::{AnomalyEvent, TelemetryEvent},
    Result,
};

/// Trait for storage backends
#[async_trait]
pub trait Storage: Send + Sync {
    /// Write a telemetry event
    async fn write_telemetry(&self, event: &TelemetryEvent) -> Result<()>;

    /// Write an anomaly event
    async fn write_anomaly(&self, anomaly: &AnomalyEvent) -> Result<()>;

    /// Batch write telemetry events
    async fn write_telemetry_batch(&self, events: &[TelemetryEvent]) -> Result<()>;

    /// Batch write anomaly events
    async fn write_anomaly_batch(&self, anomalies: &[AnomalyEvent]) -> Result<()>;

    /// Query telemetry events
    async fn query_telemetry(&self, query: query::TelemetryQuery) -> Result<Vec<TelemetryEvent>>;

    /// Query anomaly events
    async fn query_anomalies(&self, query: query::AnomalyQuery) -> Result<Vec<AnomalyEvent>>;

    /// Health check
    async fn health_check(&self) -> Result<()>;
}

/// Re-export commonly used types
pub mod prelude {
    pub use crate::cache::{BaselineCache, CacheConfig};
    pub use crate::influxdb::{InfluxDbStorage, InfluxDbConfig};
    pub use crate::query::{AnomalyQuery, TelemetryQuery, TimeRange};
    pub use crate::Storage;
}
