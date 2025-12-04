//! # LLM-Analytics-Hub Adapter
//!
//! Consumes aggregate metrics, forecasts, and historical baselines
//! from LLM-Analytics-Hub.
//!
//! ## Data Consumed
//!
//! - **Aggregate Metrics**: Time-windowed statistical aggregations
//! - **Forecasts**: Predicted values with confidence intervals
//! - **Historical Baselines**: Rolling window statistics for comparison
//!
//! ## Integration Pattern
//!
//! Analytics-Hub data enriches Sentinel's anomaly detection by providing
//! historical context and predictions. This adapter consumes data but
//! does NOT modify any detection algorithms.

use crate::adapters::UpstreamAdapter;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use llm_sentinel_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Configuration for Analytics-Hub adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsHubConfig {
    /// Analytics-Hub API endpoint
    pub api_endpoint: String,

    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,

    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,

    /// Default time window for metric queries
    pub default_time_window: TimeWindow,

    /// Cache TTL for baseline data in seconds
    pub baseline_cache_ttl_secs: u64,
}

impl Default for AnalyticsHubConfig {
    fn default() -> Self {
        Self {
            api_endpoint: "http://localhost:8082".to_string(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            default_time_window: TimeWindow::OneHour,
            baseline_cache_ttl_secs: 300,
        }
    }
}

/// Time window for aggregation (mirrors Analytics-Hub's TimeWindow)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeWindow {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    SixHours,
    OneDay,
    OneWeek,
    OneMonth,
}

impl TimeWindow {
    /// Convert to seconds
    pub fn to_seconds(&self) -> u64 {
        match self {
            TimeWindow::OneMinute => 60,
            TimeWindow::FiveMinutes => 300,
            TimeWindow::FifteenMinutes => 900,
            TimeWindow::OneHour => 3600,
            TimeWindow::SixHours => 21600,
            TimeWindow::OneDay => 86400,
            TimeWindow::OneWeek => 604800,
            TimeWindow::OneMonth => 2592000,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeWindow::OneMinute => "1m",
            TimeWindow::FiveMinutes => "5m",
            TimeWindow::FifteenMinutes => "15m",
            TimeWindow::OneHour => "1h",
            TimeWindow::SixHours => "6h",
            TimeWindow::OneDay => "1d",
            TimeWindow::OneWeek => "1w",
            TimeWindow::OneMonth => "1M",
        }
    }
}

/// Statistical measures from Analytics-Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalMeasures {
    /// Average value
    pub avg: f64,

    /// Minimum value
    pub min: f64,

    /// Maximum value
    pub max: f64,

    /// 50th percentile (median)
    pub p50: f64,

    /// 95th percentile
    pub p95: f64,

    /// 99th percentile
    pub p99: f64,

    /// Standard deviation (optional)
    #[serde(default)]
    pub stddev: Option<f64>,

    /// Sample count
    pub count: u64,

    /// Sum of all values
    pub sum: f64,
}

impl StatisticalMeasures {
    /// Calculate coefficient of variation
    pub fn coefficient_of_variation(&self) -> Option<f64> {
        self.stddev.map(|s| s / self.avg)
    }

    /// Check if a value is an outlier (beyond 3 sigma)
    pub fn is_outlier(&self, value: f64) -> bool {
        if let Some(stddev) = self.stddev {
            let z_score = (value - self.avg).abs() / stddev;
            z_score > 3.0
        } else {
            false
        }
    }
}

/// Aggregated metric from Analytics-Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetric {
    /// Metric name
    pub name: String,

    /// Aggregation window
    pub window: TimeWindow,

    /// Window start time
    pub window_start: DateTime<Utc>,

    /// Window end time
    pub window_end: DateTime<Utc>,

    /// Statistical values
    pub values: StatisticalMeasures,

    /// Grouping tags
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

/// Prediction point from Analytics-Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionPoint {
    /// Prediction timestamp
    pub timestamp: DateTime<Utc>,

    /// Predicted value
    pub value: f64,

    /// Confidence level (0.0-1.0)
    pub confidence: f64,

    /// Upper bound of prediction interval
    pub upper_bound: f64,

    /// Lower bound of prediction interval
    pub lower_bound: f64,
}

