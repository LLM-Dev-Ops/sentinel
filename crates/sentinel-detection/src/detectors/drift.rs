//! Drift Detection Agent for distributional drift in telemetry metrics.
//!
//! Detects gradual or sudden distributional drift in model behavior or system
//! performance over time by comparing rolling windows of telemetry data.
//!
//! # Classification
//! - Type: DETECTION
//! - decision_type: "drift_detection"
//!
//! # Detection Method
//! Uses Population Stability Index (PSI) to compare reference and current distributions:
//! - PSI < 0.1: No significant drift
//! - PSI 0.1-0.25: Moderate drift (warning)
//! - PSI > 0.25: Significant drift (alert)
//!
//! # Non-Responsibilities
//! This agent DOES NOT:
//! - Perform remediation
//! - Trigger retries
//! - Modify routing or thresholds
//! - Connect to databases directly

use crate::{
    baseline::{BaselineKey, BaselineManager},
    detectors::DetectionConfig,
    stats::RollingWindow,
    Detector, DetectorStats, DetectorType,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use llm_sentinel_core::{
    events::{AnomalyContext, AnomalyDetails, AnomalyEvent, TelemetryEvent},
    types::{AnomalyType, DetectionMethod, Severity},
    Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, info, warn};

/// Agent identifier for DecisionEvents
pub const AGENT_ID: &str = "drift-detection-agent";

/// Agent version
pub const AGENT_VERSION: &str = "1.0.0";

/// Drift detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftConfig {
    /// PSI threshold for moderate drift (warning)
    pub psi_moderate_threshold: f64,

    /// PSI threshold for significant drift (alert)
    pub psi_significant_threshold: f64,

    /// Size of reference window (historical data)
    pub reference_window_size: usize,

    /// Size of current window (recent data)
    pub current_window_size: usize,

    /// Minimum samples required before drift detection
    pub min_samples: usize,

    /// Number of bins for PSI histogram calculation
    pub psi_bins: usize,

    /// Epsilon for smoothing zero bins (prevents division by zero)
    pub epsilon: f64,

    /// Metrics to monitor for drift
    pub monitored_metrics: Vec<DriftMetric>,

    /// Common detection config
    pub detection: DetectionConfig,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            psi_moderate_threshold: 0.1,
            psi_significant_threshold: 0.25,
            reference_window_size: 1000,
            current_window_size: 100,
            min_samples: 50,
            psi_bins: 10,
            epsilon: 0.0001,
            monitored_metrics: vec![
                DriftMetric::Latency,
                DriftMetric::Cost,
                DriftMetric::TokenUsage,
                DriftMetric::ErrorRate,
            ],
            detection: DetectionConfig::default(),
        }
    }
}

/// Metrics that can be monitored for drift
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftMetric {
    /// Latency in milliseconds
    Latency,
    /// Cost in USD
    Cost,
    /// Total token usage
    TokenUsage,
    /// Error rate (0 or 1 per event)
    ErrorRate,
}

impl DriftMetric {
    /// Get the metric name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            DriftMetric::Latency => "latency_ms",
            DriftMetric::Cost => "cost_usd",
            DriftMetric::TokenUsage => "total_tokens",
            DriftMetric::ErrorRate => "error_rate",
        }
    }

    /// Extract the metric value from a telemetry event
    pub fn extract(&self, event: &TelemetryEvent) -> f64 {
        match self {
            DriftMetric::Latency => event.latency_ms,
            DriftMetric::Cost => event.cost_usd,
            DriftMetric::TokenUsage => event.total_tokens() as f64,
            DriftMetric::ErrorRate => event.error_rate(),
        }
    }

    /// Map to appropriate AnomalyType
    pub fn to_anomaly_type(&self) -> AnomalyType {
        match self {
            DriftMetric::Latency => AnomalyType::InputDrift,
            DriftMetric::Cost => AnomalyType::CostAnomaly,
            DriftMetric::TokenUsage => AnomalyType::InputDrift,
            DriftMetric::ErrorRate => AnomalyType::QualityDegradation,
        }
    }
}

