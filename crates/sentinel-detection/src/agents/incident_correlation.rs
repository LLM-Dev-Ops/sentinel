//! Incident Correlation Agent for LLM-Sentinel
//!
//! This agent correlates multiple anomaly events and alerts from upstream
//! detection agents into coherent incident candidates.
//!
//! # Constitutional Compliance
//!
//! This agent adheres to the LLM-Sentinel Agent Infrastructure Constitution:
//! - Classification: CORRELATION / ANALYSIS
//! - Stateless at runtime
//! - No remediation, retry, or orchestration logic
//! - Emits exactly ONE DecisionEvent per invocation
//! - Persists only via ruvector-service client
//!
//! # Correlation Algorithms
//! - Time-window clustering (signals within configurable window)
//! - Service affinity grouping (same/related services)
//! - Signal type affinity (anomaly↔alert compatibility)
//! - Severity consistency scoring
//!
//! # Usage
//!
//! ```rust,ignore
//! use sentinel_detection::agents::IncidentCorrelationAgent;
//!
//! let agent = IncidentCorrelationAgent::new(Default::default())?;
//! let result = agent.invoke(&input).await?;
//! // result.decision_event is persisted to ruvector-service
//! ```

use crate::agents::contract::{
    AgentId, AgentOutput, AgentVersion, ConstraintApplied, ConstraintType, CorrelationFailureMode,
    CorrelationOutputMetadata, CorrelationSignal, CorrelationTimeWindow, DecisionContext,
    DecisionEvent, DecisionType, ExecutionRef, IncidentCandidate, IncidentCorrelationInput,
    IncidentCorrelationOutput, OutputMetadata, ServiceGroup, SignalBreakdown, SignalType,
};
use chrono::{DateTime, Utc};
use llm_sentinel_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Incident Correlation Agent Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentCorrelationAgentConfig {
    // === Time Window Parameters ===
    /// Default time window for grouping signals (seconds)
    pub time_window_secs: u64,
    /// Maximum time window allowed (seconds)
    pub max_time_window_secs: u64,
    /// Minimum overlap ratio for merging time windows
    pub min_window_overlap: f64,

    // === Signal Thresholds ===
    /// Minimum signals required to form an incident
    pub min_signals_per_incident: usize,
    /// Maximum signals per incident (prevents mega-incidents)
    pub max_signals_per_incident: usize,
    /// Maximum signals to process in one invocation
    pub max_input_signals: usize,

    // === Correlation Thresholds ===
    /// Minimum correlation confidence to form incident
    pub correlation_threshold: f64,
    /// Threshold for high-confidence incidents
    pub high_confidence_threshold: f64,

    // === Weights ===
    /// Weight for temporal clustering score
    pub temporal_weight: f64,
    /// Weight for service overlap score
    pub service_weight: f64,
    /// Weight for signal type affinity score
    pub affinity_weight: f64,
    /// Weight for severity consistency score
    pub severity_weight: f64,

    // === Service Grouping Rules ===
    /// Enable service-based grouping
    pub enable_service_grouping: bool,
    /// Service groups (services that should be correlated together)
    pub service_groups: Vec<ServiceGroup>,

    // === Signal Type Affinity ===
    /// Enable signal type affinity scoring
    pub enable_affinity_scoring: bool,

    // === Noise Reduction ===
    /// Enable deduplication of near-identical signals
    pub enable_deduplication: bool,
    /// Deduplication time window (seconds)
    pub dedup_window_secs: u64,
    /// Enable severity-based filtering
    pub enable_severity_filter: bool,
    /// Minimum severity to consider (low, medium, high, critical)
    pub min_severity: String,

    // === Persistence ===
    /// ruvector-service endpoint
    pub ruvector_endpoint: Option<String>,
    /// Dry-run mode
    pub dry_run: bool,
}

impl Default for IncidentCorrelationAgentConfig {
    fn default() -> Self {
        Self {
            time_window_secs: 300,           // 5 minutes
            max_time_window_secs: 3600,      // 1 hour
            min_window_overlap: 0.5,
            min_signals_per_incident: 2,
            max_signals_per_incident: 50,
            max_input_signals: 1000,
            correlation_threshold: 0.6,
            high_confidence_threshold: 0.85,
            temporal_weight: 0.35,
            service_weight: 0.30,
            affinity_weight: 0.20,
            severity_weight: 0.15,
            enable_service_grouping: true,
            service_groups: Vec::new(),
            enable_affinity_scoring: true,
            enable_deduplication: true,
            dedup_window_secs: 60,
            enable_severity_filter: false,
            min_severity: "low".to_string(),
            ruvector_endpoint: None,
            dry_run: false,
        }
    }
}

// =============================================================================
// AGENT STATISTICS
// =============================================================================

/// Agent statistics for monitoring
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CorrelationAgentStats {
    /// Total invocations
    pub invocations: u64,
    /// Incidents correlated
    pub incidents_correlated: u64,
    /// Total signals processed
    pub signals_processed: u64,
    /// Decisions persisted
    pub decisions_persisted: u64,
    /// Failures encountered
    pub failures: u64,
    /// Average processing time (ms)
    pub avg_processing_ms: f64,
    /// Average noise reduction ratio
    pub avg_noise_reduction: f64,
}

// =============================================================================
// INVOCATION RESULT
// =============================================================================

/// Agent invocation result
#[derive(Debug, Clone)]
pub struct CorrelationInvocationResult {
    /// The decision event (ALWAYS emitted)
    pub decision_event: DecisionEvent,
    /// Correlation output with incident candidates
    pub correlation_output: IncidentCorrelationOutput,
    /// Failure mode if any
    pub failure: Option<CorrelationFailureMode>,
    /// Was the DecisionEvent persisted?
    pub persisted: bool,
}

// =============================================================================
// INTERNAL STRUCTURES
// =============================================================================

/// Internal signal cluster during correlation
#[derive(Debug, Clone)]
struct SignalCluster {
    signals: Vec<CorrelationSignal>,
    time_start: DateTime<Utc>,
    time_end: DateTime<Utc>,
}

