/**
 * Main SentinelClient for sending telemetry events to LLM-Sentinel
 */

import { Kafka, Producer, ProducerRecord, RecordMetadata, logLevel } from 'kafkajs';
import { TelemetryEvent, SentinelConfig, TrackOptions } from './types';
import { createConfig } from './config';
import { validateTelemetryEvent } from './validation';
import { createTelemetryEvent } from './builders';
import {
  SentinelError,
  ConnectionError,
  ValidationError,
  SendError,
} from './errors';

/**
 * Main client for sending LLM telemetry events to Sentinel
 *
 * @example
 * ```typescript
 * // Simple usage
 * const client = new SentinelClient({ brokers: ['localhost:9092'] });
 * await client.track({
 *   service: 'my-service',
 *   model: 'gpt-4',
 *   prompt: 'Hello',
 *   response: 'Hi there!',
 *   latencyMs: 150.5,
 *   promptTokens: 5,
 *   completionTokens: 10,
 *   costUsd: 0.001
 * });
 *
 * // With connection management
 * await client.connect();
 * await client.send(event);
 * await client.disconnect();
 * ```
 */
export class SentinelClient {
  private readonly config: SentinelConfig;
  private readonly kafka: Kafka;
  private producer: Producer | null = null;
  private connected = false;
  private connecting = false;

  /**
   * Create a new SentinelClient
   *
   * @param config - Client configuration
   * @throws {ConfigurationError} If configuration is invalid
   */
  constructor(config: Partial<SentinelConfig>) {
    this.config = createConfig(config);

    // Build SASL config if provided
    let saslConfig: { mechanism: 'plain'; username: string; password: string } | undefined;
    if (this.config.kafka.sasl) {
      saslConfig = {
        mechanism: 'plain',
        username: this.config.kafka.sasl.username,
        password: this.config.kafka.sasl.password,
      };
    }

    // Create Kafka instance
    this.kafka = new Kafka({
      clientId: this.config.kafka.clientId,
      brokers: this.config.kafka.brokers!,
      ssl: this.config.kafka.ssl,
      sasl: saslConfig,
      connectionTimeout: this.config.kafka.connectionTimeout,
      requestTimeout: this.config.kafka.requestTimeout,
      retry: this.config.kafka.retry,
      logLevel: this.config.debug ? logLevel.DEBUG : logLevel.ERROR,
    });

    // Create producer
    this.producer = this.kafka.producer({
      allowAutoTopicCreation: false,
      transactionTimeout: this.config.timeout,
      retry: this.config.kafka.retry,
    });
  }

