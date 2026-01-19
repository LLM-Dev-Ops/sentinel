//! Agent Contract Definitions for LLM-Sentinel
//!
//! This module defines the NON-NEGOTIABLE schemas and contracts for
//! LLM-Sentinel agents as specified in the Agent Infrastructure Constitution.
//!
//! # Agent Contract: Anomaly Detection Agent
//!
//! ## Purpose Statement (precise, non-marketing)
//! Detect statistically significant deviations in live telemetry signals
//! relative to established baselines. Operates on real-time metrics using
//! configurable detection algorithms (Z-score, IQR, MAD, CUSUM) and emits
//! anomaly events with severity levels.
//!
//! ## Classification
//! - Type: DETECTION
//!
//! ## Decision Type Semantics
//! - decision_type: "anomaly_detection"
//! - Emitted when: statistically significant deviation detected OR
//!                 no deviation detected (decision recorded either way)
//!
//! ## Confidence Semantics
//! - Statistical confidence derived from detection algorithm
//! - Range: 0.0 - 1.0
//! - Z-score: confidence = 1.0 - (1.0 / (1.0 + |z-score| - threshold))
//! - IQR: confidence = 1.0 - (1.0 / (1.0 + iqr_factor))
//!
//! ## Constraints Applied Semantics
//! - Threshold identifiers (e.g., "zscore_3.0", "iqr_1.5")
//! - Rule identifiers (e.g., "min_samples_10", "baseline_valid")
//! - Detector identifiers (e.g., "detector:zscore", "detector:iqr")
//!
//! ## What This Agent MUST NEVER Do
//! - Perform remediation
//! - Trigger retries
//! - Modify routing
//! - Modify policies
//! - Modify thresholds dynamically
//! - Call external notification systems directly
//! - Invoke other agents directly
//!
//! ## Primary Consumers
//! - Alerting Agent
//! - Incident Correlation Agent
//! - LLM-Observatory dashboards
//! - LLM-Incident-Manager

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

/// Agent identifier following agentics-contracts specification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new agent ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the agent ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Anomaly Detection Agent ID
    pub fn anomaly_detection() -> Self {
        Self::new("sentinel.detection.anomaly")
    }

    /// Alerting Agent ID
    pub fn alerting() -> Self {
        Self::new("sentinel.alerting.evaluation")
    }

    /// Root Cause Analysis Agent ID
    pub fn root_cause_analysis() -> Self {
        Self::new("sentinel.analysis.rca")
    }

    /// Incident Correlation Agent ID
    pub fn incident_correlation() -> Self {
        Self::new("sentinel.correlation.incident")
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Semantic version for agent versioning
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl AgentVersion {
    /// Create a new version
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Current version of the Anomaly Detection Agent
    pub fn current() -> Self {
        Self::new(1, 0, 0)
    }
}

impl std::fmt::Display for AgentVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Decision type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    /// Anomaly detection decision
    AnomalyDetection,
    /// Drift detection decision
    DriftDetection,
    /// Alert evaluation decision
    AlertEvaluation,
    /// Signal correlation decision
    SignalCorrelation,
    /// Root cause analysis decision
    RootCauseAnalysis,
    /// Incident correlation decision
    IncidentCorrelation,
}

impl std::fmt::Display for DecisionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionType::AnomalyDetection => write!(f, "anomaly_detection"),
            DecisionType::DriftDetection => write!(f, "drift_detection"),
            DecisionType::AlertEvaluation => write!(f, "alert_evaluation"),
            DecisionType::SignalCorrelation => write!(f, "signal_correlation"),
            DecisionType::RootCauseAnalysis => write!(f, "root_cause_analysis"),
            DecisionType::IncidentCorrelation => write!(f, "incident_correlation"),
        }
    }
}

/// Constraint applied during detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintApplied {
    /// Constraint identifier (e.g., "zscore_threshold", "min_samples")
    pub constraint_id: String,
    /// Constraint type (threshold, rule, detector)
    pub constraint_type: ConstraintType,
    /// Configured value
    pub value: serde_json::Value,
    /// Whether constraint passed
    pub passed: bool,
    /// Description of the constraint
    pub description: Option<String>,
}

/// Type of constraint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
    /// Statistical threshold (e.g., Z-score threshold)
    Threshold,
    /// Business rule (e.g., minimum samples required)
    Rule,
    /// Detector selection (e.g., which algorithm used)
    Detector,
    /// Baseline requirement (e.g., valid baseline exists)
    Baseline,
    /// Analysis constraint (e.g., correlation strength, hypothesis ranking)
    Analysis,
    /// Temporal constraint (e.g., time window, sequence ordering)
    Temporal,
}

/// Agent input following agentics-contracts schema
///
/// This wraps the TelemetryEvent with additional metadata for agent processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInput {
    /// Unique request identifier
    pub request_id: Uuid,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
    /// Source system (e.g., "llm-observatory")
    pub source: String,
    /// Telemetry event data
    pub telemetry: serde_json::Value,
    /// Processing hints
    pub hints: HashMap<String, serde_json::Value>,
}

impl AgentInput {
    /// Compute SHA-256 hash of inputs for DecisionEvent
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let json = serde_json::to_string(&self).unwrap_or_default();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Agent output following agentics-contracts schema
///
/// This wraps the detection result with decision metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    /// Whether an anomaly was detected
    pub anomaly_detected: bool,
    /// Anomaly event if detected (null otherwise)
    pub anomaly_event: Option<serde_json::Value>,
    /// List of detectors that were evaluated
    pub detectors_evaluated: Vec<String>,
    /// Detector that triggered the anomaly (if any)
    pub triggered_by: Option<String>,
    /// Processing metadata
    pub metadata: OutputMetadata,
}

/// Output metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputMetadata {
    /// Processing duration in milliseconds
    pub processing_ms: u64,
    /// Number of baselines checked
    pub baselines_checked: usize,
    /// Number of baselines valid
    pub baselines_valid: usize,
    /// Telemetry emitted
    pub telemetry_emitted: bool,
}

/// DecisionEvent - MUST be emitted exactly ONCE per agent invocation
///
/// This is the canonical output record persisted to ruvector-service.
/// Per the LLM-Sentinel Constitution, every agent invocation MUST produce
/// exactly ONE DecisionEvent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEvent {
    /// Unique decision identifier
    pub decision_id: Uuid,

    /// Agent identifier (from agentics-contracts)
    pub agent_id: AgentId,

    /// Agent version (semantic versioning)
    pub agent_version: AgentVersion,

    /// Decision type classification
    pub decision_type: DecisionType,

    /// SHA-256 hash of input data for auditability
    pub inputs_hash: String,

    /// Agent output
    pub outputs: AgentOutput,

    /// Statistical confidence score (0.0 - 1.0)
    pub confidence: f64,

    /// Constraints that were applied during processing
    pub constraints_applied: Vec<ConstraintApplied>,

    /// Reference to the triggering execution (trace_id, request_id)
    pub execution_ref: ExecutionRef,

    /// Decision timestamp (UTC)
    pub timestamp: DateTime<Utc>,

    /// Service/model context
    pub context: DecisionContext,
}

/// Execution reference for tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRef {
    /// Request ID
    pub request_id: Uuid,
    /// Trace ID (if available)
    pub trace_id: Option<String>,
    /// Span ID (if available)
    pub span_id: Option<String>,
    /// Source system
    pub source: String,
}

/// Context information for the decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContext {
    /// Service name
    pub service_name: String,
    /// Model identifier
    pub model: String,
    /// Region (if available)
    pub region: Option<String>,
    /// Environment (e.g., "production", "staging")
    pub environment: Option<String>,
}

impl DecisionEvent {
    /// Create a new DecisionEvent
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent_id: AgentId,
        agent_version: AgentVersion,
        decision_type: DecisionType,
        inputs_hash: String,
        outputs: AgentOutput,
        confidence: f64,
        constraints_applied: Vec<ConstraintApplied>,
        execution_ref: ExecutionRef,
        context: DecisionContext,
    ) -> Self {
        Self {
            decision_id: Uuid::new_v4(),
            agent_id,
            agent_version,
            decision_type,
            inputs_hash,
            outputs,
            confidence: confidence.clamp(0.0, 1.0),
            constraints_applied,
            execution_ref,
            timestamp: Utc::now(),
            context,
        }
    }

    /// Validate the DecisionEvent against the schema
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate confidence range
        if !(0.0..=1.0).contains(&self.confidence) {
            errors.push(format!(
                "confidence must be between 0.0 and 1.0, got {}",
                self.confidence
            ));
        }

        // Validate at least one constraint was applied
        if self.constraints_applied.is_empty() {
            errors.push("at least one constraint must be applied".to_string());
        }

        // Validate context
        if self.context.service_name.is_empty() {
            errors.push("context.service_name cannot be empty".to_string());
        }
        if self.context.model.is_empty() {
            errors.push("context.model cannot be empty".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Convert to JSON for ruvector-service persistence
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// CLI invocation specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliContract {
    /// Agent name for CLI
    pub agent_name: String,
    /// Available commands
    pub commands: Vec<CliCommand>,
}

/// CLI command specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliCommand {
    /// Command name (e.g., "inspect", "replay", "diagnose")
    pub name: String,
    /// Command description
    pub description: String,
    /// Required arguments
    pub required_args: Vec<CliArg>,
    /// Optional arguments
    pub optional_args: Vec<CliArg>,
    /// Example invocation
    pub example: String,
}

