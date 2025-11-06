//! API route definitions.

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::timeout::TimeoutLayer;
use std::time::Duration;

use crate::{
    handlers::{health::*, metrics::*, query::*},
    middleware::{cors_middleware, logging_middleware},
    ApiConfig,
};

/// Create the main API router
pub fn create_router(
    config: ApiConfig,
    health_state: Arc<HealthState>,
    metrics_state: Arc<MetricsState>,
    query_state: Arc<QueryState>,
) -> Router {
    // API v1 routes
    let api_v1 = Router::new()
        .route("/telemetry", get(query_telemetry))
        .route("/anomalies", get(query_anomalies))
        .with_state(query_state);

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