impl PredictionPoint {
    /// Check if an observed value is within prediction bounds
    pub fn is_within_bounds(&self, observed: f64) -> bool {
        observed >= self.lower_bound && observed <= self.upper_bound
    }

    /// Calculate deviation from predicted value
    pub fn deviation(&self, observed: f64) -> f64 {
        observed - self.value
    }

    /// Calculate relative deviation as percentage
    pub fn relative_deviation(&self, observed: f64) -> f64 {
        if self.value != 0.0 {
            ((observed - self.value) / self.value) * 100.0
        } else {
            0.0
        }
    }
}

/// Forecast data from Analytics-Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forecast {
    /// Metric name
    pub metric_name: String,

    /// Model/service this forecast applies to
    pub model: Option<String>,

    /// Service this forecast applies to
    pub service: Option<String>,

    /// Prediction points
    pub predictions: Vec<PredictionPoint>,

    /// Forecast generation time
    pub generated_at: DateTime<Utc>,

    /// Forecast method used
    pub method: String,
}

/// Historical baseline from Analytics-Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricBaseline {
    /// Metric name
    pub metric_name: String,

    /// Baseline mean value
    pub mean: f64,

    /// Standard deviation
    pub stddev: f64,

    /// Sample count
    pub sample_count: usize,

    /// Time range covered
    pub time_range_start: DateTime<Utc>,
    pub time_range_end: DateTime<Utc>,

    /// Last update time
    pub updated_at: DateTime<Utc>,

    /// Tags for filtering
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

impl MetricBaseline {
    /// Calculate Z-score for a given value
    pub fn z_score(&self, value: f64) -> f64 {
        if self.stddev != 0.0 {
            (value - self.mean) / self.stddev
        } else {
            0.0
        }
    }

    /// Check if value exceeds threshold (in sigma units)
    pub fn exceeds_threshold(&self, value: f64, threshold_sigma: f64) -> bool {
        self.z_score(value).abs() > threshold_sigma
    }
}

/// Anomaly correlation from Analytics-Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyCorrelation {
    /// Correlation identifier
    pub correlation_id: String,

    /// Correlated anomaly IDs
    pub anomaly_ids: Vec<String>,

    /// Correlation strength (0.0-1.0)
    pub strength: f64,

    /// Root cause analysis (if available)
    pub root_cause: Option<RootCauseAnalysis>,

    /// Detection timestamp
    pub detected_at: DateTime<Utc>,
}

/// Root cause analysis from Analytics-Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseAnalysis {
    /// Root event identifier
    pub root_event_id: String,

    /// Confidence in the analysis
    pub confidence: f64,

    /// Causal chain description
    pub causal_chain: Vec<String>,

    /// Contributing factors
    pub contributing_factors: Vec<String>,

    /// Recommendations
    pub recommendations: Vec<String>,
}

/// Adapter for consuming analytics data from LLM-Analytics-Hub
#[derive(Debug)]
pub struct AnalyticsHubAdapter {
    config: AnalyticsHubConfig,
    connected: Arc<RwLock<bool>>,

    // Cached baselines for quick lookup
    baseline_cache: Arc<RwLock<HashMap<String, MetricBaseline>>>,

    // Cached forecasts
    forecast_cache: Arc<RwLock<HashMap<String, Forecast>>>,

    // Statistics
    metrics_fetched: Arc<RwLock<u64>>,
    baselines_fetched: Arc<RwLock<u64>>,
    forecasts_fetched: Arc<RwLock<u64>>,
}

impl AnalyticsHubAdapter {
    /// Create a new Analytics-Hub adapter
    pub fn new(config: AnalyticsHubConfig) -> Self {
        Self {
            config,
            connected: Arc::new(RwLock::new(false)),
            baseline_cache: Arc::new(RwLock::new(HashMap::new())),
            forecast_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics_fetched: Arc::new(RwLock::new(0)),
            baselines_fetched: Arc::new(RwLock::new(0)),
            forecasts_fetched: Arc::new(RwLock::new(0)),
        }
    }