/// CLI argument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliArg {
    /// Argument name
    pub name: String,
    /// Argument type
    pub arg_type: String,
    /// Description
    pub description: String,
}

impl CliContract {
    /// Generate CLI contract for Anomaly Detection Agent
    pub fn anomaly_detection_agent() -> Self {
        Self {
            agent_name: "anomaly-detection".to_string(),
            commands: vec![
                CliCommand {
                    name: "inspect".to_string(),
                    description: "Inspect agent state, baselines, and configuration".to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        CliArg {
                            name: "--service".to_string(),
                            arg_type: "string".to_string(),
                            description: "Filter by service name".to_string(),
                        },
                        CliArg {
                            name: "--model".to_string(),
                            arg_type: "string".to_string(),
                            description: "Filter by model name".to_string(),
                        },
                        CliArg {
                            name: "--json".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Output as JSON".to_string(),
                        },
                    ],
                    example: "sentinel agent anomaly-detection inspect --service chat-api --json"
                        .to_string(),
                },
                CliCommand {
                    name: "replay".to_string(),
                    description: "Replay a telemetry event through the agent".to_string(),
                    required_args: vec![CliArg {
                        name: "--event-id".to_string(),
                        arg_type: "uuid".to_string(),
                        description: "Event ID to replay".to_string(),
                    }],
                    optional_args: vec![
                        CliArg {
                            name: "--dry-run".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Don't persist DecisionEvent".to_string(),
                        },
                        CliArg {
                            name: "--verbose".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Show detailed processing steps".to_string(),
                        },
                    ],
                    example:
                        "sentinel agent anomaly-detection replay --event-id abc123 --dry-run"
                            .to_string(),
                },
                CliCommand {
                    name: "diagnose".to_string(),
                    description: "Run diagnostics on agent health and configuration".to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        CliArg {
                            name: "--check".to_string(),
                            arg_type: "string".to_string(),
                            description:
                                "Specific check to run (baselines, detectors, ruvector, all)"
                                    .to_string(),
                        },
                        CliArg {
                            name: "--fix".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Attempt to fix issues".to_string(),
                        },
                    ],
                    example: "sentinel agent anomaly-detection diagnose --check all".to_string(),
                },
            ],
        }
    }

    /// Generate CLI contract for Alerting Agent
    pub fn alerting_agent() -> Self {
        Self {
            agent_name: "alerting".to_string(),
            commands: vec![
                CliCommand {
                    name: "inspect".to_string(),
                    description: "Inspect alerting rules, thresholds, and suppression state"
                        .to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        CliArg {
                            name: "--service".to_string(),
                            arg_type: "string".to_string(),
                            description: "Filter by service name".to_string(),
                        },
                        CliArg {
                            name: "--severity".to_string(),
                            arg_type: "string".to_string(),
                            description: "Filter by severity (low, medium, high, critical)"
                                .to_string(),
                        },
                        CliArg {
                            name: "--json".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Output as JSON".to_string(),
                        },
                    ],
                    example: "sentinel agent alerting inspect --service chat-api --json".to_string(),
                },
                CliCommand {
                    name: "replay".to_string(),
                    description: "Replay an anomaly event through alerting evaluation".to_string(),
                    required_args: vec![CliArg {
                        name: "--anomaly-id".to_string(),
                        arg_type: "uuid".to_string(),
                        description: "Anomaly event ID to replay".to_string(),
                    }],
                    optional_args: vec![
                        CliArg {
                            name: "--dry-run".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Don't persist DecisionEvent or emit alert".to_string(),
                        },
                        CliArg {
                            name: "--ignore-suppression".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Bypass suppression rules for testing".to_string(),
                        },
                        CliArg {
                            name: "--verbose".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Show detailed evaluation steps".to_string(),
                        },
                    ],
                    example: "sentinel agent alerting replay --anomaly-id abc123 --dry-run"
                        .to_string(),
                },
                CliCommand {
                    name: "diagnose".to_string(),
                    description: "Run diagnostics on alerting agent health and configuration"
                        .to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        CliArg {
                            name: "--check".to_string(),
                            arg_type: "string".to_string(),
                            description: "Specific check (rules, thresholds, ruvector, all)"
                                .to_string(),
                        },
                        CliArg {
                            name: "--validate-rules".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Validate all alerting rules".to_string(),
                        },
                    ],
                    example: "sentinel agent alerting diagnose --check all".to_string(),
                },
            ],
        }
    }
}

/// Failure modes for the Anomaly Detection Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureMode {
    /// No valid baseline available for detection
    NoValidBaseline {
        service: String,
        model: String,
        metric: String,
    },
    /// Input validation failed
    InputValidationFailed { errors: Vec<String> },
    /// ruvector-service persistence failed
    PersistenceFailed { error: String },
    /// Detector execution error
    DetectorError { detector: String, error: String },
    /// Configuration error
    ConfigurationError { error: String },
}

impl std::fmt::Display for FailureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureMode::NoValidBaseline {
                service,
                model,
                metric,
            } => {
                write!(
                    f,
                    "No valid baseline for {}/{}/{}",
                    service, model, metric
                )
            }
            FailureMode::InputValidationFailed { errors } => {
                write!(f, "Input validation failed: {}", errors.join(", "))
            }
            FailureMode::PersistenceFailed { error } => {
                write!(f, "Persistence failed: {}", error)
            }
            FailureMode::DetectorError { detector, error } => {
                write!(f, "Detector {} error: {}", detector, error)
            }
            FailureMode::ConfigurationError { error } => {
                write!(f, "Configuration error: {}", error)
            }
        }
    }
}

// =============================================================================
// ALERTING AGENT CONTRACT
// =============================================================================
//
// # Agent Contract: Alerting Agent
//
// ## Purpose Statement (precise, non-marketing)
// Evaluate detected anomalies and drift events against alerting rules to
// determine whether alerts should be raised. Applies threshold evaluation,
// deduplication, and suppression rules to control alert volume.
//
// ## Classification
// - Type: ALERTING
//
// ## Decision Type Semantics
// - decision_type: "alert_evaluation"
// - Emitted when: anomaly/drift event evaluated against alerting rules
//   (decision recorded whether alert is raised or suppressed)
//
// ## Confidence Semantics
// - Certainty score for alert decision (0.0 - 1.0)
// - Combines: anomaly confidence, rule match strength, suppression state
// - High confidence = clear threshold breach with no suppression
// - Low confidence = borderline threshold or active suppression
//
// ## Constraints Applied Semantics
// - Threshold rules (e.g., "severity_threshold:high")
// - Suppression rules (e.g., "dedup_window:300s", "maintenance_window")
// - Rate limit rules (e.g., "max_alerts_per_hour:100")
// - Routing rules (e.g., "route:pagerduty", "route:slack")
//
// ## What This Agent MUST NEVER Do
// - Send notifications directly (email, pager, slack)
// - Perform remediation
// - Trigger retries
// - Modify routing configuration
// - Modify policies
// - Modify thresholds dynamically
// - Invoke other agents directly
// - Execute incident workflows
//
// ## Primary Consumers
// - LLM-Incident-Manager (consumes alert events)
// - Governance dashboards (audit trail)
// - LLM-Observatory (telemetry visualization)
//
// ## Failure Modes
// - AlertingRuleNotFound
// - SuppressionCheckFailed
// - InputValidationFailed
// - PersistenceFailed
// - RateLimitExceeded
// =============================================================================

/// Input for Alerting Agent - wraps anomaly event with evaluation context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingAgentInput {
    /// Unique request identifier
    pub request_id: Uuid,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
    /// Source system (e.g., "sentinel.detection.anomaly")
    pub source: String,
    /// Anomaly event to evaluate
    pub anomaly: serde_json::Value,
    /// Evaluation hints (e.g., force_alert, ignore_suppression)
    pub hints: HashMap<String, serde_json::Value>,
}

