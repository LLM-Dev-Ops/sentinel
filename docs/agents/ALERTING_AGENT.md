# Alerting Agent - LLM-Sentinel

## Agent Contract Summary

| Property | Value |
|----------|-------|
| **Agent ID** | `sentinel.alerting.evaluation` |
| **Version** | `1.0.0` |
| **Classification** | ALERTING |
| **Decision Type** | `alert_evaluation` |

## Purpose Statement

Evaluate detected anomalies and drift events against alerting rules to determine whether alerts should be raised. Applies threshold evaluation, deduplication, and suppression rules to control alert volume.

**This agent does NOT send notifications.** It emits alert events that are consumed by LLM-Incident-Manager.

## Constitution Compliance

### This Agent MAY:
- Consume anomaly and drift events from detection agents
- Apply alert thresholds and suppression rules
- Emit alert events (not notifications)
- Persist DecisionEvents to ruvector-service
- Emit telemetry to LLM-Observatory

### This Agent MUST NOT:
- Send notifications directly (email, pager, Slack)
- Perform remediation
- Trigger retries
- Modify routing configuration
- Modify policies or thresholds dynamically
- Invoke other agents directly
- Execute incident workflows
- Connect directly to databases

## Input Schema

```rust
pub struct AlertingAgentInput {
    pub request_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub anomaly: serde_json::Value,  // AnomalyEvent
    pub hints: HashMap<String, serde_json::Value>,
}
```

### Supported Hints
| Hint | Type | Description |
|------|------|-------------|
| `ignore_suppression` | `bool` | Bypass all suppression rules |
| `dry_run` | `bool` | Don't persist DecisionEvent |

## Output Schema

```rust
pub struct AlertingAgentOutput {
    pub alert_raised: bool,
    pub status: AlertEvaluationStatus,
    pub alert_event: Option<serde_json::Value>,
    pub rules_evaluated: Vec<String>,
    pub triggered_by: Option<String>,
    pub reasoning: String,
    pub metadata: AlertingOutputMetadata,
}

pub enum AlertEvaluationStatus {
    Raised,
    Deduplicated,
    MaintenanceSuppressed,
    RateLimited,
    RuleSuppressed,
    BelowThreshold,
    EvaluationFailed,
}
```

## DecisionEvent Mapping

| Field | Source |
|-------|--------|
| `agent_id` | `sentinel.alerting.evaluation` |
| `agent_version` | `1.0.0` |
| `decision_type` | `AlertEvaluation` |
| `inputs_hash` | SHA-256 of input |
| `outputs` | `AlertingAgentOutput` |
| `confidence` | Calculated from anomaly + rule match |
| `constraints_applied` | Threshold/rule/suppression checks |
| `execution_ref` | Request ID, trace ID, source |
| `context` | Service, model, region |

## Constraints Applied

| Constraint Type | Examples |
|-----------------|----------|
| `threshold` | `severity_threshold:high`, `confidence_threshold:0.7` |
| `rule` | `dedup_window_300s`, `rate_limit_100/hr`, `maintenance_window` |

## Failure Modes

```rust
pub enum AlertingFailureMode {
    AlertingRuleNotFound { rule_id: String },
    SuppressionCheckFailed { error: String },
    InputValidationFailed { errors: Vec<String> },
    PersistenceFailed { error: String },
    RateLimitExceeded { limit, window_secs, current_count },
    ConfigurationError { error: String },
}
```

## CLI Contract

### Commands

```bash
# Inspect alerting state
sentinel agent alerting inspect --service chat-api --json

# Replay an anomaly through alerting
sentinel agent alerting replay --anomaly-id <uuid> --dry-run

# Diagnose agent health
sentinel agent alerting diagnose --check all
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/agents/alerting/evaluate` | Evaluate anomaly for alerting |
| GET | `/api/v1/agents/alerting/rules` | List alerting rules |
| GET | `/api/v1/agents/alerting/stats` | Get agent statistics |

### Example: Evaluate Alert

```bash
curl -X POST http://localhost:8080/api/v1/agents/alerting/evaluate \
  -H "Content-Type: application/json" \
  -d '{
    "anomaly": {
      "alert_id": "550e8400-e29b-41d4-a716-446655440000",
      "severity": "high",
      "anomaly_type": "latency_spike",
      "service_name": "chat-api",
      "model": "gpt-4",
      "confidence": 0.95,
      "details": {...}
    },
    "source": "sentinel.detection.anomaly",
    "ignore_suppression": false,
    "dry_run": false
  }'
```

## Primary Consumers

