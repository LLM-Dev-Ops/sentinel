# llm-sentinel-alerting

RabbitMQ and webhook alerting with deduplication for LLM-Sentinel.

## Overview

Flexible alerting system with multiple delivery channels:

- **RabbitMQ**: Topic-based routing with severity levels
- **Webhooks**: HTTP POST with HMAC-SHA256 signatures
- **Deduplication**: 5-minute window to prevent alert storms
- **Retry Logic**: Exponential backoff for reliable delivery

## Features

- Multiple alert channels (RabbitMQ, webhooks)
- Automatic deduplication
- HMAC signature verification
- Persistent message delivery
- Configurable retry policies
- Topic-based routing by severity

## Usage

```toml
[dependencies]
llm-llm-sentinel-alerting = "0.1.0"
```

## Example

```rust
use llm_sentinel_alerting::{AlertingManager, AlertingConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AlertingConfig {
        rabbitmq_url: "amqp://localhost:5672".to_string(),
        exchange: "sentinel.alerts".to_string(),
        ..Default::default()
    };

    let manager = AlertingManager::new(config).await?;
    manager.send_alert(&anomaly).await?;

    Ok(())
}
```

## License

Apache-2.0
