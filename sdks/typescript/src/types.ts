/**
 * Type definitions for LLM-Sentinel SDK
 * These types match the Rust implementation in sentinel-core/events.rs
 */

import { Severity, AnomalyType, DetectionMethod } from './enums';

/**
 * Prompt information
 */
export interface PromptInfo {
  /** Prompt text (may be truncated for storage, max 100,000 chars) */
  text: string;
  /** Token count (must be >= 0) */
  tokens: number;
  /** Optional embedding vector */
  embedding?: number[];
}

/**
 * Response information
 */
export interface ResponseInfo {
  /** Response text (may be truncated for storage, max 100,000 chars) */
  text: string;
  /** Token count (must be >= 0) */
  tokens: number;
  /** Finish reason (e.g., "stop", "length", "content_filter") */
  finish_reason: string;
  /** Optional embedding vector */
  embedding?: number[];
}

/**
 * Telemetry event from LLM application
 */
export interface TelemetryEvent {
  /** Unique event identifier (UUID v4) */
  event_id: string;
  /** Event timestamp (ISO 8601) */
  timestamp: string;
  /** Service name/identifier */
  service_name: string;
  /** Trace ID for distributed tracing (optional) */
  trace_id?: string;
  /** Span ID (optional) */
  span_id?: string;
  /** Model identifier (e.g., "gpt-4", "claude-3") */
  model: string;
  /** Prompt information */
  prompt: PromptInfo;
  /** Response information */
  response: ResponseInfo;
  /** Request latency in milliseconds (must be >= 0) */
  latency_ms: number;
  /** Cost in USD (must be >= 0) */
  cost_usd: number;
  /** Additional metadata (key-value pairs) */
  metadata: Record<string, string>;
  /** Error messages, if any */
  errors: string[];
}

/**
 * Detailed anomaly information
 */
export interface AnomalyDetails {
  /** Metric name */
  metric: string;
  /** Observed value */
  value: number;
  /** Baseline/expected value */
  baseline: number;
  /** Threshold that was exceeded */
  threshold: number;
  /** Standard deviations from baseline (if applicable) */
  deviation_sigma?: number;
  /** Additional details */
  additional: Record<string, unknown>;
}

/**
 * Context information for anomaly
 */
export interface AnomalyContext {
  /** Trace ID if available */
  trace_id?: string;
  /** User ID if available */
  user_id?: string;
  /** Region/datacenter */
  region?: string;
  /** Time window analyzed */
  time_window: string;
  /** Number of samples in window */
  sample_count: number;
  /** Additional context */
  additional: Record<string, string>;
}

/**
 * Anomaly event detected by Sentinel
 */
export interface AnomalyEvent {
  /** Unique alert identifier */
  alert_id: string;
  /** Detection timestamp (ISO 8601) */
  timestamp: string;
  /** Severity level */
  severity: Severity;
  /** Type of anomaly */
  anomaly_type: AnomalyType;
  /** Service name */
  service_name: string;
  /** Model identifier */
  model: string;
  /** Detection method used */
  detection_method: DetectionMethod;
  /** Confidence score (0.0 - 1.0) */
  confidence: number;
  /** Detailed anomaly information */
  details: AnomalyDetails;
  /** Context information */
  context: AnomalyContext;
  /** Root cause analysis */
  root_cause?: string;
  /** Remediation suggestions */
  remediation: string[];
  /** Related alert IDs */
  related_alerts: string[];
  /** Runbook URL */
  runbook_url?: string;
}

/**
 * Alert event sent to incident manager
 */
export interface AlertEvent {
  /** Alert identifier (same as anomaly alert_id) */
  alert_id: string;
  /** Alert timestamp (ISO 8601) */
  timestamp: string;
  /** Severity */
  severity: Severity;
  /** Alert title */
  title: string;
  /** Alert description */
  description: string;
  /** Service affected */
  service_name: string;
  /** Model affected */
  model: string;
  /** Alert tags */
  tags: string[];
  /** Source anomaly event */
  anomaly: AnomalyEvent;
}

/**
 * Kafka configuration options
 */
export interface KafkaConfig {
  /** Kafka broker addresses */
  brokers: string[];
  /** Client ID for the producer */
  clientId?: string;
  /** SSL/TLS configuration */
  ssl?: boolean | {
    rejectUnauthorized?: boolean;
    ca?: string[];
    cert?: string;
    key?: string;
  };
  /** SASL authentication */
  sasl?: {
    mechanism: 'plain' | 'scram-sha-256' | 'scram-sha-512';
    username: string;
    password: string;
  };
  /** Request timeout in milliseconds */
  requestTimeout?: number;
  /** Connection timeout in milliseconds */
  connectionTimeout?: number;
  /** Retry configuration */
  retry?: {
    initialRetryTime?: number;
    retries?: number;
    maxRetryTime?: number;
    multiplier?: number;
    factor?: number;
  };
}

/**
 * Sentinel client configuration
 */
export interface SentinelConfig {
  /** Kafka configuration */
  kafka: KafkaConfig;
  /** Kafka topic to publish to */
  topic?: string;
  /** Enable automatic connection on first send */
  autoConnect?: boolean;
  /** Enable debug logging */
  debug?: boolean;
  /** Compression type (none, gzip, snappy, lz4) */
  compression?: 'none' | 'gzip' | 'snappy' | 'lz4';
  /** Number of acknowledgments required (0, 1, -1/all) */
  acks?: number;
  /** Request timeout in milliseconds */
  timeout?: number;
}

/**
 * Options for tracking a simple telemetry event
 */
export interface TrackOptions {
  /** Service name */
  service: string;
  /** Model identifier */
  model: string;
  /** Prompt text */
  prompt: string;
  /** Response text */
  response: string;
  /** Latency in milliseconds */
  latencyMs: number;
  /** Prompt token count */
  promptTokens: number;
  /** Completion/response token count */
  completionTokens: number;
  /** Cost in USD */
  costUsd: number;
  /** Trace ID (optional) */
  traceId?: string;
  /** Span ID (optional) */
  spanId?: string;
  /** Finish reason (optional, defaults to "stop") */
  finishReason?: string;
  /** Additional metadata (optional) */
  metadata?: Record<string, string>;
  /** Error messages (optional) */
  errors?: string[];
  /** Prompt embedding (optional) */
  promptEmbedding?: number[];
  /** Response embedding (optional) */
  responseEmbedding?: number[];
}
