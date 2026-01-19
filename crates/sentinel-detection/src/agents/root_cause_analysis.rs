//! Root Cause Analysis Agent for LLM-Sentinel
//!
//! This agent performs signal-level root cause analysis on correlated telemetry
//! signals to identify the most likely sources contributing to detected incidents.
//!
//! # Constitutional Compliance
//!
//! This agent adheres to the LLM-Sentinel Agent Infrastructure Constitution:
//! - Classification: ANALYSIS
//! - Stateless at runtime
//! - No remediation, retry, or orchestration logic
//! - Emits exactly ONE DecisionEvent per invocation
//! - Persists only via ruvector-service client
//!
//! # Analysis Methods
//! - Temporal correlation (identifying leading indicators)
//! - Service dependency analysis (upstream/downstream impact)
//! - Metric correlation (identifying co-moving signals)
//! - Historical pattern matching (similar past incidents)
//!
//! # Usage
//!
//! ```rust,ignore
//! use sentinel_detection::agents::RootCauseAnalysisAgent;
//!
//! let agent = RootCauseAnalysisAgent::new(config)?;
//! let result = agent.invoke(input).await?;
//! // result.decision_event is persisted to ruvector-service
//! ```

use crate::agents::contract::{
    AgentId, AgentOutput, AgentVersion, CausalChainLink, ConstraintApplied, ConstraintType,
    DecisionContext, DecisionEvent, DecisionType, EvidenceType, ExecutionRef, OutputMetadata,
    RcaAgentInput, RcaAgentOutput, RcaAnalysisStatus, RcaCorrelatedSignal, RcaEvidence,
    RcaFailureMode, RcaIncidentSignal, RcaOutputMetadata, RootCauseCategory, RootCauseHypothesis,
};
use chrono::{DateTime, Utc};
use llm_sentinel_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Root Cause Analysis Agent Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaAgentConfig {
    /// Enable temporal correlation analysis
    pub enable_temporal_correlation: bool,
    /// Minimum correlation strength to consider (0.0 - 1.0)
    pub min_correlation_strength: f64,

    /// Enable service dependency analysis
    pub enable_dependency_analysis: bool,
    /// Service dependency weight in hypothesis scoring
    pub dependency_weight: f64,

    /// Enable metric correlation analysis
    pub enable_metric_correlation: bool,
    /// Minimum metric correlation coefficient
    pub min_metric_correlation: f64,

    /// Enable historical pattern matching
    pub enable_historical_patterns: bool,
    /// Historical pattern match threshold
    pub pattern_match_threshold: f64,

    /// Maximum time lag to consider (seconds)
    pub max_lag_seconds: i64,
    /// Minimum signals required for analysis
    pub min_signals: usize,
    /// Maximum number of hypotheses to return
    pub max_hypotheses: usize,

    /// Confidence threshold for "root cause identified" status
    pub confidence_threshold: f64,

    /// Analysis timeout (milliseconds)
    pub timeout_ms: u64,

    /// ruvector-service endpoint (for persistence)
    pub ruvector_endpoint: Option<String>,

    /// Dry-run mode (don't persist DecisionEvents)
    pub dry_run: bool,
}

impl Default for RcaAgentConfig {
    fn default() -> Self {
        Self {
            enable_temporal_correlation: true,
            min_correlation_strength: 0.5,
            enable_dependency_analysis: true,
            dependency_weight: 0.3,
            enable_metric_correlation: true,
            min_metric_correlation: 0.6,
            enable_historical_patterns: false, // Requires external data source
            pattern_match_threshold: 0.7,
            max_lag_seconds: 300, // 5 minutes
            min_signals: 2,
            max_hypotheses: 5,
            confidence_threshold: 0.7,
            timeout_ms: 30000, // 30 seconds
            ruvector_endpoint: None,
            dry_run: false,
        }
    }
}

/// RCA Agent invocation result
#[derive(Debug, Clone)]
pub struct RcaInvocationResult {
    /// The decision event (ALWAYS emitted)
    pub decision_event: DecisionEvent,
    /// RCA output with hypotheses
    pub rca_output: RcaAgentOutput,
    /// Failure mode if any
    pub failure: Option<RcaFailureMode>,
    /// Was the DecisionEvent persisted?
    pub persisted: bool,
}

/// Agent statistics
#[derive(Debug, Clone, Default)]
pub struct RcaAgentStats {
    /// Total invocations
    pub invocations: u64,
    /// Root causes identified (high confidence)
    pub root_causes_identified: u64,
    /// Inconclusive analyses
    pub inconclusive: u64,
    /// Decisions persisted
    pub decisions_persisted: u64,
    /// Failures encountered
    pub failures: u64,
    /// Average processing time (ms)
    pub avg_processing_ms: f64,
}

/// Root Cause Analysis Agent
///
/// This is the main agent implementation that:
/// 1. Accepts correlated anomaly signals
/// 2. Runs configured analysis methods
/// 3. Produces ranked root cause hypotheses
/// 4. Emits exactly ONE DecisionEvent per invocation
/// 5. Persists to ruvector-service
pub struct RootCauseAnalysisAgent {
    /// Agent configuration
    config: RcaAgentConfig,
    /// Agent ID
    agent_id: AgentId,
    /// Agent version
    agent_version: AgentVersion,
    /// Agent statistics
    stats: Arc<RwLock<RcaAgentStats>>,
}

impl std::fmt::Debug for RootCauseAnalysisAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RootCauseAnalysisAgent")
            .field("agent_id", &self.agent_id)
            .field("agent_version", &self.agent_version)
            .field("config", &self.config)
            .finish()
    }
}

