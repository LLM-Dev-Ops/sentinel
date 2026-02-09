//! Incident Correlation Agent API handlers
//!
//! This module provides HTTP endpoints for the Incident Correlation Agent:
//! - POST /api/v1/agents/correlation/correlate - Correlate signals into incidents
//! - GET /api/v1/agents/correlation/config - Get agent configuration
//! - GET /api/v1/agents/correlation/stats - Get agent statistics

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use llm_sentinel_core::execution::{create_agent_span, Evidence};
use crate::execution::ExecutionCollector;
use crate::{InstrumentedResponse, SuccessResponse};

// =============================================================================
// STATE
// =============================================================================

/// Shared state for incident correlation handlers
#[derive(Debug, Clone)]
pub struct IncidentCorrelationState {
    /// Configuration placeholder
    _config: CorrelationConfigResponse,
}

impl IncidentCorrelationState {
    /// Create new incident correlation state with defaults
    pub fn new() -> Self {
        Self {
            _config: CorrelationConfigResponse::default(),
        }
    }
}

impl Default for IncidentCorrelationState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

/// Request to correlate signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelateRequest {
    /// Signals to correlate (anomaly events, alerts)
    pub signals: Vec<serde_json::Value>,
    /// Time window override (seconds)
    #[serde(default)]
    pub time_window_secs: Option<u64>,
    /// If true, don't persist the decision
    #[serde(default)]
    pub dry_run: bool,
}

/// Response from correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelateResponse {
    /// Number of incidents identified
    pub incidents_count: usize,
    /// Number of input signals processed
    pub signals_processed: usize,
    /// Noise reduction ratio
    pub noise_reduction: f64,
    /// Status message
    pub status: String,
}

/// Agent configuration response
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorrelationConfigResponse {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// Time window (seconds)
    pub time_window_secs: u64,
    /// Minimum signals per incident
    pub min_signals_per_incident: usize,
    /// Correlation threshold
    pub correlation_threshold: f64,
}

/// Agent statistics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationStatsResponse {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// Total invocations
    pub total_invocations: u64,
    /// Incidents correlated
    pub incidents_correlated: u64,
    /// Total signals processed
    pub signals_processed: u64,
    /// Average noise reduction ratio
    pub avg_noise_reduction: f64,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// POST /api/v1/agents/correlation/correlate
///
/// Emits an agent-level execution span for the incident correlation agent.
#[instrument(skip(_state, request, collector), fields(signal_count = request.signals.len()))]
pub async fn correlate_signals(
    collector: ExecutionCollector,
    State(_state): State<Arc<IncidentCorrelationState>>,
    Json(request): Json<CorrelateRequest>,
) -> impl IntoResponse {
    info!(
        signal_count = request.signals.len(),
        "Processing correlation request"
    );

    let repo_span_id = collector.0.repo_span_id();
    let mut agent_span = create_agent_span("incident_correlation", repo_span_id);

    // Validate input
    if request.signals.is_empty() {
        agent_span.fail(vec!["No signals provided".to_string()]);
        collector.0.add_agent_span(agent_span);
        let graph = collector.0.finalize_failed(vec![
            "Correlation validation failed: no signals".to_string(),
        ]);

        return (
            StatusCode::BAD_REQUEST,
            Json(InstrumentedResponse {
                data: CorrelateResponse {
                    incidents_count: 0,
                    signals_processed: 0,
                    noise_reduction: 0.0,
                    status: "No signals provided".to_string(),
                },
                execution: graph,
            }),
        );
    }

    // Execute agent logic
    let result = CorrelateResponse {
        incidents_count: 0,
        signals_processed: request.signals.len(),
        noise_reduction: 0.0,
        status: "success".to_string(),
    };

    agent_span.attach_evidence(Evidence {
        evidence_type: "correlation_result".to_string(),
        reference: format!("incident_correlation:{}", agent_span.span_id),
        payload: serde_json::json!({
            "incidents_count": result.incidents_count,
            "signals_processed": result.signals_processed,
            "noise_reduction": result.noise_reduction,
        }),
    });

    agent_span.complete();
    collector.0.add_agent_span(agent_span);

    let graph = collector.0.finalize();

    (
        StatusCode::OK,
        Json(InstrumentedResponse {
            data: result,
            execution: graph,
        }),
    )
}

/// GET /api/v1/agents/correlation/config
#[instrument(skip(_state))]
pub async fn correlation_config(
    State(_state): State<Arc<IncidentCorrelationState>>,
) -> impl IntoResponse {
    info!("Getting correlation configuration");

    let response = CorrelationConfigResponse {
        agent_id: "sentinel.correlation.incident".to_string(),
        agent_version: "1.0.0".to_string(),
        time_window_secs: 300,
        min_signals_per_incident: 2,
        correlation_threshold: 0.6,
    };

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}

/// GET /api/v1/agents/correlation/stats
#[instrument(skip(_state))]
pub async fn correlation_stats(
    State(_state): State<Arc<IncidentCorrelationState>>,
) -> impl IntoResponse {
    info!("Getting correlation statistics");

    let response = CorrelationStatsResponse {
        agent_id: "sentinel.correlation.incident".to_string(),
        agent_version: "1.0.0".to_string(),
        total_invocations: 0,
        incidents_correlated: 0,
        signals_processed: 0,
        avg_noise_reduction: 0.0,
    };

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}
