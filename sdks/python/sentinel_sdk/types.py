"""Core type definitions for Sentinel SDK.

This module provides enums and types used throughout the SDK, matching
the Rust core types from sentinel-core/src/types.rs.
"""

from enum import Enum
from typing import Any


class Severity(str, Enum):
    """Severity level for anomalies and alerts.

    Attributes:
        LOW: Low severity - informational
        MEDIUM: Medium severity - warning
        HIGH: High severity - requires attention
        CRITICAL: Critical severity - requires immediate action
    """

    LOW = "low"
    MEDIUM = "medium"
    HIGH = "high"
    CRITICAL = "critical"

    def __str__(self) -> str:
        return self.value

    def __lt__(self, other: Any) -> bool:
        """Compare severities for ordering."""
        if not isinstance(other, Severity):
            return NotImplemented
        order = {
            Severity.LOW: 0,
            Severity.MEDIUM: 1,
            Severity.HIGH: 2,
            Severity.CRITICAL: 3,
        }
        return order[self] < order[other]

    def __le__(self, other: Any) -> bool:
        """Compare severities for ordering."""
        if not isinstance(other, Severity):
            return NotImplemented
        return self == other or self < other

    def __gt__(self, other: Any) -> bool:
        """Compare severities for ordering."""
        if not isinstance(other, Severity):
            return NotImplemented
        return not self <= other

    def __ge__(self, other: Any) -> bool:
        """Compare severities for ordering."""
        if not isinstance(other, Severity):
            return NotImplemented
        return not self < other


class AnomalyType(str, Enum):
    """Type of anomaly detected.

    Attributes:
        LATENCY_SPIKE: Latency spike detected
        THROUGHPUT_DEGRADATION: Throughput degradation
        ERROR_RATE_INCREASE: Error rate increase
        TOKEN_USAGE_SPIKE: Token usage spike
        COST_ANOMALY: Cost anomaly
        INPUT_DRIFT: Input distribution drift
        OUTPUT_DRIFT: Output distribution drift
        CONCEPT_DRIFT: Concept drift
        EMBEDDING_DRIFT: Embedding drift
        HALLUCINATION: Hallucination detected
        QUALITY_DEGRADATION: Quality degradation
        SECURITY_THREAT: Security threat
    """

    LATENCY_SPIKE = "latency_spike"
    THROUGHPUT_DEGRADATION = "throughput_degradation"
    ERROR_RATE_INCREASE = "error_rate_increase"
    TOKEN_USAGE_SPIKE = "token_usage_spike"
    COST_ANOMALY = "cost_anomaly"
    INPUT_DRIFT = "input_drift"
    OUTPUT_DRIFT = "output_drift"
    CONCEPT_DRIFT = "concept_drift"
    EMBEDDING_DRIFT = "embedding_drift"
    HALLUCINATION = "hallucination"
    QUALITY_DEGRADATION = "quality_degradation"
    SECURITY_THREAT = "security_threat"

    def __str__(self) -> str:
        return self.value


class DetectionMethod(str, Enum):
    """Detection method used to identify anomaly.

    Attributes:
        Z_SCORE: Z-Score statistical method
        IQR: Interquartile Range (IQR)
        MAD: Median Absolute Deviation
        CUSUM: Cumulative Sum (CUSUM)
        ISOLATION_FOREST: Isolation Forest ML algorithm
        LSTM_AUTOENCODER: LSTM Autoencoder
        ONE_CLASS_SVM: One-Class SVM
        PSI: Population Stability Index
        KL_DIVERGENCE: KL Divergence
        LLM_CHECK: LLM-Check hallucination detection
        RAG: RAG-based detection
    """

    Z_SCORE = "z_score"
    IQR = "iqr"
    MAD = "mad"
    CUSUM = "cusum"
    ISOLATION_FOREST = "isolation_forest"
    LSTM_AUTOENCODER = "lstm_autoencoder"
    ONE_CLASS_SVM = "one_class_svm"
    PSI = "psi"
    KL_DIVERGENCE = "kl_divergence"
    LLM_CHECK = "llm_check"
    RAG = "rag"

    def __str__(self) -> str:
        return self.value


__all__ = [
    "Severity",
    "AnomalyType",
    "DetectionMethod",
]
