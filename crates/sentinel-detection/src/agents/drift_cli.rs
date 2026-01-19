//! CLI Commands for Drift Detection Agent
//!
//! This module provides the CLI interface for the Drift Detection Agent
//! following the agentics-cli specification.
//!
//! # CLI Specification
//!
//! ```bash
//! # Inspect drift state
//! sentinel drift inspect --service <service> --model <model> --metric <metric>
//!
//! # Replay historical events
//! sentinel drift replay --file <events.json> --speed <multiplier>
//!
//! # Diagnose drift detection
//! sentinel drift diagnose --service <service> --model <model> --output <report.json>
//! ```

use crate::agents::drift_agent::{
    DriftCheckStatus, DriftDetectionAgent, DriftDetectionAgentConfig, DriftDiagnoseCheck,
    DriftDiagnoseResult, DriftInspectResult,
};
use llm_sentinel_core::{events::TelemetryEvent, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// CLI command specification for Drift Detection Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftCliContract {
    pub agent_name: String,
    pub commands: Vec<DriftCliCommand>,
}

/// CLI command definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftCliCommand {
    pub name: String,
    pub description: String,
    pub required_args: Vec<DriftCliArg>,
    pub optional_args: Vec<DriftCliArg>,
    pub example: String,
}

/// CLI argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftCliArg {
    pub name: String,
    pub arg_type: String,
    pub description: String,
}

impl DriftCliContract {
    /// Generate CLI contract for Drift Detection Agent
    pub fn new() -> Self {
        Self {
            agent_name: "drift".to_string(),
            commands: vec![
                DriftCliCommand {
                    name: "inspect".to_string(),
                    description: "Inspect drift detection state, windows, and configuration"
                        .to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        DriftCliArg {
                            name: "--service".to_string(),
                            arg_type: "string".to_string(),
                            description: "Filter by service name".to_string(),
                        },
                        DriftCliArg {
                            name: "--model".to_string(),
                            arg_type: "string".to_string(),
                            description: "Filter by model name".to_string(),
                        },
                        DriftCliArg {
                            name: "--metric".to_string(),
                            arg_type: "string".to_string(),
                            description:
                                "Filter by metric (latency_ms, cost_usd, total_tokens, error_rate)"
                                    .to_string(),
                        },
                        DriftCliArg {
                            name: "--json".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Output as JSON".to_string(),
                        },
                    ],
                    example: "sentinel drift inspect --service chat-api --metric latency_ms --json"
                        .to_string(),
                },
                DriftCliCommand {
                    name: "replay".to_string(),
                    description: "Replay telemetry events through the drift detector".to_string(),
                    required_args: vec![DriftCliArg {
                        name: "--file".to_string(),
                        arg_type: "path".to_string(),
                        description: "Path to JSON file with telemetry events".to_string(),
                    }],
                    optional_args: vec![
                        DriftCliArg {
                            name: "--speed".to_string(),
                            arg_type: "float".to_string(),
                            description: "Playback speed multiplier (default: 1.0)".to_string(),
                        },
                        DriftCliArg {
                            name: "--dry-run".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Don't persist DecisionEvents".to_string(),
                        },
                        DriftCliArg {
                            name: "--verbose".to_string(),
                            arg_type: "flag".to_string(),
                            description: "Show detailed processing output".to_string(),
                        },
                    ],
                    example: "sentinel drift replay --file events.json --speed 2.0 --dry-run"
                        .to_string(),
                },
                DriftCliCommand {
                    name: "diagnose".to_string(),
                    description: "Run diagnostics on drift detection agent health".to_string(),
                    required_args: vec![],
                    optional_args: vec![
                        DriftCliArg {
                            name: "--check".to_string(),
                            arg_type: "string".to_string(),
                            description: "Specific check (states, metrics, ruvector, all)"
                                .to_string(),
                        },
                        DriftCliArg {
                            name: "--service".to_string(),
                            arg_type: "string".to_string(),
                            description: "Service to diagnose".to_string(),
                        },
                        DriftCliArg {
                            name: "--model".to_string(),
                            arg_type: "string".to_string(),
                            description: "Model to diagnose".to_string(),
                        },
                        DriftCliArg {
                            name: "--output".to_string(),
                            arg_type: "path".to_string(),
                            description: "Output report file path".to_string(),
                        },
                    ],
                    example: "sentinel drift diagnose --check all --output report.json".to_string(),
                },
            ],
        }
    }
}

impl Default for DriftCliContract {
    fn default() -> Self {
        Self::new()
    }
}

/// CLI handler for drift commands
pub struct DriftCliHandler {
    agent: DriftDetectionAgent,
}

impl DriftCliHandler {
    /// Create a new CLI handler
    pub fn new(config: DriftDetectionAgentConfig) -> Result<Self> {
        let agent = DriftDetectionAgent::new(config)?;
        Ok(Self { agent })
    }

    /// Execute inspect command
    pub async fn inspect(
        &self,
        service: Option<&str>,
        model: Option<&str>,
        metric: Option<&str>,
        json_output: bool,
    ) -> Result<String> {
        let result = self.agent.inspect(service, model, metric).await;

        if json_output {
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        } else {
            Ok(self.format_inspect_result(&result))
        }
    }

