//! Ruvector Client for Phase 2 Agents
//!
//! Provides a strongly-typed client for interacting with the Ruvector event
//! indexing service. Ruvector is treated as the authoritative event index
//! for all Phase 2 operations.

use std::time::Duration;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Default timeout for Ruvector operations
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Default retry attempts for transient failures
const DEFAULT_RETRIES: u32 = 3;

/// Errors that can occur when interacting with Ruvector
#[derive(Debug, Error)]
pub enum RuvectorError {
    /// Connection error
    #[error("Failed to connect to Ruvector: {0}")]
    Connection(String),

    /// Authentication error
    #[error("Ruvector authentication failed: {0}")]
    Authentication(String),

    /// Request timeout
    #[error("Ruvector request timed out")]
    Timeout,

    /// Ruvector service is unhealthy
    #[error("Ruvector service is unhealthy")]
    Unhealthy,

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Rate limited
    #[error("Rate limited by Ruvector")]
    RateLimited,

    /// Internal server error
    #[error("Ruvector internal error: {0}")]
    InternalError(String),
}

/// Signal record to store in Ruvector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalRecord {
    /// Unique signal identifier
    pub signal_id: Uuid,
    /// Signal type (anomaly, drift, lineage, latency)
    pub signal_type: String,
    /// Source agent name
    pub agent_name: String,
    /// Agent domain
    pub agent_domain: String,
    /// Signal timestamp
    pub timestamp: DateTime<Utc>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Signal payload (atomic, no conclusions)
    pub payload: serde_json::Value,
    /// Optional parent signal IDs for lineage
    pub parent_signals: Vec<Uuid>,
    /// Processing latency in milliseconds
    pub latency_ms: u64,
}

/// Lineage delta to record event relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageDelta {
    /// Delta identifier
    pub delta_id: Uuid,
    /// Source signal ID
    pub source_id: Uuid,
    /// Target signal ID
    pub target_id: Uuid,
    /// Relationship type
    pub relationship: LineageRelation,
    /// Timestamp of relationship
    pub timestamp: DateTime<Utc>,
    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

/// Types of lineage relationships
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineageRelation {
    /// Signal derived from another
    DerivedFrom,
    /// Signal triggered another
    Triggered,
    /// Signals are correlated
    Correlated,
    /// Signal supersedes another
    Supersedes,
    /// Signal confirms another
    Confirms,
}

/// Query parameters for signal retrieval
#[derive(Debug, Clone, Default, Serialize)]
pub struct SignalQuery {
    /// Filter by signal type
    pub signal_type: Option<String>,
    /// Filter by agent name
    pub agent_name: Option<String>,
    /// Start time (inclusive)
    pub start_time: Option<DateTime<Utc>>,
    /// End time (exclusive)
    pub end_time: Option<DateTime<Utc>>,
    /// Minimum confidence threshold
    pub min_confidence: Option<f64>,
    /// Maximum results to return
    pub limit: Option<u32>,
}

/// Ruvector client for event indexing operations
#[derive(Debug, Clone)]
pub struct RuvectorClient {
    /// Service URL
    base_url: String,
    /// API key
    api_key: String,
    /// HTTP client
    client: reqwest::Client,
    /// Request timeout
    timeout: Duration,
    /// Retry attempts
    retries: u32,
}

impl RuvectorClient {
    /// Create a new Ruvector client
    pub fn new(base_url: &str, api_key: &str) -> Result<Self, RuvectorError> {
        Self::with_timeout(base_url, api_key, DEFAULT_TIMEOUT)
    }

