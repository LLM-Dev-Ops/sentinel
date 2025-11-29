/**
 * Event validation for LLM-Sentinel SDK
 */

import { TelemetryEvent, PromptInfo, ResponseInfo } from './types';
import { ValidationError } from './errors';

/**
 * Maximum text length for prompt and response (matching Rust validation)
 */
const MAX_TEXT_LENGTH = 100000;

/**
 * Validate UUID v4 format
 */
export function isValidUUID(uuid: string): boolean {
  const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
  return uuidRegex.test(uuid);
}

/**
 * Validate ISO 8601 timestamp format
 */
export function isValidISO8601(timestamp: string): boolean {
  try {
    const date = new Date(timestamp);
    return !isNaN(date.getTime()) && timestamp === date.toISOString();
  } catch {
    return false;
  }
}

/**
 * Validate prompt information
 */
export function validatePromptInfo(prompt: PromptInfo, fieldPrefix = 'prompt'): void {
  // Validate text
  if (typeof prompt.text !== 'string') {
    throw ValidationError.invalidValue(`${fieldPrefix}.text`, prompt.text, 'Must be a string');
  }

  if (prompt.text.length > MAX_TEXT_LENGTH) {
    throw ValidationError.invalidLength(`${fieldPrefix}.text`, prompt.text.length, MAX_TEXT_LENGTH);
  }

  // Validate tokens
  if (typeof prompt.tokens !== 'number') {
    throw ValidationError.invalidValue(`${fieldPrefix}.tokens`, prompt.tokens, 'Must be a number');
  }

  if (!Number.isInteger(prompt.tokens)) {
    throw ValidationError.invalidValue(`${fieldPrefix}.tokens`, prompt.tokens, 'Must be an integer');
  }

  if (prompt.tokens < 0) {
    throw ValidationError.outOfRange(`${fieldPrefix}.tokens`, prompt.tokens, 0);
  }

  // Validate embedding if present
  if (prompt.embedding !== undefined) {
    if (!Array.isArray(prompt.embedding)) {
      throw ValidationError.invalidValue(
        `${fieldPrefix}.embedding`,
        prompt.embedding,
        'Must be an array of numbers'
      );
    }

    for (let i = 0; i < prompt.embedding.length; i++) {
      const value = prompt.embedding[i];
      if (typeof value !== 'number' || !isFinite(value)) {
        throw ValidationError.invalidValue(
          `${fieldPrefix}.embedding[${i}]`,
          value,
          'Must be a finite number'
        );
      }
    }
  }
}

/**
 * Validate response information
 */
export function validateResponseInfo(response: ResponseInfo, fieldPrefix = 'response'): void {
  // Validate text
  if (typeof response.text !== 'string') {
    throw ValidationError.invalidValue(`${fieldPrefix}.text`, response.text, 'Must be a string');
  }

  if (response.text.length > MAX_TEXT_LENGTH) {
    throw ValidationError.invalidLength(
      `${fieldPrefix}.text`,
      response.text.length,
      MAX_TEXT_LENGTH
    );
  }

  // Validate tokens
  if (typeof response.tokens !== 'number') {
    throw ValidationError.invalidValue(`${fieldPrefix}.tokens`, response.tokens, 'Must be a number');
  }

  if (!Number.isInteger(response.tokens)) {
    throw ValidationError.invalidValue(
      `${fieldPrefix}.tokens`,
      response.tokens,
      'Must be an integer'
    );
  }

  if (response.tokens < 0) {
    throw ValidationError.outOfRange(`${fieldPrefix}.tokens`, response.tokens, 0);
  }

  // Validate finish_reason
  if (typeof response.finish_reason !== 'string') {
    throw ValidationError.invalidValue(
      `${fieldPrefix}.finish_reason`,
      response.finish_reason,
      'Must be a string'
    );
  }

  if (response.finish_reason.length === 0) {
    throw ValidationError.invalidValue(
      `${fieldPrefix}.finish_reason`,
      response.finish_reason,
      'Must not be empty'
    );
  }

  // Validate embedding if present
  if (response.embedding !== undefined) {
    if (!Array.isArray(response.embedding)) {
      throw ValidationError.invalidValue(
        `${fieldPrefix}.embedding`,
        response.embedding,
        'Must be an array of numbers'
      );
    }

    for (let i = 0; i < response.embedding.length; i++) {
      const value = response.embedding[i];
      if (typeof value !== 'number' || !isFinite(value)) {
        throw ValidationError.invalidValue(
          `${fieldPrefix}.embedding[${i}]`,
          value,
          'Must be a finite number'
        );
      }
    }
  }
}

