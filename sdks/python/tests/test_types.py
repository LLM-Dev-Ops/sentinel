"""Tests for type definitions."""

import pytest

from sentinel_sdk.types import AnomalyType, DetectionMethod, Severity


class TestSeverity:
    """Tests for Severity enum."""

    def test_values(self):
        """Test enum values."""
        assert Severity.LOW.value == "low"
        assert Severity.MEDIUM.value == "medium"
        assert Severity.HIGH.value == "high"
        assert Severity.CRITICAL.value == "critical"

    def test_ordering(self):
        """Test severity ordering."""
        assert Severity.LOW < Severity.MEDIUM
        assert Severity.MEDIUM < Severity.HIGH
        assert Severity.HIGH < Severity.CRITICAL
        assert Severity.CRITICAL > Severity.LOW

    def test_string_conversion(self):
        """Test string conversion."""
        assert str(Severity.LOW) == "low"
        assert str(Severity.CRITICAL) == "critical"


class TestAnomalyType:
    """Tests for AnomalyType enum."""

    def test_values(self):
        """Test enum values."""
        assert AnomalyType.LATENCY_SPIKE.value == "latency_spike"
        assert AnomalyType.COST_ANOMALY.value == "cost_anomaly"
        assert AnomalyType.HALLUCINATION.value == "hallucination"

    def test_string_conversion(self):
        """Test string conversion."""
        assert str(AnomalyType.LATENCY_SPIKE) == "latency_spike"
        assert str(AnomalyType.TOKEN_USAGE_SPIKE) == "token_usage_spike"


class TestDetectionMethod:
    """Tests for DetectionMethod enum."""

    def test_values(self):
        """Test enum values."""
        assert DetectionMethod.Z_SCORE.value == "z_score"
        assert DetectionMethod.IQR.value == "iqr"
        assert DetectionMethod.MAD.value == "mad"

    def test_string_conversion(self):
        """Test string conversion."""
        assert str(DetectionMethod.Z_SCORE) == "z_score"
        assert str(DetectionMethod.CUSUM) == "cusum"
