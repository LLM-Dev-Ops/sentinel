//! # Sentinel API
//!
//! REST API server for LLM-Sentinel.
//!
//! This crate provides:
//! - Health check endpoints
//! - Metrics export (Prometheus)
//! - Telemetry query API
//! - Anomaly query API
//! - Real-time anomaly stream (WebSocket)
//! - Execution span infrastructure for Agentics integration

#![warn(missing_debug_implementations, rust_2018_idioms, unreachable_pub)]

pub mod execution;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod server;

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// API server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Server bind address
    pub bind_addr: SocketAddr,
    /// Enable CORS
    pub enable_cors: bool,
    /// CORS allowed origins
    pub cors_origins: Vec<String>,
    /// Request timeout (seconds)
    pub timeout_secs: u64,
    /// Maximum request body size (bytes)
    pub max_body_size: usize,
    /// Enable request logging
    pub enable_logging: bool,
    /// Metrics endpoint path
    pub metrics_path: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".parse().unwrap(),
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
            timeout_secs: 30,
            max_body_size: 10 * 1024 * 1024, // 10MB
            enable_logging: true,
            metrics_path: "/metrics".to_string(),
        }
    }
}

/// API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
    /// Optional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Success response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse<T> {
    /// Response data
    pub data: T,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ResponseMetadata>,
}

impl<T> SuccessResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: ResponseMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Response metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    /// Total count (for paginated responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<usize>,
    /// Current page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    /// Page size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<usize>,
}

/// Response wrapper that includes the execution graph.
///
/// All agent-invoking endpoints MUST return this type to satisfy the
/// Agentics output contract. The execution graph contains the repo-level
/// span and all nested agent-level spans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentedResponse<T> {
    /// Response data from the handler
    pub data: T,
    /// The execution graph with repo and agent spans
    pub execution: llm_sentinel_core::execution::ExecutionGraph,
}

/// Re-export commonly used types
pub mod prelude {
    pub use crate::execution::{ExecutionCollector, execution_context_middleware};
    pub use crate::handlers::*;
    pub use crate::routes::{create_router, create_router_with_alerting};
    pub use crate::server::ApiServer;
    pub use crate::{ApiConfig, ErrorResponse, InstrumentedResponse, SuccessResponse};
}
