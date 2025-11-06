//! Event type definitions for telemetry and anomalies.
//!
//! This module defines the core event structures used throughout Sentinel:
//! - TelemetryEvent: Incoming telemetry from LLM applications
//! - AnomalyEvent: Detected anomalies
//! - AlertEvent: Alerts sent to incident manager

use crate::types::{AnomalyType, DetectionMethod, ModelId, ServiceId, Severity};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;

/// Telemetry event from LLM Observatory
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TelemetryEvent {
    /// Unique event identifier
    pub event_id: Uuid,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Service name
    pub service_name: ServiceId,

    /// Trace ID for distributed tracing
    pub trace_id: Option<String>,

    /// Span ID
    pub span_id: Option<String>,

    /// Model identifier (e.g., "gpt-4", "claude-3")
    pub model: ModelId,

    /// Prompt information
    pub prompt: PromptInfo,

    /// Response information
    pub response: ResponseInfo,

    /// Request latency in milliseconds
    #[validate(range(min = 0.0))]
    pub latency_ms: f64,

    /// Cost in USD
    #[validate(range(min = 0.0))]
    pub cost_usd: f64,

    /// Additional metadata
    pub metadata: HashMap<String, String>,

    /// Errors if any
    pub errors: Vec<String>,
}

/// Prompt information
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PromptInfo {
    /// Prompt text (may be truncated for storage)
    #[validate(length(max = 100000))]
    pub text: String,

    /// Token count
    #[validate(range(min = 0))]
    pub tokens: u32,

    /// Optional embedding vector
    pub embedding: Option<Vec<f32>>,
}

/// Response information
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ResponseInfo {
    /// Response text (may be truncated for storage)
    #[validate(length(max = 100000))]
    pub text: String,

    /// Token count
    #[validate(range(min = 0))]
    pub tokens: u32,

    /// Finish reason (e.g., "stop", "length", "content_filter")
    pub finish_reason: String,

    /// Optional embedding vector
    pub embedding: Option<Vec<f32>>,
}

/// Anomaly event detected by Sentinel
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AnomalyEvent {
    /// Unique alert identifier
    pub alert_id: Uuid,

    /// Detection timestamp
    pub timestamp: DateTime<Utc>,

    /// Severity level
    pub severity: Severity,

    /// Type of anomaly
    pub anomaly_type: AnomalyType,

    /// Service name
    pub service_name: ServiceId,

    /// Model identifier
    pub model: ModelId,

    /// Detection method used
    pub detection_method: DetectionMethod,

    /// Confidence score (0.0 - 1.0)
    #[validate(range(min = 0.0, max = 1.0))]
    pub confidence: f64,

    /// Detailed anomaly information
    pub details: AnomalyDetails,

    /// Context information
    pub context: AnomalyContext,

    /// Root cause analysis
    pub root_cause: Option<String>,

    /// Remediation suggestions
    pub remediation: Vec<String>,

    /// Related alert IDs
    pub related_alerts: Vec<Uuid>,

    /// Runbook URL
    pub runbook_url: Option<String>,
}

/// Detailed anomaly information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetails {
    /// Metric name
    pub metric: String,

    /// Observed value
    pub value: f64,

    /// Baseline/expected value
    pub baseline: f64,

    /// Threshold that was exceeded
    pub threshold: f64,

    /// Standard deviations from baseline (if applicable)
    pub deviation_sigma: Option<f64>,

    /// Additional details
    pub additional: HashMap<String, serde_json::Value>,
}

/// Context information for anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyContext {
    /// Trace ID if available
    pub trace_id: Option<String>,

    /// User ID if available
    pub user_id: Option<String>,

    /// Region/datacenter
    pub region: Option<String>,

    /// Time window analyzed
    pub time_window: String,

    /// Number of samples in window
    pub sample_count: usize,

    /// Additional context
    pub additional: HashMap<String, String>,
}

/// Alert event sent to incident manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    /// Alert identifier (same as anomaly alert_id)
    pub alert_id: Uuid,

    /// Alert timestamp
    pub timestamp: DateTime<Utc>,

    /// Severity
    pub severity: Severity,

    /// Alert title
    pub title: String,

    /// Alert description
    pub description: String,

    /// Service affected
    pub service_name: ServiceId,

    /// Model affected
    pub model: ModelId,

    /// Alert tags
    pub tags: Vec<String>,

    /// Source anomaly event
    pub anomaly: AnomalyEvent,
}

impl TelemetryEvent {
    /// Create a new telemetry event
    pub fn new(
        service_name: ServiceId,
        model: ModelId,
        prompt: PromptInfo,
        response: ResponseInfo,
        latency_ms: f64,
        cost_usd: f64,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            service_name,
            trace_id: None,
            span_id: None,
            model,
            prompt,
            response,
            latency_ms,
            cost_usd,
            metadata: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Check if event has errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Calculate total tokens
    pub fn total_tokens(&self) -> u32 {
        self.prompt.tokens + self.response.tokens
    }

    /// Get error rate (0 or 1 for single event)
    pub fn error_rate(&self) -> f64 {
        if self.has_errors() {
            1.0
        } else {
            0.0
        }
    }
}

impl AnomalyEvent {
    /// Create a new anomaly event
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        severity: Severity,
        anomaly_type: AnomalyType,
        service_name: ServiceId,
        model: ModelId,
        detection_method: DetectionMethod,
        confidence: f64,
        details: AnomalyDetails,
        context: AnomalyContext,
    ) -> Self {
        Self {
            alert_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            severity,
            anomaly_type,
            service_name,
            model,
            detection_method,
            confidence,
            details,
            context,
            root_cause: None,
            remediation: Vec::new(),
            related_alerts: Vec::new(),
            runbook_url: None,
        }
    }

