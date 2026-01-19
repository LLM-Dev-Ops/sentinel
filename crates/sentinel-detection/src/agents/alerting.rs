//! Alerting Agent Implementation for LLM-Sentinel
//!
//! # Agent Contract: Alerting Agent
//!
//! ## Purpose Statement (precise, non-marketing)
//! Evaluate detected anomalies and drift events against alerting rules to
//! determine whether alerts should be raised. Applies threshold evaluation,
//! deduplication, and suppression rules to control alert volume.
//!
//! ## Classification
//! - Type: ALERTING
//!
//! ## Decision Type Semantics
//! - decision_type: "alert_evaluation"
//! - Emitted when: anomaly/drift event evaluated against alerting rules
//!
//! ## This Agent MUST NEVER:
//! - Send notifications directly (email, pager, slack)
//! - Perform remediation
//! - Trigger retries
//! - Modify routing configuration
//! - Modify policies or thresholds dynamically
//! - Invoke other agents directly
//! - Execute incident workflows
//!
//! ## Primary Consumers
//! - LLM-Incident-Manager (consumes alert events)
//! - Governance dashboards (audit trail)
//! - LLM-Observatory (telemetry visualization)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use llm_sentinel_core::events::{AlertEvent, AnomalyEvent};
use llm_sentinel_core::types::Severity;
use llm_sentinel_core::Result;

use super::contract::{
    AgentId, AgentVersion, AlertEvaluationStatus, AlertingAgentInput, AlertingAgentOutput,
    AlertingOutputMetadata, AlertingRule, ConstraintApplied, ConstraintType, DecisionContext,
    DecisionEvent, DecisionType, ExecutionRef, SuppressionConfig,
};

// =============================================================================
// ALERTING AGENT CONFIGURATION
// =============================================================================

/// Configuration for the Alerting Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingAgentConfig {
    /// Default severity threshold (minimum severity to alert)
    pub default_severity_threshold: Severity,
    /// Default confidence threshold
    pub default_confidence_threshold: f64,
    /// Enable deduplication by default
    pub deduplication_enabled: bool,
    /// Default deduplication window in seconds
    pub deduplication_window_secs: u64,
    /// Maximum alerts per hour (0 = unlimited)
    pub max_alerts_per_hour: u32,
    /// Alerting rules
    pub rules: Vec<AlertingRule>,
}

impl Default for AlertingAgentConfig {
    fn default() -> Self {
        Self {
            default_severity_threshold: Severity::Medium,
            default_confidence_threshold: 0.7,
            deduplication_enabled: true,
            deduplication_window_secs: 300,
            max_alerts_per_hour: 100,
            rules: vec![AlertingRule::default_rule()],
        }
    }
}

impl AlertingRule {
    /// Create default alerting rule
    pub fn default_rule() -> Self {
        Self {
            rule_id: "default".to_string(),
            name: "Default Alerting Rule".to_string(),
            description: Some("Default rule that alerts on medium+ severity anomalies".to_string()),
            enabled: true,
            severity_threshold: Some("medium".to_string()),
            anomaly_types: vec![],
            services: vec![],
            models: vec![],
            confidence_threshold: 0.7,
            suppression: Some(SuppressionConfig {
                deduplication_enabled: true,
                deduplication_window_secs: 300,
                maintenance_windows: vec![],
                max_alerts_per_hour: 100,
            }),
            priority: 0,
        }
    }
}

// =============================================================================
// RUVECTOR SERVICE CLIENT TRAIT
// =============================================================================

/// Client trait for ruvector-service persistence
///
/// All persistence operations MUST go through this trait.
/// LLM-Sentinel NEVER connects directly to databases.
#[async_trait]
pub trait RuvectorClient: Send + Sync {
    /// Persist a DecisionEvent
    async fn persist_decision(&self, event: &DecisionEvent) -> Result<()>;

