//! # LLM-Config-Manager Adapter
//!
//! Consumes threshold configuration, model-specific detection parameters,
//! and sensitivity levels from LLM-Config-Manager.
//!
//! ## Data Consumed
//!
//! - **Threshold Configuration**: Detection thresholds (Z-score, IQR, etc.)
//! - **Model-Specific Parameters**: Per-model detection settings
//! - **Sensitivity Levels**: Environment-specific sensitivity overrides
//!
//! ## Integration Pattern
//!
//! Configuration data is consumed and made available to detection algorithms
//! without modifying their core logic. This enables dynamic threshold
//! adjustment while preserving algorithmic integrity.

use crate::adapters::UpstreamAdapter;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use llm_sentinel_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Configuration for Config-Manager adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigManagerAdapterConfig {
    /// Config-Manager API endpoint
    pub api_endpoint: String,

    /// Namespace prefix for Sentinel configuration
    pub namespace_prefix: String,

    /// Current environment
    pub environment: ConfigEnvironment,

    /// Cache refresh interval in seconds
    pub cache_refresh_secs: u64,

    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
}

impl Default for ConfigManagerAdapterConfig {
    fn default() -> Self {
        Self {
            api_endpoint: "http://localhost:8083".to_string(),
            namespace_prefix: "sentinel".to_string(),
            environment: ConfigEnvironment::Development,
            cache_refresh_secs: 60,
            connect_timeout_ms: 5000,
        }
    }
}

/// Environment for configuration (mirrors Config-Manager's Environment)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigEnvironment {
    Base,
    Development,
    Staging,
    Production,
    Edge,
}

impl ConfigEnvironment {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "base" => Some(Self::Base),
            "dev" | "development" => Some(Self::Development),
            "staging" | "stg" => Some(Self::Staging),
            "prod" | "production" => Some(Self::Production),
            "edge" => Some(Self::Edge),
            _ => None,
        }
    }
}

/// Configuration value types (mirrors Config-Manager's ConfigValue)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ConfigValue>),
    Object(HashMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Try to get as f64
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            ConfigValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get as i64
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as string
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }
}

/// Configuration entry from Config-Manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    /// Configuration key
    pub key: String,

    /// Configuration value
    pub value: ConfigValue,

    /// Environment
    pub environment: ConfigEnvironment,

    /// Version number
    pub version: u64,

    /// Last update time
    pub updated_at: DateTime<Utc>,

    /// Description
    #[serde(default)]
    pub description: Option<String>,
}

/// Detection threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionThresholds {
    /// Z-score threshold (default: 3.0)
    pub zscore_threshold: f64,

    /// IQR threshold factor (default: 1.5)
    pub iqr_threshold_factor: f64,

    /// MAD threshold (default: 3.5)
    pub mad_threshold: f64,

    /// CUSUM threshold (default: 5.0)
    pub cusum_threshold: f64,

    /// Minimum confidence for anomaly reporting
    pub min_confidence: f64,
}

impl Default for DetectionThresholds {
    fn default() -> Self {
        Self {
            zscore_threshold: 3.0,
            iqr_threshold_factor: 1.5,
            mad_threshold: 3.5,
            cusum_threshold: 5.0,
            min_confidence: 0.7,
        }
    }
}

/// Model-specific detection parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDetectionParams {
    /// Model identifier
    pub model: String,

    /// Latency threshold multiplier
    pub latency_threshold_multiplier: f64,

    /// Token usage threshold multiplier
    pub token_threshold_multiplier: f64,

    /// Cost threshold multiplier
    pub cost_threshold_multiplier: f64,

    /// Error rate threshold
    pub error_rate_threshold: f64,

    /// Custom thresholds per metric
    #[serde(default)]
    pub custom_thresholds: HashMap<String, f64>,
}

impl ModelDetectionParams {
    /// Create default params for a model
    pub fn default_for_model(model: &str) -> Self {
        Self {
            model: model.to_string(),
            latency_threshold_multiplier: 1.0,
            token_threshold_multiplier: 1.0,
            cost_threshold_multiplier: 1.0,
            error_rate_threshold: 0.05, // 5%
            custom_thresholds: HashMap::new(),
        }
    }
}

/// Sensitivity level configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SensitivityLevel {
    /// Low sensitivity - fewer alerts, higher thresholds
    Low,
    /// Medium sensitivity - balanced
    Medium,
    /// High sensitivity - more alerts, lower thresholds
    High,
    /// Maximum sensitivity - most sensitive detection
    Maximum,
}

impl SensitivityLevel {
    /// Get threshold multiplier for this sensitivity level
    pub fn threshold_multiplier(&self) -> f64 {
        match self {
            SensitivityLevel::Low => 1.5,
            SensitivityLevel::Medium => 1.0,
            SensitivityLevel::High => 0.7,
            SensitivityLevel::Maximum => 0.5,
        }
    }

