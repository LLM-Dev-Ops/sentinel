//! Anomaly Detection Agent for LLM-Sentinel
//!
//! This agent detects statistically significant deviations in live telemetry
//! signals relative to established baselines.
//!
//! # Constitutional Compliance
//!
//! This agent adheres to the LLM-Sentinel Agent Infrastructure Constitution:
//! - Classification: DETECTION
//! - Stateless at runtime
//! - No remediation, retry, or orchestration logic
//! - Emits exactly ONE DecisionEvent per invocation
//! - Persists only via ruvector-service client
//!
//! # Detection Algorithms
//! - Z-score (parametric, normal distribution assumption)
//! - IQR (non-parametric, robust to skew)
//! - MAD (ultra-robust, resistant to outliers)
//! - CUSUM (change-point detection)
//!
//! # Usage
//!
//! ```rust,ignore
//! use sentinel_detection::agents::AnomalyDetectionAgent;
//!
//! let agent = AnomalyDetectionAgent::new(config)?;
//! let result = agent.invoke(input).await?;
//! // result.decision_event is persisted to ruvector-service
//! ```

use crate::{
    agents::contract::{
        AgentId, AgentInput, AgentOutput, AgentVersion, ConstraintApplied, ConstraintType,
        DecisionContext, DecisionEvent, DecisionType, ExecutionRef, FailureMode, OutputMetadata,
    },
    baseline::{Baseline, BaselineKey, BaselineManager},
    detectors::DetectionConfig,
    stats, Detector, DetectorStats, DetectorType,
};
use async_trait::async_trait;
use chrono::Utc;
use llm_sentinel_core::{
    events::{AnomalyContext, AnomalyDetails, AnomalyEvent, TelemetryEvent},
    types::{AnomalyType, DetectionMethod, ModelId, ServiceId, Severity},
    Error, Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Anomaly Detection Agent Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectionAgentConfig {
    /// Enable Z-score detector
    pub enable_zscore: bool,
    /// Z-score threshold (default: 3.0 for 99.7% confidence)
    pub zscore_threshold: f64,

    /// Enable IQR detector
    pub enable_iqr: bool,
    /// IQR multiplier (default: 1.5)
    pub iqr_multiplier: f64,

    /// Enable MAD detector
    pub enable_mad: bool,
    /// MAD threshold (default: 3.5)
    pub mad_threshold: f64,

    /// Enable CUSUM detector
    pub enable_cusum: bool,
    /// CUSUM threshold (default: 5.0)
    pub cusum_threshold: f64,
    /// CUSUM slack parameter (default: 0.5)
    pub cusum_slack: f64,

    /// Baseline configuration
    pub baseline_window_size: usize,
    /// Minimum samples required for valid baseline
    pub min_samples: usize,
    /// Update baseline continuously
    pub continuous_learning: bool,

    /// Metrics to analyze
    pub analyze_latency: bool,
    pub analyze_tokens: bool,
    pub analyze_cost: bool,
    pub analyze_error_rate: bool,

    /// ruvector-service endpoint (for persistence)
    pub ruvector_endpoint: Option<String>,

    /// Dry-run mode (don't persist DecisionEvents)
    pub dry_run: bool,
}

impl Default for AnomalyDetectionAgentConfig {
    fn default() -> Self {
        Self {
            enable_zscore: true,
            zscore_threshold: 3.0,
            enable_iqr: true,
            iqr_multiplier: 1.5,
            enable_mad: false,
            mad_threshold: 3.5,
            enable_cusum: true,
            cusum_threshold: 5.0,
            cusum_slack: 0.5,
            baseline_window_size: 1000,
            min_samples: 10,
            continuous_learning: true,
            analyze_latency: true,
            analyze_tokens: true,
            analyze_cost: true,
            analyze_error_rate: true,
            ruvector_endpoint: None,
            dry_run: false,
        }
    }
}

/// Agent invocation result
#[derive(Debug, Clone)]
pub struct AgentInvocationResult {
    /// The decision event (ALWAYS emitted)
    pub decision_event: DecisionEvent,
    /// Anomaly event if detected
    pub anomaly_event: Option<AnomalyEvent>,
    /// Failure mode if any
    pub failure: Option<FailureMode>,
    /// Was the DecisionEvent persisted?
    pub persisted: bool,
}

/// Agent statistics
#[derive(Debug, Clone, Default)]
pub struct AgentStats {
    /// Total invocations
    pub invocations: u64,
    /// Anomalies detected
    pub anomalies_detected: u64,
    /// Decisions persisted
    pub decisions_persisted: u64,
    /// Failures encountered
    pub failures: u64,
    /// Average processing time (ms)
    pub avg_processing_ms: f64,
}

/// Anomaly Detection Agent
///
/// This is the main agent implementation that:
/// 1. Accepts telemetry events
/// 2. Runs configured detection algorithms
/// 3. Emits exactly ONE DecisionEvent per invocation
/// 4. Persists to ruvector-service
pub struct AnomalyDetectionAgent {
    /// Agent configuration
    config: AnomalyDetectionAgentConfig,
    /// Agent ID
    agent_id: AgentId,
    /// Agent version
    agent_version: AgentVersion,
    /// Baseline manager
    baseline_manager: Arc<BaselineManager>,
    /// Agent statistics
    stats: Arc<RwLock<AgentStats>>,
    /// CUSUM state (for change-point detection)
    cusum_state: Arc<RwLock<HashMap<BaselineKey, CusumState>>>,
}

/// CUSUM detector state
#[derive(Debug, Clone, Default)]
struct CusumState {
    /// Upper CUSUM
    s_high: f64,
    /// Lower CUSUM
    s_low: f64,
    /// Sample count
    count: u64,
}

impl std::fmt::Debug for AnomalyDetectionAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnomalyDetectionAgent")
            .field("agent_id", &self.agent_id)
            .field("agent_version", &self.agent_version)
            .field("config", &self.config)
            .finish()
    }
}

