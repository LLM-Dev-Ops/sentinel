# LLM-Sentinel SDK - Quick Reference

## Installation

```bash
npm install @llm-sentinel/sdk
```

## Basic Usage

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: { brokers: ['localhost:9092'] }
});

await client.track({
  service: 'my-service',
  model: 'gpt-4',
  prompt: 'Hello',
  response: 'Hi',
  latencyMs: 150,
  promptTokens: 5,
  completionTokens: 10,
  costUsd: 0.001
});

await client.disconnect();
```

## Client Methods

| Method | Description |
|--------|-------------|
| `connect()` | Manually connect to Kafka |
| `disconnect()` | Graceful disconnect |
| `send(event)` | Send telemetry event |
| `sendBatch(events)` | Send multiple events |
| `track(options)` | Convenient tracking |
| `flush()` | Flush and disconnect |
| `isConnected()` | Check connection |

## Track Options

```typescript
{
  service: string;          // Required
  model: string;            // Required
  prompt: string;           // Required
  response: string;         // Required
  latencyMs: number;        // Required
  promptTokens: number;     // Required
  completionTokens: number; // Required
  costUsd: number;          // Required
  traceId?: string;
  spanId?: string;
  finishReason?: string;
  metadata?: Record<string, string>;
  errors?: string[];
  promptEmbedding?: number[];
  responseEmbedding?: number[];
}
```

## Configuration

```typescript
{
  kafka: {
    brokers: string[];      // Required
    clientId?: string;
    ssl?: boolean | object;
    sasl?: {
      mechanism: 'plain' | 'scram-sha-256' | 'scram-sha-512';
      username: string;
      password: string;
    };
  },
  topic?: string;           // Default: 'llm.telemetry'
  autoConnect?: boolean;    // Default: true
  debug?: boolean;          // Default: false
  compression?: 'none' | 'gzip' | 'snappy' | 'lz4';  // Default: 'gzip'
  acks?: 0 | 1 | -1;       // Default: -1
}
```

## Environment Variables

```bash
SENTINEL_KAFKA_BROKERS=localhost:9092
SENTINEL_KAFKA_TOPIC=llm.telemetry
SENTINEL_KAFKA_CLIENT_ID=my-app
SENTINEL_KAFKA_USERNAME=user
SENTINEL_KAFKA_PASSWORD=pass
SENTINEL_KAFKA_SSL=true
SENTINEL_DEBUG=true
```

## Builder Pattern

```typescript
import { TelemetryEventBuilder } from '@llm-sentinel/sdk';

const event = new TelemetryEventBuilder()
  .service('my-service')
  .model('gpt-4')
  .prompt({ text: 'Hello', tokens: 5 })
  .response({ text: 'Hi', tokens: 10, finishReason: 'stop' })
  .latency(150)
  .cost(0.001)
  .traceId('trace-123')
  .addMetadata('region', 'us-east-1')
  .build();
```

## Error Handling

```typescript
import {
  ValidationError,
  ConnectionError,
  SendError,
} from '@llm-sentinel/sdk';

try {
  await client.track({ /* ... */ });
} catch (error) {
  if (error instanceof ValidationError) {
    console.error('Invalid event:', error.field, error.message);
  } else if (error instanceof ConnectionError) {
    console.error('Connection failed:', error.message);
  } else if (error instanceof SendError) {
    console.error('Send failed:', error.message);
    if (error.isRetryable) {
      // Retry logic
    }
  }
}
```

## Common Patterns

### Singleton Client

```typescript
// client.ts
export const sentinelClient = new SentinelClient({
  kafka: { brokers: process.env.KAFKA_BROKERS?.split(',') || ['localhost:9092'] }
});

// usage.ts
import { sentinelClient } from './client';
await sentinelClient.track({ /* ... */ });
```

### Wrapper Function

```typescript
async function trackedLLMCall(prompt: string) {
  const start = Date.now();
  try {
    const response = await callLLM(prompt);
    await client.track({
      service: 'my-service',
      model: 'gpt-4',
      prompt,
      response,
      latencyMs: Date.now() - start,
      // ...
    });
    return response;
  } catch (error) {
    await client.track({
      // ... track error
      errors: [error.message]
    });
    throw error;
  }
}
```

### Fire-and-Forget

```typescript
// Don't await if you don't want to block
client.track({ /* ... */ }).catch(console.error);
```

## Batch Sending

```typescript
const events = [
  createTelemetryEvent({ /* ... */ }),
  createTelemetryEvent({ /* ... */ }),
];

await client.sendBatch(events);
```

## Imports

```typescript
// Main client
import { SentinelClient } from '@llm-sentinel/sdk';

// Types
import type { TelemetryEvent, TrackOptions } from '@llm-sentinel/sdk';

// Enums
import { Severity, AnomalyType } from '@llm-sentinel/sdk';

// Builders
import { TelemetryEventBuilder, createTelemetryEvent } from '@llm-sentinel/sdk';

// Errors
import { ValidationError, SendError } from '@llm-sentinel/sdk';
```

## Type Definitions

```typescript
interface TelemetryEvent {
  event_id: string;
  timestamp: string;
  service_name: string;
  trace_id?: string;
  span_id?: string;
  model: string;
  prompt: PromptInfo;
  response: ResponseInfo;
  latency_ms: number;
  cost_usd: number;
  metadata: Record<string, string>;
  errors: string[];
}
```

## Validation Rules

- `latency_ms` >= 0
- `cost_usd` >= 0
- `tokens` >= 0 (integers)
- `text` max length: 100,000 chars
- `event_id` must be UUID v4
- `timestamp` must be ISO 8601
- All required fields must be present

## Resources

- **Full Docs**: [README.md](./README.md)
- **Getting Started**: [GETTING_STARTED.md](./GETTING_STARTED.md)
- **Examples**: [examples/](./examples)
- **Changelog**: [CHANGELOG.md](./CHANGELOG.md)