    /// Check if an alert is deduplicated (within dedup window)
    async fn check_deduplication(
        &self,
        service: &str,
        model: &str,
        anomaly_type: &str,
        window_secs: u64,
    ) -> Result<bool>;

    /// Get alert count for rate limiting
    async fn get_alert_count(&self, service: &str, window_secs: u64) -> Result<u32>;

    /// Check maintenance window status
    async fn is_in_maintenance(&self, service: &str) -> Result<bool>;
}

/// Noop implementation for testing/dry-run
#[derive(Debug, Clone, Default)]
pub struct NoopRuvectorClient;

#[async_trait]
impl RuvectorClient for NoopRuvectorClient {
    async fn persist_decision(&self, _event: &DecisionEvent) -> Result<()> {
        Ok(())
    }

    async fn check_deduplication(
        &self,
        _service: &str,
        _model: &str,
        _anomaly_type: &str,
        _window_secs: u64,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn get_alert_count(&self, _service: &str, _window_secs: u64) -> Result<u32> {
        Ok(0)
    }

    async fn is_in_maintenance(&self, _service: &str) -> Result<bool> {
        Ok(false)
    }
}

// =============================================================================
// ALERTING AGENT IMPLEMENTATION
// =============================================================================

/// Alerting Agent
///
/// Evaluates anomalies against alerting rules and emits alert events.
///
/// ## Guarantees
/// - Stateless execution (no state between invocations)
/// - Exactly ONE DecisionEvent per invocation
/// - Deterministic behavior
/// - No remediation, retry, or orchestration logic
pub struct AlertingAgent {
    /// Agent configuration
    config: AlertingAgentConfig,
    /// ruvector-service client
    ruvector: Arc<dyn RuvectorClient>,
}

impl std::fmt::Debug for AlertingAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlertingAgent")
            .field("config", &self.config)
            .field("ruvector", &"<RuvectorClient>")
            .finish()
    }
}

impl AlertingAgent {
    /// Create a new Alerting Agent
    pub fn new(config: AlertingAgentConfig, ruvector: Arc<dyn RuvectorClient>) -> Self {
        Self { config, ruvector }
    }

    /// Create with default configuration and noop persistence
    pub fn with_defaults() -> Self {
        Self {
            config: AlertingAgentConfig::default(),
            ruvector: Arc::new(NoopRuvectorClient),
        }
    }

    /// Get agent ID
    pub fn agent_id() -> AgentId {
        AgentId::alerting()
    }

    /// Get current version
    pub fn version() -> AgentVersion {
        AgentVersion::new(1, 0, 0)
    }

