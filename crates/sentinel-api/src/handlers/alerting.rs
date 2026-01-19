//! Alerting Agent API handlers
//!
//! This module provides HTTP endpoints for the Alerting Agent:
//! - POST /api/v1/agents/alerting/evaluate - Evaluate an anomaly for alerting
//! - GET /api/v1/agents/alerting/rules - List alerting rules
//! - GET /api/v1/agents/alerting/stats - Get alerting statistics
//!
//! ## Constitution Compliance
//! - All endpoints are read-only or emit alert events only
//! - No remediation actions
//! - No direct notifications (that's LLM-Incident-Manager's responsibility)
//! - DecisionEvents are persisted to ruvector-service

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

use crate::{ErrorResponse, SuccessResponse};
use llm_sentinel_detection::agents::{
    AlertingAgent, AlertingAgentInput, AlertingRule, DecisionEvent,
};

// =============================================================================
// STATE
// =============================================================================

/// Shared state for alerting handlers
#[derive(Debug)]
pub struct AlertingState {
    /// Alerting agent instance
    pub agent: Arc<AlertingAgent>,
}

impl AlertingState {
    /// Create new alerting state
    pub fn new(agent: AlertingAgent) -> Self {
        Self {
            agent: Arc::new(agent),
        }
    }
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

/// Request to evaluate an anomaly for alerting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateRequest {
    /// Anomaly event to evaluate (as JSON)
    pub anomaly: serde_json::Value,
    /// Source of the request
    #[serde(default = "default_source")]
    pub source: String,
    /// If true, skip suppression rules
    #[serde(default)]
    pub ignore_suppression: bool,
    /// If true, don't persist the decision
    #[serde(default)]
    pub dry_run: bool,
}

fn default_source() -> String {
    "api".to_string()
}

/// Response from alert evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateResponse {
    /// The decision event
    pub decision: DecisionEvent,
    /// Whether an alert was raised
    pub alert_raised: bool,
    /// Status message
    pub status: String,
}

/// Query parameters for rules listing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RulesQuery {
    /// Filter by service
    pub service: Option<String>,
    /// Filter by severity threshold
    pub severity: Option<String>,
    /// Only show enabled rules
    #[serde(default)]
    pub enabled_only: bool,
}

/// Rules listing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesResponse {
    /// List of rules
    pub rules: Vec<AlertingRule>,
    /// Total count
    pub total: usize,
}

/// Alerting statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingStats {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// Total evaluations (approximate)
    pub total_evaluations: u64,
    /// Alerts raised (approximate)
    pub alerts_raised: u64,
    /// Alerts suppressed (approximate)
    pub alerts_suppressed: u64,
    /// Current rate limit status
    pub rate_limit_status: RateLimitStatus,
    /// Configuration summary
    pub config_summary: ConfigSummary,
}

/// Rate limit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    /// Maximum alerts per hour
    pub max_per_hour: u32,
    /// Current count (approximate)
    pub current_count: u32,
    /// Whether rate limiting is active
    pub is_limited: bool,
}

/// Configuration summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummary {
    /// Default severity threshold
    pub default_severity_threshold: String,
    /// Default confidence threshold
    pub default_confidence_threshold: f64,
    /// Deduplication enabled
    pub deduplication_enabled: bool,
    /// Deduplication window (seconds)
    pub deduplication_window_secs: u64,
    /// Number of rules
    pub rule_count: usize,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// POST /api/v1/agents/alerting/evaluate
///
/// Evaluate an anomaly event against alerting rules.
/// Returns a DecisionEvent indicating whether an alert should be raised.
///
/// ## Constitution Compliance
/// - This endpoint does NOT send notifications
/// - It only evaluates and returns a decision
/// - LLM-Incident-Manager consumes these decisions
#[instrument(skip(state, request), fields(source = %request.source))]
pub async fn evaluate_alert(
    State(state): State<Arc<AlertingState>>,
    Json(request): Json<EvaluateRequest>,
) -> impl IntoResponse {
    info!("Evaluating anomaly for alerting");

    // Build input
    let mut input = AlertingAgentInput::from_anomaly(request.anomaly, &request.source);

    if request.ignore_suppression {
        input = input.with_hint("ignore_suppression", serde_json::json!(true));
    }

    if request.dry_run {
        input = input.with_hint("dry_run", serde_json::json!(true));
    }

    // Process through agent
    match state.agent.process(input).await {
        Ok(decision) => {
            let alert_raised = decision.outputs.anomaly_detected;
            let status = if alert_raised {
                "alert_raised"
            } else {
                "no_alert"
            };

            info!(
                decision_id = %decision.decision_id,
                alert_raised = alert_raised,
                "Alert evaluation complete"
            );

            let response = EvaluateResponse {
                decision,
                alert_raised,
                status: status.to_string(),
            };

            (StatusCode::OK, Json(SuccessResponse::new(response)))
        }
        Err(e) => {
            error!(error = %e, "Alert evaluation failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SuccessResponse::new(EvaluateResponse {
                    decision: create_error_decision(&e.to_string()),
                    alert_raised: false,
                    status: "error".to_string(),
                })),
            )
        }
    }
}

