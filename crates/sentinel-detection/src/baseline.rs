//! Baseline calculation and management for anomaly detection.

use crate::stats::RollingWindow;
use dashmap::DashMap;
use llm_sentinel_core::{
    types::{ModelId, ServiceId},
    Result,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// Baseline statistics for a metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    /// Mean value
    pub mean: f64,
    /// Standard deviation
    pub std_dev: f64,
    /// Median value
    pub median: f64,
    /// Median absolute deviation
    pub mad: f64,
    /// 25th percentile (Q1)
    pub q1: f64,
    /// 75th percentile (Q3)
    pub q3: f64,
    /// Interquartile range
    pub iqr: f64,
    /// 95th percentile
    pub p95: f64,
    /// 99th percentile
    pub p99: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Number of samples
    pub sample_count: usize,
}

impl Baseline {
    /// Calculate baseline from data
    pub fn from_data(data: &[f64]) -> Self {
        if data.is_empty() {
            return Self::empty();
        }

        let mean = crate::stats::mean(data);
        let std_dev = crate::stats::std_dev(data);
        let median = crate::stats::median(data);
        let mad = crate::stats::mad(data);
        let (q1, q3, iqr) = crate::stats::iqr(data);
        let p95 = crate::stats::percentile(data, 95.0);
        let p99 = crate::stats::percentile(data, 99.0);

        let min = data
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .copied()
            .unwrap_or(0.0);
        let max = data
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .copied()
            .unwrap_or(0.0);

        Self {
            mean,
            std_dev,
            median,
            mad,
            q1,
            q3,
            iqr,
            p95,
            p99,
            min,
            max,
            sample_count: data.len(),
        }
    }

    /// Create empty baseline
    pub fn empty() -> Self {
        Self {
            mean: 0.0,
            std_dev: 0.0,
            median: 0.0,
            mad: 0.0,
            q1: 0.0,
            q3: 0.0,
            iqr: 0.0,
            p95: 0.0,
            p99: 0.0,
            min: 0.0,
            max: 0.0,
            sample_count: 0,
        }
    }

    /// Check if baseline is valid (has enough samples)
    pub fn is_valid(&self) -> bool {
        self.sample_count >= 10 // Minimum 10 samples for statistical significance
    }
}

/// Baseline key for multi-dimensional baselines
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct BaselineKey {
    /// Service identifier
    pub service: ServiceId,
    /// Model identifier
    pub model: ModelId,
    /// Metric name
    pub metric: String,
}

impl BaselineKey {
    /// Create a new baseline key
    pub fn new(service: ServiceId, model: ModelId, metric: impl Into<String>) -> Self {
        Self {
            service,
            model,
            metric: metric.into(),
        }
    }

    /// Create key for latency metric
    pub fn latency(service: ServiceId, model: ModelId) -> Self {
        Self::new(service, model, "latency_ms")
    }

    /// Create key for token count metric
    pub fn tokens(service: ServiceId, model: ModelId) -> Self {
        Self::new(service, model, "total_tokens")
    }

    /// Create key for cost metric
    pub fn cost(service: ServiceId, model: ModelId) -> Self {
        Self::new(service, model, "cost_usd")
    }

    /// Create key for error rate metric
    pub fn error_rate(service: ServiceId, model: ModelId) -> Self {
        Self::new(service, model, "error_rate")
    }
}

/// Baseline manager for storing and updating baselines
pub struct BaselineManager {
    /// Window size for rolling baselines
    window_size: usize,
    /// Rolling windows for each key
    windows: Arc<DashMap<BaselineKey, RollingWindow>>,
    /// Cached baselines
    baselines: Arc<DashMap<BaselineKey, Baseline>>,
}

impl std::fmt::Debug for BaselineManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BaselineManager")
            .field("window_size", &self.window_size)
            .field("windows_count", &self.windows.len())
            .field("baselines_count", &self.baselines.len())
            .finish()
    }
}

impl BaselineManager {
    /// Create a new baseline manager
    pub fn new(window_size: usize) -> Self {
        info!("Creating baseline manager with window size {}", window_size);
        Self {
            window_size,
            windows: Arc::new(DashMap::new()),
            baselines: Arc::new(DashMap::new()),
        }
    }

    /// Update baseline with a new value
    pub fn update(&self, key: BaselineKey, value: f64) -> Result<()> {
        // Get or create rolling window
        let mut window = self
            .windows
            .entry(key.clone())
            .or_insert_with(|| RollingWindow::new(self.window_size));

        window.push(value);

        // Recalculate baseline if window is full
        if window.is_full() {
            let baseline = Baseline::from_data(window.data());
            self.baselines.insert(key.clone(), baseline);

            debug!(
                service = %key.service,
                model = %key.model,
                metric = %key.metric,
                "Updated baseline"
            );

            metrics::gauge!(
                "sentinel_baseline_mean",
                "service" => key.service.to_string(),
                "model" => key.model.to_string(),
                "metric" => key.metric.clone()
            )
            .set(self.baselines.get(&key).unwrap().mean);
        }

        Ok(())
    }

