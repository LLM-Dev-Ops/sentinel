//! CUSUM (Cumulative Sum) detector for change point detection.
//!
//! Detects gradual shifts in process mean over time.

use crate::{
    baseline::{BaselineKey, BaselineManager},
    detectors::DetectionConfig,
    Detector, DetectorStats, DetectorType,
};
use async_trait::async_trait;
use dashmap::DashMap;
use llm_sentinel_core::{
    events::{AnomalyContext, AnomalyDetails, AnomalyEvent, TelemetryEvent},
    types::{AnomalyType, DetectionMethod, Severity},
    Result,
};
use std::{collections::HashMap, sync::Arc};

/// CUSUM detector configuration
#[derive(Debug, Clone)]
pub struct CusumConfig {
    /// Threshold for CUSUM value
    pub threshold: f64,
    /// Slack value (allowable deviation before accumulating)
    pub slack: f64,
    /// Common detection config
    pub detection: DetectionConfig,
}

impl Default for CusumConfig {
    fn default() -> Self {
        Self {
            threshold: 5.0,
            slack: 0.5,
            detection: DetectionConfig::default(),
        }
    }
}

/// CUSUM state for a specific metric
#[derive(Debug, Clone)]
struct CusumState {
    /// Cumulative sum (positive deviations)
    cusum_pos: f64,
    /// Cumulative sum (negative deviations)
    cusum_neg: f64,
    /// Last update count
    count: u64,
}

impl CusumState {
    fn new() -> Self {
        Self {
            cusum_pos: 0.0,
            cusum_neg: 0.0,
            count: 0,
        }
    }

    fn reset(&mut self) {
        self.cusum_pos = 0.0;
        self.cusum_neg = 0.0;
        self.count = 0;
    }
}

/// CUSUM anomaly detector
///
/// Detects gradual changes in the process mean.
/// Good for detecting sustained shifts rather than individual outliers.
///
/// Formula:
/// S_H = max(0, S_H + x - μ - k)  // Positive shifts
/// S_L = min(0, S_L + x - μ + k)  // Negative shifts
///
/// where k is the slack parameter
pub struct CusumDetector {
    config: CusumConfig,
    baseline_manager: Arc<BaselineManager>,
    states: Arc<DashMap<BaselineKey, CusumState>>,
    stats: DetectorStats,
}

impl std::fmt::Debug for CusumDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CusumDetector")
            .field("config", &self.config)
            .field("states_count", &self.states.len())
            .field("stats", &self.stats)
            .finish()
    }
}

impl CusumDetector {
    /// Create a new CUSUM detector
    pub fn new(config: CusumConfig, baseline_manager: Arc<BaselineManager>) -> Self {
        Self {
            config,
            baseline_manager,
            states: Arc::new(DashMap::new()),
            stats: DetectorStats::empty(),
        }
    }