impl AlertingAgentInput {
    /// Create new alerting input from anomaly event
    pub fn from_anomaly(anomaly: serde_json::Value, source: impl Into<String>) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: source.into(),
            anomaly,
            hints: HashMap::new(),
        }
    }

    /// Add evaluation hint
    pub fn with_hint(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.hints.insert(key.into(), value);
        self
    }

    /// Compute SHA-256 hash of inputs for DecisionEvent
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let json = serde_json::to_string(&self).unwrap_or_default();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Alert evaluation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertEvaluationStatus {
    /// Alert raised - will be sent to incident manager
    Raised,
    /// Alert suppressed by deduplication
    Deduplicated,
    /// Alert suppressed by maintenance window
    MaintenanceSuppressed,
    /// Alert suppressed by rate limiting
    RateLimited,
    /// Alert suppressed by explicit suppression rule
    RuleSuppressed,
    /// Alert below severity threshold
    BelowThreshold,
    /// Evaluation failed
    EvaluationFailed,
}

impl std::fmt::Display for AlertEvaluationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertEvaluationStatus::Raised => write!(f, "raised"),
            AlertEvaluationStatus::Deduplicated => write!(f, "deduplicated"),
            AlertEvaluationStatus::MaintenanceSuppressed => write!(f, "maintenance_suppressed"),
            AlertEvaluationStatus::RateLimited => write!(f, "rate_limited"),
            AlertEvaluationStatus::RuleSuppressed => write!(f, "rule_suppressed"),
            AlertEvaluationStatus::BelowThreshold => write!(f, "below_threshold"),
            AlertEvaluationStatus::EvaluationFailed => write!(f, "evaluation_failed"),
        }
    }
}

/// Output for Alerting Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingAgentOutput {
    /// Whether alert was raised
    pub alert_raised: bool,
    /// Evaluation status
    pub status: AlertEvaluationStatus,
    /// Alert event if raised (serialized AlertEvent)
    pub alert_event: Option<serde_json::Value>,
    /// Rules that were evaluated
    pub rules_evaluated: Vec<String>,
    /// Rule that triggered the decision (if any)
    pub triggered_by: Option<String>,
    /// Decision reasoning
    pub reasoning: String,
    /// Processing metadata
    pub metadata: AlertingOutputMetadata,
}

/// Metadata for alerting output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingOutputMetadata {
    /// Processing duration in milliseconds
    pub processing_ms: u64,
    /// Number of suppression rules checked
    pub suppression_rules_checked: usize,
    /// Number of threshold rules checked
    pub threshold_rules_checked: usize,
    /// Deduplication window status
    pub deduplication_active: bool,
    /// Telemetry emitted
    pub telemetry_emitted: bool,
}

/// Failure modes for the Alerting Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertingFailureMode {
    /// Alerting rule not found
    AlertingRuleNotFound { rule_id: String },
    /// Suppression check failed
    SuppressionCheckFailed { error: String },
    /// Input validation failed
    InputValidationFailed { errors: Vec<String> },
    /// ruvector-service persistence failed
    PersistenceFailed { error: String },
    /// Rate limit exceeded
    RateLimitExceeded {
        limit: u32,
        window_secs: u64,
        current_count: u32,
    },
    /// Configuration error
    ConfigurationError { error: String },
}

impl std::fmt::Display for AlertingFailureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertingFailureMode::AlertingRuleNotFound { rule_id } => {
                write!(f, "Alerting rule not found: {}", rule_id)
            }
            AlertingFailureMode::SuppressionCheckFailed { error } => {
                write!(f, "Suppression check failed: {}", error)
            }
            AlertingFailureMode::InputValidationFailed { errors } => {
                write!(f, "Input validation failed: {}", errors.join(", "))
            }
            AlertingFailureMode::PersistenceFailed { error } => {
                write!(f, "Persistence failed: {}", error)
            }
            AlertingFailureMode::RateLimitExceeded {
                limit,
                window_secs,
                current_count,
            } => {
                write!(
                    f,
                    "Rate limit exceeded: {}/{} in {}s",
                    current_count, limit, window_secs
                )
            }
            AlertingFailureMode::ConfigurationError { error } => {
                write!(f, "Configuration error: {}", error)
            }
        }
    }
}

/// Alerting rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingRule {
    /// Unique rule identifier
    pub rule_id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: Option<String>,
    /// Whether rule is enabled
    pub enabled: bool,
    /// Severity threshold (minimum severity to alert)
    pub severity_threshold: Option<String>,
    /// Anomaly types this rule applies to
    pub anomaly_types: Vec<String>,
    /// Services this rule applies to (empty = all)
    pub services: Vec<String>,
    /// Models this rule applies to (empty = all)
    pub models: Vec<String>,
    /// Confidence threshold (minimum confidence to alert)
    pub confidence_threshold: f64,
    /// Suppression configuration
    pub suppression: Option<SuppressionConfig>,
    /// Priority (higher = evaluated first)
    pub priority: u32,
}

/// Suppression configuration for alerting rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuppressionConfig {
    /// Enable deduplication
    pub deduplication_enabled: bool,
    /// Deduplication window in seconds
    pub deduplication_window_secs: u64,
    /// Maintenance windows (cron expressions)
    pub maintenance_windows: Vec<String>,
    /// Max alerts per hour (0 = unlimited)
    pub max_alerts_per_hour: u32,
}

// =============================================================================
// ROOT CAUSE ANALYSIS AGENT CONTRACT
// =============================================================================
//
// # Agent Contract: Root Cause Analysis Agent
//
// ## Purpose Statement (precise, non-marketing)
// Perform signal-level root cause analysis on correlated telemetry signals
// to identify the most likely sources contributing to detected incidents.
// Analyzes temporal patterns, service dependencies, and metric correlations
// to produce ranked root cause hypotheses with confidence scores.
//
// ## Classification
// - Type: ANALYSIS
//
// ## Decision Type Semantics
// - decision_type: "root_cause_analysis"
// - Emitted when: correlated anomaly signals analyzed for root cause
//   (decision recorded whether root cause identified or inconclusive)
//
// ## Confidence Semantics
// - Analytical confidence for root cause hypothesis (0.0 - 1.0)
// - Combines: temporal correlation strength, service dependency weight,
//   metric deviation magnitude, historical pattern match
// - High confidence = strong temporal correlation + clear causal chain
// - Low confidence = weak correlation or multiple competing hypotheses
//
// ## Constraints Applied Semantics
// - Analysis rules (e.g., "min_correlation_strength:0.7")
// - Temporal constraints (e.g., "max_lag_seconds:300", "sequence_order")
// - Dependency constraints (e.g., "service_dependency_weight")
// - Hypothesis ranking (e.g., "top_k_hypotheses:5")
//
// ## What This Agent MUST NEVER Do
// - Perform remediation
// - Trigger retries
// - Modify routing
// - Modify policies
// - Modify thresholds dynamically
// - Call external notification systems directly
// - Invoke other agents directly
// - Execute incident workflows
// - Make automated decisions based on root cause (that's Incident Manager)
//
// ## Primary Consumers
// - LLM-Incident-Manager (root cause hypotheses inform incident response)
// - Post-mortem workflows (historical RCA data)
// - Governance reviews (audit trail of analysis)
// - LLM-Observatory (visualization of causal chains)
//
// ## Failure Modes
// - InsufficientSignals (not enough correlated data)
// - TemporalGapTooLarge (signals too far apart in time)
// - NoCausalChainFound (no clear dependency path)
// - InputValidationFailed
// - PersistenceFailed
// - AnalysisTimeout
// =============================================================================

/// Input for Root Cause Analysis Agent - wraps correlated anomaly signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaAgentInput {
    /// Unique request identifier
    pub request_id: Uuid,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
    /// Source system (e.g., "sentinel.alerting", "incident-manager")
    pub source: String,
    /// Primary incident/anomaly being analyzed
    pub primary_incident: RcaIncidentSignal,
    /// Correlated signals to analyze
    pub correlated_signals: Vec<RcaCorrelatedSignal>,
    /// Service dependency graph (if available)
    pub service_dependencies: Option<serde_json::Value>,
    /// Analysis hints (e.g., suspected_service, time_window)
    pub hints: HashMap<String, serde_json::Value>,
}

/// Primary incident signal for RCA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaIncidentSignal {
    /// Incident/anomaly ID
    pub incident_id: Uuid,
    /// Service where incident was detected
    pub service_name: String,
    /// Model involved (if applicable)
    pub model: Option<String>,
    /// Incident timestamp
    pub timestamp: DateTime<Utc>,
    /// Incident severity
    pub severity: String,
    /// Incident type (e.g., "latency_spike", "error_rate_increase")
    pub incident_type: String,
    /// Affected metric
    pub metric: String,
    /// Observed value
    pub observed_value: f64,
    /// Baseline value
    pub baseline_value: f64,
    /// Additional context
    pub context: HashMap<String, serde_json::Value>,
}

