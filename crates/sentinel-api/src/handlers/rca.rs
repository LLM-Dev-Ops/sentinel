//! Root Cause Analysis Agent API handlers
//!
//! This module provides HTTP endpoints for the Root Cause Analysis Agent:
//! - POST /api/v1/agents/rca/analyze - Perform root cause analysis
//! - GET /api/v1/agents/rca/config - Get agent configuration
//! - GET /api/v1/agents/rca/stats - Get agent statistics

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

/// Shared state for RCA handlers
#[derive(Debug, Clone)]
pub struct RcaState {
    /// Configuration placeholder
    _config: RcaConfigResponse,
}

impl RcaState {
    /// Create new RCA state with defaults
    pub fn new() -> Self {
        Self {
            _config: RcaConfigResponse::default(),
        }
    }
}

impl Default for RcaState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

/// Request to perform root cause analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaRequest {
    /// Incident or correlated signals to analyze
    pub incident: serde_json::Value,
    /// Maximum hypotheses to generate
    #[serde(default)]
    pub max_hypotheses: Option<usize>,
    /// If true, don't persist the decision
    #[serde(default)]
    pub dry_run: bool,
}

/// Response from RCA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaResponse {
    /// Number of hypotheses generated
    pub hypotheses_count: usize,
    /// Analysis confidence
    pub confidence: f64,
    /// Status message
    pub status: String,
}

/// Agent configuration response
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RcaConfigResponse {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// Max hypotheses
    pub max_hypotheses: usize,
    /// Temporal min strength
    pub temporal_min_strength: f64,
    /// Analysis timeout (seconds)
    pub analysis_timeout_secs: u64,
}

/// Agent statistics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaStatsResponse {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// Total invocations
    pub total_invocations: u64,
    /// Analyses completed
    pub analyses_completed: u64,
    /// Hypotheses generated
    pub hypotheses_generated: u64,
    /// Average confidence
    pub avg_confidence: f64,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// POST /api/v1/agents/rca/analyze
///
/// Emits an agent-level execution span for the root cause analysis agent.
#[instrument(skip(_state, request, collector))]
pub async fn analyze_root_cause(
    collector: ExecutionCollector,
    State(_state): State<Arc<RcaState>>,
    Json(request): Json<RcaRequest>,
) -> impl IntoResponse {
    info!("Processing root cause analysis request");

    let repo_span_id = collector.0.repo_span_id();
    let mut agent_span = create_agent_span("root_cause_analysis", repo_span_id);

    // Execute agent logic
    let result = RcaResponse {
        hypotheses_count: 0,
        confidence: 0.0,
        status: "success".to_string(),
    };

    agent_span.attach_evidence(Evidence {
        evidence_type: "rca_result".to_string(),
        reference: format!("root_cause_analysis:{}", agent_span.span_id),
        payload: serde_json::json!({
            "hypotheses_count": result.hypotheses_count,
            "confidence": result.confidence,
            "max_hypotheses_requested": request.max_hypotheses,
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

/// GET /api/v1/agents/rca/config
#[instrument(skip(_state))]
pub async fn rca_config(
    State(_state): State<Arc<RcaState>>,
) -> impl IntoResponse {
    info!("Getting RCA configuration");

    let response = RcaConfigResponse {
        agent_id: "sentinel.analysis.rca".to_string(),
        agent_version: "1.0.0".to_string(),
        max_hypotheses: 5,
        temporal_min_strength: 0.5,
        analysis_timeout_secs: 30,
    };

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}

/// GET /api/v1/agents/rca/stats
#[instrument(skip(_state))]
pub async fn rca_stats(
    State(_state): State<Arc<RcaState>>,
) -> impl IntoResponse {
    info!("Getting RCA statistics");

    let response = RcaStatsResponse {
        agent_id: "sentinel.analysis.rca".to_string(),
        agent_version: "1.0.0".to_string(),
        total_invocations: 0,
        analyses_completed: 0,
        hypotheses_generated: 0,
        avg_confidence: 0.0,
    };

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}