/**
 * Validate metadata object
 */
export function validateMetadata(metadata: Record<string, string>): void {
  if (typeof metadata !== 'object' || metadata === null || Array.isArray(metadata)) {
    throw ValidationError.invalidValue('metadata', metadata, 'Must be a plain object');
  }

  for (const [key, value] of Object.entries(metadata)) {
    if (typeof key !== 'string' || key.length === 0) {
      throw ValidationError.invalidValue('metadata', metadata, 'Keys must be non-empty strings');
    }

    if (typeof value !== 'string') {
      throw ValidationError.invalidValue(
        `metadata.${key}`,
        value,
        'Values must be strings'
      );
    }
  }
}

/**
 * Validate errors array
 */
export function validateErrors(errors: string[]): void {
  if (!Array.isArray(errors)) {
    throw ValidationError.invalidValue('errors', errors, 'Must be an array');
  }

  for (let i = 0; i < errors.length; i++) {
    if (typeof errors[i] !== 'string') {
      throw ValidationError.invalidValue(`errors[${i}]`, errors[i], 'Must be a string');
    }
  }
}

/**
 * Validate complete telemetry event
 */
export function validateTelemetryEvent(event: TelemetryEvent): void {
  // Validate event_id
  if (!event.event_id) {
    throw ValidationError.missingField('event_id');
  }

  if (!isValidUUID(event.event_id)) {
    throw ValidationError.invalidValue('event_id', event.event_id, 'Must be a valid UUID v4');
  }

  // Validate timestamp
  if (!event.timestamp) {
    throw ValidationError.missingField('timestamp');
  }

  if (!isValidISO8601(event.timestamp)) {
    throw ValidationError.invalidValue(
      'timestamp',
      event.timestamp,
      'Must be a valid ISO 8601 timestamp'
    );
  }

  // Validate service_name
  if (!event.service_name) {
    throw ValidationError.missingField('service_name');
  }

  if (typeof event.service_name !== 'string' || event.service_name.trim().length === 0) {
    throw ValidationError.invalidValue(
      'service_name',
      event.service_name,
      'Must be a non-empty string'
    );
  }

  // Validate trace_id if present
  if (event.trace_id !== undefined) {
    if (typeof event.trace_id !== 'string' || event.trace_id.length === 0) {
      throw ValidationError.invalidValue('trace_id', event.trace_id, 'Must be a non-empty string');
    }
  }

  // Validate span_id if present
  if (event.span_id !== undefined) {
    if (typeof event.span_id !== 'string' || event.span_id.length === 0) {
      throw ValidationError.invalidValue('span_id', event.span_id, 'Must be a non-empty string');
    }
  }

  // Validate model
  if (!event.model) {
    throw ValidationError.missingField('model');
  }

  if (typeof event.model !== 'string' || event.model.trim().length === 0) {
    throw ValidationError.invalidValue('model', event.model, 'Must be a non-empty string');
  }

  // Validate prompt
  if (!event.prompt) {
    throw ValidationError.missingField('prompt');
  }
  validatePromptInfo(event.prompt);

  // Validate response
  if (!event.response) {
    throw ValidationError.missingField('response');
  }
  validateResponseInfo(event.response);

  // Validate latency_ms
  if (typeof event.latency_ms !== 'number') {
    throw ValidationError.invalidValue('latency_ms', event.latency_ms, 'Must be a number');
  }

  if (!isFinite(event.latency_ms)) {
    throw ValidationError.invalidValue('latency_ms', event.latency_ms, 'Must be a finite number');
  }

  if (event.latency_ms < 0) {
    throw ValidationError.outOfRange('latency_ms', event.latency_ms, 0);
  }

  // Validate cost_usd
  if (typeof event.cost_usd !== 'number') {
    throw ValidationError.invalidValue('cost_usd', event.cost_usd, 'Must be a number');
  }

  if (!isFinite(event.cost_usd)) {
    throw ValidationError.invalidValue('cost_usd', event.cost_usd, 'Must be a finite number');
  }

  if (event.cost_usd < 0) {
    throw ValidationError.outOfRange('cost_usd', event.cost_usd, 0);
  }

  // Validate metadata
  if (!event.metadata) {
    throw ValidationError.missingField('metadata');
  }
  validateMetadata(event.metadata);

  // Validate errors
  if (!event.errors) {
    throw ValidationError.missingField('errors');
  }
  validateErrors(event.errors);
}
