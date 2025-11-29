/**
 * Enum definitions for LLM-Sentinel SDK
 * These enums match the Rust implementation in sentinel-core/types.rs
 */

/**
 * Severity level for anomalies and alerts
 */
export enum Severity {
  Low = 'low',
  Medium = 'medium',
  High = 'high',
  Critical = 'critical',
}

/**
 * Type of anomaly detected
 */
export enum AnomalyType {
  LatencySpike = 'latency_spike',
  ThroughputDegradation = 'throughput_degradation',
  ErrorRateIncrease = 'error_rate_increase',
  TokenUsageSpike = 'token_usage_spike',
  CostAnomaly = 'cost_anomaly',
  InputDrift = 'input_drift',
  OutputDrift = 'output_drift',
  ConceptDrift = 'concept_drift',
  EmbeddingDrift = 'embedding_drift',
  Hallucination = 'hallucination',
  QualityDegradation = 'quality_degradation',
  SecurityThreat = 'security_threat',
}

/**
 * Detection method used to identify anomaly
 */
export enum DetectionMethod {
  ZScore = 'z_score',
  IQR = 'iqr',
  MAD = 'mad',
  CUSUM = 'cusum',
  IsolationForest = 'isolation_forest',
  LSTMAutoencoder = 'lstm_autoencoder',
  OneClassSVM = 'one_class_svm',
  PSI = 'psi',
  KLDivergence = 'kl_divergence',
  LLMCheck = 'llm_check',
  RAG = 'rag',
}
