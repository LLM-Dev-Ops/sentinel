# LLM-Sentinel TypeScript/JavaScript SDK

[![npm version](https://badge.fury.io/js/@llm-sentinel%2Fsdk.svg)](https://www.npmjs.com/package/@llm-sentinel/sdk)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Production-ready TypeScript/JavaScript SDK for sending LLM telemetry events to **LLM-Sentinel** for real-time anomaly detection and monitoring.

## Features

- **Type-Safe**: Full TypeScript support with comprehensive type definitions
- **Promise-Based**: Modern async/await API
- **Kafka Integration**: Built on KafkaJS for reliable message delivery
- **Connection Management**: Automatic reconnection with exponential backoff
- **Validation**: Client-side validation before sending events
- **Fluent Builders**: Convenient builder pattern for constructing events
- **Error Handling**: Rich error types with detailed context
- **Configuration**: Environment variable support and flexible configuration
- **Production Ready**: Compression, batching, and graceful shutdown

## Installation

```bash
npm install @llm-sentinel/sdk
```

Or using yarn:

```bash
yarn add @llm-sentinel/sdk
```

Or using pnpm:

```bash
pnpm add @llm-sentinel/sdk
```

## Quick Start

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

// Create a client
const client = new SentinelClient({
  kafka: {
    brokers: ['localhost:9092'],
  },
});

// Track an LLM request
await client.track({
  service: 'my-chatbot',
  model: 'gpt-4',
  prompt: 'What is the capital of France?',
  response: 'The capital of France is Paris.',
  latencyMs: 245.5,
  promptTokens: 8,
  completionTokens: 9,
  costUsd: 0.00045,
});

// Gracefully disconnect
await client.disconnect();
```

## Usage Examples

### Basic Usage with Simple API

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: {
    brokers: ['localhost:9092'],
  },
  topic: 'llm.telemetry', // Optional, defaults to 'llm.telemetry'
});

// Track a simple event
await client.track({
  service: 'customer-support-bot',
  model: 'gpt-4',
  prompt: 'How do I reset my password?',
  response: 'To reset your password, click on...',
  latencyMs: 189.3,
  promptTokens: 12,
  completionTokens: 45,
  costUsd: 0.00089,
  traceId: 'trace-12345', // Optional
  spanId: 'span-67890', // Optional
  metadata: {
    region: 'us-east-1',
    userId: 'user-123',
  },
});
```

### Using the Fluent Builder

```typescript
import { TelemetryEventBuilder, SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: { brokers: ['localhost:9092'] },
});

// Build event with fluent API
const event = new TelemetryEventBuilder()
  .service('recommendation-engine')
  .model('claude-3')
  .prompt({
    text: 'Recommend books similar to 1984',
    tokens: 15,
  })
  .response({
    text: 'Based on your interest in 1984, I recommend...',
    tokens: 120,
    finishReason: 'stop',
  })
  .latency(312.7)
  .cost(0.0012)
  .traceId('trace-abc123')
  .addMetadata('region', 'eu-west-1')
  .addMetadata('experiment', 'v2-recommendations')
  .build();

// Send the event
await client.send(event);
```

### Batch Sending

```typescript
import { createTelemetryEvent, SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: { brokers: ['localhost:9092'] },
});

const events = [
  createTelemetryEvent({
    service: 'summarizer',
    model: 'gpt-3.5-turbo',
    prompt: 'Summarize this article...',
    promptTokens: 2500,
    response: 'The article discusses...',
    responseTokens: 150,
    latencyMs: 1234.5,
    costUsd: 0.0035,
  }),
  createTelemetryEvent({
    service: 'summarizer',
    model: 'gpt-3.5-turbo',
    prompt: 'Summarize another article...',
    promptTokens: 3200,
    response: 'This piece covers...',
    responseTokens: 180,
    latencyMs: 1456.2,
    costUsd: 0.0042,
  }),
];

// Send multiple events in a batch
await client.sendBatch(events);
```

### Connection Management

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: { brokers: ['localhost:9092'] },
  autoConnect: false, // Disable auto-connect
});

// Manually connect
await client.connect();

// Check connection status
console.log('Connected:', client.isConnected());

// Send events...
await client.track({ /* ... */ });

// Gracefully disconnect
await client.disconnect();
```

### Error Handling

```typescript
import {
  SentinelClient,
  ValidationError,
  ConnectionError,
  SendError,
} from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: { brokers: ['localhost:9092'] },
});