/// Correlated signal for RCA analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaCorrelatedSignal {
    /// Signal ID
    pub signal_id: Uuid,
    /// Source service
    pub service_name: String,
    /// Signal timestamp
    pub timestamp: DateTime<Utc>,
    /// Signal type
    pub signal_type: String,
    /// Metric name
    pub metric: String,
    /// Observed value
    pub value: f64,
    /// Correlation coefficient with primary incident
    pub correlation: f64,
    /// Time lag from primary incident (negative = before)
    pub lag_seconds: i64,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl RcaAgentInput {
    /// Create new RCA input from incident and correlated signals
    pub fn new(
        primary_incident: RcaIncidentSignal,
        correlated_signals: Vec<RcaCorrelatedSignal>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: source.into(),
            primary_incident,
            correlated_signals,
            service_dependencies: None,
            hints: HashMap::new(),
        }
    }

    /// Add service dependency graph
    pub fn with_dependencies(mut self, dependencies: serde_json::Value) -> Self {
        self.service_dependencies = Some(dependencies);
        self
    }

    /// Add analysis hint
    pub fn with_hint(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.hints.insert(key.into(), value);
        self
    }

    /// Compute SHA-256 hash of inputs for DecisionEvent
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let json = serde_json::to_string(&self).unwrap_or_default();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Root cause hypothesis produced by analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseHypothesis {
    /// Hypothesis ID
    pub hypothesis_id: Uuid,
    /// Rank (1 = most likely)
    pub rank: u32,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Root cause category
    pub category: RootCauseCategory,
    /// Suspected root cause service
    pub root_cause_service: String,
    /// Suspected root cause component (if known)
    pub root_cause_component: Option<String>,
    /// Root cause description
    pub description: String,
    /// Causal chain from root cause to incident
    pub causal_chain: Vec<CausalChainLink>,
    /// Supporting evidence
    pub evidence: Vec<RcaEvidence>,
    /// Suggested investigation steps
    pub investigation_steps: Vec<String>,
}

/// Category of root cause
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootCauseCategory {
    /// Infrastructure issue (e.g., CPU, memory, network)
    Infrastructure,
    /// Dependency failure (e.g., database, external API)
    DependencyFailure,
    /// Configuration change
    ConfigurationChange,
    /// Deployment/release issue
    Deployment,
    /// Traffic pattern change (e.g., spike, pattern shift)
    TrafficPattern,
    /// Model degradation (e.g., drift, quality regression)
    ModelDegradation,
    /// Data quality issue
    DataQuality,
    /// Resource exhaustion
    ResourceExhaustion,
    /// Unknown/inconclusive
    Unknown,
}

impl std::fmt::Display for RootCauseCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RootCauseCategory::Infrastructure => write!(f, "infrastructure"),
            RootCauseCategory::DependencyFailure => write!(f, "dependency_failure"),
            RootCauseCategory::ConfigurationChange => write!(f, "configuration_change"),
            RootCauseCategory::Deployment => write!(f, "deployment"),
            RootCauseCategory::TrafficPattern => write!(f, "traffic_pattern"),
            RootCauseCategory::ModelDegradation => write!(f, "model_degradation"),
            RootCauseCategory::DataQuality => write!(f, "data_quality"),
            RootCauseCategory::ResourceExhaustion => write!(f, "resource_exhaustion"),
            RootCauseCategory::Unknown => write!(f, "unknown"),
        }
    }
}

/// Link in the causal chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalChainLink {
    /// Position in chain (0 = root cause, higher = closer to incident)
    pub position: u32,
    /// Service name
    pub service: String,
    /// Component/metric affected
    pub component: String,
    /// Description of impact
    pub impact: String,
    /// Time of impact
    pub timestamp: DateTime<Utc>,
    /// Confidence in this link
    pub link_confidence: f64,
}

/// Evidence supporting a root cause hypothesis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaEvidence {
    /// Evidence type
    pub evidence_type: EvidenceType,
    /// Evidence description
    pub description: String,
    /// Strength of evidence (0.0 - 1.0)
    pub strength: f64,
    /// Source signal ID (if applicable)
    pub source_signal_id: Option<Uuid>,
    /// Raw data reference
    pub data: Option<serde_json::Value>,
}

/// Type of evidence
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    /// Temporal correlation (signal preceded incident)
    TemporalCorrelation,
    /// Metric correlation (metrics moved together)
    MetricCorrelation,
    /// Service dependency (upstream service affected)
    ServiceDependency,
    /// Historical pattern (similar past incidents)
    HistoricalPattern,
    /// Deployment correlation (recent deployment)
    DeploymentCorrelation,
    /// Configuration change (recent config update)
    ConfigurationChange,
    /// Resource metric (CPU, memory, etc.)
    ResourceMetric,
}

/// Output for Root Cause Analysis Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaAgentOutput {
    /// Whether a root cause was identified
    pub root_cause_identified: bool,
    /// Analysis status
    pub status: RcaAnalysisStatus,
    /// Ranked list of root cause hypotheses
    pub hypotheses: Vec<RootCauseHypothesis>,
    /// Primary hypothesis (if identified)
    pub primary_hypothesis: Option<RootCauseHypothesis>,
    /// Signals that were analyzed
    pub signals_analyzed: usize,
    /// Analysis methods used
    pub methods_applied: Vec<String>,
    /// Summary of analysis
    pub summary: String,
    /// Processing metadata
    pub metadata: RcaOutputMetadata,
}

/// RCA analysis status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RcaAnalysisStatus {
    /// Root cause identified with high confidence
    RootCauseIdentified,
    /// Multiple potential root causes identified
    MultipleCandidates,
    /// Analysis inconclusive
    Inconclusive,
    /// Insufficient data for analysis
    InsufficientData,
    /// Analysis failed
    AnalysisFailed,
}

impl std::fmt::Display for RcaAnalysisStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RcaAnalysisStatus::RootCauseIdentified => write!(f, "root_cause_identified"),
            RcaAnalysisStatus::MultipleCandidates => write!(f, "multiple_candidates"),
            RcaAnalysisStatus::Inconclusive => write!(f, "inconclusive"),
            RcaAnalysisStatus::InsufficientData => write!(f, "insufficient_data"),
            RcaAnalysisStatus::AnalysisFailed => write!(f, "analysis_failed"),
        }
    }
}

/// Metadata for RCA output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaOutputMetadata {
    /// Processing duration in milliseconds
    pub processing_ms: u64,
    /// Number of correlation analyses performed
    pub correlations_computed: usize,
    /// Number of dependency paths analyzed
    pub dependency_paths_analyzed: usize,
    /// Time window analyzed (seconds)
    pub time_window_seconds: u64,
    /// Telemetry emitted
    pub telemetry_emitted: bool,
}

/// Failure modes for the RCA Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RcaFailureMode {
    /// Insufficient correlated signals for analysis
    InsufficientSignals {
        required: usize,
        provided: usize,
    },
    /// Temporal gap between signals too large
    TemporalGapTooLarge {
        max_allowed_seconds: u64,
        actual_seconds: u64,
    },
    /// No causal chain could be established
    NoCausalChainFound {
        reason: String,
    },
    /// Input validation failed
    InputValidationFailed {
        errors: Vec<String>,
    },
    /// ruvector-service persistence failed
    PersistenceFailed {
        error: String,
    },
    /// Analysis timed out
    AnalysisTimeout {
        timeout_ms: u64,
        progress: String,
    },
    /// Configuration error
    ConfigurationError {
        error: String,
    },
}

impl std::fmt::Display for RcaFailureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RcaFailureMode::InsufficientSignals { required, provided } => {
                write!(
                    f,
                    "Insufficient signals: required {}, provided {}",
                    required, provided
                )
            }
            RcaFailureMode::TemporalGapTooLarge {
                max_allowed_seconds,
                actual_seconds,
            } => {
                write!(
                    f,
                    "Temporal gap too large: max {}s, actual {}s",
                    max_allowed_seconds, actual_seconds
                )
            }
            RcaFailureMode::NoCausalChainFound { reason } => {
                write!(f, "No causal chain found: {}", reason)
            }
            RcaFailureMode::InputValidationFailed { errors } => {
                write!(f, "Input validation failed: {}", errors.join(", "))
            }
            RcaFailureMode::PersistenceFailed { error } => {
                write!(f, "Persistence failed: {}", error)
            }
            RcaFailureMode::AnalysisTimeout { timeout_ms, progress } => {
                write!(f, "Analysis timeout after {}ms: {}", timeout_ms, progress)
            }
            RcaFailureMode::ConfigurationError { error } => {
                write!(f, "Configuration error: {}", error)
            }
        }
    }
}

