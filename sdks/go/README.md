# Sentinel Go SDK

Official Go SDK for [LLM-Sentinel](https://github.com/llm-sentinel/sentinel) - Send LLM telemetry events for real-time anomaly detection.

[![Go Version](https://img.shields.io/badge/go-%3E%3D1.21-blue.svg)](https://go.dev/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

## Features

- **Simple API**: Easy-to-use client for sending telemetry events
- **Fluent Builder**: Ergonomic builder pattern for constructing events
- **Production-Ready**: Built-in retry logic with exponential backoff
- **Type-Safe**: Strongly-typed event structures
- **Validation**: Automatic event validation before sending
- **Kafka Integration**: Uses `segmentio/kafka-go` for reliable message delivery
- **Context Support**: Full context.Context support for cancellation and timeouts
- **Batching**: Support for batch sends to improve throughput
- **Configuration**: Environment variable support with sensible defaults
- **Zero Dependencies**: Minimal external dependencies

## Installation

```bash
go get github.com/llm-sentinel/sentinel-sdk-go
```

## Quick Start

### Basic Usage

```go
package main

import (
    "context"
    "log"

    "github.com/llm-sentinel/sentinel-sdk-go/sentinel"
)

func main() {
    // Create client
    client, err := sentinel.NewClient(
        sentinel.WithBrokers([]string{"localhost:9092"}),
        sentinel.WithTopic("llm.telemetry"),
    )
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    // Track an LLM request using the simple API
    err = client.Track(context.Background(), sentinel.TrackOptions{
        Service:          "my-service",
        Model:            "gpt-4",
        Prompt:           "What is the capital of France?",
        Response:         "The capital of France is Paris.",
        LatencyMs:        150.5,
        PromptTokens:     8,
        CompletionTokens: 7,
        CostUsd:          0.001,
    })
    if err != nil {
        log.Printf("Failed to send event: %v", err)
    }
}
```

### Builder Pattern

```go
// Build event using fluent API
event := sentinel.NewEventBuilder().
    Service("my-service").
    Model("gpt-4").
    Prompt("What is the capital of France?", 8).
    Response("The capital of France is Paris.", 7, "stop").
    Latency(150.5).
    Cost(0.001).
    AddMetadata("region", "us-east-1").
    AddMetadata("environment", "production").
    Build()

err := client.Send(context.Background(), event)
```

### Environment Variables

```go
// Load configuration from environment variables
client, err := sentinel.NewClientFromEnv()
if err != nil {
    log.Fatal(err)
}
defer client.Close()
```

Set these environment variables:

```bash
export SENTINEL_BROKERS=kafka-1:9092,kafka-2:9092,kafka-3:9092
export SENTINEL_TOPIC=llm.telemetry
export SENTINEL_MAX_RETRIES=3
export SENTINEL_RETRY_BACKOFF=100ms
export SENTINEL_COMPRESSION=snappy
```

## Advanced Usage

### Custom Configuration

```go
config := &sentinel.Config{
    Brokers:          []string{"kafka-1:9092", "kafka-2:9092"},
    Topic:            "llm.telemetry",
    MaxRetries:       5,
    RetryBackoff:     100 * time.Millisecond,
    MaxRetryBackoff:  5 * time.Second,
    WriteTimeout:     10 * time.Second,
    Compression:      "snappy",
    RequireAcks:      -1, // Wait for all in-sync replicas
    EnableValidation: true,
    TruncateText:     true,
    MaxTextLength:    100000,
}

client, err := sentinel.NewClient(sentinel.WithConfig(config))
```

### Batch Sends

```go
events := []*sentinel.TelemetryEvent{
    sentinel.NewEventBuilder().
        Service("my-service").
        Model("gpt-4").
        Prompt("Hello", 1).
        Response("Hi there!", 2, "stop").
        Latency(100).
        Cost(0.0001).
        Build(),
    sentinel.NewEventBuilder().
        Service("my-service").
        Model("gpt-4").
        Prompt("How are you?", 3).
        Response("I'm doing well, thank you!", 5, "stop").
        Latency(120).
        Cost(0.0002).
        Build(),
}

err := client.SendBatch(context.Background(), events)
```

### Distributed Tracing

```go
event := sentinel.NewEventBuilder().
    Service("my-service").
    Model("gpt-4").
    Prompt("Hello", 1).
    Response("Hi!", 1, "stop").
    Latency(100).
    Cost(0.0001).
    TraceID("4bf92f3577b34da6a3ce929d0e0e4736").
    SpanID("00f067aa0ba902b7").
    Build()

err := client.Send(ctx, event)
```

### Error Handling

```go
err := client.Send(ctx, event)
if err != nil {
    // Check for specific error types
    var validationErr *sentinel.ValidationError
    var sendErr *sentinel.SendError
    var connErr *sentinel.ConnectionError

    switch {
    case errors.As(err, &validationErr):
        log.Printf("Validation error on field %s: %s", validationErr.Field, validationErr.Message)
    case errors.As(err, &sendErr):
        log.Printf("Send error after %d retries: %v", sendErr.Retries, sendErr.Err)
    case errors.As(err, &connErr):
        log.Printf("Connection error to brokers %v: %v", connErr.Brokers, connErr.Err)
    default:
        log.Printf("Unexpected error: %v", err)
    }
}
```

### With Embeddings

```go
promptEmbedding := []float32{0.1, 0.2, 0.3, /* ... */}
responseEmbedding := []float32{0.4, 0.5, 0.6, /* ... */}

event := sentinel.NewEventBuilder().
    Service("my-service").
    Model("text-embedding-3-small").
    PromptWithEmbedding("Hello world", 2, promptEmbedding).
    ResponseWithEmbedding("Embedding generated", 2, "stop", responseEmbedding).
    Latency(50).
    Cost(0.00001).
    Build()

err := client.Send(ctx, event)
```

### Context and Timeouts

```go
// With timeout
ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
defer cancel()

err := client.Send(ctx, event)
if errors.Is(err, sentinel.ErrContextCanceled) {
    log.Println("Send operation timed out")
}

// With cancellation
ctx, cancel := context.WithCancel(context.Background())

go func() {
    time.Sleep(2 * time.Second)
    cancel() // Cancel after 2 seconds
}()

err := client.Send(ctx, event)
```

### Async Mode

```go
// Enable async mode for fire-and-forget sends
client, err := sentinel.NewClient(
    sentinel.WithBrokers([]string{"localhost:9092"}),
    sentinel.WithAsync(true),
)

// Sends return immediately without waiting for Kafka ack
err = client.Send(ctx, event) // Non-blocking
```

### Client Statistics

```go
// Get Kafka writer statistics
stats := client.Stats()

fmt.Printf("Messages written: %d\n", stats.Writes)
fmt.Printf("Messages sent: %d\n", stats.Messages)
fmt.Printf("Bytes written: %d\n", stats.Bytes)
fmt.Printf("Errors: %d\n", stats.Errors)
fmt.Printf("Max attempts: %d\n", stats.MaxAttempts)
fmt.Printf("Batch size: %d\n", stats.BatchSize)
```

## Configuration Options

### Client Options

| Option | Description | Default |
|--------|-------------|---------|
| `WithBrokers([]string)` | Kafka broker addresses | `["localhost:9092"]` |
| `WithTopic(string)` | Kafka topic for telemetry | `"llm.telemetry"` |
| `WithMaxRetries(int)` | Max retry attempts | `3` |
| `WithRetryBackoff(duration)` | Initial retry backoff | `100ms` |
| `WithWriteTimeout(duration)` | Write timeout | `10s` |
| `WithCompression(string)` | Compression codec | `"snappy"` |
| `WithBatchSize(int)` | Batch size (0 = no batching) | `0` |
| `WithBatchTimeout(duration)` | Batch timeout | `0s` |
| `WithAsync(bool)` | Enable async mode | `false` |
| `WithValidation(bool)` | Enable validation | `true` |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SENTINEL_BROKERS` | Kafka brokers (comma-separated) | `localhost:9092` |
| `SENTINEL_TOPIC` | Kafka topic | `llm.telemetry` |
| `SENTINEL_MAX_RETRIES` | Max retry attempts | `3` |
| `SENTINEL_RETRY_BACKOFF` | Initial retry backoff | `100ms` |
| `SENTINEL_MAX_RETRY_BACKOFF` | Max retry backoff | `5s` |
| `SENTINEL_WRITE_TIMEOUT` | Write timeout | `10s` |
| `SENTINEL_READ_TIMEOUT` | Read timeout | `10s` |
| `SENTINEL_BATCH_SIZE` | Batch size | `0` |
| `SENTINEL_BATCH_TIMEOUT` | Batch timeout | `0s` |
| `SENTINEL_COMPRESSION` | Compression codec | `snappy` |
| `SENTINEL_REQUIRE_ACKS` | Required acks (-1, 0, 1) | `-1` |
| `SENTINEL_ASYNC` | Enable async mode | `false` |
| `SENTINEL_ENABLE_VALIDATION` | Enable validation | `true` |
| `SENTINEL_TRUNCATE_TEXT` | Auto-truncate text | `true` |
| `SENTINEL_MAX_TEXT_LENGTH` | Max text length | `100000` |

### Compression Codecs

Supported compression codecs:
- `none` - No compression
- `gzip` - GZIP compression
- `snappy` - Snappy compression (default, recommended)
- `lz4` - LZ4 compression
- `zstd` - Zstandard compression

### Required Acks

- `-1` (RequireAll): Wait for all in-sync replicas (most durable, default)
- `0` (RequireNone): Don't wait for any acks (fastest, least durable)
- `1` (RequireOne): Wait for leader only (balanced)

## Event Schema

```go
type TelemetryEvent struct {
    EventID     string            `json:"event_id"`      // UUID, auto-generated
    Timestamp   string            `json:"timestamp"`     // ISO 8601, auto-generated
    ServiceName string            `json:"service_name"`  // Required
    TraceID     *string           `json:"trace_id,omitempty"`
    SpanID      *string           `json:"span_id,omitempty"`
    Model       string            `json:"model"`         // Required
    Prompt      PromptInfo        `json:"prompt"`        // Required
    Response    ResponseInfo      `json:"response"`      // Required
    LatencyMs   float64           `json:"latency_ms"`    // >= 0
    CostUsd     float64           `json:"cost_usd"`      // >= 0
    Metadata    map[string]string `json:"metadata"`
    Errors      []string          `json:"errors"`
}

type PromptInfo struct {
    Text      string    `json:"text"`               // Required, max 100k chars
    Tokens    uint32    `json:"tokens"`             // Required, > 0
    Embedding []float32 `json:"embedding,omitempty"`
}

type ResponseInfo struct {
    Text         string    `json:"text"`               // Required, max 100k chars
    Tokens       uint32    `json:"tokens"`             // Required, > 0
    FinishReason string    `json:"finish_reason"`      // Required
    Embedding    []float32 `json:"embedding,omitempty"`
}
```

## Best Practices

### 1. Resource Management

Always close the client when done:

```go
client, err := sentinel.NewClient(...)
if err != nil {
    return err
}
defer client.Close()
```

### 2. Error Handling

Always handle errors from Send operations:

```go
if err := client.Send(ctx, event); err != nil {
    log.Printf("Failed to send telemetry: %v", err)
    // Consider your error handling strategy:
    // - Log and continue
    // - Retry with backoff
    // - Store locally and replay later
}
```

### 3. Context Usage

Use contexts for timeout and cancellation:

```go
ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
defer cancel()

err := client.Send(ctx, event)
```

### 4. Batching for High Throughput

Use batch sends when sending multiple events:

```go
// Instead of this:
for _, event := range events {
    client.Send(ctx, event) // Multiple network calls
}

// Do this:
client.SendBatch(ctx, events) // Single network call
```

### 5. Metadata for Context

Use metadata for searchable context:

```go
event := sentinel.NewEventBuilder().
    // ... other fields ...
    AddMetadata("user_id", userID).
    AddMetadata("region", "us-east-1").
    AddMetadata("environment", "production").
    AddMetadata("version", "v1.2.3").
    Build()
```

### 6. Validation

Keep validation enabled in production:

```go
// Validation catches issues before sending to Kafka
client, err := sentinel.NewClient(
    sentinel.WithValidation(true), // Recommended for production
)
```

### 7. Compression

Use Snappy compression for best balance:

```go
client, err := sentinel.NewClient(
    sentinel.WithCompression("snappy"), // Good balance of speed and compression
)
```

## Examples

### OpenAI Integration

```go
import (
    "context"
    "time"

    "github.com/llm-sentinel/sentinel-sdk-go/sentinel"
    openai "github.com/sashabaranov/go-openai"
)

func callOpenAI(client *sentinel.Client, prompt string) error {
    openaiClient := openai.NewClient("your-api-key")

    start := time.Now()

    resp, err := openaiClient.CreateChatCompletion(
        context.Background(),
        openai.ChatCompletionRequest{
            Model: openai.GPT4,
            Messages: []openai.ChatCompletionMessage{
                {Role: openai.ChatMessageRoleUser, Content: prompt},
            },
        },
    )

    latency := time.Since(start).Seconds() * 1000 // Convert to ms

    // Build telemetry event
    event := sentinel.NewEventBuilder().
        Service("my-chatbot").
        Model(resp.Model).
        Prompt(prompt, uint32(resp.Usage.PromptTokens)).
        Response(resp.Choices[0].Message.Content, uint32(resp.Usage.CompletionTokens), string(resp.Choices[0].FinishReason)).
        Latency(latency).
        Cost(calculateCost(resp.Usage)). // Your cost calculation
        Build()

    if err != nil {
        event.AddError(err.Error())
    }

    // Send to Sentinel
    return client.Send(context.Background(), event)
}

func calculateCost(usage openai.Usage) float64 {
    // GPT-4 pricing (example)
    promptCost := float64(usage.PromptTokens) * 0.00003
    completionCost := float64(usage.CompletionTokens) * 0.00006
    return promptCost + completionCost
}
```

### Anthropic Claude Integration

```go
import (
    "context"
    "time"

    "github.com/llm-sentinel/sentinel-sdk-go/sentinel"
    anthropic "github.com/anthropics/anthropic-sdk-go"
)

func callClaude(client *sentinel.Client, prompt string) error {
    claudeClient := anthropic.NewClient("your-api-key")

    start := time.Now()

    resp, err := claudeClient.Messages.Create(context.Background(), anthropic.MessageCreateParams{
        Model:     anthropic.ModelClaude3Sonnet,
        MaxTokens: 1024,
        Messages: []anthropic.MessageParam{
            {Role: "user", Content: prompt},
        },
    })

    latency := time.Since(start).Seconds() * 1000

    event := sentinel.NewEventBuilder().
        Service("my-assistant").
        Model(string(resp.Model)).
        Prompt(prompt, uint32(resp.Usage.InputTokens)).
        Response(resp.Content[0].Text, uint32(resp.Usage.OutputTokens), string(resp.StopReason)).
        Latency(latency).
        Cost(calculateClaudeCost(resp.Usage)).
        Build()

    if err != nil {
        event.AddError(err.Error())
    }

    return client.Send(context.Background(), event)
}
```

## Troubleshooting

### Connection Issues

```go
// Test connection with a simple event
event := sentinel.NewEventBuilder().
    Service("test").
    Model("test").
    Prompt("test", 1).
    Response("test", 1, "stop").
    Latency(1).
    Cost(0).
    Build()

if err := client.Send(context.Background(), event); err != nil {
    log.Printf("Connection test failed: %v", err)
}
```

### Enable Debug Logging

The SDK uses `segmentio/kafka-go` which supports debug logging:

```go
import "github.com/segmentio/kafka-go"

// Set log level for kafka-go (requires configuration of their logger)
```

### Verify Kafka Topic

```bash
# Check if topic exists
kafka-topics --bootstrap-server localhost:9092 --list | grep llm.telemetry

# Create topic if needed
kafka-topics --bootstrap-server localhost:9092 --create --topic llm.telemetry --partitions 3 --replication-factor 2
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

This SDK is licensed under the MIT License. See [LICENSE](../../LICENSE) for details.

## Support

- **Documentation**: https://github.com/llm-sentinel/sentinel
- **Issues**: https://github.com/llm-sentinel/sentinel/issues
- **Discussions**: https://github.com/llm-sentinel/sentinel/discussions

## Related Projects

- [LLM-Sentinel](https://github.com/llm-sentinel/sentinel) - Main project
- [Python SDK](../python/) - Python SDK
- [TypeScript SDK](../typescript/) - TypeScript SDK
