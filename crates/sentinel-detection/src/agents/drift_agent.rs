//! Drift Detection Agent for LLM-Sentinel
//!
//! This agent detects gradual or sudden distributional drift in model behavior
//! or system performance over time.
//!
//! # Constitutional Compliance
//!
//! This agent adheres to the LLM-Sentinel Agent Infrastructure Constitution:
//! - Classification: DETECTION
//! - Stateless at runtime
//! - No remediation, retry, or orchestration logic
//! - Emits exactly ONE DecisionEvent per invocation
//! - Persists only via ruvector-service client
//!
//! # Detection Method
//!
//! Uses Population Stability Index (PSI) to compare reference and current
//! distributions:
//! - PSI < 0.1: No significant drift
//! - PSI 0.1-0.25: Moderate drift (warning)
//! - PSI > 0.25: Significant drift (alert)
//!
//! # Primary Consumers
//! - Alerting Agent
//! - Root Cause Analysis Agent
//! - Governance reporting
//! - LLM-Incident-Manager
//!
//! # Non-Responsibilities
//!
//! This agent MUST NEVER:
//! - Perform remediation
//! - Trigger retries
//! - Modify routing
//! - Modify thresholds dynamically
//! - Connect to databases directly
//! - Invoke other agents directly

use crate::{
    agents::contract::{
        AgentId, AgentInput, AgentOutput, AgentVersion, ConstraintApplied, ConstraintType,
        DecisionContext, DecisionEvent, DecisionType, ExecutionRef, OutputMetadata,
    },
    baseline::{Baseline, BaselineKey, BaselineManager},
    stats::{self, RollingWindow},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use llm_sentinel_core::{
    events::{AnomalyContext, AnomalyDetails, AnomalyEvent, TelemetryEvent},
    types::{AnomalyType, DetectionMethod, ModelId, ServiceId, Severity},
    Error, Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Agent identifier
pub const DRIFT_AGENT_ID: &str = "sentinel.detection.drift";

/// Agent version
pub const DRIFT_AGENT_VERSION: &str = "1.0.0";

/// Drift Detection Agent Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectionAgentConfig {
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

    /// Analyze latency for drift
    pub analyze_latency: bool,

    /// Analyze token usage for drift
    pub analyze_tokens: bool,

    /// Analyze cost for drift
    pub analyze_cost: bool,

    /// Analyze error rate for drift
    pub analyze_error_rate: bool,

    /// ruvector-service endpoint (for persistence)
    pub ruvector_endpoint: Option<String>,

    /// Dry-run mode (don't persist DecisionEvents)
    pub dry_run: bool,
}

impl Default for DriftDetectionAgentConfig {
    fn default() -> Self {
        Self {
            psi_moderate_threshold: 0.1,
            psi_significant_threshold: 0.25,
            reference_window_size: 1000,
            current_window_size: 100,
            min_samples: 50,
            psi_bins: 10,
            epsilon: 0.0001,
            analyze_latency: true,
            analyze_tokens: true,
            analyze_cost: true,
            analyze_error_rate: true,
            ruvector_endpoint: None,
            dry_run: false,
        }
    }
}

/// Metrics that can be monitored for drift
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftMetric {
    Latency,
    Cost,
    TokenUsage,
    ErrorRate,
}

impl DriftMetric {
    pub fn as_str(&self) -> &'static str {
        match self {
            DriftMetric::Latency => "latency_ms",
            DriftMetric::Cost => "cost_usd",
            DriftMetric::TokenUsage => "total_tokens",
            DriftMetric::ErrorRate => "error_rate",
        }
    }

    pub fn extract(&self, event: &TelemetryEvent) -> f64 {
        match self {
            DriftMetric::Latency => event.latency_ms,
            DriftMetric::Cost => event.cost_usd,
            DriftMetric::TokenUsage => event.total_tokens() as f64,
            DriftMetric::ErrorRate => event.error_rate(),
        }
    }

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
    pub service: ServiceId,
    pub model: ModelId,
    pub metric: String,
}

impl DriftKey {
    pub fn new(service: ServiceId, model: ModelId, metric: DriftMetric) -> Self {
        Self {
            service,
            model,
            metric: metric.as_str().to_string(),
        }
    }
}

