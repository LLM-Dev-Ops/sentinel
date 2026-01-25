//! Phase 2 Startup Hardening
//!
//! Enforces required environment variables and validates configuration
//! before agent initialization. Fails fast if Ruvector is unavailable.

use std::env;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};

use super::ruvector::{RuvectorClient, RuvectorError};

/// Errors that can occur during Phase 2 startup
#[derive(Debug, Error)]
pub enum StartupError {
    /// Missing required environment variable
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),

    /// Invalid environment variable value
    #[error("Invalid value for {var}: {message}")]
    InvalidEnvVar { var: String, message: String },

    /// Ruvector initialization failed
    #[error("Ruvector initialization failed: {0}")]
    RuvectorInit(#[from] RuvectorError),

    /// Phase/layer mismatch
    #[error("Agent phase/layer mismatch: expected {expected}, got {actual}")]
    PhaseMismatch { expected: String, actual: String },
}

/// Phase 2 configuration loaded from environment
#[derive(Debug, Clone)]
pub struct Phase2Config {
    /// Ruvector service URL
    pub ruvector_service_url: String,
    /// Ruvector API key (from Secret Manager)
    pub ruvector_api_key: String,
    /// Agent name identifier
    pub agent_name: String,
    /// Agent domain (e.g., "detection", "correlation")
    pub agent_domain: String,
    /// Agent phase (must be "phase2")
    pub agent_phase: String,
    /// Agent layer (must be "layer1")
    pub agent_layer: String,
    /// Optional dry-run mode
    pub dry_run: bool,
}

impl Phase2Config {
    /// Required environment variables for Phase 2 agents
    const REQUIRED_VARS: &'static [&'static str] = &[
        "RUVECTOR_SERVICE_URL",
        "RUVECTOR_API_KEY",
        "AGENT_NAME",
        "AGENT_DOMAIN",
        "AGENT_PHASE",
        "AGENT_LAYER",
    ];

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, StartupError> {
        // Check all required variables first
        for var in Self::REQUIRED_VARS {
            if env::var(var).is_err() {
                return Err(StartupError::MissingEnvVar(var.to_string()));
            }
        }

        let agent_phase = env::var("AGENT_PHASE").unwrap();
        let agent_layer = env::var("AGENT_LAYER").unwrap();

        // Validate phase and layer
        if agent_phase != "phase2" {
            return Err(StartupError::PhaseMismatch {
                expected: "phase2".to_string(),
                actual: agent_phase,
            });
        }

        if agent_layer != "layer1" {
            return Err(StartupError::PhaseMismatch {
                expected: "layer1".to_string(),
                actual: agent_layer,
            });
        }

        Ok(Self {
            ruvector_service_url: env::var("RUVECTOR_SERVICE_URL").unwrap(),
            ruvector_api_key: env::var("RUVECTOR_API_KEY").unwrap(),
            agent_name: env::var("AGENT_NAME").unwrap(),
            agent_domain: env::var("AGENT_DOMAIN").unwrap(),
            agent_phase,
            agent_layer,
            dry_run: env::var("DRY_RUN")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        })
    }
}

/// Phase 2 startup orchestrator
pub struct Phase2Startup {
    config: Phase2Config,
    ruvector_client: RuvectorClient,
}

impl Phase2Startup {
    /// Initialize Phase 2 agent with full validation
    ///
    /// This function:
    /// 1. Loads and validates environment configuration
    /// 2. Initializes Ruvector client
    /// 3. Verifies Ruvector connectivity (hard failure if unavailable)
    ///
    /// # Errors
    /// Returns `StartupError` if any validation or initialization fails.
    /// Agents MUST NOT start if this function returns an error.
    pub async fn init() -> Result<Self, StartupError> {
        info!("Phase 2 Layer 1 startup initiated");

        // Load configuration from environment
        let config = Phase2Config::from_env()?;
        info!(
            agent_name = %config.agent_name,
            agent_domain = %config.agent_domain,
            phase = %config.agent_phase,
            layer = %config.agent_layer,
            "Configuration loaded"
        );

        // Initialize Ruvector client
        let ruvector_client = RuvectorClient::new(
            &config.ruvector_service_url,
            &config.ruvector_api_key,
        )?;

        // CRITICAL: Verify Ruvector connectivity - hard failure if unavailable
        info!("Verifying Ruvector connectivity...");
        match ruvector_client.health_check().await {
            Ok(healthy) if healthy => {
                info!("Ruvector health check passed");
            }
            Ok(_) => {
                error!("Ruvector health check returned unhealthy status");
                return Err(StartupError::RuvectorInit(RuvectorError::Unhealthy));
            }
            Err(e) => {
                error!(error = %e, "Ruvector health check failed - HARD FAILURE");
                return Err(StartupError::RuvectorInit(e));
            }
        }

        info!(
            "Phase 2 Layer 1 startup complete for agent: {}",
            config.agent_name
        );

        Ok(Self {
            config,
            ruvector_client,
        })
    }

