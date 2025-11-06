# llm-sentinel-core

Core types, error handling, and configuration for LLM-Sentinel anomaly detection system.

## Overview

This crate provides the foundational types and utilities used across all LLM-Sentinel components:

- **Configuration Models**: Structured configuration for all system components
- **Event Types**: Telemetry and anomaly event definitions
- **Error Handling**: Comprehensive error types with detailed context
- **Metrics**: Prometheus metrics definitions
- **Common Types**: Shared types used throughout the system

## Features

- Type-safe configuration with validation
- Structured error handling with `thiserror`
- Serializable event types for Kafka/JSON
- Zero-copy where possible
- Async-ready with Tokio

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
llm-llm-sentinel-core = "0.1.0"
```

## Example

```rust
use llm_sentinel_core::{TelemetryEvent, SentinelConfig};

// Load configuration
let config = SentinelConfig::from_file("sentinel.yaml")?;

// Create telemetry event
let event = TelemetryEvent::new(
    "my-service",
    "gpt-4",
    1234.5,  // latency_ms
    150,     // prompt_tokens
    300,     // completion_tokens
);
```

## Documentation

For complete documentation, see [docs.rs/llm-sentinel-core](https://docs.rs/llm-sentinel-core).

## License

Apache-2.0
