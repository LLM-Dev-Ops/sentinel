//! # LLM-Observatory Adapter
//!
//! Consumes telemetry streams, time-series traces, and structured logs
//! from LLM-Observatory.
//!
//! ## Data Consumed
//!
//! - **Telemetry Streams**: Real-time LLM request/response telemetry
//! - **Time-Series Traces**: Distributed traces with span context
//! - **Structured Logs**: Application logs with semantic attributes
//!
//! ## Integration Pattern
//!
//! Observatory data is consumed via OTLP protocol (gRPC/HTTP) and converted
//! to Sentinel's internal `TelemetryEvent` format. This adapter does NOT
//! modify any detection logic - it only provides a consumption layer.
//!
//! ## Note on Circular Dependencies
//!
//! LLM-Observatory depends on `llm-sentinel-core` for anomaly detection.
//! This adapter uses runtime protocol integration (OTLP) rather than
//! compile-time crate dependencies to avoid circular imports.

use crate::adapters::UpstreamAdapter;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use llm_sentinel_core::events::{PromptInfo, ResponseInfo, TelemetryEvent};
use llm_sentinel_core::types::{ModelId, ServiceId};
use llm_sentinel_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Configuration for Observatory adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservatoryConfig {
    /// OTLP endpoint for receiving telemetry (e.g., "http://localhost:4318")
    pub otlp_endpoint: String,

    /// Whether to use gRPC (true) or HTTP (false) protocol
    pub use_grpc: bool,

    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,

    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,

    /// Maximum batch size for telemetry events
    pub max_batch_size: usize,

    /// Enable TLS for connections
    pub tls_enabled: bool,
}

impl Default for ObservatoryConfig {
    fn default() -> Self {
        Self {
            otlp_endpoint: "http://localhost:4318".to_string(),
            use_grpc: false,
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            max_batch_size: 1000,
            tls_enabled: false,
        }
    }
}

/// Observatory span data (mirrors Observatory's LlmSpan structure)
///
/// This is a local representation of Observatory's span format,
/// designed for runtime deserialization without compile-time dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservatorySpan {
    /// Unique span identifier
    pub span_id: String,

    /// Trace identifier for distributed tracing
    pub trace_id: String,

    /// Parent span identifier (if nested)
    pub parent_span_id: Option<String>,

    /// Operation name
    pub name: String,

    /// LLM provider (openai, anthropic, google, etc.)
    pub provider: String,

    /// Model identifier
    pub model: String,

    /// Input data
    pub input: ObservatoryInput,

    /// Output data
    pub output: ObservatoryOutput,

    /// Token usage metrics
    pub token_usage: ObservatoryTokenUsage,

    /// Cost information
    pub cost: ObservatoryCost,

    /// Latency metrics
    pub latency: ObservatoryLatency,

    /// Span status
    pub status: ObservatoryStatus,

    /// Custom attributes
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

/// Observatory input types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ObservatoryInput {
    /// Plain text prompt
    Text(String),

    /// Chat messages
    Chat(Vec<ObservatoryChatMessage>),

    /// Multimodal content
    Multimodal(Vec<serde_json::Value>),
}

/// Chat message from Observatory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservatoryChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Observatory output structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservatoryOutput {
    pub content: String,
    pub finish_reason: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Token usage from Observatory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservatoryTokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Cost information from Observatory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservatoryCost {
    pub amount_usd: f64,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub prompt_cost: Option<f64>,
    #[serde(default)]
    pub completion_cost: Option<f64>,
}

fn default_currency() -> String {
    "USD".to_string()
}

/// Latency metrics from Observatory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservatoryLatency {
    pub total_ms: u64,
    #[serde(default)]
    pub ttft_ms: Option<u64>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

/// Span status from Observatory
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObservatoryStatus {
    Ok,
    Error,
    Unset,
}

/// Adapter for consuming telemetry from LLM-Observatory
#[derive(Debug)]
pub struct ObservatoryAdapter {
    config: ObservatoryConfig,
    connected: Arc<RwLock<bool>>,
    events_consumed: Arc<RwLock<u64>>,
}

impl ObservatoryAdapter {
    /// Create a new Observatory adapter
    pub fn new(config: ObservatoryConfig) -> Self {
        Self {
            config,
            connected: Arc::new(RwLock::new(false)),
            events_consumed: Arc::new(RwLock::new(0)),
        }
    }

