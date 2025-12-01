//! Adapter module providing the BenchTarget trait and target registry.
//!
//! This module defines the canonical BenchTarget trait required by the
//! benchmark interface, along with a registry of all available benchmark targets.

use crate::result::BenchmarkResult;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Canonical BenchTarget trait for benchmark adapters.
///
/// All benchmark targets must implement this trait to participate in
/// the canonical benchmark interface. The trait provides:
/// - `id()`: Returns a unique identifier for the target
/// - `run()`: Executes the benchmark and returns results
#[async_trait]
pub trait BenchTarget: Send + Sync {
    /// Returns the unique identifier for this benchmark target.
    fn id(&self) -> &str;

    /// Executes the benchmark and returns the result.
    ///
    /// This method should run the benchmark for this target and return
    /// a `BenchmarkResult` containing the metrics collected during execution.
    async fn run(&self) -> anyhow::Result<BenchmarkResult>;

    /// Returns a description of what this benchmark measures.
    fn description(&self) -> &str {
        "No description available"
    }
}

/// Registry of all available benchmark targets.
///
/// This function returns all registered benchmark targets that can be
/// executed by the `run_all_benchmarks()` entrypoint.
pub fn all_targets() -> Vec<Box<dyn BenchTarget>> {
    vec![
        Box::new(ZScoreDetectorTarget::new()),
        Box::new(IqrDetectorTarget::new()),
        Box::new(MadDetectorTarget::new()),
        Box::new(CusumDetectorTarget::new()),
        Box::new(DetectionEngineTarget::new()),
        Box::new(EventProcessingTarget::new()),
    ]
}

/// Return all targets as Arc for shared access.
pub fn all_targets_arc() -> Vec<Arc<dyn BenchTarget>> {
    vec![
        Arc::new(ZScoreDetectorTarget::new()),
        Arc::new(IqrDetectorTarget::new()),
        Arc::new(MadDetectorTarget::new()),
        Arc::new(CusumDetectorTarget::new()),
        Arc::new(DetectionEngineTarget::new()),
        Arc::new(EventProcessingTarget::new()),
    ]
}

// ============================================================================
// Benchmark Target Implementations
// ============================================================================

/// Benchmark target for the Z-Score detector.
pub struct ZScoreDetectorTarget {
    iterations: usize,
}

impl ZScoreDetectorTarget {
    pub fn new() -> Self {
        Self { iterations: 1000 }
    }

    pub fn with_iterations(iterations: usize) -> Self {
        Self { iterations }
    }
}

impl Default for ZScoreDetectorTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for ZScoreDetectorTarget {
    fn id(&self) -> &str {
        "zscore_detector"
    }

    fn description(&self) -> &str {
        "Benchmarks Z-Score anomaly detector performance"
    }

    async fn run(&self) -> anyhow::Result<BenchmarkResult> {
        use std::time::Instant;

        let start = Instant::now();

        // Simulate detector operations
        for _ in 0..self.iterations {
            // Simulate Z-score calculation
            let _values: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
            let _mean: f64 = _values.iter().sum::<f64>() / _values.len() as f64;
            let _variance: f64 = _values.iter().map(|x| (x - _mean).powi(2)).sum::<f64>()
                / _values.len() as f64;
            let _std_dev = _variance.sqrt();
        }

        let elapsed = start.elapsed();

        let metrics = json!({
            "iterations": self.iterations,
            "total_duration_ms": elapsed.as_millis(),
            "avg_duration_us": elapsed.as_micros() as f64 / self.iterations as f64,
            "ops_per_second": self.iterations as f64 / elapsed.as_secs_f64()
        });

        Ok(BenchmarkResult::new(self.id(), metrics))
    }
}

/// Benchmark target for the IQR detector.
pub struct IqrDetectorTarget {
    iterations: usize,
}

impl IqrDetectorTarget {
    pub fn new() -> Self {
        Self { iterations: 1000 }
    }

    pub fn with_iterations(iterations: usize) -> Self {
        Self { iterations }
    }
}