    /// Process an alerting request
    ///
    /// This is the main entry point for the Alerting Agent.
    /// Returns exactly ONE DecisionEvent as per the constitution.
    #[instrument(skip(self), fields(agent_id = %Self::agent_id(), request_id = %input.request_id))]
    pub async fn process(&self, input: AlertingAgentInput) -> Result<DecisionEvent> {
        let start = Instant::now();
        let inputs_hash = input.compute_hash();

        info!(
            request_id = %input.request_id,
            source = %input.source,
            "Processing alerting request"
        );

        // Parse anomaly from input
        let anomaly: AnomalyEvent = match serde_json::from_value(input.anomaly.clone()) {
            Ok(a) => a,
            Err(e) => {
                error!(error = %e, "Failed to parse anomaly event");
                return self.create_failure_decision(
                    &input,
                    &inputs_hash,
                    start,
                    format!("Invalid anomaly event: {}", e),
                );
            }
        };

        // Check hints for special processing
        let ignore_suppression = input
            .hints
            .get("ignore_suppression")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let dry_run = input
            .hints
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Evaluate against rules
        let evaluation_result = self
            .evaluate_anomaly(&anomaly, ignore_suppression)
            .await;

        let (status, alert_event, constraints, triggered_by, reasoning) = match evaluation_result {
            Ok(result) => result,
            Err(e) => {
                error!(error = %e, "Evaluation failed");
                return self.create_failure_decision(
                    &input,
                    &inputs_hash,
                    start,
                    format!("Evaluation failed: {}", e),
                );
            }
        };

        // Calculate confidence
        let confidence = self.calculate_confidence(&anomaly, &status, &constraints);

        // Create output
        let output = AlertingAgentOutput {
            alert_raised: status == AlertEvaluationStatus::Raised,
            status,
            alert_event: alert_event.as_ref().map(|a| serde_json::to_value(a).unwrap()),
            rules_evaluated: constraints
                .iter()
                .filter(|c| c.constraint_type == ConstraintType::Rule)
                .map(|c| c.constraint_id.clone())
                .collect(),
            triggered_by,
            reasoning: reasoning.clone(),
            metadata: AlertingOutputMetadata {
                processing_ms: start.elapsed().as_millis() as u64,
                suppression_rules_checked: constraints
                    .iter()
                    .filter(|c| c.constraint_type == ConstraintType::Rule)
                    .count(),
                threshold_rules_checked: constraints
                    .iter()
                    .filter(|c| c.constraint_type == ConstraintType::Threshold)
                    .count(),
                deduplication_active: self.config.deduplication_enabled,
                telemetry_emitted: true,
            },
        };

        // Create decision event
        let decision = self.create_decision_event(
            &input,
            &anomaly,
            inputs_hash,
            output,
            confidence,
            constraints,
        );

        // Emit telemetry
        self.emit_telemetry(&decision, start.elapsed().as_millis() as u64);

        // Persist decision (unless dry-run)
        if !dry_run {
            if let Err(e) = self.ruvector.persist_decision(&decision).await {
                error!(error = %e, "Failed to persist decision event");
                // Note: We still return the decision even if persistence fails
                // The caller should handle this appropriately
            }
        }

        info!(
            decision_id = %decision.decision_id,
            status = %status,
            anomaly_detected = decision.outputs.anomaly_detected,
            confidence = decision.confidence,
            processing_ms = start.elapsed().as_millis(),
            "Alerting decision complete"
        );

        Ok(decision)
    }