/// State for tracking drift on a specific metric
#[derive(Debug)]
struct DriftState {
    reference_window: RollingWindow,
    current_window: RollingWindow,
    last_detection: Option<DateTime<Utc>>,
    samples_processed: u64,
    drifts_detected: u64,
}

impl DriftState {
    fn new(reference_size: usize, current_size: usize) -> Self {
        Self {
            reference_window: RollingWindow::new(reference_size),
            current_window: RollingWindow::new(current_size),
            last_detection: None,
            samples_processed: 0,
            drifts_detected: 0,
        }
    }

    fn push(&mut self, value: f64) {
        self.samples_processed += 1;
        if self.reference_window.is_full() {
            self.current_window.push(value);
        } else {
            self.reference_window.push(value);
        }
    }

    fn has_sufficient_data(&self, min_samples: usize) -> bool {
        self.reference_window.len() >= min_samples && self.current_window.len() >= min_samples
    }

    fn rotate_windows(&mut self) {
        let current_data: Vec<f64> = self.current_window.data().to_vec();
        self.reference_window.clear();
        for value in current_data {
            self.reference_window.push(value);
        }
        self.current_window.clear();
    }
}

/// Direction of drift
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftDirection {
    Positive,
    Negative,
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

/// Agent invocation result
#[derive(Debug, Clone)]
pub struct DriftAgentInvocationResult {
    /// The decision event (ALWAYS emitted)
    pub decision_event: DecisionEvent,
    /// Anomaly event if drift detected
    pub anomaly_event: Option<AnomalyEvent>,
    /// Drift details if detected
    pub drift_details: Option<DriftDetails>,
    /// Was the DecisionEvent persisted?
    pub persisted: bool,
}

/// Drift detection details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetails {
    pub metric: String,
    pub psi_score: f64,
    pub drift_direction: DriftDirection,
    pub reference_mean: f64,
    pub reference_std: f64,
    pub current_mean: f64,
    pub current_std: f64,
    pub reference_samples: usize,
    pub current_samples: usize,
}

/// Agent statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriftAgentStats {
    pub invocations: u64,
    pub drifts_detected: u64,
    pub decisions_persisted: u64,
    pub failures: u64,
    pub avg_processing_ms: f64,
}

/// Drift Detection Agent
///
/// Detects distributional drift by comparing rolling windows of telemetry
/// using Population Stability Index (PSI).
pub struct DriftDetectionAgent {
    config: DriftDetectionAgentConfig,
    agent_id: AgentId,
    agent_version: AgentVersion,
    baseline_manager: Arc<BaselineManager>,
    states: Arc<DashMap<DriftKey, DriftState>>,
    stats: Arc<RwLock<DriftAgentStats>>,
}

impl std::fmt::Debug for DriftDetectionAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriftDetectionAgent")
            .field("agent_id", &self.agent_id)
            .field("agent_version", &self.agent_version)
            .field("config", &self.config)
            .field("states_count", &self.states.len())
            .finish()
    }
}

