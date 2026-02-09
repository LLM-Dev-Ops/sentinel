//! Execution context middleware and extractors for Agentics integration.
//!
//! This module enforces the Foundational Execution Unit contract:
//! - Every request MUST provide `X-Execution-Id` and `X-Parent-Span-Id` headers.
//! - Requests missing `X-Parent-Span-Id` are rejected with 400 Bad Request.
//! - A repo-level span is created on entry and threaded through the request.
//! - Handlers create agent-level spans via the `ExecutionGraphCollector`.

use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{header::HeaderName, request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use llm_sentinel_core::execution::{ExecutionContext, ExecutionGraphCollector};
use tracing::{error, info};
use uuid::Uuid;

/// Header name for the execution ID.
pub static HEADER_EXECUTION_ID: HeaderName = HeaderName::from_static("x-execution-id");
/// Header name for the parent span ID (from the Core).
pub static HEADER_PARENT_SPAN_ID: HeaderName = HeaderName::from_static("x-parent-span-id");

/// Axum middleware that extracts execution context from request headers,
/// creates a repo-level span, and injects the `ExecutionGraphCollector`
/// into request extensions.
///
/// # Enforcement
/// - Rejects requests missing `X-Parent-Span-Id` with 400 Bad Request.
/// - This repo MUST NEVER execute silently.
pub async fn execution_context_middleware(
    req: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    // Extract parent_span_id (MANDATORY)
    let parent_span_id = req
        .headers()
        .get(&HEADER_PARENT_SPAN_ID)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok());

    let parent_span_id = match parent_span_id {
        Some(id) => id,
        None => {
            error!("Missing or invalid X-Parent-Span-Id header: execution rejected");
            let body = serde_json::json!({
                "error": "EXECUTION_CONTEXT_REQUIRED",
                "message": "X-Parent-Span-Id header is required. This repository is a Foundational Execution Unit and MUST NOT execute without a valid parent span from the Core.",
                "required_headers": {
                    "X-Parent-Span-Id": "UUID of the Core-level span (REQUIRED)",
                    "X-Execution-Id": "UUID of the overall execution (optional, auto-generated if absent)"
                }
            });
            return Err((StatusCode::BAD_REQUEST, Json(body)).into_response());
        }
    };

    // Extract execution_id (optional - generate if missing)
    let execution_id = req
        .headers()
        .get(&HEADER_EXECUTION_ID)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4);

    let ctx = ExecutionContext {
        execution_id,
        parent_span_id,
    };

    info!(
        execution_id = %ctx.execution_id,
        parent_span_id = %ctx.parent_span_id,
        "Execution context established, repo-level span created"
    );

    // Create the collector and inject into request extensions
    let collector = ExecutionGraphCollector::new(&ctx);

    let mut req = req;
    req.extensions_mut().insert(collector);

    let response = next.run(req).await;

    Ok(response)
}

/// Axum extractor for the `ExecutionGraphCollector`.
///
/// Handlers use this to:
/// 1. Get the repo span ID for creating child agent spans
/// 2. Add completed agent spans to the collector
/// 3. Finalize the execution graph for the response
///
/// # Example
/// ```ignore
/// async fn my_handler(
///     collector: ExecutionCollector,
///     // ...
/// ) -> impl IntoResponse {
///     let repo_span_id = collector.repo_span_id();
///     let mut agent_span = create_agent_span("my_agent", repo_span_id);
///     // ... do work ...
///     agent_span.complete();
///     collector.add_agent_span(agent_span);
///     let graph = collector.finalize();
///     // ... return response with graph ...
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ExecutionCollector(pub ExecutionGraphCollector);

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for ExecutionCollector {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ExecutionGraphCollector>()
            .cloned()
            .map(ExecutionCollector)
            .ok_or_else(|| {
                let body = serde_json::json!({
                    "error": "EXECUTION_CONTEXT_MISSING",
                    "message": "ExecutionGraphCollector not found in request extensions. Ensure execution_context_middleware is applied."
                });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(body))
            })
    }
}