    /// Consume aggregated metrics from Analytics-Hub
    pub async fn consume_metrics(
        &self,
        metrics: Vec<AggregatedMetric>,
    ) -> Result<Vec<AggregatedMetric>> {
        let mut count = self.metrics_fetched.write().await;
        *count += metrics.len() as u64;

        debug!("Consumed {} aggregated metrics", metrics.len());
        Ok(metrics)
    }

    /// Consume and cache baselines from Analytics-Hub
    pub async fn consume_baselines(&self, baselines: Vec<MetricBaseline>) -> Result<()> {
        let mut cache = self.baseline_cache.write().await;

        for baseline in &baselines {
            let key = format!(
                "{}:{}:{}",
                baseline.metric_name,
                baseline.tags.get("service").unwrap_or(&"*".to_string()),
                baseline.tags.get("model").unwrap_or(&"*".to_string())
            );
            cache.insert(key, baseline.clone());
        }

        let mut count = self.baselines_fetched.write().await;
        *count += baselines.len() as u64;

        debug!(
            "Cached {} baselines, total cache size: {}",
            baselines.len(),
            cache.len()
        );
        Ok(())
    }

    /// Get cached baseline for a metric
    pub async fn get_baseline(
        &self,
        metric_name: &str,
        service: Option<&str>,
        model: Option<&str>,
    ) -> Option<MetricBaseline> {
        let cache = self.baseline_cache.read().await;

        // Try exact match first
        let key = format!(
            "{}:{}:{}",
            metric_name,
            service.unwrap_or("*"),
            model.unwrap_or("*")
        );

        if let Some(baseline) = cache.get(&key) {
            return Some(baseline.clone());
        }

        // Try wildcard match
        let wildcard_key = format!("{}:*:*", metric_name);
        cache.get(&wildcard_key).cloned()
    }

    /// Consume and cache forecasts from Analytics-Hub
    pub async fn consume_forecasts(&self, forecasts: Vec<Forecast>) -> Result<()> {
        let mut cache = self.forecast_cache.write().await;

        for forecast in &forecasts {
            let key = format!(
                "{}:{}:{}",
                forecast.metric_name,
                forecast.service.as_deref().unwrap_or("*"),
                forecast.model.as_deref().unwrap_or("*")
            );
            cache.insert(key, forecast.clone());
        }

        let mut count = self.forecasts_fetched.write().await;
        *count += forecasts.len() as u64;

        debug!(
            "Cached {} forecasts, total cache size: {}",
            forecasts.len(),
            cache.len()
        );
        Ok(())
    }

    /// Get cached forecast for a metric
    pub async fn get_forecast(
        &self,
        metric_name: &str,
        service: Option<&str>,
        model: Option<&str>,
    ) -> Option<Forecast> {
        let cache = self.forecast_cache.read().await;

        let key = format!(
            "{}:{}:{}",
            metric_name,
            service.unwrap_or("*"),
            model.unwrap_or("*")
        );

        cache.get(&key).cloned()
    }

    /// Enrich anomaly detection with baseline context
    ///
    /// This method provides baseline data to detection algorithms
    /// without modifying the algorithms themselves.
    pub async fn get_baseline_context(
        &self,
        metric_name: &str,
        service: Option<&str>,
        model: Option<&str>,
    ) -> Option<BaselineContext> {
        let baseline = self.get_baseline(metric_name, service, model).await?;
        let forecast = self.get_forecast(metric_name, service, model).await;

        Some(BaselineContext {
            baseline,
            forecast,
        })
    }

    /// Get adapter statistics
    pub async fn stats(&self) -> AnalyticsHubAdapterStats {
        AnalyticsHubAdapterStats {
            metrics_fetched: *self.metrics_fetched.read().await,
            baselines_fetched: *self.baselines_fetched.read().await,
            forecasts_fetched: *self.forecasts_fetched.read().await,
            baselines_cached: self.baseline_cache.read().await.len(),
            forecasts_cached: self.forecast_cache.read().await.len(),
        }
    }

    /// Clear all caches
    pub async fn clear_caches(&self) {
        self.baseline_cache.write().await.clear();
        self.forecast_cache.write().await.clear();
        debug!("Analytics-Hub adapter caches cleared");
    }
}

/// Context for baseline-aware anomaly detection
#[derive(Debug, Clone)]
pub struct BaselineContext {
    pub baseline: MetricBaseline,
    pub forecast: Option<Forecast>,
}

