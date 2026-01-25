//! # Sentinel Core
//!
//! Core types, errors, and utilities for LLM-Sentinel.
//!
//! This crate provides the foundational building blocks used across all Sentinel components:
//! - Common error types and result handling
//! - Telemetry event models (OTLP-compatible)
//! - Anomaly event models
//! - Alert definitions
//! - Configuration structures
//! - Shared utilities
//! - Phase 2 Operational Intelligence infrastructure

#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]
#![forbid(unsafe_code)]

pub mod config;
pub mod error;
pub mod events;
pub mod metrics;
pub mod phase2;
pub mod types;

pub use error::{Error, Result};

/// Re-export commonly used types
pub mod prelude {
    pub use crate::config::Config;
    pub use crate::error::{Error, Result};
    pub use crate::events::{AnomalyEvent, TelemetryEvent};
    pub use crate::phase2::{
        Phase2Config, Phase2Startup, StartupError,
        RuvectorClient, RuvectorError,
        Signal, SignalType, SignalEmitter,
        PerformanceBudget, BudgetViolation,
        LineageCache, CacheConfig,
    };
    pub use crate::types::{AnomalyType, Severity};
}