    /// Get minimum confidence for this sensitivity level
    pub fn min_confidence(&self) -> f64 {
        match self {
            SensitivityLevel::Low => 0.9,
            SensitivityLevel::Medium => 0.8,
            SensitivityLevel::High => 0.7,
            SensitivityLevel::Maximum => 0.5,
        }
    }
}

impl Default for SensitivityLevel {
    fn default() -> Self {
        Self::Medium
    }
}

/// Adapter for consuming configuration from LLM-Config-Manager
#[derive(Debug)]
pub struct ConfigManagerAdapter {
    config: ConfigManagerAdapterConfig,
    connected: Arc<RwLock<bool>>,

    // Cached configuration
    thresholds_cache: Arc<RwLock<DetectionThresholds>>,
    model_params_cache: Arc<RwLock<HashMap<String, ModelDetectionParams>>>,
    sensitivity_cache: Arc<RwLock<SensitivityLevel>>,
    raw_config_cache: Arc<RwLock<HashMap<String, ConfigEntry>>>,

    // Statistics
    config_fetches: Arc<RwLock<u64>>,
    cache_hits: Arc<RwLock<u64>>,
    cache_misses: Arc<RwLock<u64>>,
}

impl ConfigManagerAdapter {
    /// Create a new Config-Manager adapter
    pub fn new(config: ConfigManagerAdapterConfig) -> Self {
        Self {
            config,
            connected: Arc::new(RwLock::new(false)),
            thresholds_cache: Arc::new(RwLock::new(DetectionThresholds::default())),
            model_params_cache: Arc::new(RwLock::new(HashMap::new())),
            sensitivity_cache: Arc::new(RwLock::new(SensitivityLevel::default())),
            raw_config_cache: Arc::new(RwLock::new(HashMap::new())),
            config_fetches: Arc::new(RwLock::new(0)),
            cache_hits: Arc::new(RwLock::new(0)),
            cache_misses: Arc::new(RwLock::new(0)),
        }
    }

    /// Consume detection thresholds from Config-Manager
    pub async fn consume_thresholds(&self, thresholds: DetectionThresholds) -> Result<()> {
        let mut cache = self.thresholds_cache.write().await;
        *cache = thresholds;

        let mut fetches = self.config_fetches.write().await;
        *fetches += 1;

        debug!("Updated detection thresholds cache");
        Ok(())
    }

    /// Get current detection thresholds
    ///
    /// Returns cached thresholds adjusted by the current sensitivity level.
    pub async fn get_thresholds(&self) -> DetectionThresholds {
        let thresholds = self.thresholds_cache.read().await.clone();
        let sensitivity = *self.sensitivity_cache.read().await;
        let multiplier = sensitivity.threshold_multiplier();

        // Adjust thresholds based on sensitivity
        DetectionThresholds {
            zscore_threshold: thresholds.zscore_threshold * multiplier,
            iqr_threshold_factor: thresholds.iqr_threshold_factor * multiplier,
            mad_threshold: thresholds.mad_threshold * multiplier,
            cusum_threshold: thresholds.cusum_threshold * multiplier,
            min_confidence: sensitivity.min_confidence(),
        }
    }

    /// Consume model-specific parameters
    pub async fn consume_model_params(&self, params: Vec<ModelDetectionParams>) -> Result<()> {
        let mut cache = self.model_params_cache.write().await;

        for param in params {
            cache.insert(param.model.clone(), param);
        }

        let mut fetches = self.config_fetches.write().await;
        *fetches += 1;

        debug!("Updated model parameters cache with {} entries", cache.len());
        Ok(())
    }

    /// Get model-specific parameters
    pub async fn get_model_params(&self, model: &str) -> ModelDetectionParams {
        let cache = self.model_params_cache.read().await;

        if let Some(params) = cache.get(model) {
            let mut hits = self.cache_hits.write().await;
            *hits += 1;
            params.clone()
        } else {
            let mut misses = self.cache_misses.write().await;
            *misses += 1;
            ModelDetectionParams::default_for_model(model)
        }
    }

    /// Consume sensitivity level configuration
    pub async fn consume_sensitivity(&self, sensitivity: SensitivityLevel) -> Result<()> {
        let mut cache = self.sensitivity_cache.write().await;
        *cache = sensitivity;

        debug!("Updated sensitivity level to {:?}", sensitivity);
        Ok(())
    }

    /// Get current sensitivity level
    pub async fn get_sensitivity(&self) -> SensitivityLevel {
        *self.sensitivity_cache.read().await
    }