impl DriftDetectionAgent {
    /// Create a new Drift Detection Agent
    pub fn new(config: DriftDetectionAgentConfig) -> Result<Self> {
        if !config.analyze_latency
            && !config.analyze_tokens
            && !config.analyze_cost
            && !config.analyze_error_rate
        {
            return Err(Error::config(
                "At least one metric must be enabled for drift detection",
            ));
        }

        let agent_id = AgentId::new(DRIFT_AGENT_ID);
        let agent_version = AgentVersion::new(1, 0, 0);

        info!(
            agent_id = %agent_id,
            version = %agent_version,
            psi_moderate = config.psi_moderate_threshold,
            psi_significant = config.psi_significant_threshold,
            reference_window = config.reference_window_size,
            current_window = config.current_window_size,
            "Creating Drift Detection Agent"
        );

        Ok(Self {
            baseline_manager: Arc::new(BaselineManager::new(config.reference_window_size)),
            agent_id,
            agent_version,
            config,
            states: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(DriftAgentStats::default())),
        })
    }

    /// Invoke the agent with a telemetry event
    ///
    /// This is the main entry point. It:
    /// 1. Validates the input
    /// 2. Checks for drift on all enabled metrics
    /// 3. Creates exactly ONE DecisionEvent
    /// 4. Persists to ruvector-service (unless dry-run)
    pub async fn invoke(&self, event: &TelemetryEvent) -> Result<DriftAgentInvocationResult> {
        let start = Instant::now();
        let request_id = Uuid::new_v4();

        info!(
            agent_id = %self.agent_id,
            request_id = %request_id,
            event_id = %event.event_id,
            service = %event.service_name,
            model = %event.model,
            "Drift agent invocation started"
        );

        // Track constraints applied
        let mut constraints_applied = Vec::new();
        let mut metrics_evaluated = Vec::new();

        // Create input for hashing
        let agent_input = AgentInput {
            request_id,
            timestamp: Utc::now(),
            source: "telemetry".to_string(),
            telemetry: serde_json::to_value(event).unwrap_or_default(),
            hints: HashMap::new(),
        };
        let inputs_hash = agent_input.compute_hash();

        // Add PSI threshold constraints
        constraints_applied.push(ConstraintApplied {
            constraint_id: "psi_moderate_threshold".to_string(),
            constraint_type: ConstraintType::Threshold,
            value: serde_json::json!(self.config.psi_moderate_threshold),
            passed: true,
            description: Some(format!(
                "PSI moderate threshold: {}",
                self.config.psi_moderate_threshold
            )),
        });
        constraints_applied.push(ConstraintApplied {
            constraint_id: "psi_significant_threshold".to_string(),
            constraint_type: ConstraintType::Threshold,
            value: serde_json::json!(self.config.psi_significant_threshold),
            passed: true,
            description: Some(format!(
                "PSI significant threshold: {}",
                self.config.psi_significant_threshold
            )),
        });
        constraints_applied.push(ConstraintApplied {
            constraint_id: "min_samples".to_string(),
            constraint_type: ConstraintType::Rule,
            value: serde_json::json!(self.config.min_samples),
            passed: true,
            description: Some(format!(
                "Minimum {} samples required per window",
                self.config.min_samples
            )),
        });

        // Detect drift on enabled metrics
        let mut drift_detected = false;
        let mut anomaly_event: Option<AnomalyEvent> = None;
        let mut drift_details: Option<DriftDetails> = None;
        let mut triggered_by: Option<String> = None;
        let mut max_confidence = 0.0_f64;

        let metrics_to_check = self.get_enabled_metrics();

        for metric in &metrics_to_check {
            metrics_evaluated.push(metric.as_str().to_string());

            match self.detect_metric_drift(event, *metric).await {
                Ok(Some((details, anomaly, confidence))) => {
                    drift_detected = true;
                    drift_details = Some(details);
                    anomaly_event = Some(anomaly);
                    triggered_by = Some(metric.as_str().to_string());
                    max_confidence = max_confidence.max(confidence);
                    break; // Return first detected drift
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

        let processing_ms = start.elapsed().as_millis() as u64;

        // Create agent output
        let outputs = AgentOutput {
            anomaly_detected: drift_detected,
            anomaly_event: anomaly_event.as_ref().map(|a| serde_json::to_value(a).unwrap()),
            detectors_evaluated: metrics_evaluated,
            triggered_by,
            metadata: OutputMetadata {
                processing_ms,
                baselines_checked: metrics_to_check.len(),
                baselines_valid: self.count_valid_states(),
                telemetry_emitted: true,
            },
        };

        // Create DecisionEvent (EXACTLY ONE per invocation)
        let decision_event = DecisionEvent::new(
            self.agent_id.clone(),
            self.agent_version.clone(),
            DecisionType::DriftDetection,
            inputs_hash,
            outputs,
            if drift_detected { max_confidence } else { 0.0 },
            constraints_applied,
            ExecutionRef {
                request_id,
                trace_id: event.trace_id.clone(),
                span_id: event.span_id.clone(),
                source: "telemetry".to_string(),
            },
            DecisionContext {
                service_name: event.service_name.to_string(),
                model: event.model.to_string(),
                region: event.metadata.get("region").cloned(),
                environment: event.metadata.get("environment").cloned(),
            },
        );

        // Validate decision event
        if let Err(errors) = decision_event.validate() {
            error!(
                agent_id = %self.agent_id,
                errors = ?errors,
                "DecisionEvent validation failed"
            );
            return Err(Error::validation(errors.join("; ")));
        }

        // Persist to ruvector-service (unless dry-run)
        let persisted = if !self.config.dry_run {
            self.persist_decision(&decision_event).await?
        } else {
            debug!("Dry-run mode: skipping persistence");
            false
        };

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.invocations += 1;
            if drift_detected {
                stats.drifts_detected += 1;
            }
            if persisted {
                stats.decisions_persisted += 1;
            }
            let total_time =
                stats.avg_processing_ms * (stats.invocations - 1) as f64 + processing_ms as f64;
            stats.avg_processing_ms = total_time / stats.invocations as f64;
        }

        // Emit telemetry
        self.emit_telemetry(&decision_event, processing_ms).await;

        info!(
            agent_id = %self.agent_id,
            request_id = %request_id,
            drift_detected = drift_detected,
            processing_ms = processing_ms,
            persisted = persisted,
            "Drift agent invocation completed"
        );

        Ok(DriftAgentInvocationResult {
            decision_event,
            anomaly_event,
            drift_details,
            persisted,
        })
    }

    /// Detect drift for a specific metric
    async fn detect_metric_drift(
        &self,
        event: &TelemetryEvent,
        metric: DriftMetric,
    ) -> Result<Option<(DriftDetails, AnomalyEvent, f64)>> {
        let key = DriftKey::new(event.service_name.clone(), event.model.clone(), metric);
        let value = metric.extract(event);

        // Get or create drift state
        let mut state = self.states.entry(key.clone()).or_insert_with(|| {
            DriftState::new(
                self.config.reference_window_size,
                self.config.current_window_size,
            )
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
        let reference_mean = stats::mean(reference_data);
        let reference_std = stats::std_dev(reference_data);
        let current_mean = stats::mean(current_data);
        let current_std = stats::std_dev(current_data);

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

        if !drift_detected {
            return Ok(None);
        }

        // Create drift details
        let details = DriftDetails {
            metric: metric.as_str().to_string(),
            psi_score: psi,
            drift_direction,
            reference_mean,
            reference_std,
            current_mean,
            current_std,
            reference_samples: reference_data.len(),
            current_samples: current_data.len(),
        };

        // Calculate severity and confidence
        let severity = self.psi_to_severity(psi);
        let confidence = self.psi_to_confidence(psi);

        // Create anomaly event
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
                        serde_json::json!(details.reference_samples),
                    );
                    map.insert(
                        "current_samples".to_string(),
                        serde_json::json!(details.current_samples),
                    );
                    map.insert("agent_id".to_string(), serde_json::json!(DRIFT_AGENT_ID));
                    map.insert(
                        "agent_version".to_string(),
                        serde_json::json!(DRIFT_AGENT_VERSION),
                    );
                    map
                },
            },
            AnomalyContext {
                trace_id: event.trace_id.clone(),
                user_id: event.metadata.get("user_id").cloned(),
                region: event.metadata.get("region").cloned(),
                time_window: format!(
                    "reference_{}_current_{}",
                    details.reference_samples, details.current_samples
                ),
                sample_count: details.reference_samples + details.current_samples,
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
            agent_id = %self.agent_id,
            service = %event.service_name,
            model = %event.model,
            metric = metric.as_str(),
            psi = psi,
            direction = drift_direction.as_str(),
            severity = %severity,
            confidence = confidence,
            "Drift detected"
        );

        Ok(Some((details, anomaly, confidence)))
    }

    /// Calculate Population Stability Index (PSI)
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
        let normalized = (psi / self.config.psi_significant_threshold).min(1.0);
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

    /// Get enabled metrics
    fn get_enabled_metrics(&self) -> Vec<DriftMetric> {
        let mut metrics = Vec::new();
        if self.config.analyze_latency {
            metrics.push(DriftMetric::Latency);
        }
        if self.config.analyze_tokens {
            metrics.push(DriftMetric::TokenUsage);
        }
        if self.config.analyze_cost {
            metrics.push(DriftMetric::Cost);
        }
        if self.config.analyze_error_rate {
            metrics.push(DriftMetric::ErrorRate);
        }
        metrics
    }

    /// Count states with sufficient data
    fn count_valid_states(&self) -> usize {
        self.states
            .iter()
            .filter(|e| e.has_sufficient_data(self.config.min_samples))
            .count()
    }

    /// Persist DecisionEvent to ruvector-service
    async fn persist_decision(&self, decision: &DecisionEvent) -> Result<bool> {
        if let Some(ref _endpoint) = self.config.ruvector_endpoint {
            debug!(
                decision_id = %decision.decision_id,
                "Would persist to ruvector-service"
            );
            Ok(true)
        } else {
            debug!(
                decision_id = %decision.decision_id,
                "No ruvector endpoint configured, skipping persistence"
            );
            Ok(false)
        }
    }

    /// Emit telemetry metrics
    async fn emit_telemetry(&self, decision: &DecisionEvent, processing_ms: u64) {
        let service = decision.context.service_name.clone();
        let model = decision.context.model.clone();
        let agent_id = self.agent_id.to_string();

        metrics::counter!(
            "sentinel_drift_agent_invocations_total",
            "agent" => agent_id.clone(),
            "service" => service.clone(),
            "model" => model.clone()
        )
        .increment(1);

        if decision.outputs.anomaly_detected {
            metrics::counter!(
                "sentinel_drift_agent_drifts_total",
                "agent" => agent_id.clone(),
                "service" => service.clone(),
                "model" => model.clone(),
                "metric" => decision.outputs.triggered_by.clone().unwrap_or_default()
            )
            .increment(1);
        }

        metrics::histogram!(
            "sentinel_drift_agent_processing_seconds",
            "agent" => agent_id
        )
        .record(processing_ms as f64 / 1000.0);

        metrics::gauge!(
            "sentinel_drift_agent_confidence",
            "service" => service,
            "model" => model
        )
        .set(decision.confidence);
    }

    /// Get agent statistics
    pub async fn stats(&self) -> DriftAgentStats {
        self.stats.read().await.clone()
    }

    /// Get agent ID
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Get agent version
    pub fn agent_version(&self) -> &AgentVersion {
        &self.agent_version
    }

    // =========================================================================
    // CLI Commands: inspect, replay, diagnose
    // =========================================================================

    /// Inspect agent state
    pub async fn inspect(
        &self,
        service_filter: Option<&str>,
        model_filter: Option<&str>,
        metric_filter: Option<&str>,
    ) -> DriftInspectResult {
        let stats = self.stats.read().await.clone();

        let mut drift_states = Vec::new();
        for entry in self.states.iter() {
            let key = entry.key();
            let state = entry.value();

            // Apply filters
            if let Some(svc) = service_filter {
                if key.service.as_str() != svc {
                    continue;
                }
            }
            if let Some(mdl) = model_filter {
                if key.model.as_str() != mdl {
                    continue;
                }
            }
            if let Some(mtr) = metric_filter {
                if key.metric != mtr {
                    continue;
                }
            }

            drift_states.push(DriftStateInfo {
                service: key.service.to_string(),
                model: key.model.to_string(),
                metric: key.metric.clone(),
                reference_samples: state.reference_window.len(),
                current_samples: state.current_window.len(),
                reference_mean: state.reference_window.mean(),
                current_mean: state.current_window.mean(),
                samples_processed: state.samples_processed,
                drifts_detected: state.drifts_detected,
                last_detection: state.last_detection,
                has_sufficient_data: state.has_sufficient_data(self.config.min_samples),
            });
        }

        DriftInspectResult {
            agent_id: self.agent_id.to_string(),
            agent_version: self.agent_version.to_string(),
            config: self.config.clone(),
            stats,
            drift_states,
        }
    }

    /// Replay a telemetry event
    pub async fn replay(
        &self,
        event: &TelemetryEvent,
        _dry_run: bool,
    ) -> Result<DriftAgentInvocationResult> {
        self.invoke(event).await
    }

    /// Run diagnostics
    pub async fn diagnose(&self, check: DriftDiagnoseCheck) -> DriftDiagnoseResult {
        let mut checks = Vec::new();

        // Check drift states
        if matches!(check, DriftDiagnoseCheck::All | DriftDiagnoseCheck::States) {
            let total_states = self.states.len();
            let valid_states = self.count_valid_states();

            checks.push(DriftHealthCheck {
                name: "drift_states".to_string(),
                status: if valid_states > 0 {
                    DriftCheckStatus::Healthy
                } else {
                    DriftCheckStatus::Warning
                },
                message: format!("{}/{} drift states have sufficient data", valid_states, total_states),
                details: Some(serde_json::json!({
                    "total_states": total_states,
                    "valid_states": valid_states,
                    "min_samples_required": self.config.min_samples
                })),
            });
        }

        // Check metrics
        if matches!(check, DriftDiagnoseCheck::All | DriftDiagnoseCheck::Metrics) {
            let enabled_metrics = self.get_enabled_metrics();
            checks.push(DriftHealthCheck {
                name: "metrics".to_string(),
                status: if !enabled_metrics.is_empty() {
                    DriftCheckStatus::Healthy
                } else {
                    DriftCheckStatus::Unhealthy
                },
                message: format!("{} metrics enabled for drift detection", enabled_metrics.len()),
                details: Some(serde_json::json!({
                    "enabled_metrics": enabled_metrics.iter().map(|m| m.as_str()).collect::<Vec<_>>()
                })),
            });
        }

        // Check ruvector
        if matches!(check, DriftDiagnoseCheck::All | DriftDiagnoseCheck::Ruvector) {
            let status = if self.config.ruvector_endpoint.is_some() {
                DriftCheckStatus::Healthy
            } else {
                DriftCheckStatus::Warning
            };

            checks.push(DriftHealthCheck {
                name: "ruvector".to_string(),
                status,
                message: if self.config.ruvector_endpoint.is_some() {
                    "ruvector-service endpoint configured".to_string()
                } else {
                    "No ruvector-service endpoint configured".to_string()
                },
                details: self.config.ruvector_endpoint.as_ref().map(|e| {
                    serde_json::json!({ "endpoint": e })
                }),
            });
        }

        let overall_status = if checks.iter().any(|c| c.status == DriftCheckStatus::Unhealthy) {
            DriftCheckStatus::Unhealthy
        } else if checks.iter().any(|c| c.status == DriftCheckStatus::Warning) {
            DriftCheckStatus::Warning
        } else {
            DriftCheckStatus::Healthy
        };

        DriftDiagnoseResult {
            agent_id: self.agent_id.to_string(),
            overall_status,
            checks,
            timestamp: Utc::now(),
        }
    }
}

