//! Detection engine orchestration.
//!
//! Coordinates multiple detectors and manages the detection pipeline.

use crate::{
    baseline::BaselineManager,
    detectors::{
        cusum::{CusumConfig, CusumDetector},
        iqr::{IqrConfig, IqrDetector},
        mad::{MadConfig, MadDetector},
        zscore::{ZScoreConfig, ZScoreDetector},
    },
    Detector, DetectorStats,
};
use llm_sentinel_core::{
    events::{AnomalyEvent, TelemetryEvent},
    Error, Result,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Detection engine configuration
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Enable Z-Score detector
    pub enable_zscore: bool,
    /// Z-Score configuration
    pub zscore_config: ZScoreConfig,

    /// Enable IQR detector
    pub enable_iqr: bool,
    /// IQR configuration
    pub iqr_config: IqrConfig,

    /// Enable MAD detector
    pub enable_mad: bool,
    /// MAD configuration
    pub mad_config: MadConfig,

    /// Enable CUSUM detector
    pub enable_cusum: bool,
    /// CUSUM configuration
    pub cusum_config: CusumConfig,

    /// Baseline window size
    pub baseline_window_size: usize,

    /// Update baselines continuously
    pub continuous_learning: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            enable_zscore: true,
            zscore_config: ZScoreConfig::default(),
            enable_iqr: true,
            iqr_config: IqrConfig::default(),
            enable_mad: false, // Disabled by default (redundant with Z-Score/IQR)
            mad_config: MadConfig::default(),
            enable_cusum: true,
            cusum_config: CusumConfig::default(),
            baseline_window_size: 1000,
            continuous_learning: true,
        }
    }
}

/// Detection engine that orchestrates multiple detectors
pub struct DetectionEngine {
    config: EngineConfig,
    baseline_manager: Arc<BaselineManager>,
    detectors: Vec<Box<dyn Detector + Send + Sync>>,
    stats: Arc<RwLock<EngineStats>>,
}

/// Engine statistics
#[derive(Debug, Clone)]
pub struct EngineStats {
    /// Total events processed
    pub events_processed: u64,
    /// Total anomalies detected
    pub anomalies_detected: u64,
    /// Detection rate
    pub detection_rate: f64,
    /// Detector-specific stats
    pub detector_stats: Vec<(String, DetectorStats)>,
}

impl EngineStats {
    fn empty() -> Self {
        Self {
            events_processed: 0,
            anomalies_detected: 0,
            detection_rate: 0.0,
            detector_stats: Vec::new(),
        }
    }

    fn update(&mut self, detected: bool) {
        self.events_processed += 1;
        if detected {
            self.anomalies_detected += 1;
        }
        self.detection_rate = self.anomalies_detected as f64 / self.events_processed as f64;
    }
}

impl DetectionEngine {
    /// Create a new detection engine
    pub fn new(config: EngineConfig) -> Result<Self> {
        info!("Creating detection engine");

        let baseline_manager = Arc::new(BaselineManager::new(config.baseline_window_size));
        let mut detectors: Vec<Box<dyn Detector + Send + Sync>> = Vec::new();

        // Initialize enabled detectors
        if config.enable_zscore {
            info!("Enabling Z-Score detector");
            let detector = ZScoreDetector::new(
                config.zscore_config.clone(),
                Arc::clone(&baseline_manager),
            );
            detectors.push(Box::new(detector));
        }

        if config.enable_iqr {
            info!("Enabling IQR detector");
            let detector = IqrDetector::new(config.iqr_config.clone(), Arc::clone(&baseline_manager));
            detectors.push(Box::new(detector));
        }

        if config.enable_mad {
            info!("Enabling MAD detector");
            let detector = MadDetector::new(config.mad_config.clone(), Arc::clone(&baseline_manager));
            detectors.push(Box::new(detector));
        }

        if config.enable_cusum {
            info!("Enabling CUSUM detector");
            let detector = CusumDetector::new(
                config.cusum_config.clone(),
                Arc::clone(&baseline_manager),
            );
            detectors.push(Box::new(detector));
        }

        if detectors.is_empty() {
            return Err(Error::config("No detectors enabled"));
        }

        info!("Detection engine created with {} detectors", detectors.len());

        Ok(Self {
            config,
            baseline_manager,
            detectors,
            stats: Arc::new(RwLock::new(EngineStats::empty())),
        })
    }

    /// Detect anomalies in a telemetry event
    ///
    /// Runs all enabled detectors and returns the first anomaly found.
    /// In production, this could be extended to:
    /// - Run detectors in parallel
    /// - Aggregate multiple detections
    /// - Apply ensemble voting
    pub async fn detect(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        debug!(
            event_id = %event.event_id,
            service = %event.service_name,
            model = %event.model,
            "Running anomaly detection"
        );

        let start = std::time::Instant::now();

        // Run detectors sequentially (can be parallelized for performance)
        for detector in &self.detectors {
            match detector.detect(event).await {
                Ok(Some(anomaly)) => {
                    let elapsed = start.elapsed();
                    info!(
                        event_id = %event.event_id,
                        detector = detector.name(),
                        anomaly_type = %anomaly.anomaly_type,
                        severity = %anomaly.severity,
                        confidence = anomaly.confidence,
                        detection_ms = elapsed.as_millis(),
                        "Anomaly detected"
                    );

                    // Update stats
                    let mut stats = self.stats.write().await;
                    stats.update(true);

                    // Record metrics
                    metrics::counter!(
                        "sentinel_anomalies_detected_total",
                        "detector" => detector.name(),
                        "type" => anomaly.anomaly_type.to_string(),
                        "severity" => anomaly.severity.to_string()
                    )
                    .increment(1);

                    metrics::histogram!(
                        "sentinel_detection_duration_seconds",
                        "detector" => detector.name()
                    )
                    .record(elapsed.as_secs_f64());

                    return Ok(Some(anomaly));
                }
                Ok(None) => {
                    // No anomaly detected by this detector
                    continue;
                }
                Err(e) => {
                    warn!(
                        event_id = %event.event_id,
                        detector = detector.name(),
                        error = %e,
                        "Detector error"
                    );
                    metrics::counter!(
                        "sentinel_detection_errors_total",
                        "detector" => detector.name()
                    )
                    .increment(1);
                    // Continue with other detectors
                    continue;
                }
            }
        }

        // No anomalies detected
        let mut stats = self.stats.write().await;
        stats.update(false);

        let elapsed = start.elapsed();
        metrics::histogram!("sentinel_detection_duration_seconds", "detector" => "all")
            .record(elapsed.as_secs_f64());

        Ok(None)
    }