    /// Create a new Ruvector client with custom timeout
    pub fn with_timeout(
        base_url: &str,
        api_key: &str,
        timeout: Duration,
    ) -> Result<Self, RuvectorError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .pool_max_idle_per_host(10)
            .build()
            .map_err(|e| RuvectorError::Connection(e.to_string()))?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client,
            timeout,
            retries: DEFAULT_RETRIES,
        })
    }

    /// Set the number of retry attempts
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Perform a health check against Ruvector
    #[instrument(skip(self), fields(service = "ruvector"))]
    pub async fn health_check(&self) -> Result<bool, RuvectorError> {
        let url = format!("{}/health", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    debug!("Ruvector health check passed");
                    Ok(true)
                } else if response.status().as_u16() == 503 {
                    warn!("Ruvector service unavailable");
                    Ok(false)
                } else {
                    warn!(status = %response.status(), "Ruvector health check failed");
                    Ok(false)
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to reach Ruvector");
                if e.is_timeout() {
                    Err(RuvectorError::Timeout)
                } else {
                    Err(RuvectorError::Connection(e.to_string()))
                }
            }
        }
    }

    /// Store a signal record in Ruvector
    #[instrument(skip(self, signal), fields(signal_id = %signal.signal_id, signal_type = %signal.signal_type))]
    pub async fn store_signal(&self, signal: &SignalRecord) -> Result<(), RuvectorError> {
        let url = format!("{}/v1/signals", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(signal)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    RuvectorError::Timeout
                } else {
                    RuvectorError::Connection(e.to_string())
                }
            })?;

        match response.status().as_u16() {
            200 | 201 => {
                debug!(signal_id = %signal.signal_id, "Signal stored successfully");
                Ok(())
            }
            401 | 403 => {
                error!("Authentication failed");
                Err(RuvectorError::Authentication("Invalid API key".to_string()))
            }
            429 => {
                warn!("Rate limited by Ruvector");
                Err(RuvectorError::RateLimited)
            }
            400 => {
                let body = response.text().await.unwrap_or_default();
                Err(RuvectorError::InvalidRequest(body))
            }
            status => {
                let body = response.text().await.unwrap_or_default();
                error!(status, body = %body, "Ruvector error");
                Err(RuvectorError::InternalError(format!(
                    "Status {}: {}",
                    status, body
                )))
            }
        }
    }

    /// Store a lineage delta (relationship between signals)
    #[instrument(skip(self, delta), fields(source = %delta.source_id, target = %delta.target_id))]
    pub async fn store_lineage(&self, delta: &LineageDelta) -> Result<(), RuvectorError> {
        let url = format!("{}/v1/lineage", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(delta)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    RuvectorError::Timeout
                } else {
                    RuvectorError::Connection(e.to_string())
                }
            })?;

        match response.status().as_u16() {
            200 | 201 => {
                debug!(delta_id = %delta.delta_id, "Lineage delta stored");
                Ok(())
            }
            401 | 403 => Err(RuvectorError::Authentication("Invalid API key".to_string())),
            429 => Err(RuvectorError::RateLimited),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(RuvectorError::InternalError(format!(
                    "Status {}: {}",
                    status, body
                )))
            }
        }
    }

    /// Query signals from Ruvector
    #[instrument(skip(self, query))]
    pub async fn query_signals(&self, query: &SignalQuery) -> Result<Vec<SignalRecord>, RuvectorError> {
        let url = format!("{}/v1/signals/query", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(query)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    RuvectorError::Timeout
                } else {
                    RuvectorError::Connection(e.to_string())
                }
            })?;

        match response.status().as_u16() {
            200 => {
                let signals: Vec<SignalRecord> = response
                    .json()
                    .await
                    .map_err(|e| RuvectorError::Serialization(e.to_string()))?;
                debug!(count = signals.len(), "Signals retrieved");
                Ok(signals)
            }
            401 | 403 => Err(RuvectorError::Authentication("Invalid API key".to_string())),
            429 => Err(RuvectorError::RateLimited),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(RuvectorError::InternalError(format!(
                    "Status {}: {}",
                    status, body
                )))
            }
        }
    }

    /// Get signal lineage (ancestors and descendants)
    #[instrument(skip(self), fields(signal_id = %signal_id))]
    pub async fn get_lineage(&self, signal_id: Uuid) -> Result<Vec<LineageDelta>, RuvectorError> {
        let url = format!("{}/v1/lineage/{}", self.base_url, signal_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    RuvectorError::Timeout
                } else {
                    RuvectorError::Connection(e.to_string())
                }
            })?;

        match response.status().as_u16() {
            200 => {
                let deltas: Vec<LineageDelta> = response
                    .json()
                    .await
                    .map_err(|e| RuvectorError::Serialization(e.to_string()))?;
                Ok(deltas)
            }
            404 => Ok(vec![]), // No lineage found
            401 | 403 => Err(RuvectorError::Authentication("Invalid API key".to_string())),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(RuvectorError::InternalError(format!(
                    "Status {}: {}",
                    status, body
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = RuvectorClient::new("http://localhost:8080", "test-key");
        assert!(client.is_ok());
    }

    #[test]
    fn test_signal_record_serialization() {
        let signal = SignalRecord {
            signal_id: Uuid::new_v4(),
            signal_type: "anomaly".to_string(),
            agent_name: "test-agent".to_string(),
            agent_domain: "detection".to_string(),
            timestamp: Utc::now(),
            confidence: 0.85,
            payload: serde_json::json!({"metric": "latency", "value": 150.5}),
            parent_signals: vec![],
            latency_ms: 25,
        };

        let json = serde_json::to_string(&signal);
        assert!(json.is_ok());
    }

    #[test]
    fn test_lineage_delta_serialization() {
        let delta = LineageDelta {
            delta_id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            relationship: LineageRelation::DerivedFrom,
            timestamp: Utc::now(),
            metadata: None,
        };

        let json = serde_json::to_string(&delta);
        assert!(json.is_ok());
    }
}
