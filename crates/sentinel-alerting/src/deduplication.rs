//! Alert deduplication to prevent alert storms.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use llm_sentinel_core::{
    events::AnomalyEvent,
    types::{ModelId, ServiceId},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

/// Configuration for alert deduplication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicationConfig {
    /// Time window for deduplication (seconds)
    pub window_secs: u64,
    /// Enable deduplication
    pub enabled: bool,
    /// Cleanup interval (seconds)
    pub cleanup_interval_secs: u64,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            window_secs: 300,        // 5 minutes
            enabled: true,
            cleanup_interval_secs: 60, // 1 minute
        }
    }
}

/// Key for deduplication - represents a unique alert signature
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeduplicationKey {
    pub service: ServiceId,
    pub model: ModelId,
    pub anomaly_type: String,
    pub severity: String,
}

impl DeduplicationKey {
    /// Create key from anomaly event
    pub fn from_event(event: &AnomalyEvent) -> Self {
        Self {
            service: event.service_name.clone(),
            model: event.model.clone(),
            anomaly_type: event.anomaly_type.to_string(),
            severity: event.severity.to_string(),
        }
    }
}

/// Deduplication entry tracking when an alert was last seen
#[derive(Debug, Clone)]
struct DeduplicationEntry {
    /// Last occurrence timestamp
    last_seen: DateTime<Utc>,
    /// Number of occurrences in current window
    count: u64,
    /// Alert IDs that were deduplicated
    alert_ids: Vec<String>,
}

impl DeduplicationEntry {
    fn new(alert_id: String) -> Self {
        Self {
            last_seen: Utc::now(),
            count: 1,
            alert_ids: vec![alert_id],
        }
    }

    fn increment(&mut self, alert_id: String) {
        self.last_seen = Utc::now();
        self.count += 1;
        self.alert_ids.push(alert_id);
    }

    fn is_expired(&self, window: Duration) -> bool {
        let elapsed = Utc::now().signed_duration_since(self.last_seen);
        elapsed.num_seconds() > window.as_secs() as i64
    }
}

/// Alert deduplicator to prevent duplicate alerts
pub struct AlertDeduplicator {
    /// Map of alert signatures to last occurrence
    entries: Arc<DashMap<DeduplicationKey, DeduplicationEntry>>,
    /// Configuration
    config: DeduplicationConfig,
}

impl AlertDeduplicator {
    /// Create a new deduplicator
    pub fn new(config: DeduplicationConfig) -> Self {
        info!(
            "Creating alert deduplicator with {}s window",
            config.window_secs
        );

        Self {
            entries: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Check if alert should be sent or deduplicated
    ///
    /// Returns:
    /// - `true` if alert should be sent
    /// - `false` if alert is a duplicate and should be suppressed
    pub fn should_send(&self, event: &AnomalyEvent) -> bool {
        if !self.config.enabled {
            return true;
        }

        let key = DeduplicationKey::from_event(event);
        let alert_id = event.alert_id.to_string();

        // Check if we've seen this alert signature recently
        if let Some(mut entry) = self.entries.get_mut(&key) {
            let window = Duration::from_secs(self.config.window_secs);

            if entry.is_expired(window) {
                // Window expired, reset and send
                debug!(
                    "Deduplication window expired for {:?}, sending alert",
                    key
                );
                *entry = DeduplicationEntry::new(alert_id);
                metrics::counter!("sentinel_alerts_sent_total").increment(1);
                true
            } else {
                // Still in window, deduplicate
                entry.increment(alert_id);
                metrics::counter!("sentinel_alerts_deduplicated_total").increment(1);
                debug!(
                    "Alert deduplicated: {:?}, count: {}",
                    key, entry.count
                );
                false
            }
        } else {
            // First time seeing this alert signature
            self.entries
                .insert(key.clone(), DeduplicationEntry::new(alert_id));
            metrics::counter!("sentinel_alerts_sent_total").increment(1);
            debug!("New alert signature: {:?}, sending", key);
            true
        }
    }

    /// Get statistics about deduplicated alerts
    pub fn get_stats(&self) -> DeduplicationStats {
        let mut stats = DeduplicationStats {
            total_signatures: self.entries.len(),
            total_deduplicated: 0,
            by_severity: std::collections::HashMap::new(),
        };

        for entry in self.entries.iter() {
            stats.total_deduplicated += entry.value().count.saturating_sub(1);

            let severity_count = stats
                .by_severity
                .entry(entry.key().severity.clone())
                .or_insert(0);
            *severity_count += entry.value().count.saturating_sub(1);
        }

        stats
    }

    /// Clean up expired entries
    pub fn cleanup_expired(&self) {
        let window = Duration::from_secs(self.config.window_secs);
        let mut removed = 0;

        self.entries.retain(|_, entry| {
            let keep = !entry.is_expired(window);
            if !keep {
                removed += 1;
            }
            keep
        });

        if removed > 0 {
            info!("Cleaned up {} expired deduplication entries", removed);
        }

        metrics::gauge!("sentinel_deduplication_entries").set(self.entries.len() as f64);
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>) {
        let interval = Duration::from_secs(self.config.cleanup_interval_secs);

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;
                self.cleanup_expired();
            }
        });
    }

    /// Clear all entries (for testing)
    pub fn clear(&self) {
        self.entries.clear();
        info!("Cleared all deduplication entries");
    }

    /// Get number of active deduplication entries
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Statistics about deduplicated alerts
#[derive(Debug, Clone)]
pub struct DeduplicationStats {
    /// Total unique alert signatures
    pub total_signatures: usize,
    /// Total alerts deduplicated
    pub total_deduplicated: u64,
    /// Deduplicated count by severity
    pub by_severity: std::collections::HashMap<String, u64>,
}