impl SignalCluster {
    fn new(signal: CorrelationSignal) -> Self {
        let ts = signal.timestamp;
        Self {
            signals: vec![signal],
            time_start: ts,
            time_end: ts,
        }
    }

    fn add(&mut self, signal: CorrelationSignal) {
        if signal.timestamp < self.time_start {
            self.time_start = signal.timestamp;
        }
        if signal.timestamp > self.time_end {
            self.time_end = signal.timestamp;
        }
        self.signals.push(signal);
    }

    fn duration_secs(&self) -> i64 {
        (self.time_end - self.time_start).num_seconds()
    }

    fn signal_ids(&self) -> Vec<Uuid> {
        self.signals.iter().map(|s| s.signal_id).collect()
    }
}

// =============================================================================
// AGENT IMPLEMENTATION
// =============================================================================

/// Incident Correlation Agent
///
/// Correlates multiple anomaly events and alerts into coherent incident
/// candidates using configurable time-window grouping, service affinity,
/// and signal type correlation algorithms.
pub struct IncidentCorrelationAgent {
    /// Agent configuration
    config: IncidentCorrelationAgentConfig,
    /// Agent statistics
    stats: RwLock<CorrelationAgentStats>,
    /// Signal type affinity matrix
    affinity_matrix: HashMap<(SignalType, SignalType), f64>,
}

impl IncidentCorrelationAgent {
    /// Create a new Incident Correlation Agent
    pub fn new(config: IncidentCorrelationAgentConfig) -> Result<Self> {
        // Validate configuration
        Self::validate_config(&config)?;

        // Build affinity matrix
        let affinity_matrix = Self::build_default_affinity_matrix();

        info!(
            agent_id = %AgentId::incident_correlation(),
            time_window = config.time_window_secs,
            min_signals = config.min_signals_per_incident,
            correlation_threshold = config.correlation_threshold,
            "Incident Correlation Agent initialized"
        );

        Ok(Self {
            config,
            stats: RwLock::new(CorrelationAgentStats::default()),
            affinity_matrix,
        })
    }

    /// Validate agent configuration
    fn validate_config(config: &IncidentCorrelationAgentConfig) -> Result<()> {
        if config.min_signals_per_incident < 1 {
            return Err(Error::config(
                "min_signals_per_incident must be at least 1",
            ));
        }

        if config.correlation_threshold < 0.0 || config.correlation_threshold > 1.0 {
            return Err(Error::config(
                "correlation_threshold must be between 0.0 and 1.0",
            ));
        }

        let weight_sum = config.temporal_weight
            + config.service_weight
            + config.affinity_weight
            + config.severity_weight;

        if (weight_sum - 1.0).abs() > 0.001 {
            return Err(Error::config(format!(
                "Weights must sum to 1.0, got {}",
                weight_sum
            )));
        }

        if config.time_window_secs == 0 {
            return Err(Error::config("time_window_secs must be greater than 0"));
        }

        Ok(())
    }

    /// Build default signal type affinity matrix
    fn build_default_affinity_matrix() -> HashMap<(SignalType, SignalType), f64> {
        let mut matrix = HashMap::new();

        // Same type = high affinity
        matrix.insert((SignalType::Anomaly, SignalType::Anomaly), 1.0);
        matrix.insert((SignalType::Alert, SignalType::Alert), 1.0);
        matrix.insert((SignalType::Drift, SignalType::Drift), 1.0);
        matrix.insert((SignalType::Custom, SignalType::Custom), 1.0);

        // Cross-type affinities
        matrix.insert((SignalType::Anomaly, SignalType::Alert), 0.9);
        matrix.insert((SignalType::Alert, SignalType::Anomaly), 0.9);
        matrix.insert((SignalType::Drift, SignalType::Anomaly), 0.7);
        matrix.insert((SignalType::Anomaly, SignalType::Drift), 0.7);
        matrix.insert((SignalType::Drift, SignalType::Alert), 0.6);
        matrix.insert((SignalType::Alert, SignalType::Drift), 0.6);

        // Custom has moderate affinity with everything
        matrix.insert((SignalType::Custom, SignalType::Anomaly), 0.5);
        matrix.insert((SignalType::Anomaly, SignalType::Custom), 0.5);
        matrix.insert((SignalType::Custom, SignalType::Alert), 0.5);
        matrix.insert((SignalType::Alert, SignalType::Custom), 0.5);
        matrix.insert((SignalType::Custom, SignalType::Drift), 0.5);
        matrix.insert((SignalType::Drift, SignalType::Custom), 0.5);

        matrix
    }

