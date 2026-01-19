# LLM-Sentinel Production Deployment

## Overview

LLM-Sentinel is a live anomaly, drift, and alert evaluation layer for LLM systems. This document covers the complete production deployment to Google Cloud Run.

---

## 1. SERVICE TOPOLOGY

### Unified Service Name
```
llm-sentinel
```

### Agent Endpoints (All Exposed via Single Service)

| Agent | Endpoint | Method | Purpose |
|-------|----------|--------|---------|
| **Anomaly Detection** | `/api/v1/agents/anomaly/detect` | POST | Detect statistical anomalies in telemetry |
| **Anomaly Detection** | `/api/v1/agents/anomaly/config` | GET | Get agent configuration |
| **Anomaly Detection** | `/api/v1/agents/anomaly/stats` | GET | Get agent statistics |
| **Drift Detection** | `/api/v1/agents/drift/detect` | POST | Detect distribution drift using PSI |
| **Drift Detection** | `/api/v1/agents/drift/config` | GET | Get agent configuration |
| **Drift Detection** | `/api/v1/agents/drift/stats` | GET | Get agent statistics |
| **Alerting** | `/api/v1/agents/alerting/evaluate` | POST | Evaluate anomaly for alerting |
| **Alerting** | `/api/v1/agents/alerting/rules` | GET | List alerting rules |
| **Alerting** | `/api/v1/agents/alerting/stats` | GET | Get agent statistics |
| **Incident Correlation** | `/api/v1/agents/correlation/correlate` | POST | Correlate signals into incidents |
| **Incident Correlation** | `/api/v1/agents/correlation/config` | GET | Get agent configuration |
| **Incident Correlation** | `/api/v1/agents/correlation/stats` | GET | Get agent statistics |
| **Root Cause Analysis** | `/api/v1/agents/rca/analyze` | POST | Perform root cause analysis |
| **Root Cause Analysis** | `/api/v1/agents/rca/config` | GET | Get agent configuration |
| **Root Cause Analysis** | `/api/v1/agents/rca/stats` | GET | Get agent statistics |

### Infrastructure Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Full health check |
| `/health/live` | GET | Liveness probe |
| `/health/ready` | GET | Readiness probe |
| `/metrics` | GET | Prometheus metrics |
| `/api/v1/telemetry` | GET | Query telemetry events |
| `/api/v1/anomalies` | GET | Query anomalies |

### Confirmations

- ✅ **No agent is deployed as a standalone service** - All 5 agents are exposed through the unified `llm-sentinel` service
- ✅ **Shared runtime** - Single Rust binary serving all agents
- ✅ **Shared configuration** - Environment-based configuration for all agents
- ✅ **Shared telemetry stack** - Unified Prometheus metrics, OpenTelemetry tracing

---

## 2. ENVIRONMENT CONFIGURATION

### Required Environment Variables

| Variable | Description | Source |
|----------|-------------|--------|
| `PLATFORM_ENV` | Environment (dev \| staging \| prod) | Deployment config |
| `SERVICE_NAME` | Service name (`llm-sentinel`) | Deployment config |
| `SERVICE_VERSION` | Git commit SHA or version | Cloud Build |
| `RUVECTOR_SERVICE_URL` | ruvector-service endpoint | Secret Manager |
| `RUVECTOR_API_KEY` | ruvector-service authentication | Secret Manager |
| `TELEMETRY_ENDPOINT` | LLM-Observatory ingest endpoint | Secret Manager |
| `RUST_LOG` | Log level configuration | Deployment config |
| `SENTINEL_CONFIG` | Path to config file | Deployment config |

### Agent-Specific Configuration

