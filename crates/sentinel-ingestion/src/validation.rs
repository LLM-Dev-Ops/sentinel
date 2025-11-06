//! Event validation and sanitization.

use llm_sentinel_core::{events::TelemetryEvent, Error, Result};
use tracing::{debug, warn};
use validator::Validate;

/// Event validator
#[derive(Debug, Clone)]
pub struct EventValidator {
    /// Minimum latency threshold (ms)
    min_latency_ms: f64,
    /// Maximum latency threshold (ms)
    max_latency_ms: f64,
    /// Maximum token count
    max_tokens: u32,
    /// Maximum cost (USD)
    max_cost_usd: f64,
}

impl Default for EventValidator {
    fn default() -> Self {
        Self {
            min_latency_ms: 0.0,
            max_latency_ms: 600_000.0, // 10 minutes
            max_tokens: 128_000,       // Max context length for most models
            max_cost_usd: 100.0,       // Sanity check for per-request cost
        }
    }
}

impl EventValidator {
    /// Create a new event validator
    pub fn new(
        min_latency_ms: f64,
        max_latency_ms: f64,
        max_tokens: u32,
        max_cost_usd: f64,
    ) -> Self {
        Self {
            min_latency_ms,
            max_latency_ms,
            max_tokens,
            max_cost_usd,
        }
    }

    /// Validate a telemetry event
    pub fn validate(&self, event: &TelemetryEvent) -> Result<()> {
        // Run struct-level validation first
        event
            .validate()
            .map_err(|e| Error::validation(format!("Event validation failed: {}", e)))?;

        // Validate latency range
        if event.latency_ms < self.min_latency_ms {
            warn!(
                event_id = %event.event_id,
                latency = event.latency_ms,
                "Latency below minimum threshold"
            );
            return Err(Error::validation(format!(
                "Latency {} ms is below minimum {} ms",
                event.latency_ms, self.min_latency_ms
            )));
        }

        if event.latency_ms > self.max_latency_ms {
            warn!(
                event_id = %event.event_id,
                latency = event.latency_ms,
                "Latency above maximum threshold"
            );
            return Err(Error::validation(format!(
                "Latency {} ms exceeds maximum {} ms",
                event.latency_ms, self.max_latency_ms
            )));
        }

        // Validate token counts
        let total_tokens = event.total_tokens();
        if total_tokens > self.max_tokens {
            warn!(
                event_id = %event.event_id,
                tokens = total_tokens,
                "Token count exceeds maximum"
            );
            return Err(Error::validation(format!(
                "Total tokens {} exceeds maximum {}",
                total_tokens, self.max_tokens
            )));
        }

        // Validate cost
        if event.cost_usd > self.max_cost_usd {
            warn!(
                event_id = %event.event_id,
                cost = event.cost_usd,
                "Cost exceeds maximum threshold"
            );
            return Err(Error::validation(format!(
                "Cost ${} exceeds maximum ${}",
                event.cost_usd, self.max_cost_usd
            )));
        }

        // Validate consistency
        if event.cost_usd < 0.0 {
            return Err(Error::validation("Cost cannot be negative".to_string()));
        }

        debug!(
            event_id = %event.event_id,
            service = %event.service_name,
            "Event validated successfully"
        );

        Ok(())
    }

    /// Sanitize an event (remove PII, truncate, etc.)
    pub fn sanitize(&self, event: &mut TelemetryEvent) -> Result<()> {
        // Check for potential PII patterns in prompt/response text
        if self.contains_pii(&event.prompt.text) {
            warn!(
                event_id = %event.event_id,
                "Potential PII detected in prompt, masking"
            );
            event.prompt.text = self.mask_pii(&event.prompt.text);
        }

        if self.contains_pii(&event.response.text) {
            warn!(
                event_id = %event.event_id,
                "Potential PII detected in response, masking"
            );
            event.response.text = self.mask_pii(&event.response.text);
        }

        // Remove sensitive metadata
        event.metadata.remove("api_key");
        event.metadata.remove("password");
        event.metadata.remove("secret");

        debug!(
            event_id = %event.event_id,
            "Event sanitized"
        );

        Ok(())
    }