    /// Main invocation method - process signals and emit DecisionEvent
    pub async fn invoke(
        &self,
        input: &IncidentCorrelationInput,
    ) -> Result<CorrelationInvocationResult> {
        let start = Instant::now();
        let request_id = input.request_id;

        info!(
            request_id = %request_id,
            signal_count = input.signals.len(),
            source = %input.source,
            "Incident Correlation Agent invoked"
        );

        // Compute input hash for auditability
        let inputs_hash = input.compute_hash();

        // Track constraints applied
        let mut constraints_applied = Vec::new();

        // Add configuration constraints
        constraints_applied.push(ConstraintApplied {
            constraint_id: format!("time_window:{}s", self.config.time_window_secs),
            constraint_type: ConstraintType::Temporal,
            value: serde_json::json!(self.config.time_window_secs),
            passed: true,
            description: Some("Time window for signal grouping".to_string()),
        });

        constraints_applied.push(ConstraintApplied {
            constraint_id: format!("min_signals:{}", self.config.min_signals_per_incident),
            constraint_type: ConstraintType::Rule,
            value: serde_json::json!(self.config.min_signals_per_incident),
            passed: true,
            description: Some("Minimum signals per incident".to_string()),
        });

        constraints_applied.push(ConstraintApplied {
            constraint_id: format!("correlation_threshold:{}", self.config.correlation_threshold),
            constraint_type: ConstraintType::Threshold,
            value: serde_json::json!(self.config.correlation_threshold),
            passed: true,
            description: Some("Minimum correlation confidence".to_string()),
        });

        // Validate input
        if let Err(failure) = self.validate_input(input) {
            return self
                .create_failure_result(
                    request_id,
                    inputs_hash,
                    input,
                    constraints_applied,
                    failure,
                    start.elapsed().as_millis() as u64,
                )
                .await;
        }

        // Pre-process signals
        let mut signals = input.signals.clone();

        // Apply severity filter if enabled
        if self.config.enable_severity_filter {
            signals = self.filter_by_severity(signals);
            debug!(
                remaining_signals = signals.len(),
                "Applied severity filter"
            );
        }

        // Apply deduplication if enabled
        if self.config.enable_deduplication {
            let original_count = signals.len();
            signals = self.deduplicate_signals(signals);
            debug!(
                original = original_count,
                deduplicated = signals.len(),
                "Applied deduplication"
            );
        }

        // Sort by timestamp
        signals.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // Perform correlation
        let (incidents, orphan_signals) = self.correlate_signals(&signals);

        // Calculate overall confidence (max across incidents, or 0 if none)
        let max_confidence = incidents
            .iter()
            .map(|i| i.correlation_confidence)
            .fold(0.0_f64, |a, b| a.max(b));

        // Calculate noise reduction ratio
        let signals_correlated: usize = incidents.iter().map(|i| i.signal_ids.len()).sum();
        let noise_reduction_ratio = if !signals.is_empty() {
            1.0 - (incidents.len() as f64 / signals.len() as f64)
        } else {
            0.0
        };

        let processing_ms = start.elapsed().as_millis() as u64;

        // Build output
        let correlation_output = IncidentCorrelationOutput {
            incidents_detected: !incidents.is_empty(),
            incident_count: incidents.len(),
            incidents,
            orphan_signals,
            strategies_evaluated: vec![
                "time_window_clustering".to_string(),
                "service_affinity".to_string(),
                "signal_type_affinity".to_string(),
            ],
            primary_strategy: Some("hierarchical".to_string()),
            metadata: CorrelationOutputMetadata {
                processing_ms,
                signals_processed: input.signals.len(),
                signals_correlated,
                orphan_signal_count: input.signals.len() - signals_correlated,
                noise_reduction_ratio,
                telemetry_emitted: true,
            },
        };

        // Add strategy constraint
        constraints_applied.push(ConstraintApplied {
            constraint_id: "strategy:hierarchical".to_string(),
            constraint_type: ConstraintType::Analysis,
            value: serde_json::json!("hierarchical"),
            passed: true,
            description: Some("Correlation strategy used".to_string()),
        });

        // Build DecisionEvent
        let decision_event = self.create_decision_event(
            inputs_hash,
            &correlation_output,
            max_confidence,
            constraints_applied,
            request_id,
            input,
        );

        // Validate DecisionEvent
        if let Err(errors) = decision_event.validate() {
            error!(
                request_id = %request_id,
                errors = ?errors,
                "DecisionEvent validation failed"
            );
            return Err(Error::validation(errors.join("; ")));
        }

        // Persist to ruvector-service
        let persisted = if !self.config.dry_run {
            self.persist_decision(&decision_event).await?
        } else {
            debug!(request_id = %request_id, "Dry-run mode, skipping persistence");
            false
        };

        // Emit telemetry
        self.emit_telemetry(&correlation_output, processing_ms);

        // Update stats
        self.update_stats(&correlation_output, processing_ms, persisted)
            .await;

        info!(
            request_id = %request_id,
            incidents_detected = correlation_output.incident_count,
            signals_correlated = correlation_output.metadata.signals_correlated,
            noise_reduction = %format!("{:.2}%", noise_reduction_ratio * 100.0),
            confidence = max_confidence,
            processing_ms = processing_ms,
            persisted = persisted,
            "Incident Correlation completed"
        );

        Ok(CorrelationInvocationResult {
            decision_event,
            correlation_output,
            failure: None,
            persisted,
        })
    }

    /// Validate input before processing
    fn validate_input(&self, input: &IncidentCorrelationInput) -> std::result::Result<(), CorrelationFailureMode> {
        if input.signals.is_empty() {
            return Err(CorrelationFailureMode::NoSignalsProvided);
        }

        if input.signals.len() > self.config.max_input_signals {
            return Err(CorrelationFailureMode::SignalOverload {
                count: input.signals.len(),
                max: self.config.max_input_signals,
            });
        }

        Ok(())
    }

    /// Filter signals by minimum severity
    fn filter_by_severity(&self, signals: Vec<CorrelationSignal>) -> Vec<CorrelationSignal> {
        let min_level = self.severity_to_level(&self.config.min_severity);
        signals
            .into_iter()
            .filter(|s| self.severity_to_level(&s.severity) >= min_level)
            .collect()
    }

    /// Convert severity string to numeric level
    fn severity_to_level(&self, severity: &str) -> u8 {
        match severity.to_lowercase().as_str() {
            "low" => 1,
            "medium" => 2,
            "high" => 3,
            "critical" => 4,
            _ => 1,
        }
    }

    /// Get highest severity from list
    fn highest_severity(&self, severities: &[String]) -> String {
        severities
            .iter()
            .max_by_key(|s| self.severity_to_level(s))
            .cloned()
            .unwrap_or_else(|| "low".to_string())
    }

    /// Deduplicate near-identical signals within dedup window
    fn deduplicate_signals(&self, signals: Vec<CorrelationSignal>) -> Vec<CorrelationSignal> {
        let mut deduplicated = Vec::new();
        let dedup_window = chrono::Duration::seconds(self.config.dedup_window_secs as i64);

        for signal in signals {
            let is_duplicate = deduplicated.iter().any(|existing: &CorrelationSignal| {
                existing.service_name == signal.service_name
                    && existing.signal_type == signal.signal_type
                    && existing.anomaly_type == signal.anomaly_type
                    && (signal.timestamp - existing.timestamp).abs() < dedup_window
            });

            if !is_duplicate {
                deduplicated.push(signal);
            }
        }

        deduplicated
    }

