/**
 * LLM-Sentinel TypeScript/JavaScript SDK
 *
 * A production-ready SDK for sending LLM telemetry events to Sentinel for anomaly detection.
 *
 * @example
 * ```typescript
 * import { SentinelClient } from '@llm-sentinel/sdk';
 *
 * const client = new SentinelClient({ brokers: ['localhost:9092'] });
 *
 * await client.track({
 *   service: 'my-service',
 *   model: 'gpt-4',
 *   prompt: 'Hello, world!',
 *   response: 'Hi there!',
 *   latencyMs: 150.5,
 *   promptTokens: 5,
 *   completionTokens: 10,
 *   costUsd: 0.001
 * });
 *
 * await client.disconnect();
 * ```
 *
 * @packageDocumentation
 */

// Main client
export { SentinelClient } from './client';

// Types
export type {
  TelemetryEvent,
  PromptInfo,
  ResponseInfo,
  AnomalyEvent,
  AnomalyDetails,
  AnomalyContext,
  AlertEvent,
  KafkaConfig,
  SentinelConfig,
  TrackOptions,
} from './types';

// Enums
export { Severity, AnomalyType, DetectionMethod } from './enums';

// Builders
export { TelemetryEventBuilder, createTelemetryEvent } from './builders';

// Errors
export {
  SentinelError,
  ConnectionError,
  ValidationError,
  SendError,
  ConfigurationError,
} from './errors';

// Validation utilities
export {
  validateTelemetryEvent,
  validatePromptInfo,
  validateResponseInfo,
  validateMetadata,
  validateErrors,
  isValidUUID,
  isValidISO8601,
} from './validation';

// Configuration utilities
export {
  createConfig,
  loadConfigFromEnv,
  mergeConfig,
  validateConfig,
  DEFAULT_TOPIC,
  DEFAULT_CLIENT_ID,
  DEFAULT_CONFIG,
  DEFAULT_KAFKA_CONFIG,
  ENV_VARS,
} from './config';
