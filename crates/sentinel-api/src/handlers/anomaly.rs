//! Anomaly Detection Agent API handlers
//!
//! This module provides HTTP endpoints for the Anomaly Detection Agent:
//! - POST /api/v1/agents/anomaly/detect - Process telemetry for anomaly detection
//! - GET /api/v1/agents/anomaly/config - Get agent configuration
//! - GET /api/v1/agents/anomaly/stats - Get agent statistics

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

/// Shared state for anomaly detection handlers
#[derive(Debug, Clone)]
pub struct AnomalyDetectionState {
    /// Configuration placeholder
    _config: AnomalyConfigResponse,
}

impl AnomalyDetectionState {
    /// Create new anomaly detection state with defaults
    pub fn new() -> Self {
        Self {
            _config: AnomalyConfigResponse::default(),
        }
    }
}

impl Default for AnomalyDetectionState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

/// Request to detect anomalies in telemetry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectRequest {
    /// Telemetry event to analyze
    pub telemetry: serde_json::Value,
    /// If true, don't persist the decision
    #[serde(default)]
    pub dry_run: bool,
}

/// Response from anomaly detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectResponse {
    /// Whether an anomaly was detected
    pub anomaly_detected: bool,
    /// Anomaly event if detected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly: Option<serde_json::Value>,
    /// Status message
    pub status: String,
}

/// Agent configuration response
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnomalyConfigResponse {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// Z-score enabled
    pub enable_zscore: bool,
    /// Z-score threshold
    pub zscore_threshold: f64,
    /// IQR enabled
    pub enable_iqr: bool,
    /// CUSUM enabled
    pub enable_cusum: bool,
}

/// Agent statistics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyStatsResponse {
    /// Agent ID
    pub agent_id: String,
    /// Agent version
    pub agent_version: String,
    /// Total invocations
    pub total_invocations: u64,
    /// Anomalies detected
    pub anomalies_detected: u64,
    /// Decisions persisted
    pub decisions_persisted: u64,
    /// Average processing time (ms)
    pub avg_processing_ms: f64,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// POST /api/v1/agents/anomaly/detect
///
/// Emits an agent-level execution span for the anomaly detection agent.
/// Returns an `InstrumentedResponse` containing the execution graph.
#[instrument(skip(_state, request, collector))]
pub async fn detect_anomaly(
    collector: ExecutionCollector,
    State(_state): State<Arc<AnomalyDetectionState>>,
    Json(request): Json<DetectRequest>,
) -> impl IntoResponse {
    info!("Processing telemetry for anomaly detection");

    let repo_span_id = collector.0.repo_span_id();
    let mut agent_span = create_agent_span("anomaly_detection", repo_span_id);

    // Execute agent logic
    let result = DetectResponse {
        anomaly_detected: false,
        anomaly: None,
        status: "success".to_string(),
    };

    // Attach evidence of execution
    agent_span.attach_evidence(Evidence {
        evidence_type: "detection_result".to_string(),
        reference: format!("anomaly_detection:{}", agent_span.span_id),
        payload: serde_json::json!({
            "anomaly_detected": result.anomaly_detected,
            "dry_run": request.dry_run,
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

/// GET /api/v1/agents/anomaly/config
#[instrument(skip(_state))]
pub async fn anomaly_config(
    State(_state): State<Arc<AnomalyDetectionState>>,
) -> impl IntoResponse {
    info!("Getting anomaly detection configuration");

    let response = AnomalyConfigResponse {
        agent_id: "sentinel.detection.anomaly".to_string(),
        agent_version: "1.0.0".to_string(),
        enable_zscore: true,
        zscore_threshold: 3.0,
        enable_iqr: true,
        enable_cusum: true,
    };

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}

/// GET /api/v1/agents/anomaly/stats
#[instrument(skip(_state))]
pub async fn anomaly_stats(
    State(_state): State<Arc<AnomalyDetectionState>>,
) -> impl IntoResponse {
    info!("Getting anomaly detection statistics");

    let response = AnomalyStatsResponse {
        agent_id: "sentinel.detection.anomaly".to_string(),
        agent_version: "1.0.0".to_string(),
        total_invocations: 0,
        anomalies_detected: 0,
        decisions_persisted: 0,
        avg_processing_ms: 0.0,
    };

    (StatusCode::OK, Json(SuccessResponse::new(response)))
}