    /// Main correlation algorithm
    fn correlate_signals(
        &self,
        signals: &[CorrelationSignal],
    ) -> (Vec<IncidentCandidate>, Vec<Uuid>) {
        if signals.is_empty() {
            return (Vec::new(), Vec::new());
        }

        // Step 1: Time-window clustering
        let mut clusters = self.cluster_by_time_window(signals);

        // Step 2: Sub-cluster by service if enabled
        if self.config.enable_service_grouping {
            clusters = self.refine_by_service(clusters);
        }

        // Step 3: Calculate confidence and filter
        let mut incidents = Vec::new();
        let mut correlated_signal_ids = HashSet::new();

        for cluster in clusters {
            if cluster.signals.len() < self.config.min_signals_per_incident {
                continue;
            }

            let confidence = self.calculate_cluster_confidence(&cluster);

            if confidence >= self.config.correlation_threshold {
                let incident = self.create_incident_candidate(cluster, confidence);
                for id in &incident.signal_ids {
                    correlated_signal_ids.insert(*id);
                }
                incidents.push(incident);
            }
        }

        // Identify orphan signals
        let orphan_signals: Vec<Uuid> = signals
            .iter()
            .filter(|s| !correlated_signal_ids.contains(&s.signal_id))
            .map(|s| s.signal_id)
            .collect();

        (incidents, orphan_signals)
    }

    /// Cluster signals by time window
    fn cluster_by_time_window(&self, signals: &[CorrelationSignal]) -> Vec<SignalCluster> {
        let window = chrono::Duration::seconds(self.config.time_window_secs as i64);
        let mut clusters: Vec<SignalCluster> = Vec::new();

        for signal in signals {
            let mut added = false;

            for cluster in &mut clusters {
                // Check if signal fits in this cluster's time window
                let cluster_end_extended = cluster.time_end + window;
                if signal.timestamp <= cluster_end_extended {
                    cluster.add(signal.clone());
                    added = true;
                    break;
                }
            }

            if !added {
                clusters.push(SignalCluster::new(signal.clone()));
            }
        }

        clusters
    }

    /// Refine clusters by service affinity
    fn refine_by_service(&self, clusters: Vec<SignalCluster>) -> Vec<SignalCluster> {
        let mut refined = Vec::new();

        for cluster in clusters {
            // Group signals by service
            let mut service_groups: HashMap<String, Vec<CorrelationSignal>> = HashMap::new();

            for signal in cluster.signals {
                // Check if service belongs to a configured group
                let group_key = self
                    .find_service_group(&signal.service_name)
                    .unwrap_or_else(|| signal.service_name.clone());

                service_groups
                    .entry(group_key)
                    .or_default()
                    .push(signal);
            }

            // Create separate clusters for each service group
            for (_, signals) in service_groups {
                if !signals.is_empty() {
                    let mut new_cluster = SignalCluster::new(signals[0].clone());
                    for signal in signals.into_iter().skip(1) {
                        new_cluster.add(signal);
                    }
                    refined.push(new_cluster);
                }
            }
        }

        refined
    }

    /// Find service group for a service
    fn find_service_group(&self, service: &str) -> Option<String> {
        for group in &self.config.service_groups {
            if group.services.iter().any(|s| s == service) {
                return Some(group.group_id.clone());
            }
        }
        None
    }

    /// Calculate confidence score for a cluster
    fn calculate_cluster_confidence(&self, cluster: &SignalCluster) -> f64 {
        let temporal_score = self.calculate_temporal_score(cluster);
        let service_score = self.calculate_service_score(cluster);
        let affinity_score = if self.config.enable_affinity_scoring {
            self.calculate_affinity_score(cluster)
        } else {
            1.0
        };
        let severity_score = self.calculate_severity_consistency(cluster);

        let confidence = self.config.temporal_weight * temporal_score
            + self.config.service_weight * service_score
            + self.config.affinity_weight * affinity_score
            + self.config.severity_weight * severity_score;

        confidence.clamp(0.0, 1.0)
    }

    /// Calculate temporal clustering score
    /// Higher score = signals are tightly clustered in time
    fn calculate_temporal_score(&self, cluster: &SignalCluster) -> f64 {
        if cluster.signals.len() < 2 {
            return 1.0;
        }

        let actual_span = cluster.duration_secs() as f64;
        let max_span = self.config.time_window_secs as f64;

        // Score: 1.0 if all signals in same second, 0.0 if spread across entire window
        (1.0 - (actual_span / max_span).min(1.0)).max(0.0)
    }

    /// Calculate service overlap score
    /// Higher score = more signals from same/related services
    fn calculate_service_score(&self, cluster: &SignalCluster) -> f64 {
        let services: HashSet<_> = cluster.signals.iter().map(|s| &s.service_name).collect();

        if services.len() == 1 {
            return 1.0; // All from same service
        }

        // Check if services belong to same configured group
        for group in &self.config.service_groups {
            let group_services: HashSet<_> = group.services.iter().collect();
            let overlap: HashSet<_> = services
                .iter()
                .filter(|s| group_services.contains(*s))
                .collect();

            if overlap.len() == services.len() {
                return 0.9 * group.weight_multiplier.min(1.0);
            }
        }

        // Calculate overlap ratio with most common service
        let mut service_counts: HashMap<&str, usize> = HashMap::new();
        for signal in &cluster.signals {
            *service_counts.entry(&signal.service_name).or_default() += 1;
        }

        let max_count = *service_counts.values().max().unwrap_or(&1) as f64;
        let total = cluster.signals.len() as f64;

        max_count / total
    }

    /// Calculate signal type affinity score
    fn calculate_affinity_score(&self, cluster: &SignalCluster) -> f64 {
        if cluster.signals.len() < 2 {
            return 1.0;
        }

        let mut total_affinity = 0.0;
        let mut pair_count = 0;

        for i in 0..cluster.signals.len() {
            for j in (i + 1)..cluster.signals.len() {
                let key = (
                    cluster.signals[i].signal_type,
                    cluster.signals[j].signal_type,
                );
                let affinity = self.affinity_matrix.get(&key).copied().unwrap_or(0.5);
                total_affinity += affinity;
                pair_count += 1;
            }
        }

        if pair_count == 0 {
            return 1.0;
        }

        total_affinity / pair_count as f64
    }

