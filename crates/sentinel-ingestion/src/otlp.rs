//! OpenTelemetry Protocol (OTLP) parsing for telemetry events.

use llm_sentinel_core::{
    events::{PromptInfo, ResponseInfo, TelemetryEvent},
    types::{ModelId, ServiceId},
    Error, Result,
};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn};

/// OTLP parser for telemetry events
#[derive(Debug, Clone)]
pub struct OtlpParser {
    /// Maximum text length to store
    max_text_length: usize,
}

impl Default for OtlpParser {
    fn default() -> Self {
        Self {
            max_text_length: 10000,
        }
    }
}

impl OtlpParser {
    /// Create a new OTLP parser
    pub fn new(max_text_length: usize) -> Self {
        Self { max_text_length }
    }

    /// Parse OTLP span to telemetry event
    ///
    /// OTLP spans contain LLM request/response data as attributes
    pub fn parse_span(&self, span_data: &Value) -> Result<TelemetryEvent> {
        let attributes = span_data
            .get("attributes")
            .and_then(|v| v.as_object())
            .ok_or_else(|| Error::ingestion("Missing attributes in span"))?;

        // Extract service name
        let service_name = self
            .extract_string(attributes, "service.name")
            .unwrap_or_else(|| "unknown".to_string());

        // Extract model
        let model = self
            .extract_string(attributes, "llm.model")
            .ok_or_else(|| Error::ingestion("Missing llm.model attribute"))?;

        // Extract trace and span IDs
        let trace_id = self.extract_string(span_data, "trace_id");
        let span_id = self.extract_string(span_data, "span_id");

        // Extract prompt
        let prompt_text = self
            .extract_string(attributes, "llm.prompt")
            .ok_or_else(|| Error::ingestion("Missing llm.prompt attribute"))?;
        let prompt_tokens = self
            .extract_number(attributes, "llm.prompt.tokens")
            .unwrap_or(0) as u32;
        let prompt_embedding = self.extract_embedding(attributes, "llm.prompt.embedding");

        // Extract response
        let response_text = self
            .extract_string(attributes, "llm.response")
            .ok_or_else(|| Error::ingestion("Missing llm.response attribute"))?;
        let response_tokens = self
            .extract_number(attributes, "llm.response.tokens")
            .unwrap_or(0) as u32;
        let finish_reason = self
            .extract_string(attributes, "llm.response.finish_reason")
            .unwrap_or_else(|| "unknown".to_string());
        let response_embedding = self.extract_embedding(attributes, "llm.response.embedding");

        // Extract latency (from span duration or attribute)
        let latency_ms = self
            .extract_number(attributes, "llm.latency_ms")
            .or_else(|| {
                // Calculate from start/end time if available
                let start = span_data.get("start_time_unix_nano")?.as_i64()?;
                let end = span_data.get("end_time_unix_nano")?.as_i64()?;
                Some(((end - start) as f64) / 1_000_000.0) // Convert ns to ms
            })
            .unwrap_or(0.0);

        // Extract cost
        let cost_usd = self
            .extract_number(attributes, "llm.cost_usd")
            .unwrap_or(0.0);

        // Extract errors
        let errors = if let Some(status) = span_data.get("status") {
            if status.get("code").and_then(|v| v.as_i64()) != Some(0) {
                vec![status
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string()]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Extract metadata
        let mut metadata = HashMap::new();
        if let Some(user_id) = self.extract_string(attributes, "user.id") {
            metadata.insert("user_id".to_string(), user_id);
        }
        if let Some(api_key) = self.extract_string(attributes, "api.key") {
            metadata.insert("api_key".to_string(), api_key);
        }
        if let Some(region) = self.extract_string(attributes, "cloud.region") {
            metadata.insert("region".to_string(), region);
        }
        if let Some(version) = self.extract_string(attributes, "service.version") {
            metadata.insert("version".to_string(), version);
        }

        let mut event = TelemetryEvent::new(
            ServiceId::new(service_name),
            ModelId::new(model),
            PromptInfo {
                text: self.truncate_text(prompt_text),
                tokens: prompt_tokens,
                embedding: prompt_embedding,
            },
            ResponseInfo {
                text: self.truncate_text(response_text),
                tokens: response_tokens,
                finish_reason,
                embedding: response_embedding,
            },
            latency_ms,
            cost_usd,
        );

        event.trace_id = trace_id;
        event.span_id = span_id;
        event.metadata = metadata;
        event.errors = errors;

        debug!(
            event_id = %event.event_id,
            service = %event.service_name,
            model = %event.model,
            "Parsed OTLP span to telemetry event"
        );

        Ok(event)
    }

    /// Extract string value from attributes
    fn extract_string(&self, obj: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
        obj.get(key)?.as_str().map(|s| s.to_string())
    }

    /// Extract number value from attributes
    fn extract_number(&self, obj: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
        obj.get(key)?.as_f64()
    }

    /// Extract embedding vector from attributes
    fn extract_embedding(
        &self,
        obj: &serde_json::Map<String, Value>,
        key: &str,
    ) -> Option<Vec<f32>> {
        let array = obj.get(key)?.as_array()?;
        let embedding: Option<Vec<f32>> = array
            .iter()
            .map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        embedding
    }

    /// Truncate text to maximum length
    fn truncate_text(&self, text: String) -> String {
        if text.len() > self.max_text_length {
            warn!(
                "Truncating text from {} to {} chars",
                text.len(),
                self.max_text_length
            );
            let mut truncated = text.chars().take(self.max_text_length).collect::<String>();
            truncated.push_str("...[truncated]");
            truncated
        } else {
            text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_otlp_parser_creation() {
        let parser = OtlpParser::new(5000);
        assert_eq!(parser.max_text_length, 5000);

        let default_parser = OtlpParser::default();
        assert_eq!(default_parser.max_text_length, 10000);
    }

    #[test]
    fn test_parse_span_valid() {
        let parser = OtlpParser::default();
        let span = json!({
            "trace_id": "abc123",
            "span_id": "def456",
            "start_time_unix_nano": 1000000000000,
            "end_time_unix_nano": 1000100000000,
            "attributes": {
                "service.name": "test-service",
                "llm.model": "gpt-4",
                "llm.prompt": "Test prompt",
                "llm.prompt.tokens": 10,
                "llm.response": "Test response",
                "llm.response.tokens": 20,
                "llm.response.finish_reason": "stop",
                "llm.latency_ms": 100.0,
                "llm.cost_usd": 0.001,
                "user.id": "user-123"
            },
            "status": {
                "code": 0
            }
        });

        let result = parser.parse_span(&span);
        assert!(result.is_ok());

        let event = result.unwrap();
        assert_eq!(event.service_name.as_str(), "test-service");
        assert_eq!(event.model.as_str(), "gpt-4");
        assert_eq!(event.prompt.tokens, 10);
        assert_eq!(event.response.tokens, 20);
        assert_eq!(event.latency_ms, 100.0);
        assert_eq!(event.cost_usd, 0.001);
        assert!(!event.has_errors());
        assert_eq!(event.metadata.get("user_id").unwrap(), "user-123");
    }

    #[test]
    fn test_parse_span_with_error() {
        let parser = OtlpParser::default();
        let span = json!({
            "attributes": {
                "service.name": "test-service",
                "llm.model": "gpt-4",
                "llm.prompt": "Test",
                "llm.response": "Error occurred",
                "llm.latency_ms": 50.0,
                "llm.cost_usd": 0.0
            },
            "status": {
                "code": 1,
                "message": "API rate limit exceeded"
            }
        });

        let result = parser.parse_span(&span);
        assert!(result.is_ok());

        let event = result.unwrap();
        assert!(event.has_errors());
        assert_eq!(event.errors.len(), 1);
        assert_eq!(event.errors[0], "API rate limit exceeded");
    }

    #[test]
    fn test_parse_span_missing_required_field() {
        let parser = OtlpParser::default();
        let span = json!({
            "attributes": {
                "service.name": "test-service",
                // Missing llm.model
                "llm.prompt": "Test",
                "llm.response": "Response"
            }
        });

        let result = parser.parse_span(&span);
        assert!(result.is_err());
    }

    #[test]
    fn test_text_truncation() {
        let parser = OtlpParser::new(10);
        let long_text = "a".repeat(100);
        let truncated = parser.truncate_text(long_text);
        assert!(truncated.len() < 100);
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn test_embedding_extraction() {
        let parser = OtlpParser::default();
        let mut obj = serde_json::Map::new();
        obj.insert(
            "embedding".to_string(),
            json!([0.1, 0.2, 0.3, 0.4, 0.5]),
        );

        let embedding = parser.extract_embedding(&obj, "embedding");
        assert!(embedding.is_some());
        assert_eq!(embedding.unwrap().len(), 5);
    }
}
