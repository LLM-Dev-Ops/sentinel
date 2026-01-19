//! Anomaly detection implementations.

pub mod cusum;
pub mod drift;
pub mod iqr;
pub mod mad;
pub mod zscore;

use serde::{Deserialize, Serialize};

/// Common detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    /// Minimum samples required before detection
    pub min_samples: usize,
    /// Update baseline with every event
    pub update_baseline: bool,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            min_samples: 10,
            update_baseline: true,
        }
    }
}
