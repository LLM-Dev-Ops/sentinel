/**
 * Example using the fluent TelemetryEventBuilder
 */

import { SentinelClient, TelemetryEventBuilder } from '../src';

async function main() {
  const client = new SentinelClient({
    kafka: {
      brokers: ['localhost:9092'],
    },
    debug: true,
  });

  console.log('Building and sending telemetry event with fluent builder...');

  try {
    // Build event with fluent API
    const event = new TelemetryEventBuilder()
      .service('recommendation-engine')
      .model('claude-3-sonnet')
      .prompt({
        text: 'Recommend 5 books similar to "1984" by George Orwell',
        tokens: 15,
      })
      .response({
        text: 'Based on your interest in "1984", here are 5 similar books:\n1. Brave New World by Aldous Huxley\n2. Fahrenheit 451 by Ray Bradbury\n3. The Handmaid\'s Tale by Margaret Atwood\n4. Animal Farm by George Orwell\n5. We by Yevgeny Zamyatin',
        tokens: 120,
        finishReason: 'stop',
      })
      .latency(312.7)
      .cost(0.0012)
      .traceId('trace-abc123')
      .spanId('span-xyz789')
      .addMetadata('region', 'eu-west-1')
      .addMetadata('experiment', 'v2-recommendations')
      .addMetadata('user_tier', 'premium')
      .build();

    console.log('Event built:', {
      id: event.event_id,
      service: event.service_name,
      model: event.model,
    });

    // Send the event
    const metadata = await client.send(event);
    console.log('Event sent to partition:', metadata[0].partition);
    console.log('Event sent successfully!');
  } catch (error) {
    console.error('Failed to send event:', error);
  } finally {
    await client.disconnect();
    console.log('Disconnected from Kafka');
  }
}

main().catch(console.error);
