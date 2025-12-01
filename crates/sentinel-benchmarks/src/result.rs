//! Canonical BenchmarkResult struct for the benchmark interface.
//!
//! This module provides the standardized result format required by the
//! canonical benchmark interface across all benchmark-target repositories.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Canonical BenchmarkResult struct containing standardized fields.
///
/// This struct is required by the canonical benchmark interface and contains:
/// - `target_id`: Unique identifier for the benchmark target
/// - `metrics`: JSON value containing benchmark metrics
/// - `timestamp`: UTC timestamp when the benchmark was executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Unique identifier for the benchmark target (e.g., "zscore_detector", "detection_engine")
    pub target_id: String,

    /// JSON value containing benchmark metrics (latency, throughput, memory, etc.)
    pub metrics: serde_json::Value,

    /// UTC timestamp when the benchmark was executed
    pub timestamp: DateTime<Utc>,
}

impl BenchmarkResult {
    /// Create a new BenchmarkResult with the current UTC timestamp.
    pub fn new(target_id: impl Into<String>, metrics: serde_json::Value) -> Self {
        Self {
            target_id: target_id.into(),
            metrics,
            timestamp: Utc::now(),
        }
    }

    /// Create a new BenchmarkResult with a specific timestamp.
    pub fn with_timestamp(
        target_id: impl Into<String>,
        metrics: serde_json::Value,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            target_id: target_id.into(),
            metrics,
            timestamp,
        }
    }

    /// Get the target ID.
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Get the metrics as a reference.
    pub fn metrics(&self) -> &serde_json::Value {
        &self.metrics
    }

    /// Get the timestamp.
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Convert the result to a JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Create a BenchmarkResult from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BenchmarkResult {{ target_id: {}, timestamp: {}, metrics: {} }}",
            self.target_id,
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.metrics
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_benchmark_result_new() {
        let metrics = json!({
            "latency_ms": 10.5,
            "throughput": 1000,
            "memory_mb": 128
        });

        let result = BenchmarkResult::new("test_target", metrics.clone());

        assert_eq!(result.target_id(), "test_target");
        assert_eq!(result.metrics(), &metrics);
        assert!(result.timestamp() <= Utc::now());
    }

    #[test]
    fn test_benchmark_result_serialization() {
        let metrics = json!({
            "latency_ms": 10.5,
            "throughput": 1000
        });

        let result = BenchmarkResult::new("test_target", metrics);
        let json_str = result.to_json().unwrap();
        let deserialized = BenchmarkResult::from_json(&json_str).unwrap();

        assert_eq!(result.target_id(), deserialized.target_id());
        assert_eq!(result.metrics(), deserialized.metrics());
    }

    #[test]
    fn test_benchmark_result_display() {
        let metrics = json!({"value": 42});
        let result = BenchmarkResult::new("display_test", metrics);
        let display = format!("{}", result);

        assert!(display.contains("display_test"));
        assert!(display.contains("42"));
    }
}