try {
  await client.track({
    service: 'my-service',
    model: 'gpt-4',
    prompt: 'Hello',
    response: 'Hi',
    latencyMs: -100, // Invalid: negative latency
    promptTokens: 5,
    completionTokens: 10,
    costUsd: 0.001,
  });
} catch (error) {
  if (error instanceof ValidationError) {
    console.error('Validation failed:', error.field, error.message);
  } else if (error instanceof ConnectionError) {
    console.error('Connection error:', error.message, error.details);
  } else if (error instanceof SendError) {
    console.error('Send failed:', error.message);
    if (error.isRetryable) {
      console.log('Error is retriable');
    }
  }
}
```

### With Distributed Tracing

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';
import { trace, context } from '@opentelemetry/api';

const client = new SentinelClient({
  kafka: { brokers: ['localhost:9092'] },
});

// In your traced function
const span = trace.getSpan(context.active());

await client.track({
  service: 'my-service',
  model: 'gpt-4',
  prompt: 'What is machine learning?',
  response: 'Machine learning is...',
  latencyMs: 234.5,
  promptTokens: 15,
  completionTokens: 80,
  costUsd: 0.0008,
  traceId: span?.spanContext().traceId,
  spanId: span?.spanContext().spanId,
});
```

### Error Tracking

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: { brokers: ['localhost:9092'] },
});

// Track an LLM request with errors
await client.track({
  service: 'translation-service',
  model: 'gpt-4',
  prompt: 'Translate to French: Hello',
  response: '', // Empty response due to error
  latencyMs: 5432.1, // High latency due to timeout
  promptTokens: 10,
  completionTokens: 0,
  costUsd: 0,
  errors: ['Request timeout', 'Retry limit exceeded'],
  finishReason: 'error',
});
```

### With Embeddings

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: { brokers: ['localhost:9092'] },
});

// Include embeddings for drift detection
await client.track({
  service: 'semantic-search',
  model: 'text-embedding-ada-002',
  prompt: 'machine learning algorithms',
  response: '', // Embedding models don't produce text
  latencyMs: 123.4,
  promptTokens: 10,
  completionTokens: 0,
  costUsd: 0.00001,
  finishReason: 'stop',
  promptEmbedding: [0.123, -0.456, 0.789, /* ... */],
  responseEmbedding: [0.234, -0.567, 0.890, /* ... */],
});
```

## Configuration

### Environment Variables

The SDK supports configuration via environment variables:

```bash
# Kafka Configuration
SENTINEL_KAFKA_BROKERS=localhost:9092,broker2:9092
SENTINEL_KAFKA_TOPIC=llm.telemetry
SENTINEL_KAFKA_CLIENT_ID=my-app

# Authentication (optional)
SENTINEL_KAFKA_USERNAME=my-user
SENTINEL_KAFKA_PASSWORD=my-password
SENTINEL_KAFKA_SSL=true

# Debug Mode
SENTINEL_DEBUG=true
```

### Configuration Options

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  // Kafka configuration (required)
  kafka: {
    brokers: ['localhost:9092', 'broker2:9092'],
    clientId: 'my-service',

    // SSL/TLS (optional)
    ssl: true,
    // Or with custom certificates
    ssl: {
      rejectUnauthorized: true,
      ca: ['-----BEGIN CERTIFICATE-----\n...'],
      cert: '-----BEGIN CERTIFICATE-----\n...',
      key: '-----BEGIN PRIVATE KEY-----\n...',
    },

    // SASL Authentication (optional)
    sasl: {
      mechanism: 'plain', // or 'scram-sha-256', 'scram-sha-512'
      username: 'my-user',
      password: 'my-password',
    },

    // Connection settings
    requestTimeout: 30000,
    connectionTimeout: 10000,

    // Retry configuration
    retry: {
      initialRetryTime: 100,
      retries: 8,
      maxRetryTime: 30000,
      multiplier: 2,
      factor: 0.2,
    },
  },

  // Topic (optional, defaults to 'llm.telemetry')
  topic: 'llm.telemetry',

  // Auto-connect on first send (optional, defaults to true)
  autoConnect: true,

  // Debug logging (optional, defaults to false)
  debug: false,

  // Compression (optional, defaults to 'gzip')
  compression: 'gzip', // 'none', 'gzip', 'snappy', 'lz4'

  // Acknowledgments (optional, defaults to -1/all)
  acks: -1, // 0 (none), 1 (leader), -1 (all in-sync replicas)

  // Request timeout (optional, defaults to 30000ms)
  timeout: 30000,
});
```

## API Reference

### SentinelClient

#### Constructor

```typescript
new SentinelClient(config: Partial<SentinelConfig>)
```

Creates a new Sentinel client instance.

#### Methods

##### `connect(): Promise<void>`

Manually connect to Kafka brokers. Not required if `autoConnect` is enabled.

##### `disconnect(): Promise<void>`

Gracefully disconnect from Kafka, flushing any pending messages.

##### `isConnected(): boolean`

Check if the client is currently connected.

##### `track(options: TrackOptions): Promise<RecordMetadata[]>`

Convenient method to track a simple telemetry event.

##### `send(event: TelemetryEvent): Promise<RecordMetadata[]>`

Send a telemetry event to Sentinel.

##### `sendBatch(events: TelemetryEvent[]): Promise<RecordMetadata[]>`

Send multiple telemetry events in a batch.

##### `flush(timeout?: number): Promise<void>`

Flush pending messages and disconnect.

##### `getConfig(): Readonly<SentinelConfig>`

Get the current configuration.

### TelemetryEventBuilder

Fluent builder for constructing telemetry events.

```typescript
new TelemetryEventBuilder()
  .eventId(string)
  .timestamp(string | Date)
  .service(string)
  .traceId(string)
  .spanId(string)
  .model(string)
  .prompt(PromptInfo)
  .response(ResponseInfo)
  .latency(number)
  .cost(number)
  .addMetadata(key, value)
  .metadata(object)
  .addError(string)
  .errors(string[])
  .build()