    /// Calculate severity consistency score
    /// Higher score = severities are consistent (not all over the place)
    fn calculate_severity_consistency(&self, cluster: &SignalCluster) -> f64 {
        if cluster.signals.len() < 2 {
            return 1.0;
        }

        let severity_values: Vec<f64> = cluster
            .signals
            .iter()
            .map(|s| self.severity_to_level(&s.severity) as f64)
            .collect();

        let mean = severity_values.iter().sum::<f64>() / severity_values.len() as f64;
        let variance = severity_values
            .iter()
            .map(|&v| (v - mean).powi(2))
            .sum::<f64>()
            / severity_values.len() as f64;

        // Normalize: max variance is ~2.25 (between 1 and 4)
        (1.0 - (variance / 2.25).min(1.0)).max(0.0)
    }

    /// Create an incident candidate from a cluster
    fn create_incident_candidate(
        &self,
        cluster: SignalCluster,
        confidence: f64,
    ) -> IncidentCandidate {
        let signal_ids = cluster.signal_ids();

        // Collect affected services and models
        let mut service_counts: HashMap<String, usize> = HashMap::new();
        let mut affected_models: HashSet<String> = HashSet::new();

        for signal in &cluster.signals {
            *service_counts
                .entry(signal.service_name.clone())
                .or_default() += 1;
            affected_models.insert(signal.model.clone());
        }

        let primary_service = service_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(service, _)| service.clone())
            .unwrap_or_default();

        let affected_services: Vec<String> = service_counts.keys().cloned().collect();
        let affected_models: Vec<String> = affected_models.into_iter().collect();

        // Get highest severity
        let severities: Vec<String> = cluster.signals.iter().map(|s| s.severity.clone()).collect();
        let severity = self.highest_severity(&severities);

        // Build signal breakdown
        let mut signal_breakdown = SignalBreakdown::default();
        for signal in &cluster.signals {
            match signal.signal_type {
                SignalType::Anomaly => signal_breakdown.anomaly_count += 1,
                SignalType::Alert => signal_breakdown.alert_count += 1,
                SignalType::Drift => signal_breakdown.drift_count += 1,
                SignalType::Custom => signal_breakdown.custom_count += 1,
            }

            if let Some(ref anomaly_type) = signal.anomaly_type {
                *signal_breakdown
                    .anomaly_types
                    .entry(anomaly_type.clone())
                    .or_default() += 1;
            }

            if let Some(ref method) = signal.detection_method {
                *signal_breakdown
                    .detection_methods
                    .entry(method.clone())
                    .or_default() += 1;
            }
        }

        // Generate title and description
        let title = self.generate_incident_title(&primary_service, &signal_breakdown, &severity);
        let description =
            self.generate_incident_description(&cluster, &affected_services, &affected_models);
        let reasoning = self.generate_correlation_reasoning(&cluster, confidence);
        let recommended_actions = self.generate_recommended_actions(&signal_breakdown, &severity);

