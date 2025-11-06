//! API server implementation.

use crate::{
    handlers::{health::HealthState, metrics::MetricsState, query::QueryState},
    routes::create_router,
    ApiConfig,
};
use llm_sentinel_storage::Storage;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, error};

/// API server
pub struct ApiServer {
    config: ApiConfig,
    health_state: Arc<HealthState>,
    metrics_state: Arc<MetricsState>,
    query_state: Arc<QueryState>,
}

impl ApiServer {
    /// Create a new API server
    pub fn new(
        config: ApiConfig,
        storage: Arc<dyn Storage>,
        version: String,
    ) -> Self {
        let storage_clone = storage.clone();
        let health_state = Arc::new(HealthState::new(
            version,
            Arc::new(move || {
                match futures::executor::block_on(storage_clone.health_check()) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.to_string()),
                }
            }),
        ));

        let metrics_state = Arc::new(MetricsState::new());
        let query_state = Arc::new(QueryState::new(storage));

        Self {
            config,
            health_state,
            metrics_state,
            query_state,
        }
    }

    /// Start the API server
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting API server on {}", self.config.bind_addr);

        let router = create_router(
            self.config.clone(),
            self.health_state,
            self.metrics_state,
            self.query_state,
        );

        let listener = TcpListener::bind(self.config.bind_addr).await?;

        info!("API server listening on {}", self.config.bind_addr);
        info!("Health check: http://{}/health", self.config.bind_addr);
        info!(
            "Metrics: http://{}{}",
            self.config.bind_addr, self.config.metrics_path
        );
        info!("API docs: http://{}/api/v1", self.config.bind_addr);

        axum::serve(listener, router)
            .await
            .map_err(|e| {
                error!("Server error: {}", e);
                Box::new(e) as Box<dyn std::error::Error>
            })
    }

    /// Get server bind address
    pub fn bind_addr(&self) -> std::net::SocketAddr {
        self.config.bind_addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_storage::Storage;

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
    fn test_server_creation() {
        let config = ApiConfig::default();
        let storage: Arc<dyn Storage> = Arc::new(MockStorage);
        let server = ApiServer::new(config.clone(), storage, "0.1.0".to_string());

        assert_eq!(server.bind_addr(), config.bind_addr);
    }
}
