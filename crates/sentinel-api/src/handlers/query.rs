//! Query endpoints for telemetry and anomalies.

use axum::{extract::{Query, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use llm_sentinel_core::{
    events::{AnomalyEvent, TelemetryEvent},
    types::{AnomalyType, ModelId, ServiceId, Severity},
};
use llm_sentinel_storage::{
    query::{AnomalyQuery, TelemetryQuery, TimeRange},
    Storage,
};
use std::sync::Arc;
use tracing::{debug, error};

use crate::{ErrorResponse, ResponseMetadata, SuccessResponse};

/// Application state for queries
#[derive(Clone)]
pub struct QueryState {
    pub storage: Arc<dyn Storage>,
}

impl QueryState {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }
}

/// Query parameters for telemetry
#[derive(Debug, Deserialize)]
pub struct TelemetryQueryParams {
    /// Service ID filter
    pub service: Option<String>,
    /// Model ID filter
    pub model: Option<String>,
    /// Start time (ISO 8601)
    pub start: Option<String>,
    /// End time (ISO 8601)
    pub end: Option<String>,
    /// Time range in hours (alternative to start/end)
    pub hours: Option<i64>,
    /// Limit results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
    /// Sort ascending
    pub ascending: Option<bool>,
}

/// Query parameters for anomalies
#[derive(Debug, Deserialize)]
pub struct AnomalyQueryParams {
    /// Service ID filter
    pub service: Option<String>,
    /// Model ID filter
    pub model: Option<String>,
    /// Severity filter
    pub severity: Option<String>,
    /// Anomaly type filter
    pub anomaly_type: Option<String>,
    /// Minimum confidence
    pub min_confidence: Option<f64>,
    /// Start time (ISO 8601)
    pub start: Option<String>,
    /// End time (ISO 8601)
    pub end: Option<String>,
    /// Time range in hours
    pub hours: Option<i64>,
    /// Limit results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
}

/// Telemetry query endpoint
pub async fn query_telemetry(
    State(state): State<Arc<QueryState>>,
    Query(params): Query<TelemetryQueryParams>,
) -> Result<Json<SuccessResponse<Vec<TelemetryEvent>>>, (StatusCode, Json<ErrorResponse>)> {
    debug!("Telemetry query: {:?}", params);

    // Build time range
    let time_range = match (params.start, params.end, params.hours) {
        (Some(start), Some(end), _) => {
            let start_dt = chrono::DateTime::parse_from_rfc3339(&start)
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse::new(
                            "invalid_time",
                            format!("Invalid start time: {}", e),
                        )),
                    )
                })?
                .with_timezone(&chrono::Utc);

            let end_dt = chrono::DateTime::parse_from_rfc3339(&end)
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse::new(
                            "invalid_time",
                            format!("Invalid end time: {}", e),
                        )),
                    )
                })?
                .with_timezone(&chrono::Utc);

            TimeRange::new(start_dt, end_dt)
        }
        (_, _, Some(hours)) => TimeRange::last_hours(hours),
        _ => TimeRange::last_hours(24), // Default: last 24 hours
    };

    // Build query
    let mut query = TelemetryQuery::new(time_range);

    if let Some(service) = params.service {
        query = query.with_service(ServiceId::new(service));
    }

    if let Some(model) = params.model {
        query = query.with_model(ModelId::new(model));
    }

    if let Some(limit) = params.limit {
        query = query.with_limit(limit);
    }

    if let Some(offset) = params.offset {
        query = query.with_offset(offset);
    }

    if params.ascending.unwrap_or(false) {
        query = query.ascending();
    } else {
        query = query.descending();
    }

    // Execute query
    let events = state
        .storage
        .query_telemetry(query)
        .await
        .map_err(|e| {
            error!("Telemetry query failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("query_failed", e.to_string())),
            )
        })?;

    debug!("Retrieved {} telemetry events", events.len());

    let response = SuccessResponse::new(events.clone()).with_metadata(ResponseMetadata {
        total_count: Some(events.len()),
        page: params.offset.map(|o| o / params.limit.unwrap_or(100)),
        page_size: params.limit,
    });

    Ok(Json(response))
}

