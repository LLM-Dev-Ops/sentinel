//! MAD (Median Absolute Deviation) anomaly detector.
//!
//! Very robust to outliers, uses median instead of mean.

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

/// MAD detector configuration
#[derive(Debug, Clone)]
pub struct MadConfig {
    /// Modified Z-score threshold (typically 3.5)
    pub threshold: f64,
    /// Common detection config
    pub detection: DetectionConfig,
}

impl Default for MadConfig {
    fn default() -> Self {
        Self {
            threshold: 3.5, // Conservative threshold for MAD
            detection: DetectionConfig::default(),
        }
    }
}

/// MAD anomaly detector
///
/// Uses Median Absolute Deviation for robust outlier detection.
/// More resistant to outliers than both Z-Score and IQR.
///
/// Modified Z-score formula:
/// M = 0.6745 × (x - median) / MAD
///
/// where MAD = median(|x_i - median(x)|)
pub struct MadDetector {
    config: MadConfig,
    baseline_manager: Arc<BaselineManager>,
    stats: DetectorStats,
}

impl MadDetector {
    /// Create a new MAD detector
    pub fn new(config: MadConfig, baseline_manager: Arc<BaselineManager>) -> Self {
        Self {
            config,
            baseline_manager,
            stats: DetectorStats::empty(),
        }
    }

    fn detect_latency(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());

        if !self.baseline_manager.has_valid_baseline(&key) {
            return Ok(None);
        }

        let baseline = self.baseline_manager.get(&key).unwrap();
        let latency = event.latency_ms;

        if stats::is_mad_outlier(latency, baseline.median, baseline.mad, self.config.threshold) {
            let severity = if latency > baseline.p99 {
                Severity::High
            } else {
                Severity::Medium
            };

            let modified_zscore = if baseline.mad > 0.0 {
                0.6745 * (latency - baseline.median).abs() / baseline.mad
            } else {
                0.0
            };

            let confidence = (modified_zscore / self.config.threshold).min(0.99);

            let anomaly = AnomalyEvent::new(
                severity,
                AnomalyType::LatencySpike,
                event.service_name.clone(),
                event.model.clone(),
                DetectionMethod::Mad,
                confidence,
                AnomalyDetails {
                    metric: "latency_ms".to_string(),
                    value: latency,
                    baseline: baseline.median,
                    threshold: baseline.median + self.config.threshold * baseline.mad,
                    deviation_sigma: Some(modified_zscore),
                    additional: {
                        let mut map = HashMap::new();
                        map.insert("mad".to_string(), serde_json::json!(baseline.mad));
                        map.insert("modified_zscore".to_string(), serde_json::json!(modified_zscore));
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
                "Latency {:.2}ms deviates significantly from median {:.2}ms (MAD: {:.2})",
                latency, baseline.median, baseline.mad
            ));

            return Ok(Some(anomaly));
        }

        Ok(None)
    }
}

#[async_trait]
impl Detector for MadDetector {
    async fn detect(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        self.detect_latency(event)
    }

    fn name(&self) -> &str {
        "mad"
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Statistical
    }

    async fn update(&mut self, event: &TelemetryEvent) -> Result<()> {
        if !self.config.detection.update_baseline {
            return Ok(());
        }

        let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());
        self.baseline_manager.update(key, event.latency_ms)?;
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
