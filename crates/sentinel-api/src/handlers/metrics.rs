//! Prometheus metrics endpoint.

use axum::http::StatusCode;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::Arc;
use tracing::{debug, warn};

/// Metrics exporter handle
#[derive(Clone)]
pub struct MetricsState {
    handle: Arc<PrometheusHandle>,
}

impl MetricsState {
    /// Create a new metrics state with Prometheus exporter
    pub fn new() -> Self {
        let handle = PrometheusBuilder::new()
            .set_buckets_for_metric(
                Matcher::Full("sentinel_detection_latency_seconds".to_string()),
                &[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0],
            )
            .unwrap()
            .set_buckets_for_metric(
                Matcher::Full("sentinel_ingestion_latency_seconds".to_string()),
                &[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0],
            )
            .unwrap()
            .install_recorder()
            .unwrap();

        Self {
            handle: Arc::new(handle),
        }
    }

    /// Get the Prometheus handle
    pub fn handle(&self) -> Arc<PrometheusHandle> {
        self.handle.clone()
    }
}

/// Prometheus metrics endpoint handler
pub async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<Arc<MetricsState>>,
) -> Result<String, StatusCode> {
    debug!("Metrics endpoint called");

    let metrics = state.handle.render();
    debug!("Rendered {} bytes of metrics", metrics.len());
    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_state_creation() {
        let state = MetricsState::new();
        let handle = state.handle();
        assert!(Arc::strong_count(&handle) >= 1);
    }

    #[tokio::test]
    async fn test_metrics_handler() {
        let state = Arc::new(MetricsState::new());

        // Increment a test metric
        metrics::counter!("test_counter").increment(1);

        let result = metrics_handler(axum::extract::State(state)).await;
        assert!(result.is_ok());

        let metrics_text = result.unwrap();
        assert!(metrics_text.contains("test_counter"));
    }
}
