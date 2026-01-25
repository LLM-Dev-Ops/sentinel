//! Phase 2 Signal Types and Emission
//!
//! Defines the four core signal types for Phase 2 agents:
//! - Anomaly signals
//! - Drift signals
//! - Memory lineage signals
//! - Latency signals
//!
//! All signals are:
//! - Atomic (self-contained, no dependencies)
//! - Confidence-scored (0.0-1.0)
//! - Conclusion-free (raw observations only)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, instrument, warn};
use uuid::Uuid;

use super::ruvector::{RuvectorClient, RuvectorError, SignalRecord, LineageDelta, LineageRelation};

/// Errors during signal emission
#[derive(Debug, Error)]
pub enum SignalError {
    /// Signal validation failed
    #[error("Signal validation failed: {0}")]
    Validation(String),

    /// Emission failed
    #[error("Failed to emit signal: {0}")]
    Emission(#[from] RuvectorError),

    /// Confidence out of range
    #[error("Confidence must be between 0.0 and 1.0, got {0}")]
    InvalidConfidence(f64),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Signal types emitted by Phase 2 agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    /// Anomaly detection signal
    Anomaly,
    /// Distribution drift signal
    Drift,
    /// Memory lineage tracking signal
    MemoryLineage,
    /// Latency measurement signal
    Latency,
}

impl std::fmt::Display for SignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anomaly => write!(f, "anomaly"),
            Self::Drift => write!(f, "drift"),
            Self::MemoryLineage => write!(f, "memory_lineage"),
            Self::Latency => write!(f, "latency"),
        }
    }
}

/// A Phase 2 signal - atomic unit of observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// Unique signal identifier
    pub id: Uuid,
    /// Signal type
    pub signal_type: SignalType,
    /// Emitting agent name
    pub agent_name: String,
    /// Agent domain
    pub agent_domain: String,
    /// Emission timestamp
    pub timestamp: DateTime<Utc>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Raw observation payload (no conclusions)
    pub payload: SignalPayload,
    /// Parent signal IDs for lineage tracking
    pub parent_ids: Vec<Uuid>,
    /// Processing latency in milliseconds
    pub latency_ms: u64,
}

impl Signal {
    /// Create a new signal builder
    pub fn builder(signal_type: SignalType) -> SignalBuilder {
        SignalBuilder::new(signal_type)
    }

    /// Validate the signal
    pub fn validate(&self) -> Result<(), SignalError> {
        if self.confidence < 0.0 || self.confidence > 1.0 {
            return Err(SignalError::InvalidConfidence(self.confidence));
        }
        if self.agent_name.is_empty() {
            return Err(SignalError::MissingField("agent_name".to_string()));
        }
        if self.agent_domain.is_empty() {
            return Err(SignalError::MissingField("agent_domain".to_string()));
        }
        Ok(())
    }

    /// Convert to Ruvector signal record
    pub fn to_record(&self) -> SignalRecord {
        SignalRecord {
            signal_id: self.id,
            signal_type: self.signal_type.to_string(),
            agent_name: self.agent_name.clone(),
            agent_domain: self.agent_domain.clone(),
            timestamp: self.timestamp,
            confidence: self.confidence,
            payload: serde_json::to_value(&self.payload).unwrap_or_default(),
            parent_signals: self.parent_ids.clone(),
            latency_ms: self.latency_ms,
        }
    }
}

/// Signal payload variants - atomic observations without conclusions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SignalPayload {
    /// Anomaly signal payload
    Anomaly(AnomalyPayload),
    /// Drift signal payload
    Drift(DriftPayload),
    /// Memory lineage payload
    MemoryLineage(MemoryLineagePayload),
    /// Latency signal payload
    Latency(LatencyPayload),
}

/// Anomaly signal payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyPayload {
    /// Service name
    pub service: String,
    /// Model name
    pub model: String,
    /// Metric name (latency, tokens, cost, error_rate)
    pub metric: String,
    /// Observed value
    pub observed_value: f64,
    /// Expected baseline value
    pub baseline_value: f64,
    /// Statistical deviation (z-score, IQR multiplier, etc.)
    pub deviation: f64,
    /// Detection method used
    pub detection_method: String,
    /// Threshold that was crossed
    pub threshold: f64,
}

/// Drift signal payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftPayload {
    /// Service name
    pub service: String,
    /// Model name
    pub model: String,
    /// Metric being monitored
    pub metric: String,
    /// Population Stability Index (or similar)
    pub drift_score: f64,
    /// Reference distribution hash
    pub reference_hash: String,
    /// Current distribution hash
    pub current_hash: String,
    /// Number of bins in comparison
    pub bin_count: u32,
    /// Reference window size
    pub reference_window_size: u64,
    /// Current window size
    pub current_window_size: u64,
}

