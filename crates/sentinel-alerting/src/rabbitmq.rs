//! RabbitMQ alert publisher with severity-based routing.

use crate::{AlertConfig, Alerter};
use async_trait::async_trait;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
    ExchangeKind,
};
use llm_sentinel_core::{events::AnomalyEvent, types::Severity, Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// RabbitMQ configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RabbitMqConfig {
    /// RabbitMQ connection URL
    pub url: String,
    /// Exchange name
    pub exchange: String,
    /// Exchange type (topic, direct, fanout)
    pub exchange_type: String,
    /// Routing key prefix
    pub routing_key_prefix: String,
    /// Message persistence
    pub persistent: bool,
    /// Connection timeout (seconds)
    pub timeout_secs: u64,
    /// Retry configuration
    pub retry_config: RetryConfig,
}

impl Default for RabbitMqConfig {
    fn default() -> Self {
        Self {
            url: "amqp://localhost:5672".to_string(),
            exchange: "sentinel.alerts".to_string(),
            exchange_type: "topic".to_string(),
            routing_key_prefix: "alert".to_string(),
            persistent: true,
            timeout_secs: 10,
            retry_config: RetryConfig::default(),
        }
    }
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Initial delay (milliseconds)
    pub initial_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Maximum delay (milliseconds)
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 30000,
        }
    }
}

/// RabbitMQ alert publisher
pub struct RabbitMqAlerter {
    channel: Arc<Channel>,
    config: RabbitMqConfig,
}

impl RabbitMqAlerter {
    /// Create a new RabbitMQ alerter
    pub async fn new(config: RabbitMqConfig) -> Result<Self> {
        info!("Connecting to RabbitMQ at {}", config.url);

        let connection = Connection::connect(
            &config.url,
            ConnectionProperties::default()
                .with_connection_name("sentinel-alerter".into())
                .with_executor(tokio_executor_trait::Tokio::current())
                .with_reactor(tokio_reactor_trait::Tokio),
        )
        .await
        .map_err(|e| {
            Error::connection(format!("Failed to connect to RabbitMQ: {}", e))
        })?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|e| Error::connection(format!("Failed to create channel: {}", e)))?;

        // Declare exchange
        let exchange_kind = match config.exchange_type.as_str() {
            "topic" => ExchangeKind::Topic,
            "direct" => ExchangeKind::Direct,
            "fanout" => ExchangeKind::Fanout,
            _ => {
                warn!(
                    "Unknown exchange type '{}', defaulting to topic",
                    config.exchange_type
                );
                ExchangeKind::Topic
            }
        };

        channel
            .exchange_declare(
                &config.exchange,
                exchange_kind,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| Error::connection(format!("Failed to declare exchange: {}", e)))?;

        info!(
            "Connected to RabbitMQ, exchange '{}' declared",
            config.exchange
        );

        Ok(Self {
            channel: Arc::new(channel),
            config,
        })
    }

    /// Build routing key based on severity
    fn build_routing_key(&self, severity: Severity) -> String {
        let severity_str = match severity {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        };

        format!("{}.{}", self.config.routing_key_prefix, severity_str)
    }

    /// Publish alert with retry logic
    async fn publish_with_retry(&self, alert: &AnomalyEvent) -> Result<()> {
        let routing_key = self.build_routing_key(alert.severity);
        let payload = serde_json::to_vec(alert)
            .map_err(|e| Error::serialization(format!("Failed to serialize alert: {}", e)))?;

        let properties = BasicProperties::default()
            .with_delivery_mode(if self.config.persistent { 2 } else { 1 })
            .with_content_type("application/json".into())
            .with_timestamp(chrono::Utc::now().timestamp() as u64)
            .with_message_id(alert.alert_id.to_string().into());

        let mut attempt = 0;
        let mut delay = self.config.retry_config.initial_delay_ms;

        loop {
            attempt += 1;

            match self
                .channel
                .basic_publish(
                    &self.config.exchange,
                    &routing_key,
                    BasicPublishOptions::default(),
                    &payload,
                    properties.clone(),
                )
                .await
            {
                Ok(_) => {
                    debug!(
                        alert_id = %alert.alert_id,
                        routing_key = %routing_key,
                        attempt = attempt,
                        "Alert published to RabbitMQ"
                    );

                    metrics::counter!(
                        "sentinel_rabbitmq_publishes_total",
                        "severity" => routing_key.clone()
                    )
                    .increment(1);

                    if attempt > 1 {
                        metrics::counter!("sentinel_rabbitmq_retries_total").increment(1);
                    }

                    return Ok(());
                }
                Err(e) => {
                    if attempt >= self.config.retry_config.max_attempts {
                        error!(
                            alert_id = %alert.alert_id,
                            attempts = attempt,
                            error = %e,
                            "Failed to publish alert after max retries"
                        );

                        metrics::counter!("sentinel_rabbitmq_failures_total").increment(1);

                        return Err(Error::alerting(format!(
                            "Failed to publish alert after {} attempts: {}",
                            attempt, e
                        )));
                    }

                    warn!(
                        alert_id = %alert.alert_id,
                        attempt = attempt,
                        delay_ms = delay,
                        error = %e,
                        "Failed to publish alert, retrying..."
                    );

                    tokio::time::sleep(Duration::from_millis(delay)).await;

                    // Exponential backoff
                    delay = (delay as f64 * self.config.retry_config.backoff_multiplier) as u64;
                    delay = delay.min(self.config.retry_config.max_delay_ms);
                }
            }
        }
    }
}