    /// Evaluate an anomaly against alerting rules
    async fn evaluate_anomaly(
        &self,
        anomaly: &AnomalyEvent,
        ignore_suppression: bool,
    ) -> Result<(
        AlertEvaluationStatus,
        Option<AlertEvent>,
        Vec<ConstraintApplied>,
        Option<String>,
        String,
    )> {
        let mut constraints = Vec::new();
        let service = anomaly.service_name.as_str();
        let model = anomaly.model.as_str();
        let anomaly_type = anomaly.anomaly_type.to_string();

        // Check maintenance window first (if not ignoring suppression)
        if !ignore_suppression {
            let in_maintenance = self.ruvector.is_in_maintenance(service).await?;
            constraints.push(ConstraintApplied {
                constraint_id: "maintenance_window".to_string(),
                constraint_type: ConstraintType::Rule,
                value: serde_json::json!(in_maintenance),
                passed: !in_maintenance,
                description: Some("Maintenance window check".to_string()),
            });

            if in_maintenance {
                return Ok((
                    AlertEvaluationStatus::MaintenanceSuppressed,
                    None,
                    constraints,
                    Some("maintenance_window".to_string()),
                    "Alert suppressed due to active maintenance window".to_string(),
                ));
            }
        }

        // Check deduplication (if not ignoring suppression)
        if !ignore_suppression && self.config.deduplication_enabled {
            let is_duplicate = self
                .ruvector
                .check_deduplication(
                    service,
                    model,
                    &anomaly_type,
                    self.config.deduplication_window_secs,
                )
                .await?;

            constraints.push(ConstraintApplied {
                constraint_id: format!("dedup_window_{}s", self.config.deduplication_window_secs),
                constraint_type: ConstraintType::Rule,
                value: serde_json::json!({
                    "window_secs": self.config.deduplication_window_secs,
                    "is_duplicate": is_duplicate
                }),
                passed: !is_duplicate,
                description: Some("Deduplication window check".to_string()),
            });

            if is_duplicate {
                return Ok((
                    AlertEvaluationStatus::Deduplicated,
                    None,
                    constraints,
                    Some(format!(
                        "dedup_window_{}s",
                        self.config.deduplication_window_secs
                    )),
                    format!(
                        "Alert deduplicated within {}s window",
                        self.config.deduplication_window_secs
                    ),
                ));
            }
        }

        // Check rate limiting (if not ignoring suppression)
        if !ignore_suppression && self.config.max_alerts_per_hour > 0 {
            let alert_count = self.ruvector.get_alert_count(service, 3600).await?;

            constraints.push(ConstraintApplied {
                constraint_id: format!("rate_limit_{}/hr", self.config.max_alerts_per_hour),
                constraint_type: ConstraintType::Rule,
                value: serde_json::json!({
                    "limit": self.config.max_alerts_per_hour,
                    "current": alert_count
                }),
                passed: alert_count < self.config.max_alerts_per_hour,
                description: Some("Rate limit check".to_string()),
            });

            if alert_count >= self.config.max_alerts_per_hour {
                return Ok((
                    AlertEvaluationStatus::RateLimited,
                    None,
                    constraints,
                    Some(format!("rate_limit_{}/hr", self.config.max_alerts_per_hour)),
                    format!(
                        "Alert rate limited: {}/{} per hour",
                        alert_count, self.config.max_alerts_per_hour
                    ),
                ));
            }
        }

        // Check severity threshold
        let passes_severity =
            anomaly.severity >= self.config.default_severity_threshold;

        constraints.push(ConstraintApplied {
            constraint_id: format!(
                "severity_threshold:{}",
                self.config.default_severity_threshold
            ),
            constraint_type: ConstraintType::Threshold,
            value: serde_json::json!({
                "threshold": self.config.default_severity_threshold.to_string(),
                "actual": anomaly.severity.to_string()
            }),
            passed: passes_severity,
            description: Some("Minimum severity threshold".to_string()),
        });

        if !passes_severity {
            return Ok((
                AlertEvaluationStatus::BelowThreshold,
                None,
                constraints,
                Some(format!(
                    "severity_threshold:{}",
                    self.config.default_severity_threshold
                )),
                format!(
                    "Alert below severity threshold: {} < {}",
                    anomaly.severity, self.config.default_severity_threshold
                ),
            ));
        }

        // Check confidence threshold
        let passes_confidence =
            anomaly.confidence >= self.config.default_confidence_threshold;

        constraints.push(ConstraintApplied {
            constraint_id: format!(
                "confidence_threshold:{}",
                self.config.default_confidence_threshold
            ),
            constraint_type: ConstraintType::Threshold,
            value: serde_json::json!({
                "threshold": self.config.default_confidence_threshold,
                "actual": anomaly.confidence
            }),
            passed: passes_confidence,
            description: Some("Minimum confidence threshold".to_string()),
        });

        if !passes_confidence {
            return Ok((
                AlertEvaluationStatus::BelowThreshold,
                None,
                constraints,
                Some(format!(
                    "confidence_threshold:{}",
                    self.config.default_confidence_threshold
                )),
                format!(
                    "Alert below confidence threshold: {:.2} < {:.2}",
                    anomaly.confidence, self.config.default_confidence_threshold
                ),
            ));
        }

        // Evaluate custom rules (sorted by priority)
        let mut rules: Vec<_> = self.config.rules.iter().filter(|r| r.enabled).collect();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        for rule in rules {
            let matches = self.rule_matches(rule, anomaly);

            constraints.push(ConstraintApplied {
                constraint_id: format!("rule:{}", rule.rule_id),
                constraint_type: ConstraintType::Rule,
                value: serde_json::json!({
                    "rule_id": rule.rule_id,
                    "matches": matches
                }),
                passed: matches,
                description: rule.description.clone(),
            });

            if matches {
                debug!(
                    rule_id = %rule.rule_id,
                    "Rule matched, raising alert"
                );
            }
        }

        // All checks passed - raise alert
        let alert_event = AlertEvent::from_anomaly(anomaly.clone());

        Ok((
            AlertEvaluationStatus::Raised,
            Some(alert_event),
            constraints,
            None,
            format!(
                "Alert raised for {} anomaly in {}/{}",
                anomaly.anomaly_type, service, model
            ),
        ))
    }