/// Memory lineage payload - tracks event relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLineagePayload {
    /// Source event/signal identifier
    pub source_id: Uuid,
    /// Target event/signal identifier
    pub target_id: Uuid,
    /// Relationship type
    pub relationship: String,
    /// Event index reference (Ruvector)
    pub event_index: String,
    /// Optional context metadata
    pub context: Option<serde_json::Value>,
}

/// Latency signal payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyPayload {
    /// Service name
    pub service: String,
    /// Operation name
    pub operation: String,
    /// Measured latency in milliseconds
    pub latency_ms: f64,
    /// Percentile if applicable (e.g., p50, p95, p99)
    pub percentile: Option<String>,
    /// Sample count used for percentile calculation
    pub sample_count: Option<u64>,
    /// Time window in seconds
    pub window_seconds: Option<u64>,
}

/// Builder for constructing signals
#[derive(Debug)]
pub struct SignalBuilder {
    signal_type: SignalType,
    agent_name: Option<String>,
    agent_domain: Option<String>,
    confidence: Option<f64>,
    payload: Option<SignalPayload>,
    parent_ids: Vec<Uuid>,
    latency_ms: Option<u64>,
}

impl SignalBuilder {
    /// Create a new signal builder
    pub fn new(signal_type: SignalType) -> Self {
        Self {
            signal_type,
            agent_name: None,
            agent_domain: None,
            confidence: None,
            payload: None,
            parent_ids: Vec::new(),
            latency_ms: None,
        }
    }

    /// Set agent name
    pub fn agent_name(mut self, name: impl Into<String>) -> Self {
        self.agent_name = Some(name.into());
        self
    }

    /// Set agent domain
    pub fn agent_domain(mut self, domain: impl Into<String>) -> Self {
        self.agent_domain = Some(domain.into());
        self
    }

    /// Set confidence score (0.0-1.0)
    pub fn confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Set the payload
    pub fn payload(mut self, payload: SignalPayload) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Add parent signal ID for lineage
    pub fn parent(mut self, parent_id: Uuid) -> Self {
        self.parent_ids.push(parent_id);
        self
    }

    /// Add multiple parent signal IDs
    pub fn parents(mut self, parent_ids: impl IntoIterator<Item = Uuid>) -> Self {
        self.parent_ids.extend(parent_ids);
        self
    }

    /// Set processing latency
    pub fn latency_ms(mut self, latency: u64) -> Self {
        self.latency_ms = Some(latency);
        self
    }

    /// Build the signal
    pub fn build(self) -> Result<Signal, SignalError> {
        let signal = Signal {
            id: Uuid::new_v4(),
            signal_type: self.signal_type,
            agent_name: self.agent_name.ok_or(SignalError::MissingField("agent_name".to_string()))?,
            agent_domain: self.agent_domain.ok_or(SignalError::MissingField("agent_domain".to_string()))?,
            timestamp: Utc::now(),
            confidence: self.confidence.ok_or(SignalError::MissingField("confidence".to_string()))?,
            payload: self.payload.ok_or(SignalError::MissingField("payload".to_string()))?,
            parent_ids: self.parent_ids,
            latency_ms: self.latency_ms.unwrap_or(0),
        };

        signal.validate()?;
        Ok(signal)
    }
}

/// Signal emitter for Phase 2 agents
#[derive(Debug)]
pub struct SignalEmitter {
    /// Agent name
    agent_name: String,
    /// Agent domain
    agent_domain: String,
    /// Ruvector client
    ruvector: RuvectorClient,
    /// Dry-run mode (don't persist)
    dry_run: bool,
}

impl SignalEmitter {
    /// Create a new signal emitter
    pub fn new(
        agent_name: impl Into<String>,
        agent_domain: impl Into<String>,
        ruvector: RuvectorClient,
        dry_run: bool,
    ) -> Self {
        Self {
            agent_name: agent_name.into(),
            agent_domain: agent_domain.into(),
            ruvector,
            dry_run,
        }
    }

    /// Emit an anomaly signal
    #[instrument(skip(self, payload), fields(signal_type = "anomaly"))]
    pub async fn emit_anomaly(
        &self,
        payload: AnomalyPayload,
        confidence: f64,
        latency_ms: u64,
    ) -> Result<Signal, SignalError> {
        self.emit_signal(
            SignalType::Anomaly,
            SignalPayload::Anomaly(payload),
            confidence,
            latency_ms,
            Vec::new(),
        )
        .await
    }

    /// Emit a drift signal
    #[instrument(skip(self, payload), fields(signal_type = "drift"))]
    pub async fn emit_drift(
        &self,
        payload: DriftPayload,
        confidence: f64,
        latency_ms: u64,
    ) -> Result<Signal, SignalError> {
        self.emit_signal(
            SignalType::Drift,
            SignalPayload::Drift(payload),
            confidence,
            latency_ms,
            Vec::new(),
        )
        .await
    }