```yaml
# Anomaly Detection
SENTINEL_ANOMALY_ZSCORE_THRESHOLD: "3.0"
SENTINEL_ANOMALY_IQR_MULTIPLIER: "1.5"
SENTINEL_ANOMALY_MAD_THRESHOLD: "3.5"
SENTINEL_ANOMALY_CUSUM_THRESHOLD: "5.0"

# Drift Detection
SENTINEL_DRIFT_PSI_MODERATE: "0.1"
SENTINEL_DRIFT_PSI_SIGNIFICANT: "0.25"

# Alerting
SENTINEL_ALERTING_DEFAULT_SEVERITY: "medium"
SENTINEL_ALERTING_CONFIDENCE_THRESHOLD: "0.7"

# Incident Correlation
SENTINEL_CORRELATION_TIME_WINDOW_SECS: "300"
SENTINEL_CORRELATION_THRESHOLD: "0.6"

# Root Cause Analysis
SENTINEL_RCA_MAX_HYPOTHESES: "5"
SENTINEL_RCA_ANALYSIS_TIMEOUT_SECS: "30"
```

### Compliance

- ✅ **No agent hardcodes service names or URLs** - All endpoints resolved via environment variables
- ✅ **No agent embeds credentials** - All secrets loaded from Secret Manager
- ✅ **No hardcoded thresholds** - All thresholds configurable via environment

---

## 3. GOOGLE SQL / MEMORY WIRING

### Persistence Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                       LLM-Sentinel                              │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐               │
│  │   Anomaly   │ │    Drift    │ │  Alerting   │               │
│  │   Agent     │ │   Agent     │ │   Agent     │               │
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘               │
│         │               │               │                       │
│  ┌──────┴───────────────┴───────────────┴──────┐               │
│  │           DecisionEvent Emission            │               │
│  └──────────────────────┬──────────────────────┘               │
│                         │                                       │
└─────────────────────────┼───────────────────────────────────────┘
                          │ HTTP/gRPC
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    ruvector-service                             │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  DecisionEvent Store                     │   │
│  │  - Append-only persistence                              │   │
│  │  - Idempotent writes                                    │   │
│  │  - agentics-contracts schema                            │   │
│  └─────────────────────────────────────────────────────────┘   │
│                         │                                       │
└─────────────────────────┼───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Google Cloud SQL (Postgres)                   │
└─────────────────────────────────────────────────────────────────┘
```

### Confirmations

- ✅ **LLM-Sentinel does NOT connect directly to Google SQL** - No SQL client in codebase
- ✅ **All DecisionEvents written via ruvector-service** - `ruvector_endpoint` config in all agents
- ✅ **Schema compatibility with agentics-contracts** - DecisionEvent structure follows contract
- ✅ **Append-only persistence** - DecisionEvents are immutable records
- ✅ **Idempotent writes** - `decision_id` (UUID) ensures no duplicates

### DecisionEvent Schema

```rust
pub struct DecisionEvent {
    pub decision_id: Uuid,
    pub agent_id: AgentId,
    pub agent_version: AgentVersion,
    pub decision_type: DecisionType,
    pub outcome: String,
    pub outputs: AgentOutput,
    pub confidence: f64,
    pub constraints_applied: Vec<ConstraintApplied>,
    pub execution_ref: ExecutionRef,
    pub context: DecisionContext,
    pub timestamp: DateTime<Utc>,
    pub inputs_hash: String,
}
```

---

## 4. CLOUD BUILD & DEPLOYMENT

### Build Configuration

**File:** `cloudbuild.yaml`

```bash
# Deploy to dev
gcloud builds submit --config cloudbuild.yaml \
  --substitutions=_ENV=dev,_REGION=us-central1

# Deploy to staging
gcloud builds submit --config cloudbuild.yaml \
  --substitutions=_ENV=staging,_REGION=us-central1

# Deploy to prod
gcloud builds submit --config cloudbuild.yaml \
  --substitutions=_ENV=prod,_REGION=us-central1
