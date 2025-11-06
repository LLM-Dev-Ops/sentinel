//! Z-Score anomaly detector for identifying statistical outliers.

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

/// Z-Score detector configuration
#[derive(Debug, Clone)]
pub struct ZScoreConfig {
    /// Z-score threshold (typically 3.0 for 99.7% confidence)
    pub threshold: f64,
    /// Common detection config
    pub detection: DetectionConfig,
}

impl Default for ZScoreConfig {
    fn default() -> Self {
        Self {
            threshold: 3.0, // 3 sigma = 99.7% confidence interval
            detection: DetectionConfig::default(),
        }
    }
}

/// Z-Score anomaly detector
///
/// Detects outliers based on standard deviations from the mean.
/// A Z-score measures how many standard deviations a value is from the mean.
///
/// Formula: z = (x - μ) / σ
/// where:
/// - x = observed value
/// - μ = mean of baseline
/// - σ = standard deviation of baseline
///
/// Threshold interpretation:
/// - 2σ: ~95% of data within range (5% outliers)
/// - 3σ: ~99.7% of data within range (0.3% outliers)
/// - 4σ: ~99.99% of data within range (0.01% outliers)
pub struct ZScoreDetector {
    config: ZScoreConfig,
    baseline_manager: Arc<BaselineManager>,
    stats: DetectorStats,
}

impl ZScoreDetector {
    /// Create a new Z-Score detector
    pub fn new(config: ZScoreConfig, baseline_manager: Arc<BaselineManager>) -> Self {
        Self {
            config,
            baseline_manager,
            stats: DetectorStats::empty(),
        }
    }

    /// Detect latency anomaly
    fn detect_latency(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());

        // Check if we have a valid baseline
        if !self.baseline_manager.has_valid_baseline(&key) {
            debug!(
                service = %event.service_name,
                model = %event.model,
                "No valid baseline for latency detection"
            );
            return Ok(None);
        }

        let baseline = self.baseline_manager.get(&key).unwrap();
        let latency = event.latency_ms;

        // Calculate Z-score
        let z = stats::zscore(latency, baseline.mean, baseline.std_dev);

        if z.abs() > self.config.threshold {
            let severity = self.calculate_severity(z.abs());
            let confidence = self.calculate_confidence(z.abs());

            let anomaly = AnomalyEvent::new(
                severity,
                AnomalyType::LatencySpike,
                event.service_name.clone(),
                event.model.clone(),
                DetectionMethod::ZScore,
                confidence,
                AnomalyDetails {
                    metric: "latency_ms".to_string(),
                    value: latency,
                    baseline: baseline.mean,
                    threshold: baseline.mean + self.config.threshold * baseline.std_dev,
                    deviation_sigma: Some(z.abs()),
                    additional: HashMap::new(),
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
                "Latency {:.2}ms is {:.2} standard deviations above baseline {:.2}ms",
                latency, z, baseline.mean
            ))
            .with_remediation("Check service health and resource utilization")
            .with_remediation("Review recent deployments or configuration changes");

            debug!(
                event_id = %event.event_id,
                latency = latency,
                baseline = baseline.mean,
                z_score = z,
                "Latency anomaly detected"
            );

            return Ok(Some(anomaly));
        }

        Ok(None)
    }

    /// Detect token usage anomaly
    fn detect_tokens(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        let key = BaselineKey::tokens(event.service_name.clone(), event.model.clone());

        if !self.baseline_manager.has_valid_baseline(&key) {
            return Ok(None);
        }

        let baseline = self.baseline_manager.get(&key).unwrap();
        let tokens = event.total_tokens() as f64;

        let z = stats::zscore(tokens, baseline.mean, baseline.std_dev);

        if z.abs() > self.config.threshold {
            let severity = self.calculate_severity(z.abs());
            let confidence = self.calculate_confidence(z.abs());

            let anomaly = AnomalyEvent::new(
                severity,
                AnomalyType::TokenUsageSpike,
                event.service_name.clone(),
                event.model.clone(),
                DetectionMethod::ZScore,
                confidence,
                AnomalyDetails {
                    metric: "total_tokens".to_string(),
                    value: tokens,
                    baseline: baseline.mean,
                    threshold: baseline.mean + self.config.threshold * baseline.std_dev,
                    deviation_sigma: Some(z.abs()),
                    additional: HashMap::new(),
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
                "Token usage {} is {:.2} standard deviations above baseline {:.0}",
                tokens as u32, z, baseline.mean
            ))
            .with_remediation("Review prompt templates for excessive verbosity")
            .with_remediation("Check for potential token abuse or prompt injection");

            return Ok(Some(anomaly));
        }

        Ok(None)
    }

    /// Detect cost anomaly
    fn detect_cost(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        let key = BaselineKey::cost(event.service_name.clone(), event.model.clone());

        if !self.baseline_manager.has_valid_baseline(&key) {
            return Ok(None);
        }

        let baseline = self.baseline_manager.get(&key).unwrap();
        let cost = event.cost_usd;

        let z = stats::zscore(cost, baseline.mean, baseline.std_dev);

        if z.abs() > self.config.threshold {
            let severity = self.calculate_severity(z.abs());
            let confidence = self.calculate_confidence(z.abs());

            let anomaly = AnomalyEvent::new(
                severity,
                AnomalyType::CostAnomaly,
                event.service_name.clone(),
                event.model.clone(),
                DetectionMethod::ZScore,
                confidence,
                AnomalyDetails {
                    metric: "cost_usd".to_string(),
                    value: cost,
                    baseline: baseline.mean,
                    threshold: baseline.mean + self.config.threshold * baseline.std_dev,
                    deviation_sigma: Some(z.abs()),
                    additional: HashMap::new(),
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
                "Cost ${:.4} is {:.2} standard deviations above baseline ${:.4}",
                cost, z, baseline.mean
            ))
            .with_remediation("Review API usage patterns for cost optimization")
            .with_remediation("Consider rate limiting or budget alerts");

            return Ok(Some(anomaly));
        }

        Ok(None)
    }

    /// Calculate severity based on Z-score
    fn calculate_severity(&self, z_score: f64) -> Severity {
        if z_score >= 6.0 {
            Severity::Critical // Extremely rare (1 in 506 million)
        } else if z_score >= 4.0 {
            Severity::High // Very rare (1 in 15,787)
        } else if z_score >= 3.0 {
            Severity::Medium // Rare (1 in 370)
        } else {
            Severity::Low
        }
    }

    /// Calculate confidence score based on Z-score
    fn calculate_confidence(&self, z_score: f64) -> f64 {
        // Map Z-score to confidence (0.0 - 1.0)
        // 3σ = 0.95, 4σ = 0.98, 6σ = 0.99+
        let confidence = 1.0 - (1.0 / (1.0 + (z_score - self.config.threshold)));
        confidence.clamp(0.5, 0.99)
    }
}