    /// Convert Observatory span to Sentinel TelemetryEvent
    ///
    /// This is the core transformation logic. It maps Observatory's
    /// rich span data to Sentinel's telemetry format without modifying
    /// any detection logic.
    pub fn convert_span(&self, span: &ObservatorySpan) -> TelemetryEvent {
        // Extract prompt text from input
        let prompt_text = match &span.input {
            ObservatoryInput::Text(text) => text.clone(),
            ObservatoryInput::Chat(messages) => messages
                .iter()
                .map(|m| format!("[{}]: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n"),
            ObservatoryInput::Multimodal(_) => "[multimodal content]".to_string(),
        };

        // Build metadata from attributes
        let mut metadata: HashMap<String, String> = span
            .attributes
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect();

        metadata.insert("provider".to_string(), span.provider.clone());
        metadata.insert("span_id".to_string(), span.span_id.clone());

        if let Some(parent_id) = &span.parent_span_id {
            metadata.insert("parent_span_id".to_string(), parent_id.clone());
        }

        // Collect errors if status is Error
        let errors = match span.status {
            ObservatoryStatus::Error => vec!["Span completed with error status".to_string()],
            _ => Vec::new(),
        };

        TelemetryEvent {
            event_id: Uuid::new_v4(),
            timestamp: span.latency.end_time,
            service_name: ServiceId::new(&span.name),
            trace_id: Some(span.trace_id.clone()),
            span_id: Some(span.span_id.clone()),
            model: ModelId::new(&span.model),
            prompt: PromptInfo {
                text: prompt_text,
                tokens: span.token_usage.prompt_tokens,
                embedding: None,
            },
            response: ResponseInfo {
                text: span.output.content.clone(),
                tokens: span.token_usage.completion_tokens,
                finish_reason: span.output.finish_reason.clone(),
                embedding: None,
            },
            latency_ms: span.latency.total_ms as f64,
            cost_usd: span.cost.amount_usd,
            metadata,
            errors,
        }
    }

    /// Consume a batch of spans from Observatory
    ///
    /// This method would be called by the ingestion pipeline to
    /// fetch telemetry data from Observatory's OTLP endpoint.
    pub async fn consume_batch(&self, spans: Vec<ObservatorySpan>) -> Result<Vec<TelemetryEvent>> {
        let events: Vec<TelemetryEvent> = spans.iter().map(|s| self.convert_span(s)).collect();

        // Update metrics
        let mut count = self.events_consumed.write().await;
        *count += events.len() as u64;

        debug!(
            "Converted {} Observatory spans to TelemetryEvents",
            events.len()
        );

        Ok(events)
    }

    /// Get the number of events consumed
    pub async fn events_consumed(&self) -> u64 {
        *self.events_consumed.read().await
    }
}

#[async_trait]
impl UpstreamAdapter for ObservatoryAdapter {
    fn name(&self) -> &'static str {
        "llm-observatory"
    }

    async fn health_check(&self) -> Result<()> {
        let connected = *self.connected.read().await;
        if connected {
            Ok(())
        } else {
            Err(llm_sentinel_core::Error::connection(
                "Observatory adapter not connected",
            ))
        }
    }

    async fn connect(&mut self) -> Result<()> {
        info!(
            "Connecting to Observatory at {}",
            self.config.otlp_endpoint
        );

        // In production, this would establish OTLP connection
        // For now, we mark as connected for structural validation
        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Observatory adapter connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Observatory");

        let mut connected = self.connected.write().await;
        *connected = false;

        info!(
            "Observatory adapter disconnected. Total events consumed: {}",
            *self.events_consumed.read().await
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_span() -> ObservatorySpan {
        ObservatorySpan {
            span_id: "span-123".to_string(),
            trace_id: "trace-456".to_string(),
            parent_span_id: None,
            name: "chat-service".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            input: ObservatoryInput::Text("Hello, world!".to_string()),
            output: ObservatoryOutput {
                content: "Hi there!".to_string(),
                finish_reason: "stop".to_string(),
                metadata: HashMap::new(),
            },
            token_usage: ObservatoryTokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            cost: ObservatoryCost {
                amount_usd: 0.001,
                currency: "USD".to_string(),
                prompt_cost: Some(0.0006),
                completion_cost: Some(0.0004),
            },
            latency: ObservatoryLatency {
                total_ms: 250,
                ttft_ms: Some(50),
                start_time: Utc::now(),
                end_time: Utc::now(),
            },
            status: ObservatoryStatus::Ok,
            attributes: HashMap::new(),
        }
    }

    #[test]
    fn test_span_conversion() {
        let adapter = ObservatoryAdapter::new(ObservatoryConfig::default());
        let span = create_test_span();

        let event = adapter.convert_span(&span);

        assert_eq!(event.model.as_str(), "gpt-4");
        assert_eq!(event.prompt.tokens, 10);
        assert_eq!(event.response.tokens, 5);
        assert_eq!(event.latency_ms, 250.0);
        assert_eq!(event.cost_usd, 0.001);
        assert!(event.trace_id.is_some());
        assert!(event.errors.is_empty());
    }

    #[test]
    fn test_chat_input_conversion() {
        let adapter = ObservatoryAdapter::new(ObservatoryConfig::default());
        let mut span = create_test_span();
        span.input = ObservatoryInput::Chat(vec![
            ObservatoryChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
                name: None,
            },
            ObservatoryChatMessage {
                role: "assistant".to_string(),
                content: "Hi!".to_string(),
                name: None,
            },
        ]);

        let event = adapter.convert_span(&span);

        assert!(event.prompt.text.contains("[user]: Hello"));
        assert!(event.prompt.text.contains("[assistant]: Hi!"));
    }

    #[test]
    fn test_error_status_conversion() {
        let adapter = ObservatoryAdapter::new(ObservatoryConfig::default());
        let mut span = create_test_span();
        span.status = ObservatoryStatus::Error;

        let event = adapter.convert_span(&span);

        assert!(!event.errors.is_empty());
        assert!(event.errors[0].contains("error status"));
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        let mut adapter = ObservatoryAdapter::new(ObservatoryConfig::default());

        // Initially not connected
        assert!(adapter.health_check().await.is_err());

        // Connect
        adapter.connect().await.unwrap();
        assert!(adapter.health_check().await.is_ok());

        // Disconnect
        adapter.disconnect().await.unwrap();
        assert!(adapter.health_check().await.is_err());
    }

    #[tokio::test]
    async fn test_batch_consumption() {
        let adapter = ObservatoryAdapter::new(ObservatoryConfig::default());
        let spans = vec![create_test_span(), create_test_span(), create_test_span()];

        let events = adapter.consume_batch(spans).await.unwrap();

        assert_eq!(events.len(), 3);
        assert_eq!(adapter.events_consumed().await, 3);
    }
}
