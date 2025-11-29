# LLM Sentinel Python SDK

Python SDK for [LLM Sentinel](https://github.com/yourusername/sentinel) - Send LLM telemetry events to Sentinel for real-time anomaly detection and monitoring.

## Features

- **Simple API**: Easy-to-use client for tracking LLM requests
- **Sync & Async**: Support for both synchronous and asynchronous operations
- **Builder Pattern**: Fluent builder for constructing complex events
- **Type Safety**: Full type hints and validation
- **Configuration**: Environment variable support and sensible defaults
- **Kafka Integration**: Built on kafka-python and aiokafka
- **Production Ready**: Retry logic, connection pooling, and graceful shutdown

## Installation

### Basic Installation

```bash
pip install llm-sentinel-sdk
```

### With Async Support

```bash
pip install llm-sentinel-sdk[async]
```

### Development Installation

```bash
pip install llm-sentinel-sdk[dev]
```

## Quick Start

### Synchronous Usage

```python
from sentinel_sdk import SentinelClient

# Create a client
client = SentinelClient(brokers=["localhost:9092"])

# Track an LLM request
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

# Close the client when done
client.close()
```

### Context Manager (Recommended)

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
# Client is automatically closed
```

### Asynchronous Usage

```python
from sentinel_sdk import AsyncSentinelClient

async def main():
    async with AsyncSentinelClient(brokers=["localhost:9092"]) as client:
        await client.track(
            service="my-service",
            model="claude-3-opus",
            prompt="Explain quantum computing",
            response="Quantum computing is...",
            latency_ms=250.0,
            prompt_tokens=15,
            completion_tokens=100,
            cost_usd=0.015
        )

import asyncio
asyncio.run(main())
```

## Advanced Usage

### Builder Pattern

For more control over event construction:

```python
from sentinel_sdk import SentinelClient, TelemetryEventBuilder

client = SentinelClient(brokers=["localhost:9092"])

event = (TelemetryEventBuilder()
    .service("my-service")
    .model("gpt-4")
    .prompt(text="Hello, world!", tokens=3)
    .response(text="Hi there!", tokens=3, finish_reason="stop")
    .latency(150.5)
    .cost(0.001)
    .metadata({
        "region": "us-east-1",
        "environment": "production"
    })
    .trace_id("trace-123")
    .span_id("span-456")
    .build())

client.send(event)
client.close()
```

### With Error Tracking

```python
client.track(
    service="my-service",
    model="gpt-4",
    prompt="Write a poem",
    response="",
    latency_ms=5000.0,
    prompt_tokens=10,
    completion_tokens=0,
    cost_usd=0.0,
    finish_reason="error",
    errors=["Connection timeout", "Retry failed"]
)
```

### With Embeddings

```python
from sentinel_sdk import TelemetryEventBuilder

event = (TelemetryEventBuilder()
    .service("my-service")
    .model("text-embedding-ada-002")
    .prompt(
        text="Example text",
        tokens=5,
        embedding=[0.1, 0.2, 0.3, ...]  # Embedding vector
    )
    .response(
        text="",
        tokens=0,
        finish_reason="stop",
        embedding=[0.1, 0.2, 0.3, ...]  # Response embedding
    )
    .latency(50.0)
    .cost(0.0001)
    .build())

client.send(event)
```

### With Distributed Tracing

```python
# OpenTelemetry integration example
from opentelemetry import trace

tracer = trace.get_tracer(__name__)

with tracer.start_as_current_span("llm-request") as span:
    trace_id = format(span.get_span_context().trace_id, 'x')
    span_id = format(span.get_span_context().span_id, 'x')

    client.track(
        service="my-service",
        model="gpt-4",
        prompt="Hello",
        response="Hi!",
        latency_ms=150.0,
        prompt_tokens=5,
        completion_tokens=10,
        cost_usd=0.001,
        trace_id=trace_id,
        span_id=span_id
    )
```

## Configuration

### Environment Variables

The SDK supports configuration via environment variables:

```bash
export SENTINEL_BROKERS="localhost:9092,localhost:9093"
export SENTINEL_TOPIC="llm-telemetry"
export SENTINEL_CLIENT_ID="my-service"
export SENTINEL_COMPRESSION_TYPE="gzip"
export SENTINEL_ACKS="all"
```

Then use:

```python
from sentinel_sdk import SentinelClient

# Automatically loads from environment
client = SentinelClient.from_env()
```

### Custom Configuration

```python
from sentinel_sdk import SentinelClient, SentinelConfig

config = SentinelConfig(
    brokers=["localhost:9092"],
    topic="llm-telemetry",
    client_id="my-service",
    compression_type="gzip",
    acks="all",
    retries=3,
    request_timeout_ms=30000
)

client = SentinelClient(config=config)
```

### SSL/TLS Configuration

```python
from sentinel_sdk import SentinelConfig

config = SentinelConfig(
    brokers=["secure-broker:9093"],
    security_protocol="SSL",
    ssl_ca_location="/path/to/ca-cert",
    ssl_cert_location="/path/to/client-cert",
    ssl_key_location="/path/to/client-key"
)

client = SentinelClient(config=config)
```

### SASL Authentication

```python
from sentinel_sdk import SentinelConfig

config = SentinelConfig(
    brokers=["secure-broker:9093"],
    security_protocol="SASL_SSL",
    sasl_mechanism="PLAIN",
    sasl_username="your-username",
    sasl_password="your-password"
)

client = SentinelClient(config=config)
```

## Event Structure

The SDK sends events with the following structure:

```python
{
    "event_id": "uuid",
    "timestamp": "2024-01-01T00:00:00.000Z",
    "service_name": "my-service",
    "model": "gpt-4",
    "trace_id": "optional-trace-id",
    "span_id": "optional-span-id",
    "prompt": {
        "text": "prompt text",
        "tokens": 10,
        "embedding": [...]  # optional
    },
    "response": {
        "text": "response text",
        "tokens": 20,
        "finish_reason": "stop",
        "embedding": [...]  # optional
    },
    "latency_ms": 150.5,
    "cost_usd": 0.001,
    "metadata": {
        "key": "value"
    },
    "errors": []
}
```

## Type Definitions

### Severity Levels

```python
from sentinel_sdk import Severity

Severity.LOW
Severity.MEDIUM
Severity.HIGH
Severity.CRITICAL
```

### Anomaly Types

```python
from sentinel_sdk import AnomalyType

AnomalyType.LATENCY_SPIKE
AnomalyType.THROUGHPUT_DEGRADATION
AnomalyType.ERROR_RATE_INCREASE
AnomalyType.TOKEN_USAGE_SPIKE
AnomalyType.COST_ANOMALY
AnomalyType.INPUT_DRIFT
AnomalyType.OUTPUT_DRIFT
AnomalyType.CONCEPT_DRIFT
AnomalyType.EMBEDDING_DRIFT
AnomalyType.HALLUCINATION
AnomalyType.QUALITY_DEGRADATION
AnomalyType.SECURITY_THREAT
```

### Detection Methods

```python
from sentinel_sdk import DetectionMethod

DetectionMethod.Z_SCORE
DetectionMethod.IQR
DetectionMethod.MAD
DetectionMethod.CUSUM
DetectionMethod.ISOLATION_FOREST
DetectionMethod.LSTM_AUTOENCODER
DetectionMethod.ONE_CLASS_SVM
DetectionMethod.PSI
DetectionMethod.KL_DIVERGENCE
DetectionMethod.LLM_CHECK
DetectionMethod.RAG
```

## Exception Handling

```python
from sentinel_sdk import (
    SentinelError,
    ConnectionError,
    ValidationError,
    SendError,
    TimeoutError
)

try:
    client.track(
        service="my-service",
        model="gpt-4",
        prompt="Hello",
        response="Hi",
        latency_ms=150.0,
        prompt_tokens=5,
        completion_tokens=10,
        cost_usd=0.001
    )
except ValidationError as e:
    print(f"Invalid event: {e}")
except ConnectionError as e:
    print(f"Connection failed: {e}")
except SendError as e:
    print(f"Failed to send: {e}")
except TimeoutError as e:
    print(f"Request timed out: {e}")
except SentinelError as e:
    print(f"SDK error: {e}")
```

## Best Practices

### 1. Use Context Managers

Always use context managers to ensure proper resource cleanup:

```python
with SentinelClient(brokers=["localhost:9092"]) as client:
    client.track(...)
```

### 2. Configure Timeouts

Set appropriate timeouts for your use case:

```python
client.track(..., timeout=5.0)  # 5 second timeout
```

### 3. Batch Operations

The SDK automatically batches messages for efficiency. You can flush manually:

```python
client.flush()
```

### 4. Error Handling

Always handle exceptions to prevent application crashes:

```python
try:
    client.track(...)
except SentinelError as e:
    logger.error(f"Failed to send telemetry: {e}")
```

### 5. Environment-Based Configuration

Use environment variables for different environments:

```python
# Production
export SENTINEL_BROKERS="prod-kafka:9092"

# Development
export SENTINEL_BROKERS="localhost:9092"

# Code works in both environments
client = SentinelClient.from_env()
```

## Integration Examples

### OpenAI

```python
import openai
from sentinel_sdk import SentinelClient
import time

client = SentinelClient(brokers=["localhost:9092"])

def track_openai_request(prompt, **kwargs):
    start = time.time()

    try:
        response = openai.ChatCompletion.create(
            model="gpt-4",
            messages=[{"role": "user", "content": prompt}],
            **kwargs
        )

        latency = (time.time() - start) * 1000

        client.track(
            service="my-app",
            model="gpt-4",
            prompt=prompt,
            response=response.choices[0].message.content,
            latency_ms=latency,
            prompt_tokens=response.usage.prompt_tokens,
            completion_tokens=response.usage.completion_tokens,
            cost_usd=calculate_cost(response.usage),
            finish_reason=response.choices[0].finish_reason
        )

        return response
    except Exception as e:
        client.track(
            service="my-app",
            model="gpt-4",
            prompt=prompt,
            response="",
            latency_ms=(time.time() - start) * 1000,
            prompt_tokens=0,
            completion_tokens=0,
            cost_usd=0.0,
            errors=[str(e)]
        )
        raise
```

### Anthropic Claude

```python
import anthropic
from sentinel_sdk import SentinelClient
import time

client = SentinelClient(brokers=["localhost:9092"])
claude = anthropic.Anthropic()

def track_claude_request(prompt, **kwargs):
    start = time.time()

    message = claude.messages.create(
        model="claude-3-opus-20240229",
        messages=[{"role": "user", "content": prompt}],
        **kwargs
    )

    latency = (time.time() - start) * 1000

    client.track(
        service="my-app",
        model="claude-3-opus",
        prompt=prompt,
        response=message.content[0].text,
        latency_ms=latency,
        prompt_tokens=message.usage.input_tokens,
        completion_tokens=message.usage.output_tokens,
        cost_usd=calculate_claude_cost(message.usage),
        finish_reason=message.stop_reason
    )

    return message
```

### LangChain

```python
from langchain.callbacks.base import BaseCallbackHandler
from sentinel_sdk import SentinelClient
import time

class SentinelCallbackHandler(BaseCallbackHandler):
    def __init__(self, service_name: str):
        self.client = SentinelClient(brokers=["localhost:9092"])
        self.service_name = service_name
        self.start_time = None
        self.prompt = None

    def on_llm_start(self, serialized, prompts, **kwargs):
        self.start_time = time.time()
        self.prompt = prompts[0] if prompts else ""

    def on_llm_end(self, response, **kwargs):
        if self.start_time:
            latency = (time.time() - self.start_time) * 1000

            self.client.track(
                service=self.service_name,
                model=response.llm_output.get("model_name", "unknown"),
                prompt=self.prompt,
                response=response.generations[0][0].text,
                latency_ms=latency,
                prompt_tokens=response.llm_output.get("token_usage", {}).get("prompt_tokens", 0),
                completion_tokens=response.llm_output.get("token_usage", {}).get("completion_tokens", 0),
                cost_usd=0.0  # Calculate based on your pricing
            )

# Use with LangChain
from langchain.llms import OpenAI

llm = OpenAI(callbacks=[SentinelCallbackHandler("my-langchain-app")])
result = llm("What is the capital of France?")
```

## Development

### Running Tests

```bash
# Install development dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Run tests with coverage
pytest --cov=sentinel_sdk --cov-report=html
```

### Code Formatting

```bash
# Format code
black sentinel_sdk/

# Sort imports
isort sentinel_sdk/

# Type checking
mypy sentinel_sdk/

# Linting
flake8 sentinel_sdk/
```

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## Support

- Documentation: [GitHub Repository](https://github.com/yourusername/sentinel)
- Issues: [GitHub Issues](https://github.com/yourusername/sentinel/issues)
- Discussions: [GitHub Discussions](https://github.com/yourusername/sentinel/discussions)
