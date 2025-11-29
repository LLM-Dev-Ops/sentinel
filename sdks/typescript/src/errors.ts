/**
 * Custom error classes for LLM-Sentinel SDK
 */

/**
 * Base error class for all Sentinel SDK errors
 */
export class SentinelError extends Error {
  public readonly code: string;
  public readonly details?: unknown;

  constructor(message: string, code: string = 'SENTINEL_ERROR', details?: unknown) {
    super(message);
    this.name = 'SentinelError';
    this.code = code;
    this.details = details;

    // Maintains proper stack trace for where our error was thrown (only available on V8)
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }
}

/**
 * Connection error - thrown when connection to Kafka fails
 */
export class ConnectionError extends SentinelError {
  constructor(message: string, details?: unknown) {
    super(message, 'CONNECTION_ERROR', details);
    this.name = 'ConnectionError';
  }

  /**
   * Create a ConnectionError for broker connection failures
   */
  static brokerConnectionFailed(brokers: string[], cause?: Error): ConnectionError {
    return new ConnectionError(
      `Failed to connect to Kafka brokers: ${brokers.join(', ')}`,
      { brokers, cause: cause?.message }
    );
  }

  /**
   * Create a ConnectionError for timeout issues
   */
  static timeout(operation: string, timeoutMs: number): ConnectionError {
    return new ConnectionError(
      `${operation} timed out after ${timeoutMs}ms`,
      { operation, timeoutMs }
    );
  }
}

/**
 * Validation error - thrown when event data is invalid
 */
export class ValidationError extends SentinelError {
  public readonly field?: string;

  constructor(message: string, field?: string, details?: unknown) {
    super(message, 'VALIDATION_ERROR', details);
    this.name = 'ValidationError';
    this.field = field;
  }

  /**
   * Create a ValidationError for missing required fields
   */
  static missingField(field: string): ValidationError {
    return new ValidationError(`Missing required field: ${field}`, field);
  }

  /**
   * Create a ValidationError for invalid field values
   */
  static invalidValue(field: string, value: unknown, reason: string): ValidationError {
    return new ValidationError(
      `Invalid value for field '${field}': ${reason}`,
      field,
      { value, reason }
    );
  }

  /**
   * Create a ValidationError for range violations
   */
  static outOfRange(field: string, value: number, min?: number, max?: number): ValidationError {
    let message = `Value for field '${field}' is out of range: ${value}`;
    if (min !== undefined && max !== undefined) {
      message += ` (expected ${min} - ${max})`;
    } else if (min !== undefined) {
      message += ` (expected >= ${min})`;
    } else if (max !== undefined) {
      message += ` (expected <= ${max})`;
    }
    return new ValidationError(message, field, { value, min, max });
  }

  /**
   * Create a ValidationError for string length violations
   */
  static invalidLength(field: string, length: number, maxLength: number): ValidationError {
    return new ValidationError(
      `Field '${field}' exceeds maximum length: ${length} > ${maxLength}`,
      field,
      { length, maxLength }
    );
  }
}

/**
 * Send error - thrown when sending events to Kafka fails
 */
export class SendError extends SentinelError {
  public readonly eventId?: string;
  public readonly isRetryable: boolean;

  constructor(message: string, isRetryable: boolean = true, eventId?: string, details?: unknown) {
    super(message, 'SEND_ERROR', details);
    this.name = 'SendError';
    this.eventId = eventId;
    this.isRetryable = isRetryable;
  }

  /**
   * Create a SendError for producer errors
   */
  static producerError(message: string, cause?: Error, eventId?: string): SendError {
    return new SendError(
      `Failed to send event: ${message}`,
      true,
      eventId,
      { cause: cause?.message }
    );
  }

  /**
   * Create a SendError for serialization failures
   */
  static serializationError(eventId: string, cause?: Error): SendError {
    return new SendError(
      `Failed to serialize event: ${cause?.message || 'Unknown error'}`,
      false,
      eventId,
      { cause: cause?.message }
    );
  }

  /**
   * Create a SendError for broker unavailability
   */
  static brokerUnavailable(eventId?: string): SendError {
    return new SendError(
      'Kafka broker is unavailable',
      true,
      eventId
    );
  }

  /**
   * Create a SendError for disconnected state
   */
  static notConnected(): SendError {
    return new SendError(
      'Cannot send event: client is not connected. Call connect() first or enable autoConnect.',
      false
    );
  }
}

/**
 * Configuration error - thrown when client configuration is invalid
 */
export class ConfigurationError extends SentinelError {
  constructor(message: string, details?: unknown) {
    super(message, 'CONFIGURATION_ERROR', details);
    this.name = 'ConfigurationError';
  }

  /**
   * Create a ConfigurationError for missing configuration
   */
  static missingConfig(field: string): ConfigurationError {
    return new ConfigurationError(
      `Missing required configuration: ${field}`,
      { field }
    );
  }

  /**
   * Create a ConfigurationError for invalid configuration
   */
  static invalidConfig(field: string, reason: string): ConfigurationError {
    return new ConfigurationError(
      `Invalid configuration for '${field}': ${reason}`,
      { field, reason }
    );
  }
}
