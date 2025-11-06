//! HTTP middleware for logging, CORS, and error handling.

use axum::{
    http::{header, Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, warn};

/// Create CORS middleware
pub fn cors_middleware(origins: Vec<String>) -> CorsLayer {
    if origins.contains(&"*".to_string()) {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    } else {
        let allowed_origins: Vec<_> = origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();

        CorsLayer::new()
            .allow_origin(allowed_origins)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    }
}

/// Request logging middleware
pub async fn logging_middleware(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let method = req.method().clone();
    let uri = req.uri().clone();

    debug!("Incoming request: {} {}", method, uri);

    let response = next.run(req).await;

    debug!("Response: {} {} - {}", method, uri, response.status());

    Ok(response)
}

/// Error handling middleware
pub async fn error_handling_middleware(
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let response = next.run(req).await;

    if response.status().is_server_error() {
        warn!("Server error: {}", response.status());
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_wildcard() {
        let cors = cors_middleware(vec!["*".to_string()]);
        // Just test that it creates without panicking
        drop(cors);
    }

    #[test]
    fn test_cors_specific_origins() {
        let cors = cors_middleware(vec![
            "https://example.com".to_string(),
            "https://app.example.com".to_string(),
        ]);
        drop(cors);
    }
}