    /// Consume raw configuration entries
    pub async fn consume_raw_config(&self, entries: Vec<ConfigEntry>) -> Result<()> {
        let mut cache = self.raw_config_cache.write().await;

        for entry in entries {
            cache.insert(entry.key.clone(), entry);
        }

        let mut fetches = self.config_fetches.write().await;
        *fetches += 1;

        debug!("Updated raw config cache with {} entries", cache.len());
        Ok(())
    }

    /// Get raw configuration value
    pub async fn get_raw_config(&self, key: &str) -> Option<ConfigValue> {
        let cache = self.raw_config_cache.read().await;
        cache.get(key).map(|e| e.value.clone())
    }

    /// Get effective threshold for a specific detector and model
    ///
    /// This combines base thresholds with model-specific multipliers
    /// and sensitivity adjustments.
    pub async fn get_effective_threshold(
        &self,
        detector_type: &str,
        model: &str,
    ) -> f64 {
        let thresholds = self.get_thresholds().await;
        let model_params = self.get_model_params(model).await;

        // Get base threshold for detector type
        let base_threshold = match detector_type {
            "zscore" => thresholds.zscore_threshold,
            "iqr" => thresholds.iqr_threshold_factor,
            "mad" => thresholds.mad_threshold,
            "cusum" => thresholds.cusum_threshold,
            _ => {
                // Check custom thresholds
                model_params
                    .custom_thresholds
                    .get(detector_type)
                    .copied()
                    .unwrap_or(3.0) // Default
            }
        };

        // Apply model-specific multiplier if relevant
        let multiplier = match detector_type {
            "zscore" | "iqr" | "mad" | "cusum" => 1.0, // Base detectors use raw threshold
            _ => model_params.latency_threshold_multiplier, // Use latency as default multiplier
        };

        base_threshold * multiplier
    }

    /// Get adapter statistics
    pub async fn stats(&self) -> ConfigManagerAdapterStats {
        ConfigManagerAdapterStats {
            config_fetches: *self.config_fetches.read().await,
            cache_hits: *self.cache_hits.read().await,
            cache_misses: *self.cache_misses.read().await,
            models_configured: self.model_params_cache.read().await.len(),
            raw_config_entries: self.raw_config_cache.read().await.len(),
        }
    }

    /// Clear all caches (force refresh on next access)
    pub async fn clear_caches(&self) {
        *self.thresholds_cache.write().await = DetectionThresholds::default();
        self.model_params_cache.write().await.clear();
        *self.sensitivity_cache.write().await = SensitivityLevel::default();
        self.raw_config_cache.write().await.clear();

        debug!("Config-Manager adapter caches cleared");
    }
}

/// Statistics for Config-Manager adapter
#[derive(Debug, Clone)]
pub struct ConfigManagerAdapterStats {
    pub config_fetches: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub models_configured: usize,
    pub raw_config_entries: usize,
}