/// Key for drift detection state
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DriftKey {
    /// Service identifier
    pub service: llm_sentinel_core::types::ServiceId,
    /// Model identifier
    pub model: llm_sentinel_core::types::ModelId,
    /// Metric being tracked
    pub metric: String,
}

impl DriftKey {
    /// Create a new drift key
    pub fn new(
        service: llm_sentinel_core::types::ServiceId,
        model: llm_sentinel_core::types::ModelId,
        metric: DriftMetric,
    ) -> Self {
        Self {
            service,
            model,
            metric: metric.as_str().to_string(),
        }
    }
}

/// State for tracking drift on a specific metric
#[derive(Debug)]
pub struct DriftState {
    /// Reference window (historical baseline)
    reference_window: RollingWindow,
    /// Current window (recent observations)
    current_window: RollingWindow,
    /// Last drift detection timestamp
    last_detection: Option<DateTime<Utc>>,
    /// Total samples processed
    samples_processed: u64,
    /// Number of drifts detected
    drifts_detected: u64,
}

impl DriftState {
    /// Create a new drift state
    pub fn new(reference_size: usize, current_size: usize) -> Self {
        Self {
            reference_window: RollingWindow::new(reference_size),
            current_window: RollingWindow::new(current_size),
            last_detection: None,
            samples_processed: 0,
            drifts_detected: 0,
        }
    }

    /// Add a value to the windows
    pub fn push(&mut self, value: f64) {
        self.samples_processed += 1;

        // If reference window is full, push to current window
        // Otherwise, build up reference window first
        if self.reference_window.is_full() {
            self.current_window.push(value);
        } else {
            self.reference_window.push(value);
        }
    }

    /// Check if we have enough data for drift detection
    pub fn has_sufficient_data(&self, min_samples: usize) -> bool {
        self.reference_window.len() >= min_samples && self.current_window.len() >= min_samples
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.reference_window.clear();
        self.current_window.clear();
        self.last_detection = None;
        self.samples_processed = 0;
        self.drifts_detected = 0;
    }

    /// Rotate windows: current becomes reference, current is cleared
    pub fn rotate_windows(&mut self) {
        // Move current window data to reference
        let current_data: Vec<f64> = self.current_window.data().to_vec();
        self.reference_window.clear();
        for value in current_data {
            self.reference_window.push(value);
        }
        self.current_window.clear();
    }
}

/// Drift detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftResult {
    /// PSI score
    pub psi_score: f64,
    /// Whether drift was detected
    pub drift_detected: bool,
    /// Drift direction (positive = increase, negative = decrease)
    pub drift_direction: DriftDirection,
    /// Reference window statistics
    pub reference_mean: f64,
    pub reference_std: f64,
    /// Current window statistics
    pub current_mean: f64,
    pub current_std: f64,
    /// Sample counts
    pub reference_samples: usize,
    pub current_samples: usize,
}

/// Direction of drift
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftDirection {
    /// Values are increasing
    Positive,
    /// Values are decreasing
    Negative,
    /// No significant direction
    Neutral,
}

impl DriftDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            DriftDirection::Positive => "positive",
            DriftDirection::Negative => "negative",
            DriftDirection::Neutral => "neutral",
        }
    }
}

/// Drift Detection Agent
///
/// Detects distributional drift by comparing rolling windows of telemetry
/// using Population Stability Index (PSI).
///
/// # Example
/// ```ignore
/// let config = DriftConfig::default();
/// let baseline_manager = Arc::new(BaselineManager::new(1000));
/// let detector = DriftDetector::new(config, baseline_manager);
///
/// // Process events
/// for event in events {
///     if let Some(anomaly) = detector.detect(&event).await? {
///         // Handle drift detection
///     }
/// }
/// ```
pub struct DriftDetector {
    config: DriftConfig,
    baseline_manager: Arc<BaselineManager>,
    states: Arc<DashMap<DriftKey, DriftState>>,
    stats: DetectorStats,
}

