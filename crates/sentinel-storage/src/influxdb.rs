//! InfluxDB storage backend for time-series data.

use crate::{query::{AnomalyQuery, TelemetryQuery}, Storage};
use async_trait::async_trait;
use influxdb2::models::DataPoint;
use influxdb2::Client;
use llm_sentinel_core::{
    events::{AnomalyEvent, TelemetryEvent},
    Error, Result,
};
use tracing::{debug, error, info, warn};

/// InfluxDB configuration
#[derive(Debug, Clone)]
pub struct InfluxDbConfig {
    /// InfluxDB URL
    pub url: String,
    /// Organization
    pub org: String,
    /// Bucket name for telemetry
    pub telemetry_bucket: String,
    /// Bucket name for anomalies
    pub anomaly_bucket: String,
    /// Auth token
    pub token: String,
    /// Batch size for writes
    pub batch_size: usize,
    /// Connection timeout in seconds
    pub timeout_secs: u64,
}

impl Default for InfluxDbConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8086".to_string(),
            org: "sentinel".to_string(),
            telemetry_bucket: "telemetry".to_string(),
            anomaly_bucket: "anomalies".to_string(),
            token: String::new(),
            batch_size: 100,
            timeout_secs: 10,
        }
    }
}

/// InfluxDB storage backend
pub struct InfluxDbStorage {
    client: Client,
    config: InfluxDbConfig,
}

impl InfluxDbStorage {
    /// Create a new InfluxDB storage backend
    pub async fn new(config: InfluxDbConfig) -> Result<Self> {
        info!(
            "Connecting to InfluxDB at {} (org: {})",
            config.url, config.org
        );

        let client = Client::new(&config.url, &config.org, &config.token);

        // Test connection
        if let Err(e) = client.health().await {
            error!("Failed to connect to InfluxDB: {}", e);
            return Err(Error::connection(format!(
                "InfluxDB connection failed: {}",
                e
            )));
        }

        info!("Connected to InfluxDB successfully");

        Ok(Self { client, config })
    }

    /// Convert telemetry event to InfluxDB data point
    fn telemetry_to_point(&self, event: &TelemetryEvent) -> DataPoint {
        let mut point = DataPoint::builder("telemetry")
            .tag("service", event.service_name.as_str())
            .tag("model", event.model.as_str())
            .field("latency_ms", event.latency_ms)
            .field("prompt_tokens", event.prompt.tokens as i64)
            .field("response_tokens", event.response.tokens as i64)
            .field("total_tokens", event.total_tokens() as i64)
            .field("cost_usd", event.cost_usd)
            .field("has_errors", event.has_errors() as i64)
            .timestamp(event.timestamp.timestamp_nanos_opt().unwrap_or(0));

        // Add metadata as tags
        for (key, value) in &event.metadata {
            point = point.tag(key, value);
        }

        point.build().unwrap()
    }

    /// Convert anomaly event to InfluxDB data point
    fn anomaly_to_point(&self, anomaly: &AnomalyEvent) -> DataPoint {
        DataPoint::builder("anomaly")
            .tag("service", anomaly.service_name.as_str())
            .tag("model", anomaly.model.as_str())
            .tag("severity", &anomaly.severity.to_string())
            .tag("type", &anomaly.anomaly_type.to_string())
            .tag("method", &anomaly.detection_method.to_string())
            .field("confidence", anomaly.confidence)
            .field("metric", anomaly.details.metric.as_str())
            .field("value", anomaly.details.value)
            .field("baseline", anomaly.details.baseline)
            .field("threshold", anomaly.details.threshold)
            .timestamp(anomaly.timestamp.timestamp_nanos_opt().unwrap_or(0))
            .build()
            .unwrap()
    }
}

#[async_trait]
impl Storage for InfluxDbStorage {
    async fn write_telemetry(&self, event: &TelemetryEvent) -> Result<()> {
        let point = self.telemetry_to_point(event);

        self.client
            .write(&self.config.telemetry_bucket, futures::stream::iter(vec![point]))
            .await
            .map_err(|e| Error::storage(format!("Failed to write telemetry: {}", e)))?;

        debug!(event_id = %event.event_id, "Wrote telemetry to InfluxDB");
        metrics::counter!("sentinel_storage_writes_total", "type" => "telemetry").increment(1);

        Ok(())
    }

    async fn write_anomaly(&self, anomaly: &AnomalyEvent) -> Result<()> {
        let point = self.anomaly_to_point(anomaly);

        self.client
            .write(&self.config.anomaly_bucket, futures::stream::iter(vec![point]))
            .await
            .map_err(|e| Error::storage(format!("Failed to write anomaly: {}", e)))?;

        debug!(alert_id = %anomaly.alert_id, "Wrote anomaly to InfluxDB");
        metrics::counter!("sentinel_storage_writes_total", "type" => "anomaly").increment(1);

        Ok(())
    }