    /// Initialize with custom timeout for Ruvector verification
    pub async fn init_with_timeout(timeout: Duration) -> Result<Self, StartupError> {
        info!("Phase 2 Layer 1 startup initiated (timeout: {:?})", timeout);

        let config = Phase2Config::from_env()?;
        info!(
            agent_name = %config.agent_name,
            agent_domain = %config.agent_domain,
            "Configuration loaded"
        );

        let ruvector_client = RuvectorClient::with_timeout(
            &config.ruvector_service_url,
            &config.ruvector_api_key,
            timeout,
        )?;

        // Verify connectivity with timeout
        match tokio::time::timeout(timeout, ruvector_client.health_check()).await {
            Ok(Ok(true)) => {
                info!("Ruvector health check passed");
            }
            Ok(Ok(false)) => {
                error!("Ruvector health check returned unhealthy");
                return Err(StartupError::RuvectorInit(RuvectorError::Unhealthy));
            }
            Ok(Err(e)) => {
                error!(error = %e, "Ruvector health check failed");
                return Err(StartupError::RuvectorInit(e));
            }
            Err(_) => {
                error!("Ruvector health check timed out after {:?}", timeout);
                return Err(StartupError::RuvectorInit(RuvectorError::Timeout));
            }
        }

        info!("Phase 2 Layer 1 startup complete");

        Ok(Self {
            config,
            ruvector_client,
        })
    }

    /// Get the loaded configuration
    pub fn config(&self) -> &Phase2Config {
        &self.config
    }

    /// Get the Ruvector client
    pub fn ruvector(&self) -> &RuvectorClient {
        &self.ruvector_client
    }

    /// Get agent name
    pub fn agent_name(&self) -> &str {
        &self.config.agent_name
    }

    /// Get agent domain
    pub fn agent_domain(&self) -> &str {
        &self.config.agent_domain
    }

    /// Check if dry-run mode is enabled
    pub fn is_dry_run(&self) -> bool {
        self.config.dry_run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_env_var_error() {
        // Clear any existing vars
        for var in Phase2Config::REQUIRED_VARS {
            env::remove_var(var);
        }

        let result = Phase2Config::from_env();
        assert!(matches!(result, Err(StartupError::MissingEnvVar(_))));
    }

    #[test]
    fn test_phase_mismatch_error() {
        env::set_var("RUVECTOR_SERVICE_URL", "http://localhost:8080");
        env::set_var("RUVECTOR_API_KEY", "test-key");
        env::set_var("AGENT_NAME", "test-agent");
        env::set_var("AGENT_DOMAIN", "detection");
        env::set_var("AGENT_PHASE", "phase1"); // Wrong phase
        env::set_var("AGENT_LAYER", "layer1");

        let result = Phase2Config::from_env();
        assert!(matches!(result, Err(StartupError::PhaseMismatch { .. })));

        // Cleanup
        for var in Phase2Config::REQUIRED_VARS {
            env::remove_var(var);
        }
    }

    #[test]
    fn test_valid_config() {
        env::set_var("RUVECTOR_SERVICE_URL", "http://localhost:8080");
        env::set_var("RUVECTOR_API_KEY", "test-key");
        env::set_var("AGENT_NAME", "test-agent");
        env::set_var("AGENT_DOMAIN", "detection");
        env::set_var("AGENT_PHASE", "phase2");
        env::set_var("AGENT_LAYER", "layer1");

        let result = Phase2Config::from_env();
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.agent_name, "test-agent");
        assert_eq!(config.agent_domain, "detection");
        assert_eq!(config.agent_phase, "phase2");
        assert_eq!(config.agent_layer, "layer1");

        // Cleanup
        for var in Phase2Config::REQUIRED_VARS {
            env::remove_var(var);
        }
    }
}