impl std::fmt::Debug for DriftDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriftDetector")
            .field("config", &self.config)
            .field("states_count", &self.states.len())
            .field("stats", &self.stats)
            .finish()
    }
}

impl DriftDetector {
    /// Create a new drift detector
    pub fn new(config: DriftConfig, baseline_manager: Arc<BaselineManager>) -> Self {
        info!(
            agent_id = AGENT_ID,
            agent_version = AGENT_VERSION,
            psi_moderate = config.psi_moderate_threshold,
            psi_significant = config.psi_significant_threshold,
            reference_window = config.reference_window_size,
            current_window = config.current_window_size,
            "Creating drift detection agent"
        );

        Self {
            config,
            baseline_manager,
            states: Arc::new(DashMap::new()),
            stats: DetectorStats::empty(),
        }
    }

    /// Calculate Population Stability Index (PSI)
    ///
    /// PSI measures the shift between two distributions by comparing
    /// the proportion of observations in each bin.
    ///
    /// Formula: PSI = Σ (Current% - Reference%) * ln(Current% / Reference%)
    fn calculate_psi(&self, reference: &[f64], current: &[f64]) -> f64 {
        if reference.is_empty() || current.is_empty() {
            return 0.0;
        }

        // Find global min/max for binning
        let all_values: Vec<f64> = reference.iter().chain(current.iter()).copied().collect();
        let min_val = all_values
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .copied()
            .unwrap_or(0.0);
        let max_val = all_values
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .copied()
            .unwrap_or(1.0);

        // Handle edge case where all values are the same
        if (max_val - min_val).abs() < self.config.epsilon {
            return 0.0;
        }

        let bin_width = (max_val - min_val) / self.config.psi_bins as f64;

        // Count observations in each bin
        let mut ref_counts = vec![0usize; self.config.psi_bins];
        let mut cur_counts = vec![0usize; self.config.psi_bins];

        for &val in reference {
            let bin = ((val - min_val) / bin_width).floor() as usize;
            let bin = bin.min(self.config.psi_bins - 1);
            ref_counts[bin] += 1;
        }

        for &val in current {
            let bin = ((val - min_val) / bin_width).floor() as usize;
            let bin = bin.min(self.config.psi_bins - 1);
            cur_counts[bin] += 1;
        }

        // Calculate PSI with epsilon smoothing
        let ref_total = reference.len() as f64;
        let cur_total = current.len() as f64;
        let mut psi = 0.0;

        for i in 0..self.config.psi_bins {
            let ref_pct = (ref_counts[i] as f64 / ref_total).max(self.config.epsilon);
            let cur_pct = (cur_counts[i] as f64 / cur_total).max(self.config.epsilon);

            psi += (cur_pct - ref_pct) * (cur_pct / ref_pct).ln();
        }

        psi.abs()
    }

    /// Convert PSI score to confidence value (0.0-1.0)
    fn psi_to_confidence(&self, psi: f64) -> f64 {
        // Map PSI to confidence:
        // PSI 0.0 -> confidence 0.0 (no drift)
        // PSI 0.1 -> confidence ~0.5 (moderate)
        // PSI 0.25+ -> confidence ~0.9+ (high)
        let normalized = (psi / self.config.psi_significant_threshold).min(1.0);
        // Apply sigmoid-like scaling
        (1.0 / (1.0 + (-5.0 * (normalized - 0.5)).exp())).min(0.99)
    }

