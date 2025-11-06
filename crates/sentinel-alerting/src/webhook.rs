//! Webhook alert delivery for HTTP-based notifications.

use crate::{AlertConfig, Alerter};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use llm_sentinel_core::{events::AnomalyEvent, Error, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL
    pub url: String,
    /// HTTP method (POST, PUT)
    pub method: HttpMethod,
    /// Request timeout (seconds)
    pub timeout_secs: u64,
    /// Custom headers
    pub headers: Vec<(String, String)>,
    /// Retry configuration
    pub max_retries: u32,
    /// Initial retry delay (milliseconds)
    pub retry_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Secret for HMAC signing (optional)
    pub secret: Option<String>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            method: HttpMethod::Post,
            timeout_secs: 10,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            max_retries: 3,
            retry_delay_ms: 1000,
            backoff_multiplier: 2.0,
            secret: None,
        }
    }
}

/// HTTP method for webhook
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    #[serde(rename = "POST")]
    Post,
    #[serde(rename = "PUT")]
    Put,
}

/// Webhook payload wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Webhook event type
    pub event_type: String,
    /// Timestamp of webhook
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// The anomaly event
    pub data: AnomalyEvent,
    /// Optional signature for verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Webhook alerter
pub struct WebhookAlerter {
    client: Client,
    config: WebhookConfig,
}

impl WebhookAlerter {
    /// Create a new webhook alerter
    pub fn new(config: WebhookConfig) -> Result<Self> {
        if config.url.is_empty() {
            return Err(Error::config("Webhook URL cannot be empty"));
        }

        info!("Creating webhook alerter for {}", config.url);

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| Error::config(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, config })
    }

    /// Generate HMAC signature for payload
    fn generate_signature(&self, payload: &str) -> Option<String> {
        self.config.secret.as_ref().map(|secret| {
            use hmac::{Hmac, Mac};
            use sha2::Sha256;

            type HmacSha256 = Hmac<Sha256>;

            let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
            mac.update(payload.as_bytes());
            let result = mac.finalize();
            hex::encode(result.into_bytes())
        })
    }

    /// Send webhook with retry logic
    async fn send_with_retry(&self, alert: &AnomalyEvent) -> Result<()> {
        let mut payload = WebhookPayload {
            event_type: "anomaly.detected".to_string(),
            timestamp: chrono::Utc::now(),
            data: alert.clone(),
            signature: None,
        };

        let payload_json = serde_json::to_string(&payload).map_err(|e| {
            Error::serialization(format!("Failed to serialize webhook payload: {}", e))
        })?;

        // Generate signature if secret is configured
        if let Some(signature) = self.generate_signature(&payload_json) {
            payload.signature = Some(signature);
        }

        let final_payload = serde_json::to_string(&payload).map_err(|e| {
            Error::serialization(format!("Failed to serialize webhook payload: {}", e))
        })?;

        let mut attempt = 0;
        let mut delay = self.config.retry_delay_ms;

        loop {
            attempt += 1;

            let mut request = match self.config.method {
                HttpMethod::Post => self.client.post(&self.config.url),
                HttpMethod::Put => self.client.put(&self.config.url),
            };

            // Add custom headers
            for (key, value) in &self.config.headers {
                request = request.header(key, value);
            }

            // Add signature header if present
            if let Some(ref sig) = payload.signature {
                request = request.header("X-Sentinel-Signature", sig);
            }

            request = request.body(final_payload.clone());

            match request.send().await {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        debug!(
                            alert_id = %alert.alert_id,
                            url = %self.config.url,
                            status = %status,
                            attempt = attempt,
                            "Webhook sent successfully"
                        );

                        metrics::counter!("sentinel_webhook_success_total").increment(1);

                        if attempt > 1 {
                            metrics::counter!("sentinel_webhook_retries_total").increment(1);
                        }

                        return Ok(());
                    } else if Self::is_retryable_status(status) && attempt < self.config.max_retries
                    {
                        warn!(
                            alert_id = %alert.alert_id,
                            status = %status,
                            attempt = attempt,
                            delay_ms = delay,
                            "Webhook failed with retryable status, retrying..."
                        );

                        tokio::time::sleep(Duration::from_millis(delay)).await;

                        delay = (delay as f64 * self.config.backoff_multiplier) as u64;
                    } else {
                        let body = response.text().await.unwrap_or_default();

                        error!(
                            alert_id = %alert.alert_id,
                            status = %status,
                            body = %body,
                            attempts = attempt,
                            "Webhook failed with non-retryable status or max retries reached"
                        );

                        metrics::counter!("sentinel_webhook_failures_total").increment(1);

                        return Err(Error::alerting(format!(
                            "Webhook failed with status {}: {}",
                            status, body
                        )));
                    }
                }
                Err(e) => {
                    if attempt >= self.config.max_retries {
                        error!(
                            alert_id = %alert.alert_id,
                            attempts = attempt,
                            error = %e,
                            "Webhook failed after max retries"
                        );

                        metrics::counter!("sentinel_webhook_failures_total").increment(1);

                        return Err(Error::alerting(format!(
                            "Webhook failed after {} attempts: {}",
                            attempt, e
                        )));
                    }

                    warn!(
                        alert_id = %alert.alert_id,
                        attempt = attempt,
                        delay_ms = delay,
                        error = %e,
                        "Webhook request failed, retrying..."
                    );

                    tokio::time::sleep(Duration::from_millis(delay)).await;

                    delay = (delay as f64 * self.config.backoff_multiplier) as u64;
                }
            }
        }
    }

    /// Check if HTTP status code is retryable
    fn is_retryable_status(status: StatusCode) -> bool {
        matches!(
            status,
            StatusCode::REQUEST_TIMEOUT
                | StatusCode::TOO_MANY_REQUESTS
                | StatusCode::INTERNAL_SERVER_ERROR
                | StatusCode::BAD_GATEWAY
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::GATEWAY_TIMEOUT
        )
    }
}

