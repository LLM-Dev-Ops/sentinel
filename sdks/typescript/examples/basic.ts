/**
 * Basic usage example for LLM-Sentinel SDK
 */

import { SentinelClient } from '../src';

async function main() {
  // Create a client with Kafka configuration
  const client = new SentinelClient({
    kafka: {
      brokers: ['localhost:9092'],
    },
    topic: 'llm.telemetry',
    debug: true,
  });

  console.log('Sending telemetry event...');

  try {
    // Track a simple LLM request
    await client.track({
      service: 'example-chatbot',
      model: 'gpt-4',
      prompt: 'What is the capital of France?',
      response: 'The capital of France is Paris.',
      latencyMs: 245.5,
      promptTokens: 8,
      completionTokens: 9,
      costUsd: 0.00045,
      metadata: {
        environment: 'development',
        version: '1.0.0',
      },
    });

    console.log('Event sent successfully!');
  } catch (error) {
    console.error('Failed to send event:', error);
  } finally {
    // Always disconnect gracefully
    await client.disconnect();
    console.log('Disconnected from Kafka');
  }
}

// Run the example
main().catch(console.error);