    async fn write_telemetry_batch(&self, events: &[TelemetryEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let points: Vec<_> = events.iter().map(|e| self.telemetry_to_point(e)).collect();

        self.client
            .write(&self.config.telemetry_bucket, futures::stream::iter(points))
            .await
            .map_err(|e| Error::storage(format!("Failed to write telemetry batch: {}", e)))?;

        info!("Wrote {} telemetry events to InfluxDB", events.len());
        metrics::counter!("sentinel_storage_writes_total", "type" => "telemetry")
            .increment(events.len() as u64);

        Ok(())
    }

    async fn write_anomaly_batch(&self, anomalies: &[AnomalyEvent]) -> Result<()> {
        if anomalies.is_empty() {
            return Ok(());
        }

        let points: Vec<_> = anomalies.iter().map(|a| self.anomaly_to_point(a)).collect();

        self.client
            .write(&self.config.anomaly_bucket, futures::stream::iter(points))
            .await
            .map_err(|e| Error::storage(format!("Failed to write anomaly batch: {}", e)))?;

        info!("Wrote {} anomalies to InfluxDB", anomalies.len());
        metrics::counter!("sentinel_storage_writes_total", "type" => "anomaly")
            .increment(anomalies.len() as u64);

        Ok(())
    }

    async fn query_telemetry(&self, query: TelemetryQuery) -> Result<Vec<TelemetryEvent>> {
        // Build Flux query
        let mut flux = format!(
            r#"from(bucket: "{}")
              |> range(start: {}, stop: {})
              |> filter(fn: (r) => r._measurement == "telemetry")"#,
            self.config.telemetry_bucket,
            query.time_range.start.to_rfc3339(),
            query.time_range.end.to_rfc3339()
        );

        if let Some(ref service) = query.service {
            flux.push_str(&format!(
                r#" |> filter(fn: (r) => r.service == "{}")"#,
                service.as_str()
            ));
        }

        if let Some(ref model) = query.model {
            flux.push_str(&format!(
                r#" |> filter(fn: (r) => r.model == "{}")"#,
                model.as_str()
            ));
        }

        if let Some(limit) = query.limit {
            flux.push_str(&format!(" |> limit(n: {})", limit));
        }

        debug!("Executing InfluxDB query: {}", flux);

        // Execute query
        // Note: Full query implementation requires parsing InfluxDB response format
        // This is a simplified version - production would need full deserialization

        warn!("Query telemetry not fully implemented yet - returning empty results");

        Ok(Vec::new())
    }

    async fn query_anomalies(&self, query: AnomalyQuery) -> Result<Vec<AnomalyEvent>> {
        let mut flux = format!(
            r#"from(bucket: "{}")
              |> range(start: {}, stop: {})
              |> filter(fn: (r) => r._measurement == "anomaly")"#,
            self.config.anomaly_bucket,
            query.time_range.start.to_rfc3339(),
            query.time_range.end.to_rfc3339()
        );

        if let Some(ref service) = query.service {
            flux.push_str(&format!(
                r#" |> filter(fn: (r) => r.service == "{}")"#,
                service.as_str()
            ));
        }

        if let Some(ref severity) = query.severity {
            flux.push_str(&format!(
                r#" |> filter(fn: (r) => r.severity == "{}")"#,
                severity.to_string()
            ));
        }

        if let Some(limit) = query.limit {
            flux.push_str(&format!(" |> limit(n: {})", limit));
        }

        debug!("Executing InfluxDB query: {}", flux);

        warn!("Query anomalies not fully implemented yet - returning empty results");

        Ok(Vec::new())
    }

    async fn health_check(&self) -> Result<()> {
        self.client
            .health()
            .await
            .map_err(|e| Error::connection(format!("InfluxDB health check failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        events::{PromptInfo, ResponseInfo},
        types::{ModelId, ServiceId},
    };

    fn create_test_config() -> InfluxDbConfig {
        InfluxDbConfig {
            url: "http://localhost:8086".to_string(),
            org: "test".to_string(),
            telemetry_bucket: "test-telemetry".to_string(),
            anomaly_bucket: "test-anomalies".to_string(),
            token: "test-token".to_string(),
            batch_size: 100,
            timeout_secs: 10,
        }
    }

    fn create_test_event() -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test"),
            ModelId::new("gpt-4"),
            PromptInfo {
                text: "test".to_string(),
                tokens: 10,
                embedding: None,
            },
            ResponseInfo {
                text: "response".to_string(),
                tokens: 20,
                finish_reason: "stop".to_string(),
                embedding: None,
            },
            100.0,
            0.01,
        )
    }

    #[test]
    fn test_config_creation() {
        let config = create_test_config();
        assert_eq!(config.org, "test");
        assert_eq!(config.batch_size, 100);
    }

    #[test]
    fn test_telemetry_to_point() {
        let config = create_test_config();
        let storage = InfluxDbStorage {
            client: Client::new(&config.url, &config.org, &config.token),
            config,
        };

        let event = create_test_event();
        let point = storage.telemetry_to_point(&event);

        // Point is created successfully (actual write would require running InfluxDB)
        assert!(point.name == "telemetry");
    }
}