    /// Set root cause
    pub fn with_root_cause(mut self, root_cause: impl Into<String>) -> Self {
        self.root_cause = Some(root_cause.into());
        self
    }

    /// Add remediation suggestion
    pub fn with_remediation(mut self, suggestion: impl Into<String>) -> Self {
        self.remediation.push(suggestion.into());
        self
    }

    /// Add related alert
    pub fn with_related_alert(mut self, alert_id: Uuid) -> Self {
        self.related_alerts.push(alert_id);
        self
    }

    /// Set runbook URL
    pub fn with_runbook(mut self, url: impl Into<String>) -> Self {
        self.runbook_url = Some(url.into());
        self
    }
}

impl AlertEvent {
    /// Create alert from anomaly
    pub fn from_anomaly(anomaly: AnomalyEvent) -> Self {
        let title = format!(
            "{} detected in {} ({})",
            anomaly.anomaly_type, anomaly.service_name, anomaly.model
        );

        let description = format!(
            "Detected {} anomaly using {} method. Confidence: {:.2}%. Metric: {} = {:.2} (baseline: {:.2}, threshold: {:.2})",
            anomaly.anomaly_type,
            anomaly.detection_method,
            anomaly.confidence * 100.0,
            anomaly.details.metric,
            anomaly.details.value,
            anomaly.details.baseline,
            anomaly.details.threshold
        );

        let tags = vec![
            format!("severity:{}", anomaly.severity),
            format!("type:{}", anomaly.anomaly_type),
            format!("service:{}", anomaly.service_name),
            format!("model:{}", anomaly.model),
            format!("method:{}", anomaly.detection_method),
        ];

        Self {
            alert_id: anomaly.alert_id,
            timestamp: anomaly.timestamp,
            severity: anomaly.severity,
            title,
            description,
            service_name: anomaly.service_name.clone(),
            model: anomaly.model.clone(),
            tags,
            anomaly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_telemetry_event() -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test-service"),
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
            150.5,
            0.002,
        )
    }

    #[test]
    fn test_telemetry_event_creation() {
        let event = create_test_telemetry_event();
        assert_eq!(event.service_name.as_str(), "test-service");
        assert_eq!(event.model.as_str(), "gpt-4");
        assert_eq!(event.total_tokens(), 30);
        assert!(!event.has_errors());
    }

    #[test]
    fn test_telemetry_event_validation() {
        let event = create_test_telemetry_event();
        assert!(event.validate().is_ok());
    }

    #[test]
    fn test_anomaly_event_creation() {
        let anomaly = AnomalyEvent::new(
            Severity::High,
            AnomalyType::LatencySpike,
            ServiceId::new("test-service"),
            ModelId::new("gpt-4"),
            DetectionMethod::ZScore,
            0.95,
            AnomalyDetails {
                metric: "latency_ms".to_string(),
                value: 5000.0,
                baseline: 150.0,
                threshold: 500.0,
                deviation_sigma: Some(5.2),
                additional: HashMap::new(),
            },
            AnomalyContext {
                trace_id: None,
                user_id: None,
                region: Some("us-east-1".to_string()),
                time_window: "last_5_minutes".to_string(),
                sample_count: 1000,
                additional: HashMap::new(),
            },
        )
        .with_root_cause("Database query timeout")
        .with_remediation("Check database connection pool")
        .with_runbook("https://wiki.example.com/runbooks/latency");

        assert_eq!(anomaly.severity, Severity::High);
        assert_eq!(anomaly.confidence, 0.95);
        assert!(anomaly.root_cause.is_some());
        assert_eq!(anomaly.remediation.len(), 1);
        assert!(anomaly.runbook_url.is_some());
    }

    #[test]
    fn test_alert_event_from_anomaly() {
        let anomaly = AnomalyEvent::new(
            Severity::Critical,
            AnomalyType::LatencySpike,
            ServiceId::new("test-service"),
            ModelId::new("gpt-4"),
            DetectionMethod::ZScore,
            0.98,
            AnomalyDetails {
                metric: "latency_ms".to_string(),
                value: 10000.0,
                baseline: 150.0,
                threshold: 500.0,
                deviation_sigma: Some(8.5),
                additional: HashMap::new(),
            },
            AnomalyContext {
                trace_id: None,
                user_id: None,
                region: Some("us-east-1".to_string()),
                time_window: "last_5_minutes".to_string(),
                sample_count: 1000,
                additional: HashMap::new(),
            },
        );

        let alert = AlertEvent::from_anomaly(anomaly.clone());
        assert_eq!(alert.alert_id, anomaly.alert_id);
        assert_eq!(alert.severity, Severity::Critical);
        assert!(alert.title.contains("latency_spike"));
        assert!(alert.description.contains("98.00%"));
        assert_eq!(alert.tags.len(), 5);
    }

    #[test]
    fn test_telemetry_event_serialization() {
        let event = create_test_telemetry_event();
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: TelemetryEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event.event_id, deserialized.event_id);
        assert_eq!(event.service_name, deserialized.service_name);
    }
}