    /// Check if a rule matches the anomaly
    fn rule_matches(&self, rule: &AlertingRule, anomaly: &AnomalyEvent) -> bool {
        // Check anomaly types filter
        if !rule.anomaly_types.is_empty() {
            let anomaly_type_str = anomaly.anomaly_type.to_string();
            if !rule.anomaly_types.iter().any(|t| t == &anomaly_type_str) {
                return false;
            }
        }

        // Check services filter
        if !rule.services.is_empty() {
            if !rule.services.iter().any(|s| s == anomaly.service_name.as_str()) {
                return false;
            }
        }

        // Check models filter
        if !rule.models.is_empty() {
            if !rule.models.iter().any(|m| m == anomaly.model.as_str()) {
                return false;
            }
        }

        // Check confidence threshold
        if anomaly.confidence < rule.confidence_threshold {
            return false;
        }

        // Check severity threshold
        if let Some(ref threshold_str) = rule.severity_threshold {
            let threshold = parse_severity(threshold_str);
            if anomaly.severity < threshold {
                return false;
            }
        }

        true
    }

    /// Calculate decision confidence
    fn calculate_confidence(
        &self,
        anomaly: &AnomalyEvent,
        status: &AlertEvaluationStatus,
        constraints: &[ConstraintApplied],
    ) -> f64 {
        match status {
            AlertEvaluationStatus::Raised => {
                // High confidence for raised alerts
                // Combine anomaly confidence with constraint pass rate
                let pass_rate = constraints.iter().filter(|c| c.passed).count() as f64
                    / constraints.len().max(1) as f64;
                (anomaly.confidence + pass_rate) / 2.0
            }
            AlertEvaluationStatus::Deduplicated
            | AlertEvaluationStatus::MaintenanceSuppressed
            | AlertEvaluationStatus::RateLimited => {
                // High confidence for suppression decisions (clear rule match)
                0.95
            }
            AlertEvaluationStatus::BelowThreshold | AlertEvaluationStatus::RuleSuppressed => {
                // Medium confidence for threshold decisions
                0.8
            }
            AlertEvaluationStatus::EvaluationFailed => 0.0,
        }
    }

    /// Create a DecisionEvent
    fn create_decision_event(
        &self,
        input: &AlertingAgentInput,
        anomaly: &AnomalyEvent,
        inputs_hash: String,
        output: AlertingAgentOutput,
        confidence: f64,
        constraints: Vec<ConstraintApplied>,
    ) -> DecisionEvent {
        // Convert AlertingAgentOutput to generic AgentOutput format
        let agent_output = super::contract::AgentOutput {
            anomaly_detected: output.alert_raised,
            anomaly_event: output.alert_event.clone(),
            detectors_evaluated: output.rules_evaluated.clone(),
            triggered_by: output.triggered_by.clone(),
            metadata: super::contract::OutputMetadata {
                processing_ms: output.metadata.processing_ms,
                baselines_checked: 0,
                baselines_valid: 0,
                telemetry_emitted: output.metadata.telemetry_emitted,
            },
        };

        DecisionEvent::new(
            Self::agent_id(),
            Self::version(),
            DecisionType::AlertEvaluation,
            inputs_hash,
            agent_output,
            confidence,
            constraints,
            ExecutionRef {
                request_id: input.request_id,
                trace_id: anomaly.context.trace_id.clone(),
                span_id: None,
                source: input.source.clone(),
            },
            DecisionContext {
                service_name: anomaly.service_name.to_string(),
                model: anomaly.model.to_string(),
                region: anomaly.context.region.clone(),
                environment: None,
            },
        )
    }