    /// Execute replay command
    pub async fn replay(
        &self,
        events_file: PathBuf,
        _speed: f64,
        dry_run: bool,
        verbose: bool,
    ) -> Result<String> {
        // Load events from file
        let content =
            std::fs::read_to_string(&events_file).map_err(|e| llm_sentinel_core::Error::Io(e))?;

        let events: Vec<TelemetryEvent> = serde_json::from_str(&content)
            .map_err(|e| llm_sentinel_core::Error::validation(format!("Invalid JSON: {}", e)))?;

        let mut results = Vec::new();
        let mut drifts_detected = 0;

        for (i, event) in events.iter().enumerate() {
            let result = self.agent.replay(event, dry_run).await?;

            if result.decision_event.outputs.anomaly_detected {
                drifts_detected += 1;
            }

            if verbose {
                results.push(format!(
                    "[{}/{}] event_id={} drift={} confidence={:.2}",
                    i + 1,
                    events.len(),
                    event.event_id,
                    result.decision_event.outputs.anomaly_detected,
                    result.decision_event.confidence
                ));
            }
        }

        let summary = format!(
            "Replayed {} events, detected {} drifts{}",
            events.len(),
            drifts_detected,
            if dry_run { " (dry-run)" } else { "" }
        );

        if verbose {
            Ok(format!("{}\n\n{}", results.join("\n"), summary))
        } else {
            Ok(summary)
        }
    }

    /// Execute diagnose command
    pub async fn diagnose(
        &self,
        check: Option<&str>,
        output_file: Option<PathBuf>,
        json_output: bool,
    ) -> Result<String> {
        let check_type = check
            .map(|c| c.parse().unwrap_or(DriftDiagnoseCheck::All))
            .unwrap_or(DriftDiagnoseCheck::All);

        let result = self.agent.diagnose(check_type).await;

        // Write to file if specified
        if let Some(path) = output_file {
            let json = serde_json::to_string_pretty(&result).unwrap_or_default();
            std::fs::write(&path, &json).map_err(|e| llm_sentinel_core::Error::Io(e))?;
        }

        if json_output {
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        } else {
            Ok(self.format_diagnose_result(&result))
        }
    }

    /// Format inspect result for human-readable output
    fn format_inspect_result(&self, result: &DriftInspectResult) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Drift Detection Agent: {}", result.agent_id));
        lines.push(format!("Version: {}", result.agent_version));
        lines.push(String::new());

        lines.push("Statistics:".to_string());
        lines.push(format!("  Invocations: {}", result.stats.invocations));
        lines.push(format!("  Drifts Detected: {}", result.stats.drifts_detected));
        lines.push(format!(
            "  Avg Processing: {:.2}ms",
            result.stats.avg_processing_ms
        ));
        lines.push(String::new());

        lines.push("Drift States:".to_string());
        if result.drift_states.is_empty() {
            lines.push("  No drift states tracked yet".to_string());
        } else {
            for state in &result.drift_states {
                lines.push(format!(
                    "  - {}/{}/{}:",
                    state.service, state.model, state.metric
                ));
                lines.push(format!(
                    "      Reference: {} samples, mean={:.2}",
                    state.reference_samples, state.reference_mean
                ));
                lines.push(format!(
                    "      Current: {} samples, mean={:.2}",
                    state.current_samples, state.current_mean
                ));
                lines.push(format!(
                    "      Drifts: {}, Sufficient Data: {}",
                    state.drifts_detected, state.has_sufficient_data
                ));
            }
        }

        lines.join("\n")
    }

    /// Format diagnose result for human-readable output
    fn format_diagnose_result(&self, result: &DriftDiagnoseResult) -> String {
        let mut lines = Vec::new();

        let status_icon = match result.overall_status {
            DriftCheckStatus::Healthy => "✓",
            DriftCheckStatus::Warning => "⚠",
            DriftCheckStatus::Unhealthy => "✗",
        };

        lines.push(format!(
            "{} Drift Detection Agent: {}",
            status_icon, result.agent_id
        ));
        lines.push(format!("Timestamp: {}", result.timestamp));
        lines.push(String::new());

        lines.push("Health Checks:".to_string());
        for check in &result.checks {
            let icon = match check.status {
                DriftCheckStatus::Healthy => "✓",
                DriftCheckStatus::Warning => "⚠",
                DriftCheckStatus::Unhealthy => "✗",
            };
            lines.push(format!("  {} {}: {}", icon, check.name, check.message));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_contract() {
        let contract = DriftCliContract::new();
        assert_eq!(contract.agent_name, "drift");
        assert_eq!(contract.commands.len(), 3);

        let inspect = contract.commands.iter().find(|c| c.name == "inspect");
        assert!(inspect.is_some());

        let replay = contract.commands.iter().find(|c| c.name == "replay");
        assert!(replay.is_some());
        assert_eq!(replay.unwrap().required_args.len(), 1);

        let diagnose = contract.commands.iter().find(|c| c.name == "diagnose");
        assert!(diagnose.is_some());
    }
}