#[async_trait]
impl UpstreamAdapter for ConfigManagerAdapter {
    fn name(&self) -> &'static str {
        "llm-config-manager"
    }

    async fn health_check(&self) -> Result<()> {
        let connected = *self.connected.read().await;
        if connected {
            Ok(())
        } else {
            Err(llm_sentinel_core::Error::connection(
                "Config-Manager adapter not connected",
            ))
        }
    }

    async fn connect(&mut self) -> Result<()> {
        info!(
            "Connecting to Config-Manager at {}",
            self.config.api_endpoint
        );

        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Config-Manager adapter connected successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Config-Manager");

        let mut connected = self.connected.write().await;
        *connected = false;

        let stats = self.stats().await;
        info!(
            "Config-Manager adapter disconnected. Fetches: {}, Hits: {}, Misses: {}",
            stats.config_fetches, stats.cache_hits, stats.cache_misses
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitivity_multipliers() {
        assert!(SensitivityLevel::Low.threshold_multiplier() > 1.0);
        assert!((SensitivityLevel::Medium.threshold_multiplier() - 1.0).abs() < 0.001);
        assert!(SensitivityLevel::High.threshold_multiplier() < 1.0);
        assert!(SensitivityLevel::Maximum.threshold_multiplier() < SensitivityLevel::High.threshold_multiplier());
    }

    #[test]
    fn test_config_value_conversions() {
        let float = ConfigValue::Float(3.14);
        assert!((float.as_f64().unwrap() - 3.14).abs() < 0.001);

        let int = ConfigValue::Integer(42);
        assert_eq!(int.as_i64(), Some(42));
        assert!((int.as_f64().unwrap() - 42.0).abs() < 0.001);

        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(bool_val.as_bool(), Some(true));

        let string = ConfigValue::String("test".to_string());
        assert_eq!(string.as_str(), Some("test"));
    }

    #[test]
    fn test_environment_parsing() {
        assert_eq!(
            ConfigEnvironment::from_str("dev"),
            Some(ConfigEnvironment::Development)
        );
        assert_eq!(
            ConfigEnvironment::from_str("development"),
            Some(ConfigEnvironment::Development)
        );
        assert_eq!(
            ConfigEnvironment::from_str("prod"),
            Some(ConfigEnvironment::Production)
        );
        assert_eq!(
            ConfigEnvironment::from_str("production"),
            Some(ConfigEnvironment::Production)
        );
        assert_eq!(ConfigEnvironment::from_str("invalid"), None);
    }

    #[tokio::test]
    async fn test_threshold_caching() {
        let adapter = ConfigManagerAdapter::new(ConfigManagerAdapterConfig::default());

        let thresholds = DetectionThresholds {
            zscore_threshold: 2.5,
            iqr_threshold_factor: 1.2,
            mad_threshold: 3.0,
            cusum_threshold: 4.5,
            min_confidence: 0.8,
        };

        adapter.consume_thresholds(thresholds.clone()).await.unwrap();

        let cached = adapter.get_thresholds().await;
        assert!((cached.zscore_threshold - 2.5).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_sensitivity_affects_thresholds() {
        let adapter = ConfigManagerAdapter::new(ConfigManagerAdapterConfig::default());

        let thresholds = DetectionThresholds {
            zscore_threshold: 3.0,
            ..Default::default()
        };

        adapter.consume_thresholds(thresholds).await.unwrap();

        // Default (Medium) sensitivity
        let medium = adapter.get_thresholds().await;
        assert!((medium.zscore_threshold - 3.0).abs() < 0.001);

        // High sensitivity should lower threshold
        adapter.consume_sensitivity(SensitivityLevel::High).await.unwrap();
        let high = adapter.get_thresholds().await;
        assert!(high.zscore_threshold < 3.0);

        // Low sensitivity should raise threshold
        adapter.consume_sensitivity(SensitivityLevel::Low).await.unwrap();
        let low = adapter.get_thresholds().await;
        assert!(low.zscore_threshold > 3.0);
    }

    #[tokio::test]
    async fn test_model_params_caching() {
        let adapter = ConfigManagerAdapter::new(ConfigManagerAdapterConfig::default());

        let params = vec![ModelDetectionParams {
            model: "gpt-4".to_string(),
            latency_threshold_multiplier: 1.5,
            token_threshold_multiplier: 1.2,
            cost_threshold_multiplier: 2.0,
            error_rate_threshold: 0.03,
            custom_thresholds: HashMap::new(),
        }];

        adapter.consume_model_params(params).await.unwrap();

        let cached = adapter.get_model_params("gpt-4").await;
        assert!((cached.latency_threshold_multiplier - 1.5).abs() < 0.001);

        // Unknown model should return defaults
        let unknown = adapter.get_model_params("unknown-model").await;
        assert!((unknown.latency_threshold_multiplier - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_effective_threshold() {
        let adapter = ConfigManagerAdapter::new(ConfigManagerAdapterConfig::default());

        let thresholds = DetectionThresholds {
            zscore_threshold: 3.0,
            ..Default::default()
        };
        adapter.consume_thresholds(thresholds).await.unwrap();

        let effective = adapter.get_effective_threshold("zscore", "gpt-4").await;
        assert!((effective - 3.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_raw_config() {
        let adapter = ConfigManagerAdapter::new(ConfigManagerAdapterConfig::default());

        let entries = vec![ConfigEntry {
            key: "sentinel/detection/enabled".to_string(),
            value: ConfigValue::Boolean(true),
            environment: ConfigEnvironment::Production,
            version: 1,
            updated_at: Utc::now(),
            description: Some("Enable detection".to_string()),
        }];

        adapter.consume_raw_config(entries).await.unwrap();

        let value = adapter.get_raw_config("sentinel/detection/enabled").await;
        assert!(value.is_some());
        assert_eq!(value.unwrap().as_bool(), Some(true));
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        let mut adapter = ConfigManagerAdapter::new(ConfigManagerAdapterConfig::default());

        assert!(adapter.health_check().await.is_err());

        adapter.connect().await.unwrap();
        assert!(adapter.health_check().await.is_ok());

        adapter.disconnect().await.unwrap();
        assert!(adapter.health_check().await.is_err());
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let adapter = ConfigManagerAdapter::new(ConfigManagerAdapterConfig::default());

        // Fetch model params (miss)
        let _ = adapter.get_model_params("unknown").await;

        // Add and fetch (hit)
        adapter
            .consume_model_params(vec![ModelDetectionParams::default_for_model("gpt-4")])
            .await
            .unwrap();
        let _ = adapter.get_model_params("gpt-4").await;

        let stats = adapter.stats().await;
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.cache_hits, 1);
    }
}
