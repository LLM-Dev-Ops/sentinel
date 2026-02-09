//! Execution span infrastructure for Agentics integration.
//!
//! This module provides the core types for hierarchical execution tracking
//! as a Foundational Execution Unit within the Agentics execution system.
//!
//! # Invariants
//!
//! The span structure MUST always be:
//! ```text
//! Core
//!   └─ Repo (this repo: sentinel)
//!       └─ Agent (one or more)
//! ```
//!
//! - Every externally-invoked operation MUST have a `parent_span_id` from the Core.
//! - Every agent that executes logic MUST emit its own agent-level span.
//! - Artifacts MUST be attached at agent or repo level only.
//! - Spans are append-only and causally ordered via `parent_span_id`.
//! - If no agent span exists, execution is INVALID.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// The repository name for this execution unit.
pub const REPO_NAME: &str = "sentinel";

// =============================================================================
// EXECUTION CONTEXT
// =============================================================================

/// Execution context passed by the Core to this repo.
///
/// Every externally-invoked operation MUST accept this context.
/// Execution MUST be rejected if `parent_span_id` is missing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Unique identifier for the overall execution.
    pub execution_id: Uuid,
    /// Span ID of the Core-level span that invoked this repo.
    pub parent_span_id: Uuid,
}

// =============================================================================
// SPAN TYPES
// =============================================================================

/// The type of execution span.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    /// Repo-level span: created on entry to this repository.
    Repo,
    /// Agent-level span: created for each agent that executes logic.
    Agent,
}

/// Status of a span's execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpanStatus {
    /// Span is currently executing.
    Running,
    /// Span completed successfully.
    Completed,
    /// Span failed.
    Failed,
}

/// An artifact produced by an agent or repo-level operation.
///
/// Artifacts MUST:
/// - Be attached to the agent-level span that produced them
/// - Include a stable reference (ID, URI, hash, or filename)
/// - Never be attached directly to the Core span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Stable reference identifier (ID, URI, hash, or filename).
    pub reference: String,
    /// Human-readable description of the artifact.
    pub kind: String,
    /// Optional artifact content or summary (JSON-serializable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
}

/// A single execution span within the hierarchical execution graph.
///
/// Spans are append-only and causally ordered via `parent_span_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSpan {
    /// Unique span identifier.
    pub span_id: Uuid,
    /// Parent span ID (Core span for repo, repo span for agents).
    pub parent_span_id: Uuid,
    /// Span type: repo or agent.
    #[serde(rename = "type")]
    pub span_type: SpanType,
    /// Repository name (always "sentinel" for this repo).
    pub repo_name: String,
    /// Agent name (only set for agent-type spans).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    /// Span execution status.
    pub status: SpanStatus,
    /// Start time of the span.
    pub start_time: DateTime<Utc>,
    /// End time of the span (None if still running).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    /// Artifacts produced during this span.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    /// Evidence entries (machine-verifiable, not inferred).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
    /// Failure reasons (populated when status is FAILED).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_reasons: Vec<String>,
}

/// Machine-verifiable evidence attached to a span.
///
/// Evidence MUST NOT be inferred or synthesized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Type of evidence (e.g., "metric", "decision_event", "detection_result").
    pub evidence_type: String,
    /// Stable reference to the evidence source.
    pub reference: String,
    /// The evidence payload.
    pub payload: serde_json::Value,
}

// =============================================================================
// EXECUTION GRAPH
// =============================================================================

/// The complete execution graph output from this repo.
///
/// Contains the repo-level span and all nested agent-level spans.
/// This is the output contract: JSON-serializable without loss.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionGraph {
    /// The execution ID from the invoking Core.
    pub execution_id: Uuid,
    /// The repo-level span.
    pub repo_span: ExecutionSpan,
    /// All agent-level spans nested under the repo span.
    pub agent_spans: Vec<ExecutionSpan>,
}

impl ExecutionGraph {
    /// Validate the execution graph against invariants.
    ///
    /// Returns `Err` with reasons if:
    /// - No agent-level spans were emitted
    /// - Any agent span has wrong parent
    /// - Repo span is not type "repo"
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Repo span must be type "repo"
        if self.repo_span.span_type != SpanType::Repo {
            errors.push("Repo span must have type 'repo'".to_string());
        }

        // Must have at least one agent span
        if self.agent_spans.is_empty() {
            errors.push("No agent-level spans emitted: execution is INVALID".to_string());
        }

