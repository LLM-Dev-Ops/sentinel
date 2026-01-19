//! LLM-Sentinel Agents
//!
//! This module contains agent implementations following the LLM-Sentinel
//! Agent Infrastructure Constitution:
//!
//! - Agents are stateless at runtime
//! - All persistence via ruvector-service
//! - No remediation, retry, or orchestration logic
//! - Each invocation emits exactly ONE DecisionEvent
//!
//! # Available Agents
//!
//! - **AnomalyDetectionAgent**: Detects statistically significant deviations
//!   in live telemetry signals relative to established baselines.
//!   Classification: DETECTION
//!
//! - **AlertingAgent**: Evaluates detected anomalies against alerting rules.
//!   Classification: ALERTING
//!
//! - **DriftDetectionAgent**: Detects model and data drift patterns.
//!   Classification: DETECTION
//!
//! - **RootCauseAnalysisAgent**: Performs signal-level root cause analysis
//!   on correlated telemetry signals to identify the most likely sources
//!   contributing to detected incidents.
//!   Classification: ANALYSIS
//!
//! - **IncidentCorrelationAgent**: Correlates multiple anomaly events and alerts
//!   into coherent incident candidates using time-window grouping, service
//!   affinity, and signal type correlation algorithms.
//!   Classification: CORRELATION / ANALYSIS

pub mod alerting;
pub mod anomaly_detection;
pub mod contract;
pub mod drift_agent;
pub mod drift_cli;
pub mod incident_correlation;
pub mod root_cause_analysis;

// Re-export agents
pub use alerting::AlertingAgent;
pub use anomaly_detection::AnomalyDetectionAgent;
pub use drift_agent::DriftDetectionAgent;
pub use drift_cli::{DriftCliContract, DriftCliHandler};
pub use incident_correlation::IncidentCorrelationAgent;
pub use root_cause_analysis::RootCauseAnalysisAgent;

// Re-export core contract types
pub use contract::{
    AgentId, AgentInput, AgentOutput, AgentVersion, ConstraintApplied, DecisionEvent, DecisionType,
};

// Re-export Alerting agent types
pub use contract::{
    AlertEvaluationStatus, AlertingAgentInput, AlertingAgentOutput, AlertingFailureMode,
    AlertingOutputMetadata, AlertingRule, SuppressionConfig,
};

// Re-export RCA agent types
pub use contract::{
    CausalChainLink, EvidenceType, RcaAgentInput, RcaAgentOutput, RcaAnalysisStatus,
    RcaCorrelatedSignal, RcaEvidence, RcaFailureMode, RcaIncidentSignal, RcaOutputMetadata,
    RootCauseCategory, RootCauseHypothesis,
};

// Re-export Incident Correlation agent types
pub use contract::{
    CorrelationFailureMode, CorrelationOutputMetadata, CorrelationSignal, CorrelationTimeWindow,
    IncidentCandidate, IncidentCorrelationInput, IncidentCorrelationOutput, ServiceGroup,
    SignalBreakdown, SignalType,
};

// Re-export agent configs and results
pub use anomaly_detection::{AgentInvocationResult, AgentStats, AnomalyDetectionAgentConfig};
pub use incident_correlation::{
    CorrelationAgentStats, CorrelationInvocationResult, DiagnoseCheck, DiagnoseCheckResult,
    DiagnoseResult, IncidentCorrelationAgentConfig, InspectResult,
};
pub use root_cause_analysis::{
    RcaAgentConfig, RcaAgentStats, RcaDiagnoseCheck, RcaDiagnoseResult, RcaInspectResult,
    RcaInvocationResult,
};
