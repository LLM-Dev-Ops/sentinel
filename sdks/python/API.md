# API Reference

Complete API reference for the LLM Sentinel Python SDK.

## Client Classes

### SentinelClient

Synchronous client for sending telemetry events to Sentinel.

```python
class SentinelClient:
    def __init__(
        self,
        brokers: Optional[List[str]] = None,
        config: Optional[SentinelConfig] = None,
        **kwargs
    ) -> None
```

**Parameters:**
- `brokers` (List[str], optional): List of Kafka broker addresses
- `config` (SentinelConfig, optional): Configuration object
- `**kwargs`: Additional configuration options

**Methods:**

#### send()
```python
def send(self, event: TelemetryEvent, timeout: Optional[float] = None) -> None
```
Send a telemetry event to Sentinel.

**Parameters:**
- `event` (TelemetryEvent): The event to send
- `timeout` (float, optional): Timeout in seconds

**Raises:**
- `SendError`: If sending fails
- `TimeoutError`: If sending times out
- `ValidationError`: If event validation fails

#### track()
```python
def track(
    self,
    service: str,
    model: str,
    prompt: str,
    response: str,
    latency_ms: float,
    prompt_tokens: int,
    completion_tokens: int,
    cost_usd: float,
    finish_reason: str = "stop",
    trace_id: Optional[str] = None,
    span_id: Optional[str] = None,
    metadata: Optional[Dict[str, str]] = None,
    errors: Optional[List[str]] = None,
    prompt_embedding: Optional[List[float]] = None,
    response_embedding: Optional[List[float]] = None,
    timeout: Optional[float] = None,
) -> None
```
Convenience method to track an LLM request.

#### flush()
```python
def flush(self, timeout: Optional[float] = None) -> None
```
Flush pending messages.

#### close()
```python
def close() -> None
```
Close the client and release resources.

### AsyncSentinelClient

Asynchronous client for sending telemetry events to Sentinel.

```python
class AsyncSentinelClient:
    def __init__(
        self,
        brokers: Optional[List[str]] = None,
        config: Optional[SentinelConfig] = None,
        **kwargs
    ) -> None
```

**Methods:**

#### start()
```python
async def start() -> None
```
Start the async producer.

#### send()
```python
async def send(self, event: TelemetryEvent, timeout: Optional[float] = None) -> None
```
Send a telemetry event to Sentinel (async).

#### track()
```python
async def track(...) -> None
```
Convenience method to track an LLM request (async).

#### flush()
```python
async def flush() -> None
```
Flush pending messages (async).

#### close()
```python
async def close() -> None
```
Close the client and release resources (async).

## Event Classes

### TelemetryEvent

```python
@dataclass
class TelemetryEvent:
    service_name: str
    model: str
    prompt: PromptInfo
    response: ResponseInfo
    latency_ms: float
    cost_usd: float
    event_id: UUID = field(default_factory=uuid4)
    timestamp: datetime = field(default_factory=datetime.utcnow)
    trace_id: Optional[str] = None
    span_id: Optional[str] = None
    metadata: Dict[str, str] = field(default_factory=dict)
    errors: List[str] = field(default_factory=list)
```

**Methods:**

- `has_errors() -> bool`: Check if event has errors
- `total_tokens() -> int`: Calculate total tokens
- `error_rate() -> float`: Get error rate (0 or 1)
- `to_dict() -> Dict`: Convert to dictionary
- `from_dict(data: Dict) -> TelemetryEvent`: Create from dictionary (classmethod)

### PromptInfo

```python
@dataclass
class PromptInfo:
    text: str
    tokens: int
    embedding: Optional[List[float]] = None
```

**Methods:**
- `to_dict() -> Dict`: Convert to dictionary

### ResponseInfo

```python
@dataclass
class ResponseInfo:
    text: str
    tokens: int
    finish_reason: str
    embedding: Optional[List[float]] = None
```

**Methods:**
- `to_dict() -> Dict`: Convert to dictionary

## Builder Classes

### TelemetryEventBuilder

Fluent builder for constructing TelemetryEvent instances.

```python
class TelemetryEventBuilder:
    def __init__(self) -> None
```

**Methods:**

All methods return `self` for method chaining.

- `event_id(event_id: UUID) -> TelemetryEventBuilder`
- `timestamp(timestamp: datetime) -> TelemetryEventBuilder`
- `service(service_name: str) -> TelemetryEventBuilder`
- `model(model: str) -> TelemetryEventBuilder`
- `trace_id(trace_id: str) -> TelemetryEventBuilder`
- `span_id(span_id: str) -> TelemetryEventBuilder`
- `prompt(text: str, tokens: int, embedding: Optional[List[float]] = None) -> TelemetryEventBuilder`
- `prompt_info(prompt: PromptInfo) -> TelemetryEventBuilder`
- `response(text: str, tokens: int, finish_reason: str, embedding: Optional[List[float]] = None) -> TelemetryEventBuilder`
- `response_info(response: ResponseInfo) -> TelemetryEventBuilder`
- `latency(latency_ms: float) -> TelemetryEventBuilder`
- `cost(cost_usd: float) -> TelemetryEventBuilder`
- `metadata(metadata: Dict[str, str]) -> TelemetryEventBuilder`
- `add_metadata(key: str, value: str) -> TelemetryEventBuilder`
- `errors(errors: List[str]) -> TelemetryEventBuilder`
- `add_error(error: str) -> TelemetryEventBuilder`
- `build() -> TelemetryEvent`