    /// Determine severity based on PSI score
    fn psi_to_severity(&self, psi: f64) -> Severity {
        if psi >= self.config.psi_significant_threshold * 2.0 {
            Severity::Critical
        } else if psi >= self.config.psi_significant_threshold {
            Severity::High
        } else if psi >= self.config.psi_moderate_threshold {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Detect drift for a specific metric
    fn detect_metric_drift(
        &self,
        event: &TelemetryEvent,
        metric: DriftMetric,
    ) -> Result<Option<(DriftResult, AnomalyEvent)>> {
        let key = DriftKey::new(event.service_name.clone(), event.model.clone(), metric);
        let value = metric.extract(event);

        // Get or create drift state
        let mut state = self.states.entry(key.clone()).or_insert_with(|| {
            DriftState::new(self.config.reference_window_size, self.config.current_window_size)
        });

        // Add value to windows
        state.push(value);

        // Check if we have enough data
        if !state.has_sufficient_data(self.config.min_samples) {
            debug!(
                service = %event.service_name,
                model = %event.model,
                metric = metric.as_str(),
                reference_samples = state.reference_window.len(),
                current_samples = state.current_window.len(),
                min_required = self.config.min_samples,
                "Insufficient samples for drift detection"
            );
            return Ok(None);
        }

        // Calculate PSI
        let reference_data = state.reference_window.data();
        let current_data = state.current_window.data();
        let psi = self.calculate_psi(reference_data, current_data);

        // Calculate statistics
        let reference_mean = crate::stats::mean(reference_data);
        let reference_std = crate::stats::std_dev(reference_data);
        let current_mean = crate::stats::mean(current_data);
        let current_std = crate::stats::std_dev(current_data);

        // Determine drift direction
        let drift_direction = if (current_mean - reference_mean).abs() < self.config.epsilon {
            DriftDirection::Neutral
        } else if current_mean > reference_mean {
            DriftDirection::Positive
        } else {
            DriftDirection::Negative
        };

        // Check if drift threshold exceeded
        let drift_detected = psi >= self.config.psi_moderate_threshold;

        let result = DriftResult {
            psi_score: psi,
            drift_detected,
            drift_direction,
            reference_mean,
            reference_std,
            current_mean,
            current_std,
            reference_samples: reference_data.len(),
            current_samples: current_data.len(),
        };

        if !drift_detected {
            return Ok(None);
        }

        // Create anomaly event
        let severity = self.psi_to_severity(psi);
        let confidence = self.psi_to_confidence(psi);

        let anomaly = AnomalyEvent::new(
            severity,
            metric.to_anomaly_type(),
            event.service_name.clone(),
            event.model.clone(),
            DetectionMethod::Psi,
            confidence,
            AnomalyDetails {
                metric: metric.as_str().to_string(),
                value: current_mean,
                baseline: reference_mean,
                threshold: self.config.psi_moderate_threshold,
                deviation_sigma: Some((current_mean - reference_mean) / reference_std.max(0.001)),
                additional: {
                    let mut map = HashMap::new();
                    map.insert("psi_score".to_string(), serde_json::json!(psi));
                    map.insert(
                        "drift_direction".to_string(),
                        serde_json::json!(drift_direction.as_str()),
                    );
                    map.insert(
                        "reference_samples".to_string(),
                        serde_json::json!(result.reference_samples),
                    );
                    map.insert(
                        "current_samples".to_string(),
                        serde_json::json!(result.current_samples),
                    );
                    map.insert("reference_std".to_string(), serde_json::json!(reference_std));
                    map.insert("current_std".to_string(), serde_json::json!(current_std));
                    map.insert("agent_id".to_string(), serde_json::json!(AGENT_ID));
                    map.insert("agent_version".to_string(), serde_json::json!(AGENT_VERSION));
                    map
                },
            },
            AnomalyContext {
                trace_id: event.trace_id.clone(),
                user_id: event.metadata.get("user_id").cloned(),
                region: event.metadata.get("region").cloned(),
                time_window: format!(
                    "reference_{}_current_{}",
                    result.reference_samples, result.current_samples
                ),
                sample_count: result.reference_samples + result.current_samples,
                additional: {
                    let mut map = HashMap::new();
                    map.insert("decision_type".to_string(), "drift_detection".to_string());
                    map
                },
            },
        )
        .with_root_cause(format!(
            "Distributional drift detected in {} (PSI: {:.4}, direction: {})",
            metric.as_str(),
            psi,
            drift_direction.as_str()
        ))
        .with_remediation("Review recent model or system changes")
        .with_remediation("Compare input distributions across time periods")
        .with_remediation("Check for data pipeline issues");

        // Update state
        state.drifts_detected += 1;
        state.last_detection = Some(Utc::now());

        // Rotate windows after drift detection to establish new baseline
        state.rotate_windows();

        info!(
            agent_id = AGENT_ID,
            service = %event.service_name,
            model = %event.model,
            metric = metric.as_str(),
            psi = psi,
            direction = drift_direction.as_str(),
            severity = %severity,
            confidence = confidence,
            "Drift detected"
        );

        Ok(Some((result, anomaly)))
    }

    /// Clone for detection (interior mutability pattern)
    fn clone_for_detection(&self) -> Self {
        Self {
            config: self.config.clone(),
            baseline_manager: Arc::clone(&self.baseline_manager),
            states: Arc::clone(&self.states),
            stats: self.stats.clone(),
        }
    }

    /// Get drift state for inspection
    pub fn get_state(&self, key: &DriftKey) -> Option<DriftStateSnapshot> {
        self.states.get(key).map(|state| DriftStateSnapshot {
            reference_samples: state.reference_window.len(),
            current_samples: state.current_window.len(),
            samples_processed: state.samples_processed,
            drifts_detected: state.drifts_detected,
            last_detection: state.last_detection,
            reference_mean: state.reference_window.mean(),
            current_mean: state.current_window.mean(),
        })
    }

    /// Get all drift keys
    pub fn get_keys(&self) -> Vec<DriftKey> {
        self.states.iter().map(|e| e.key().clone()).collect()
    }
}

/// Snapshot of drift state for inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftStateSnapshot {
    pub reference_samples: usize,
    pub current_samples: usize,
    pub samples_processed: u64,
    pub drifts_detected: u64,
    pub last_detection: Option<DateTime<Utc>>,
    pub reference_mean: f64,
    pub current_mean: f64,
}

#[async_trait]
impl Detector for DriftDetector {
    async fn detect(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        let detector = self.clone_for_detection();

        // Check each monitored metric for drift
        // Return the first detected drift (highest severity)
        let mut highest_severity_anomaly: Option<AnomalyEvent> = None;

        for metric in &detector.config.monitored_metrics {
            match detector.detect_metric_drift(event, *metric) {
                Ok(Some((result, anomaly))) => {
                    debug!(
                        metric = metric.as_str(),
                        psi = result.psi_score,
                        "Drift detected for metric"
                    );

                    // Keep track of highest severity
                    match &highest_severity_anomaly {
                        None => highest_severity_anomaly = Some(anomaly),
                        Some(existing) if anomaly.severity > existing.severity => {
                            highest_severity_anomaly = Some(anomaly);
                        }
                        _ => {}
                    }
                }
                Ok(None) => {
                    // No drift for this metric
                }
                Err(e) => {
                    warn!(
                        metric = metric.as_str(),
                        error = %e,
                        "Error detecting drift for metric"
                    );
                    metrics::counter!(
                        "sentinel_drift_detection_errors_total",
                        "metric" => metric.as_str().to_string()
                    )
                    .increment(1);
                }
            }
        }

        // Record metrics
        if let Some(ref anomaly) = highest_severity_anomaly {
            metrics::counter!(
                "sentinel_drift_detected_total",
                "service" => anomaly.service_name.to_string(),
                "model" => anomaly.model.to_string(),
                "severity" => anomaly.severity.to_string()
            )
            .increment(1);
        }

        Ok(highest_severity_anomaly)
    }

