//! # LLM-Shield Adapter
//!
//! Consumes security events, policy violations, and PII detection signals
//! from LLM-Shield.
//!
//! ## Data Consumed
//!
//! - **Security Events**: Threat detections, attack attempts
//! - **Policy Violations**: Rule breaches, compliance failures
//! - **PII Detection Signals**: Sensitive data exposure alerts
//!
//! ## Integration Pattern
//!
//! Shield data is consumed and converted to Sentinel's internal event format.
//! Security signals enrich anomaly detection context but do NOT modify
//! detection algorithms.

use crate::adapters::UpstreamAdapter;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use llm_sentinel_core::events::{AnomalyContext, AnomalyDetails, AnomalyEvent};
use llm_sentinel_core::types::{AnomalyType, DetectionMethod, ModelId, ServiceId, Severity};
use llm_sentinel_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Configuration for Shield adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldConfig {
    /// Shield API endpoint
    pub api_endpoint: String,

    /// Message queue topic for security events
    pub events_topic: String,

    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,

    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,

    /// Minimum severity to consume (filter out lower severities)
    pub min_severity: ShieldSeverity,
}

impl Default for ShieldConfig {
    fn default() -> Self {
        Self {
            api_endpoint: "http://localhost:8081".to_string(),
            events_topic: "security.events".to_string(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            min_severity: ShieldSeverity::Low,
        }
    }
}

/// Shield severity levels (mirrors Shield's Severity enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShieldSeverity {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl ShieldSeverity {
    /// Convert to Sentinel Severity
    pub fn to_sentinel_severity(&self) -> Severity {
        match self {
            ShieldSeverity::None | ShieldSeverity::Low => Severity::Low,
            ShieldSeverity::Medium => Severity::Medium,
            ShieldSeverity::High => Severity::High,
            ShieldSeverity::Critical => Severity::Critical,
        }
    }

    /// Get numeric threshold for filtering
    pub fn threshold(&self) -> f32 {
        match self {
            ShieldSeverity::None => 0.0,
            ShieldSeverity::Low => 0.01,
            ShieldSeverity::Medium => 0.4,
            ShieldSeverity::High => 0.7,
            ShieldSeverity::Critical => 0.9,
        }
    }
}

/// Shield scan result (mirrors Shield's ScanResult)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldScanResult {
    /// Processed/sanitized content
    pub sanitized_text: String,

    /// Safety determination (pass/fail)
    pub is_valid: bool,

    /// Numerical threat level (0.0-1.0)
    pub risk_score: f32,

    /// Detected sensitive entities
    pub entities: Vec<ShieldEntity>,

    /// Individual threat contributors
    pub risk_factors: Vec<ShieldRiskFactor>,

    /// Additional context
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ShieldScanResult {
    /// Derive severity from risk score
    pub fn severity(&self) -> ShieldSeverity {
        if self.risk_score >= 0.9 {
            ShieldSeverity::Critical
        } else if self.risk_score >= 0.7 {
            ShieldSeverity::High
        } else if self.risk_score >= 0.4 {
            ShieldSeverity::Medium
        } else if self.risk_score >= 0.01 {
            ShieldSeverity::Low
        } else {
            ShieldSeverity::None
        }
    }
}

