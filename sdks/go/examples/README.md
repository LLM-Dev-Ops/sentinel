# Sentinel Go SDK Examples

This directory contains example code demonstrating various features of the Sentinel Go SDK.

## Prerequisites

1. Ensure Kafka is running:
```bash
docker-compose up -d kafka
```

2. Install dependencies:
```bash
cd /workspaces/sentinel/sdks/go
go mod download
```

## Running Examples

### Basic Usage

Demonstrates the core SDK features including:
- Simple Track API
- Fluent Builder pattern
- Distributed tracing
- Batch sends
- Timeouts

```bash
go run examples/basic.go
```

### Error Handling

Shows comprehensive error handling patterns:
- Validation errors
- Send errors
- Connection errors
- Recording application errors in events

```bash
go run examples/error_handling.go
```

## Example Output

When running the basic example, you should see output similar to:

```
Example 1: Simple Track API
Event sent successfully!

Example 2: Builder Pattern
Event sent successfully!

Example 3: With Distributed Tracing
Event sent successfully!

Example 4: Batch Send
Batch of 3 events sent successfully!

Example 5: With Timeout
Event sent successfully!

Client Statistics:
  Messages written: 6
  Messages sent: 6
  Bytes written: 2048
  Errors: 0

All examples completed!
```

## Creating Your Own Examples

To create a new example:

1. Create a new `.go` file in this directory
2. Use `package main` and include a `main()` function
3. Import the SDK: `import "github.com/llm-sentinel/sentinel-sdk-go/sentinel"`
4. Follow the patterns shown in the existing examples

Example template:

```go
package main

import (
    "context"
    "log"

    "github.com/llm-sentinel/sentinel-sdk-go/sentinel"
)

func main() {
    client, err := sentinel.NewClient(
        sentinel.WithBrokers([]string{"localhost:9092"}),
    )
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    // Your code here
}
```

## Troubleshooting

### Cannot connect to Kafka

If you see connection errors:
1. Check if Kafka is running: `docker ps | grep kafka`
2. Verify the broker address matches your setup
3. Check network connectivity to the broker

### Validation errors

If events fail validation:
1. Check required fields are set (service, model, prompt, response)
2. Ensure numeric fields are >= 0
3. Verify text lengths don't exceed 100,000 characters

### Send timeouts

If sends are timing out:
1. Increase the write timeout: `sentinel.WithWriteTimeout(30*time.Second)`
2. Check Kafka broker health
3. Reduce batch sizes if using batching