impl CliContract {
    /// Generate CLI contract for Root Cause Analysis Agent
    pub fn rca_agent() -> Self {
        Self {
            agent_name: "rca".to_string(),
            commands: vec![
                CliCommand {
                    name: "inspect".to_string(),
                    description: "Inspect RCA agent state and recent analyses".to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        CliArg {
                            name: "--service".to_string(),
                            arg_type: "string".to_string(),
                            description: "Filter by service name".to_string(),
                        },
                        CliArg {
                            name: "--incident-id".to_string(),
                            arg_type: "uuid".to_string(),
                            description: "Show analysis for specific incident".to_string(),
                        },
                        CliArg {
                            name: "--json".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Output as JSON".to_string(),
                        },
                    ],
                    example: "sentinel agent rca inspect --service chat-api --json".to_string(),
                },
                CliCommand {
                    name: "replay".to_string(),
                    description: "Replay an incident through RCA analysis".to_string(),
                    required_args: vec![CliArg {
                        name: "--incident-id".to_string(),
                        arg_type: "uuid".to_string(),
                        description: "Incident ID to analyze".to_string(),
                    }],
                    optional_args: vec![
                        CliArg {
                            name: "--dry-run".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Don't persist DecisionEvent".to_string(),
                        },
                        CliArg {
                            name: "--time-window".to_string(),
                            arg_type: "string".to_string(),
                            description: "Override analysis time window (e.g., '30m', '1h')"
                                .to_string(),
                        },
                        CliArg {
                            name: "--verbose".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Show detailed analysis steps".to_string(),
                        },
                    ],
                    example: "sentinel agent rca replay --incident-id abc123 --dry-run".to_string(),
                },
                CliCommand {
                    name: "diagnose".to_string(),
                    description: "Run diagnostics on RCA agent health and configuration"
                        .to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        CliArg {
                            name: "--check".to_string(),
                            arg_type: "string".to_string(),
                            description:
                                "Specific check (dependencies, correlations, ruvector, all)"
                                    .to_string(),
                        },
                        CliArg {
                            name: "--validate-graph".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Validate service dependency graph".to_string(),
                        },
                    ],
                    example: "sentinel agent rca diagnose --check all".to_string(),
                },
            ],
        }
    }

    /// Generate CLI contract for Incident Correlation Agent
    pub fn incident_correlation_agent() -> Self {
        Self {
            agent_name: "incident-correlation".to_string(),
            commands: vec![
                CliCommand {
                    name: "inspect".to_string(),
                    description: "Inspect correlation agent configuration and state".to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        CliArg {
                            name: "--service".to_string(),
                            arg_type: "string".to_string(),
                            description: "Filter by service name".to_string(),
                        },
                        CliArg {
                            name: "--time-window".to_string(),
                            arg_type: "duration".to_string(),
                            description: "Time window to inspect (e.g., 1h, 30m)".to_string(),
                        },
                        CliArg {
                            name: "--json".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Output as JSON".to_string(),
                        },
                    ],
                    example: "sentinel agent incident-correlation inspect --service chat-api --json"
                        .to_string(),
                },
                CliCommand {
                    name: "replay".to_string(),
                    description: "Replay a batch of signals through correlation".to_string(),
                    required_args: vec![CliArg {
                        name: "--input-file".to_string(),
                        arg_type: "path".to_string(),
                        description: "JSON file containing signals to correlate".to_string(),
                    }],
                    optional_args: vec![
                        CliArg {
                            name: "--dry-run".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Don't persist DecisionEvent".to_string(),
                        },
                        CliArg {
                            name: "--time-window".to_string(),
                            arg_type: "seconds".to_string(),
                            description: "Override correlation time window".to_string(),
                        },
                        CliArg {
                            name: "--verbose".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Show detailed correlation steps".to_string(),
                        },
                    ],
                    example:
                        "sentinel agent incident-correlation replay --input-file signals.json --dry-run"
                            .to_string(),
                },
                CliCommand {
                    name: "diagnose".to_string(),
                    description: "Run diagnostics on correlation agent health".to_string(),
                    required_args: vec![],
                    optional_args: vec![CliArg {
                        name: "--check".to_string(),
                        arg_type: "string".to_string(),
                        description: "Specific check (config, affinity, ruvector, all)".to_string(),
                    }],
                    example: "sentinel agent incident-correlation diagnose --check all".to_string(),
                },
            ],
        }
    }
}

// =============================================================================
// INCIDENT CORRELATION AGENT CONTRACT
// =============================================================================
//
// # Agent Contract: Incident Correlation Agent
//
// ## Purpose Statement (precise, non-marketing)
// Correlate multiple anomaly events and alerts from upstream detection agents
// into coherent incident candidates. Operates on batches of signals using
// configurable time-window grouping, service affinity, and signal type
// correlation algorithms. Emits incident correlation events with grouped
// signals and correlation quality scores.
//
// ## Classification
// - Type: CORRELATION / ANALYSIS
//
// ## Decision Type Semantics
// - decision_type: "incident_correlation"
// - Emitted when: batch of signals processed for correlation
//   (decision recorded whether incidents correlated or not)
//
// ## Confidence Semantics
// - Correlation quality score (0.0 - 1.0)
// - Combines: temporal clustering, service affinity, signal type compatibility,
//   severity consistency
// - High confidence = tight time clustering + same service + related signals
// - Low confidence = scattered signals or weak correlation
//
// ## Constraints Applied Semantics
// - Temporal rules (e.g., "time_window:300s", "min_window_overlap:0.5")
// - Threshold rules (e.g., "min_signals:2", "correlation_threshold:0.6")
// - Analysis rules (e.g., "strategy:hierarchical", "affinity:high")
//
// ## What This Agent MUST NEVER Do
// - Perform remediation
// - Trigger retries
// - Modify routing
// - Modify policies
// - Modify thresholds dynamically
// - Call external notification systems directly
// - Invoke other agents directly
// - Create or close actual incidents (only candidates)
// - Store incident state beyond the current invocation
//
// ## Primary Consumers
// - LLM-Incident-Manager (consumes incident candidates)
// - Post-incident analysis dashboards
// - Governance reporting
// - LLM-Observatory incident visualization
//
// ## Failure Modes
// - NoSignalsProvided
// - InputValidationFailed
// - StrategyFailed
// - PersistenceFailed
// - TimeWindowError
// - ConfigurationError
// - SignalOverload
// =============================================================================

/// Signal type for correlation classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    /// Anomaly detection signal
    Anomaly,
    /// Alert signal
    Alert,
    /// Drift detection signal
    Drift,
    /// Custom signal type
    Custom,
}

impl std::fmt::Display for SignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalType::Anomaly => write!(f, "anomaly"),
            SignalType::Alert => write!(f, "alert"),
            SignalType::Drift => write!(f, "drift"),
            SignalType::Custom => write!(f, "custom"),
        }
    }
}

/// A signal that can be correlated (anomaly event, alert, or drift)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationSignal {
    /// Unique signal identifier
    pub signal_id: Uuid,
    /// Signal timestamp
    pub timestamp: DateTime<Utc>,
    /// Signal type (anomaly, alert, drift)
    pub signal_type: SignalType,
    /// Service that generated the signal
    pub service_name: String,
    /// Model identifier
    pub model: String,
    /// Severity level
    pub severity: String,
    /// Confidence score of the original detection
    pub confidence: f64,
    /// Anomaly type if applicable (e.g., "latency_spike", "error_rate_increase")
    pub anomaly_type: Option<String>,
    /// Detection method used (e.g., "zscore", "iqr")
    pub detection_method: Option<String>,
    /// Metric name if applicable
    pub metric: Option<String>,
    /// Observed value if applicable
    pub observed_value: Option<f64>,
    /// Baseline value if applicable
    pub baseline_value: Option<f64>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CorrelationSignal {
    /// Create a new correlation signal from an anomaly event
    pub fn from_anomaly(
        signal_id: Uuid,
        timestamp: DateTime<Utc>,
        service_name: impl Into<String>,
        model: impl Into<String>,
        severity: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            signal_id,
            timestamp,
            signal_type: SignalType::Anomaly,
            service_name: service_name.into(),
            model: model.into(),
            severity: severity.into(),
            confidence,
            anomaly_type: None,
            detection_method: None,
            metric: None,
            observed_value: None,
            baseline_value: None,
            metadata: HashMap::new(),
        }
    }

    /// Add anomaly details
    pub fn with_anomaly_details(
        mut self,
        anomaly_type: impl Into<String>,
        detection_method: impl Into<String>,
    ) -> Self {
        self.anomaly_type = Some(anomaly_type.into());
        self.detection_method = Some(detection_method.into());
        self
    }

    /// Add metric details
    pub fn with_metric(
        mut self,
        metric: impl Into<String>,
        observed: f64,
        baseline: f64,
    ) -> Self {
        self.metric = Some(metric.into());
        self.observed_value = Some(observed);
        self.baseline_value = Some(baseline);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Input for Incident Correlation Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentCorrelationInput {
    /// Unique request identifier
    pub request_id: Uuid,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
    /// Source system (e.g., "sentinel.detection.anomaly", "sentinel.alerting")
    pub source: String,
    /// Batch of signals to correlate
    pub signals: Vec<CorrelationSignal>,
    /// Correlation hints (e.g., force_correlation, time_window_override)
    pub hints: HashMap<String, serde_json::Value>,
}

impl IncidentCorrelationInput {
    /// Create new correlation input with signals
    pub fn new(signals: Vec<CorrelationSignal>, source: impl Into<String>) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: source.into(),
            signals,
            hints: HashMap::new(),
        }
    }

    /// Add correlation hint
    pub fn with_hint(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.hints.insert(key.into(), value);
        self
    }

    /// Compute SHA-256 hash of inputs for DecisionEvent
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        let json = serde_json::to_string(&self).unwrap_or_default();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Time window for an incident
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationTimeWindow {
    /// Start of the time window
    pub start: DateTime<Utc>,
    /// End of the time window
    pub end: DateTime<Utc>,
    /// Duration in seconds
    pub duration_secs: u64,
}

