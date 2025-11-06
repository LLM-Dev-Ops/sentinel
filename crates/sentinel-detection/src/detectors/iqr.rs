//! IQR (Interquartile Range) anomaly detector.
//!
//! More robust to outliers than Z-Score, uses quartiles instead of mean/std.

use crate::{
    baseline::{BaselineKey, BaselineManager},
    detectors::DetectionConfig,
    stats, Detector, DetectorStats, DetectorType,
};
use async_trait::async_trait;
use llm_sentinel_core::{
    events::{AnomalyContext, AnomalyDetails, AnomalyEvent, TelemetryEvent},
    types::{AnomalyType, DetectionMethod, Severity},
    Result,
};
use std::{collections::HashMap, sync::Arc};
use tracing::debug;

/// IQR detector configuration
#[derive(Debug, Clone)]
pub struct IqrConfig {
    /// IQR multiplier (typically 1.5 or 3.0)
    /// 1.5 = moderate outliers, 3.0 = extreme outliers
    pub multiplier: f64,
    /// Common detection config
    pub detection: DetectionConfig,
}

impl Default for IqrConfig {
    fn default() -> Self {
        Self {
            multiplier: 1.5, // Tukey's rule for moderate outliers
            detection: DetectionConfig::default(),
        }
    }
}

/// IQR anomaly detector
///
/// Uses interquartile range to detect outliers.
/// More robust to extreme values than Z-Score.
///
/// Formula:
/// - Lower bound = Q1 - multiplier × IQR
/// - Upper bound = Q3 + multiplier × IQR
/// - Outlier if value < lower bound OR value > upper bound
///
/// where IQR = Q3 - Q1
pub struct IqrDetector {
    config: IqrConfig,
    baseline_manager: Arc<BaselineManager>,
    stats: DetectorStats,
}

impl IqrDetector {
    /// Create a new IQR detector
    pub fn new(config: IqrConfig, baseline_manager: Arc<BaselineManager>) -> Self {
        Self {
            config,
            baseline_manager,
            stats: DetectorStats::empty(),
        }
    }

    /// Detect latency anomaly using IQR
    fn detect_latency(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());

        if !self.baseline_manager.has_valid_baseline(&key) {
            return Ok(None);
        }

        let baseline = self.baseline_manager.get(&key).unwrap();
        let latency = event.latency_ms;

        // Check if outlier using IQR method
        if stats::is_iqr_outlier(latency, baseline.q1, baseline.q3, baseline.iqr, self.config.multiplier) {
            let severity = self.calculate_severity(latency, &baseline);
            let confidence = self.calculate_confidence(latency, &baseline);

            let lower_bound = baseline.q1 - self.config.multiplier * baseline.iqr;
            let upper_bound = baseline.q3 + self.config.multiplier * baseline.iqr;

            let anomaly = AnomalyEvent::new(
                severity,
                AnomalyType::LatencySpike,
                event.service_name.clone(),
                event.model.clone(),
                DetectionMethod::Iqr,
                confidence,
                AnomalyDetails {
                    metric: "latency_ms".to_string(),
                    value: latency,
                    baseline: baseline.median,
                    threshold: upper_bound,
                    deviation_sigma: None,
                    additional: {
                        let mut map = HashMap::new();
                        map.insert("q1".to_string(), serde_json::json!(baseline.q1));
                        map.insert("q3".to_string(), serde_json::json!(baseline.q3));
                        map.insert("iqr".to_string(), serde_json::json!(baseline.iqr));
                        map.insert("lower_bound".to_string(), serde_json::json!(lower_bound));
                        map.insert("upper_bound".to_string(), serde_json::json!(upper_bound));
                        map
                    },
                },
                AnomalyContext {
                    trace_id: event.trace_id.clone(),
                    user_id: event.metadata.get("user_id").cloned(),
                    region: event.metadata.get("region").cloned(),
                    time_window: "rolling_window".to_string(),
                    sample_count: baseline.sample_count,
                    additional: HashMap::new(),
                },
            )
            .with_root_cause(format!(
                "Latency {:.2}ms exceeds IQR bounds (median: {:.2}ms, IQR: {:.2}ms)",
                latency, baseline.median, baseline.iqr
            ))
            .with_remediation("Check for resource contention or external dependencies");

            debug!(
                event_id = %event.event_id,
                latency = latency,
                median = baseline.median,
                "IQR latency anomaly detected"
            );

            return Ok(Some(anomaly));
        }

        Ok(None)
    }

    /// Calculate severity based on how far beyond bounds
    fn calculate_severity(&self, value: f64, baseline: &crate::baseline::Baseline) -> Severity {
        let upper_bound = baseline.q3 + self.config.multiplier * baseline.iqr;
        let extreme_bound = baseline.q3 + 3.0 * baseline.iqr;

        if value > extreme_bound {
            Severity::Critical
        } else if value > upper_bound * 1.5 {
            Severity::High
        } else if value > upper_bound {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Calculate confidence based on distance from bounds
    fn calculate_confidence(&self, value: f64, baseline: &crate::baseline::Baseline) -> f64 {
        let upper_bound = baseline.q3 + self.config.multiplier * baseline.iqr;
        let distance_ratio = (value - upper_bound) / baseline.iqr;
        let confidence = 0.7 + (distance_ratio.min(3.0) * 0.1);
        confidence.clamp(0.7, 0.99)
    }
}

#[async_trait]
impl Detector for IqrDetector {
    async fn detect(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        self.detect_latency(event)
    }

    fn name(&self) -> &str {
        "iqr"
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Statistical
    }

    async fn update(&mut self, event: &TelemetryEvent) -> Result<()> {
        if !self.config.detection.update_baseline {
            return Ok(());
        }

        let latency_key = BaselineKey::latency(event.service_name.clone(), event.model.clone());
        self.baseline_manager
            .update(latency_key, event.latency_ms)?;

        Ok(())
    }

    async fn reset(&mut self) -> Result<()> {
        self.baseline_manager.clear_all()?;
        self.stats = DetectorStats::empty();
        Ok(())
    }

    fn stats(&self) -> DetectorStats {
        self.stats.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        events::{PromptInfo, ResponseInfo},
        types::{ModelId, ServiceId},
    };

    fn create_test_event(latency: f64) -> TelemetryEvent {
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
            latency,
            0.01,
        )
    }

    #[tokio::test]
    async fn test_iqr_detector() {
        let baseline_manager = Arc::new(BaselineManager::new(20));
        let config = IqrConfig::default();
        let mut detector = IqrDetector::new(config, Arc::clone(&baseline_manager));

        // Build baseline with values 1-20
        for i in 1..=20 {
            let event = create_test_event(i as f64 * 10.0);
            detector.update(&event).await.unwrap();
        }

        // Normal value within range
        let normal = create_test_event(100.0);
        assert!(detector.detect(&normal).await.unwrap().is_none());

        // Extreme outlier
        let outlier = create_test_event(500.0);
        let result = detector.detect(&outlier).await.unwrap();
        assert!(result.is_some());

        let anomaly = result.unwrap();
        assert_eq!(anomaly.detection_method, DetectionMethod::Iqr);
        assert!(anomaly.confidence >= 0.7);
    }
}