impl RootCauseAnalysisAgent {
    /// Create a new Root Cause Analysis Agent
    pub fn new(config: RcaAgentConfig) -> Result<Self> {
        // Validate configuration
        if !config.enable_temporal_correlation
            && !config.enable_dependency_analysis
            && !config.enable_metric_correlation
            && !config.enable_historical_patterns
        {
            return Err(Error::config("At least one analysis method must be enabled"));
        }

        if config.min_signals < 1 {
            return Err(Error::config("min_signals must be at least 1"));
        }

        if !(0.0..=1.0).contains(&config.min_correlation_strength) {
            return Err(Error::config(
                "min_correlation_strength must be between 0.0 and 1.0",
            ));
        }

        info!(
            agent_id = %AgentId::root_cause_analysis(),
            version = %AgentVersion::current(),
            "Creating Root Cause Analysis Agent"
        );

        Ok(Self {
            agent_id: AgentId::root_cause_analysis(),
            agent_version: AgentVersion::current(),
            config,
            stats: Arc::new(RwLock::new(RcaAgentStats::default())),
        })
    }

    /// Create with default configuration
    pub fn with_defaults() -> Result<Self> {
        Self::new(RcaAgentConfig::default())
    }

    /// Invoke the agent with correlated signals
    ///
    /// This is the main entry point for the agent. It:
    /// 1. Validates the input
    /// 2. Runs all enabled analysis methods
    /// 3. Produces ranked root cause hypotheses
    /// 4. Creates exactly ONE DecisionEvent
    /// 5. Persists to ruvector-service (unless dry-run)
    pub async fn invoke(&self, input: &RcaAgentInput) -> Result<RcaInvocationResult> {
        let start = Instant::now();

        info!(
            agent_id = %self.agent_id,
            request_id = %input.request_id,
            incident_id = %input.primary_incident.incident_id,
            service = %input.primary_incident.service_name,
            signals_count = input.correlated_signals.len(),
            "RCA Agent invocation started"
        );

        // Validate input
        if let Err(failure) = self.validate_input(input) {
            return self.create_failure_result(input, failure, start).await;
        }

        // Track constraints applied
        let mut constraints_applied = Vec::new();
        let mut methods_applied = Vec::new();
        let mut correlations_computed = 0usize;
        let mut dependency_paths_analyzed = 0usize;

        // Compute inputs hash
        let inputs_hash = input.compute_hash();

        // Collect all hypotheses from analysis methods
        let mut all_hypotheses: Vec<RootCauseHypothesis> = Vec::new();

        // 1. Temporal Correlation Analysis
        if self.config.enable_temporal_correlation {
            methods_applied.push("temporal_correlation".to_string());
            constraints_applied.push(ConstraintApplied {
                constraint_id: "analysis:temporal_correlation".to_string(),
                constraint_type: ConstraintType::Analysis,
                value: serde_json::json!(true),
                passed: true,
                description: Some("Temporal correlation analysis enabled".to_string()),
            });
            constraints_applied.push(ConstraintApplied {
                constraint_id: "min_correlation_strength".to_string(),
                constraint_type: ConstraintType::Threshold,
                value: serde_json::json!(self.config.min_correlation_strength),
                passed: true,
                description: Some(format!(
                    "Minimum correlation strength: {}",
                    self.config.min_correlation_strength
                )),
            });

            let (hypotheses, correlations) = self.analyze_temporal_correlation(input).await;
            correlations_computed += correlations;
            all_hypotheses.extend(hypotheses);
        }

        // 2. Service Dependency Analysis
        if self.config.enable_dependency_analysis {
            methods_applied.push("service_dependency".to_string());
            constraints_applied.push(ConstraintApplied {
                constraint_id: "analysis:service_dependency".to_string(),
                constraint_type: ConstraintType::Analysis,
                value: serde_json::json!(true),
                passed: true,
                description: Some("Service dependency analysis enabled".to_string()),
            });
            constraints_applied.push(ConstraintApplied {
                constraint_id: "dependency_weight".to_string(),
                constraint_type: ConstraintType::Analysis,
                value: serde_json::json!(self.config.dependency_weight),
                passed: true,
                description: Some(format!(
                    "Dependency weight in scoring: {}",
                    self.config.dependency_weight
                )),
            });

            let (hypotheses, paths) = self.analyze_service_dependencies(input).await;
            dependency_paths_analyzed += paths;
            all_hypotheses.extend(hypotheses);
        }

        // 3. Metric Correlation Analysis
        if self.config.enable_metric_correlation {
            methods_applied.push("metric_correlation".to_string());
            constraints_applied.push(ConstraintApplied {
                constraint_id: "analysis:metric_correlation".to_string(),
                constraint_type: ConstraintType::Analysis,
                value: serde_json::json!(true),
                passed: true,
                description: Some("Metric correlation analysis enabled".to_string()),
            });
            constraints_applied.push(ConstraintApplied {
                constraint_id: "min_metric_correlation".to_string(),
                constraint_type: ConstraintType::Threshold,
                value: serde_json::json!(self.config.min_metric_correlation),
                passed: true,
                description: Some(format!(
                    "Minimum metric correlation: {}",
                    self.config.min_metric_correlation
                )),
            });

            let (hypotheses, correlations) = self.analyze_metric_correlations(input).await;
            correlations_computed += correlations;
            all_hypotheses.extend(hypotheses);
        }

        // 4. Historical Pattern Matching (if enabled)
        if self.config.enable_historical_patterns {
            methods_applied.push("historical_patterns".to_string());
            constraints_applied.push(ConstraintApplied {
                constraint_id: "analysis:historical_patterns".to_string(),
                constraint_type: ConstraintType::Analysis,
                value: serde_json::json!(true),
                passed: true,
                description: Some("Historical pattern matching enabled".to_string()),
            });

            let hypotheses = self.analyze_historical_patterns(input).await;
            all_hypotheses.extend(hypotheses);
        }

        // Add temporal constraints
        constraints_applied.push(ConstraintApplied {
            constraint_id: "max_lag_seconds".to_string(),
            constraint_type: ConstraintType::Temporal,
            value: serde_json::json!(self.config.max_lag_seconds),
            passed: true,
            description: Some(format!(
                "Maximum time lag: {}s",
                self.config.max_lag_seconds
            )),
        });

        // Merge and rank hypotheses
        let ranked_hypotheses = self.merge_and_rank_hypotheses(all_hypotheses);

        // Determine analysis status
        let (status, root_cause_identified, primary_hypothesis) =
            self.determine_status(&ranked_hypotheses);

        let processing_ms = start.elapsed().as_millis() as u64;

        // Calculate time window analyzed
        let time_window_seconds = self.calculate_time_window(input);

        // Create RCA output
        let rca_output = RcaAgentOutput {
            root_cause_identified,
            status,
            hypotheses: ranked_hypotheses.clone(),
            primary_hypothesis: primary_hypothesis.clone(),
            signals_analyzed: input.correlated_signals.len(),
            methods_applied: methods_applied.clone(),
            summary: self.generate_summary(&status, &primary_hypothesis, input),
            metadata: RcaOutputMetadata {
                processing_ms,
                correlations_computed,
                dependency_paths_analyzed,
                time_window_seconds,
                telemetry_emitted: true,
            },
        };

        // Create agent output (adapting RCA output to generic agent output)
        let agent_output = AgentOutput {
            anomaly_detected: root_cause_identified, // Reusing field for "analysis found something"
            anomaly_event: primary_hypothesis
                .as_ref()
                .map(|h| serde_json::to_value(h).unwrap_or_default()),
            detectors_evaluated: methods_applied,
            triggered_by: primary_hypothesis
                .as_ref()
                .map(|h| h.category.to_string()),
            metadata: OutputMetadata {
                processing_ms,
                baselines_checked: correlations_computed,
                baselines_valid: dependency_paths_analyzed,
                telemetry_emitted: true,
            },
        };

        // Calculate overall confidence
        let confidence = ranked_hypotheses
            .first()
            .map(|h| h.confidence)
            .unwrap_or(0.0);

        // Create DecisionEvent (EXACTLY ONE per invocation)
        let decision_event = DecisionEvent::new(
            self.agent_id.clone(),
            self.agent_version.clone(),
            DecisionType::RootCauseAnalysis,
            inputs_hash,
            agent_output,
            confidence,
            constraints_applied,
            ExecutionRef {
                request_id: input.request_id,
                trace_id: input
                    .primary_incident
                    .context
                    .get("trace_id")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                span_id: None,
                source: input.source.clone(),
            },
            DecisionContext {
                service_name: input.primary_incident.service_name.clone(),
                model: input.primary_incident.model.clone().unwrap_or_default(),
                region: input
                    .primary_incident
                    .context
                    .get("region")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                environment: input
                    .primary_incident
                    .context
                    .get("environment")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            },
        );

        // Validate decision event
        if let Err(errors) = decision_event.validate() {
            error!(
                agent_id = %self.agent_id,
                errors = ?errors,
                "DecisionEvent validation failed"
            );
            return Err(Error::validation(errors.join("; ")));
        }

        // Persist to ruvector-service (unless dry-run)
        let persisted = if !self.config.dry_run {
            self.persist_decision(&decision_event).await?
        } else {
            debug!("Dry-run mode: skipping persistence");
            false
        };

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.invocations += 1;
            if root_cause_identified {
                stats.root_causes_identified += 1;
            } else if status == RcaAnalysisStatus::Inconclusive {
                stats.inconclusive += 1;
            }
            if persisted {
                stats.decisions_persisted += 1;
            }
            // Update running average of processing time
            let total_time =
                stats.avg_processing_ms * (stats.invocations - 1) as f64 + processing_ms as f64;
            stats.avg_processing_ms = total_time / stats.invocations as f64;
        }