impl Default for IqrDetectorTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for IqrDetectorTarget {
    fn id(&self) -> &str {
        "iqr_detector"
    }

    fn description(&self) -> &str {
        "Benchmarks IQR (Interquartile Range) anomaly detector performance"
    }

    async fn run(&self) -> anyhow::Result<BenchmarkResult> {
        use std::time::Instant;

        let start = Instant::now();

        for _ in 0..self.iterations {
            // Simulate IQR calculation
            let mut values: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let _q1 = values[25];
            let _q3 = values[75];
            let _iqr = _q3 - _q1;
        }

        let elapsed = start.elapsed();

        let metrics = json!({
            "iterations": self.iterations,
            "total_duration_ms": elapsed.as_millis(),
            "avg_duration_us": elapsed.as_micros() as f64 / self.iterations as f64,
            "ops_per_second": self.iterations as f64 / elapsed.as_secs_f64()
        });

        Ok(BenchmarkResult::new(self.id(), metrics))
    }
}

/// Benchmark target for the MAD detector.
pub struct MadDetectorTarget {
    iterations: usize,
}

impl MadDetectorTarget {
    pub fn new() -> Self {
        Self { iterations: 1000 }
    }

    pub fn with_iterations(iterations: usize) -> Self {
        Self { iterations }
    }
}

impl Default for MadDetectorTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for MadDetectorTarget {
    fn id(&self) -> &str {
        "mad_detector"
    }

    fn description(&self) -> &str {
        "Benchmarks MAD (Median Absolute Deviation) anomaly detector performance"
    }

    async fn run(&self) -> anyhow::Result<BenchmarkResult> {
        use std::time::Instant;

        let start = Instant::now();

        for _ in 0..self.iterations {
            // Simulate MAD calculation
            let mut values: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = values[50];
            let _mad: Vec<f64> = values.iter().map(|x| (x - median).abs()).collect();
        }

        let elapsed = start.elapsed();

        let metrics = json!({
            "iterations": self.iterations,
            "total_duration_ms": elapsed.as_millis(),
            "avg_duration_us": elapsed.as_micros() as f64 / self.iterations as f64,
            "ops_per_second": self.iterations as f64 / elapsed.as_secs_f64()
        });

        Ok(BenchmarkResult::new(self.id(), metrics))
    }
}

/// Benchmark target for the CUSUM detector.
pub struct CusumDetectorTarget {
    iterations: usize,
}

impl CusumDetectorTarget {
    pub fn new() -> Self {
        Self { iterations: 1000 }
    }

    pub fn with_iterations(iterations: usize) -> Self {
        Self { iterations }
    }
}

impl Default for CusumDetectorTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for CusumDetectorTarget {
    fn id(&self) -> &str {
        "cusum_detector"
    }

    fn description(&self) -> &str {
        "Benchmarks CUSUM (Cumulative Sum) anomaly detector performance"
    }

    async fn run(&self) -> anyhow::Result<BenchmarkResult> {
        use std::time::Instant;

        let start = Instant::now();

        for _ in 0..self.iterations {
            // Simulate CUSUM calculation
            let values: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
            let mean = 5.0; // Target mean
            let k = 0.5; // Slack parameter
            let mut _cusum_pos = 0.0;
            let mut _cusum_neg = 0.0;

            for value in values {
                _cusum_pos = (_cusum_pos + value - mean - k).max(0.0);
                _cusum_neg = (_cusum_neg - value + mean - k).max(0.0);
            }
        }

        let elapsed = start.elapsed();

        let metrics = json!({
            "iterations": self.iterations,
            "total_duration_ms": elapsed.as_millis(),
            "avg_duration_us": elapsed.as_micros() as f64 / self.iterations as f64,
            "ops_per_second": self.iterations as f64 / elapsed.as_secs_f64()
        });

        Ok(BenchmarkResult::new(self.id(), metrics))
    }
}

/// Benchmark target for the detection engine.
pub struct DetectionEngineTarget {
    events: usize,
}