## Configuration

### SentinelConfig

```python
@dataclass
class SentinelConfig:
    brokers: List[str]
    topic: str = "llm-telemetry"
    client_id: str = "sentinel-sdk-python"
    compression_type: str = "gzip"
    acks: str = "all"
    retries: int = 3
    retry_backoff_ms: int = 100
    request_timeout_ms: int = 30000
    linger_ms: int = 10
    batch_size: int = 16384
    max_in_flight_requests: int = 5
    enable_idempotence: bool = True
    buffer_memory: int = 33554432
    max_request_size: int = 1048576
    metadata_max_age_ms: int = 300000
    security_protocol: str = "PLAINTEXT"
    sasl_mechanism: Optional[str] = None
    sasl_username: Optional[str] = None
    sasl_password: Optional[str] = None
    ssl_ca_location: Optional[str] = None
    ssl_cert_location: Optional[str] = None
    ssl_key_location: Optional[str] = None
    additional_config: Dict[str, str] = field(default_factory=dict)
```

**Methods:**

- `from_env(**kwargs) -> SentinelConfig`: Create from environment variables (classmethod)
- `to_kafka_config() -> Dict[str, str]`: Convert to kafka-python config
- `to_aiokafka_config() -> Dict[str, str]`: Convert to aiokafka config

## Type Enums

### Severity

```python
class Severity(str, Enum):
    LOW = "low"
    MEDIUM = "medium"
    HIGH = "high"
    CRITICAL = "critical"
```

Supports ordering: `LOW < MEDIUM < HIGH < CRITICAL`

### AnomalyType

```python
class AnomalyType(str, Enum):
    LATENCY_SPIKE = "latency_spike"
    THROUGHPUT_DEGRADATION = "throughput_degradation"
    ERROR_RATE_INCREASE = "error_rate_increase"
    TOKEN_USAGE_SPIKE = "token_usage_spike"
    COST_ANOMALY = "cost_anomaly"
    INPUT_DRIFT = "input_drift"
    OUTPUT_DRIFT = "output_drift"
    CONCEPT_DRIFT = "concept_drift"
    EMBEDDING_DRIFT = "embedding_drift"
    HALLUCINATION = "hallucination"
    QUALITY_DEGRADATION = "quality_degradation"
    SECURITY_THREAT = "security_threat"
```

### DetectionMethod

```python
class DetectionMethod(str, Enum):
    Z_SCORE = "z_score"
    IQR = "iqr"
    MAD = "mad"
    CUSUM = "cusum"
    ISOLATION_FOREST = "isolation_forest"
    LSTM_AUTOENCODER = "lstm_autoencoder"
    ONE_CLASS_SVM = "one_class_svm"
    PSI = "psi"
    KL_DIVERGENCE = "kl_divergence"
    LLM_CHECK = "llm_check"
    RAG = "rag"
```

## Exceptions

### SentinelError

Base exception for all SDK errors.

```python
class SentinelError(Exception):
    def __init__(self, message: str, cause: Exception | None = None) -> None
```

**Attributes:**
- `message`: Error message
- `cause`: Optional underlying exception

### ConnectionError

Exception raised when connection to Kafka fails.

```python
class ConnectionError(SentinelError)
```

### ValidationError

Exception raised when event validation fails.

```python
class ValidationError(SentinelError)
```

### SendError

Exception raised when sending an event fails.

```python
class SendError(SentinelError)
```

### ConfigurationError

Exception raised when SDK configuration is invalid.

```python
class ConfigurationError(SentinelError)
```

### TimeoutError

Exception raised when an operation times out.

```python
class TimeoutError(SentinelError)
```

## Environment Variables

The SDK supports the following environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `SENTINEL_BROKERS` | Comma-separated list of brokers | `localhost:9092` |
| `SENTINEL_TOPIC` | Kafka topic name | `llm-telemetry` |
| `SENTINEL_CLIENT_ID` | Client identifier | `sentinel-sdk-python` |
| `SENTINEL_COMPRESSION_TYPE` | Compression type | `gzip` |
| `SENTINEL_ACKS` | Acknowledgment mode | `all` |
| `SENTINEL_RETRIES` | Number of retries | `3` |
| `SENTINEL_SECURITY_PROTOCOL` | Security protocol | `PLAINTEXT` |
| `SENTINEL_SASL_MECHANISM` | SASL mechanism | None |
| `SENTINEL_SASL_USERNAME` | SASL username | None |
| `SENTINEL_SASL_PASSWORD` | SASL password | None |
| `SENTINEL_SSL_CA_LOCATION` | SSL CA certificate | None |
| `SENTINEL_SSL_CERT_LOCATION` | SSL certificate | None |
| `SENTINEL_SSL_KEY_LOCATION` | SSL key | None |

## Type Hints

The SDK is fully typed and includes a `py.typed` marker file for PEP 561 compliance. All public APIs include type hints for IDE support and static type checking with mypy.

## Version

Current version: `0.1.0`

Access via:
```python
from sentinel_sdk import __version__
print(__version__)  # "0.1.0"
```