  /**
   * Connect to Kafka
   *
   * @throws {ConnectionError} If connection fails
   */
  async connect(): Promise<void> {
    if (this.connected) {
      return;
    }

    if (this.connecting) {
      // Wait for existing connection attempt
      while (this.connecting) {
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      if (this.connected) {
        return;
      }
    }

    this.connecting = true;

    try {
      if (!this.producer) {
        throw new ConnectionError('Producer not initialized');
      }

      await this.producer.connect();
      this.connected = true;

      if (this.config.debug) {
        console.log('[Sentinel] Connected to Kafka brokers:', this.config.kafka.brokers);
      }
    } catch (error) {
      this.connected = false;
      throw ConnectionError.brokerConnectionFailed(
        this.config.kafka.brokers,
        error as Error
      );
    } finally {
      this.connecting = false;
    }
  }

  /**
   * Disconnect from Kafka
   */
  async disconnect(): Promise<void> {
    if (!this.connected || !this.producer) {
      return;
    }

    try {
      await this.producer.disconnect();
      this.connected = false;

      if (this.config.debug) {
        console.log('[Sentinel] Disconnected from Kafka');
      }
    } catch (error) {
      if (this.config.debug) {
        console.error('[Sentinel] Error during disconnect:', error);
      }
      // Still mark as disconnected even if error occurs
      this.connected = false;
      throw new ConnectionError(
        `Failed to disconnect: ${(error as Error).message}`,
        { cause: (error as Error).message }
      );
    }
  }

  /**
   * Check if client is connected
   */
  isConnected(): boolean {
    return this.connected;
  }

  /**
   * Send a telemetry event to Sentinel
   *
   * @param event - The telemetry event to send
   * @returns Metadata about the sent message
   * @throws {ValidationError} If event is invalid
   * @throws {SendError} If sending fails
   * @throws {ConnectionError} If not connected and autoConnect is disabled
   */
  async send(event: TelemetryEvent): Promise<RecordMetadata[]> {
    // Validate event
    try {
      validateTelemetryEvent(event);
    } catch (error) {
      if (error instanceof ValidationError) {
        throw error;
      }
      throw new ValidationError(
        `Event validation failed: ${(error as Error).message}`,
        undefined,
        { cause: (error as Error).message }
      );
    }

    // Ensure connected
    if (!this.connected) {
      if (this.config.autoConnect) {
        await this.connect();
      } else {
        throw SendError.notConnected();
      }
    }

    if (!this.producer) {
      throw new SendError('Producer not initialized', false);
    }

    // Serialize event
    let payload: string;
    try {
      payload = JSON.stringify(event);
    } catch (error) {
      throw SendError.serializationError(event.event_id, error as Error);
    }

    // Prepare Kafka message
    const record: ProducerRecord = {
      topic: this.config.topic!,
      messages: [
        {
          key: event.service_name,
          value: payload,
          headers: {
            'event_id': event.event_id,
            'service_name': event.service_name,
            'model': event.model,
          },
        },
      ],
      acks: this.config.acks,
      timeout: this.config.timeout,
    };

    // Send to Kafka
    try {
      const metadata = await this.producer.send(record);

      if (this.config.debug && metadata[0]) {
        console.log(
          `[Sentinel] Sent event ${event.event_id} to partition ${metadata[0].partition ?? 'unknown'} at offset ${metadata[0].baseOffset ?? 'unknown'}`
        );
      }

      return metadata;
    } catch (error) {
      const kafkaError = error as Error;
      throw SendError.producerError(kafkaError.message, kafkaError, event.event_id);
    }
  }

  /**
   * Send multiple telemetry events in a batch
   *
   * @param events - Array of telemetry events
   * @returns Array of metadata for each sent message
   * @throws {ValidationError} If any event is invalid
   * @throws {SendError} If sending fails
   */
  async sendBatch(events: TelemetryEvent[]): Promise<RecordMetadata[]> {
    // Validate all events first
    for (const event of events) {
      try {
        validateTelemetryEvent(event);
      } catch (error) {
        if (error instanceof ValidationError) {
          throw error;
        }
        throw new ValidationError(
          `Event validation failed: ${(error as Error).message}`,
          undefined,
          { cause: (error as Error).message, eventId: event.event_id }
        );
      }
    }

    // Ensure connected
    if (!this.connected) {
      if (this.config.autoConnect) {
        await this.connect();
      } else {
        throw SendError.notConnected();
      }
    }

    if (!this.producer) {
      throw new SendError('Producer not initialized', false);
    }

    // Prepare messages
    const messages = events.map(event => {
      let payload: string;
      try {
        payload = JSON.stringify(event);
      } catch (error) {
        throw SendError.serializationError(event.event_id, error as Error);
      }

      return {
        key: event.service_name,
        value: payload,
        headers: {
          'event_id': event.event_id,
          'service_name': event.service_name,
          'model': event.model,
        },
      };
    });

    const record: ProducerRecord = {
      topic: this.config.topic!,
      messages,
      acks: this.config.acks,
      timeout: this.config.timeout,
    };

    // Send to Kafka
    try {
      const metadata = await this.producer.send(record);

      if (this.config.debug) {
        console.log(`[Sentinel] Sent batch of ${events.length} events`);
      }

      return metadata;
    } catch (error) {
      throw SendError.producerError((error as Error).message, error as Error);
    }
  }

  /**
   * Convenient method to track a simple telemetry event
   *
   * @param options - Simple tracking options
   * @returns Metadata about the sent message
   * @throws {ValidationError} If options are invalid
   * @throws {SendError} If sending fails
   *
   * @example
   * ```typescript
   * await client.track({
   *   service: 'my-service',
   *   model: 'gpt-4',
   *   prompt: 'Hello',
   *   response: 'Hi there!',
   *   latencyMs: 150.5,
   *   promptTokens: 5,
   *   completionTokens: 10,
   *   costUsd: 0.001
   * });
   * ```
   */
  async track(options: TrackOptions): Promise<RecordMetadata[]> {
    const event = createTelemetryEvent({
      service: options.service,
      model: options.model,
      prompt: options.prompt,
      promptTokens: options.promptTokens,
      response: options.response,
      responseTokens: options.completionTokens,
      latencyMs: options.latencyMs,
      costUsd: options.costUsd,
      finishReason: options.finishReason,
      traceId: options.traceId,
      spanId: options.spanId,
      metadata: options.metadata,
      errors: options.errors,
      promptEmbedding: options.promptEmbedding,
      responseEmbedding: options.responseEmbedding,
    });

    return this.send(event);
  }

  /**
   * Flush any pending messages and disconnect gracefully
   *
   * @param _timeout - Optional timeout in milliseconds (reserved for future use)
   */
  async flush(_timeout?: number): Promise<void> {
    if (!this.producer || !this.connected) {
      return;
    }

    try {
      // KafkaJS producer doesn't have explicit flush, but disconnect waits for pending messages
      await this.disconnect();
    } catch (error) {
      if (this.config.debug) {
        console.error('[Sentinel] Error during flush:', error);
      }
      throw new SentinelError(
        `Failed to flush: ${(error as Error).message}`,
        'FLUSH_ERROR',
        { cause: (error as Error).message }
      );
    }
  }

  /**
   * Get the current configuration
   */
  getConfig(): Readonly<SentinelConfig> {
    return Object.freeze({ ...this.config });
  }
}