    /// Emit a memory lineage signal
    #[instrument(skip(self, payload), fields(signal_type = "memory_lineage"))]
    pub async fn emit_lineage(
        &self,
        payload: MemoryLineagePayload,
        confidence: f64,
        latency_ms: u64,
        parent_ids: Vec<Uuid>,
    ) -> Result<Signal, SignalError> {
        self.emit_signal(
            SignalType::MemoryLineage,
            SignalPayload::MemoryLineage(payload),
            confidence,
            latency_ms,
            parent_ids,
        )
        .await
    }

    /// Emit a latency signal
    #[instrument(skip(self, payload), fields(signal_type = "latency"))]
    pub async fn emit_latency(
        &self,
        payload: LatencyPayload,
        confidence: f64,
        latency_ms: u64,
    ) -> Result<Signal, SignalError> {
        self.emit_signal(
            SignalType::Latency,
            SignalPayload::Latency(payload),
            confidence,
            latency_ms,
            Vec::new(),
        )
        .await
    }

    /// Internal signal emission with validation and persistence
    async fn emit_signal(
        &self,
        signal_type: SignalType,
        payload: SignalPayload,
        confidence: f64,
        latency_ms: u64,
        parent_ids: Vec<Uuid>,
    ) -> Result<Signal, SignalError> {
        // Build and validate signal
        let signal = Signal::builder(signal_type)
            .agent_name(&self.agent_name)
            .agent_domain(&self.agent_domain)
            .confidence(confidence)
            .payload(payload)
            .parents(parent_ids)
            .latency_ms(latency_ms)
            .build()?;

        debug!(
            signal_id = %signal.id,
            signal_type = %signal.signal_type,
            confidence = signal.confidence,
            "Signal validated"
        );

        // Persist to Ruvector (unless dry-run)
        if !self.dry_run {
            let record = signal.to_record();
            self.ruvector.store_signal(&record).await?;
            debug!(signal_id = %signal.id, "Signal persisted to Ruvector");
        } else {
            debug!(signal_id = %signal.id, "Dry-run: signal not persisted");
        }

        Ok(signal)
    }

    /// Record a lineage relationship between signals
    #[instrument(skip(self), fields(source = %source_id, target = %target_id))]
    pub async fn record_lineage(
        &self,
        source_id: Uuid,
        target_id: Uuid,
        relationship: LineageRelation,
    ) -> Result<(), SignalError> {
        if self.dry_run {
            debug!("Dry-run: lineage not recorded");
            return Ok(());
        }

        let delta = LineageDelta {
            delta_id: Uuid::new_v4(),
            source_id,
            target_id,
            relationship,
            timestamp: Utc::now(),
            metadata: None,
        };

        self.ruvector.store_lineage(&delta).await?;
        debug!(delta_id = %delta.delta_id, "Lineage delta recorded");

        Ok(())
    }

    /// Get agent name
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    /// Get agent domain
    pub fn agent_domain(&self) -> &str {
        &self.agent_domain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_builder() {
        let signal = Signal::builder(SignalType::Anomaly)
            .agent_name("test-agent")
            .agent_domain("detection")
            .confidence(0.85)
            .payload(SignalPayload::Anomaly(AnomalyPayload {
                service: "api".to_string(),
                model: "gpt-4".to_string(),
                metric: "latency".to_string(),
                observed_value: 150.0,
                baseline_value: 100.0,
                deviation: 3.5,
                detection_method: "zscore".to_string(),
                threshold: 3.0,
            }))
            .latency_ms(25)
            .build();

        assert!(signal.is_ok());
        let s = signal.unwrap();
        assert_eq!(s.signal_type, SignalType::Anomaly);
        assert_eq!(s.confidence, 0.85);
    }

    #[test]
    fn test_invalid_confidence() {
        let signal = Signal::builder(SignalType::Anomaly)
            .agent_name("test-agent")
            .agent_domain("detection")
            .confidence(1.5) // Invalid
            .payload(SignalPayload::Anomaly(AnomalyPayload {
                service: "api".to_string(),
                model: "gpt-4".to_string(),
                metric: "latency".to_string(),
                observed_value: 150.0,
                baseline_value: 100.0,
                deviation: 3.5,
                detection_method: "zscore".to_string(),
                threshold: 3.0,
            }))
            .build();

        assert!(matches!(signal, Err(SignalError::InvalidConfidence(_))));
    }

    #[test]
    fn test_signal_type_display() {
        assert_eq!(SignalType::Anomaly.to_string(), "anomaly");
        assert_eq!(SignalType::Drift.to_string(), "drift");
        assert_eq!(SignalType::MemoryLineage.to_string(), "memory_lineage");
        assert_eq!(SignalType::Latency.to_string(), "latency");
    }
}