    fn name(&self) -> &str {
        "drift"
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Statistical
    }

    async fn update(&mut self, event: &TelemetryEvent) -> Result<()> {
        if !self.config.detection.update_baseline {
            return Ok(());
        }

        // Update baseline manager for each metric
        for metric in &self.config.monitored_metrics {
            let key = BaselineKey::new(
                event.service_name.clone(),
                event.model.clone(),
                metric.as_str(),
            );
            let value = metric.extract(event);
            self.baseline_manager.update(key, value)?;
        }

        Ok(())
    }

    async fn reset(&mut self) -> Result<()> {
        self.states.clear();
        self.stats = DetectorStats::empty();
        info!(
            agent_id = AGENT_ID,
            "Drift detector reset"
        );
        Ok(())
    }

    fn stats(&self) -> DetectorStats {
        let mut stats = self.stats.clone();

        // Aggregate stats from all states
        let total_drifts: u64 = self.states.iter().map(|e| e.drifts_detected).sum();
        let total_samples: u64 = self.states.iter().map(|e| e.samples_processed).sum();

        stats.events_processed = total_samples;
        stats.anomalies_detected = total_drifts;
        stats.detection_rate = if total_samples > 0 {
            total_drifts as f64 / total_samples as f64
        } else {
            0.0
        };

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::BaselineManager;
    use llm_sentinel_core::{
        events::{PromptInfo, ResponseInfo},
        types::{ModelId, ServiceId},
    };

    fn create_test_event(latency: f64, cost: f64, tokens: u32) -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test-service"),
            ModelId::new("gpt-4"),
            PromptInfo {
                text: "test".to_string(),
                tokens: tokens / 2,
                embedding: None,
            },
            ResponseInfo {
                text: "response".to_string(),
                tokens: tokens / 2,
                finish_reason: "stop".to_string(),
                embedding: None,
            },
            latency,
            cost,
        )
    }