impl BaselineContext {
    /// Check if a value is anomalous given the baseline
    pub fn is_anomalous(&self, value: f64, threshold_sigma: f64) -> bool {
        self.baseline.exceeds_threshold(value, threshold_sigma)
    }

    /// Get the nearest forecast prediction for a timestamp
    pub fn get_prediction_at(&self, timestamp: DateTime<Utc>) -> Option<&PredictionPoint> {
        self.forecast.as_ref().and_then(|f| {
            f.predictions
                .iter()
                .min_by_key(|p| (p.timestamp - timestamp).num_seconds().abs())
        })
    }
}

/// Statistics for Analytics-Hub adapter
#[derive(Debug, Clone)]
pub struct AnalyticsHubAdapterStats {
    pub metrics_fetched: u64,
    pub baselines_fetched: u64,
    pub forecasts_fetched: u64,
    pub baselines_cached: usize,
    pub forecasts_cached: usize,
}

#[async_trait]
impl UpstreamAdapter for AnalyticsHubAdapter {
    fn name(&self) -> &'static str {
        "llm-analytics-hub"
    }

    async fn health_check(&self) -> Result<()> {
        let connected = *self.connected.read().await;
        if connected {
            Ok(())
        } else {
            Err(llm_sentinel_core::Error::connection(
                "Analytics-Hub adapter not connected",
            ))
        }
    }

    async fn connect(&mut self) -> Result<()> {
        info!(
            "Connecting to Analytics-Hub at {}",
            self.config.api_endpoint
        );

        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Analytics-Hub adapter connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Analytics-Hub");

        let mut connected = self.connected.write().await;
        *connected = false;

        let stats = self.stats().await;
        info!(
            "Analytics-Hub adapter disconnected. Stats: metrics={}, baselines={}, forecasts={}",
            stats.metrics_fetched, stats.baselines_fetched, stats.forecasts_fetched
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_baseline() -> MetricBaseline {
        MetricBaseline {
            metric_name: "latency_ms".to_string(),
            mean: 150.0,
            stddev: 30.0,
            sample_count: 1000,
            time_range_start: Utc::now() - chrono::Duration::hours(24),
            time_range_end: Utc::now(),
            updated_at: Utc::now(),
            tags: {
                let mut tags = HashMap::new();
                tags.insert("service".to_string(), "chat-api".to_string());
                tags.insert("model".to_string(), "gpt-4".to_string());
                tags
            },
        }
    }

    fn create_test_forecast() -> Forecast {
        Forecast {
            metric_name: "latency_ms".to_string(),
            model: Some("gpt-4".to_string()),
            service: Some("chat-api".to_string()),
            predictions: vec![
                PredictionPoint {
                    timestamp: Utc::now() + chrono::Duration::hours(1),
                    value: 160.0,
                    confidence: 0.85,
                    upper_bound: 200.0,
                    lower_bound: 120.0,
                },
                PredictionPoint {
                    timestamp: Utc::now() + chrono::Duration::hours(2),
                    value: 155.0,
                    confidence: 0.80,
                    upper_bound: 195.0,
                    lower_bound: 115.0,
                },
            ],
            generated_at: Utc::now(),
            method: "arima".to_string(),
        }
    }

    #[test]
    fn test_time_window_conversion() {
        assert_eq!(TimeWindow::OneMinute.to_seconds(), 60);
        assert_eq!(TimeWindow::OneHour.to_seconds(), 3600);
        assert_eq!(TimeWindow::OneDay.to_seconds(), 86400);
    }

    #[test]
    fn test_baseline_z_score() {
        let baseline = create_test_baseline();

        // Value at mean should have z-score of 0
        assert!((baseline.z_score(150.0)).abs() < 0.001);

        // Value 1 stddev above mean
        assert!((baseline.z_score(180.0) - 1.0).abs() < 0.001);

        // Value 2 stddev below mean
        assert!((baseline.z_score(90.0) - (-2.0)).abs() < 0.001);
    }

    #[test]
    fn test_baseline_threshold() {
        let baseline = create_test_baseline();

        // 3 sigma = 150 + 90 = 240
        assert!(!baseline.exceeds_threshold(200.0, 3.0));
        assert!(baseline.exceeds_threshold(250.0, 3.0));
        assert!(baseline.exceeds_threshold(50.0, 3.0)); // Below
    }

    #[test]
    fn test_prediction_bounds() {
        let prediction = PredictionPoint {
            timestamp: Utc::now(),
            value: 100.0,
            confidence: 0.9,
            upper_bound: 120.0,
            lower_bound: 80.0,
        };

        assert!(prediction.is_within_bounds(100.0));
        assert!(prediction.is_within_bounds(80.0));
        assert!(prediction.is_within_bounds(120.0));
        assert!(!prediction.is_within_bounds(79.0));
        assert!(!prediction.is_within_bounds(121.0));
    }

    #[test]
    fn test_prediction_deviation() {
        let prediction = PredictionPoint {
            timestamp: Utc::now(),
            value: 100.0,
            confidence: 0.9,
            upper_bound: 120.0,
            lower_bound: 80.0,
        };

        assert!((prediction.deviation(110.0) - 10.0).abs() < 0.001);
        assert!((prediction.relative_deviation(110.0) - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_statistical_measures_outlier() {
        let measures = StatisticalMeasures {
            avg: 100.0,
            min: 50.0,
            max: 200.0,
            p50: 95.0,
            p95: 180.0,
            p99: 195.0,
            stddev: Some(20.0),
            count: 1000,
            sum: 100000.0,
        };

        assert!(!measures.is_outlier(100.0)); // At mean
        assert!(!measures.is_outlier(150.0)); // 2.5 sigma
        assert!(measures.is_outlier(170.0)); // 3.5 sigma
    }

    #[tokio::test]
    async fn test_baseline_caching() {
        let adapter = AnalyticsHubAdapter::new(AnalyticsHubConfig::default());
        let baseline = create_test_baseline();

        adapter.consume_baselines(vec![baseline.clone()]).await.unwrap();

        let cached = adapter
            .get_baseline("latency_ms", Some("chat-api"), Some("gpt-4"))
            .await;

        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert_eq!(cached.mean, 150.0);
    }

    #[tokio::test]
    async fn test_forecast_caching() {
        let adapter = AnalyticsHubAdapter::new(AnalyticsHubConfig::default());
        let forecast = create_test_forecast();

        adapter.consume_forecasts(vec![forecast.clone()]).await.unwrap();

        let cached = adapter
            .get_forecast("latency_ms", Some("chat-api"), Some("gpt-4"))
            .await;

        assert!(cached.is_some());
        assert_eq!(cached.unwrap().predictions.len(), 2);
    }

    #[tokio::test]
    async fn test_baseline_context() {
        let adapter = AnalyticsHubAdapter::new(AnalyticsHubConfig::default());

        adapter
            .consume_baselines(vec![create_test_baseline()])
            .await
            .unwrap();
        adapter
            .consume_forecasts(vec![create_test_forecast()])
            .await
            .unwrap();

        let context = adapter
            .get_baseline_context("latency_ms", Some("chat-api"), Some("gpt-4"))
            .await;

        assert!(context.is_some());
        let context = context.unwrap();
        assert!(context.forecast.is_some());
        assert!(!context.is_anomalous(150.0, 3.0)); // Within baseline
        assert!(context.is_anomalous(300.0, 3.0)); // Anomalous
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        let mut adapter = AnalyticsHubAdapter::new(AnalyticsHubConfig::default());

        assert!(adapter.health_check().await.is_err());

        adapter.connect().await.unwrap();
        assert!(adapter.health_check().await.is_ok());

        adapter.disconnect().await.unwrap();
        assert!(adapter.health_check().await.is_err());
    }

    #[tokio::test]
    async fn test_cache_clearing() {
        let adapter = AnalyticsHubAdapter::new(AnalyticsHubConfig::default());

        adapter
            .consume_baselines(vec![create_test_baseline()])
            .await
            .unwrap();

        let stats = adapter.stats().await;
        assert_eq!(stats.baselines_cached, 1);

        adapter.clear_caches().await;

        let stats = adapter.stats().await;
        assert_eq!(stats.baselines_cached, 0);
    }
}
