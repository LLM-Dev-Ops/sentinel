//! # Sentinel Detection
//!
//! Anomaly detection engines for LLM telemetry.
//!
//! This crate provides:
//! - Statistical detection methods (Z-Score, IQR, CUSUM, MAD)
//! - Baseline calculation and management
//! - Detection engine orchestration
//! - Multi-detector support with confidence scoring

#![warn(missing_debug_implementations, rust_2018_idioms, unreachable_pub)]

pub mod agents;
pub mod baseline;
pub mod detectors;
pub mod engine;
pub mod stats;

use async_trait::async_trait;
use llm_sentinel_core::{
    events::{AnomalyEvent, TelemetryEvent},
    Result,
};

/// Trait for anomaly detectors
#[async_trait]
pub trait Detector: Send + Sync {
    /// Detect anomalies in a telemetry event
    ///
    /// Returns `Some(AnomalyEvent)` if an anomaly is detected, `None` otherwise.
    async fn detect(&self, event: &TelemetryEvent) -> Result<Option<AnomalyEvent>>;

    /// Get the detector name
    fn name(&self) -> &str;

    /// Get the detector type (statistical, ml, llm)
    fn detector_type(&self) -> DetectorType;

    /// Update detector with new data (for learning-based detectors)
    async fn update(&mut self, event: &TelemetryEvent) -> Result<()> {
        // Default implementation: no-op for stateless detectors
        let _ = event;
        Ok(())
    }

    /// Reset detector state
    async fn reset(&mut self) -> Result<()>;

    /// Get detector statistics
    fn stats(&self) -> DetectorStats;
}

/// Detector type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorType {
    /// Statistical methods (Z-Score, IQR, etc.)
    Statistical,
    /// Machine learning methods (Isolation Forest, etc.)
    MachineLearning,
    /// LLM-powered detection
    LlmPowered,
}

/// Detector statistics
#[derive(Debug, Clone)]
pub struct DetectorStats {
    /// Total events processed
    pub events_processed: u64,
    /// Total anomalies detected
    pub anomalies_detected: u64,
    /// Detection rate (anomalies / events)
    pub detection_rate: f64,
    /// Average confidence score
    pub avg_confidence: f64,
}

impl DetectorStats {
    /// Create empty stats
    pub fn empty() -> Self {
        Self {
            events_processed: 0,
            anomalies_detected: 0,
            detection_rate: 0.0,
            avg_confidence: 0.0,
        }
    }

    /// Update stats with detection result
    pub fn update(&mut self, detected: bool, confidence: Option<f64>) {
        self.events_processed += 1;
        if detected {
            self.anomalies_detected += 1;
        }
        self.detection_rate = self.anomalies_detected as f64 / self.events_processed as f64;

        if let Some(conf) = confidence {
            // Update running average
            let total_conf = self.avg_confidence * (self.events_processed - 1) as f64 + conf;
            self.avg_confidence = total_conf / self.events_processed as f64;
        }
    }
}

/// Re-export commonly used types
pub mod prelude {
    pub use crate::agents::{
        // Core agent types
        AgentId, AgentInput, AgentOutput, AgentVersion,
        ConstraintApplied, DecisionEvent, DecisionType,
        // Anomaly detection agent
        AnomalyDetectionAgent,
        // Alerting agent
        AlertingAgent, AlertEvaluationStatus, AlertingAgentInput, AlertingAgentOutput,
        AlertingFailureMode, AlertingOutputMetadata, AlertingRule, SuppressionConfig,
    };
    pub use crate::baseline::{Baseline, BaselineManager};
    pub use crate::detectors::{
        cusum::CusumDetector, drift::DriftDetector, iqr::IqrDetector, mad::MadDetector,
        zscore::ZScoreDetector,
    };
    pub use crate::engine::{DetectionEngine, EngineConfig};
    pub use crate::{Detector, DetectorStats, DetectorType};
}
