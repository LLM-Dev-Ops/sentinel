# Sentinel Go SDK - API Reference

Complete API documentation for the Sentinel Go SDK.

## Table of Contents

- [Client](#client)
- [Events](#events)
- [Builder](#builder)
- [Configuration](#configuration)
- [Validation](#validation)
- [Errors](#errors)
- [Types](#types)

## Client

### NewClient

Creates a new Sentinel client with functional options.

```go
func NewClient(opts ...ClientOption) (*Client, error)
```

**Example:**
```go
client, err := sentinel.NewClient(
    sentinel.WithBrokers([]string{"localhost:9092"}),
    sentinel.WithTopic("llm.telemetry"),
)
```

### NewClientFromEnv

Creates a client using environment variables.

```go
func NewClientFromEnv() (*Client, error)
```

### Client.Send

Sends a single telemetry event.

```go
func (c *Client) Send(ctx context.Context, event *TelemetryEvent) error
```

**Example:**
```go
err := client.Send(context.Background(), event)
```

### Client.SendBatch

Sends multiple events in a single batch.

```go
func (c *Client) SendBatch(ctx context.Context, events []*TelemetryEvent) error
```

**Example:**
```go
err := client.SendBatch(ctx, []*TelemetryEvent{event1, event2})
```

### Client.Track

Convenience method for simple event tracking.

```go
func (c *Client) Track(ctx context.Context, opts TrackOptions) error
```

**Example:**
```go
err := client.Track(ctx, sentinel.TrackOptions{
    Service: "my-service",
    Model: "gpt-4",
    Prompt: "Hello",
    Response: "Hi",
    LatencyMs: 100,
    PromptTokens: 1,
    CompletionTokens: 1,
    CostUsd: 0.001,
})
```

### Client.Close

Closes the client and releases resources.

```go
func (c *Client) Close() error
```

### Client.Stats

Returns Kafka writer statistics.

```go
func (c *Client) Stats() kafka.WriterStats
```

### Client.IsClosed

Returns true if the client has been closed.

```go
func (c *Client) IsClosed() bool
```

## Events

### TelemetryEvent

Main event structure for LLM telemetry.

```go
type TelemetryEvent struct {
    EventID     string            // UUID, auto-generated
    Timestamp   string            // ISO 8601, auto-generated
    ServiceName string            // Required
    TraceID     *string           // Optional
    SpanID      *string           // Optional
    Model       string            // Required
    Prompt      PromptInfo        // Required
    Response    ResponseInfo      // Required
    LatencyMs   float64           // >= 0
    CostUsd     float64           // >= 0
    Metadata    map[string]string
    Errors      []string
}
```

### PromptInfo

Information about the input prompt.

```go
type PromptInfo struct {
    Text      string    // Max 100,000 chars
    Tokens    uint32    // > 0
    Embedding []float32 // Optional
}
```

### ResponseInfo

Information about the LLM response.

```go
type ResponseInfo struct {
    Text         string    // Max 100,000 chars
    Tokens       uint32    // > 0
    FinishReason string    // Required
    Embedding    []float32 // Optional
}
```

### NewTelemetryEvent

Creates a new event with auto-generated ID and timestamp.

```go
func NewTelemetryEvent(serviceName, model string) *TelemetryEvent
```

### Event Methods

```go
// Check if event has errors
func (e *TelemetryEvent) HasErrors() bool

// Get total tokens (prompt + response)
func (e *TelemetryEvent) TotalTokens() uint32

// Get error rate (0.0 or 1.0)
func (e *TelemetryEvent) ErrorRate() float64

// Set metadata key-value pair
func (e *TelemetryEvent) SetMetadata(key, value string)

// Add error message
func (e *TelemetryEvent) AddError(err string)
```

## Builder

### NewEventBuilder

Creates a new event builder.

```go
func NewEventBuilder() *EventBuilder
```

### Builder Methods

All methods return `*EventBuilder` for chaining.

```go
// Set service name
Service(name string) *EventBuilder

// Set model identifier
Model(model string) *EventBuilder

// Set prompt text and tokens
Prompt(text string, tokens uint32) *EventBuilder

// Set prompt with embedding
PromptWithEmbedding(text string, tokens uint32, embedding []float32) *EventBuilder

// Set response text, tokens, and finish reason
Response(text string, tokens uint32, finishReason string) *EventBuilder

// Set response with embedding
ResponseWithEmbedding(text string, tokens uint32, finishReason string, embedding []float32) *EventBuilder

// Set latency in milliseconds
Latency(latencyMs float64) *EventBuilder

// Set cost in USD
Cost(costUsd float64) *EventBuilder

// Set trace ID
TraceID(traceID string) *EventBuilder

// Set span ID
SpanID(spanID string) *EventBuilder

// Set metadata map
Metadata(metadata map[string]string) *EventBuilder

// Add single metadata key-value
AddMetadata(key, value string) *EventBuilder

// Add error message
Error(err string) *EventBuilder

// Set errors slice
Errors(errors []string) *EventBuilder

// Set custom event ID
EventID(id string) *EventBuilder

// Set custom timestamp
Timestamp(timestamp string) *EventBuilder

// Build and return the event
Build() *TelemetryEvent
```

**Example:**
```go
event := sentinel.NewEventBuilder().
    Service("my-service").
    Model("gpt-4").
    Prompt("Hello", 1).
    Response("Hi", 1, "stop").
    Latency(100.0).
    Cost(0.001).
    AddMetadata("region", "us-east-1").
    Build()
```

## Configuration

### Config

Configuration structure for the client.

```go
type Config struct {
    Brokers          []string      // Kafka brokers
    Topic            string        // Kafka topic
    MaxRetries       int           // Max retry attempts
    RetryBackoff     time.Duration // Initial retry backoff
    MaxRetryBackoff  time.Duration // Max retry backoff
    WriteTimeout     time.Duration // Write timeout
    ReadTimeout      time.Duration // Read timeout
    BatchSize        int           // Batch size (0 = no batching)
    BatchTimeout     time.Duration // Batch timeout
    Compression      string        // Compression codec
    RequireAcks      int           // Required acks (-1, 0, 1)
    Async            bool          // Async mode
    MaxAttempts      int           // Max connection attempts
    EnableValidation bool          // Enable validation
    TruncateText     bool          // Auto-truncate text
    MaxTextLength    int           // Max text length
}
```

### DefaultConfig

Returns configuration with sensible defaults.

```go
func DefaultConfig() *Config
```

### LoadConfigFromEnv

Loads configuration from environment variables.

```go
func LoadConfigFromEnv() (*Config, error)
```

### Config.Validate

Validates the configuration.

```go
func (c *Config) Validate() error
```

### Config.Clone

Creates a deep copy of the configuration.

```go
func (c *Config) Clone() *Config
```

### Client Options

Functional options for configuring the client.

```go
WithBrokers(brokers []string) ClientOption
WithTopic(topic string) ClientOption
WithMaxRetries(maxRetries int) ClientOption
WithRetryBackoff(backoff time.Duration) ClientOption
WithWriteTimeout(timeout time.Duration) ClientOption
WithCompression(codec string) ClientOption
WithBatchSize(size int) ClientOption
WithBatchTimeout(timeout time.Duration) ClientOption
WithAsync(async bool) ClientOption
WithValidation(enabled bool) ClientOption
WithConfig(config *Config) ClientOption
```

## Validation

### Validate

Validates a telemetry event.

```go
func Validate(event *TelemetryEvent) error
```

Returns `ValidationError` if validation fails.

### TruncateText

Truncates text fields to the specified maximum length.

```go
func TruncateText(event *TelemetryEvent, maxLength int)
```

### SanitizeEvent

Ensures the event is valid and ready to send.
Auto-generates missing fields and truncates text if needed.

```go
func SanitizeEvent(event *TelemetryEvent, config *Config)
```

### NormalizeServiceName

Normalizes a service name to lowercase with hyphens.

```go
func NormalizeServiceName(name string) string
```

### NormalizeModelName

Normalizes a model name to lowercase.

```go
func NormalizeModelName(name string) string
```

## Errors

### Standard Errors

```go
var (
    ErrInvalidConfig   error // Invalid configuration
    ErrNotConnected    error // Not connected to Kafka
    ErrClientClosed    error // Client is closed
    ErrContextCanceled error // Context was canceled
    ErrInvalidEvent    error // Event validation failed
)
```

### SentinelError

Interface for all Sentinel SDK errors.

```go
type SentinelError interface {
    error
    Unwrap() error
    Code() string
}
```

### ConnectionError

Error connecting to Kafka.

```go
type ConnectionError struct {
    Brokers []string
    Err     error
}

func NewConnectionError(brokers []string, err error) *ConnectionError
```

### ValidationError

Event validation error.

```go
type ValidationError struct {
    Field   string
    Message string
    Value   interface{}
}

func NewValidationError(field, message string, value interface{}) *ValidationError
```

### SendError

Error sending event to Kafka.

```go
type SendError struct {
    Topic   string
    EventID string
    Err     error
    Retries int
}

func NewSendError(topic, eventID string, err error, retries int) *SendError
```

### ConfigError

Configuration error.

```go
type ConfigError struct {
    Field   string
    Message string
}

func NewConfigError(field, message string) *ConfigError
```

## Types

### Severity

Severity level for anomalies and alerts.

```go
type Severity string

const (
    SeverityLow      Severity = "low"
    SeverityMedium   Severity = "medium"
    SeverityHigh     Severity = "high"
    SeverityCritical Severity = "critical"
)
```

### AnomalyType

Type of anomaly detected.

```go
type AnomalyType string

const (
    AnomalyTypeLatencySpike              AnomalyType = "latency_spike"
    AnomalyTypeThroughputDegradation    AnomalyType = "throughput_degradation"
    AnomalyTypeErrorRateIncrease        AnomalyType = "error_rate_increase"
    AnomalyTypeTokenUsageSpike          AnomalyType = "token_usage_spike"
    AnomalyTypeCostAnomaly              AnomalyType = "cost_anomaly"
    AnomalyTypeInputDrift               AnomalyType = "input_drift"
    AnomalyTypeOutputDrift              AnomalyType = "output_drift"
    AnomalyTypeConceptDrift             AnomalyType = "concept_drift"
    AnomalyTypeEmbeddingDrift           AnomalyType = "embedding_drift"
    AnomalyTypeHallucination            AnomalyType = "hallucination"
    AnomalyTypeQualityDegradation       AnomalyType = "quality_degradation"
    AnomalyTypeSecurityThreat           AnomalyType = "security_threat"
)
```

### DetectionMethod

Method used to identify an anomaly.

```go
type DetectionMethod string

const (
    DetectionMethodZScore           DetectionMethod = "z_score"
    DetectionMethodIQR              DetectionMethod = "iqr"
    DetectionMethodMAD              DetectionMethod = "mad"
    DetectionMethodCUSUM            DetectionMethod = "cusum"
    DetectionMethodIsolationForest  DetectionMethod = "isolation_forest"
    DetectionMethodLSTMAutoencoder  DetectionMethod = "lstm_autoencoder"
    DetectionMethodOneClassSVM      DetectionMethod = "one_class_svm"
    DetectionMethodPSI              DetectionMethod = "psi"
    DetectionMethodKLDivergence     DetectionMethod = "kl_divergence"
    DetectionMethodLLMCheck         DetectionMethod = "llm_check"
    DetectionMethodRAG              DetectionMethod = "rag"
)
```

## TrackOptions

Simplified options for the Track() API.

```go
type TrackOptions struct {
    Service           string
    Model             string
    Prompt            string
    Response          string
    LatencyMs         float64
    PromptTokens      uint32
    CompletionTokens  uint32
    CostUsd           float64
    FinishReason      string            // Default: "stop"
    TraceID           string            // Optional
    SpanID            string            // Optional
    Metadata          map[string]string // Optional
    Errors            []string          // Optional
    PromptEmbedding   []float32         // Optional
    ResponseEmbedding []float32         // Optional
}
```

## Package-Level Functions

### BuildFromOptions

Creates a telemetry event from TrackOptions.

```go
func BuildFromOptions(opts TrackOptions) *TelemetryEvent
```
