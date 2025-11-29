# Quick Start Guide

Get started with the LLM Sentinel Python SDK in 5 minutes.

## Installation

```bash
pip install llm-sentinel-sdk
```

For async support:
```bash
pip install llm-sentinel-sdk[async]
```

## Basic Usage

### 1. Import the SDK

```python
from sentinel_sdk import SentinelClient
```

### 2. Create a Client

```python
# Connect to local Kafka
client = SentinelClient(brokers=["localhost:9092"])

# Or use environment variables
# export SENTINEL_BROKERS="localhost:9092"
client = SentinelClient.from_env()
```

### 3. Track LLM Requests

```python
# Simple tracking
client.track(
    service="my-service",
    model="gpt-4",
    prompt="What is the capital of France?",
    response="The capital of France is Paris.",
    latency_ms=150.5,
    prompt_tokens=8,
    completion_tokens=7,
    cost_usd=0.001
)
```

### 4. Close the Client

```python
client.close()
```

## Recommended: Use Context Manager

```python
from sentinel_sdk import SentinelClient

with SentinelClient(brokers=["localhost:9092"]) as client:
    client.track(
        service="my-service",
        model="gpt-4",
        prompt="Hello!",
        response="Hi there!",
        latency_ms=150.5,
        prompt_tokens=5,
        completion_tokens=10,
        cost_usd=0.001
    )
# Client automatically closed
```

## Async Usage

```python
from sentinel_sdk import AsyncSentinelClient
import asyncio

async def main():
    async with AsyncSentinelClient(brokers=["localhost:9092"]) as client:
        await client.track(
            service="my-service",
            model="gpt-4",
            prompt="Hello!",
            response="Hi!",
            latency_ms=150.0,
            prompt_tokens=5,
            completion_tokens=10,
            cost_usd=0.001
        )

asyncio.run(main())
```

## Advanced: Builder Pattern

```python
from sentinel_sdk import SentinelClient, TelemetryEventBuilder

client = SentinelClient(brokers=["localhost:9092"])

event = (TelemetryEventBuilder()
    .service("my-service")
    .model("gpt-4")
    .prompt(text="Hello", tokens=5)
    .response(text="Hi!", tokens=10, finish_reason="stop")
    .latency(150.5)
    .cost(0.001)
    .metadata({"region": "us-east-1"})
    .build())

client.send(event)
client.close()
```

## Configuration

### Environment Variables

```bash
export SENTINEL_BROKERS="localhost:9092"
export SENTINEL_TOPIC="llm-telemetry"
export SENTINEL_CLIENT_ID="my-service"
```

### Programmatic Configuration

```python
from sentinel_sdk import SentinelConfig, SentinelClient

config = SentinelConfig(
    brokers=["localhost:9092"],
    topic="llm-telemetry",
    compression_type="gzip",
    acks="all"
)

client = SentinelClient(config=config)
```

## Error Handling

```python
from sentinel_sdk import SentinelError

try:
    client.track(...)
except SentinelError as e:
    print(f"Error: {e}")
```

## Next Steps

- Read the full [README.md](README.md) for detailed documentation
- Check out [examples/](examples/) for integration examples
- See the [main Sentinel documentation](../../README.md) for system setup

## Common Issues

### Connection Refused

If you see "Connection refused" errors:
1. Ensure Kafka is running: `docker-compose up -d`
2. Check broker addresses are correct
3. Verify network connectivity

### Import Errors

If you see import errors:
1. Ensure the SDK is installed: `pip install llm-sentinel-sdk`
2. For async support: `pip install llm-sentinel-sdk[async]`

## Getting Help

- GitHub Issues: https://github.com/yourusername/sentinel/issues
- Documentation: https://github.com/yourusername/sentinel