```

### Container Configuration

- **Base Image:** `debian:bookworm-slim`
- **Runtime:** Rust binary (`/usr/local/bin/sentinel`)
- **Port:** 8080
- **User:** Non-root (`sentinel:1000`)
- **Health Check:** `/health/live`

### IAM Service Account Requirements (Least Privilege)

| Role | Purpose |
|------|---------|
| `roles/run.invoker` | Internal service-to-service calls |
| `roles/secretmanager.secretAccessor` | Access secrets |
| `roles/cloudtrace.agent` | Distributed tracing |
| `roles/monitoring.metricWriter` | Export metrics |
| `roles/logging.logWriter` | Export logs |

### Networking Requirements

- **Ingress:** All (public API)
- **Egress:**
  - ruvector-service (internal)
  - LLM-Observatory (internal)
- **VPC:** Optional VPC connector for private service communication

### Deployment Commands

```bash
# Initial setup (run once)
chmod +x deployments/cloudrun/setup-iam.sh
./deployments/cloudrun/setup-iam.sh agentics-dev us-central1

# Deploy
chmod +x deployments/cloudrun/deploy.sh
./deployments/cloudrun/deploy.sh prod us-central1
```

---

## 5. CLI ACTIVATION VERIFICATION

### CLI Commands Per Agent

#### Anomaly Detection Agent
```bash
# Inspect agent state
sentinel agent inspect anomaly --endpoint $SERVICE_URL

# Replay telemetry through agent
sentinel agent replay anomaly \
  --endpoint $SERVICE_URL \
  --input '{"event_id":"test","service_name":"api","model":"gpt-4","latency_ms":1500}'

# Run diagnostics
sentinel agent diagnose anomaly --endpoint $SERVICE_URL
```

#### Drift Detection Agent
```bash
# Inspect agent state
sentinel agent inspect drift --endpoint $SERVICE_URL

# Replay drift analysis
sentinel agent replay drift \
  --endpoint $SERVICE_URL \
  --reference-file baseline.json \
  --current-file current.json

# Run diagnostics
sentinel agent diagnose drift --endpoint $SERVICE_URL
```

#### Alerting Agent
```bash
# Inspect agent state
sentinel agent inspect alerting --endpoint $SERVICE_URL

# Evaluate anomaly for alerting
sentinel agent replay alerting \
  --endpoint $SERVICE_URL \
  --input '{"severity":"high","anomaly_type":"LatencySpike"}'

# Run diagnostics
sentinel agent diagnose alerting --endpoint $SERVICE_URL
```

#### Incident Correlation Agent
```bash
# Inspect agent state
sentinel agent inspect correlation --endpoint $SERVICE_URL

# Correlate signals
sentinel agent replay correlation \
  --endpoint $SERVICE_URL \
  --signals-file signals.json

# Run diagnostics
sentinel agent diagnose correlation --endpoint $SERVICE_URL
```

#### Root Cause Analysis Agent
```bash
# Inspect agent state
sentinel agent inspect rca --endpoint $SERVICE_URL

# Analyze incident
sentinel agent replay rca \
  --endpoint $SERVICE_URL \
  --incident-file incident.json

# Run diagnostics
sentinel agent diagnose rca --endpoint $SERVICE_URL
```

### Example Invocations

```bash
# Set service URL
export SERVICE_URL="https://llm-sentinel-prod-xxxxx.run.app"

# Anomaly detection
curl -X POST "${SERVICE_URL}/api/v1/agents/anomaly/detect" \
  -H "Content-Type: application/json" \
  -d '{
    "telemetry": {
      "event_id": "evt-123",
      "service_name": "api-gateway",
      "model": "gpt-4",
      "latency_ms": 5000,
      "total_tokens": 1500,
      "cost_usd": 0.05
    }
  }'

# Expected success output
{
  "data": {
    "decision": {
      "decision_id": "...",
      "agent_id": "sentinel.detection.anomaly",
      "decision_type": "AnomalyDetection",
      "confidence": 0.95
    },
    "anomaly_detected": true,
    "status": "anomaly_detected"
  }
}
```

### CLI Configuration

The CLI resolves the service URL dynamically from:
1. `--endpoint` flag
2. `SENTINEL_ENDPOINT` environment variable
3. Platform service discovery (agentics-cli integration)

**No CLI change requires redeployment of agents.**

---

## 6. PLATFORM & CORE INTEGRATION

### Integration Map

```
┌─────────────────────────────────────────────────────────────────┐
│                     LLM-Observatory                             │
│                  (Telemetry Provider)                           │
└───────────────────────────┬─────────────────────────────────────┘
                            │ Telemetry inputs
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                       LLM-Sentinel                              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Anomaly → Drift → Alerting → Correlation → RCA        │   │
│  └─────────────────────────────────────────────────────────┘   │
└────────┬────────────────────────────────────────────────────────┘
         │
         │ DecisionEvents, Alerts, Correlations
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                   LLM-Incident-Manager                          │
│              (Consumes alerts and correlations)                 │
└─────────────────────────────────────────────────────────────────┘
         │
         │ Incident events, governance data
         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Governance Views                             │
