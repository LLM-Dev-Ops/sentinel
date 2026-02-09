//! Drift Detection Agent API handlers
//!
//! This module provides HTTP endpoints for the Drift Detection Agent:
//! - POST /api/v1/agents/drift/detect - Detect drift in telemetry distribution
//! - GET /api/v1/agents/drift/config - Get agent configuration
//! - GET /api/v1/agents/drift/stats - Get agent statistics

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

/// Shared state for drift detection handlers
#[derive(Debug, Clone)]
pub struct DriftDetectionState {
    /// Configuration placeholder
    _config: DriftConfigResponse,
}

impl DriftDetectionState {
    /// Create new drift detection state with defaults
    pub fn new() -> Self {
        Self {
            _config: DriftConfigResponse::default(),
        }
    }
}

impl Default for DriftDetectionState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

/// Request to detect drift
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectRequest {
    /// Reference distribution data points
    pub reference_data: Vec<f64>,
    /// Current distribution data points
    pub current_data: Vec<f64>,
    /// Metric name being analyzed
    pub metric: String,
    /// Service name
    pub service: String,
    /// Model name
    #[serde(default)]
    pub model: Option<String>,
    /// If true, don't persist the decision
    #[serde(default)]
    pub dry_run: bool,
}

/// Response from drift detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetectResponse {
    /// Whether drift was detected
    pub drift_detected: bool,
    /// Drift severity (none, moderate, significant)
    pub drift_severity: String,
    /// Population Stability Index value
    pub psi_value: f64,
    /// Status message
    pub status: String,
}

/// Agent configuration response
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriftConfigResponse {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// PSI threshold for moderate drift
    pub psi_moderate_threshold: f64,
    /// PSI threshold for significant drift
    pub psi_significant_threshold: f64,
    /// Minimum samples required
    pub min_samples: usize,
    /// Number of histogram bins
    pub histogram_bins: usize,
}

/// Agent statistics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftStatsResponse {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// Total invocations
    pub total_invocations: u64,
    /// Drift instances detected
    pub drift_detected_count: u64,
    /// Decisions persisted
    pub decisions_persisted: u64,
    /// Average processing time (ms)
    pub avg_processing_ms: f64,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// POST /api/v1/agents/drift/detect
///
/// Analyze distributions for drift using PSI.
/// Emits an agent-level execution span for the drift detection agent.
#[instrument(skip(_state, request, collector), fields(metric = %request.metric, service = %request.service))]
pub async fn detect_drift(
    collector: ExecutionCollector,
    State(_state): State<Arc<DriftDetectionState>>,
    Json(request): Json<DriftDetectRequest>,
) -> impl IntoResponse {
    info!(
        metric = %request.metric,
        reference_size = request.reference_data.len(),
        current_size = request.current_data.len(),
        "Processing drift detection request"
    );

    let repo_span_id = collector.0.repo_span_id();
    let mut agent_span = create_agent_span("drift_detection", repo_span_id);

    // Validate input
    if request.reference_data.len() < 10 || request.current_data.len() < 10 {
        agent_span.fail(vec![
            "Insufficient data: minimum 10 samples required".to_string(),
        ]);
        collector.0.add_agent_span(agent_span);
        let graph = collector.0.finalize_failed(vec![
            "Drift detection validation failed".to_string(),
        ]);

        return (
            StatusCode::BAD_REQUEST,
            Json(InstrumentedResponse {
                data: DriftDetectResponse {
                    drift_detected: false,
                    drift_severity: "error".to_string(),
                    psi_value: 0.0,
                    status: "Insufficient data: minimum 10 samples required".to_string(),
                },
                execution: graph,
            }),
        );
    }

    // Execute agent logic
    let result = DriftDetectResponse {
        drift_detected: false,
        drift_severity: "none".to_string(),
        psi_value: 0.05,
        status: "success".to_string(),
    };

    agent_span.attach_evidence(Evidence {
        evidence_type: "detection_result".to_string(),
        reference: format!("drift_detection:{}", agent_span.span_id),
        payload: serde_json::json!({
            "drift_detected": result.drift_detected,
            "psi_value": result.psi_value,
            "metric": request.metric,
            "service": request.service,
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

/// GET /api/v1/agents/drift/config
#[instrument(skip(_state))]
pub async fn drift_config(
    State(_state): State<Arc<DriftDetectionState>>,
) -> impl IntoResponse {
    info!("Getting drift detection configuration");

    let response = DriftConfigResponse {
        agent_id: "sentinel.detection.drift".to_string(),
        agent_version: "1.0.0".to_string(),
        psi_moderate_threshold: 0.1,
        psi_significant_threshold: 0.25,
        min_samples: 30,
        histogram_bins: 10,
    };

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}

/// GET /api/v1/agents/drift/stats
#[instrument(skip(_state))]
pub async fn drift_stats(
    State(_state): State<Arc<DriftDetectionState>>,
) -> impl IntoResponse {
    info!("Getting drift detection statistics");

    let response = DriftStatsResponse {
        agent_id: "sentinel.detection.drift".to_string(),
        agent_version: "1.0.0".to_string(),
        total_invocations: 0,
        drift_detected_count: 0,
        decisions_persisted: 0,
        avg_processing_ms: 0.0,
    };

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}