impl CorrelationTimeWindow {
    /// Create a new time window
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        let duration_secs = (end - start).num_seconds().max(0) as u64;
        Self {
            start,
            end,
            duration_secs,
        }
    }
}

/// Breakdown of signal types in an incident
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignalBreakdown {
    /// Count of anomaly signals
    pub anomaly_count: usize,
    /// Count of alert signals
    pub alert_count: usize,
    /// Count of drift signals
    pub drift_count: usize,
    /// Count of custom signals
    pub custom_count: usize,
    /// Breakdown by anomaly type
    pub anomaly_types: HashMap<String, usize>,
    /// Breakdown by detection method
    pub detection_methods: HashMap<String, usize>,
}

/// An incident candidate (group of correlated signals)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentCandidate {
    /// Unique incident candidate identifier
    pub incident_id: Uuid,
    /// Incident title (auto-generated)
    pub title: String,
    /// Incident description
    pub description: String,
    /// Grouped signal IDs
    pub signal_ids: Vec<Uuid>,
    /// Primary service affected (service with most signals)
    pub primary_service: String,
    /// All services affected
    pub affected_services: Vec<String>,
    /// All models affected
    pub affected_models: Vec<String>,
    /// Aggregated severity (highest among signals)
    pub severity: String,
    /// Correlation confidence score
    pub correlation_confidence: f64,
    /// Correlation reasoning
    pub correlation_reasoning: String,
    /// Time window of the incident
    pub time_window: CorrelationTimeWindow,
    /// Signal type breakdown
    pub signal_breakdown: SignalBreakdown,
    /// Recommended actions
    pub recommended_actions: Vec<String>,
}

/// Metadata for correlation output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationOutputMetadata {
    /// Processing duration in milliseconds
    pub processing_ms: u64,
    /// Total signals processed
    pub signals_processed: usize,
    /// Signals correlated into incidents
    pub signals_correlated: usize,
    /// Signals that could not be correlated (orphans)
    pub orphan_signal_count: usize,
    /// Noise reduction ratio (1 - incidents/signals)
    pub noise_reduction_ratio: f64,
    /// Telemetry emitted
    pub telemetry_emitted: bool,
}

/// Output for Incident Correlation Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentCorrelationOutput {
    /// Whether any incidents were correlated
    pub incidents_detected: bool,
    /// Number of incident candidates identified
    pub incident_count: usize,
    /// List of correlated incident candidates
    pub incidents: Vec<IncidentCandidate>,
    /// Signal IDs that could not be correlated (orphans)
    pub orphan_signals: Vec<Uuid>,
    /// Correlation strategies evaluated
    pub strategies_evaluated: Vec<String>,
    /// Strategy that produced the best correlation
    pub primary_strategy: Option<String>,
    /// Processing metadata
    pub metadata: CorrelationOutputMetadata,
}

/// Service group for correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceGroup {
    /// Group identifier
    pub group_id: String,
    /// Group name
    pub name: String,
    /// Services in this group
    pub services: Vec<String>,
    /// Correlation weight multiplier for this group
    pub weight_multiplier: f64,
}

/// Failure modes for the Incident Correlation Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorrelationFailureMode {
    /// No signals provided for correlation
    NoSignalsProvided,
    /// Input validation failed
    InputValidationFailed { errors: Vec<String> },
    /// Correlation strategy failed
    StrategyFailed { strategy: String, error: String },
    /// ruvector-service persistence failed
    PersistenceFailed { error: String },
    /// Time window computation error
    TimeWindowError { error: String },
    /// Configuration error
    ConfigurationError { error: String },
    /// Too many signals to process
    SignalOverload { count: usize, max: usize },
}

