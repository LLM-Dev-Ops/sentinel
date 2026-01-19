//! API route definitions.
//!
//! This module defines all HTTP routes for the LLM-Sentinel unified service.
//! All agents are exposed through a single service with the following endpoints:
//!
//! ## Agent Endpoints
//! - `/api/v1/agents/anomaly/*` - Anomaly Detection Agent
//! - `/api/v1/agents/drift/*` - Drift Detection Agent
//! - `/api/v1/agents/alerting/*` - Alerting Agent
//! - `/api/v1/agents/correlation/*` - Incident Correlation Agent
//! - `/api/v1/agents/rca/*` - Root Cause Analysis Agent
//!
//! ## Query Endpoints
//! - `/api/v1/telemetry` - Query telemetry events
//! - `/api/v1/anomalies` - Query anomalies
//!
//! ## Infrastructure Endpoints
//! - `/health`, `/health/live`, `/health/ready` - Health checks
//! - `/metrics` - Prometheus metrics

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::timeout::TimeoutLayer;
use std::time::Duration;

use crate::{
    handlers::{
        alerting::*, anomaly::*, correlation::*, drift::*, health::*, metrics::*, query::*, rca::*,
    },
    middleware::{cors_middleware, logging_middleware},
    ApiConfig,
};

/// Agent states for the unified service
#[derive(Debug, Default)]
pub struct AgentStates {
    pub anomaly: Option<Arc<AnomalyDetectionState>>,
    pub drift: Option<Arc<DriftDetectionState>>,
    pub alerting: Option<Arc<AlertingState>>,
    pub correlation: Option<Arc<IncidentCorrelationState>>,
    pub rca: Option<Arc<RcaState>>,
}

/// Create the main API router (legacy compatibility)
pub fn create_router(
    config: ApiConfig,
    health_state: Arc<HealthState>,
    metrics_state: Arc<MetricsState>,
    query_state: Arc<QueryState>,
) -> Router {
    create_router_with_alerting(config, health_state, metrics_state, query_state, None)
}

/// Create the main API router with optional alerting state (legacy compatibility)
pub fn create_router_with_alerting(
    config: ApiConfig,
    health_state: Arc<HealthState>,
    metrics_state: Arc<MetricsState>,
    query_state: Arc<QueryState>,
    alerting_state: Option<Arc<AlertingState>>,
) -> Router {
    let agent_states = AgentStates {
        alerting: alerting_state,
        ..Default::default()
    };
    create_unified_router(config, health_state, metrics_state, query_state, agent_states)
}