```

### Error Classes

- **`SentinelError`**: Base error class
- **`ValidationError`**: Thrown when event validation fails
- **`ConnectionError`**: Thrown when connection to Kafka fails
- **`SendError`**: Thrown when sending events fails
- **`ConfigurationError`**: Thrown when configuration is invalid

## TypeScript Support

This SDK is written in TypeScript and provides comprehensive type definitions:

```typescript
import {
  TelemetryEvent,
  PromptInfo,
  ResponseInfo,
  AnomalyEvent,
  Severity,
  AnomalyType,
  DetectionMethod,
} from '@llm-sentinel/sdk';

// All types are fully typed
const event: TelemetryEvent = {
  event_id: '...',
  timestamp: new Date().toISOString(),
  service_name: 'my-service',
  model: 'gpt-4',
  prompt: {
    text: 'Hello',
    tokens: 5,
  },
  response: {
    text: 'Hi',
    tokens: 10,
    finish_reason: 'stop',
  },
  latency_ms: 150.5,
  cost_usd: 0.001,
  metadata: {},
  errors: [],
};
```

## Testing

To run tests:

```bash
npm test
```

To run tests with coverage:

```bash
npm run test:coverage
```

## Development

```bash
# Install dependencies
npm install

# Build the project
npm run build

# Watch mode for development
npm run build:watch

# Run linter
npm run lint

# Format code
npm run format

# Type check
npm run typecheck
```

## Best Practices

1. **Reuse Client Instances**: Create one client instance and reuse it across your application
2. **Graceful Shutdown**: Always call `disconnect()` when shutting down
3. **Error Handling**: Implement proper error handling for production reliability
4. **Batching**: Use `sendBatch()` for high-throughput scenarios
5. **Metadata**: Include relevant metadata (region, user_id, etc.) for better analysis
6. **Tracing**: Include trace_id and span_id for correlation with distributed traces
7. **Validation**: The SDK validates events client-side, but ensure your data is accurate

## Performance Considerations

- Events are sent asynchronously via Kafka
- Connection pooling is handled automatically by KafkaJS
- Compression (gzip by default) reduces network bandwidth
- Batch sending reduces per-message overhead
- Automatic retry with exponential backoff for transient failures

## Troubleshooting

### Connection Issues

If you're having trouble connecting to Kafka:

1. Verify broker addresses are correct
2. Check network connectivity
3. Ensure Kafka is running and accessible
4. Enable debug mode: `debug: true` in configuration
5. Check SSL/SASL configuration if using authentication

### Validation Errors

If events fail validation:

1. Check that all required fields are provided
2. Ensure numeric fields (latency_ms, cost_usd, tokens) are >= 0
3. Verify text fields don't exceed 100,000 characters
4. Ensure event_id is a valid UUID v4
5. Confirm timestamp is valid ISO 8601 format

## License

MIT License - see LICENSE file for details

## Contributing

Contributions are welcome! Please see CONTRIBUTING.md for guidelines.

## Support

- GitHub Issues: https://github.com/yourusername/llm-sentinel/issues
- Documentation: https://github.com/yourusername/llm-sentinel
- Email: support@llm-sentinel.io

## Related Projects

- [LLM-Sentinel Core](https://github.com/yourusername/llm-sentinel) - The main Sentinel anomaly detection system
- [Sentinel Python SDK](../python) - Python SDK for Sentinel
- [Sentinel Go SDK](../go) - Go SDK for Sentinel