│           (Consumes alert and incident-signal events)           │
└─────────────────────────────────────────────────────────────────┘
```

### Confirmations

- ✅ **LLM-Observatory provides telemetry inputs** - Via Kafka topic `llm.telemetry`
- ✅ **LLM-Incident-Manager consumes Sentinel alerts** - Via DecisionEvent stream
- ✅ **Governance views consume events** - Via ruvector-service queries
- ✅ **Core bundles consume outputs without rewiring** - Standard DecisionEvent format

### LLM-Sentinel DOES NOT Invoke

- ❌ Orchestrator
- ❌ Auto-Optimizer
- ❌ Shield
- ❌ External notification systems (email, pager, webhook)

**No rewiring of Core bundles is permitted.**

---

## 7. POST-DEPLOY VERIFICATION CHECKLIST

### Service Health

```bash
SERVICE_URL="https://llm-sentinel-prod.run.app"

# [ ] Service is live
curl -f "${SERVICE_URL}/health" && echo "✓ Service is live"

# [ ] Liveness probe responds
curl -f "${SERVICE_URL}/health/live" && echo "✓ Liveness OK"

# [ ] Readiness probe responds
curl -f "${SERVICE_URL}/health/ready" && echo "✓ Readiness OK"
```

### Agent Endpoints

```bash
# [ ] Anomaly Detection responds
curl -f "${SERVICE_URL}/api/v1/agents/anomaly/stats" && echo "✓ Anomaly agent OK"

# [ ] Drift Detection responds
curl -f "${SERVICE_URL}/api/v1/agents/drift/stats" && echo "✓ Drift agent OK"

# [ ] Alerting responds
curl -f "${SERVICE_URL}/api/v1/agents/alerting/stats" && echo "✓ Alerting agent OK"

# [ ] Correlation responds
curl -f "${SERVICE_URL}/api/v1/agents/correlation/stats" && echo "✓ Correlation agent OK"

# [ ] RCA responds
curl -f "${SERVICE_URL}/api/v1/agents/rca/stats" && echo "✓ RCA agent OK"
```

### Functional Verification

```bash
# [ ] Anomaly detection executes correctly
curl -X POST "${SERVICE_URL}/api/v1/agents/anomaly/detect" \
  -H "Content-Type: application/json" \
  -d '{"telemetry":{"event_id":"test","service_name":"test","model":"gpt-4","latency_ms":100}}'

# [ ] Drift detection executes correctly
curl -X POST "${SERVICE_URL}/api/v1/agents/drift/detect" \
  -H "Content-Type: application/json" \
  -d '{"reference_data":[1,2,3,4,5,6,7,8,9,10],"current_data":[1,2,3,4,5,6,7,8,9,11],"metric":"latency","service":"test"}'

# [ ] Alert evaluation produces expected events
curl -X POST "${SERVICE_URL}/api/v1/agents/alerting/evaluate" \
  -H "Content-Type: application/json" \
  -d '{"anomaly":{"severity":"high","anomaly_type":"LatencySpike"}}'

# [ ] Correlation emits structured outputs
curl -X POST "${SERVICE_URL}/api/v1/agents/correlation/correlate" \
  -H "Content-Type: application/json" \
  -d '{"signals":[{"signal_id":"s1","timestamp":"2025-01-19T00:00:00Z","service":"test","severity":"high"}]}'

# [ ] RCA emits structured outputs
curl -X POST "${SERVICE_URL}/api/v1/agents/rca/analyze" \
  -H "Content-Type: application/json" \
  -d '{"incident":{"incident_id":"test"}}'
