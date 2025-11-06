# llm-sentinel-storage

InfluxDB time-series storage and multi-layer caching for LLM-Sentinel.

## Overview

High-performance storage layer with multiple caching tiers:

- **InfluxDB v3**: Time-series storage for telemetry and anomalies
- **Moka Cache**: In-memory cache (10,000 entry capacity)
- **Redis**: Distributed cache for multi-instance deployments
- **Query API**: Historical data retrieval with time-range queries

## Features

- 8,000+ writes/second (batched)
- Dual-layer caching (memory + distributed)
- Automatic TTL management
- Batch writes for efficiency
- Query API for analytics
- Configurable retention policies

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
llm-llm-sentinel-storage = "0.1.0"
```

## Example

```rust
use llm_sentinel_storage::{StorageManager, StorageConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = StorageConfig {
        influxdb_url: "http://localhost:8086".to_string(),
        influxdb_org: "sentinel".to_string(),
        influxdb_token: std::env::var("INFLUXDB_TOKEN")?,
        telemetry_bucket: "telemetry".to_string(),
        ..Default::default()
    };

    let manager = StorageManager::new(config).await?;

    // Store telemetry
    manager.store_telemetry(&event).await?;

    // Query historical data
    let results = manager.query_telemetry(
        "service_name = 'chat-api'",
        chrono::Duration::hours(24)
    ).await?;

    Ok(())
}
```

## Caching Strategy

1. **L1 Cache (Moka)**: Hot data in memory, sub-microsecond access
2. **L2 Cache (Redis)**: Distributed cache for multi-instance setups
3. **L3 Storage (InfluxDB)**: Persistent time-series storage

## License

Apache-2.0