    /// Get baseline for a key
    pub fn get(&self, key: &BaselineKey) -> Option<Baseline> {
        self.baselines.get(key).map(|b| b.clone())
    }

    /// Check if baseline exists and is valid
    pub fn has_valid_baseline(&self, key: &BaselineKey) -> bool {
        self.baselines
            .get(key)
            .map(|b| b.is_valid())
            .unwrap_or(false)
    }

    /// Get all baseline keys
    pub fn keys(&self) -> Vec<BaselineKey> {
        self.baselines
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Clear baseline for a key
    pub fn clear(&self, key: &BaselineKey) -> Result<()> {
        self.windows.remove(key);
        self.baselines.remove(key);
        info!(
            service = %key.service,
            model = %key.model,
            metric = %key.metric,
            "Cleared baseline"
        );
        Ok(())
    }

    /// Clear all baselines
    pub fn clear_all(&self) -> Result<()> {
        self.windows.clear();
        self.baselines.clear();
        info!("Cleared all baselines");
        Ok(())
    }

    /// Get statistics about baseline manager
    pub fn stats(&self) -> BaselineManagerStats {
        let total_baselines = self.baselines.len();
        let valid_baselines = self
            .baselines
            .iter()
            .filter(|entry| entry.value().is_valid())
            .count();

        BaselineManagerStats {
            total_baselines,
            valid_baselines,
            window_size: self.window_size,
        }
    }
}

/// Baseline manager statistics
#[derive(Debug, Clone)]
pub struct BaselineManagerStats {
    /// Total number of baselines
    pub total_baselines: usize,
    /// Number of valid baselines
    pub valid_baselines: usize,
    /// Window size
    pub window_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_from_data() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let baseline = Baseline::from_data(&data);

        assert_eq!(baseline.mean, 5.5);
        assert_eq!(baseline.median, 5.5);
        assert_eq!(baseline.min, 1.0);
        assert_eq!(baseline.max, 10.0);
        assert_eq!(baseline.sample_count, 10);
        assert!(baseline.is_valid());
    }

    #[test]
    fn test_baseline_empty() {
        let baseline = Baseline::empty();
        assert_eq!(baseline.mean, 0.0);
        assert_eq!(baseline.sample_count, 0);
        assert!(!baseline.is_valid());
    }

    #[test]
    fn test_baseline_key_creation() {
        let key = BaselineKey::latency(ServiceId::new("test"), ModelId::new("gpt-4"));
        assert_eq!(key.service.as_str(), "test");
        assert_eq!(key.model.as_str(), "gpt-4");
        assert_eq!(key.metric, "latency_ms");
    }

    #[test]
    fn test_baseline_manager_update() {
        let manager = BaselineManager::new(10);
        let key = BaselineKey::latency(ServiceId::new("test"), ModelId::new("gpt-4"));

        // Add values
        for i in 1..=10 {
            manager.update(key.clone(), i as f64).unwrap();
        }

        // Check baseline exists
        assert!(manager.has_valid_baseline(&key));
        let baseline = manager.get(&key).unwrap();
        assert_eq!(baseline.sample_count, 10);
        assert_eq!(baseline.mean, 5.5);
    }

    #[test]
    fn test_baseline_manager_clear() {
        let manager = BaselineManager::new(10);
        let key = BaselineKey::latency(ServiceId::new("test"), ModelId::new("gpt-4"));

        // Add values
        for i in 1..=10 {
            manager.update(key.clone(), i as f64).unwrap();
        }

        assert!(manager.has_valid_baseline(&key));

        // Clear
        manager.clear(&key).unwrap();
        assert!(!manager.has_valid_baseline(&key));
    }

    #[test]
    fn test_baseline_manager_stats() {
        let manager = BaselineManager::new(10);

        // Add baselines for multiple keys
        let key1 = BaselineKey::latency(ServiceId::new("service1"), ModelId::new("gpt-4"));
        let key2 = BaselineKey::tokens(ServiceId::new("service2"), ModelId::new("gpt-3.5"));

        for i in 1..=10 {
            manager.update(key1.clone(), i as f64).unwrap();
            manager.update(key2.clone(), i as f64).unwrap();
        }

        let stats = manager.stats();
        assert_eq!(stats.total_baselines, 2);
        assert_eq!(stats.valid_baselines, 2);
        assert_eq!(stats.window_size, 10);
    }
}