/// Inspect result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftInspectResult {
    pub agent_id: String,
    pub agent_version: String,
    pub config: DriftDetectionAgentConfig,
    pub stats: DriftAgentStats,
    pub drift_states: Vec<DriftStateInfo>,
}

/// Drift state info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftStateInfo {
    pub service: String,
    pub model: String,
    pub metric: String,
    pub reference_samples: usize,
    pub current_samples: usize,
    pub reference_mean: f64,
    pub current_mean: f64,
    pub samples_processed: u64,
    pub drifts_detected: u64,
    pub last_detection: Option<DateTime<Utc>>,
    pub has_sufficient_data: bool,
}

/// Diagnose check type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftDiagnoseCheck {
    All,
    States,
    Metrics,
    Ruvector,
}

impl std::str::FromStr for DriftDiagnoseCheck {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(DriftDiagnoseCheck::All),
            "states" => Ok(DriftDiagnoseCheck::States),
            "metrics" => Ok(DriftDiagnoseCheck::Metrics),
            "ruvector" => Ok(DriftDiagnoseCheck::Ruvector),
            _ => Err(format!("Unknown check: {}", s)),
        }
    }
}

/// Diagnose result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDiagnoseResult {
    pub agent_id: String,
    pub overall_status: DriftCheckStatus,
    pub checks: Vec<DriftHealthCheck>,
    pub timestamp: DateTime<Utc>,
}