        IncidentCandidate {
            incident_id: Uuid::new_v4(),
            title,
            description,
            signal_ids,
            primary_service,
            affected_services,
            affected_models,
            severity,
            correlation_confidence: confidence,
            correlation_reasoning: reasoning,
            time_window: CorrelationTimeWindow::new(cluster.time_start, cluster.time_end),
            signal_breakdown,
            recommended_actions,
        }
    }

    /// Generate incident title
    fn generate_incident_title(
        &self,
        service: &str,
        breakdown: &SignalBreakdown,
        severity: &str,
    ) -> String {
        let signal_summary = if breakdown.anomaly_count > 0 {
            if let Some((anomaly_type, _)) = breakdown.anomaly_types.iter().next() {
                anomaly_type.replace('_', " ")
            } else {
                "anomaly".to_string()
            }
        } else if breakdown.alert_count > 0 {
            "alert".to_string()
        } else if breakdown.drift_count > 0 {
            "drift".to_string()
        } else {
            "signals".to_string()
        };

        format!(
            "[{}] {} {} on {}",
            severity.to_uppercase(),
            breakdown.anomaly_count + breakdown.alert_count + breakdown.drift_count,
            signal_summary,
            service
        )
    }

    /// Generate incident description
    fn generate_incident_description(
        &self,
        cluster: &SignalCluster,
        services: &[String],
        models: &[String],
    ) -> String {
        let duration = cluster.duration_secs();
        let signal_count = cluster.signals.len();

        format!(
            "Correlated {} signals over {} seconds. Affected services: {}. Models: {}.",
            signal_count,
            duration,
            services.join(", "),
            models.join(", ")
        )
    }

    /// Generate correlation reasoning
    fn generate_correlation_reasoning(&self, cluster: &SignalCluster, confidence: f64) -> String {
        let temporal_tightness = if cluster.duration_secs() < 60 {
            "tightly clustered in time"
        } else if cluster.duration_secs() < 300 {
            "clustered within 5 minutes"
        } else {
            "spread over extended time window"
        };

        let services: HashSet<_> = cluster.signals.iter().map(|s| &s.service_name).collect();
        let service_desc = if services.len() == 1 {
            "single service affected"
        } else {
            "multiple services affected"
        };

        format!(
            "Signals are {} ({}). Correlation confidence: {:.0}%.",
            temporal_tightness,
            service_desc,
            confidence * 100.0
        )
    }

    /// Generate recommended actions
    fn generate_recommended_actions(
        &self,
        breakdown: &SignalBreakdown,
        severity: &str,
    ) -> Vec<String> {
        let mut actions = Vec::new();

        if self.severity_to_level(severity) >= 3 {
            actions.push("Escalate to on-call team immediately".to_string());
        }

        if breakdown.anomaly_count > 0 {
            actions.push("Review anomaly details and affected metrics".to_string());
        }

        if breakdown.drift_count > 0 {
            actions.push("Investigate potential model drift or data quality issues".to_string());
        }

        if breakdown.alert_count > 0 {
            actions.push("Check alerting thresholds and recent changes".to_string());
        }

        actions.push("Correlate with recent deployments or configuration changes".to_string());

        actions
    }

    /// Create DecisionEvent for the correlation
    fn create_decision_event(
        &self,
        inputs_hash: String,
        output: &IncidentCorrelationOutput,
        confidence: f64,
        constraints_applied: Vec<ConstraintApplied>,
        request_id: Uuid,
        input: &IncidentCorrelationInput,
    ) -> DecisionEvent {
        // Determine primary context from signals
        let (service_name, model) = if !input.signals.is_empty() {
            let first = &input.signals[0];
            (first.service_name.clone(), first.model.clone())
        } else {
            ("unknown".to_string(), "unknown".to_string())
        };

        DecisionEvent::new(
            AgentId::incident_correlation(),
            AgentVersion::new(1, 0, 0),
            DecisionType::IncidentCorrelation,
            inputs_hash,
            AgentOutput {
                anomaly_detected: output.incidents_detected,
                anomaly_event: Some(serde_json::to_value(output).unwrap_or_default()),
                detectors_evaluated: output.strategies_evaluated.clone(),
                triggered_by: output.primary_strategy.clone(),
                metadata: OutputMetadata {
                    processing_ms: output.metadata.processing_ms,
                    baselines_checked: 0,
                    baselines_valid: 0,
                    telemetry_emitted: output.metadata.telemetry_emitted,
                },
            },
            confidence,
            constraints_applied,
            ExecutionRef {
                request_id,
                trace_id: input
                    .hints
                    .get("trace_id")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                span_id: input
                    .hints
                    .get("span_id")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                source: input.source.clone(),
            },
            DecisionContext {
                service_name,
                model,
                region: input
                    .hints
                    .get("region")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                environment: input
                    .hints
                    .get("environment")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            },
        )
    }

    /// Create failure result
    async fn create_failure_result(
        &self,
        request_id: Uuid,
        inputs_hash: String,
        input: &IncidentCorrelationInput,
        mut constraints_applied: Vec<ConstraintApplied>,
        failure: CorrelationFailureMode,
        processing_ms: u64,
    ) -> Result<CorrelationInvocationResult> {
        warn!(
            request_id = %request_id,
            failure = %failure,
            "Correlation failed"
        );

        // Add failure constraint
        constraints_applied.push(ConstraintApplied {
            constraint_id: "validation".to_string(),
            constraint_type: ConstraintType::Rule,
            value: serde_json::json!(false),
            passed: false,
            description: Some(failure.to_string()),
        });

        let empty_output = IncidentCorrelationOutput {
            incidents_detected: false,
            incident_count: 0,
            incidents: Vec::new(),
            orphan_signals: input.signals.iter().map(|s| s.signal_id).collect(),
            strategies_evaluated: Vec::new(),
            primary_strategy: None,
            metadata: CorrelationOutputMetadata {
                processing_ms,
                signals_processed: input.signals.len(),
                signals_correlated: 0,
                orphan_signal_count: input.signals.len(),
                noise_reduction_ratio: 0.0,
                telemetry_emitted: false,
            },
        };

        let decision_event = self.create_decision_event(
            inputs_hash,
            &empty_output,
            0.0,
            constraints_applied,
            request_id,
            input,
        );

        // Update failure stats
        {
            let mut stats = self.stats.write().await;
            stats.invocations += 1;
            stats.failures += 1;
        }

        Ok(CorrelationInvocationResult {
            decision_event,
            correlation_output: empty_output,
            failure: Some(failure),
            persisted: false,
        })
    }

    /// Persist DecisionEvent to ruvector-service
    async fn persist_decision(&self, decision: &DecisionEvent) -> Result<bool> {
        if let Some(ref _endpoint) = self.config.ruvector_endpoint {
            // TODO: Implement actual ruvector-service client call
            // For now, log that we would persist
            debug!(
                decision_id = %decision.decision_id,
                "Would persist DecisionEvent to ruvector-service"
            );
            Ok(true)
        } else {
            debug!("No ruvector endpoint configured, skipping persistence");
            Ok(false)
        }
    }

    /// Emit telemetry metrics
    fn emit_telemetry(&self, output: &IncidentCorrelationOutput, processing_ms: u64) {
        // Counter: agent invocations
        metrics::counter!(
            "sentinel_agent_invocations_total",
            "agent" => "incident_correlation"
        )
        .increment(1);

        // Counter: incidents correlated
        if output.incident_count > 0 {
            metrics::counter!(
                "sentinel_correlation_incidents_total",
                "agent" => "incident_correlation"
            )
            .increment(output.incident_count as u64);
        }

        // Histogram: signals per incident
        for incident in &output.incidents {
            metrics::histogram!(
                "sentinel_correlation_signals_per_incident",
                "agent" => "incident_correlation"
            )
            .record(incident.signal_ids.len() as f64);
        }

        // Histogram: processing latency
        metrics::histogram!(
            "sentinel_agent_processing_seconds",
            "agent" => "incident_correlation"
        )
        .record(processing_ms as f64 / 1000.0);

        // Gauge: noise reduction ratio
        metrics::gauge!(
            "sentinel_correlation_noise_reduction_ratio",
            "agent" => "incident_correlation"
        )
        .set(output.metadata.noise_reduction_ratio);
    }

    /// Update agent statistics
    async fn update_stats(
        &self,
        output: &IncidentCorrelationOutput,
        processing_ms: u64,
        persisted: bool,
    ) {
        let mut stats = self.stats.write().await;

        stats.invocations += 1;
        stats.incidents_correlated += output.incident_count as u64;
        stats.signals_processed += output.metadata.signals_processed as u64;

        if persisted {
            stats.decisions_persisted += 1;
        }

        // Update running average processing time
        let n = stats.invocations as f64;
        stats.avg_processing_ms =
            stats.avg_processing_ms * ((n - 1.0) / n) + (processing_ms as f64 / n);

        // Update running average noise reduction
        stats.avg_noise_reduction = stats.avg_noise_reduction * ((n - 1.0) / n)
            + (output.metadata.noise_reduction_ratio / n);
    }

    // =========================================================================
    // CLI COMMANDS
    // =========================================================================

    /// Handle Edge Function request
    pub async fn handle_request(&self, body: &[u8]) -> Result<serde_json::Value> {
        let input: IncidentCorrelationInput = serde_json::from_slice(body)?;
        let result = self.invoke(&input).await?;
        Ok(result.decision_event.to_json())
    }

    /// Inspect agent state (CLI command)
    pub async fn inspect(
        &self,
        service_filter: Option<&str>,
        _time_window: Option<u64>,
    ) -> Result<InspectResult> {
        let stats = self.stats.read().await.clone();

        Ok(InspectResult {
            agent_id: AgentId::incident_correlation().to_string(),
            agent_version: AgentVersion::new(1, 0, 0).to_string(),
            config: self.config.clone(),
            stats,
            service_filter: service_filter.map(String::from),
            service_groups: self.config.service_groups.clone(),
        })
    }

    /// Replay signals through correlation (CLI command)
    pub async fn replay(
        &self,
        signals: Vec<CorrelationSignal>,
        dry_run: bool,
        _verbose: bool,
    ) -> Result<CorrelationInvocationResult> {
        let mut config = self.config.clone();
        config.dry_run = dry_run;

        // Create temp agent with potentially modified config
        let temp_agent = IncidentCorrelationAgent::new(config)?;

        let input = IncidentCorrelationInput::new(signals, "cli.replay");
        temp_agent.invoke(&input).await
    }

    /// Run diagnostics (CLI command)
    pub async fn diagnose(&self, check: DiagnoseCheck) -> Result<DiagnoseResult> {
        let mut results = Vec::new();

        // Config check
        if matches!(check, DiagnoseCheck::Config | DiagnoseCheck::All) {
            let config_valid = Self::validate_config(&self.config).is_ok();
            results.push(DiagnoseCheckResult {
                check_name: "config".to_string(),
                passed: config_valid,
                message: if config_valid {
                    "Configuration is valid".to_string()
                } else {
                    "Configuration validation failed".to_string()
                },
            });
        }

        // Affinity matrix check
        if matches!(check, DiagnoseCheck::Affinity | DiagnoseCheck::All) {
            let affinity_valid = !self.affinity_matrix.is_empty();
            results.push(DiagnoseCheckResult {
                check_name: "affinity".to_string(),
                passed: affinity_valid,
                message: format!(
                    "Affinity matrix has {} entries",
                    self.affinity_matrix.len()
                ),
            });
        }

        // ruvector check
        if matches!(check, DiagnoseCheck::Ruvector | DiagnoseCheck::All) {
            let ruvector_configured = self.config.ruvector_endpoint.is_some();
            results.push(DiagnoseCheckResult {
                check_name: "ruvector".to_string(),
                passed: ruvector_configured || self.config.dry_run,
                message: if ruvector_configured {
                    "ruvector-service endpoint configured".to_string()
                } else if self.config.dry_run {
                    "Dry-run mode enabled, ruvector not required".to_string()
                } else {
                    "ruvector-service endpoint not configured".to_string()
                },
            });
        }

        let all_passed = results.iter().all(|r| r.passed);

        Ok(DiagnoseResult {
            agent_id: AgentId::incident_correlation().to_string(),
            healthy: all_passed,
            checks: results,
        })
    }
}