#[async_trait]
impl Alerter for WebhookAlerter {
    async fn send(&self, alert: &AnomalyEvent) -> Result<()> {
        self.send_with_retry(alert).await
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
                    "Failed to send webhook in batch"
                );
                errors.push(e);
            }
        }

        if !errors.is_empty() {
            return Err(Error::alerting(format!(
                "Failed to send {} out of {} webhooks",
                errors.len(),
                alerts.len()
            )));
        }

        info!("Successfully sent batch of {} webhooks", alerts.len());
        Ok(())
    }

    async fn health_check(&self) -> Result<()> {
        // Simple HEAD request to check if endpoint is reachable
        match self.client.head(&self.config.url).send().await {
            Ok(response) => {
                if response.status().is_success() || response.status() == StatusCode::METHOD_NOT_ALLOWED {
                    Ok(())
                } else {
                    Err(Error::connection(format!(
                        "Webhook health check failed with status: {}",
                        response.status()
                    )))
                }
            }
            Err(e) => Err(Error::connection(format!(
                "Webhook health check failed: {}",
                e
            ))),
        }
    }

    fn name(&self) -> &str {
        "Webhook"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        events::{AnomalyDetails, PromptInfo, ResponseInfo, TelemetryEvent},
        types::{AnomalyType, DetectionMethod, ModelId, ServiceId, Severity},
    };

    fn create_test_config(url: &str) -> WebhookConfig {
        WebhookConfig {
            url: url.to_string(),
            method: HttpMethod::Post,
            timeout_secs: 5,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            max_retries: 2,
            retry_delay_ms: 100,
            backoff_multiplier: 2.0,
            secret: Some("test-secret".to_string()),
        }
    }

    fn create_test_anomaly() -> AnomalyEvent {
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
            Severity::High,
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
        let config = create_test_config("https://example.com/webhook");
        assert_eq!(config.url, "https://example.com/webhook");
        assert_eq!(config.method, HttpMethod::Post);
    }

    #[test]
    fn test_empty_url_error() {
        let config = WebhookConfig {
            url: String::new(),
            ..Default::default()
        };

        let result = WebhookAlerter::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_retryable_status_codes() {
        assert!(WebhookAlerter::is_retryable_status(
            StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(WebhookAlerter::is_retryable_status(
            StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(WebhookAlerter::is_retryable_status(
            StatusCode::TOO_MANY_REQUESTS
        ));
        assert!(!WebhookAlerter::is_retryable_status(StatusCode::NOT_FOUND));
        assert!(!WebhookAlerter::is_retryable_status(StatusCode::BAD_REQUEST));
    }

    #[tokio::test]
    async fn test_webhook_payload_serialization() {
        let alert = create_test_anomaly();
        let payload = WebhookPayload {
            event_type: "anomaly.detected".to_string(),
            timestamp: chrono::Utc::now(),
            data: alert,
            signature: Some("test-signature".to_string()),
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("anomaly.detected"));
        assert!(json.contains("test-signature"));
    }

    // Integration tests with wiremock
    #[tokio::test]
    async fn test_successful_webhook() {
        use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/webhook"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = create_test_config(&format!("{}/webhook", mock_server.uri()));
        let alerter = WebhookAlerter::new(config).unwrap();
        let alert = create_test_anomaly();

        let result = alerter.send(&alert).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_webhook_retry_on_500() {
        use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // First call fails with 500, second succeeds
        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = create_test_config(&format!("{}/webhook", mock_server.uri()));
        let alerter = WebhookAlerter::new(config).unwrap();
        let alert = create_test_anomaly();

        let result = alerter.send(&alert).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_webhook_max_retries_exceeded() {
        use wiremock::{matchers::*, Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Always return 500
        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = create_test_config(&format!("{}/webhook", mock_server.uri()));
        let alerter = WebhookAlerter::new(config).unwrap();
        let alert = create_test_anomaly();

        let result = alerter.send(&alert).await;
        assert!(result.is_err());
    }
}