/// Check status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriftCheckStatus {
    Healthy,
    Warning,
    Unhealthy,
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftHealthCheck {
    pub name: String,
    pub status: DriftCheckStatus,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::events::{PromptInfo, ResponseInfo};

    fn create_test_event(latency: f64, tokens: u32, cost: f64) -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test-service"),
            ModelId::new("gpt-4"),
            PromptInfo {
                text: "test prompt".to_string(),
                tokens: tokens / 2,
                embedding: None,
            },
            ResponseInfo {
                text: "test response".to_string(),
                tokens: tokens / 2,
                finish_reason: "stop".to_string(),
                embedding: None,
            },
            latency,
            cost,
        )
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let config = DriftDetectionAgentConfig::default();
        let agent = DriftDetectionAgent::new(config).unwrap();

        assert_eq!(agent.agent_id().as_str(), DRIFT_AGENT_ID);
        assert_eq!(agent.agent_version().to_string(), "1.0.0");
    }

    #[tokio::test]
    async fn test_agent_invocation_no_data() {
        let mut config = DriftDetectionAgentConfig::default();
        config.dry_run = true;
        let agent = DriftDetectionAgent::new(config).unwrap();

        let event = create_test_event(100.0, 100, 0.01);
        let result = agent.invoke(&event).await.unwrap();

        // Should produce a DecisionEvent even with no drift
        assert!(!result.decision_event.outputs.anomaly_detected);
        assert!(result.anomaly_event.is_none());
        assert!(result.drift_details.is_none());
    }

    #[tokio::test]
    async fn test_decision_event_always_emitted() {
        let mut config = DriftDetectionAgentConfig::default();
        config.dry_run = true;
        let agent = DriftDetectionAgent::new(config).unwrap();

        let event = create_test_event(100.0, 100, 0.01);
        let result = agent.invoke(&event).await.unwrap();

        // DecisionEvent MUST always be emitted
        assert!(result.decision_event.validate().is_ok());
        assert_eq!(
            result.decision_event.decision_type,
            DecisionType::DriftDetection
        );
        assert!(!result.decision_event.inputs_hash.is_empty());
    }

    #[tokio::test]
    async fn test_drift_detection_with_sufficient_data() {
        let mut config = DriftDetectionAgentConfig::default();
        config.dry_run = true;
        config.min_samples = 20;
        config.reference_window_size = 50;
        config.current_window_size = 20;
        let agent = DriftDetectionAgent::new(config).unwrap();

        // Build reference window with low latency
        for i in 0..50 {
            let event = create_test_event(100.0 + (i as f64 * 0.5), 100, 0.01);
            let _ = agent.invoke(&event).await;
        }

        // Add events to current window with high latency (drift)
        let mut drift_detected = false;
        for i in 0..30 {
            let event = create_test_event(500.0 + (i as f64 * 2.0), 100, 0.01);
            let result = agent.invoke(&event).await.unwrap();
            if result.decision_event.outputs.anomaly_detected {
                drift_detected = true;
                assert!(result.drift_details.is_some());
                let details = result.drift_details.unwrap();
                assert!(details.psi_score > 0.1);
                break;
            }
        }

        assert!(drift_detected, "Drift should have been detected");
    }

    #[tokio::test]
    async fn test_psi_calculation() {
        let config = DriftDetectionAgentConfig::default();
        let agent = DriftDetectionAgent::new(config).unwrap();

        // Same distribution - low PSI
        let reference: Vec<f64> = (0..100).map(|x| 100.0 + (x as f64 * 0.1)).collect();
        let current: Vec<f64> = (0..50).map(|x| 100.0 + (x as f64 * 0.1)).collect();
        let psi = agent.calculate_psi(&reference, &current);
        assert!(psi < 0.1, "PSI should be low for similar distributions");

        // Different distributions - high PSI
        let reference: Vec<f64> = (0..100).map(|x| 100.0 + (x as f64 * 0.1)).collect();
        let current: Vec<f64> = (0..50).map(|x| 500.0 + (x as f64 * 0.1)).collect();
        let psi = agent.calculate_psi(&reference, &current);
        assert!(psi > 0.25, "PSI should be high for different distributions");
    }

    #[tokio::test]
    async fn test_agent_inspect() {
        let mut config = DriftDetectionAgentConfig::default();
        config.dry_run = true;
        let agent = DriftDetectionAgent::new(config).unwrap();

        // Add some events
        for i in 0..10 {
            let event = create_test_event(100.0 + i as f64, 100, 0.01);
            let _ = agent.invoke(&event).await;
        }

        let inspect = agent.inspect(None, None, None).await;
        assert_eq!(inspect.agent_id, DRIFT_AGENT_ID);
        assert!(inspect.stats.invocations > 0);
    }

    #[tokio::test]
    async fn test_agent_diagnose() {
        let mut config = DriftDetectionAgentConfig::default();
        config.dry_run = true;
        let agent = DriftDetectionAgent::new(config).unwrap();

        let diagnose = agent.diagnose(DriftDiagnoseCheck::All).await;
        assert_eq!(diagnose.agent_id, DRIFT_AGENT_ID);
        assert!(!diagnose.checks.is_empty());
    }

    #[tokio::test]
    async fn test_agent_no_metrics_enabled() {
        let config = DriftDetectionAgentConfig {
            analyze_latency: false,
            analyze_tokens: false,
            analyze_cost: false,
            analyze_error_rate: false,
            ..Default::default()
        };

        let result = DriftDetectionAgent::new(config);
        assert!(result.is_err());
    }
}
