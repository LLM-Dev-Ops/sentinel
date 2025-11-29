# Getting Started with LLM-Sentinel TypeScript SDK

This guide will help you get up and running with the LLM-Sentinel TypeScript SDK in minutes.

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Basic Concepts](#basic-concepts)
4. [Common Patterns](#common-patterns)
5. [Next Steps](#next-steps)

## Installation

Install the SDK using npm, yarn, or pnpm:

```bash
# Using npm
npm install @llm-sentinel/sdk

# Using yarn
yarn add @llm-sentinel/sdk

# Using pnpm
pnpm add @llm-sentinel/sdk
```

## Quick Start

### Step 1: Create a Client

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

const client = new SentinelClient({
  kafka: {
    brokers: ['localhost:9092'],
  },
});
```

### Step 2: Track an LLM Request

```typescript
await client.track({
  service: 'my-chatbot',
  model: 'gpt-4',
  prompt: 'What is machine learning?',
  response: 'Machine learning is a subset of artificial intelligence...',
  latencyMs: 234.5,
  promptTokens: 10,
  completionTokens: 50,
  costUsd: 0.0008,
});
```

### Step 3: Disconnect When Done

```typescript
await client.disconnect();
```

### Complete Example

```typescript
import { SentinelClient } from '@llm-sentinel/sdk';

async function main() {
  // Create client
  const client = new SentinelClient({
    kafka: {
      brokers: ['localhost:9092'],
    },
  });

  try {
    // Track LLM request
    await client.track({
      service: 'my-chatbot',
      model: 'gpt-4',
      prompt: 'What is machine learning?',
      response: 'Machine learning is a subset of AI...',
      latencyMs: 234.5,
      promptTokens: 10,
      completionTokens: 50,
      costUsd: 0.0008,
    });

    console.log('Event sent successfully!');
  } catch (error) {
    console.error('Failed to send event:', error);
  } finally {
    await client.disconnect();
  }
}

main();
```

## Basic Concepts

### TelemetryEvent

A telemetry event represents a single LLM request/response interaction:

```typescript
{
  event_id: 'uuid-v4',           // Auto-generated
  timestamp: '2024-01-15T10:30:00Z', // Auto-generated
  service_name: 'my-service',    // Your service identifier
  model: 'gpt-4',                // LLM model used
  prompt: {
    text: 'Hello',
    tokens: 5
  },
  response: {
    text: 'Hi there!',
    tokens: 10,
    finish_reason: 'stop'
  },
  latency_ms: 150.5,             // Request latency
  cost_usd: 0.001,               // Request cost
  metadata: {},                  // Custom metadata
  errors: []                     // Error messages
}
```

### Client Configuration

Configure the client with Kafka settings:

```typescript
const client = new SentinelClient({
  kafka: {
    brokers: ['localhost:9092'],    // Kafka brokers
    clientId: 'my-app',             // Optional client ID
  },
  topic: 'llm.telemetry',           // Optional topic (default: llm.telemetry)
  autoConnect: true,                // Auto-connect on first send (default: true)
  debug: false,                     // Debug logging (default: false)
});
```

## Common Patterns

### Pattern 1: Singleton Client

Create one client instance and reuse it:

```typescript
// client.ts
import { SentinelClient } from '@llm-sentinel/sdk';

export const sentinelClient = new SentinelClient({
  kafka: {
    brokers: process.env.KAFKA_BROKERS?.split(',') || ['localhost:9092'],
  },
});

// usage.ts
import { sentinelClient } from './client';

await sentinelClient.track({
  service: 'my-service',
  model: 'gpt-4',
  // ... other fields
});
```

### Pattern 2: Wrapper Function

Create a wrapper around your LLM calls:

```typescript
import { sentinelClient } from './client';
import OpenAI from 'openai';

const openai = new OpenAI();

async function trackedCompletion(prompt: string) {
  const startTime = Date.now();

  try {
    const completion = await openai.chat.completions.create({
      model: 'gpt-4',
      messages: [{ role: 'user', content: prompt }],
    });

    const latency = Date.now() - startTime;
    const response = completion.choices[0].message.content || '';

    // Track successful request
    await sentinelClient.track({
      service: 'my-chatbot',
      model: 'gpt-4',
      prompt,
      response,
      latencyMs: latency,
      promptTokens: completion.usage?.prompt_tokens || 0,
      completionTokens: completion.usage?.completion_tokens || 0,
      costUsd: calculateCost(completion.usage),
      finishReason: completion.choices[0].finish_reason,
    });

    return response;
  } catch (error) {
    const latency = Date.now() - startTime;

    // Track failed request
    await sentinelClient.track({
      service: 'my-chatbot',
      model: 'gpt-4',
      prompt,
      response: '',
      latencyMs: latency,
      promptTokens: 0,
      completionTokens: 0,
      costUsd: 0,
      errors: [error.message],
      finishReason: 'error',
    });

    throw error;
  }
}

function calculateCost(usage: any): number {
  // GPT-4 pricing (example)
  const promptCost = (usage?.prompt_tokens || 0) * 0.00003;
  const completionCost = (usage?.completion_tokens || 0) * 0.00006;
  return promptCost + completionCost;
}
```

### Pattern 3: Middleware/Decorator

Use as middleware in your application:

```typescript
import { sentinelClient } from './client';

function trackLLM() {
  return function (
    target: any,
    propertyKey: string,
    descriptor: PropertyDescriptor
  ) {
    const originalMethod = descriptor.value;

    descriptor.value = async function (...args: any[]) {
      const startTime = Date.now();

      try {
        const result = await originalMethod.apply(this, args);
        const latency = Date.now() - startTime;

        // Track after successful completion
        await sentinelClient.track({
          service: 'my-service',
          model: this.model || 'unknown',
          prompt: args[0],
          response: result,
          latencyMs: latency,
          // ... other fields
        });

        return result;
      } catch (error) {
        const latency = Date.now() - startTime;

        await sentinelClient.track({
          service: 'my-service',
          model: this.model || 'unknown',
          prompt: args[0],
          response: '',
          latencyMs: latency,
          errors: [error.message],
          // ... other fields
        });

        throw error;
      }
    };

    return descriptor;
  };
}

class ChatService {
  model = 'gpt-4';

  @trackLLM()
  async generateResponse(prompt: string): Promise<string> {
    // Your LLM call here
    return 'response';
  }
}
```

### Pattern 4: Express Middleware

Integrate with Express.js:

```typescript
import express from 'express';
import { sentinelClient } from './client';

const app = express();

app.post('/chat', async (req, res) => {
  const { message } = req.body;
  const startTime = Date.now();

  try {
    // Your LLM call
    const response = await callLLM(message);
    const latency = Date.now() - startTime;

    // Track in background (don't await)
    sentinelClient.track({
      service: 'chat-api',
      model: 'gpt-4',
      prompt: message,
      response,
      latencyMs: latency,
      // ... other fields
      metadata: {
        userId: req.user?.id,
        endpoint: '/chat',
      },
    }).catch(err => {
      console.error('Failed to track telemetry:', err);
    });

    res.json({ response });
  } catch (error) {
    const latency = Date.now() - startTime;

    // Track error
    sentinelClient.track({
      service: 'chat-api',
      model: 'gpt-4',
      prompt: message,
      response: '',
      latencyMs: latency,
      errors: [error.message],
      // ... other fields
    }).catch(console.error);

    res.status(500).json({ error: error.message });
  }
});
```

## Next Steps

1. **Explore Examples**: Check out the [examples](./examples) directory for more use cases
2. **Read Full Documentation**: See [README.md](./README.md) for complete API reference
3. **Configure for Production**: Set up SSL/SASL, adjust retry settings, enable compression
4. **Add Metadata**: Include relevant metadata (user IDs, regions, experiments) for better analysis
5. **Set Up Monitoring**: Use the Sentinel dashboard to view detected anomalies
6. **Integrate with Tracing**: Add trace_id and span_id for correlation with distributed traces

## Support

- **Issues**: https://github.com/yourusername/llm-sentinel/issues
- **Documentation**: https://github.com/yourusername/llm-sentinel
- **Examples**: [./examples](./examples)

## Tips

- **Reuse Clients**: Create one client and reuse it across your app
- **Don't Block**: Use fire-and-forget pattern for tracking (catch errors separately)
- **Add Context**: Include metadata like user_id, region, experiment_id
- **Track Errors**: Always track failed LLM requests for complete visibility
- **Use Batching**: For high-volume scenarios, use `sendBatch()`
- **Enable Compression**: Use gzip compression to reduce network bandwidth
- **Monitor Metrics**: Check Sentinel dashboard for anomaly alerts