    /// Create a failure decision event
    fn create_failure_decision(
        &self,
        input: &AlertingAgentInput,
        inputs_hash: &str,
        start: Instant,
        error_msg: String,
    ) -> Result<DecisionEvent> {
        let output = super::contract::AgentOutput {
            anomaly_detected: false,
            anomaly_event: None,
            detectors_evaluated: vec![],
            triggered_by: None,
            metadata: super::contract::OutputMetadata {
                processing_ms: start.elapsed().as_millis() as u64,
                baselines_checked: 0,
                baselines_valid: 0,
                telemetry_emitted: false,
            },
        };

        let constraints = vec![ConstraintApplied {
            constraint_id: "evaluation_error".to_string(),
            constraint_type: ConstraintType::Rule,
            value: serde_json::json!({ "error": error_msg }),
            passed: false,
            description: Some("Evaluation failed".to_string()),
        }];

        Ok(DecisionEvent::new(
            Self::agent_id(),
            Self::version(),
            DecisionType::AlertEvaluation,
            inputs_hash.to_string(),
            output,
            0.0,
            constraints,
            ExecutionRef {
                request_id: input.request_id,
                trace_id: None,
                span_id: None,
                source: input.source.clone(),
            },
            DecisionContext {
                service_name: "unknown".to_string(),
                model: "unknown".to_string(),
                region: None,
                environment: None,
            },
        ))
    }

    /// Emit telemetry for the decision
    fn emit_telemetry(&self, decision: &DecisionEvent, processing_ms: u64) {
        // Note: In production, this would emit metrics via the metrics crate
        // For now, we use structured logging which is compatible with LLM-Observatory

        let status = if decision.outputs.anomaly_detected {
            "raised"
        } else {
            "suppressed"
        };

        info!(
            target: "sentinel.alerting.telemetry",
            agent_id = %decision.agent_id,
            agent_version = %decision.agent_version,
            decision_id = %decision.decision_id,
            decision_type = %decision.decision_type,
            status = status,
            confidence = decision.confidence,
            service = %decision.context.service_name,
            model = %decision.context.model,
            processing_ms = processing_ms,
            constraints_count = decision.constraints_applied.len(),
            "Alerting agent decision telemetry"
        );

        // Emit metrics (commented out - would use metrics crate in production)
        // metrics::counter!("sentinel_alerting_decisions_total", "status" => status).increment(1);
        // metrics::histogram!("sentinel_alerting_processing_duration_ms").record(processing_ms as f64);
    }
}