        // All agent spans must reference the repo span as parent
        for span in &self.agent_spans {
            if span.span_type != SpanType::Agent {
                errors.push(format!(
                    "Span {} has type {:?} but is in agent_spans",
                    span.span_id, span.span_type
                ));
            }
            if span.parent_span_id != self.repo_span.span_id {
                errors.push(format!(
                    "Agent span {} has parent_span_id {} but repo span is {}",
                    span.span_id, span.parent_span_id, self.repo_span.span_id
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// =============================================================================
// SPAN BUILDER
// =============================================================================

/// Builder for constructing repo-level spans.
pub fn create_repo_span(parent_span_id: Uuid) -> ExecutionSpan {
    ExecutionSpan {
        span_id: Uuid::new_v4(),
        parent_span_id,
        span_type: SpanType::Repo,
        repo_name: REPO_NAME.to_string(),
        agent_name: None,
        status: SpanStatus::Running,
        start_time: Utc::now(),
        end_time: None,
        artifacts: Vec::new(),
        evidence: Vec::new(),
        failure_reasons: Vec::new(),
    }
}

/// Builder for constructing agent-level spans.
pub fn create_agent_span(agent_name: &str, repo_span_id: Uuid) -> ExecutionSpan {
    ExecutionSpan {
        span_id: Uuid::new_v4(),
        parent_span_id: repo_span_id,
        span_type: SpanType::Agent,
        repo_name: REPO_NAME.to_string(),
        agent_name: Some(agent_name.to_string()),
        status: SpanStatus::Running,
        start_time: Utc::now(),
        end_time: None,
        artifacts: Vec::new(),
        evidence: Vec::new(),
        failure_reasons: Vec::new(),
    }
}

impl ExecutionSpan {
    /// Mark this span as completed.
    pub fn complete(&mut self) {
        self.status = SpanStatus::Completed;
        self.end_time = Some(Utc::now());
    }

    /// Mark this span as failed with reasons.
    pub fn fail(&mut self, reasons: Vec<String>) {
        self.status = SpanStatus::Failed;
        self.end_time = Some(Utc::now());
        self.failure_reasons = reasons;
    }

    /// Attach an artifact to this span.
    pub fn attach_artifact(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
    }

    /// Attach evidence to this span.
    pub fn attach_evidence(&mut self, evidence: Evidence) {
        self.evidence.push(evidence);
    }
}

// =============================================================================
// EXECUTION GRAPH COLLECTOR
// =============================================================================

/// Thread-safe collector for building an ExecutionGraph during request processing.
///
/// Shared via `Arc` across handlers and middleware to accumulate agent spans.
#[derive(Debug, Clone)]
pub struct ExecutionGraphCollector {
    /// The execution ID from the Core.
    pub execution_id: Uuid,
    /// The repo-level span (mutable for completion).
    repo_span: Arc<Mutex<ExecutionSpan>>,
    /// Collected agent spans (append-only).
    agent_spans: Arc<Mutex<Vec<ExecutionSpan>>>,
}

impl ExecutionGraphCollector {
    /// Create a new collector from an execution context.
    pub fn new(ctx: &ExecutionContext) -> Self {
        let repo_span = create_repo_span(ctx.parent_span_id);
        Self {
            execution_id: ctx.execution_id,
            repo_span: Arc::new(Mutex::new(repo_span)),
            agent_spans: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get the repo span ID (for creating child agent spans).
    pub fn repo_span_id(&self) -> Uuid {
        self.repo_span.lock().unwrap().span_id
    }

    /// Add a completed agent span.
    pub fn add_agent_span(&self, span: ExecutionSpan) {
        self.agent_spans.lock().unwrap().push(span);
    }

    /// Finalize the execution graph.
    ///
    /// Marks the repo span as completed (or failed if no agent spans exist)
    /// and returns the complete execution graph.
    pub fn finalize(self) -> ExecutionGraph {
        let agent_spans = self.agent_spans.lock().unwrap().clone();
        let mut repo_span = self.repo_span.lock().unwrap().clone();

        if agent_spans.is_empty() {
            repo_span.fail(vec![
                "No agent-level spans emitted: execution is INVALID".to_string(),
            ]);
        } else {
            repo_span.complete();
        }

        ExecutionGraph {
            execution_id: self.execution_id,
            repo_span,
            agent_spans,
        }
    }

    /// Finalize the execution graph with a failure.
    pub fn finalize_failed(self, reasons: Vec<String>) -> ExecutionGraph {
        let agent_spans = self.agent_spans.lock().unwrap().clone();
        let mut repo_span = self.repo_span.lock().unwrap().clone();
        repo_span.fail(reasons);

        ExecutionGraph {
            execution_id: self.execution_id,
            repo_span,
            agent_spans,
        }
    }
}
