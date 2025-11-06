# llm-sentinel-api

REST API server with health checks, metrics, and query endpoints for LLM-Sentinel.

## Overview

Production-ready REST API built with Axum:

- **Health Endpoints**: Liveness and readiness probes
- **Metrics**: Prometheus metrics exporter
- **Query API**: Historical telemetry and anomaly queries
- **CORS**: Configurable cross-origin support

## Features

- Sub-10ms request latency
- Prometheus metrics integration
- Health check endpoints for Kubernetes
- Query API for historical data
- Request validation and error handling
- Rate limiting support

## Usage

```toml
[dependencies]
llm-llm-sentinel-api = "0.1.0"
```

## Example

```rust
use llm_sentinel_api::{ApiServer, ApiConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ApiConfig {
        bind_addr: "0.0.0.0:8080".parse()?,
        enable_cors: true,
        ..Default::default()
    };

    let server = ApiServer::new(config);
    server.start().await?;

    Ok(())
}
```

## Endpoints

- `GET /health/live` - Liveness probe
- `GET /health/ready` - Readiness probe
- `GET /metrics` - Prometheus metrics
- `GET /api/v1/telemetry` - Query telemetry
- `GET /api/v1/anomalies` - Query anomalies

## License

Apache-2.0
