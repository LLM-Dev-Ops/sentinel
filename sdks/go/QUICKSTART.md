# Sentinel Go SDK - Quick Start Guide

Get started with the Sentinel Go SDK in 5 minutes.

## Step 1: Install

```bash
go get github.com/llm-sentinel/sentinel-sdk-go
```

## Step 2: Import

```go
import "github.com/llm-sentinel/sentinel-sdk-go/sentinel"
```

## Step 3: Create a Client

```go
client, err := sentinel.NewClient(
    sentinel.WithBrokers([]string{"localhost:9092"}),
    sentinel.WithTopic("llm.telemetry"),
)
if err != nil {
    log.Fatal(err)
}
defer client.Close()
```

## Step 4: Send Your First Event

### Option A: Simple Track API

```go
err = client.Track(context.Background(), sentinel.TrackOptions{
    Service:          "my-service",
    Model:            "gpt-4",
    Prompt:           "What is 2+2?",
    Response:         "2+2 equals 4",
    LatencyMs:        150.5,
    PromptTokens:     5,
    CompletionTokens: 6,
    CostUsd:          0.001,
})
```

### Option B: Builder Pattern

```go
event := sentinel.NewEventBuilder().
    Service("my-service").
    Model("gpt-4").
    Prompt("What is 2+2?", 5).
    Response("2+2 equals 4", 6, "stop").
    Latency(150.5).
    Cost(0.001).
    Build()

err = client.Send(context.Background(), event)
```

## Step 5: Handle Errors

```go
if err != nil {
    var validationErr *sentinel.ValidationError
    if errors.As(err, &validationErr) {
        log.Printf("Validation error: %s", validationErr.Message)
    } else {
        log.Printf("Error sending event: %v", err)
    }
}
```

## Complete Example

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
    )
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    // Send event
    err = client.Track(context.Background(), sentinel.TrackOptions{
        Service:          "my-app",
        Model:            "gpt-4",
        Prompt:           "Hello, world!",
        Response:         "Hi there!",
        LatencyMs:        100.0,
        PromptTokens:     3,
        CompletionTokens: 2,
        CostUsd:          0.0001,
    })

    if err != nil {
        log.Printf("Error: %v", err)
    } else {
        log.Println("Event sent successfully!")
    }
}
```

## Environment Variables

Instead of hardcoding configuration, use environment variables:

```bash
export SENTINEL_BROKERS=kafka-1:9092,kafka-2:9092
export SENTINEL_TOPIC=llm.telemetry
export SENTINEL_COMPRESSION=snappy
```

Then create client from environment:

```go
client, err := sentinel.NewClientFromEnv()
```

## Next Steps

- Read the [full README](README.md) for advanced features
- Check out [examples](examples/) for more use cases
- Learn about [error handling](examples/error_handling.go)
- Explore [batch sends](README.md#batch-sends) for high throughput
- Enable [distributed tracing](README.md#distributed-tracing)

## Common Issues

### Cannot connect to Kafka

Make sure Kafka is running:
```bash
docker-compose up -d kafka
```

### Validation errors

Ensure all required fields are set:
- `Service` (service name)
- `Model` (model identifier)
- `Prompt` (text and token count > 0)
- `Response` (text, token count > 0, and finish reason)
- `LatencyMs` (>= 0)
- `CostUsd` (>= 0)

## Support

- **Documentation**: https://github.com/llm-sentinel/sentinel
- **Issues**: https://github.com/llm-sentinel/sentinel/issues