    #[test]
    fn test_psi_calculation_no_drift() {
        let baseline_manager = Arc::new(BaselineManager::new(100));
        let config = DriftConfig {
            reference_window_size: 100,
            current_window_size: 50,
            min_samples: 10,
            ..Default::default()
        };
        let detector = DriftDetector::new(config, baseline_manager);

        // Same distribution
        let reference: Vec<f64> = (0..100).map(|x| 100.0 + (x as f64 * 0.1)).collect();
        let current: Vec<f64> = (0..50).map(|x| 100.0 + (x as f64 * 0.1)).collect();

        let psi = detector.calculate_psi(&reference, &current);
        assert!(psi < 0.1, "PSI should be low for similar distributions");
    }

    #[test]
    fn test_psi_calculation_with_drift() {
        let baseline_manager = Arc::new(BaselineManager::new(100));
        let config = DriftConfig {
            reference_window_size: 100,
            current_window_size: 50,
            min_samples: 10,
            ..Default::default()
        };
        let detector = DriftDetector::new(config, baseline_manager);

        // Different distributions
        let reference: Vec<f64> = (0..100).map(|x| 100.0 + (x as f64 * 0.1)).collect();
        let current: Vec<f64> = (0..50).map(|x| 200.0 + (x as f64 * 0.1)).collect();

        let psi = detector.calculate_psi(&reference, &current);
        assert!(psi > 0.25, "PSI should be high for different distributions");
    }