/// Parse severity string to Severity enum
fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "low" => Severity::Low,
        "medium" => Severity::Medium,
        "high" => Severity::High,
        "critical" => Severity::Critical,
        _ => Severity::Medium,
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::events::{AnomalyContext, AnomalyDetails};
    use llm_sentinel_core::types::{AnomalyType, DetectionMethod, ModelId, ServiceId};

    fn create_test_anomaly(severity: Severity, confidence: f64) -> AnomalyEvent {
        AnomalyEvent::new(
            severity,
            AnomalyType::LatencySpike,
            ServiceId::new("test-service"),
            ModelId::new("gpt-4"),
            DetectionMethod::ZScore,
            confidence,
            AnomalyDetails {
                metric: "latency_ms".to_string(),
                value: 5000.0,
                baseline: 150.0,
                threshold: 500.0,
                deviation_sigma: Some(5.2),
                additional: HashMap::new(),
            },
            AnomalyContext {
                trace_id: Some("trace-123".to_string()),
                user_id: None,
                region: Some("us-east-1".to_string()),
                time_window: "last_5_minutes".to_string(),
                sample_count: 1000,
                additional: HashMap::new(),
            },
        )
    }

    #[tokio::test]
    async fn test_alerting_agent_creation() {
        let agent = AlertingAgent::with_defaults();
        assert_eq!(AlertingAgent::agent_id().as_str(), "sentinel.alerting.evaluation");
        assert_eq!(AlertingAgent::version().to_string(), "1.0.0");
    }

    #[tokio::test]
    async fn test_alert_raised_for_high_severity() {
        let agent = AlertingAgent::with_defaults();
        let anomaly = create_test_anomaly(Severity::High, 0.95);
        let input = AlertingAgentInput::from_anomaly(
            serde_json::to_value(&anomaly).unwrap(),
            "test",
        );

        let decision = agent.process(input).await.unwrap();

        assert!(decision.outputs.anomaly_detected);
        assert_eq!(decision.decision_type, DecisionType::AlertEvaluation);
        assert!(decision.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_alert_suppressed_for_low_severity() {
        let mut config = AlertingAgentConfig::default();
        config.default_severity_threshold = Severity::High;

        let agent = AlertingAgent::new(config, Arc::new(NoopRuvectorClient));
        let anomaly = create_test_anomaly(Severity::Low, 0.95);
        let input = AlertingAgentInput::from_anomaly(
            serde_json::to_value(&anomaly).unwrap(),
            "test",
        );

        let decision = agent.process(input).await.unwrap();

        assert!(!decision.outputs.anomaly_detected);
        assert!(decision
            .constraints_applied
            .iter()
            .any(|c| c.constraint_id.contains("severity_threshold")));
    }

    #[tokio::test]
    async fn test_alert_suppressed_for_low_confidence() {
        let mut config = AlertingAgentConfig::default();
        config.default_confidence_threshold = 0.9;

        let agent = AlertingAgent::new(config, Arc::new(NoopRuvectorClient));
        let anomaly = create_test_anomaly(Severity::High, 0.5);
        let input = AlertingAgentInput::from_anomaly(
            serde_json::to_value(&anomaly).unwrap(),
            "test",
        );

        let decision = agent.process(input).await.unwrap();

        assert!(!decision.outputs.anomaly_detected);
        assert!(decision
            .constraints_applied
            .iter()
            .any(|c| c.constraint_id.contains("confidence_threshold")));
    }

    #[tokio::test]
    async fn test_decision_event_structure() {
        let agent = AlertingAgent::with_defaults();
        let anomaly = create_test_anomaly(Severity::Critical, 0.99);
        let input = AlertingAgentInput::from_anomaly(
            serde_json::to_value(&anomaly).unwrap(),
            "sentinel.detection.anomaly",
        );

        let decision = agent.process(input).await.unwrap();

        // Validate required DecisionEvent fields
        assert!(!decision.decision_id.is_nil());
        assert_eq!(decision.agent_id.as_str(), "sentinel.alerting.evaluation");
        assert_eq!(decision.agent_version.to_string(), "1.0.0");
        assert_eq!(decision.decision_type, DecisionType::AlertEvaluation);
        assert!(!decision.inputs_hash.is_empty());
        assert!(!decision.constraints_applied.is_empty());
        assert!(!decision.context.service_name.is_empty());
        assert!(!decision.context.model.is_empty());

        // Validate the decision
        assert!(decision.validate().is_ok());
    }

    #[tokio::test]
    async fn test_dry_run_no_persistence() {
        // Create a mock client that tracks calls
        struct TrackingClient {
            persist_called: std::sync::atomic::AtomicBool,
        }

        #[async_trait]
        impl RuvectorClient for TrackingClient {
            async fn persist_decision(&self, _event: &DecisionEvent) -> Result<()> {
                self.persist_called
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
            async fn check_deduplication(
                &self,
                _: &str,
                _: &str,
                _: &str,
                _: u64,
            ) -> Result<bool> {
                Ok(false)
            }
            async fn get_alert_count(&self, _: &str, _: u64) -> Result<u32> {
                Ok(0)
            }
            async fn is_in_maintenance(&self, _: &str) -> Result<bool> {
                Ok(false)
            }
        }

        let client = Arc::new(TrackingClient {
            persist_called: std::sync::atomic::AtomicBool::new(false),
        });
        let agent = AlertingAgent::new(AlertingAgentConfig::default(), client.clone());

        let anomaly = create_test_anomaly(Severity::High, 0.95);
        let input = AlertingAgentInput::from_anomaly(
            serde_json::to_value(&anomaly).unwrap(),
            "test",
        )
        .with_hint("dry_run", serde_json::json!(true));

        let _ = agent.process(input).await.unwrap();

        assert!(!client
            .persist_called
            .load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_ignore_suppression_hint() {
        struct MaintenanceClient;

        #[async_trait]
        impl RuvectorClient for MaintenanceClient {
            async fn persist_decision(&self, _: &DecisionEvent) -> Result<()> {
                Ok(())
            }
            async fn check_deduplication(&self, _: &str, _: &str, _: &str, _: u64) -> Result<bool> {
                Ok(true) // Would normally suppress
            }
            async fn get_alert_count(&self, _: &str, _: u64) -> Result<u32> {
                Ok(1000) // Would normally rate limit
            }
            async fn is_in_maintenance(&self, _: &str) -> Result<bool> {
                Ok(true) // Would normally suppress
            }
        }

        let agent = AlertingAgent::new(
            AlertingAgentConfig::default(),
            Arc::new(MaintenanceClient),
        );

        let anomaly = create_test_anomaly(Severity::High, 0.95);
        let input = AlertingAgentInput::from_anomaly(
            serde_json::to_value(&anomaly).unwrap(),
            "test",
        )
        .with_hint("ignore_suppression", serde_json::json!(true));

        let decision = agent.process(input).await.unwrap();

        // Should still raise alert despite suppression conditions
        assert!(decision.outputs.anomaly_detected);
    }

    #[test]
    fn test_rule_matching() {
        let agent = AlertingAgent::with_defaults();

        // Rule that matches latency_spike for test-service
        let rule = AlertingRule {
            rule_id: "test-rule".to_string(),
            name: "Test Rule".to_string(),
            description: None,
            enabled: true,
            severity_threshold: Some("medium".to_string()),
            anomaly_types: vec!["latency_spike".to_string()],
            services: vec!["test-service".to_string()],
            models: vec![],
            confidence_threshold: 0.5,
            suppression: None,
            priority: 10,
        };

        let anomaly = create_test_anomaly(Severity::High, 0.8);
        assert!(agent.rule_matches(&rule, &anomaly));

        // Non-matching service
        let non_matching_anomaly = AnomalyEvent::new(
            Severity::High,
            AnomalyType::LatencySpike,
            ServiceId::new("other-service"),
            ModelId::new("gpt-4"),
            DetectionMethod::ZScore,
            0.8,
            AnomalyDetails {
                metric: "latency_ms".to_string(),
                value: 5000.0,
                baseline: 150.0,
                threshold: 500.0,
                deviation_sigma: None,
                additional: HashMap::new(),
            },
            AnomalyContext {
                trace_id: None,
                user_id: None,
                region: None,
                time_window: "last_5_minutes".to_string(),
                sample_count: 100,
                additional: HashMap::new(),
            },
        );
        assert!(!agent.rule_matches(&rule, &non_matching_anomaly));
    }

    #[test]
    fn test_parse_severity() {
        assert_eq!(parse_severity("low"), Severity::Low);
        assert_eq!(parse_severity("MEDIUM"), Severity::Medium);
        assert_eq!(parse_severity("High"), Severity::High);
        assert_eq!(parse_severity("CRITICAL"), Severity::Critical);
        assert_eq!(parse_severity("invalid"), Severity::Medium);
    }
}