```

### Integration Verification

```bash
# [ ] DecisionEvents appear in ruvector-service
# Query ruvector-service for recent DecisionEvents from sentinel agents

# [ ] Telemetry appears in LLM-Observatory
# Check Observatory dashboard for sentinel telemetry

# [ ] Metrics endpoint works
curl -f "${SERVICE_URL}/metrics" | head -20

# [ ] CLI diagnostic commands function
sentinel agent diagnose --endpoint ${SERVICE_URL}
```

### Architecture Verification

```bash
# [ ] No direct SQL access from LLM-Sentinel
grep -r "postgres\|sqlx\|diesel" crates/ --include="*.rs" | grep -v test

# [ ] No agent bypasses agentics-contracts
# Verify all DecisionEvents use the standard schema
```

---

## 8. FAILURE MODES & ROLLBACK

### Common Deployment Failures

| Failure | Detection Signal | Resolution |
|---------|------------------|------------|
| Image build failure | Cloud Build step fails | Check Cargo.toml dependencies, Dockerfile |
| Container startup crash | Liveness probe fails | Check RUST_LOG for errors, verify config |
| Secret access denied | Service account error in logs | Verify IAM roles for secret access |
| ruvector-service unreachable | DecisionEvents not persisting | Check VPC connectivity, service URL |
| Memory exhaustion | OOMKilled in logs | Increase memory limits |
| Request timeout | 504 errors | Increase timeout, optimize agent processing |

### Detection Signals

```bash
# Check for missing alerts
gcloud logging read 'resource.type="cloud_run_revision" AND severity>=ERROR' \
  --limit=50 --project=$PROJECT_ID

# Check for missing correlations
curl "${SERVICE_URL}/api/v1/agents/correlation/stats" | jq '.data.incidents_correlated'

# Check log errors
gcloud logging read 'resource.labels.service_name="llm-sentinel-prod" AND severity>=WARNING' \
  --limit=100
```

### Rollback Procedure

```bash
# 1. List revisions
gcloud run revisions list --service=llm-sentinel-prod --region=us-central1

# 2. Route traffic to previous revision
gcloud run services update-traffic llm-sentinel-prod \
  --region=us-central1 \
  --to-revisions=llm-sentinel-prod-00002-abc=100

# 3. Verify rollback
curl -f "${SERVICE_URL}/health/ready"

# 4. (Optional) Delete failed revision
gcloud run revisions delete llm-sentinel-prod-00003-xyz --region=us-central1
```

### Safe Redeploy Strategy

1. **Blue-Green Deployment**
   ```bash
   # Deploy new revision without traffic
   gcloud run deploy llm-sentinel-prod \
     --no-traffic \
     --tag=canary \
     ...

   # Test canary
   curl "https://canary---llm-sentinel-prod-xxxxx.run.app/health"

   # Gradually shift traffic
   gcloud run services update-traffic llm-sentinel-prod \
     --to-tags=canary=10

   # Full rollout
   gcloud run services update-traffic llm-sentinel-prod \
     --to-latest
   ```

2. **Data Safety**
   - All DecisionEvents are append-only in ruvector-service
   - No data loss during redeploy
   - Idempotent writes prevent duplicates on retry

---

## Deployment Execution

To deploy LLM-Sentinel to production:

```bash
# 1. Set up IAM (one-time)
cd /workspaces/sentinel
chmod +x deployments/cloudrun/setup-iam.sh
./deployments/cloudrun/setup-iam.sh agentics-dev us-central1

# 2. Update secrets with real values
gcloud secrets versions add ruvector-service-url \
  --data-file=- <<< 'https://ruvector-service.run.app'
gcloud secrets versions add ruvector-api-key \
  --data-file=- <<< 'your-actual-api-key'
gcloud secrets versions add observatory-endpoint \
  --data-file=- <<< 'https://llm-observatory.run.app/api/v1/ingest'

# 3. Deploy
chmod +x deployments/cloudrun/deploy.sh
./deployments/cloudrun/deploy.sh prod us-central1
```