    #[test]
    fn test_drift_direction() {
        let baseline_manager = Arc::new(BaselineManager::new(100));
        let config = DriftConfig {
            reference_window_size: 20,
            current_window_size: 10,
            min_samples: 10,
            ..Default::default()
        };
        let detector = DriftDetector::new(config, baseline_manager);

        // Build reference window with low values
        for i in 0..20 {
            let event = create_test_event(100.0 + i as f64, 0.01, 100);
            let key = DriftKey::new(
                event.service_name.clone(),
                event.model.clone(),
                DriftMetric::Latency,
            );
            let mut state = detector.states.entry(key).or_insert_with(|| {
                DriftState::new(
                    detector.config.reference_window_size,
                    detector.config.current_window_size,
                )
            });
            state.push(event.latency_ms);
        }

        // Add high values to current window
        for i in 0..10 {
            let event = create_test_event(500.0 + i as f64, 0.01, 100);
            let key = DriftKey::new(
                event.service_name.clone(),
                event.model.clone(),
                DriftMetric::Latency,
            );
            let mut state = detector.states.get_mut(&key).unwrap();
            state.push(event.latency_ms);
        }

        // Check state
        let key = DriftKey::new(
            ServiceId::new("test-service"),
            ModelId::new("gpt-4"),
            DriftMetric::Latency,
        );
        let state = detector.states.get(&key).unwrap();
        let ref_mean = state.reference_window.mean();
        let cur_mean = state.current_window.mean();

        assert!(cur_mean > ref_mean, "Current mean should be higher");
    }

    #[tokio::test]
    async fn test_drift_detection_integration() {
        let baseline_manager = Arc::new(BaselineManager::new(100));
        let config = DriftConfig {
            reference_window_size: 20,
            current_window_size: 10,
            min_samples: 10,
            psi_moderate_threshold: 0.1,
            psi_significant_threshold: 0.25,
            monitored_metrics: vec![DriftMetric::Latency],
            ..Default::default()
        };
        let detector = DriftDetector::new(config, baseline_manager);

        // Build baseline with low latency
        for i in 0..20 {
            let event = create_test_event(100.0 + (i as f64 * 0.5), 0.01, 100);
            let _ = detector.detect(&event).await;
        }

        // Inject drift with high latency
        let mut drift_detected = false;
        for i in 0..15 {
            let event = create_test_event(500.0 + (i as f64 * 2.0), 0.01, 100);
            if let Ok(Some(anomaly)) = detector.detect(&event).await {
                drift_detected = true;
                assert_eq!(anomaly.detection_method, DetectionMethod::Psi);
                assert!(anomaly.confidence > 0.0);
                break;
            }
        }

        assert!(drift_detected, "Drift should have been detected");
    }

    #[tokio::test]
    async fn test_detector_reset() {
        let baseline_manager = Arc::new(BaselineManager::new(100));
        let config = DriftConfig::default();
        let mut detector = DriftDetector::new(config, baseline_manager);

        // Add some data
        for i in 0..50 {
            let event = create_test_event(100.0 + i as f64, 0.01, 100);
            let _ = detector.detect(&event).await;
        }

        assert!(!detector.states.is_empty());

        // Reset
        detector.reset().await.unwrap();

        assert!(detector.states.is_empty());
    }

    #[test]
    fn test_psi_to_confidence() {
        let baseline_manager = Arc::new(BaselineManager::new(100));
        let config = DriftConfig::default();
        let detector = DriftDetector::new(config, baseline_manager);

        // Low PSI -> low confidence
        let low_conf = detector.psi_to_confidence(0.05);
        assert!(low_conf < 0.5);

        // Moderate PSI -> moderate confidence
        let mod_conf = detector.psi_to_confidence(0.15);
        assert!(mod_conf > 0.5 && mod_conf < 0.9);

        // High PSI -> high confidence
        let high_conf = detector.psi_to_confidence(0.30);
        assert!(high_conf > 0.8);
    }

    #[test]
    fn test_psi_to_severity() {
        let baseline_manager = Arc::new(BaselineManager::new(100));
        let config = DriftConfig::default();
        let detector = DriftDetector::new(config, baseline_manager);

        assert_eq!(detector.psi_to_severity(0.05), Severity::Low);
        assert_eq!(detector.psi_to_severity(0.15), Severity::Medium);
        assert_eq!(detector.psi_to_severity(0.30), Severity::High);
        assert_eq!(detector.psi_to_severity(0.60), Severity::Critical);
    }
}