/// Create the unified API router with all agent states
///
/// This is the primary router for production deployment.
/// All agents are exposed through a single service.
pub fn create_unified_router(
    config: ApiConfig,
    health_state: Arc<HealthState>,
    metrics_state: Arc<MetricsState>,
    query_state: Arc<QueryState>,
    agent_states: AgentStates,
) -> Router {
    // API v1 routes - query endpoints
    let api_v1 = Router::new()
        .route("/telemetry", get(query_telemetry))
        .route("/anomalies", get(query_anomalies))
        .with_state(query_state);

    // Anomaly Detection Agent routes
    let anomaly_routes = if let Some(state) = agent_states.anomaly {
        Router::new()
            .route("/agents/anomaly/detect", post(detect_anomaly))
            .route("/agents/anomaly/config", get(anomaly_config))
            .route("/agents/anomaly/stats", get(anomaly_stats))
            .with_state(state)
    } else {
        Router::new()
    };

    // Drift Detection Agent routes
    let drift_routes = if let Some(state) = agent_states.drift {
        Router::new()
            .route("/agents/drift/detect", post(detect_drift))
            .route("/agents/drift/config", get(drift_config))
            .route("/agents/drift/stats", get(drift_stats))
            .with_state(state)
    } else {
        Router::new()
    };

    // Alerting Agent routes
    let alerting_routes = if let Some(state) = agent_states.alerting {
        Router::new()
            .route("/agents/alerting/evaluate", post(evaluate_alert))
            .route("/agents/alerting/rules", get(list_rules))
            .route("/agents/alerting/stats", get(alerting_stats))
            .with_state(state)
    } else {
        Router::new()
    };

    // Incident Correlation Agent routes
    let correlation_routes = if let Some(state) = agent_states.correlation {
        Router::new()
            .route("/agents/correlation/correlate", post(correlate_signals))
            .route("/agents/correlation/config", get(correlation_config))
            .route("/agents/correlation/stats", get(correlation_stats))
            .with_state(state)
    } else {
        Router::new()
    };

    // Root Cause Analysis Agent routes
    let rca_routes = if let Some(state) = agent_states.rca {
        Router::new()
            .route("/agents/rca/analyze", post(analyze_root_cause))
            .route("/agents/rca/config", get(rca_config))
            .route("/agents/rca/stats", get(rca_stats))
            .with_state(state)
    } else {
        Router::new()
    };

    // Merge all agent routes into API v1
    let api_v1 = api_v1
        .merge(anomaly_routes)
        .merge(drift_routes)
        .merge(alerting_routes)
        .merge(correlation_routes)
        .merge(rca_routes);

    // Health routes
    let health_routes = Router::new()
        .route("/health", get(health))
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
        .with_state(health_state);

    // Metrics route
    let metrics_route = Router::new()
        .route(&config.metrics_path, get(metrics_handler))
        .with_state(metrics_state);

    // Combine all routes
    let app = Router::new()
        .nest("/api/v1", api_v1)
        .merge(health_routes)
        .merge(metrics_route);

    // Add middleware
    let app = if config.enable_logging {
        app.layer(middleware::from_fn(logging_middleware))
    } else {
        app
    };

    let app = app.layer(cors_middleware(config.cors_origins));

    let app = app.layer(TimeoutLayer::new(Duration::from_secs(config.timeout_secs)));

    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_storage::Storage;
    use std::sync::Arc;

    // Mock storage for testing
    struct MockStorage;

    #[async_trait::async_trait]
    impl Storage for MockStorage {
        async fn write_telemetry(
            &self,
            _event: &sentinel_core::events::TelemetryEvent,
        ) -> sentinel_core::Result<()> {
            Ok(())
        }

        async fn write_anomaly(
            &self,
            _anomaly: &sentinel_core::events::AnomalyEvent,
        ) -> sentinel_core::Result<()> {
            Ok(())
        }

        async fn write_telemetry_batch(
            &self,
            _events: &[sentinel_core::events::TelemetryEvent],
        ) -> sentinel_core::Result<()> {
            Ok(())
        }

        async fn write_anomaly_batch(
            &self,
            _anomalies: &[sentinel_core::events::AnomalyEvent],
        ) -> sentinel_core::Result<()> {
            Ok(())
        }

        async fn query_telemetry(
            &self,
            _query: sentinel_storage::query::TelemetryQuery,
        ) -> sentinel_core::Result<Vec<sentinel_core::events::TelemetryEvent>> {
            Ok(Vec::new())
        }

        async fn query_anomalies(
            &self,
            _query: sentinel_storage::query::AnomalyQuery,
        ) -> sentinel_core::Result<Vec<sentinel_core::events::AnomalyEvent>> {
            Ok(Vec::new())
        }

        async fn health_check(&self) -> sentinel_core::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_router_creation() {
        let config = ApiConfig::default();

        let health_state = Arc::new(HealthState::new(
            "0.1.0".to_string(),
            Arc::new(|| Ok(())),
        ));

        let metrics_state = Arc::new(MetricsState::new());

        let storage: Arc<dyn Storage> = Arc::new(MockStorage);
        let query_state = Arc::new(QueryState::new(storage));

        let router = create_router(config, health_state, metrics_state, query_state);

        // Just test that it creates without panicking
        drop(router);
    }
}
