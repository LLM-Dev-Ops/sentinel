//! Phase 2 - Operational Intelligence (Layer 1)
//!
//! This module provides the core infrastructure for Phase 2 agents:
//! - Startup hardening with environment validation
//! - Ruvector client initialization and health checks
//! - Signal emission types (anomaly, drift, memory lineage, latency)
//! - Performance budget enforcement
//! - Caching layer with TTL support

pub mod startup;
pub mod ruvector;
pub mod signals;
pub mod budget;
pub mod cache;

pub use startup::{Phase2Config, Phase2Startup, StartupError};
pub use ruvector::{RuvectorClient, RuvectorError};
pub use signals::{Signal, SignalType, SignalEmitter};
pub use budget::{PerformanceBudget, BudgetViolation};
pub use cache::{LineageCache, CacheConfig};