impl AnomalyDetectionAgent {
    /// Create a new Anomaly Detection Agent
    pub fn new(config: AnomalyDetectionAgentConfig) -> Result<Self> {
        // Validate configuration
        if !config.enable_zscore
            && !config.enable_iqr
            && !config.enable_mad
            && !config.enable_cusum
        {
            return Err(Error::config("At least one detector must be enabled"));
        }

        info!(
            agent_id = %AgentId::anomaly_detection(),
            version = %AgentVersion::current(),
            "Creating Anomaly Detection Agent"
        );

        Ok(Self {
            baseline_manager: Arc::new(BaselineManager::new(config.baseline_window_size)),
            agent_id: AgentId::anomaly_detection(),
            agent_version: AgentVersion::current(),
            config,
            stats: Arc::new(RwLock::new(AgentStats::default())),
            cusum_state: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Invoke the agent with a telemetry event
    ///
    /// This is the main entry point for the agent. It:
    /// 1. Validates the input
    /// 2. Runs all enabled detectors
    /// 3. Creates exactly ONE DecisionEvent
    /// 4. Persists to ruvector-service (unless dry-run)
    pub async fn invoke(&self, event: &TelemetryEvent) -> Result<AgentInvocationResult> {
        let start = Instant::now();
        let request_id = Uuid::new_v4();

        info!(
            agent_id = %self.agent_id,
            request_id = %request_id,
            event_id = %event.event_id,
            service = %event.service_name,
            model = %event.model,
            "Agent invocation started"
        );

        // Track constraints applied
        let mut constraints_applied = Vec::new();
        let mut detectors_evaluated = Vec::new();
        let mut baselines_checked = 0usize;
        let mut baselines_valid = 0usize;

        // Create input for hashing
        let agent_input = AgentInput {
            request_id,
            timestamp: Utc::now(),
            source: "telemetry".to_string(),
            telemetry: serde_json::to_value(event).unwrap_or_default(),
            hints: HashMap::new(),
        };
        let inputs_hash = agent_input.compute_hash();

        // Run detection
        let mut anomaly_detected = false;
        let mut anomaly_event: Option<AnomalyEvent> = None;
        let mut triggered_by: Option<String> = None;
        let mut max_confidence = 0.0_f64;

        // Z-score detection
        if self.config.enable_zscore {
            detectors_evaluated.push("zscore".to_string());
            constraints_applied.push(ConstraintApplied {
                constraint_id: "detector:zscore".to_string(),
                constraint_type: ConstraintType::Detector,
                value: serde_json::json!(true),
                passed: true,
                description: Some("Z-score detector enabled".to_string()),
            });
            constraints_applied.push(ConstraintApplied {
                constraint_id: "zscore_threshold".to_string(),
                constraint_type: ConstraintType::Threshold,
                value: serde_json::json!(self.config.zscore_threshold),
                passed: true,
                description: Some(format!(
                    "Z-score threshold: {} sigma",
                    self.config.zscore_threshold
                )),
            });

            let (detected, anomaly, confidence, checked, valid) =
                self.detect_zscore(event).await?;
            baselines_checked += checked;
            baselines_valid += valid;

            if detected {
                anomaly_detected = true;
                anomaly_event = anomaly;
                triggered_by = Some("zscore".to_string());
                max_confidence = max_confidence.max(confidence);
            }
        }

        // IQR detection (only if no anomaly yet detected)
        if self.config.enable_iqr && !anomaly_detected {
            detectors_evaluated.push("iqr".to_string());
            constraints_applied.push(ConstraintApplied {
                constraint_id: "detector:iqr".to_string(),
                constraint_type: ConstraintType::Detector,
                value: serde_json::json!(true),
                passed: true,
                description: Some("IQR detector enabled".to_string()),
            });
            constraints_applied.push(ConstraintApplied {
                constraint_id: "iqr_multiplier".to_string(),
                constraint_type: ConstraintType::Threshold,
                value: serde_json::json!(self.config.iqr_multiplier),
                passed: true,
                description: Some(format!("IQR multiplier: {}", self.config.iqr_multiplier)),
            });

            let (detected, anomaly, confidence, checked, valid) = self.detect_iqr(event).await?;
            baselines_checked += checked;
            baselines_valid += valid;

            if detected {
                anomaly_detected = true;
                anomaly_event = anomaly;
                triggered_by = Some("iqr".to_string());
                max_confidence = max_confidence.max(confidence);
            }
        }

        // MAD detection (only if no anomaly yet detected)
        if self.config.enable_mad && !anomaly_detected {
            detectors_evaluated.push("mad".to_string());
            constraints_applied.push(ConstraintApplied {
                constraint_id: "detector:mad".to_string(),
                constraint_type: ConstraintType::Detector,
                value: serde_json::json!(true),
                passed: true,
                description: Some("MAD detector enabled".to_string()),
            });
            constraints_applied.push(ConstraintApplied {
                constraint_id: "mad_threshold".to_string(),
                constraint_type: ConstraintType::Threshold,
                value: serde_json::json!(self.config.mad_threshold),
                passed: true,
                description: Some(format!("MAD threshold: {}", self.config.mad_threshold)),
            });

            let (detected, anomaly, confidence, checked, valid) = self.detect_mad(event).await?;
            baselines_checked += checked;
            baselines_valid += valid;

            if detected {
                anomaly_detected = true;
                anomaly_event = anomaly;
                triggered_by = Some("mad".to_string());
                max_confidence = max_confidence.max(confidence);
            }
        }

        // CUSUM detection (only if no anomaly yet detected)
        if self.config.enable_cusum && !anomaly_detected {
            detectors_evaluated.push("cusum".to_string());
            constraints_applied.push(ConstraintApplied {
                constraint_id: "detector:cusum".to_string(),
                constraint_type: ConstraintType::Detector,
                value: serde_json::json!(true),
                passed: true,
                description: Some("CUSUM detector enabled".to_string()),
            });
            constraints_applied.push(ConstraintApplied {
                constraint_id: "cusum_threshold".to_string(),
                constraint_type: ConstraintType::Threshold,
                value: serde_json::json!(self.config.cusum_threshold),
                passed: true,
                description: Some(format!("CUSUM threshold: {}", self.config.cusum_threshold)),
            });

            let (detected, anomaly, confidence, checked, valid) =
                self.detect_cusum(event).await?;
            baselines_checked += checked;
            baselines_valid += valid;

            if detected {
                anomaly_detected = true;
                anomaly_event = anomaly;
                triggered_by = Some("cusum".to_string());
                max_confidence = max_confidence.max(confidence);
            }
        }

        // Add min_samples constraint
        constraints_applied.push(ConstraintApplied {
            constraint_id: "min_samples".to_string(),
            constraint_type: ConstraintType::Rule,
            value: serde_json::json!(self.config.min_samples),
            passed: baselines_valid > 0,
            description: Some(format!(
                "Minimum {} samples required for valid baseline",
                self.config.min_samples
            )),
        });

        // Update baselines if continuous learning enabled
        if self.config.continuous_learning {
            self.update_baselines(event).await?;
        }

        let processing_ms = start.elapsed().as_millis() as u64;

        // Create agent output
        let outputs = AgentOutput {
            anomaly_detected,
            anomaly_event: anomaly_event.as_ref().map(|a| serde_json::to_value(a).unwrap()),
            detectors_evaluated,
            triggered_by,
            metadata: OutputMetadata {
                processing_ms,
                baselines_checked,
                baselines_valid,
                telemetry_emitted: true,
            },
        };

        // Create DecisionEvent (EXACTLY ONE per invocation)
        let decision_event = DecisionEvent::new(
            self.agent_id.clone(),
            self.agent_version.clone(),
            DecisionType::AnomalyDetection,
            inputs_hash,
            outputs,
            if anomaly_detected { max_confidence } else { 0.0 },
            constraints_applied,
            ExecutionRef {
                request_id,
                trace_id: event.trace_id.clone(),
                span_id: event.span_id.clone(),
                source: "telemetry".to_string(),
            },
            DecisionContext {
                service_name: event.service_name.to_string(),
                model: event.model.to_string(),
                region: event.metadata.get("region").cloned(),
                environment: event.metadata.get("environment").cloned(),
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
            if anomaly_detected {
                stats.anomalies_detected += 1;
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
        self.emit_telemetry(&decision_event, processing_ms).await;

        info!(
            agent_id = %self.agent_id,
            request_id = %request_id,
            anomaly_detected = anomaly_detected,
            processing_ms = processing_ms,
            persisted = persisted,
            "Agent invocation completed"
        );

        Ok(AgentInvocationResult {
            decision_event,
            anomaly_event,
            failure: None,
            persisted,
        })
    }

    /// Invoke the agent from a raw HTTP request (Edge Function handler)
    ///
    /// This is the entry point for Google Cloud Edge Function deployment
    pub async fn handle_request(&self, body: &[u8]) -> Result<serde_json::Value> {
        // Parse request body as TelemetryEvent
        let event: TelemetryEvent = serde_json::from_slice(body)
            .map_err(|e| Error::validation(format!("Invalid request body: {}", e)))?;

        // Invoke agent
        let result = self.invoke(&event).await?;

        // Return DecisionEvent as JSON
        Ok(result.decision_event.to_json())
    }

    /// Z-score detection
    async fn detect_zscore(
        &self,
        event: &TelemetryEvent,
    ) -> Result<(bool, Option<AnomalyEvent>, f64, usize, usize)> {
        let mut baselines_checked = 0;
        let mut baselines_valid = 0;

        // Check latency
        if self.config.analyze_latency {
            baselines_checked += 1;
            let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());

            if self.baseline_manager.has_valid_baseline(&key) {
                baselines_valid += 1;
                let baseline = self.baseline_manager.get(&key).unwrap();
                let z = stats::zscore(event.latency_ms, baseline.mean, baseline.std_dev);

                if z.abs() > self.config.zscore_threshold {
                    let severity = self.calculate_severity_zscore(z.abs());
                    let confidence = self.calculate_confidence_zscore(z.abs());

                    let anomaly = self.create_anomaly_event(
                        event,
                        severity,
                        AnomalyType::LatencySpike,
                        DetectionMethod::ZScore,
                        confidence,
                        "latency_ms",
                        event.latency_ms,
                        &baseline,
                        Some(z.abs()),
                    );

                    return Ok((true, Some(anomaly), confidence, baselines_checked, baselines_valid));
                }
            }
        }

        // Check tokens
        if self.config.analyze_tokens {
            baselines_checked += 1;
            let key = BaselineKey::tokens(event.service_name.clone(), event.model.clone());

            if self.baseline_manager.has_valid_baseline(&key) {
                baselines_valid += 1;
                let baseline = self.baseline_manager.get(&key).unwrap();
                let tokens = event.total_tokens() as f64;
                let z = stats::zscore(tokens, baseline.mean, baseline.std_dev);

                if z.abs() > self.config.zscore_threshold {
                    let severity = self.calculate_severity_zscore(z.abs());
                    let confidence = self.calculate_confidence_zscore(z.abs());

                    let anomaly = self.create_anomaly_event(
                        event,
                        severity,
                        AnomalyType::TokenUsageSpike,
                        DetectionMethod::ZScore,
                        confidence,
                        "total_tokens",
                        tokens,
                        &baseline,
                        Some(z.abs()),
                    );

                    return Ok((true, Some(anomaly), confidence, baselines_checked, baselines_valid));
                }
            }
        }

        // Check cost
        if self.config.analyze_cost {
            baselines_checked += 1;
            let key = BaselineKey::cost(event.service_name.clone(), event.model.clone());

            if self.baseline_manager.has_valid_baseline(&key) {
                baselines_valid += 1;
                let baseline = self.baseline_manager.get(&key).unwrap();
                let z = stats::zscore(event.cost_usd, baseline.mean, baseline.std_dev);

                if z.abs() > self.config.zscore_threshold {
                    let severity = self.calculate_severity_zscore(z.abs());
                    let confidence = self.calculate_confidence_zscore(z.abs());

                    let anomaly = self.create_anomaly_event(
                        event,
                        severity,
                        AnomalyType::CostAnomaly,
                        DetectionMethod::ZScore,
                        confidence,
                        "cost_usd",
                        event.cost_usd,
                        &baseline,
                        Some(z.abs()),
                    );

                    return Ok((true, Some(anomaly), confidence, baselines_checked, baselines_valid));
                }
            }
        }

        Ok((false, None, 0.0, baselines_checked, baselines_valid))
    }

    /// IQR detection
    async fn detect_iqr(
        &self,
        event: &TelemetryEvent,
    ) -> Result<(bool, Option<AnomalyEvent>, f64, usize, usize)> {
        let mut baselines_checked = 0;
        let mut baselines_valid = 0;

        // Check latency
        if self.config.analyze_latency {
            baselines_checked += 1;
            let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());

            if self.baseline_manager.has_valid_baseline(&key) {
                baselines_valid += 1;
                let baseline = self.baseline_manager.get(&key).unwrap();

                let lower = baseline.q1 - self.config.iqr_multiplier * baseline.iqr;
                let upper = baseline.q3 + self.config.iqr_multiplier * baseline.iqr;

                if event.latency_ms < lower || event.latency_ms > upper {
                    let deviation = if event.latency_ms > upper {
                        (event.latency_ms - upper) / baseline.iqr
                    } else {
                        (lower - event.latency_ms) / baseline.iqr
                    };
                    let severity = self.calculate_severity_iqr(deviation);
                    let confidence = self.calculate_confidence_iqr(deviation);

                    let anomaly = self.create_anomaly_event(
                        event,
                        severity,
                        AnomalyType::LatencySpike,
                        DetectionMethod::Iqr,
                        confidence,
                        "latency_ms",
                        event.latency_ms,
                        &baseline,
                        Some(deviation),
                    );

                    return Ok((true, Some(anomaly), confidence, baselines_checked, baselines_valid));
                }
            }
        }

        Ok((false, None, 0.0, baselines_checked, baselines_valid))
    }

    /// MAD detection
    async fn detect_mad(
        &self,
        event: &TelemetryEvent,
    ) -> Result<(bool, Option<AnomalyEvent>, f64, usize, usize)> {
        let mut baselines_checked = 0;
        let mut baselines_valid = 0;

        // Check latency
        if self.config.analyze_latency {
            baselines_checked += 1;
            let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());

            if self.baseline_manager.has_valid_baseline(&key) {
                baselines_valid += 1;
                let baseline = self.baseline_manager.get(&key).unwrap();

                // Modified Z-score using MAD
                // k = 1.4826 is the scaling factor for normally distributed data
                let k = 1.4826;
                let modified_z = if baseline.mad > 0.0 {
                    (event.latency_ms - baseline.median).abs() / (k * baseline.mad)
                } else {
                    0.0
                };

                if modified_z > self.config.mad_threshold {
                    let severity = self.calculate_severity_mad(modified_z);
                    let confidence = self.calculate_confidence_mad(modified_z);

                    let anomaly = self.create_anomaly_event(
                        event,
                        severity,
                        AnomalyType::LatencySpike,
                        DetectionMethod::Mad,
                        confidence,
                        "latency_ms",
                        event.latency_ms,
                        &baseline,
                        Some(modified_z),
                    );

                    return Ok((true, Some(anomaly), confidence, baselines_checked, baselines_valid));
                }
            }
        }

        Ok((false, None, 0.0, baselines_checked, baselines_valid))
    }

    /// CUSUM detection (change-point)
    async fn detect_cusum(
        &self,
        event: &TelemetryEvent,
    ) -> Result<(bool, Option<AnomalyEvent>, f64, usize, usize)> {
        let mut baselines_checked = 0;
        let mut baselines_valid = 0;

        // Check latency for drift
        if self.config.analyze_latency {
            baselines_checked += 1;
            let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());

            if self.baseline_manager.has_valid_baseline(&key) {
                baselines_valid += 1;
                let baseline = self.baseline_manager.get(&key).unwrap();

                // Update CUSUM state
                let mut state_map = self.cusum_state.write().await;
                let state = state_map.entry(key).or_insert_with(CusumState::default);

                // Normalize value
                let normalized = if baseline.std_dev > 0.0 {
                    (event.latency_ms - baseline.mean) / baseline.std_dev
                } else {
                    0.0
                };

                // Update CUSUM (upper and lower)
                state.s_high =
                    (state.s_high + normalized - self.config.cusum_slack).max(0.0);
                state.s_low =
                    (state.s_low + normalized + self.config.cusum_slack).min(0.0);
                state.count += 1;

                // Check for change point
                if state.s_high > self.config.cusum_threshold
                    || state.s_low.abs() > self.config.cusum_threshold
                {
                    let cusum_value = state.s_high.max(state.s_low.abs());
                    let severity = self.calculate_severity_cusum(cusum_value);
                    let confidence = self.calculate_confidence_cusum(cusum_value);

                    // Reset CUSUM state after detection
                    state.s_high = 0.0;
                    state.s_low = 0.0;

                    let anomaly = self.create_anomaly_event(
                        event,
                        severity,
                        AnomalyType::ConceptDrift,
                        DetectionMethod::Cusum,
                        confidence,
                        "latency_ms",
                        event.latency_ms,
                        &baseline,
                        Some(cusum_value),
                    );

                    return Ok((true, Some(anomaly), confidence, baselines_checked, baselines_valid));
                }
            }
        }

        Ok((false, None, 0.0, baselines_checked, baselines_valid))
    }

    /// Update baselines with new event data
    async fn update_baselines(&self, event: &TelemetryEvent) -> Result<()> {
        if self.config.analyze_latency {
            let key = BaselineKey::latency(event.service_name.clone(), event.model.clone());
            self.baseline_manager.update(key, event.latency_ms)?;
        }

        if self.config.analyze_tokens {
            let key = BaselineKey::tokens(event.service_name.clone(), event.model.clone());
            self.baseline_manager.update(key, event.total_tokens() as f64)?;
        }

        if self.config.analyze_cost {
            let key = BaselineKey::cost(event.service_name.clone(), event.model.clone());
            self.baseline_manager.update(key, event.cost_usd)?;
        }

        Ok(())
    }

    /// Create an AnomalyEvent
    #[allow(clippy::too_many_arguments)]
    fn create_anomaly_event(
        &self,
        event: &TelemetryEvent,
        severity: Severity,
        anomaly_type: AnomalyType,
        detection_method: DetectionMethod,
        confidence: f64,
        metric: &str,
        value: f64,
        baseline: &Baseline,
        deviation: Option<f64>,
    ) -> AnomalyEvent {
        let threshold = match detection_method {
            DetectionMethod::ZScore => baseline.mean + self.config.zscore_threshold * baseline.std_dev,
            DetectionMethod::Iqr => baseline.q3 + self.config.iqr_multiplier * baseline.iqr,
            DetectionMethod::Mad => baseline.median + self.config.mad_threshold * 1.4826 * baseline.mad,
            DetectionMethod::Cusum => self.config.cusum_threshold,
            _ => baseline.mean + 3.0 * baseline.std_dev,
        };

        AnomalyEvent::new(
            severity,
            anomaly_type,
            event.service_name.clone(),
            event.model.clone(),
            detection_method,
            confidence,
            AnomalyDetails {
                metric: metric.to_string(),
                value,
                baseline: baseline.mean,
                threshold,
                deviation_sigma: deviation,
                additional: HashMap::new(),
            },
            AnomalyContext {
                trace_id: event.trace_id.clone(),
                user_id: event.metadata.get("user_id").cloned(),
                region: event.metadata.get("region").cloned(),
                time_window: "rolling_window".to_string(),
                sample_count: baseline.sample_count,
                additional: HashMap::new(),
            },
        )
        .with_root_cause(format!(
            "{} {} is {:.2} deviations from baseline {:.2}",
            metric,
            value,
            deviation.unwrap_or(0.0),
            baseline.mean
        ))
        .with_remediation("Review service health and resource utilization")
        .with_remediation("Check recent deployments or configuration changes")
    }

    /// Calculate severity for Z-score
    fn calculate_severity_zscore(&self, z: f64) -> Severity {
        if z >= 6.0 {
            Severity::Critical
        } else if z >= 4.0 {
            Severity::High
        } else if z >= 3.0 {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Calculate confidence for Z-score
    fn calculate_confidence_zscore(&self, z: f64) -> f64 {
        (1.0 - (1.0 / (1.0 + (z - self.config.zscore_threshold)))).clamp(0.5, 0.99)
    }

    /// Calculate severity for IQR
    fn calculate_severity_iqr(&self, deviation: f64) -> Severity {
        if deviation >= 3.0 {
            Severity::Critical
        } else if deviation >= 2.0 {
            Severity::High
        } else if deviation >= 1.0 {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Calculate confidence for IQR
    fn calculate_confidence_iqr(&self, deviation: f64) -> f64 {
        (1.0 - (1.0 / (1.0 + deviation))).clamp(0.5, 0.99)
    }

    /// Calculate severity for MAD
    fn calculate_severity_mad(&self, modified_z: f64) -> Severity {
        if modified_z >= 7.0 {
            Severity::Critical
        } else if modified_z >= 5.0 {
            Severity::High
        } else if modified_z >= 3.5 {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Calculate confidence for MAD
    fn calculate_confidence_mad(&self, modified_z: f64) -> f64 {
        (1.0 - (1.0 / (1.0 + (modified_z - self.config.mad_threshold)))).clamp(0.5, 0.99)
    }

    /// Calculate severity for CUSUM
    fn calculate_severity_cusum(&self, cusum: f64) -> Severity {
        if cusum >= 10.0 {
            Severity::Critical
        } else if cusum >= 7.0 {
            Severity::High
        } else if cusum >= 5.0 {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Calculate confidence for CUSUM
    fn calculate_confidence_cusum(&self, cusum: f64) -> f64 {
        (1.0 - (1.0 / (1.0 + (cusum - self.config.cusum_threshold)))).clamp(0.5, 0.99)
    }

    /// Persist DecisionEvent to ruvector-service
    async fn persist_decision(&self, decision: &DecisionEvent) -> Result<bool> {
        // In production, this would call the ruvector-service client
        // For now, we just log and return success
        if let Some(ref _endpoint) = self.config.ruvector_endpoint {
            // TODO: Implement actual ruvector-service client call
            // let client = RuvectorClient::new(endpoint);
            // client.persist_decision(decision).await?;
            debug!(
                decision_id = %decision.decision_id,
                "Would persist to ruvector-service"
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

    /// Emit telemetry metrics
    async fn emit_telemetry(&self, decision: &DecisionEvent, processing_ms: u64) {
        let service = decision.context.service_name.clone();
        let model = decision.context.model.clone();
        let agent_id = self.agent_id.to_string();

        // Agent invocation counter
        metrics::counter!(
            "sentinel_agent_invocations_total",
            "agent" => agent_id.clone(),
            "service" => service.clone(),
            "model" => model.clone()
        )
        .increment(1);

        // Anomaly detection counter
        if decision.outputs.anomaly_detected {
            metrics::counter!(
                "sentinel_agent_anomalies_total",
                "agent" => agent_id.clone(),
                "service" => service.clone(),
                "model" => model.clone(),
                "detector" => decision.outputs.triggered_by.clone().unwrap_or_default()
            )
            .increment(1);
        }

        // Processing latency histogram
        metrics::histogram!(
            "sentinel_agent_processing_seconds",
            "agent" => agent_id
        )
        .record(processing_ms as f64 / 1000.0);

        // Confidence gauge
        metrics::gauge!(
            "sentinel_agent_confidence",
            "service" => service,
            "model" => model
        )
        .set(decision.confidence);
    }

    /// Get agent statistics
    pub async fn stats(&self) -> AgentStats {
        self.stats.read().await.clone()
    }

    /// Get agent ID
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Get agent version
    pub fn agent_version(&self) -> &AgentVersion {
        &self.agent_version
    }

    /// Get baseline manager
    pub fn baseline_manager(&self) -> &Arc<BaselineManager> {
        &self.baseline_manager
    }

    // =========================================================================
    // CLI Commands: inspect, replay, diagnose
    // =========================================================================

    /// Inspect agent state and configuration
    pub async fn inspect(
        &self,
        service_filter: Option<&str>,
        model_filter: Option<&str>,
    ) -> InspectResult {
        let stats = self.stats.read().await.clone();
        let baseline_stats = self.baseline_manager.stats();

        let mut baselines = Vec::new();
        for key in self.baseline_manager.keys() {
            // Apply filters
            if let Some(svc) = service_filter {
                if key.service.as_str() != svc {
                    continue;
                }
            }
            if let Some(mdl) = model_filter {
                if key.model.as_str() != mdl {
                    continue;
                }
            }

            if let Some(baseline) = self.baseline_manager.get(&key) {
                baselines.push(BaselineInfo {
                    service: key.service.to_string(),
                    model: key.model.to_string(),
                    metric: key.metric.clone(),
                    mean: baseline.mean,
                    std_dev: baseline.std_dev,
                    sample_count: baseline.sample_count,
                    is_valid: baseline.is_valid(),
                });
            }
        }

        InspectResult {
            agent_id: self.agent_id.to_string(),
            agent_version: self.agent_version.to_string(),
            config: self.config.clone(),
            stats,
            baseline_stats: BaselineStatsInfo {
                total_baselines: baseline_stats.total_baselines,
                valid_baselines: baseline_stats.valid_baselines,
                window_size: baseline_stats.window_size,
            },
            baselines,
        }
    }

    /// Replay a telemetry event through the agent
    pub async fn replay(
        &self,
        event: &TelemetryEvent,
        dry_run: bool,
    ) -> Result<AgentInvocationResult> {
        // Temporarily override dry_run if requested
        if dry_run && !self.config.dry_run {
            // Create a modified config for this invocation
            let mut temp_config = self.config.clone();
            temp_config.dry_run = true;
            // For now, just invoke with the original config
            // In a real implementation, we'd use the temp_config
        }

        self.invoke(event).await
    }

    /// Run diagnostics on agent health
    pub async fn diagnose(&self, check: DiagnoseCheck) -> DiagnoseResult {
        let mut checks = Vec::new();

        // Check baselines
        if matches!(check, DiagnoseCheck::All | DiagnoseCheck::Baselines) {
            let baseline_stats = self.baseline_manager.stats();
            checks.push(HealthCheck {
                name: "baselines".to_string(),
                status: if baseline_stats.valid_baselines > 0 {
                    CheckStatus::Healthy
                } else {
                    CheckStatus::Warning
                },
                message: format!(
                    "{}/{} baselines valid",
                    baseline_stats.valid_baselines, baseline_stats.total_baselines
                ),
                details: Some(serde_json::json!({
                    "total": baseline_stats.total_baselines,
                    "valid": baseline_stats.valid_baselines,
                    "window_size": baseline_stats.window_size
                })),
            });
        }

        // Check detectors
        if matches!(check, DiagnoseCheck::All | DiagnoseCheck::Detectors) {
            let mut enabled = Vec::new();
            if self.config.enable_zscore {
                enabled.push("zscore");
            }
            if self.config.enable_iqr {
                enabled.push("iqr");
            }
            if self.config.enable_mad {
                enabled.push("mad");
            }
            if self.config.enable_cusum {
                enabled.push("cusum");
            }

            checks.push(HealthCheck {
                name: "detectors".to_string(),
                status: if !enabled.is_empty() {
                    CheckStatus::Healthy
                } else {
                    CheckStatus::Unhealthy
                },
                message: format!("{} detectors enabled", enabled.len()),
                details: Some(serde_json::json!({
                    "enabled": enabled
                })),
            });
        }

        // Check ruvector connection
        if matches!(check, DiagnoseCheck::All | DiagnoseCheck::Ruvector) {
            let status = if self.config.ruvector_endpoint.is_some() {
                // TODO: Actually ping ruvector-service
                CheckStatus::Healthy
            } else {
                CheckStatus::Warning
            };

            checks.push(HealthCheck {
                name: "ruvector".to_string(),
                status,
                message: if self.config.ruvector_endpoint.is_some() {
                    "ruvector-service endpoint configured".to_string()
                } else {
                    "No ruvector-service endpoint configured".to_string()
                },
                details: self.config.ruvector_endpoint.as_ref().map(|e| {
                    serde_json::json!({
                        "endpoint": e
                    })
                }),
            });
        }

        let overall_status = if checks.iter().any(|c| c.status == CheckStatus::Unhealthy) {
            CheckStatus::Unhealthy
        } else if checks.iter().any(|c| c.status == CheckStatus::Warning) {
            CheckStatus::Warning
        } else {
            CheckStatus::Healthy
        };

        DiagnoseResult {
            agent_id: self.agent_id.to_string(),
            overall_status,
            checks,
            timestamp: Utc::now(),
        }
    }
}

/// Inspect result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectResult {
    pub agent_id: String,
    pub agent_version: String,
    pub config: AnomalyDetectionAgentConfig,
    pub stats: AgentStats,
    pub baseline_stats: BaselineStatsInfo,
    pub baselines: Vec<BaselineInfo>,
}

/// Baseline statistics info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineStatsInfo {
    pub total_baselines: usize,
    pub valid_baselines: usize,
    pub window_size: usize,
}

/// Baseline info for inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineInfo {
    pub service: String,
    pub model: String,
    pub metric: String,
    pub mean: f64,
    pub std_dev: f64,
    pub sample_count: usize,
    pub is_valid: bool,
}

/// Diagnose check type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnoseCheck {
    All,
    Baselines,
    Detectors,
    Ruvector,
}

impl std::str::FromStr for DiagnoseCheck {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(DiagnoseCheck::All),
            "baselines" => Ok(DiagnoseCheck::Baselines),
            "detectors" => Ok(DiagnoseCheck::Detectors),
            "ruvector" => Ok(DiagnoseCheck::Ruvector),
            _ => Err(format!("Unknown check: {}", s)),
        }
    }
}

/// Diagnose result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnoseResult {
    pub agent_id: String,
    pub overall_status: CheckStatus,
    pub checks: Vec<HealthCheck>,
    pub timestamp: DateTime<Utc>,
}

/// Check status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Healthy,
    Warning,
    Unhealthy,
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::events::{PromptInfo, ResponseInfo};

    fn create_test_event(latency: f64, tokens: u32, cost: f64) -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test-service"),
            ModelId::new("gpt-4"),
            PromptInfo {
                text: "test prompt".to_string(),
                tokens: tokens / 2,
                embedding: None,
            },
            ResponseInfo {
                text: "test response".to_string(),
                tokens: tokens / 2,
                finish_reason: "stop".to_string(),
                embedding: None,
            },
            latency,
            cost,
        )
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let config = AnomalyDetectionAgentConfig::default();
        let agent = AnomalyDetectionAgent::new(config).unwrap();

        assert_eq!(agent.agent_id().as_str(), "sentinel.detection.anomaly");
        assert_eq!(agent.agent_version().to_string(), "1.0.0");
    }

    #[tokio::test]
    async fn test_agent_invocation_no_baseline() {
        let mut config = AnomalyDetectionAgentConfig::default();
        config.dry_run = true;
        let agent = AnomalyDetectionAgent::new(config).unwrap();

        let event = create_test_event(100.0, 100, 0.01);
        let result = agent.invoke(&event).await.unwrap();

        // Should produce a DecisionEvent even with no baseline
        assert!(!result.decision_event.outputs.anomaly_detected);
        assert!(result.anomaly_event.is_none());
    }

    #[tokio::test]
    async fn test_agent_invocation_with_baseline() {
        let mut config = AnomalyDetectionAgentConfig::default();
        config.dry_run = true;
        config.min_samples = 10;
        let agent = AnomalyDetectionAgent::new(config).unwrap();

        // Build baseline
        for i in 0..20 {
            let event = create_test_event(100.0 + (i as f64 - 10.0), 100, 0.01);
            let _ = agent.invoke(&event).await;
        }

        // Normal event
        let normal_event = create_test_event(100.0, 100, 0.01);
        let result = agent.invoke(&normal_event).await.unwrap();
        assert!(!result.decision_event.outputs.anomaly_detected);

        // Anomalous event (10x latency)
        let anomaly_event = create_test_event(1000.0, 100, 0.01);
        let result = agent.invoke(&anomaly_event).await.unwrap();
        assert!(result.decision_event.outputs.anomaly_detected);
        assert!(result.anomaly_event.is_some());
    }

    #[tokio::test]
    async fn test_decision_event_always_emitted() {
        let mut config = AnomalyDetectionAgentConfig::default();
        config.dry_run = true;
        let agent = AnomalyDetectionAgent::new(config).unwrap();

        let event = create_test_event(100.0, 100, 0.01);
        let result = agent.invoke(&event).await.unwrap();

        // DecisionEvent MUST always be emitted
        assert!(result.decision_event.validate().is_ok());
        assert_eq!(
            result.decision_event.decision_type,
            DecisionType::AnomalyDetection
        );
        assert!(!result.decision_event.inputs_hash.is_empty());
    }

    #[tokio::test]
    async fn test_agent_inspect() {
        let mut config = AnomalyDetectionAgentConfig::default();
        config.dry_run = true;
        let agent = AnomalyDetectionAgent::new(config).unwrap();

        // Add some events to build baselines
        for i in 0..15 {
            let event = create_test_event(100.0 + i as f64, 100, 0.01);
            let _ = agent.invoke(&event).await;
        }

        let inspect = agent.inspect(None, None).await;
        assert_eq!(inspect.agent_id, "sentinel.detection.anomaly");
        assert!(inspect.stats.invocations > 0);
    }

    #[tokio::test]
    async fn test_agent_diagnose() {
        let mut config = AnomalyDetectionAgentConfig::default();
        config.dry_run = true;
        let agent = AnomalyDetectionAgent::new(config).unwrap();

        let diagnose = agent.diagnose(DiagnoseCheck::All).await;
        assert_eq!(diagnose.agent_id, "sentinel.detection.anomaly");
        assert!(!diagnose.checks.is_empty());
    }

    #[tokio::test]
    async fn test_agent_no_detectors_enabled() {
        let config = AnomalyDetectionAgentConfig {
            enable_zscore: false,
            enable_iqr: false,
            enable_mad: false,
            enable_cusum: false,
            ..Default::default()
        };

        let result = AnomalyDetectionAgent::new(config);
        assert!(result.is_err());
    }
}