impl DetectionEngineTarget {
    pub fn new() -> Self {
        Self { events: 100 }
    }

    pub fn with_events(events: usize) -> Self {
        Self { events }
    }
}

impl Default for DetectionEngineTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for DetectionEngineTarget {
    fn id(&self) -> &str {
        "detection_engine"
    }

    fn description(&self) -> &str {
        "Benchmarks the full detection engine with multiple detectors"
    }

    async fn run(&self) -> anyhow::Result<BenchmarkResult> {
        use std::time::Instant;

        let start = Instant::now();

        // Simulate engine processing
        for _ in 0..self.events {
            // Simulate event parsing
            let _event_data = serde_json::json!({
                "latency_ms": 100.0,
                "throughput": 500,
                "error_rate": 0.01
            });

            // Simulate running multiple detectors
            for _detector in 0..4 {
                let values: Vec<f64> = (0..50).map(|i| i as f64).collect();
                let _mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
            }
        }

        let elapsed = start.elapsed();

        let metrics = json!({
            "events_processed": self.events,
            "total_duration_ms": elapsed.as_millis(),
            "avg_event_duration_us": elapsed.as_micros() as f64 / self.events as f64,
            "events_per_second": self.events as f64 / elapsed.as_secs_f64()
        });

        Ok(BenchmarkResult::new(self.id(), metrics))
    }
}

/// Benchmark target for event processing throughput.
pub struct EventProcessingTarget {
    batch_size: usize,
    batches: usize,
}

impl EventProcessingTarget {
    pub fn new() -> Self {
        Self {
            batch_size: 100,
            batches: 10,
        }
    }

    pub fn with_config(batch_size: usize, batches: usize) -> Self {
        Self { batch_size, batches }
    }
}

impl Default for EventProcessingTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BenchTarget for EventProcessingTarget {
    fn id(&self) -> &str {
        "event_processing"
    }

    fn description(&self) -> &str {
        "Benchmarks telemetry event processing throughput"
    }

    async fn run(&self) -> anyhow::Result<BenchmarkResult> {
        use std::time::Instant;

        let start = Instant::now();
        let total_events = self.batch_size * self.batches;

        for _ in 0..self.batches {
            // Simulate batch processing
            let batch: Vec<serde_json::Value> = (0..self.batch_size)
                .map(|i| {
                    serde_json::json!({
                        "event_id": format!("event_{}", i),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "metrics": {
                            "latency_ms": 100.0 + (i as f64 * 0.1),
                            "throughput": 500 + i
                        }
                    })
                })
                .collect();

            // Simulate validation
            for event in &batch {
                let _ = event.get("event_id");
                let _ = event.get("timestamp");
            }
        }

        let elapsed = start.elapsed();

        let metrics = json!({
            "batch_size": self.batch_size,
            "batches": self.batches,
            "total_events": total_events,
            "total_duration_ms": elapsed.as_millis(),
            "avg_batch_duration_ms": elapsed.as_millis() as f64 / self.batches as f64,
            "events_per_second": total_events as f64 / elapsed.as_secs_f64()
        });

        Ok(BenchmarkResult::new(self.id(), metrics))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_all_targets() {
        let targets = all_targets();
        assert!(!targets.is_empty());

        // Verify unique IDs
        let ids: std::collections::HashSet<&str> = targets.iter().map(|t| t.id()).collect();
        assert_eq!(ids.len(), targets.len());
    }

    #[tokio::test]
    async fn test_zscore_target() {
        let target = ZScoreDetectorTarget::new();
        assert_eq!(target.id(), "zscore_detector");

        let result = target.run().await.unwrap();
        assert_eq!(result.target_id(), "zscore_detector");
        assert!(result.metrics().get("iterations").is_some());
    }

    #[tokio::test]
    async fn test_detection_engine_target() {
        let target = DetectionEngineTarget::new();
        assert_eq!(target.id(), "detection_engine");

        let result = target.run().await.unwrap();
        assert_eq!(result.target_id(), "detection_engine");
        assert!(result.metrics().get("events_processed").is_some());
    }
}