impl DeduplicationStats {
    /// Get deduplication rate (0.0 to 1.0)
    pub fn deduplication_rate(&self) -> f64 {
        if self.total_signatures == 0 {
            return 0.0;
        }

        self.total_deduplicated as f64
            / (self.total_deduplicated as f64 + self.total_signatures as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        events::{AnomalyDetails, PromptInfo, ResponseInfo, TelemetryEvent},
        types::{DetectionMethod, ModelId, ServiceId},
    };

    fn create_test_anomaly(severity: Severity, anomaly_type: AnomalyType) -> AnomalyEvent {
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
            anomaly_type,
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
    fn test_deduplication_key_creation() {
        let event = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);
        let key = DeduplicationKey::from_event(&event);

        assert_eq!(key.service.as_str(), "test-service");
        assert_eq!(key.model.as_str(), "gpt-4");
        assert_eq!(key.severity, Severity::High);
        assert_eq!(key.anomaly_type, AnomalyType::LatencySpike);
    }

    #[test]
    fn test_deduplication_first_alert_sent() {
        let config = DeduplicationConfig {
            enabled: true,
            window_secs: 300,
            cleanup_interval_secs: 60,
        };

        let deduplicator = AlertDeduplicator::new(config);
        let event = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);

        // First alert should be sent
        assert!(deduplicator.should_send(&event));
    }

    #[test]
    fn test_deduplication_duplicate_suppressed() {
        let config = DeduplicationConfig {
            enabled: true,
            window_secs: 300,
            cleanup_interval_secs: 60,
        };

        let deduplicator = AlertDeduplicator::new(config);
        let event1 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);
        let event2 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);

        // First alert sent
        assert!(deduplicator.should_send(&event1));

        // Duplicate suppressed
        assert!(!deduplicator.should_send(&event2));
    }

    #[test]
    fn test_deduplication_different_severity_not_deduplicated() {
        let config = DeduplicationConfig::default();
        let deduplicator = AlertDeduplicator::new(config);

        let event1 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);
        let event2 = create_test_anomaly(Severity::Critical, AnomalyType::LatencySpike);

        // Different severities should both be sent
        assert!(deduplicator.should_send(&event1));
        assert!(deduplicator.should_send(&event2));
    }

    #[test]
    fn test_deduplication_different_type_not_deduplicated() {
        let config = DeduplicationConfig::default();
        let deduplicator = AlertDeduplicator::new(config);

        let event1 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);
        let event2 = create_test_anomaly(Severity::High, AnomalyType::CostAnomaly);

        // Different types should both be sent
        assert!(deduplicator.should_send(&event1));
        assert!(deduplicator.should_send(&event2));
    }

    #[test]
    fn test_deduplication_stats() {
        let config = DeduplicationConfig::default();
        let deduplicator = AlertDeduplicator::new(config);

        let event1 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);
        let event2 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);
        let event3 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);

        deduplicator.should_send(&event1); // Sent
        deduplicator.should_send(&event2); // Deduplicated
        deduplicator.should_send(&event3); // Deduplicated

        let stats = deduplicator.get_stats();
        assert_eq!(stats.total_signatures, 1);
        assert_eq!(stats.total_deduplicated, 2);
    }

    #[test]
    fn test_deduplication_disabled() {
        let config = DeduplicationConfig {
            enabled: false,
            window_secs: 300,
            cleanup_interval_secs: 60,
        };

        let deduplicator = AlertDeduplicator::new(config);
        let event1 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);
        let event2 = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);

        // Both should be sent when disabled
        assert!(deduplicator.should_send(&event1));
        assert!(deduplicator.should_send(&event2));
    }

    #[test]
    fn test_cleanup_expired() {
        let config = DeduplicationConfig {
            enabled: true,
            window_secs: 1, // 1 second window
            cleanup_interval_secs: 60,
        };

        let deduplicator = AlertDeduplicator::new(config);
        let event = create_test_anomaly(Severity::High, AnomalyType::LatencySpike);

        deduplicator.should_send(&event);
        assert_eq!(deduplicator.entry_count(), 1);

        // Wait for expiration
        std::thread::sleep(std::time::Duration::from_secs(2));

        deduplicator.cleanup_expired();
        assert_eq!(deduplicator.entry_count(), 0);
    }
}
