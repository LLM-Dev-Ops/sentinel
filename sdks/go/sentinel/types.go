// Package sentinel provides a Go SDK for sending LLM telemetry events to Sentinel for anomaly detection.
package sentinel

// Severity represents the severity level for anomalies and alerts.
type Severity string

const (
	// SeverityLow indicates informational severity
	SeverityLow Severity = "low"
	// SeverityMedium indicates warning severity
	SeverityMedium Severity = "medium"
	// SeverityHigh indicates a severity that requires attention
	SeverityHigh Severity = "high"
	// SeverityCritical indicates a severity that requires immediate action
	SeverityCritical Severity = "critical"
)

// String returns the string representation of the severity.
func (s Severity) String() string {
	return string(s)
}

// AnomalyType represents the type of anomaly detected.
type AnomalyType string

const (
	// AnomalyTypeLatencySpike indicates a latency spike was detected
	AnomalyTypeLatencySpike AnomalyType = "latency_spike"
	// AnomalyTypeThroughputDegradation indicates throughput degradation
	AnomalyTypeThroughputDegradation AnomalyType = "throughput_degradation"
	// AnomalyTypeErrorRateIncrease indicates an error rate increase
	AnomalyTypeErrorRateIncrease AnomalyType = "error_rate_increase"
	// AnomalyTypeTokenUsageSpike indicates a token usage spike
	AnomalyTypeTokenUsageSpike AnomalyType = "token_usage_spike"
	// AnomalyTypeCostAnomaly indicates a cost anomaly
	AnomalyTypeCostAnomaly AnomalyType = "cost_anomaly"
	// AnomalyTypeInputDrift indicates input distribution drift
	AnomalyTypeInputDrift AnomalyType = "input_drift"
	// AnomalyTypeOutputDrift indicates output distribution drift
	AnomalyTypeOutputDrift AnomalyType = "output_drift"
	// AnomalyTypeConceptDrift indicates concept drift
	AnomalyTypeConceptDrift AnomalyType = "concept_drift"
	// AnomalyTypeEmbeddingDrift indicates embedding drift
	AnomalyTypeEmbeddingDrift AnomalyType = "embedding_drift"
	// AnomalyTypeHallucination indicates hallucination was detected
	AnomalyTypeHallucination AnomalyType = "hallucination"
	// AnomalyTypeQualityDegradation indicates quality degradation
	AnomalyTypeQualityDegradation AnomalyType = "quality_degradation"
	// AnomalyTypeSecurityThreat indicates a security threat
	AnomalyTypeSecurityThreat AnomalyType = "security_threat"
)

// String returns the string representation of the anomaly type.
func (a AnomalyType) String() string {
	return string(a)
}

// DetectionMethod represents the method used to identify an anomaly.
type DetectionMethod string

const (
	// DetectionMethodZScore indicates Z-Score statistical method
	DetectionMethodZScore DetectionMethod = "z_score"
	// DetectionMethodIQR indicates Interquartile Range method
	DetectionMethodIQR DetectionMethod = "iqr"
	// DetectionMethodMAD indicates Median Absolute Deviation method
	DetectionMethodMAD DetectionMethod = "mad"
	// DetectionMethodCUSUM indicates Cumulative Sum method
	DetectionMethodCUSUM DetectionMethod = "cusum"
	// DetectionMethodIsolationForest indicates Isolation Forest ML algorithm
	DetectionMethodIsolationForest DetectionMethod = "isolation_forest"
	// DetectionMethodLSTMAutoencoder indicates LSTM Autoencoder
	DetectionMethodLSTMAutoencoder DetectionMethod = "lstm_autoencoder"
	// DetectionMethodOneClassSVM indicates One-Class SVM
	DetectionMethodOneClassSVM DetectionMethod = "one_class_svm"
	// DetectionMethodPSI indicates Population Stability Index
	DetectionMethodPSI DetectionMethod = "psi"
	// DetectionMethodKLDivergence indicates KL Divergence
	DetectionMethodKLDivergence DetectionMethod = "kl_divergence"
	// DetectionMethodLLMCheck indicates LLM-Check hallucination detection
	DetectionMethodLLMCheck DetectionMethod = "llm_check"
	// DetectionMethodRAG indicates RAG-based detection
	DetectionMethodRAG DetectionMethod = "rag"
)

// String returns the string representation of the detection method.
func (d DetectionMethod) String() string {
	return string(d)
}