/// GET /api/v1/agents/alerting/rules
///
/// List configured alerting rules.
#[instrument(skip(state))]
pub async fn list_rules(
    State(state): State<Arc<AlertingState>>,
    Query(query): Query<RulesQuery>,
) -> impl IntoResponse {
    info!(?query, "Listing alerting rules");

    // For now, return a placeholder response
    // In production, this would read from the agent's configuration
    let rules = vec![AlertingRule::default_rule()];

    let filtered: Vec<_> = rules
        .into_iter()
        .filter(|r| {
            if query.enabled_only && !r.enabled {
                return false;
            }
            if let Some(ref service) = query.service {
                if !r.services.is_empty() && !r.services.contains(service) {
                    return false;
                }
            }
            true
        })
        .collect();

    let total = filtered.len();

    (
        StatusCode::OK,
        Json(SuccessResponse::new(RulesResponse {
            rules: filtered,
            total,
        })),
    )
}

/// GET /api/v1/agents/alerting/stats
///
/// Get alerting agent statistics.
#[instrument(skip(state))]
pub async fn alerting_stats(State(state): State<Arc<AlertingState>>) -> impl IntoResponse {
    info!("Getting alerting statistics");

    // In production, these would come from metrics/ruvector-service
    let stats = AlertingStats {
        agent_id: AlertingAgent::agent_id().to_string(),
        agent_version: AlertingAgent::version().to_string(),
        total_evaluations: 0,
        alerts_raised: 0,
        alerts_suppressed: 0,
        rate_limit_status: RateLimitStatus {
            max_per_hour: 100,
            current_count: 0,
            is_limited: false,
        },
        config_summary: ConfigSummary {
            default_severity_threshold: "medium".to_string(),
            default_confidence_threshold: 0.7,
            deduplication_enabled: true,
            deduplication_window_secs: 300,
            rule_count: 1,
        },
    };

    (StatusCode::OK, Json(SuccessResponse::new(stats)))
}

// =============================================================================
// HELPERS
// =============================================================================

/// Create an error decision event for API error responses
fn create_error_decision(error_msg: &str) -> DecisionEvent {
    use llm_sentinel_detection::agents::{
        AgentId, AgentOutput, AgentVersion, ConstraintApplied, ConstraintType, DecisionContext,
        DecisionType, ExecutionRef, OutputMetadata,
    };
    use uuid::Uuid;

    DecisionEvent::new(
        AgentId::alerting(),
        AgentVersion::new(1, 0, 0),
        DecisionType::AlertEvaluation,
        "error".to_string(),
        AgentOutput {
            anomaly_detected: false,
            anomaly_event: None,
            detectors_evaluated: vec![],
            triggered_by: None,
            metadata: OutputMetadata {
                processing_ms: 0,
                baselines_checked: 0,
                baselines_valid: 0,
                telemetry_emitted: false,
            },
        },
        0.0,
        vec![ConstraintApplied {
            constraint_id: "api_error".to_string(),
            constraint_type: ConstraintType::Rule,
            value: serde_json::json!({ "error": error_msg }),
            passed: false,
            description: Some("API error occurred".to_string()),
        }],
        ExecutionRef {
            request_id: Uuid::new_v4(),
            trace_id: None,
            span_id: None,
            source: "api_error".to_string(),
        },
        DecisionContext {
            service_name: "unknown".to_string(),
            model: "unknown".to_string(),
            region: None,
            environment: None,
        },
    )
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::events::{AnomalyContext, AnomalyDetails, AnomalyEvent};
    use llm_sentinel_core::types::{AnomalyType, DetectionMethod, ModelId, ServiceId, Severity};
    use std::collections::HashMap;

    fn create_test_anomaly() -> serde_json::Value {
        let anomaly = AnomalyEvent::new(
            Severity::High,
            AnomalyType::LatencySpike,
            ServiceId::new("test-service"),
            ModelId::new("gpt-4"),
            DetectionMethod::ZScore,
            0.95,
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
        );
        serde_json::to_value(anomaly).unwrap()
    }

    #[test]
    fn test_evaluate_request_deserialization() {
        let json = r#"{
            "anomaly": {"alert_id": "test"},
            "source": "test",
            "ignore_suppression": true,
            "dry_run": true
        }"#;

        let request: EvaluateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.source, "test");
        assert!(request.ignore_suppression);
        assert!(request.dry_run);
    }

    #[test]
    fn test_evaluate_request_defaults() {
        let json = r#"{
            "anomaly": {"alert_id": "test"}
        }"#;

        let request: EvaluateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.source, "api");
        assert!(!request.ignore_suppression);
        assert!(!request.dry_run);
    }

    #[test]
    fn test_alerting_state_creation() {
        let agent = AlertingAgent::with_defaults();
        let state = AlertingState::new(agent);
        assert!(Arc::strong_count(&state.agent) == 1);
    }

    #[test]
    fn test_error_decision_creation() {
        let decision = create_error_decision("test error");
        assert!(!decision.outputs.anomaly_detected);
        assert_eq!(decision.confidence, 0.0);
        assert!(decision
            .constraints_applied
            .iter()
            .any(|c| c.constraint_id == "api_error"));
    }
}
