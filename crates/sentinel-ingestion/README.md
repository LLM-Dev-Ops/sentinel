# llm-sentinel-ingestion

Kafka ingestion, OTLP parsing, and telemetry validation for LLM-Sentinel.

## Overview

This crate handles the ingestion pipeline for LLM telemetry data:

- **Kafka Consumer**: High-throughput Kafka consumer with group management
- **OTLP Parsing**: OpenTelemetry Protocol (OTLP) and JSON parsing
- **Validation**: Schema validation and PII detection
- **Pipeline Processing**: Async streaming pipeline for telemetry events

## Features

- 10,000+ events/second throughput
- Automatic offset management
- Configurable batch processing
- PII sanitization
- Schema validation with detailed error messages
- Backpressure handling

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
llm-llm-sentinel-ingestion = "0.1.0"
```

## Example

```rust
use llm_sentinel_ingestion::{KafkaConsumer, IngestionConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = IngestionConfig {
        kafka_brokers: vec!["localhost:9092".to_string()],
        topic: "llm.telemetry".to_string(),
        group_id: "sentinel-consumer".to_string(),
        ..Default::default()
    };

    let consumer = KafkaConsumer::new(config).await?;
    consumer.start().await?;

    Ok(())
}
```

## License

Apache-2.0
