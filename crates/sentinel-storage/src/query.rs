//! Query definitions for storage backends.

use chrono::{DateTime, Utc};
use llm_sentinel_core::types::{AnomalyType, ModelId, ServiceId, Severity};
use serde::{Deserialize, Serialize};

/// Time range for queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start time (inclusive)
    pub start: DateTime<Utc>,
    /// End time (exclusive)
    pub end: DateTime<Utc>,
}

impl TimeRange {
    /// Create a new time range
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    /// Create a time range for the last N hours
    pub fn last_hours(hours: i64) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::hours(hours);
        Self { start, end }
    }

    /// Create a time range for the last N days
    pub fn last_days(days: i64) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(days);
        Self { start, end }
    }

    /// Create a time range for the last N minutes
    pub fn last_minutes(minutes: i64) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::minutes(minutes);
        Self { start, end }
    }

    /// Duration in seconds
    pub fn duration_secs(&self) -> i64 {
        (self.end - self.start).num_seconds()
    }
}

/// Query for telemetry events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryQuery {
    /// Time range
    pub time_range: TimeRange,

    /// Filter by service
    pub service: Option<ServiceId>,

    /// Filter by model
    pub model: Option<ModelId>,

    /// Limit number of results
    pub limit: Option<usize>,

    /// Offset for pagination
    pub offset: Option<usize>,

    /// Sort order (true = ascending, false = descending)
    pub ascending: bool,
}

impl TelemetryQuery {
    /// Create a new telemetry query
    pub fn new(time_range: TimeRange) -> Self {
        Self {
            time_range,
            service: None,
            model: None,
            limit: Some(1000), // Default limit
            offset: None,
            ascending: false, // Default: newest first
        }
    }

    /// Filter by service
    pub fn with_service(mut self, service: ServiceId) -> Self {
        self.service = Some(service);
        self
    }

    /// Filter by model
    pub fn with_model(mut self, model: ModelId) -> Self {
        self.model = Some(model);
        self
    }

    /// Set limit
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set offset
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set sort order
    pub fn ascending(mut self) -> Self {
        self.ascending = true;
        self
    }

    /// Set sort order
    pub fn descending(mut self) -> Self {
        self.ascending = false;
        self
    }
}

/// Query for anomaly events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyQuery {
    /// Time range
    pub time_range: TimeRange,

    /// Filter by service
    pub service: Option<ServiceId>,

    /// Filter by model
    pub model: Option<ModelId>,

    /// Filter by severity
    pub severity: Option<Severity>,

    /// Filter by anomaly type
    pub anomaly_type: Option<AnomalyType>,

    /// Minimum confidence threshold
    pub min_confidence: Option<f64>,

    /// Limit number of results
    pub limit: Option<usize>,

    /// Offset for pagination
    pub offset: Option<usize>,

    /// Sort order
    pub ascending: bool,
}

impl AnomalyQuery {
    /// Create a new anomaly query
    pub fn new(time_range: TimeRange) -> Self {
        Self {
            time_range,
            service: None,
            model: None,
            severity: None,
            anomaly_type: None,
            min_confidence: None,
            limit: Some(1000),
            offset: None,
            ascending: false,
        }
    }

    /// Filter by service
    pub fn with_service(mut self, service: ServiceId) -> Self {
        self.service = Some(service);
        self
    }

    /// Filter by model
    pub fn with_model(mut self, model: ModelId) -> Self {
        self.model = Some(model);
        self
    }

    /// Filter by severity
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// Filter by anomaly type
    pub fn with_type(mut self, anomaly_type: AnomalyType) -> Self {
        self.anomaly_type = Some(anomaly_type);
        self
    }

    /// Filter by minimum confidence
    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = Some(confidence);
        self
    }

    /// Set limit
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_range_creation() {
        let range = TimeRange::last_hours(24);
        assert!(range.duration_secs() > 86300); // ~24 hours
        assert!(range.duration_secs() < 86500);
    }

    #[test]
    fn test_time_range_last_days() {
        let range = TimeRange::last_days(7);
        assert!(range.duration_secs() > 604700); // ~7 days
    }

    #[test]
    fn test_telemetry_query_builder() {
        let query = TelemetryQuery::new(TimeRange::last_hours(1))
            .with_service(ServiceId::new("test"))
            .with_model(ModelId::new("gpt-4"))
            .with_limit(100)
            .descending();

        assert_eq!(query.limit, Some(100));
        assert!(!query.ascending);
        assert!(query.service.is_some());
    }

    #[test]
    fn test_anomaly_query_builder() {
        let query = AnomalyQuery::new(TimeRange::last_hours(24))
            .with_severity(Severity::High)
            .with_type(AnomalyType::LatencySpike)
            .with_min_confidence(0.9)
            .with_limit(50);

        assert_eq!(query.severity, Some(Severity::High));
        assert_eq!(query.min_confidence, Some(0.9));
        assert_eq!(query.limit, Some(50));
    }
}
