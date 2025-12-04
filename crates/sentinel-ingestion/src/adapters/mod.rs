//! # Upstream Adapter Modules
//!
//! Thin consumes-from integrations for LLM DevOps ecosystem dependencies.
//!
//! This module provides adapter layers for consuming data from:
//! - **LLM-Observatory**: Telemetry streams, time-series traces, structured logs
//! - **LLM-Shield**: Security events, policy violations, PII detection signals
//! - **LLM-Analytics-Hub**: Aggregate metrics, forecasts, historical baselines
//! - **LLM-Config-Manager**: Threshold configuration, detection parameters
//!
//! ## Design Principles
//!
//! 1. **Additive Only**: No modifications to core detection pipelines
//! 2. **No Public API Changes**: Internal consumption layers only
//! 3. **No Circular Imports**: One-way dependency flow from upstream
//! 4. **Thin Adapters**: Minimal transformation, delegate to core types
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    UPSTREAM SOURCES                          │
//! ├─────────────────┬─────────────────┬─────────────────────────┤
//! │  Observatory    │     Shield      │   Analytics-Hub         │
//! │  (telemetry)    │   (security)    │   (metrics/forecasts)   │
//! └────────┬────────┴────────┬────────┴────────┬────────────────┘
//!          │                 │                 │
//!          ▼                 ▼                 ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    ADAPTER LAYER                             │
//! │  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐ │
//! │  │ observatory  │ │   shield     │ │   analytics_hub      │ │
//! │  │   adapter    │ │   adapter    │ │      adapter         │ │
//! │  └──────┬───────┘ └──────┬───────┘ └──────────┬───────────┘ │
//! └─────────┼────────────────┼───────────────────┼──────────────┘
//!           │                │                   │
//!           ▼                ▼                   ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │              SENTINEL CORE TYPES                             │
//! │  TelemetryEvent, AnomalyEvent, Config                       │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod analytics_hub;
pub mod config_manager;
pub mod observatory;
pub mod shield;

// Re-export adapter traits and common types
pub use analytics_hub::AnalyticsHubAdapter;
pub use config_manager::ConfigManagerAdapter;
pub use observatory::ObservatoryAdapter;
pub use shield::ShieldAdapter;

use async_trait::async_trait;
use llm_sentinel_core::Result;

/// Common trait for all upstream adapters
#[async_trait]
pub trait UpstreamAdapter: Send + Sync {
    /// Adapter name for logging and metrics
    fn name(&self) -> &'static str;

    /// Check if the upstream source is available
    async fn health_check(&self) -> Result<()>;

    /// Initialize connection to upstream source
    async fn connect(&mut self) -> Result<()>;

    /// Gracefully disconnect from upstream source
    async fn disconnect(&mut self) -> Result<()>;
}
