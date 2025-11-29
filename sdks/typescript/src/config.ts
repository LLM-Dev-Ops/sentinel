/**
 * Configuration management for LLM-Sentinel SDK
 */

import { SentinelConfig, KafkaConfig } from './types';
import { ConfigurationError } from './errors';

/**
 * Default Kafka topic for telemetry events
 */
export const DEFAULT_TOPIC = 'llm.telemetry';

/**
 * Default client ID prefix
 */
export const DEFAULT_CLIENT_ID = 'llm-sentinel-sdk';

/**
 * Default configuration values
 */
export const DEFAULT_CONFIG = {
  topic: DEFAULT_TOPIC,
  autoConnect: true,
  debug: false,
  compression: 'gzip' as const,
  acks: -1, // Wait for all in-sync replicas
  timeout: 30000, // 30 seconds
};

/**
 * Default Kafka configuration values
 */
export const DEFAULT_KAFKA_CONFIG = {
  requestTimeout: 30000,
  connectionTimeout: 10000,
  retry: {
    initialRetryTime: 100,
    retries: 8,
    maxRetryTime: 30000,
    multiplier: 2,
    factor: 0.2,
  },
};

/**
 * Environment variable names for configuration
 */
export const ENV_VARS = {
  KAFKA_BROKERS: 'SENTINEL_KAFKA_BROKERS',
  KAFKA_TOPIC: 'SENTINEL_KAFKA_TOPIC',
  KAFKA_CLIENT_ID: 'SENTINEL_KAFKA_CLIENT_ID',
  KAFKA_USERNAME: 'SENTINEL_KAFKA_USERNAME',
  KAFKA_PASSWORD: 'SENTINEL_KAFKA_PASSWORD',
  KAFKA_SSL: 'SENTINEL_KAFKA_SSL',
  DEBUG: 'SENTINEL_DEBUG',
} as const;

/**
 * Load configuration from environment variables
 */
export function loadConfigFromEnv(): Partial<SentinelConfig> {
  const config: Partial<SentinelConfig> = {};

  // Load Kafka brokers
  const brokers = process.env[ENV_VARS.KAFKA_BROKERS];
  if (brokers) {
    config.kafka = {
      brokers: brokers.split(',').map(b => b.trim()),
    };

    // Load client ID
    const clientId = process.env[ENV_VARS.KAFKA_CLIENT_ID];
    if (clientId) {
      config.kafka.clientId = clientId;
    }

    // Load SASL credentials if provided
    const username = process.env[ENV_VARS.KAFKA_USERNAME];
    const password = process.env[ENV_VARS.KAFKA_PASSWORD];
    if (username && password) {
      config.kafka.sasl = {
        mechanism: 'plain',
        username,
        password,
      };
    }

    // Load SSL setting
    const ssl = process.env[ENV_VARS.KAFKA_SSL];
    if (ssl) {
      config.kafka.ssl = ssl.toLowerCase() === 'true';
    }
  }

  // Load topic
  const topic = process.env[ENV_VARS.KAFKA_TOPIC];
  if (topic) {
    config.topic = topic;
  }

  // Load debug setting
  const debug = process.env[ENV_VARS.DEBUG];
  if (debug) {
    config.debug = debug.toLowerCase() === 'true';
  }

  return config;
}

/**
 * Merge configuration objects with defaults
 */
export function mergeConfig(
  userConfig: Partial<SentinelConfig>,
  envConfig?: Partial<SentinelConfig>
): SentinelConfig {
  // Merge Kafka configs
  const mergedKafka = {
    ...DEFAULT_KAFKA_CONFIG,
    ...(envConfig?.kafka || {}),
    ...(userConfig.kafka || {}),
  };

  // Ensure brokers are provided
  if (!mergedKafka.brokers || mergedKafka.brokers.length === 0) {
    throw ConfigurationError.missingConfig('kafka.brokers');
  }

  // Set default client ID if not provided
  if (!mergedKafka.clientId) {
    mergedKafka.clientId = `${DEFAULT_CLIENT_ID}-${Math.random().toString(36).substr(2, 9)}`;
  }

  // Build final kafka config with required brokers
  const kafkaConfig: KafkaConfig = {
    ...mergedKafka,
    brokers: mergedKafka.brokers,
  };

  // Merge top-level configs
  return {
    ...DEFAULT_CONFIG,
    ...(envConfig || {}),
    ...userConfig,
    kafka: kafkaConfig,
  };
}

/**
 * Validate configuration
 */
export function validateConfig(config: SentinelConfig): void {
  // Validate Kafka brokers
  if (!config.kafka.brokers || config.kafka.brokers.length === 0) {
    throw ConfigurationError.missingConfig('kafka.brokers');
  }

  for (const broker of config.kafka.brokers) {
    if (!broker || typeof broker !== 'string' || broker.trim().length === 0) {
      throw ConfigurationError.invalidConfig('kafka.brokers', 'Invalid broker address');
    }
  }

  // Validate topic
  if (!config.topic || typeof config.topic !== 'string' || config.topic.trim().length === 0) {
    throw ConfigurationError.invalidConfig('topic', 'Topic must be a non-empty string');
  }

  // Validate acks
  if (config.acks !== undefined && ![0, 1, -1].includes(config.acks)) {
    throw ConfigurationError.invalidConfig('acks', 'Must be 0, 1, or -1 (all)');
  }

  // Validate timeout
  if (config.timeout !== undefined && (config.timeout < 1000 || config.timeout > 300000)) {
    throw ConfigurationError.invalidConfig('timeout', 'Must be between 1000ms and 300000ms');
  }

  // Validate compression
  const validCompression = ['none', 'gzip', 'snappy', 'lz4'];
  if (config.compression && !validCompression.includes(config.compression)) {
    throw ConfigurationError.invalidConfig(
      'compression',
      `Must be one of: ${validCompression.join(', ')}`
    );
  }
}

/**
 * Create a complete configuration from partial user config
 */
export function createConfig(userConfig: Partial<SentinelConfig>): SentinelConfig {
  const envConfig = loadConfigFromEnv();
  const config = mergeConfig(userConfig, envConfig);
  validateConfig(config);
  return config;
}
