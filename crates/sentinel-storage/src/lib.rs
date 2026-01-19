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

/// No-op storage for API-only deployments
#[derive(Debug, Clone, Default)]
pub struct NoopStorage;

impl NoopStorage {
    /// Create a new no-op storage
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Storage for NoopStorage {
    async fn write_telemetry(&self, _event: &TelemetryEvent) -> Result<()> {
        Ok(())
    }

    async fn write_anomaly(&self, _anomaly: &AnomalyEvent) -> Result<()> {
        Ok(())
    }

    async fn write_telemetry_batch(&self, _events: &[TelemetryEvent]) -> Result<()> {
        Ok(())
    }

    async fn write_anomaly_batch(&self, _anomalies: &[AnomalyEvent]) -> Result<()> {
        Ok(())
    }

    async fn query_telemetry(&self, _query: query::TelemetryQuery) -> Result<Vec<TelemetryEvent>> {
        Ok(vec![])
    }

    async fn query_anomalies(&self, _query: query::AnomalyQuery) -> Result<Vec<AnomalyEvent>> {
        Ok(vec![])
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// Re-export commonly used types
pub mod prelude {
    pub use crate::cache::{BaselineCache, CacheConfig};
    pub use crate::influxdb::{InfluxDbStorage, InfluxDbConfig};
    pub use crate::query::{AnomalyQuery, TelemetryQuery, TimeRange};
    pub use crate::{NoopStorage, Storage};
}
