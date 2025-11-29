/**
 * Example of batch sending telemetry events
 */

import { SentinelClient, createTelemetryEvent } from '../src';

async function main() {
  const client = new SentinelClient({
    kafka: {
      brokers: ['localhost:9092'],
    },
    debug: true,
  });

  console.log('Sending batch of telemetry events...');

  try {
    // Create multiple events
    const events = [
      createTelemetryEvent({
        service: 'document-summarizer',
        model: 'gpt-3.5-turbo',
        prompt: 'Summarize this article about climate change in 3 sentences...',
        promptTokens: 2500,
        response: 'The article discusses the accelerating impacts of climate change...',
        responseTokens: 150,
        latencyMs: 1234.5,
        costUsd: 0.0035,
        metadata: { document_id: 'doc-001' },
      }),
      createTelemetryEvent({
        service: 'document-summarizer',
        model: 'gpt-3.5-turbo',
        prompt: 'Summarize this article about renewable energy...',
        promptTokens: 3200,
        response: 'This piece covers the latest advancements in solar and wind energy...',
        responseTokens: 180,
        latencyMs: 1456.2,
        costUsd: 0.0042,
        metadata: { document_id: 'doc-002' },
      }),
      createTelemetryEvent({
        service: 'document-summarizer',
        model: 'gpt-3.5-turbo',
        prompt: 'Summarize this article about AI ethics...',
        promptTokens: 2800,
        response: 'The article examines ethical considerations in AI development...',
        responseTokens: 165,
        latencyMs: 1312.8,
        costUsd: 0.0038,
        metadata: { document_id: 'doc-003' },
      }),
    ];

    console.log(`Sending ${events.length} events in batch...`);

    // Send all events in a single batch
    const metadata = await client.sendBatch(events);

    console.log(`Successfully sent ${metadata.length} events`);
    metadata.forEach((meta, i) => {
      console.log(`  Event ${i + 1}: partition=${meta.partition}, offset=${meta.baseOffset}`);
    });
  } catch (error) {
    console.error('Failed to send batch:', error);
  } finally {
    await client.disconnect();
    console.log('Disconnected from Kafka');
  }
}

main().catch(console.error);
