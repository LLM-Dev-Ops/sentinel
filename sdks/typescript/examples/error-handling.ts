/**
 * Example demonstrating error handling
 */

import {
  SentinelClient,
  ValidationError,
  ConnectionError,
  SendError,
} from '../src';

async function main() {
  const client = new SentinelClient({
    kafka: {
      brokers: ['localhost:9092'],
    },
    debug: true,
  });

  // Example 1: Validation Error
  console.log('\n=== Example 1: Validation Error ===');
  try {
    await client.track({
      service: 'test-service',
      model: 'gpt-4',
      prompt: 'Hello',
      response: 'Hi',
      latencyMs: -100, // Invalid: negative latency!
      promptTokens: 5,
      completionTokens: 10,
      costUsd: 0.001,
    });
  } catch (error) {
    if (error instanceof ValidationError) {
      console.error('Validation Error Details:');
      console.error('  Field:', error.field);
      console.error('  Message:', error.message);
      console.error('  Code:', error.code);
      console.error('  Details:', error.details);
    }
  }

  // Example 2: Successful Send
  console.log('\n=== Example 2: Successful Send ===');
  try {
    const result = await client.track({
      service: 'test-service',
      model: 'gpt-4',
      prompt: 'What is TypeScript?',
      response: 'TypeScript is a typed superset of JavaScript...',
      latencyMs: 234.5,
      promptTokens: 10,
      completionTokens: 50,
      costUsd: 0.0008,
    });
    console.log('Success! Message sent to partition:', result[0].partition);
  } catch (error) {
    if (error instanceof SendError) {
      console.error('Send Error:', error.message);
      console.error('  Event ID:', error.eventId);
      console.error('  Retryable:', error.isRetryable);
    } else if (error instanceof ConnectionError) {
      console.error('Connection Error:', error.message);
      console.error('  Details:', error.details);
    } else {
      console.error('Unexpected error:', error);
    }
  }

  // Example 3: Handling Errors in Events
  console.log('\n=== Example 3: Tracking LLM Request with Errors ===');
  try {
    await client.track({
      service: 'translation-service',
      model: 'gpt-4',
      prompt: 'Translate to French: Hello world',
      response: '', // Empty due to error
      latencyMs: 5000, // High latency due to timeout
      promptTokens: 10,
      completionTokens: 0,
      costUsd: 0,
      errors: [
        'Request timeout after 5000ms',
        'Retry limit exceeded (3 attempts)',
      ],
      finishReason: 'error',
      metadata: {
        error_type: 'timeout',
        attempts: '3',
      },
    });
    console.log('Successfully tracked failed LLM request');
  } catch (error) {
    console.error('Failed to track error event:', error);
  }

  // Cleanup
  await client.disconnect();
  console.log('\n=== Disconnected ===');
}

main().catch(console.error);