        // Emit telemetry
        self.emit_telemetry(&decision_event, &rca_output, processing_ms)
            .await;

        info!(
            agent_id = %self.agent_id,
            request_id = %input.request_id,
            status = %status,
            hypotheses_count = ranked_hypotheses.len(),
            processing_ms = processing_ms,
            persisted = persisted,
            "RCA Agent invocation completed"
        );

        Ok(RcaInvocationResult {
            decision_event,
            rca_output,
            failure: None,
            persisted,
        })
    }

    /// Invoke the agent from a raw HTTP request (Edge Function handler)
    pub async fn handle_request(&self, body: &[u8]) -> Result<serde_json::Value> {
        // Parse request body
        let input: RcaAgentInput = serde_json::from_slice(body)
            .map_err(|e| Error::validation(format!("Invalid request body: {}", e)))?;

        // Invoke agent
        let result = self.invoke(&input).await?;

        // Return both decision event and RCA output
        Ok(serde_json::json!({
            "decision_event": result.decision_event.to_json(),
            "rca_output": serde_json::to_value(&result.rca_output).unwrap_or_default(),
            "persisted": result.persisted
        }))
    }

    // =========================================================================
    // Validation
    // =========================================================================

    fn validate_input(&self, input: &RcaAgentInput) -> std::result::Result<(), RcaFailureMode> {
        // Check minimum signals
        if input.correlated_signals.len() < self.config.min_signals {
            return Err(RcaFailureMode::InsufficientSignals {
                required: self.config.min_signals,
                provided: input.correlated_signals.len(),
            });
        }

        // Check temporal gap
        let max_lag = input
            .correlated_signals
            .iter()
            .map(|s| s.lag_seconds.abs())
            .max()
            .unwrap_or(0);

        if max_lag > self.config.max_lag_seconds {
            return Err(RcaFailureMode::TemporalGapTooLarge {
                max_allowed_seconds: self.config.max_lag_seconds as u64,
                actual_seconds: max_lag as u64,
            });
        }

        Ok(())
    }

    // =========================================================================
    // Analysis Methods
    // =========================================================================

    /// Analyze temporal correlations to find leading indicators
    async fn analyze_temporal_correlation(
        &self,
        input: &RcaAgentInput,
    ) -> (Vec<RootCauseHypothesis>, usize) {
        let mut hypotheses = Vec::new();
        let mut correlations_computed = 0;

        // Group signals by service
        let mut signals_by_service: HashMap<&str, Vec<&RcaCorrelatedSignal>> = HashMap::new();
        for signal in &input.correlated_signals {
            signals_by_service
                .entry(&signal.service_name)
                .or_default()
                .push(signal);
        }

        // Find leading indicators (signals that preceded the incident)
        for (service, signals) in &signals_by_service {
            // Find the earliest signal that preceded the incident
            let leading_signals: Vec<_> = signals
                .iter()
                .filter(|s| s.lag_seconds < 0) // Before incident
                .filter(|s| s.correlation >= self.config.min_correlation_strength)
                .collect();

            correlations_computed += signals.len();

            if !leading_signals.is_empty() {
                // Find the strongest correlated leading signal
                if let Some(best_signal) = leading_signals
                    .iter()
                    .max_by(|a, b| a.correlation.partial_cmp(&b.correlation).unwrap())
                {
                    let confidence = self.calculate_temporal_confidence(best_signal, input);

                    if confidence >= self.config.min_correlation_strength {
                        let hypothesis = self.create_temporal_hypothesis(
                            best_signal,
                            service,
                            confidence,
                            input,
                        );
                        hypotheses.push(hypothesis);
                    }
                }
            }
        }

        (hypotheses, correlations_computed)
    }

    /// Analyze service dependencies to find upstream causes
    async fn analyze_service_dependencies(
        &self,
        input: &RcaAgentInput,
    ) -> (Vec<RootCauseHypothesis>, usize) {
        let mut hypotheses = Vec::new();
        let mut paths_analyzed = 0;

        // If we have a dependency graph, use it
        if let Some(ref deps) = input.service_dependencies {
            // Extract upstream services from dependency graph
            if let Some(upstream) = deps.get("upstream") {
                if let Some(upstream_arr) = upstream.as_array() {
                    for upstream_service in upstream_arr {
                        paths_analyzed += 1;

                        if let Some(service_name) = upstream_service.as_str() {
                            // Check if we have signals from this upstream service
                            let upstream_signals: Vec<_> = input
                                .correlated_signals
                                .iter()
                                .filter(|s| s.service_name == service_name)
                                .filter(|s| s.lag_seconds < 0) // Before incident
                                .collect();

                            if !upstream_signals.is_empty() {
                                let best_signal = upstream_signals
                                    .iter()
                                    .max_by(|a, b| {
                                        a.correlation.partial_cmp(&b.correlation).unwrap()
                                    })
                                    .unwrap();

                                let confidence =
                                    self.calculate_dependency_confidence(best_signal, deps);

                                if confidence >= self.config.min_correlation_strength {
                                    let hypothesis = self.create_dependency_hypothesis(
                                        best_signal,
                                        service_name,
                                        confidence,
                                        input,
                                    );
                                    hypotheses.push(hypothesis);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Without explicit dependencies, infer from signal patterns
            // Services with earlier signals that correlate strongly might be upstream
            for signal in &input.correlated_signals {
                if signal.lag_seconds < -30 && signal.correlation >= 0.7 {
                    paths_analyzed += 1;
                    let confidence = signal.correlation * 0.8; // Reduced confidence without explicit deps

                    let hypothesis = self.create_inferred_dependency_hypothesis(
                        signal,
                        confidence,
                        input,
                    );
                    hypotheses.push(hypothesis);
                }
            }
        }

        (hypotheses, paths_analyzed)
    }

    /// Analyze metric correlations
    async fn analyze_metric_correlations(
        &self,
        input: &RcaAgentInput,
    ) -> (Vec<RootCauseHypothesis>, usize) {
        let mut hypotheses = Vec::new();
        let mut correlations_computed = 0;

        // Group signals by metric type
        let mut signals_by_metric: HashMap<&str, Vec<&RcaCorrelatedSignal>> = HashMap::new();
        for signal in &input.correlated_signals {
            signals_by_metric
                .entry(&signal.metric)
                .or_default()
                .push(signal);
        }

        // Look for resource-related metrics that might indicate root cause
        let resource_metrics = ["cpu_percent", "memory_percent", "disk_io", "network_io"];
        for metric in &resource_metrics {
            if let Some(signals) = signals_by_metric.get(metric) {
                correlations_computed += signals.len();

                // Find signals with high correlation and preceding the incident
                let causal_signals: Vec<_> = signals
                    .iter()
                    .filter(|s| s.correlation >= self.config.min_metric_correlation)
                    .filter(|s| s.lag_seconds < 0)
                    .collect();

                if let Some(best_signal) = causal_signals
                    .iter()
                    .max_by(|a, b| a.correlation.partial_cmp(&b.correlation).unwrap())
                {
                    let confidence = best_signal.correlation;
                    let hypothesis =
                        self.create_resource_hypothesis(best_signal, metric, confidence, input);
                    hypotheses.push(hypothesis);
                }
            }
        }

        (hypotheses, correlations_computed)
    }

    /// Analyze historical patterns
    async fn analyze_historical_patterns(
        &self,
        _input: &RcaAgentInput,
    ) -> Vec<RootCauseHypothesis> {
        // Historical pattern matching requires external data source
        // This is a placeholder for future implementation
        debug!("Historical pattern matching not yet implemented");
        Vec::new()
    }

    // =========================================================================
    // Hypothesis Creation
    // =========================================================================

    fn create_temporal_hypothesis(
        &self,
        signal: &RcaCorrelatedSignal,
        service: &str,
        confidence: f64,
        input: &RcaAgentInput,
    ) -> RootCauseHypothesis {
        let category = self.infer_category_from_signal(signal);

        RootCauseHypothesis {
            hypothesis_id: Uuid::new_v4(),
            rank: 0, // Will be set during ranking
            confidence,
            category,
            root_cause_service: service.to_string(),
            root_cause_component: Some(signal.metric.clone()),
            description: format!(
                "{} in {} preceded incident by {}s with {:.0}% correlation",
                signal.signal_type,
                service,
                signal.lag_seconds.abs(),
                signal.correlation * 100.0
            ),
            causal_chain: vec![
                CausalChainLink {
                    position: 0,
                    service: service.to_string(),
                    component: signal.metric.clone(),
                    impact: format!("{} deviation detected", signal.signal_type),
                    timestamp: signal.timestamp,
                    link_confidence: confidence,
                },
                CausalChainLink {
                    position: 1,
                    service: input.primary_incident.service_name.clone(),
                    component: input.primary_incident.metric.clone(),
                    impact: format!("{} incident triggered", input.primary_incident.incident_type),
                    timestamp: input.primary_incident.timestamp,
                    link_confidence: signal.correlation,
                },
            ],
            evidence: vec![RcaEvidence {
                evidence_type: EvidenceType::TemporalCorrelation,
                description: format!(
                    "Signal preceded incident by {}s",
                    signal.lag_seconds.abs()
                ),
                strength: signal.correlation,
                source_signal_id: Some(signal.signal_id),
                data: Some(serde_json::json!({
                    "lag_seconds": signal.lag_seconds,
                    "correlation": signal.correlation,
                    "value": signal.value
                })),
            }],
            investigation_steps: vec![
                format!("Check {} metrics for {}", signal.metric, service),
                format!(
                    "Review logs around {} UTC",
                    signal.timestamp.format("%Y-%m-%d %H:%M:%S")
                ),
                format!("Verify {} configuration", service),
            ],
        }
    }

    fn create_dependency_hypothesis(
        &self,
        signal: &RcaCorrelatedSignal,
        service: &str,
        confidence: f64,
        input: &RcaAgentInput,
    ) -> RootCauseHypothesis {
        RootCauseHypothesis {
            hypothesis_id: Uuid::new_v4(),
            rank: 0,
            confidence,
            category: RootCauseCategory::DependencyFailure,
            root_cause_service: service.to_string(),
            root_cause_component: Some(signal.metric.clone()),
            description: format!(
                "Upstream dependency {} showed {} anomaly before incident",
                service, signal.signal_type
            ),
            causal_chain: vec![
                CausalChainLink {
                    position: 0,
                    service: service.to_string(),
                    component: signal.metric.clone(),
                    impact: "Upstream service degradation".to_string(),
                    timestamp: signal.timestamp,
                    link_confidence: confidence,
                },
                CausalChainLink {
                    position: 1,
                    service: input.primary_incident.service_name.clone(),
                    component: input.primary_incident.metric.clone(),
                    impact: "Downstream impact from dependency".to_string(),
                    timestamp: input.primary_incident.timestamp,
                    link_confidence: signal.correlation,
                },
            ],
            evidence: vec![RcaEvidence {
                evidence_type: EvidenceType::ServiceDependency,
                description: format!("{} is an upstream dependency", service),
                strength: confidence,
                source_signal_id: Some(signal.signal_id),
                data: None,
            }],
            investigation_steps: vec![
                format!("Check health of {} service", service),
                format!("Review dependency connections between {} and {}",
                    service, input.primary_incident.service_name),
                "Check for recent deployments or configuration changes".to_string(),
            ],
        }
    }

    fn create_inferred_dependency_hypothesis(
        &self,
        signal: &RcaCorrelatedSignal,
        confidence: f64,
        input: &RcaAgentInput,
    ) -> RootCauseHypothesis {
        RootCauseHypothesis {
            hypothesis_id: Uuid::new_v4(),
            rank: 0,
            confidence,
            category: RootCauseCategory::DependencyFailure,
            root_cause_service: signal.service_name.clone(),
            root_cause_component: Some(signal.metric.clone()),
            description: format!(
                "Possible upstream dependency {} (inferred from temporal pattern)",
                signal.service_name
            ),
            causal_chain: vec![
                CausalChainLink {
                    position: 0,
                    service: signal.service_name.clone(),
                    component: signal.metric.clone(),
                    impact: "Potential upstream cause".to_string(),
                    timestamp: signal.timestamp,
                    link_confidence: confidence,
                },
                CausalChainLink {
                    position: 1,
                    service: input.primary_incident.service_name.clone(),
                    component: input.primary_incident.metric.clone(),
                    impact: "Incident manifestation".to_string(),
                    timestamp: input.primary_incident.timestamp,
                    link_confidence: signal.correlation,
                },
            ],
            evidence: vec![RcaEvidence {
                evidence_type: EvidenceType::TemporalCorrelation,
                description: "Inferred dependency from temporal pattern".to_string(),
                strength: confidence,
                source_signal_id: Some(signal.signal_id),
                data: None,
            }],
            investigation_steps: vec![
                format!("Verify if {} is a dependency", signal.service_name),
                "Review service architecture documentation".to_string(),
            ],
        }
    }

    fn create_resource_hypothesis(
        &self,
        signal: &RcaCorrelatedSignal,
        metric: &str,
        confidence: f64,
        input: &RcaAgentInput,
    ) -> RootCauseHypothesis {
        let category = match *metric {
            "cpu_percent" | "memory_percent" => RootCauseCategory::ResourceExhaustion,
            "disk_io" | "network_io" => RootCauseCategory::Infrastructure,
            _ => RootCauseCategory::Unknown,
        };

        RootCauseHypothesis {
            hypothesis_id: Uuid::new_v4(),
            rank: 0,
            confidence,
            category,
            root_cause_service: signal.service_name.clone(),
            root_cause_component: Some(metric.to_string()),
            description: format!(
                "Resource metric {} on {} correlated with incident ({:.0}%)",
                metric,
                signal.service_name,
                confidence * 100.0
            ),
            causal_chain: vec![
                CausalChainLink {
                    position: 0,
                    service: signal.service_name.clone(),
                    component: metric.to_string(),
                    impact: format!("{} at {:.1}%", metric, signal.value),
                    timestamp: signal.timestamp,
                    link_confidence: confidence,
                },
                CausalChainLink {
                    position: 1,
                    service: input.primary_incident.service_name.clone(),
                    component: input.primary_incident.metric.clone(),
                    impact: input.primary_incident.incident_type.clone(),
                    timestamp: input.primary_incident.timestamp,
                    link_confidence: signal.correlation,
                },
            ],
            evidence: vec![RcaEvidence {
                evidence_type: EvidenceType::ResourceMetric,
                description: format!("{} = {:.1}", metric, signal.value),
                strength: confidence,
                source_signal_id: Some(signal.signal_id),
                data: Some(serde_json::json!({
                    "metric": metric,
                    "value": signal.value
                })),
            }],
            investigation_steps: vec![
                format!("Check {} utilization on {}", metric, signal.service_name),
                "Review resource allocation and limits".to_string(),
                "Check for resource leaks or inefficient queries".to_string(),
            ],
        }
    }

    // =========================================================================
    // Hypothesis Ranking
    // =========================================================================

    fn merge_and_rank_hypotheses(
        &self,
        mut hypotheses: Vec<RootCauseHypothesis>,
    ) -> Vec<RootCauseHypothesis> {
        // Sort by confidence (descending)
        hypotheses.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        // Assign ranks
        for (i, hypothesis) in hypotheses.iter_mut().enumerate() {
            hypothesis.rank = (i + 1) as u32;
        }

        // Limit to max_hypotheses
        hypotheses.truncate(self.config.max_hypotheses);

        hypotheses
    }

    fn determine_status(
        &self,
        hypotheses: &[RootCauseHypothesis],
    ) -> (RcaAnalysisStatus, bool, Option<RootCauseHypothesis>) {
        if hypotheses.is_empty() {
            return (RcaAnalysisStatus::InsufficientData, false, None);
        }

        let top_hypothesis = &hypotheses[0];

        if top_hypothesis.confidence >= self.config.confidence_threshold {
            (
                RcaAnalysisStatus::RootCauseIdentified,
                true,
                Some(top_hypothesis.clone()),
            )
        } else if hypotheses.len() > 1 {
            (
                RcaAnalysisStatus::MultipleCandidates,
                false,
                Some(top_hypothesis.clone()),
            )
        } else {
            (
                RcaAnalysisStatus::Inconclusive,
                false,
                Some(top_hypothesis.clone()),
            )
        }
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    fn calculate_temporal_confidence(
        &self,
        signal: &RcaCorrelatedSignal,
        _input: &RcaAgentInput,
    ) -> f64 {
        // Confidence based on correlation strength and temporal proximity
        let correlation_factor = signal.correlation;
        let temporal_factor = 1.0 - (signal.lag_seconds.abs() as f64 / self.config.max_lag_seconds as f64);

        (correlation_factor * 0.7 + temporal_factor.max(0.0) * 0.3).clamp(0.0, 1.0)
    }

    fn calculate_dependency_confidence(
        &self,
        signal: &RcaCorrelatedSignal,
        deps: &serde_json::Value,
    ) -> f64 {
        // Higher confidence if service is in the dependency graph
        let base_confidence = signal.correlation;
        let dependency_bonus = if deps.get("upstream")
            .and_then(|u| u.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some(&signal.service_name)))
            .unwrap_or(false)
        {
            self.config.dependency_weight
        } else {
            0.0
        };

        (base_confidence + dependency_bonus).clamp(0.0, 1.0)
    }

    fn infer_category_from_signal(&self, signal: &RcaCorrelatedSignal) -> RootCauseCategory {
        match signal.signal_type.as_str() {
            "cpu_spike" | "memory_spike" => RootCauseCategory::ResourceExhaustion,
            "error_rate" | "failure" => RootCauseCategory::DependencyFailure,
            "latency_spike" => RootCauseCategory::Infrastructure,
            "deployment" | "release" => RootCauseCategory::Deployment,
            "config_change" => RootCauseCategory::ConfigurationChange,
            "traffic_spike" | "traffic_pattern" => RootCauseCategory::TrafficPattern,
            "drift" | "quality_degradation" => RootCauseCategory::ModelDegradation,
            _ => RootCauseCategory::Unknown,
        }
    }

    fn calculate_time_window(&self, input: &RcaAgentInput) -> u64 {
        let min_ts = input
            .correlated_signals
            .iter()
            .map(|s| s.timestamp)
            .min()
            .unwrap_or(input.primary_incident.timestamp);

        let max_ts = input.primary_incident.timestamp;

        (max_ts - min_ts).num_seconds().unsigned_abs()
    }

    fn generate_summary(
        &self,
        status: &RcaAnalysisStatus,
        primary_hypothesis: &Option<RootCauseHypothesis>,
        input: &RcaAgentInput,
    ) -> String {
        match (status, primary_hypothesis) {
            (RcaAnalysisStatus::RootCauseIdentified, Some(h)) => {
                format!(
                    "Root cause identified: {} in {} ({:.0}% confidence). {}",
                    h.category,
                    h.root_cause_service,
                    h.confidence * 100.0,
                    h.description
                )
            }
            (RcaAnalysisStatus::MultipleCandidates, Some(h)) => {
                format!(
                    "Multiple potential root causes identified. Top candidate: {} in {} ({:.0}% confidence)",
                    h.category,
                    h.root_cause_service,
                    h.confidence * 100.0
                )
            }
            (RcaAnalysisStatus::Inconclusive, _) => {
                format!(
                    "Analysis inconclusive for {} incident in {}. Additional signals needed.",
                    input.primary_incident.incident_type,
                    input.primary_incident.service_name
                )
            }
            (RcaAnalysisStatus::InsufficientData, _) => {
                format!(
                    "Insufficient data for root cause analysis. Received {} signals, minimum {} required.",
                    input.correlated_signals.len(),
                    self.config.min_signals
                )
            }
            (RcaAnalysisStatus::AnalysisFailed, _) => {
                "Root cause analysis failed due to internal error".to_string()
            }
        }
    }

    // =========================================================================
    // Persistence and Telemetry
    // =========================================================================

    async fn persist_decision(&self, decision: &DecisionEvent) -> Result<bool> {
        if let Some(ref _endpoint) = self.config.ruvector_endpoint {
            // TODO: Implement actual ruvector-service client call
            // let client = RuvectorClient::new(endpoint);
            // client.persist_decision(decision).await?;
            debug!(
                decision_id = %decision.decision_id,
                "Would persist RCA decision to ruvector-service"
            );
            Ok(true)
        } else {
            debug!(
                decision_id = %decision.decision_id,
                "No ruvector endpoint configured, skipping persistence"
            );
            Ok(false)
        }
    }

    async fn emit_telemetry(
        &self,
        decision: &DecisionEvent,
        rca_output: &RcaAgentOutput,
        processing_ms: u64,
    ) {
        // In production, emit metrics to observatory
        debug!(
            agent_id = %self.agent_id,
            decision_id = %decision.decision_id,
            status = %rca_output.status,
            hypotheses_count = rca_output.hypotheses.len(),
            processing_ms = processing_ms,
            "RCA telemetry emitted"
        );
    }

    async fn create_failure_result(
        &self,
        input: &RcaAgentInput,
        failure: RcaFailureMode,
        start: Instant,
    ) -> Result<RcaInvocationResult> {
        warn!(
            agent_id = %self.agent_id,
            failure = %failure,
            "RCA analysis failed validation"
        );

        let processing_ms = start.elapsed().as_millis() as u64;
        let inputs_hash = input.compute_hash();

        // Create failure output
        let rca_output = RcaAgentOutput {
            root_cause_identified: false,
            status: match &failure {
                RcaFailureMode::InsufficientSignals { .. } => RcaAnalysisStatus::InsufficientData,
                RcaFailureMode::TemporalGapTooLarge { .. } => RcaAnalysisStatus::AnalysisFailed,
                _ => RcaAnalysisStatus::AnalysisFailed,
            },
            hypotheses: vec![],
            primary_hypothesis: None,
            signals_analyzed: input.correlated_signals.len(),
            methods_applied: vec![],
            summary: failure.to_string(),
            metadata: RcaOutputMetadata {
                processing_ms,
                correlations_computed: 0,
                dependency_paths_analyzed: 0,
                time_window_seconds: 0,
                telemetry_emitted: true,
            },
        };

        // Still emit a DecisionEvent for audit trail
        let agent_output = AgentOutput {
            anomaly_detected: false,
            anomaly_event: None,
            detectors_evaluated: vec![],
            triggered_by: None,
            metadata: OutputMetadata {
                processing_ms,
                baselines_checked: 0,
                baselines_valid: 0,
                telemetry_emitted: true,
            },
        };

        let decision_event = DecisionEvent::new(
            self.agent_id.clone(),
            self.agent_version.clone(),
            DecisionType::RootCauseAnalysis,
            inputs_hash,
            agent_output,
            0.0,
            vec![ConstraintApplied {
                constraint_id: "validation_failed".to_string(),
                constraint_type: ConstraintType::Rule,
                value: serde_json::json!(failure.to_string()),
                passed: false,
                description: Some("Input validation failed".to_string()),
            }],
            ExecutionRef {
                request_id: input.request_id,
                trace_id: None,
                span_id: None,
                source: input.source.clone(),
            },
            DecisionContext {
                service_name: input.primary_incident.service_name.clone(),
                model: input.primary_incident.model.clone().unwrap_or_default(),
                region: None,
                environment: None,
            },
        );

        // Persist even failure decisions
        let persisted = if !self.config.dry_run {
            self.persist_decision(&decision_event).await.unwrap_or(false)
        } else {
            false
        };

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.invocations += 1;
            stats.failures += 1;
        }

        Ok(RcaInvocationResult {
            decision_event,
            rca_output,
            failure: Some(failure),
            persisted,
        })
    }

    // =========================================================================
    // CLI Commands
    // =========================================================================

    /// Inspect agent state and recent analyses
    pub async fn inspect(
        &self,
        service_filter: Option<&str>,
        incident_id: Option<Uuid>,
    ) -> RcaInspectResult {
        let stats = self.stats.read().await.clone();

        RcaInspectResult {
            agent_id: self.agent_id.clone(),
            agent_version: self.agent_version.clone(),
            config: self.config.clone(),
            stats,
            service_filter: service_filter.map(String::from),
            incident_id,
        }
    }

    /// Replay an incident through RCA analysis
    pub async fn replay(
        &self,
        input: &RcaAgentInput,
        dry_run: bool,
    ) -> Result<RcaInvocationResult> {
        // Create a modified config for replay
        let original_dry_run = self.config.dry_run;

        // For replay, we might want to force dry_run
        if dry_run {
            // Note: In a real implementation, we'd use interior mutability or create a new agent
            debug!("Replay requested with dry_run={}", dry_run);
        }

        self.invoke(input).await
    }

    /// Run diagnostics
    pub async fn diagnose(&self, check: RcaDiagnoseCheck) -> RcaDiagnoseResult {
        let mut results = Vec::new();

        match check {
            RcaDiagnoseCheck::All => {
                // Check all components
                results.push(self.check_analysis_methods().await);
                results.push(self.check_ruvector_connection().await);
            }
            RcaDiagnoseCheck::AnalysisMethods => {
                results.push(self.check_analysis_methods().await);
            }
            RcaDiagnoseCheck::Dependencies => {
                results.push(("dependencies".to_string(), true, "Dependency analysis configured".to_string()));
            }
            RcaDiagnoseCheck::Ruvector => {
                results.push(self.check_ruvector_connection().await);
            }
        }

        let all_passed = results.iter().all(|(_, passed, _)| *passed);

        RcaDiagnoseResult {
            agent_id: self.agent_id.clone(),
            overall_status: if all_passed { "healthy" } else { "degraded" }.to_string(),
            checks: results,
        }
    }

    async fn check_analysis_methods(&self) -> (String, bool, String) {
        let enabled_count = [
            self.config.enable_temporal_correlation,
            self.config.enable_dependency_analysis,
            self.config.enable_metric_correlation,
            self.config.enable_historical_patterns,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        (
            "analysis_methods".to_string(),
            enabled_count > 0,
            format!("{} analysis methods enabled", enabled_count),
        )
    }

    async fn check_ruvector_connection(&self) -> (String, bool, String) {
        if self.config.ruvector_endpoint.is_some() {
            // TODO: Actually test connection
            ("ruvector".to_string(), true, "Endpoint configured".to_string())
        } else {
            ("ruvector".to_string(), false, "No endpoint configured".to_string())
        }
    }

    /// Get agent statistics
    pub async fn stats(&self) -> RcaAgentStats {
        self.stats.read().await.clone()
    }
}

/// Inspect result for CLI
#[derive(Debug, Clone, Serialize)]
pub struct RcaInspectResult {
    pub agent_id: AgentId,
    pub agent_version: AgentVersion,
    pub config: RcaAgentConfig,
    pub stats: RcaAgentStats,
    pub service_filter: Option<String>,
    pub incident_id: Option<Uuid>,
}

/// Diagnose check type
#[derive(Debug, Clone, Copy)]
pub enum RcaDiagnoseCheck {
    All,
    AnalysisMethods,
    Dependencies,
    Ruvector,
}

/// Diagnose result for CLI
#[derive(Debug, Clone, Serialize)]
pub struct RcaDiagnoseResult {
    pub agent_id: AgentId,
    pub overall_status: String,
    pub checks: Vec<(String, bool, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_input() -> RcaAgentInput {
        let primary_incident = RcaIncidentSignal {
            incident_id: Uuid::new_v4(),
            service_name: "chat-api".to_string(),
            model: Some("gpt-4".to_string()),
            timestamp: Utc::now(),
            severity: "high".to_string(),
            incident_type: "latency_spike".to_string(),
            metric: "latency_ms".to_string(),
            observed_value: 5000.0,
            baseline_value: 150.0,
            context: HashMap::new(),
        };

        let correlated_signals = vec![
            RcaCorrelatedSignal {
                signal_id: Uuid::new_v4(),
                service_name: "database".to_string(),
                timestamp: Utc::now() - chrono::Duration::seconds(60),
                signal_type: "cpu_spike".to_string(),
                metric: "cpu_percent".to_string(),
                value: 95.0,
                correlation: 0.85,
                lag_seconds: -60,
                metadata: HashMap::new(),
            },
            RcaCorrelatedSignal {
                signal_id: Uuid::new_v4(),
                service_name: "cache".to_string(),
                timestamp: Utc::now() - chrono::Duration::seconds(45),
                signal_type: "error_rate".to_string(),
                metric: "error_percent".to_string(),
                value: 15.0,
                correlation: 0.72,
                lag_seconds: -45,
                metadata: HashMap::new(),
            },
        ];

        RcaAgentInput::new(primary_incident, correlated_signals, "test")
    }

    #[test]
    fn test_rca_agent_creation() {
        let agent = RootCauseAnalysisAgent::with_defaults().unwrap();
        assert_eq!(agent.agent_id.as_str(), "sentinel.analysis.rca");
    }

    #[test]
    fn test_config_validation_no_methods() {
        let config = RcaAgentConfig {
            enable_temporal_correlation: false,
            enable_dependency_analysis: false,
            enable_metric_correlation: false,
            enable_historical_patterns: false,
            ..Default::default()
        };

        let result = RootCauseAnalysisAgent::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_invalid_correlation() {
        let config = RcaAgentConfig {
            min_correlation_strength: 1.5, // Invalid
            ..Default::default()
        };

        let result = RootCauseAnalysisAgent::new(config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rca_invocation() {
        let config = RcaAgentConfig {
            dry_run: true,
            ..Default::default()
        };
        let agent = RootCauseAnalysisAgent::new(config).unwrap();
        let input = create_test_input();

        let result = agent.invoke(&input).await.unwrap();

        assert!(result.failure.is_none());
        assert_eq!(
            result.decision_event.decision_type,
            DecisionType::RootCauseAnalysis
        );
        assert!(!result.rca_output.hypotheses.is_empty());
    }

    #[tokio::test]
    async fn test_insufficient_signals() {
        let config = RcaAgentConfig {
            min_signals: 10, // Require more than we'll provide
            dry_run: true,
            ..Default::default()
        };
        let agent = RootCauseAnalysisAgent::new(config).unwrap();
        let input = create_test_input();

        let result = agent.invoke(&input).await.unwrap();

        assert!(result.failure.is_some());
        matches!(
            result.failure,
            Some(RcaFailureMode::InsufficientSignals { .. })
        );
    }

    #[tokio::test]
    async fn test_hypothesis_ranking() {
        let config = RcaAgentConfig {
            dry_run: true,
            ..Default::default()
        };
        let agent = RootCauseAnalysisAgent::new(config).unwrap();
        let input = create_test_input();

        let result = agent.invoke(&input).await.unwrap();

        // Verify hypotheses are ranked by confidence
        let hypotheses = &result.rca_output.hypotheses;
        for i in 1..hypotheses.len() {
            assert!(hypotheses[i - 1].confidence >= hypotheses[i].confidence);
            assert_eq!(hypotheses[i].rank, (i + 1) as u32);
        }
    }

    #[tokio::test]
    async fn test_inspect() {
        let agent = RootCauseAnalysisAgent::with_defaults().unwrap();
        let result = agent.inspect(Some("chat-api"), None).await;

        assert_eq!(result.agent_id.as_str(), "sentinel.analysis.rca");
        assert_eq!(result.service_filter, Some("chat-api".to_string()));
    }

    #[tokio::test]
    async fn test_diagnose() {
        let agent = RootCauseAnalysisAgent::with_defaults().unwrap();
        let result = agent.diagnose(RcaDiagnoseCheck::All).await;

        assert_eq!(result.agent_id.as_str(), "sentinel.analysis.rca");
        assert!(!result.checks.is_empty());
    }

    #[test]
    fn test_category_inference() {
        let agent = RootCauseAnalysisAgent::with_defaults().unwrap();

        let signal = RcaCorrelatedSignal {
            signal_id: Uuid::new_v4(),
            service_name: "test".to_string(),
            timestamp: Utc::now(),
            signal_type: "cpu_spike".to_string(),
            metric: "cpu_percent".to_string(),
            value: 95.0,
            correlation: 0.8,
            lag_seconds: -30,
            metadata: HashMap::new(),
        };

        assert_eq!(
            agent.infer_category_from_signal(&signal),
            RootCauseCategory::ResourceExhaustion
        );
    }
}