    /// Check if text contains potential PII
    fn contains_pii(&self, text: &str) -> bool {
        // Simple pattern matching for common PII
        // In production, use more sophisticated methods

        // Email pattern
        if text.contains('@') && text.contains('.') {
            return true;
        }

        // Credit card pattern (sequences of 13-19 digits)
        let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 13 {
            return true;
        }

        // SSN pattern (XXX-XX-XXXX)
        if text.contains("SSN") || text.contains("social security") {
            return true;
        }

        false
    }

    /// Mask PII in text
    fn mask_pii(&self, text: &str) -> String {
        // Simple masking - replace emails and numbers
        let mut masked = text.to_string();

        // Mask emails
        if let Some(at_pos) = masked.find('@') {
            if let Some(space_before) = masked[..at_pos].rfind(' ') {
                if let Some(space_after) = masked[at_pos..].find(' ') {
                    let email_start = space_before + 1;
                    let email_end = at_pos + space_after;
                    masked.replace_range(email_start..email_end, "[EMAIL_REDACTED]");
                }
            }
        }

        // Mask long number sequences
        let re_numbers = regex::Regex::new(r"\d{4,}").unwrap();
        masked = re_numbers
            .replace_all(&masked, "[NUMBER_REDACTED]")
            .to_string();

        masked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        events::{PromptInfo, ResponseInfo},
        types::{ModelId, ServiceId},
    };

    fn create_test_event() -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test"),
            ModelId::new("gpt-4"),
            PromptInfo {
                text: "Test prompt".to_string(),
                tokens: 10,
                embedding: None,
            },
            ResponseInfo {
                text: "Test response".to_string(),
                tokens: 20,
                finish_reason: "stop".to_string(),
                embedding: None,
            },
            100.0,
            0.001,
        )
    }

    #[test]
    fn test_validator_creation() {
        let validator = EventValidator::default();
        assert_eq!(validator.min_latency_ms, 0.0);
        assert_eq!(validator.max_latency_ms, 600_000.0);
    }

    #[test]
    fn test_valid_event() {
        let validator = EventValidator::default();
        let event = create_test_event();
        assert!(validator.validate(&event).is_ok());
    }

    #[test]
    fn test_latency_too_high() {
        let validator = EventValidator::default();
        let mut event = create_test_event();
        event.latency_ms = 700_000.0; // 11+ minutes
        assert!(validator.validate(&event).is_err());
    }

    #[test]
    fn test_latency_negative() {
        let validator = EventValidator::default();
        let mut event = create_test_event();
        event.latency_ms = -1.0;
        // Should fail validator validation
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_tokens_too_high() {
        let validator = EventValidator::default();
        let mut event = create_test_event();
        event.prompt.tokens = 100_000;
        event.response.tokens = 50_000; // Total 150k > 128k
        assert!(validator.validate(&event).is_err());
    }

    #[test]
    fn test_cost_too_high() {
        let validator = EventValidator::default();
        let mut event = create_test_event();
        event.cost_usd = 150.0;
        assert!(validator.validate(&event).is_err());
    }

    #[test]
    fn test_pii_detection() {
        let validator = EventValidator::default();

        assert!(validator.contains_pii("Contact me at john@example.com"));
        assert!(validator.contains_pii("My SSN is 123-45-6789"));
        assert!(validator.contains_pii("Card number: 1234567890123456"));
        assert!(!validator.contains_pii("This is a normal message"));
    }

    #[test]
    fn test_pii_masking() {
        let validator = EventValidator::default();

        let text = "My credit card is 1234567890123456";
        let masked = validator.mask_pii(text);
        assert!(masked.contains("[NUMBER_REDACTED]"));
        assert!(!masked.contains("1234567890123456"));
    }

    #[test]
    fn test_sanitize_event() {
        let validator = EventValidator::default();
        let mut event = create_test_event();

        // Add sensitive metadata
        event
            .metadata
            .insert("api_key".to_string(), "secret123".to_string());
        event
            .metadata
            .insert("user_id".to_string(), "user123".to_string());

        assert!(validator.sanitize(&mut event).is_ok());

        // API key should be removed
        assert!(!event.metadata.contains_key("api_key"));
        // User ID should remain
        assert!(event.metadata.contains_key("user_id"));
    }
}
