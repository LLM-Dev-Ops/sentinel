//! # Sentinel Ingestion
//!
//! Telemetry ingestion service for LLM-Sentinel.
//!
//! This crate provides:
//! - Kafka consumer for high-throughput event streaming
//! - OpenTelemetry Protocol (OTLP) parsing
//! - Event validation and normalization
//! - Buffering and batching for efficient processing

#![warn(missing_debug_implementations, rust_2018_idioms, unreachable_pub)]

pub mod kafka;
pub mod otlp;
pub mod pipeline;
pub mod validation;

use async_trait::async_trait;
use llm_sentinel_core::{events::TelemetryEvent, Result};

/// Trait for telemetry ingesters
#[async_trait]
pub trait Ingester: Send + Sync {
    /// Start the ingester
    async fn start(&mut self) -> Result<()>;

    /// Stop the ingester gracefully
    async fn stop(&mut self) -> Result<()>;

    /// Get the next batch of telemetry events
    async fn next_batch(&mut self) -> Result<Vec<TelemetryEvent>>;

    /// Check if ingester is healthy
    async fn health_check(&self) -> Result<()>;
}

/// Re-export commonly used types
pub mod prelude {
    pub use crate::kafka::KafkaIngester;
    pub use crate::otlp::OtlpParser;
    pub use crate::pipeline::{IngestionPipeline, PipelineConfig};
    pub use crate::validation::EventValidator;
    pub use crate::Ingester;
}