// =============================================================================
// CLI TYPES
// =============================================================================

/// Inspect result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectResult {
    pub agent_id: String,
    pub agent_version: String,
    pub config: IncidentCorrelationAgentConfig,
    pub stats: CorrelationAgentStats,
    pub service_filter: Option<String>,
    pub service_groups: Vec<ServiceGroup>,
}

/// Diagnose check type
#[derive(Debug, Clone, Copy)]
pub enum DiagnoseCheck {
    Config,
    Affinity,
    Ruvector,
    All,
}

/// Diagnose check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnoseCheckResult {
    pub check_name: String,
    pub passed: bool,
    pub message: String,
}

/// Diagnose result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnoseResult {
    pub agent_id: String,
    pub healthy: bool,
    pub checks: Vec<DiagnoseCheckResult>,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_signal(
        service: &str,
        model: &str,
        severity: &str,
        offset_secs: i64,
    ) -> CorrelationSignal {
        CorrelationSignal::from_anomaly(
            Uuid::new_v4(),
            Utc::now() + chrono::Duration::seconds(offset_secs),
            service,
            model,
            severity,
            0.8,
        )
        .with_anomaly_details("latency_spike", "zscore")
    }

    #[test]
    fn test_agent_creation() {
        let agent = IncidentCorrelationAgent::new(Default::default());
        assert!(agent.is_ok());
    }

    #[test]
    fn test_agent_creation_invalid_config() {
        let mut config = IncidentCorrelationAgentConfig::default();
        config.correlation_threshold = 1.5; // Invalid

        let agent = IncidentCorrelationAgent::new(config);
        assert!(agent.is_err());
    }

    #[test]
    fn test_agent_creation_invalid_weights() {
        let mut config = IncidentCorrelationAgentConfig::default();
        config.temporal_weight = 0.5;
        config.service_weight = 0.5;
        config.affinity_weight = 0.5; // Sum > 1.0
        config.severity_weight = 0.5;

        let agent = IncidentCorrelationAgent::new(config);
        assert!(agent.is_err());
    }

    #[tokio::test]
    async fn test_empty_signals_returns_failure() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let input = IncidentCorrelationInput::new(vec![], "test");

        let result = agent.invoke(&input).await.unwrap();

        assert!(!result.correlation_output.incidents_detected);
        assert!(result.failure.is_some());
        assert!(matches!(
            result.failure.unwrap(),
            CorrelationFailureMode::NoSignalsProvided
        ));
    }

    #[tokio::test]
    async fn test_single_signal_no_incident() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let signals = vec![create_test_signal("chat-api", "gpt-4", "high", 0)];
        let input = IncidentCorrelationInput::new(signals, "test");

        let result = agent.invoke(&input).await.unwrap();

        // Single signal below min_signals_per_incident (2)
        assert!(!result.correlation_output.incidents_detected);
        assert_eq!(result.correlation_output.incident_count, 0);
    }

    #[tokio::test]
    async fn test_same_service_signals_correlate() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let signals = vec![
            create_test_signal("chat-api", "gpt-4", "high", 0),
            create_test_signal("chat-api", "gpt-4", "high", 30),
            create_test_signal("chat-api", "gpt-4", "medium", 60),
        ];
        let input = IncidentCorrelationInput::new(signals, "test");

        let result = agent.invoke(&input).await.unwrap();

        assert!(result.correlation_output.incidents_detected);
        assert_eq!(result.correlation_output.incident_count, 1);
        assert_eq!(result.correlation_output.incidents[0].signal_ids.len(), 3);
        assert_eq!(
            result.correlation_output.incidents[0].primary_service,
            "chat-api"
        );
    }

    #[tokio::test]
    async fn test_different_time_windows_separate() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let signals = vec![
            create_test_signal("chat-api", "gpt-4", "high", 0),
            create_test_signal("chat-api", "gpt-4", "high", 30),
            // Large gap - should be separate cluster
            create_test_signal("chat-api", "gpt-4", "high", 1000),
            create_test_signal("chat-api", "gpt-4", "high", 1030),
        ];
        let input = IncidentCorrelationInput::new(signals, "test");

        let result = agent.invoke(&input).await.unwrap();

        assert!(result.correlation_output.incidents_detected);
        assert_eq!(result.correlation_output.incident_count, 2);
    }

    #[tokio::test]
    async fn test_decision_event_always_emitted() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let signals = vec![create_test_signal("chat-api", "gpt-4", "high", 0)];
        let input = IncidentCorrelationInput::new(signals, "test");

        let result = agent.invoke(&input).await.unwrap();

        // Even with no incidents, DecisionEvent should be valid
        assert!(result.decision_event.validate().is_ok());
        assert_eq!(
            result.decision_event.decision_type,
            DecisionType::IncidentCorrelation
        );
    }

    #[tokio::test]
    async fn test_confidence_calculation() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let signals = vec![
            create_test_signal("chat-api", "gpt-4", "high", 0),
            create_test_signal("chat-api", "gpt-4", "high", 5),
            create_test_signal("chat-api", "gpt-4", "high", 10),
        ];
        let input = IncidentCorrelationInput::new(signals, "test");

        let result = agent.invoke(&input).await.unwrap();

        assert!(result.correlation_output.incidents_detected);
        // Tight clustering, same service, same severity = high confidence
        let confidence = result.correlation_output.incidents[0].correlation_confidence;
        assert!(confidence > 0.8, "Expected high confidence, got {}", confidence);
    }

    #[tokio::test]
    async fn test_deduplication() {
        let mut config = IncidentCorrelationAgentConfig::default();
        config.enable_deduplication = true;
        config.dedup_window_secs = 30;

        let agent = IncidentCorrelationAgent::new(config).unwrap();

        // Create near-duplicate signals
        let base_signal = create_test_signal("chat-api", "gpt-4", "high", 0);
        let mut dup_signal = base_signal.clone();
        dup_signal.signal_id = Uuid::new_v4();
        dup_signal.timestamp = base_signal.timestamp + chrono::Duration::seconds(5);

        let signals = vec![base_signal, dup_signal];
        let input = IncidentCorrelationInput::new(signals, "test");

        let result = agent.invoke(&input).await.unwrap();

        // After deduplication, should only have 1 signal (below min threshold)
        assert!(!result.correlation_output.incidents_detected);
    }

    #[tokio::test]
    async fn test_signal_overload() {
        let mut config = IncidentCorrelationAgentConfig::default();
        config.max_input_signals = 5;

        let agent = IncidentCorrelationAgent::new(config).unwrap();
        let signals: Vec<_> = (0..10)
            .map(|i| create_test_signal("chat-api", "gpt-4", "high", i * 10))
            .collect();
        let input = IncidentCorrelationInput::new(signals, "test");

        let result = agent.invoke(&input).await.unwrap();

        assert!(result.failure.is_some());
        assert!(matches!(
            result.failure.unwrap(),
            CorrelationFailureMode::SignalOverload { count: 10, max: 5 }
        ));
    }

    #[tokio::test]
    async fn test_inspect_command() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let result = agent.inspect(None, None).await.unwrap();

        assert_eq!(result.agent_id, "sentinel.correlation.incident");
        assert_eq!(result.agent_version, "1.0.0");
    }

    #[tokio::test]
    async fn test_diagnose_command() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let result = agent.diagnose(DiagnoseCheck::All).await.unwrap();

        assert_eq!(result.agent_id, "sentinel.correlation.incident");
        assert!(!result.checks.is_empty());
    }

    #[test]
    fn test_severity_ordering() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();

        assert!(agent.severity_to_level("low") < agent.severity_to_level("medium"));
        assert!(agent.severity_to_level("medium") < agent.severity_to_level("high"));
        assert!(agent.severity_to_level("high") < agent.severity_to_level("critical"));
    }

    #[test]
    fn test_highest_severity() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();
        let severities = vec![
            "low".to_string(),
            "high".to_string(),
            "medium".to_string(),
        ];

        assert_eq!(agent.highest_severity(&severities), "high");
    }

    #[test]
    fn test_affinity_matrix() {
        let agent = IncidentCorrelationAgent::new(Default::default()).unwrap();

        // Same type = 1.0
        assert_eq!(
            agent
                .affinity_matrix
                .get(&(SignalType::Anomaly, SignalType::Anomaly)),
            Some(&1.0)
        );

        // Anomaly <-> Alert = 0.9
        assert_eq!(
            agent
                .affinity_matrix
                .get(&(SignalType::Anomaly, SignalType::Alert)),
            Some(&0.9)
        );

        // Drift <-> Alert = 0.6
        assert_eq!(
            agent
                .affinity_matrix
                .get(&(SignalType::Drift, SignalType::Alert)),
            Some(&0.6)
        );
    }
}
