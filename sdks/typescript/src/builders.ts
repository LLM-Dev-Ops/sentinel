/**
 * Fluent builder for constructing telemetry events
 */

import { v4 as uuidv4 } from 'uuid';
import { TelemetryEvent, PromptInfo, ResponseInfo } from './types';
import { ValidationError } from './errors';

/**
 * Fluent builder for TelemetryEvent
 *
 * @example
 * ```typescript
 * const event = new TelemetryEventBuilder()
 *   .service('my-service')
 *   .model('gpt-4')
 *   .prompt({ text: 'Hello', tokens: 5 })
 *   .response({ text: 'Hi!', tokens: 10, finishReason: 'stop' })
 *   .latency(150.5)
 *   .cost(0.001)
 *   .build();
 * ```
 */
export class TelemetryEventBuilder {
  private _eventId?: string;
  private _timestamp?: string;
  private _serviceName?: string;
  private _traceId?: string;
  private _spanId?: string;
  private _model?: string;
  private _prompt?: PromptInfo;
  private _response?: ResponseInfo;
  private _latencyMs?: number;
  private _costUsd?: number;
  private _metadata: Record<string, string> = {};
  private _errors: string[] = [];

  /**
   * Set event ID (auto-generated if not set)
   */
  eventId(id: string): this {
    this._eventId = id;
    return this;
  }

  /**
   * Set timestamp (auto-generated as current time if not set)
   */
  timestamp(timestamp: string | Date): this {
    if (timestamp instanceof Date) {
      this._timestamp = timestamp.toISOString();
    } else {
      this._timestamp = timestamp;
    }
    return this;
  }

  /**
   * Set service name
   */
  service(name: string): this {
    this._serviceName = name;
    return this;
  }

  /**
   * Set trace ID for distributed tracing
   */
  traceId(id: string): this {
    this._traceId = id;
    return this;
  }

  /**
   * Set span ID
   */
  spanId(id: string): this {
    this._spanId = id;
    return this;
  }

  /**
   * Set model identifier
   */
  model(model: string): this {
    this._model = model;
    return this;
  }

  /**
   * Set prompt information
   */
  prompt(prompt: PromptInfo | { text: string; tokens: number; embedding?: number[] }): this {
    this._prompt = prompt as PromptInfo;
    return this;
  }

  /**
   * Set response information
   */
  response(
    response: ResponseInfo | { text: string; tokens: number; finishReason: string; embedding?: number[] }
  ): this {
    // Map finishReason to finish_reason for convenience
    const responseObj = response as any;
    if (responseObj.finishReason && !responseObj.finish_reason) {
      responseObj.finish_reason = responseObj.finishReason;
      delete responseObj.finishReason;
    }
    this._response = responseObj as ResponseInfo;
    return this;
  }

  /**
   * Set latency in milliseconds
   */
  latency(ms: number): this {
    this._latencyMs = ms;
    return this;
  }

  /**
   * Set cost in USD
   */
  cost(usd: number): this {
    this._costUsd = usd;
    return this;
  }

  /**
   * Add metadata key-value pair
   */
  addMetadata(key: string, value: string): this {
    this._metadata[key] = value;
    return this;
  }

  /**
   * Set metadata object (replaces existing metadata)
   */
  metadata(metadata: Record<string, string>): this {
    this._metadata = { ...metadata };
    return this;
  }

  /**
   * Add an error message
   */
  addError(error: string): this {
    this._errors.push(error);
    return this;
  }

  /**
   * Set errors array (replaces existing errors)
   */
  errors(errors: string[]): this {
    this._errors = [...errors];
    return this;
  }

  /**
   * Build the TelemetryEvent
   * @throws {ValidationError} If required fields are missing
   */
  build(): TelemetryEvent {
    // Validate required fields
    if (!this._serviceName) {
      throw ValidationError.missingField('service_name');
    }

    if (!this._model) {
      throw ValidationError.missingField('model');
    }

    if (!this._prompt) {
      throw ValidationError.missingField('prompt');
    }

    if (!this._response) {
      throw ValidationError.missingField('response');
    }

    if (this._latencyMs === undefined) {
      throw ValidationError.missingField('latency_ms');
    }

    if (this._costUsd === undefined) {
      throw ValidationError.missingField('cost_usd');
    }

    // Build the event
    const event: TelemetryEvent = {
      event_id: this._eventId || uuidv4(),
      timestamp: this._timestamp || new Date().toISOString(),
      service_name: this._serviceName,
      trace_id: this._traceId,
      span_id: this._spanId,
      model: this._model,
      prompt: this._prompt,
      response: this._response,
      latency_ms: this._latencyMs,
      cost_usd: this._costUsd,
      metadata: this._metadata,
      errors: this._errors,
    };

    return event;
  }

  /**
   * Reset the builder to initial state
   */
  reset(): this {
    this._eventId = undefined;
    this._timestamp = undefined;
    this._serviceName = undefined;
    this._traceId = undefined;
    this._spanId = undefined;
    this._model = undefined;
    this._prompt = undefined;
    this._response = undefined;
    this._latencyMs = undefined;
    this._costUsd = undefined;
    this._metadata = {};
    this._errors = [];
    return this;
  }
}

/**
 * Helper function to create a TelemetryEvent from simple parameters
 */
export function createTelemetryEvent(params: {
  service: string;
  model: string;
  prompt: string;
  promptTokens: number;
  response: string;
  responseTokens: number;
  latencyMs: number;
  costUsd: number;
  finishReason?: string;
  traceId?: string;
  spanId?: string;
  metadata?: Record<string, string>;
  errors?: string[];
  promptEmbedding?: number[];
  responseEmbedding?: number[];
}): TelemetryEvent {
  const builder = new TelemetryEventBuilder()
    .service(params.service)
    .model(params.model)
    .prompt({
      text: params.prompt,
      tokens: params.promptTokens,
      embedding: params.promptEmbedding,
    })
    .response({
      text: params.response,
      tokens: params.responseTokens,
      finish_reason: params.finishReason || 'stop',
      embedding: params.responseEmbedding,
    })
    .latency(params.latencyMs)
    .cost(params.costUsd);

  if (params.traceId) {
    builder.traceId(params.traceId);
  }

  if (params.spanId) {
    builder.spanId(params.spanId);
  }

  if (params.metadata) {
    builder.metadata(params.metadata);
  }

  if (params.errors) {
    builder.errors(params.errors);
  }

  return builder.build();
}
