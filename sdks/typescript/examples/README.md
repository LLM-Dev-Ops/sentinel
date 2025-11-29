# LLM-Sentinel SDK Examples

This directory contains example code demonstrating various features of the LLM-Sentinel TypeScript SDK.

## Prerequisites

1. Install dependencies:
   ```bash
   cd /workspaces/sentinel/sdks/typescript
   npm install
   ```

2. Build the SDK:
   ```bash
   npm run build
   ```

3. Ensure Kafka is running:
   ```bash
   # Using Docker
   docker run -d --name kafka -p 9092:9092 apache/kafka:latest
   ```

## Running Examples

### Basic Usage

Simple example showing how to track an LLM request:

```bash
npx ts-node examples/basic.ts
```

### Fluent Builder

Example using the TelemetryEventBuilder for constructing complex events:

```bash
npx ts-node examples/builder.ts
```

### Batch Sending

Demonstrates sending multiple events in a batch for better performance:

```bash
npx ts-node examples/batch.ts
```

### Error Handling

Shows how to handle various error types and track failed LLM requests:

```bash
npx ts-node examples/error-handling.ts
```

## Example Descriptions

### basic.ts
- Simple client creation
- Basic event tracking with `track()` method
- Graceful disconnect
- Good starting point for beginners

### builder.ts
- Using `TelemetryEventBuilder` for fluent event construction
- Adding metadata with chaining
- Including trace and span IDs
- More control over event creation

### batch.ts
- Creating multiple events
- Batch sending for high-throughput scenarios
- Inspecting metadata from batch sends
- Reducing per-message overhead

### error-handling.ts
- Handling ValidationError, ConnectionError, SendError
- Tracking failed LLM requests with errors
- Accessing error details
- Best practices for production error handling

## Modifying Examples

Feel free to modify these examples to experiment with different configurations:

- Change Kafka broker addresses
- Adjust compression settings
- Try different retry configurations
- Test with SASL/SSL authentication
- Add custom metadata
- Include embeddings for drift detection

## Integration with Your Application

These examples can serve as templates for integrating Sentinel into your LLM application:

1. **API Gateway**: Track requests at the gateway level
2. **Microservices**: Add to individual LLM-consuming services
3. **Background Jobs**: Monitor batch processing jobs
4. **Testing**: Track test suite LLM calls
5. **Local Development**: Monitor development environment usage

## Troubleshooting

If examples fail to run:

1. Check Kafka is accessible at `localhost:9092`
2. Verify the topic exists (or enable auto-creation in Kafka)
3. Check network connectivity
4. Review debug logs (set `debug: true` in configuration)
5. Ensure you've built the SDK with `npm run build`

## Additional Resources

- [Main README](../README.md) - Full SDK documentation
- [API Reference](../README.md#api-reference) - Detailed API docs
- [Configuration Guide](../README.md#configuration) - Configuration options