/// Anomaly query endpoint
pub async fn query_anomalies(
    State(state): State<Arc<QueryState>>,
    Query(params): Query<AnomalyQueryParams>,
) -> Result<Json<SuccessResponse<Vec<AnomalyEvent>>>, (StatusCode, Json<ErrorResponse>)> {
    debug!("Anomaly query: {:?}", params);

    // Build time range
    let time_range = match (params.start, params.end, params.hours) {
        (Some(start), Some(end), _) => {
            let start_dt = chrono::DateTime::parse_from_rfc3339(&start)
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse::new(
                            "invalid_time",
                            format!("Invalid start time: {}", e),
                        )),
                    )
                })?
                .with_timezone(&chrono::Utc);

            let end_dt = chrono::DateTime::parse_from_rfc3339(&end)
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse::new(
                            "invalid_time",
                            format!("Invalid end time: {}", e),
                        )),
                    )
                })?
                .with_timezone(&chrono::Utc);

            TimeRange::new(start_dt, end_dt)
        }
        (_, _, Some(hours)) => TimeRange::last_hours(hours),
        _ => TimeRange::last_hours(24), // Default: last 24 hours
    };

    // Build query
    let mut query = AnomalyQuery::new(time_range);

    if let Some(service) = params.service {
        query = query.with_service(ServiceId::new(service));
    }

    if let Some(model) = params.model {
        query = query.with_model(ModelId::new(model));
    }

    if let Some(severity_str) = params.severity {
        let severity = parse_severity(&severity_str).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("invalid_severity", e)),
            )
        })?;
        query = query.with_severity(severity);
    }

    if let Some(type_str) = params.anomaly_type {
        let anomaly_type = parse_anomaly_type(&type_str).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("invalid_anomaly_type", e)),
            )
        })?;
        query = query.with_type(anomaly_type);
    }

    if let Some(confidence) = params.min_confidence {
        query = query.with_min_confidence(confidence);
    }

    if let Some(limit) = params.limit {
        query = query.with_limit(limit);
    }

    // Execute query
    let anomalies = state
        .storage
        .query_anomalies(query)
        .await
        .map_err(|e| {
            error!("Anomaly query failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("query_failed", e.to_string())),
            )
        })?;

    debug!("Retrieved {} anomalies", anomalies.len());

    let response = SuccessResponse::new(anomalies.clone()).with_metadata(ResponseMetadata {
        total_count: Some(anomalies.len()),
        page: params.offset.map(|o| o / params.limit.unwrap_or(100)),
        page_size: params.limit,
    });

    Ok(Json(response))
}

/// Parse severity string
fn parse_severity(s: &str) -> Result<Severity, String> {
    match s.to_lowercase().as_str() {
        "low" => Ok(Severity::Low),
        "medium" => Ok(Severity::Medium),
        "high" => Ok(Severity::High),
        "critical" => Ok(Severity::Critical),
        _ => Err(format!("Invalid severity: {}", s)),
    }
}

/// Parse anomaly type string
fn parse_anomaly_type(s: &str) -> Result<AnomalyType, String> {
    match s.to_lowercase().replace('-', "_").as_str() {
        "latency_spike" => Ok(AnomalyType::LatencySpike),
        "token_usage_spike" => Ok(AnomalyType::TokenUsageSpike),
        "cost_anomaly" => Ok(AnomalyType::CostAnomaly),
        "error_rate_spike" => Ok(AnomalyType::ErrorRateSpike),
        "prompt_injection" => Ok(AnomalyType::PromptInjection),
        "data_exfiltration" => Ok(AnomalyType::DataExfiltration),
        "unusual_pattern" => Ok(AnomalyType::UnusualPattern),
        "jailbreak_attempt" => Ok(AnomalyType::JailbreakAttempt),
        "context_overflow" => Ok(AnomalyType::ContextOverflow),
        "model_drift" => Ok(AnomalyType::ModelDrift),
        "quality_degradation" => Ok(AnomalyType::QualityDegradation),
        "compliance_violation" => Ok(AnomalyType::ComplianceViolation),
        _ => Err(format!("Invalid anomaly type: {}", s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_severity() {
        assert_eq!(parse_severity("low"), Ok(Severity::Low));
        assert_eq!(parse_severity("HIGH"), Ok(Severity::High));
        assert_eq!(parse_severity("Medium"), Ok(Severity::Medium));
        assert!(parse_severity("invalid").is_err());
    }

    #[test]
    fn test_parse_anomaly_type() {
        assert_eq!(
            parse_anomaly_type("latency-spike"),
            Ok(AnomalyType::LatencySpike)
        );
        assert_eq!(
            parse_anomaly_type("COST_ANOMALY"),
            Ok(AnomalyType::CostAnomaly)
        );
        assert!(parse_anomaly_type("invalid").is_err());
    }
}