    /// Update detectors with new event (for learning)
    pub async fn update(&mut self, event: &TelemetryEvent) -> Result<()> {
        if !self.config.continuous_learning {
            return Ok(());
        }

        for detector in &mut self.detectors {
            if let Err(e) = detector.update(event).await {
                warn!(
                    detector = detector.name(),
                    error = %e,
                    "Failed to update detector"
                );
            }
        }

        Ok(())
    }

    /// Process a telemetry event (detect + update)
    pub async fn process(&mut self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>> {
        // First detect anomalies
        let anomaly = self.detect(event).await?;

        // Then update baselines (if continuous learning enabled)
        // Note: We update even if anomaly detected, to adapt to changing patterns
        self.update(event).await?;

        Ok(anomaly)
    }

    /// Get engine statistics
    pub async fn stats(&self) -> EngineStats {
        let mut stats = self.stats.read().await.clone();

        // Collect detector-specific stats
        stats.detector_stats = self
            .detectors
            .iter()
            .map(|d| (d.name().to_string(), d.stats()))
            .collect();

        stats
    }

    /// Reset all detectors
    pub async fn reset(&mut self) -> Result<()> {
        info!("Resetting detection engine");

        for detector in &mut self.detectors {
            detector.reset().await?;
        }

        let mut stats = self.stats.write().await;
        *stats = EngineStats::empty();

        info!("Detection engine reset complete");
        Ok(())
    }

    /// Get baseline manager for external access
    pub fn baseline_manager(&self) -> &Arc<BaselineManager> {
        &self.baseline_manager
    }

    /// Get number of enabled detectors
    pub fn detector_count(&self) -> usize {
        self.detectors.len()
    }

    /// Get detector names
    pub fn detector_names(&self) -> Vec<String> {
        self.detectors.iter().map(|d| d.name().to_string()).collect()
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
    async fn test_engine_creation() {
        let config = EngineConfig::default();
        let engine = DetectionEngine::new(config).unwrap();
        assert!(engine.detector_count() > 0);
        assert!(engine.detector_names().contains(&"zscore".to_string()));
    }

    #[tokio::test]
    async fn test_engine_detection() {
        let config = EngineConfig::default();
        let mut engine = DetectionEngine::new(config).unwrap();

        // Build baselines
        for i in 1..=20 {
            let event = create_test_event(100.0 + i as f64, 100, 0.01);
            engine.update(&event).await.unwrap();
        }

        // Normal event
        let normal = create_test_event(110.0, 100, 0.01);
        let result = engine.detect(&normal).await.unwrap();
        assert!(result.is_none());

        // Anomalous event
        let anomaly = create_test_event(1000.0, 100, 0.01);
        let result = engine.detect(&anomaly).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_engine_process() {
        let config = EngineConfig::default();
        let mut engine = DetectionEngine::new(config).unwrap();

        // Process events
        for i in 1..=20 {
            let event = create_test_event(100.0 + i as f64, 100, 0.01);
            let _ = engine.process(&event).await;
        }

        let stats = engine.stats().await;
        assert_eq!(stats.events_processed, 20);
    }

    #[tokio::test]
    async fn test_engine_reset() {
        let config = EngineConfig::default();
        let mut engine = DetectionEngine::new(config).unwrap();

        // Process some events
        for i in 1..=10 {
            let event = create_test_event(100.0 + i as f64, 100, 0.01);
            engine.process(&event).await.unwrap();
        }

        let stats_before = engine.stats().await;
        assert_eq!(stats_before.events_processed, 10);

        // Reset
        engine.reset().await.unwrap();

        let stats_after = engine.stats().await;
        assert_eq!(stats_after.events_processed, 0);
    }

    #[tokio::test]
    async fn test_engine_selective_detectors() {
        let config = EngineConfig {
            enable_zscore: true,
            enable_iqr: false,
            enable_mad: false,
            enable_cusum: false,
            ..Default::default()
        };

        let engine = DetectionEngine::new(config).unwrap();
        assert_eq!(engine.detector_count(), 1);
        assert_eq!(engine.detector_names(), vec!["zscore"]);
    }

    #[tokio::test]
    async fn test_engine_no_detectors() {
        let config = EngineConfig {
            enable_zscore: false,
            enable_iqr: false,
            enable_mad: false,
            enable_cusum: false,
            ..Default::default()
        };

        let result = DetectionEngine::new(config);
        assert!(result.is_err());
    }
}