impl std::fmt::Display for CorrelationFailureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorrelationFailureMode::NoSignalsProvided => {
                write!(f, "No signals provided for correlation")
            }
            CorrelationFailureMode::InputValidationFailed { errors } => {
                write!(f, "Input validation failed: {}", errors.join(", "))
            }
            CorrelationFailureMode::StrategyFailed { strategy, error } => {
                write!(f, "Correlation strategy '{}' failed: {}", strategy, error)
            }
            CorrelationFailureMode::PersistenceFailed { error } => {
                write!(f, "Persistence failed: {}", error)
            }
            CorrelationFailureMode::TimeWindowError { error } => {
                write!(f, "Time window error: {}", error)
            }
            CorrelationFailureMode::ConfigurationError { error } => {
                write!(f, "Configuration error: {}", error)
            }
            CorrelationFailureMode::SignalOverload { count, max } => {
                write!(
                    f,
                    "Signal overload: {} signals provided (max: {})",
                    count, max
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_creation() {
        let id = AgentId::anomaly_detection();
        assert_eq!(id.as_str(), "sentinel.detection.anomaly");
    }

    #[test]
    fn test_agent_version() {
        let version = AgentVersion::current();
        assert_eq!(version.to_string(), "1.0.0");
    }

    #[test]
    fn test_decision_type_display() {
        assert_eq!(
            DecisionType::AnomalyDetection.to_string(),
            "anomaly_detection"
        );
    }

    #[test]
    fn test_decision_event_validation() {
        let event = DecisionEvent::new(
            AgentId::anomaly_detection(),
            AgentVersion::current(),
            DecisionType::AnomalyDetection,
            "abc123".to_string(),
            AgentOutput {
                anomaly_detected: false,
                anomaly_event: None,
                detectors_evaluated: vec!["zscore".to_string()],
                triggered_by: None,
                metadata: OutputMetadata {
                    processing_ms: 10,
                    baselines_checked: 3,
                    baselines_valid: 2,
                    telemetry_emitted: true,
                },
            },
            0.5,
            vec![ConstraintApplied {
                constraint_id: "zscore_threshold".to_string(),
                constraint_type: ConstraintType::Threshold,
                value: serde_json::json!(3.0),
                passed: true,
                description: Some("Z-score threshold check".to_string()),
            }],
            ExecutionRef {
                request_id: Uuid::new_v4(),
                trace_id: None,
                span_id: None,
                source: "test".to_string(),
            },
            DecisionContext {
                service_name: "test-service".to_string(),
                model: "gpt-4".to_string(),
                region: None,
                environment: None,
            },
        );

        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_decision_event_validation_fails() {
        let event = DecisionEvent::new(
            AgentId::anomaly_detection(),
            AgentVersion::current(),
            DecisionType::AnomalyDetection,
            "abc123".to_string(),
            AgentOutput {
                anomaly_detected: false,
                anomaly_event: None,
                detectors_evaluated: vec![],
                triggered_by: None,
                metadata: OutputMetadata {
                    processing_ms: 10,
                    baselines_checked: 0,
                    baselines_valid: 0,
                    telemetry_emitted: false,
                },
            },
            0.5,
            vec![], // No constraints - should fail
            ExecutionRef {
                request_id: Uuid::new_v4(),
                trace_id: None,
                span_id: None,
                source: "test".to_string(),
            },
            DecisionContext {
                service_name: "".to_string(), // Empty - should fail
                model: "gpt-4".to_string(),
                region: None,
                environment: None,
            },
        );

        let result = event.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("constraint")));
        assert!(errors.iter().any(|e| e.contains("service_name")));
    }

    #[test]
    fn test_cli_contract() {
        let contract = CliContract::anomaly_detection_agent();
        assert_eq!(contract.agent_name, "anomaly-detection");
        assert_eq!(contract.commands.len(), 3);

        let inspect = contract.commands.iter().find(|c| c.name == "inspect");
        assert!(inspect.is_some());

        let replay = contract.commands.iter().find(|c| c.name == "replay");
        assert!(replay.is_some());
        assert_eq!(replay.unwrap().required_args.len(), 1);

        let diagnose = contract.commands.iter().find(|c| c.name == "diagnose");
        assert!(diagnose.is_some());
    }

    #[test]
    fn test_input_hash() {
        let input = AgentInput {
            request_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            timestamp: Utc::now(),
            source: "test".to_string(),
            telemetry: serde_json::json!({"latency_ms": 100}),
            hints: HashMap::new(),
        };

        let hash = input.compute_hash();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 hex
    }

    // =========================================================================
    // Alerting Agent Tests
    // =========================================================================

    #[test]
    fn test_alerting_agent_id() {
        let id = AgentId::alerting();
        assert_eq!(id.as_str(), "sentinel.alerting.evaluation");
    }

    #[test]
    fn test_alerting_cli_contract() {
        let contract = CliContract::alerting_agent();
        assert_eq!(contract.agent_name, "alerting");
        assert_eq!(contract.commands.len(), 3);

        let inspect = contract.commands.iter().find(|c| c.name == "inspect");
        assert!(inspect.is_some());

        let replay = contract.commands.iter().find(|c| c.name == "replay");
        assert!(replay.is_some());
        assert_eq!(replay.unwrap().required_args.len(), 1);
        assert_eq!(
            replay.unwrap().required_args[0].name,
            "--anomaly-id"
        );

        let diagnose = contract.commands.iter().find(|c| c.name == "diagnose");
        assert!(diagnose.is_some());
    }

    #[test]
    fn test_alerting_agent_input() {
        let anomaly = serde_json::json!({
            "alert_id": "550e8400-e29b-41d4-a716-446655440000",
            "severity": "high",
            "anomaly_type": "latency_spike"
        });

        let input = AlertingAgentInput::from_anomaly(anomaly, "sentinel.detection.anomaly")
            .with_hint("ignore_suppression", serde_json::json!(true));

        assert_eq!(input.source, "sentinel.detection.anomaly");
        assert!(input.hints.contains_key("ignore_suppression"));

        let hash = input.compute_hash();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_alert_evaluation_status_display() {
        assert_eq!(AlertEvaluationStatus::Raised.to_string(), "raised");
        assert_eq!(
            AlertEvaluationStatus::Deduplicated.to_string(),
            "deduplicated"
        );
        assert_eq!(
            AlertEvaluationStatus::MaintenanceSuppressed.to_string(),
            "maintenance_suppressed"
        );
        assert_eq!(AlertEvaluationStatus::RateLimited.to_string(), "rate_limited");
        assert_eq!(
            AlertEvaluationStatus::BelowThreshold.to_string(),
            "below_threshold"
        );
    }

    #[test]
    fn test_alerting_rule_serialization() {
        let rule = AlertingRule {
            rule_id: "rule-001".to_string(),
            name: "High Severity Alerts".to_string(),
            description: Some("Alert on high severity anomalies".to_string()),
            enabled: true,
            severity_threshold: Some("high".to_string()),
            anomaly_types: vec!["latency_spike".to_string(), "error_rate_increase".to_string()],
            services: vec![],
            models: vec![],
            confidence_threshold: 0.8,
            suppression: Some(SuppressionConfig {
                deduplication_enabled: true,
                deduplication_window_secs: 300,
                maintenance_windows: vec![],
                max_alerts_per_hour: 100,
            }),
            priority: 10,
        };

        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: AlertingRule = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.rule_id, "rule-001");
        assert_eq!(deserialized.confidence_threshold, 0.8);
        assert!(deserialized.suppression.is_some());
    }

    #[test]
    fn test_alerting_failure_mode_display() {
        let failure = AlertingFailureMode::RateLimitExceeded {
            limit: 100,
            window_secs: 3600,
            current_count: 150,
        };
        assert!(failure.to_string().contains("150/100"));
        assert!(failure.to_string().contains("3600s"));

        let failure = AlertingFailureMode::AlertingRuleNotFound {
            rule_id: "rule-123".to_string(),
        };
        assert!(failure.to_string().contains("rule-123"));
    }

    #[test]
    fn test_alerting_agent_output() {
        let output = AlertingAgentOutput {
            alert_raised: true,
            status: AlertEvaluationStatus::Raised,
            alert_event: Some(serde_json::json!({
                "alert_id": "test-alert",
                "severity": "high"
            })),
            rules_evaluated: vec!["rule-001".to_string(), "rule-002".to_string()],
            triggered_by: Some("rule-001".to_string()),
            reasoning: "High severity latency spike detected".to_string(),
            metadata: AlertingOutputMetadata {
                processing_ms: 5,
                suppression_rules_checked: 2,
                threshold_rules_checked: 3,
                deduplication_active: true,
                telemetry_emitted: true,
            },
        };

        assert!(output.alert_raised);
        assert_eq!(output.rules_evaluated.len(), 2);
        assert_eq!(output.metadata.processing_ms, 5);
    }

    // =========================================================================
    // Root Cause Analysis Agent Tests
    // =========================================================================

    #[test]
    fn test_rca_agent_id() {
        let id = AgentId::root_cause_analysis();
        assert_eq!(id.as_str(), "sentinel.analysis.rca");
    }

    #[test]
    fn test_rca_decision_type_display() {
        assert_eq!(
            DecisionType::RootCauseAnalysis.to_string(),
            "root_cause_analysis"
        );
    }

    #[test]
    fn test_rca_cli_contract() {
        let contract = CliContract::rca_agent();
        assert_eq!(contract.agent_name, "rca");
        assert_eq!(contract.commands.len(), 3);

        let inspect = contract.commands.iter().find(|c| c.name == "inspect");
        assert!(inspect.is_some());

        let replay = contract.commands.iter().find(|c| c.name == "replay");
        assert!(replay.is_some());
        assert_eq!(replay.unwrap().required_args.len(), 1);
        assert_eq!(replay.unwrap().required_args[0].name, "--incident-id");

        let diagnose = contract.commands.iter().find(|c| c.name == "diagnose");
        assert!(diagnose.is_some());
    }

    #[test]
    fn test_rca_agent_input() {
        let primary_incident = RcaIncidentSignal {
            incident_id: Uuid::new_v4(),
            service_name: "chat-api".to_string(),
            model: Some("gpt-4".to_string()),
            timestamp: Utc::now(),
            severity: "high".to_string(),
            incident_type: "latency_spike".to_string(),
            metric: "latency_ms".to_string(),
            observed_value: 5000.0,
            baseline_value: 150.0,
            context: HashMap::new(),
        };

        let correlated_signal = RcaCorrelatedSignal {
            signal_id: Uuid::new_v4(),
            service_name: "database".to_string(),
            timestamp: Utc::now(),
            signal_type: "resource_metric".to_string(),
            metric: "cpu_percent".to_string(),
            value: 95.0,
            correlation: 0.85,
            lag_seconds: -30,
            metadata: HashMap::new(),
        };

        let input = RcaAgentInput::new(
            primary_incident,
            vec![correlated_signal],
            "incident-manager",
        )
        .with_hint("suspected_service", serde_json::json!("database"));

        assert_eq!(input.source, "incident-manager");
        assert_eq!(input.correlated_signals.len(), 1);
        assert!(input.hints.contains_key("suspected_service"));

        let hash = input.compute_hash();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_root_cause_hypothesis() {
        let hypothesis = RootCauseHypothesis {
            hypothesis_id: Uuid::new_v4(),
            rank: 1,
            confidence: 0.85,
            category: RootCauseCategory::DependencyFailure,
            root_cause_service: "database".to_string(),
            root_cause_component: Some("connection_pool".to_string()),
            description: "Database connection pool exhaustion".to_string(),
            causal_chain: vec![
                CausalChainLink {
                    position: 0,
                    service: "database".to_string(),
                    component: "connection_pool".to_string(),
                    impact: "Pool exhausted, new connections blocked".to_string(),
                    timestamp: Utc::now(),
                    link_confidence: 0.9,
                },
                CausalChainLink {
                    position: 1,
                    service: "chat-api".to_string(),
                    component: "db_client".to_string(),
                    impact: "Requests waiting for DB connections".to_string(),
                    timestamp: Utc::now(),
                    link_confidence: 0.85,
                },
            ],
            evidence: vec![RcaEvidence {
                evidence_type: EvidenceType::TemporalCorrelation,
                description: "CPU spike preceded latency increase by 30s".to_string(),
                strength: 0.8,
                source_signal_id: None,
                data: None,
            }],
            investigation_steps: vec![
                "Check database connection pool metrics".to_string(),
                "Review recent query patterns".to_string(),
            ],
        };

        assert_eq!(hypothesis.rank, 1);
        assert_eq!(hypothesis.confidence, 0.85);
        assert_eq!(hypothesis.causal_chain.len(), 2);
        assert_eq!(hypothesis.evidence.len(), 1);
    }

    #[test]
    fn test_root_cause_category_display() {
        assert_eq!(
            RootCauseCategory::Infrastructure.to_string(),
            "infrastructure"
        );
        assert_eq!(
            RootCauseCategory::DependencyFailure.to_string(),
            "dependency_failure"
        );
        assert_eq!(
            RootCauseCategory::ModelDegradation.to_string(),
            "model_degradation"
        );
        assert_eq!(RootCauseCategory::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_rca_analysis_status_display() {
        assert_eq!(
            RcaAnalysisStatus::RootCauseIdentified.to_string(),
            "root_cause_identified"
        );
        assert_eq!(
            RcaAnalysisStatus::MultipleCandidates.to_string(),
            "multiple_candidates"
        );
        assert_eq!(
            RcaAnalysisStatus::Inconclusive.to_string(),
            "inconclusive"
        );
        assert_eq!(
            RcaAnalysisStatus::InsufficientData.to_string(),
            "insufficient_data"
        );
    }

    #[test]
    fn test_rca_agent_output() {
        let output = RcaAgentOutput {
            root_cause_identified: true,
            status: RcaAnalysisStatus::RootCauseIdentified,
            hypotheses: vec![],
            primary_hypothesis: None,
            signals_analyzed: 5,
            methods_applied: vec![
                "temporal_correlation".to_string(),
                "service_dependency".to_string(),
            ],
            summary: "Database connection pool exhaustion identified as root cause".to_string(),
            metadata: RcaOutputMetadata {
                processing_ms: 150,
                correlations_computed: 10,
                dependency_paths_analyzed: 3,
                time_window_seconds: 1800,
                telemetry_emitted: true,
            },
        };

        assert!(output.root_cause_identified);
        assert_eq!(output.signals_analyzed, 5);
        assert_eq!(output.methods_applied.len(), 2);
        assert_eq!(output.metadata.processing_ms, 150);
    }

    #[test]
    fn test_rca_failure_mode_display() {
        let failure = RcaFailureMode::InsufficientSignals {
            required: 3,
            provided: 1,
        };
        assert!(failure.to_string().contains("required 3"));
        assert!(failure.to_string().contains("provided 1"));

        let failure = RcaFailureMode::TemporalGapTooLarge {
            max_allowed_seconds: 300,
            actual_seconds: 600,
        };
        assert!(failure.to_string().contains("max 300s"));
        assert!(failure.to_string().contains("actual 600s"));

        let failure = RcaFailureMode::NoCausalChainFound {
            reason: "No dependency path found".to_string(),
        };
        assert!(failure.to_string().contains("No dependency path found"));

        let failure = RcaFailureMode::AnalysisTimeout {
            timeout_ms: 30000,
            progress: "50% complete".to_string(),
        };
        assert!(failure.to_string().contains("30000ms"));
        assert!(failure.to_string().contains("50% complete"));
    }

    #[test]
    fn test_rca_input_serialization() {
        let primary_incident = RcaIncidentSignal {
            incident_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            service_name: "chat-api".to_string(),
            model: Some("gpt-4".to_string()),
            timestamp: Utc::now(),
            severity: "high".to_string(),
            incident_type: "latency_spike".to_string(),
            metric: "latency_ms".to_string(),
            observed_value: 5000.0,
            baseline_value: 150.0,
            context: HashMap::new(),
        };

        let input = RcaAgentInput::new(primary_incident, vec![], "test");

        let json = serde_json::to_string(&input).unwrap();
        let deserialized: RcaAgentInput = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.source, "test");
        assert_eq!(deserialized.primary_incident.service_name, "chat-api");
    }

    // =========================================================================
    // Incident Correlation Agent Tests
    // =========================================================================

    #[test]
    fn test_incident_correlation_agent_id() {
        let id = AgentId::incident_correlation();
        assert_eq!(id.as_str(), "sentinel.correlation.incident");
    }

    #[test]
    fn test_incident_correlation_decision_type_display() {
        assert_eq!(
            DecisionType::IncidentCorrelation.to_string(),
            "incident_correlation"
        );
    }

    #[test]
    fn test_incident_correlation_cli_contract() {
        let contract = CliContract::incident_correlation_agent();
        assert_eq!(contract.agent_name, "incident-correlation");
        assert_eq!(contract.commands.len(), 3);

        let inspect = contract.commands.iter().find(|c| c.name == "inspect");
        assert!(inspect.is_some());

        let replay = contract.commands.iter().find(|c| c.name == "replay");
        assert!(replay.is_some());
        assert_eq!(replay.unwrap().required_args.len(), 1);
        assert_eq!(replay.unwrap().required_args[0].name, "--input-file");

        let diagnose = contract.commands.iter().find(|c| c.name == "diagnose");
        assert!(diagnose.is_some());
    }

    #[test]
    fn test_signal_type_display() {
        assert_eq!(SignalType::Anomaly.to_string(), "anomaly");
        assert_eq!(SignalType::Alert.to_string(), "alert");
        assert_eq!(SignalType::Drift.to_string(), "drift");
        assert_eq!(SignalType::Custom.to_string(), "custom");
    }

    #[test]
    fn test_correlation_signal_creation() {
        let signal = CorrelationSignal::from_anomaly(
            Uuid::new_v4(),
            Utc::now(),
            "chat-api",
            "gpt-4",
            "high",
            0.85,
        )
        .with_anomaly_details("latency_spike", "zscore")
        .with_metric("latency_ms", 5000.0, 150.0)
        .with_metadata("region", serde_json::json!("us-east-1"));

        assert_eq!(signal.service_name, "chat-api");
        assert_eq!(signal.model, "gpt-4");
        assert_eq!(signal.severity, "high");
        assert_eq!(signal.confidence, 0.85);
        assert_eq!(signal.signal_type, SignalType::Anomaly);
        assert_eq!(signal.anomaly_type, Some("latency_spike".to_string()));
        assert_eq!(signal.detection_method, Some("zscore".to_string()));
        assert_eq!(signal.metric, Some("latency_ms".to_string()));
        assert_eq!(signal.observed_value, Some(5000.0));
        assert_eq!(signal.baseline_value, Some(150.0));
        assert!(signal.metadata.contains_key("region"));
    }

    #[test]
    fn test_incident_correlation_input() {
        let signals = vec![
            CorrelationSignal::from_anomaly(
                Uuid::new_v4(),
                Utc::now(),
                "chat-api",
                "gpt-4",
                "high",
                0.9,
            ),
            CorrelationSignal::from_anomaly(
                Uuid::new_v4(),
                Utc::now(),
                "chat-api",
                "gpt-4",
                "medium",
                0.75,
            ),
        ];

        let input = IncidentCorrelationInput::new(signals, "sentinel.detection.anomaly")
            .with_hint("force_correlation", serde_json::json!(true));

        assert_eq!(input.source, "sentinel.detection.anomaly");
        assert_eq!(input.signals.len(), 2);
        assert!(input.hints.contains_key("force_correlation"));

        let hash = input.compute_hash();
        assert_eq!(hash.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_correlation_time_window() {
        let start = Utc::now();
        let end = start + chrono::Duration::seconds(300);
        let window = CorrelationTimeWindow::new(start, end);

        assert_eq!(window.duration_secs, 300);
        assert_eq!(window.start, start);
        assert_eq!(window.end, end);
    }

    #[test]
    fn test_correlation_failure_mode_display() {
        let failure = CorrelationFailureMode::NoSignalsProvided;
        assert_eq!(failure.to_string(), "No signals provided for correlation");

        let failure = CorrelationFailureMode::InputValidationFailed {
            errors: vec!["error1".to_string(), "error2".to_string()],
        };
        assert!(failure.to_string().contains("error1"));
        assert!(failure.to_string().contains("error2"));

        let failure = CorrelationFailureMode::SignalOverload { count: 1500, max: 1000 };
        assert!(failure.to_string().contains("1500"));
        assert!(failure.to_string().contains("1000"));

        let failure = CorrelationFailureMode::StrategyFailed {
            strategy: "hierarchical".to_string(),
            error: "timeout".to_string(),
        };
        assert!(failure.to_string().contains("hierarchical"));
        assert!(failure.to_string().contains("timeout"));
    }

    #[test]
    fn test_signal_breakdown_default() {
        let breakdown = SignalBreakdown::default();
        assert_eq!(breakdown.anomaly_count, 0);
        assert_eq!(breakdown.alert_count, 0);
        assert_eq!(breakdown.drift_count, 0);
        assert!(breakdown.anomaly_types.is_empty());
    }

    #[test]
    fn test_service_group_serialization() {
        let group = ServiceGroup {
            group_id: "api-services".to_string(),
            name: "API Services".to_string(),
            services: vec!["chat-api".to_string(), "search-api".to_string()],
            weight_multiplier: 1.2,
        };

        let json = serde_json::to_string(&group).unwrap();
        let deserialized: ServiceGroup = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.group_id, "api-services");
        assert_eq!(deserialized.services.len(), 2);
        assert_eq!(deserialized.weight_multiplier, 1.2);
    }
}