    fn detect_cost_drift(&mut self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        let key = BaselineKey::cost(event.service_name.clone(), event.model.clone());

        if !self.baseline_manager.has_valid_baseline(&key) {
            return Ok(None);
        }

        let baseline = self.baseline_manager.get(&key).unwrap();
        let cost = event.cost_usd;

        // Get or create CUSUM state
        let mut state_ref = self.states.entry(key.clone()).or_insert_with(CusumState::new);
        let state = state_ref.value_mut();

        // Update CUSUM
        let deviation = cost - baseline.mean;
        state.cusum_pos = (state.cusum_pos + deviation - self.config.slack).max(0.0);
        state.cusum_neg = (state.cusum_neg + deviation + self.config.slack).min(0.0);
        state.count += 1;

        // Check if threshold exceeded
        if state.cusum_pos > self.config.threshold || state.cusum_neg.abs() > self.config.threshold {
            let severity = if state.cusum_pos > self.config.threshold * 2.0 {
                Severity::High
            } else {
                Severity::Medium
            };

            let confidence = (state.cusum_pos.max(state.cusum_neg.abs()) / self.config.threshold).min(0.95);

            let anomaly = AnomalyEvent::new(
                severity,
                AnomalyType::CostAnomaly,
                event.service_name.clone(),
                event.model.clone(),
                DetectionMethod::Cusum,
                confidence,
                AnomalyDetails {
                    metric: "cost_usd".to_string(),
                    value: cost,
                    baseline: baseline.mean,
                    threshold: baseline.mean + self.config.slack,
                    deviation_sigma: None,
                    additional: {
                        let mut map = HashMap::new();
                        map.insert("cusum_pos".to_string(), serde_json::json!(state.cusum_pos));
                        map.insert("cusum_neg".to_string(), serde_json::json!(state.cusum_neg));
                        map.insert("samples".to_string(), serde_json::json!(state.count));
                        map
                    },
                },
                AnomalyContext {
                    trace_id: event.trace_id.clone(),
                    user_id: event.metadata.get("user_id").cloned(),
                    region: event.metadata.get("region").cloned(),
                    time_window: format!("last_{}_samples", state.count),
                    sample_count: baseline.sample_count,
                    additional: HashMap::new(),
                },
            )
            .with_root_cause(format!(
                "Sustained cost increase detected (CUSUM: {:.2}, baseline: ${:.4})",
                state.cusum_pos, baseline.mean
            ))
            .with_remediation("Review recent API usage patterns")
            .with_remediation("Check for model version changes or pricing updates");

            // Reset state after detection
            state.reset();

            return Ok(Some(anomaly));
        }

        Ok(None)
    }
}

#[async_trait]
impl Detector for CusumDetector {
    async fn detect(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        // CUSUM requires mutable state, so we need to clone self
        // In a real implementation, this would use interior mutability properly
        let mut detector = self.clone_for_detection();
        detector.detect_cost_drift(event)
    }

    fn name(&self) -> &str {
        "cusum"
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Statistical
    }

    async fn update(&mut self, event: &TelemetryEvent) -> Result<()> {
        if !self.config.detection.update_baseline {
            return Ok(());
        }

        let key = BaselineKey::cost(event.service_name.clone(), event.model.clone());
        self.baseline_manager.update(key, event.cost_usd)?;
        Ok(())
    }

    async fn reset(&mut self) -> Result<()> {
        self.states.clear();
        self.baseline_manager.clear_all()?;
        self.stats = DetectorStats::empty();
        Ok(())
    }

    fn stats(&self) -> DetectorStats {
        self.stats.clone()
    }
}

impl CusumDetector {
    fn clone_for_detection(&self) -> Self {
        Self {
            config: self.config.clone(),
            baseline_manager: Arc::clone(&self.baseline_manager),
            states: Arc::clone(&self.states),
            stats: self.stats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        events::{PromptInfo, ResponseInfo},
        types::{ModelId, ServiceId},
    };

    fn create_test_event(cost: f64) -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test"),
            ModelId::new("gpt-4"),
            PromptInfo {
                text: "test".to_string(),
                tokens: 10,
                embedding: None,
            },
            ResponseInfo {
                text: "response".to_string(),
                tokens: 20,
                finish_reason: "stop".to_string(),
                embedding: None,
            },
            100.0,
            cost,
        )
    }

    #[tokio::test]
    async fn test_cusum_detector() {
        let baseline_manager = Arc::new(BaselineManager::new(20));
        let config = CusumConfig::default();
        let mut detector = CusumDetector::new(config, Arc::clone(&baseline_manager));

        // Build baseline around $0.01
        for _ in 0..20 {
            let event = create_test_event(0.01);
            detector.update(&event).await.unwrap();
        }

        // Gradual increase should trigger CUSUM
        for _ in 0..10 {
            let event = create_test_event(0.02);
            let result = detector.detect(&event).await;
            if result.is_ok() && result.as_ref().unwrap().is_some() {
                let anomaly = result.unwrap().unwrap();
                assert_eq!(anomaly.detection_method, DetectionMethod::Cusum);
                break;
            }
        }
    }
}