#[async_trait]
impl Detector for ZScoreDetector {
    async fn detect(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        // Try detecting different anomaly types
        // Return the first anomaly found (can be extended to detect multiple)

        if let Some(anomaly) = self.detect_latency(event)? {
            return Ok(Some(anomaly));
        }

        if let Some(anomaly) = self.detect_tokens(event)? {
            return Ok(Some(anomaly));
        }

        if let Some(anomaly) = self.detect_cost(event)? {
            return Ok(Some(anomaly));
        }

        Ok(None)
    }

    fn name(&self) -> &str {
        "zscore"
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Statistical
    }

    async fn update(&mut self, event: &TelemetryEvent) -> Result<()> {
        if !self.config.detection.update_baseline {
            return Ok(());
        }

        // Update baselines with event data
        let latency_key = BaselineKey::latency(event.service_name.clone(), event.model.clone());
        self.baseline_manager
            .update(latency_key, event.latency_ms)?;

        let tokens_key = BaselineKey::tokens(event.service_name.clone(), event.model.clone());
        self.baseline_manager
            .update(tokens_key, event.total_tokens() as f64)?;

        let cost_key = BaselineKey::cost(event.service_name.clone(), event.model.clone());
        self.baseline_manager.update(cost_key, event.cost_usd)?;

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

    fn create_test_event(latency: f64, tokens: u32, cost: f64) -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test"),
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

    #[tokio::test]
    async fn test_zscore_detector_no_baseline() {
        let baseline_manager = Arc::new(BaselineManager::new(10));
        let config = ZScoreConfig::default();
        let detector = ZScoreDetector::new(config, baseline_manager);

        let event = create_test_event(100.0, 100, 0.01);
        let result = detector.detect(&event).await.unwrap();
        assert!(result.is_none()); // No baseline yet
    }

    #[tokio::test]
    async fn test_zscore_detector_with_baseline() {
        let baseline_manager = Arc::new(BaselineManager::new(10));
        let config = ZScoreConfig {
            threshold: 3.0,
            detection: DetectionConfig {
                min_samples: 10,
                update_baseline: true,
            },
        };
        let mut detector = ZScoreDetector::new(config, Arc::clone(&baseline_manager));

        // Build baseline with normal values (100ms ± 10ms)
        for i in 0..10 {
            let event = create_test_event(100.0 + (i as f64 - 5.0), 100, 0.01);
            detector.update(&event).await.unwrap();
        }

        // Test with normal value
        let normal_event = create_test_event(100.0, 100, 0.01);
        let result = detector.detect(&normal_event).await.unwrap();
        assert!(result.is_none());

        // Test with outlier (10x normal)
        let outlier_event = create_test_event(1000.0, 100, 0.01);
        let result = detector.detect(&outlier_event).await.unwrap();
        assert!(result.is_some());

        let anomaly = result.unwrap();
        assert_eq!(anomaly.anomaly_type, AnomalyType::LatencySpike);
        assert_eq!(anomaly.detection_method, DetectionMethod::ZScore);
        assert!(anomaly.confidence > 0.9);
    }

    #[tokio::test]
    async fn test_zscore_severity_calculation() {
        let baseline_manager = Arc::new(BaselineManager::new(10));
        let detector = ZScoreDetector::new(ZScoreConfig::default(), baseline_manager);

        assert_eq!(detector.calculate_severity(3.0), Severity::Medium);
        assert_eq!(detector.calculate_severity(4.0), Severity::High);
        assert_eq!(detector.calculate_severity(6.0), Severity::Critical);
    }

    #[tokio::test]
    async fn test_zscore_confidence_calculation() {
        let baseline_manager = Arc::new(BaselineManager::new(10));
        let detector = ZScoreDetector::new(ZScoreConfig::default(), baseline_manager);

        let conf_3 = detector.calculate_confidence(3.0);
        let conf_4 = detector.calculate_confidence(4.0);
        let conf_6 = detector.calculate_confidence(6.0);

        assert!(conf_3 >= 0.5 && conf_3 < 1.0);
        assert!(conf_4 > conf_3);
        assert!(conf_6 > conf_4);
    }
}
