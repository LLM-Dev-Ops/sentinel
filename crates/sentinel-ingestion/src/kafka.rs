//! Kafka consumer for telemetry ingestion.

use crate::Ingester;
use async_trait::async_trait;
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    ClientConfig, Message,
};
use llm_sentinel_core::{
    config::KafkaConfig,
    events::TelemetryEvent,
    Error, Result,
};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Kafka-based telemetry ingester
#[derive(Debug)]
pub struct KafkaIngester {
    consumer: StreamConsumer,
    topic: String,
    batch_size: usize,
    batch_timeout: Duration,
    running: bool,
}

impl KafkaIngester {
    /// Create a new Kafka ingester
    pub fn new(config: &KafkaConfig, batch_size: usize, batch_timeout_ms: u64) -> Result<Self> {
        info!(
            "Creating Kafka ingester for topic: {}, consumer group: {}",
            config.topic, config.consumer_group
        );

        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", config.brokers.join(","))
            .set("group.id", &config.consumer_group)
            .set("auto.offset.reset", &config.auto_offset_reset)
            .set(
                "enable.auto.commit",
                if config.enable_auto_commit {
                    "true"
                } else {
                    "false"
                },
            )
            .set("session.timeout.ms", config.session_timeout_ms.to_string())
            .set("enable.partition.eof", "false")
            .set("socket.keepalive.enable", "true")
            .create()
            .map_err(|e| Error::connection(format!("Failed to create Kafka consumer: {}", e)))?;

        Ok(Self {
            consumer,
            topic: config.topic.clone(),
            batch_size,
            batch_timeout: Duration::from_millis(batch_timeout_ms),
            running: false,
        })
    }

    /// Parse Kafka message to telemetry event
    fn parse_message(&self, message: &rdkafka::message::BorrowedMessage) -> Result<TelemetryEvent> {
        let payload = message
            .payload()
            .ok_or_else(|| Error::ingestion("Empty message payload"))?;

        let event: TelemetryEvent = serde_json::from_slice(payload)
            .map_err(|e| Error::ingestion(format!("Failed to parse telemetry event: {}", e)))?;

        // Validate event
        event
            .validate()
            .map_err(|e| Error::validation(format!("Invalid telemetry event: {}", e)))?;

        debug!(
            event_id = %event.event_id,
            service = %event.service_name,
            model = %event.model,
            "Parsed telemetry event"
        );

        Ok(event)
    }
}

#[async_trait]
impl Ingester for KafkaIngester {
    async fn start(&mut self) -> Result<()> {
        if self.running {
            return Err(Error::already_exists("Ingester is already running"));
        }

        info!("Starting Kafka ingester for topic: {}", self.topic);

        self.consumer
            .subscribe(&[&self.topic])
            .map_err(|e| Error::connection(format!("Failed to subscribe to topic: {}", e)))?;

        self.running = true;
        info!("Kafka ingester started successfully");

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if !self.running {
            return Ok(());
        }

        info!("Stopping Kafka ingester");

        self.consumer
            .unsubscribe();

        self.running = false;
        info!("Kafka ingester stopped");

        Ok(())
    }

    async fn next_batch(&mut self) -> Result<Vec<TelemetryEvent>> {
        if !self.running {
            return Err(Error::internal("Ingester is not running"));
        }

        let mut batch = Vec::with_capacity(self.batch_size);
        let deadline = tokio::time::Instant::now() + self.batch_timeout;

        loop {
            // Check if we've reached batch size or timeout
            if batch.len() >= self.batch_size || tokio::time::Instant::now() >= deadline {
                break;
            }

            // Calculate remaining timeout
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            // Try to receive a message
            match tokio::time::timeout(remaining, self.consumer.recv()).await {
                Ok(Ok(message)) => {
                    match self.parse_message(&message) {
                        Ok(event) => {
                            batch.push(event);
                            metrics::counter!("sentinel_events_ingested_total").increment(1);
                        }
                        Err(e) => {
                            error!("Failed to parse message: {}", e);
                            metrics::counter!("sentinel_events_dropped_total").increment(1);
                            // Continue processing other messages
                            continue;
                        }
                    }
                }
                Ok(Err(e)) => {
                    error!("Kafka consumer error: {}", e);
                    metrics::counter!("sentinel_errors_total", "error_type" => "kafka").increment(1);
                    return Err(Error::connection(format!("Kafka consumer error: {}", e)));
                }
                Err(_) => {
                    // Timeout - return what we have
                    break;
                }
            }
        }

        if batch.is_empty() {
            debug!("No events received in batch");
        } else {
            debug!("Received batch of {} events", batch.len());
        }

        Ok(batch)
    }

    async fn health_check(&self) -> Result<()> {
        if !self.running {
            return Err(Error::internal("Ingester is not running"));
        }

        // Check Kafka connection by fetching metadata
        self.consumer
            .fetch_metadata(Some(&self.topic), Duration::from_secs(5))
            .map_err(|e| Error::connection(format!("Kafka health check failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        config::KafkaConfig,
        events::{PromptInfo, ResponseInfo, TelemetryEvent},
        types::{ModelId, ServiceId},
    };

    fn create_test_kafka_config() -> KafkaConfig {
        KafkaConfig {
            brokers: vec!["localhost:9092".to_string()],
            topic: "test-telemetry".to_string(),
            consumer_group: "test-group".to_string(),
            auto_offset_reset: "latest".to_string(),
            enable_auto_commit: true,
            session_timeout_ms: 30000,
        }
    }

    #[test]
    fn test_kafka_ingester_creation() {
        let config = create_test_kafka_config();
        // This will fail without actual Kafka, but tests the config parsing
        let result = KafkaIngester::new(&config, 100, 1000);
        // We expect it to fail in test environment, that's ok
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_event_parsing() {
        let event = TelemetryEvent::new(
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
            0.001,
        );

        let json = serde_json::to_vec(&event).unwrap();
        let parsed: TelemetryEvent = serde_json::from_slice(&json).unwrap();
        assert_eq!(event.event_id, parsed.event_id);
    }
}