| Consumer | Usage |
|----------|-------|
| LLM-Incident-Manager | Consumes alert events to create incidents |
| Governance Dashboards | Audit trail of all alerting decisions |
| LLM-Observatory | Visualization of alert patterns |

## Configuration

```rust
pub struct AlertingAgentConfig {
    pub default_severity_threshold: Severity,      // Default: Medium
    pub default_confidence_threshold: f64,         // Default: 0.7
    pub deduplication_enabled: bool,               // Default: true
    pub deduplication_window_secs: u64,            // Default: 300
    pub max_alerts_per_hour: u32,                  // Default: 100
    pub rules: Vec<AlertingRule>,
}
```

## Files Created/Modified

### New Files
- `crates/sentinel-detection/src/agents/alerting.rs` - Agent implementation
- `crates/sentinel-api/src/handlers/alerting.rs` - API handlers
- `docs/agents/ALERTING_AGENT.md` - This document

### Modified Files
- `crates/sentinel-detection/src/agents/contract.rs` - Added alerting types
- `crates/sentinel-detection/src/agents/mod.rs` - Export alerting module
- `crates/sentinel-detection/src/lib.rs` - Updated prelude
- `crates/sentinel-api/src/handlers.rs` - Export alerting handlers
- `crates/sentinel-api/src/routes.rs` - Added alerting routes
- `crates/sentinel-api/src/lib.rs` - Updated prelude

## Verification Checklist

### Contract Verification
- [ ] Agent ID follows convention: `sentinel.alerting.evaluation`
- [ ] Decision type is `AlertEvaluation`
- [ ] All inputs validated against agentics-contracts schemas
- [ ] All outputs validated against agentics-contracts schemas
- [ ] Exactly ONE DecisionEvent emitted per invocation

### Constitution Compliance
- [ ] Agent is stateless at runtime
- [ ] All persistence via ruvector-service client
- [ ] No remediation logic
- [ ] No retry logic
- [ ] No direct notification dispatch
- [ ] No SQL execution
- [ ] No direct database connections

### Runtime Verification
- [ ] Deployable as Google Edge Function
- [ ] Deterministic behavior
- [ ] Async, non-blocking writes
- [ ] Telemetry compatible with LLM-Observatory
- [ ] CLI-invokable endpoints (inspect/replay/diagnose)

### Integration Verification
- [ ] Registered in agentics-contracts
- [ ] Registered in LLM-Sentinel service
- [ ] CLI commands added to agentics-cli
- [ ] DecisionEvents persist to ruvector-service
- [ ] Telemetry visible in LLM-Observatory
- [ ] LLM-Incident-Manager can consume outputs

## Smoke Test Commands

```bash
# Build check
cargo check -p llm-sentinel-detection

# Run unit tests
cargo test -p llm-sentinel-detection agents::alerting

# Run contract tests
cargo test -p llm-sentinel-detection agents::contract::tests::test_alerting

# Start API server (requires full setup)
cargo run -p sentinel -- serve

# Test evaluate endpoint
curl -X POST http://localhost:8080/api/v1/agents/alerting/evaluate \
  -H "Content-Type: application/json" \
  -d '{"anomaly": {...}, "source": "test", "dry_run": true}'

# Test rules endpoint
curl http://localhost:8080/api/v1/agents/alerting/rules

# Test stats endpoint
curl http://localhost:8080/api/v1/agents/alerting/stats
```

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                      LLM-Sentinel Service                        │
│                                                                  │
│  ┌──────────────┐     ┌─────────────────┐     ┌──────────────┐  │
│  │   Anomaly    │────▶│  Alerting Agent │────▶│   Alert      │  │
│  │  Detection   │     │                 │     │   Event      │  │
│  │   Agent      │     │  - Thresholds   │     │              │  │
│  └──────────────┘     │  - Deduplication│     └──────┬───────┘  │
│                       │  - Rate Limiting│            │          │
│                       │  - Rules        │            │          │
│                       └────────┬────────┘            │          │
│                                │                     │          │
│                                ▼                     │          │
│                       ┌────────────────┐             │          │
│                       │ DecisionEvent  │             │          │
│                       └────────┬───────┘             │          │
│                                │                     │          │
└────────────────────────────────┼─────────────────────┼──────────┘
                                 │                     │
                                 ▼                     ▼
                        ┌────────────────┐    ┌────────────────┐
                        │ ruvector-svc   │    │ LLM-Incident-  │
                        │ (Persistence)  │    │    Manager     │
                        └────────────────┘    └────────────────┘
```