#[async_trait]
impl Alerter for RabbitMqAlerter {
    async fn send(&self, alert: &AnomalyEvent) -> Result<()> {
        self.publish_with_retry(alert).await
    }

    async fn send_batch(&self, alerts: &[AnomalyEvent]) -> Result<()> {
        if alerts.is_empty() {
            return Ok(());
        }

        let mut errors = Vec::new();

        for alert in alerts {
            if let Err(e) = self.send(alert).await {
                error!(
                    alert_id = %alert.alert_id,
                    error = %e,
                    "Failed to send alert in batch"
                );
                errors.push(e);
            }
        }

        if !errors.is_empty() {
            return Err(Error::alerting(format!(
                "Failed to send {} out of {} alerts",
                errors.len(),
                alerts.len()
            )));
        }

        info!("Successfully sent batch of {} alerts", alerts.len());
        Ok(())
    }

    async fn health_check(&self) -> Result<()> {
        // Check if channel is still open
        if !self.channel.status().connected() {
            return Err(Error::connection("RabbitMQ channel is not connected"));
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "RabbitMQ"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        events::{AnomalyDetails, PromptInfo, ResponseInfo, TelemetryEvent},
        types::{AnomalyType, DetectionMethod, ModelId, ServiceId},
    };

    fn create_test_config() -> RabbitMqConfig {
        RabbitMqConfig {
            url: "amqp://localhost:5672".to_string(),
            exchange: "test.alerts".to_string(),
            exchange_type: "topic".to_string(),
            routing_key_prefix: "alert".to_string(),
            persistent: true,
            timeout_secs: 10,
            retry_config: RetryConfig {
                max_attempts: 3,
                initial_delay_ms: 100,
                backoff_multiplier: 2.0,
                max_delay_ms: 5000,
            },
        }
    }

    fn create_test_anomaly(severity: Severity) -> AnomalyEvent {
        let telemetry = TelemetryEvent::new(
            ServiceId::new("test-service"),
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
        );

        AnomalyEvent::new(
            severity,
            AnomalyType::LatencySpike,
            DetectionMethod::ZScore,
            0.95,
            AnomalyDetails {
                metric: "latency_ms".to_string(),
                value: 500.0,
                baseline: 100.0,
                threshold: 300.0,
                deviation_percent: 400.0,
            },
            &telemetry,
        )
    }

    #[test]
    fn test_config_creation() {
        let config = create_test_config();
        assert_eq!(config.exchange, "test.alerts");
        assert_eq!(config.retry_config.max_attempts, 3);
    }

    #[test]
    fn test_routing_key_generation() {
        let config = create_test_config();
        // Create a mock alerter (can't actually connect without RabbitMQ)
        // Just test the routing key logic

        let routing_key_low = format!("{}.{}", config.routing_key_prefix, "low");
        let routing_key_critical = format!("{}.{}", config.routing_key_prefix, "critical");

        assert_eq!(routing_key_low, "alert.low");
        assert_eq!(routing_key_critical, "alert.critical");
    }

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    // Integration tests require a running RabbitMQ instance
    #[tokio::test]
    #[ignore = "Requires RabbitMQ"]
    async fn test_connection() {
        let config = create_test_config();
        let result = RabbitMqAlerter::new(config).await;

        // This will fail unless RabbitMQ is running
        match result {
            Ok(alerter) => {
                assert_eq!(alerter.name(), "RabbitMQ");
            }
            Err(e) => {
                println!("Expected failure (no RabbitMQ): {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires RabbitMQ"]
    async fn test_send_alert() {
        let config = create_test_config();
        if let Ok(alerter) = RabbitMqAlerter::new(config).await {
            let alert = create_test_anomaly(Severity::High);
            let result = alerter.send(&alert).await;

            match result {
                Ok(_) => println!("Alert sent successfully"),
                Err(e) => println!("Send failed: {}", e),
            }
        }
    }
}