/// Detected sensitive entity from Shield
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldEntity {
    /// Entity type (e.g., "email", "api_key", "credit_card", "ssn")
    pub entity_type: String,

    /// The detected content (may be redacted)
    pub text: String,

    /// Character position start
    pub start: usize,

    /// Character position end
    pub end: usize,

    /// Detection confidence (0.0-1.0)
    pub confidence: f32,

    /// Additional context
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Risk factor from Shield
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldRiskFactor {
    /// Factor type (e.g., "prompt_injection", "toxicity", "jailbreak")
    pub factor_type: String,

    /// Human-readable description
    pub description: String,

    /// Risk level classification
    pub severity: ShieldSeverity,

    /// Contribution to overall risk score
    pub score_contribution: f32,
}

/// Security event from Shield
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldSecurityEvent {
    /// Unique event identifier
    pub event_id: Uuid,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Service that generated the event
    pub service_name: String,

    /// Model being used
    pub model: String,

    /// Scan result details
    pub scan_result: ShieldScanResult,

    /// Scanner that produced the result
    pub scanner_name: String,

    /// Scan latency in milliseconds
    pub scan_latency_ms: u64,

    /// Trace ID for correlation
    pub trace_id: Option<String>,
}

/// Adapter for consuming security events from LLM-Shield
#[derive(Debug)]
pub struct ShieldAdapter {
    config: ShieldConfig,
    connected: Arc<RwLock<bool>>,
    events_consumed: Arc<RwLock<u64>>,
    violations_detected: Arc<RwLock<u64>>,
}

impl ShieldAdapter {
    /// Create a new Shield adapter
    pub fn new(config: ShieldConfig) -> Self {
        Self {
            config,
            connected: Arc::new(RwLock::new(false)),
            events_consumed: Arc::new(RwLock::new(0)),
            violations_detected: Arc::new(RwLock::new(0)),
        }
    }

    /// Convert Shield security event to Sentinel AnomalyEvent
    ///
    /// This conversion enriches Sentinel's anomaly detection with
    /// security context from Shield, without modifying detection logic.
    pub fn convert_to_anomaly(&self, event: &ShieldSecurityEvent) -> Option<AnomalyEvent> {
        let scan = &event.scan_result;

        // Only convert events that represent actual security concerns
        if scan.is_valid && scan.severity() < self.config.min_severity {
            return None;
        }

        // Determine anomaly type based on risk factors
        let anomaly_type = self.determine_anomaly_type(&scan.risk_factors);

        // Build details from scan result
        let mut additional: HashMap<String, serde_json::Value> = HashMap::new();
        additional.insert(
            "risk_score".to_string(),
            serde_json::json!(scan.risk_score),
        );
        additional.insert("is_valid".to_string(), serde_json::json!(scan.is_valid));
        additional.insert(
            "entity_count".to_string(),
            serde_json::json!(scan.entities.len()),
        );
        additional.insert(
            "risk_factor_count".to_string(),
            serde_json::json!(scan.risk_factors.len()),
        );
        additional.insert(
            "scanner".to_string(),
            serde_json::json!(event.scanner_name),
        );

        // Add entity type summary
        let entity_types: Vec<String> = scan.entities.iter().map(|e| e.entity_type.clone()).collect();
        additional.insert("entity_types".to_string(), serde_json::json!(entity_types));

        // Add risk factor summary
        let risk_factor_types: Vec<String> = scan
            .risk_factors
            .iter()
            .map(|r| r.factor_type.clone())
            .collect();
        additional.insert(
            "risk_factor_types".to_string(),
            serde_json::json!(risk_factor_types),
        );

        let details = AnomalyDetails {
            metric: "security_risk_score".to_string(),
            value: scan.risk_score as f64,
            baseline: 0.0, // Security events don't have baselines
            threshold: scan.severity().threshold() as f64,
            deviation_sigma: None,
            additional,
        };

        let mut context_additional: HashMap<String, String> = HashMap::new();
        context_additional.insert("scanner".to_string(), event.scanner_name.clone());
        context_additional.insert(
            "scan_latency_ms".to_string(),
            event.scan_latency_ms.to_string(),
        );

        let context = AnomalyContext {
            trace_id: event.trace_id.clone(),
            user_id: None,
            region: None,
            time_window: "instant".to_string(),
            sample_count: 1,
            additional: context_additional,
        };

        let mut anomaly = AnomalyEvent::new(
            scan.severity().to_sentinel_severity(),
            anomaly_type,
            ServiceId::new(&event.service_name),
            ModelId::new(&event.model),
            DetectionMethod::Custom("llm-shield".to_string()),
            scan.risk_score as f64,
            details,
            context,
        );

        // Add remediation suggestions based on risk factors
        for factor in &scan.risk_factors {
            anomaly = anomaly.with_remediation(format!(
                "Address {}: {}",
                factor.factor_type, factor.description
            ));
        }

        // Override alert_id to match Shield's event_id for correlation
        Some(AnomalyEvent {
            alert_id: event.event_id,
            ..anomaly
        })
    }

    /// Determine anomaly type from Shield risk factors
    fn determine_anomaly_type(&self, risk_factors: &[ShieldRiskFactor]) -> AnomalyType {
        // Find the highest-severity risk factor
        let primary_factor = risk_factors
            .iter()
            .max_by(|a, b| a.severity.cmp(&b.severity));

        match primary_factor {
            Some(factor) => match factor.factor_type.as_str() {
                "prompt_injection" | "jailbreak" => AnomalyType::SecurityThreat,
                "toxicity" | "bias" => AnomalyType::QualityDegradation,
                "pii" | "secret" => AnomalyType::SecurityThreat,
                _ => AnomalyType::Custom(format!("shield_{}", factor.factor_type)),
            },
            None => AnomalyType::SecurityThreat,
        }
    }

    /// Consume a batch of security events from Shield
    pub async fn consume_batch(
        &self,
        events: Vec<ShieldSecurityEvent>,
    ) -> Result<Vec<AnomalyEvent>> {
        let anomalies: Vec<AnomalyEvent> = events
            .iter()
            .filter_map(|e| self.convert_to_anomaly(e))
            .collect();

        // Update metrics
        let mut consumed = self.events_consumed.write().await;
        *consumed += events.len() as u64;

        let mut violations = self.violations_detected.write().await;
        *violations += anomalies.len() as u64;

        debug!(
            "Processed {} Shield events, generated {} anomalies",
            events.len(),
            anomalies.len()
        );

        Ok(anomalies)
    }

    /// Get statistics
    pub async fn stats(&self) -> ShieldAdapterStats {
        ShieldAdapterStats {
            events_consumed: *self.events_consumed.read().await,
            violations_detected: *self.violations_detected.read().await,
        }
    }
}

/// Statistics for Shield adapter
#[derive(Debug, Clone)]
pub struct ShieldAdapterStats {
    pub events_consumed: u64,
    pub violations_detected: u64,
}

#[async_trait]
impl UpstreamAdapter for ShieldAdapter {
    fn name(&self) -> &'static str {
        "llm-shield"
    }

    async fn health_check(&self) -> Result<()> {
        let connected = *self.connected.read().await;
        if connected {
            Ok(())
        } else {
            Err(llm_sentinel_core::Error::connection(
                "Shield adapter not connected",
            ))
        }
    }

    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to Shield at {}", self.config.api_endpoint);

        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Shield adapter connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Shield");

        let mut connected = self.connected.write().await;
        *connected = false;

        let stats = self.stats().await;
        info!(
            "Shield adapter disconnected. Events consumed: {}, Violations detected: {}",
            stats.events_consumed, stats.violations_detected
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_scan_result(risk_score: f32, is_valid: bool) -> ShieldScanResult {
        ShieldScanResult {
            sanitized_text: "sanitized content".to_string(),
            is_valid,
            risk_score,
            entities: vec![ShieldEntity {
                entity_type: "email".to_string(),
                text: "[REDACTED]".to_string(),
                start: 0,
                end: 10,
                confidence: 0.95,
                metadata: HashMap::new(),
            }],
            risk_factors: vec![ShieldRiskFactor {
                factor_type: "pii".to_string(),
                description: "Email address detected".to_string(),
                severity: ShieldSeverity::Medium,
                score_contribution: 0.4,
            }],
            metadata: HashMap::new(),
        }
    }

    fn create_test_security_event(risk_score: f32) -> ShieldSecurityEvent {
        ShieldSecurityEvent {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            service_name: "test-service".to_string(),
            model: "gpt-4".to_string(),
            scan_result: create_test_scan_result(risk_score, false),
            scanner_name: "pii-scanner".to_string(),
            scan_latency_ms: 25,
            trace_id: Some("trace-123".to_string()),
        }
    }

    #[test]
    fn test_severity_conversion() {
        assert_eq!(
            ShieldSeverity::None.to_sentinel_severity(),
            Severity::Low
        );
        assert_eq!(
            ShieldSeverity::Low.to_sentinel_severity(),
            Severity::Low
        );
        assert_eq!(
            ShieldSeverity::Medium.to_sentinel_severity(),
            Severity::Medium
        );
        assert_eq!(
            ShieldSeverity::High.to_sentinel_severity(),
            Severity::High
        );
        assert_eq!(
            ShieldSeverity::Critical.to_sentinel_severity(),
            Severity::Critical
        );
    }

    #[test]
    fn test_scan_result_severity() {
        let low = create_test_scan_result(0.2, false);
        assert_eq!(low.severity(), ShieldSeverity::Low);

        let medium = create_test_scan_result(0.5, false);
        assert_eq!(medium.severity(), ShieldSeverity::Medium);

        let high = create_test_scan_result(0.8, false);
        assert_eq!(high.severity(), ShieldSeverity::High);

        let critical = create_test_scan_result(0.95, false);
        assert_eq!(critical.severity(), ShieldSeverity::Critical);
    }

    #[test]
    fn test_security_event_conversion() {
        let adapter = ShieldAdapter::new(ShieldConfig::default());
        let event = create_test_security_event(0.75);

        let anomaly = adapter.convert_to_anomaly(&event);

        assert!(anomaly.is_some());
        let anomaly = anomaly.unwrap();

        assert_eq!(anomaly.severity, Severity::High);
        assert_eq!(anomaly.service_name.as_str(), "test-service");
        assert_eq!(anomaly.model.as_str(), "gpt-4");
        assert!(anomaly.context.trace_id.is_some());
    }

    #[test]
    fn test_low_risk_filtering() {
        let config = ShieldConfig {
            min_severity: ShieldSeverity::Medium,
            ..Default::default()
        };
        let adapter = ShieldAdapter::new(config);

        // Low risk event should be filtered out
        let mut event = create_test_security_event(0.1);
        event.scan_result.is_valid = true;

        let anomaly = adapter.convert_to_anomaly(&event);
        assert!(anomaly.is_none());
    }

    #[test]
    fn test_anomaly_type_determination() {
        let adapter = ShieldAdapter::new(ShieldConfig::default());

        let pii_factors = vec![ShieldRiskFactor {
            factor_type: "pii".to_string(),
            description: "PII detected".to_string(),
            severity: ShieldSeverity::High,
            score_contribution: 0.7,
        }];
        assert!(matches!(
            adapter.determine_anomaly_type(&pii_factors),
            AnomalyType::SecurityThreat
        ));

        let toxicity_factors = vec![ShieldRiskFactor {
            factor_type: "toxicity".to_string(),
            description: "Toxic content".to_string(),
            severity: ShieldSeverity::Medium,
            score_contribution: 0.5,
        }];
        assert!(matches!(
            adapter.determine_anomaly_type(&toxicity_factors),
            AnomalyType::QualityDegradation
        ));
    }

    #[tokio::test]
    async fn test_batch_consumption() {
        let adapter = ShieldAdapter::new(ShieldConfig::default());
        let events = vec![
            create_test_security_event(0.8),
            create_test_security_event(0.9),
            create_test_security_event(0.3),
        ];

        let anomalies = adapter.consume_batch(events).await.unwrap();

        // All 3 events should generate anomalies (is_valid = false)
        assert_eq!(anomalies.len(), 3);

        let stats = adapter.stats().await;
        assert_eq!(stats.events_consumed, 3);
        assert_eq!(stats.violations_detected, 3);
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        let mut adapter = ShieldAdapter::new(ShieldConfig::default());

        assert!(adapter.health_check().await.is_err());

        adapter.connect().await.unwrap();
        assert!(adapter.health_check().await.is_ok());

        adapter.disconnect().await.unwrap();
        assert!(adapter.health_check().await.is_err());
    }
}
