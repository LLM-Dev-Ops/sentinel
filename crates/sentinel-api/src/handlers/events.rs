//! Event ingestion handler for intelligence-core integration.
//!
//! - POST /api/v1/events - Accept events from intelligence-core

use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

/// Inbound event from intelligence-core
#[derive(Debug, Clone, Deserialize)]
pub struct IngestEventRequest {
    /// Source system identifier
    pub source: String,
    /// Type of event
    pub event_type: String,
    /// Execution ID for correlation
    pub execution_id: String,
    /// ISO-8601 timestamp
    pub timestamp: String,
    /// Arbitrary event payload
    pub payload: serde_json::Value,
}

/// Acknowledgement returned to the caller
#[derive(Debug, Clone, Serialize)]
pub struct IngestEventResponse {
    /// Status indicator
    pub status: String,
    /// Echoed execution_id
    pub execution_id: String,
}

// =============================================================================
// HANDLER
// =============================================================================

/// POST /api/v1/events
///
/// Accepts events emitted by the intelligence-core bundle.
/// Returns 202 Accepted with the execution_id echoed back.
#[instrument(skip(request), fields(source = %request.source, event_type = %request.event_type, execution_id = %request.execution_id))]
pub async fn ingest_event(
    Json(request): Json<IngestEventRequest>,
) -> impl IntoResponse {
    info!(
        source = %request.source,
        event_type = %request.event_type,
        execution_id = %request.execution_id,
        timestamp = %request.timestamp,
        "Received event from intelligence-core"
    );

    let response = IngestEventResponse {
        status: "accepted".to_string(),
        execution_id: request.execution_id,
    };

    (StatusCode::ACCEPTED, Json(response))
}
