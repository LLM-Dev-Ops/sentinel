"""Tests for event classes."""

import pytest
from datetime import datetime
from uuid import UUID

from sentinel_sdk.events import PromptInfo, ResponseInfo, TelemetryEvent


class TestPromptInfo:
    """Tests for PromptInfo class."""

    def test_creation(self):
        """Test basic creation."""
        prompt = PromptInfo(text="Hello", tokens=5)
        assert prompt.text == "Hello"
        assert prompt.tokens == 5
        assert prompt.embedding is None

    def test_with_embedding(self):
        """Test creation with embedding."""
        embedding = [0.1, 0.2, 0.3]
        prompt = PromptInfo(text="Hello", tokens=5, embedding=embedding)
        assert prompt.embedding == embedding

    def test_validation_negative_tokens(self):
        """Test validation fails for negative tokens."""
        with pytest.raises(ValueError, match="tokens must be >= 0"):
            PromptInfo(text="Hello", tokens=-1)

    def test_validation_text_too_long(self):
        """Test validation fails for text exceeding max length."""
        with pytest.raises(ValueError, match="text must be <= 100000 characters"):
            PromptInfo(text="a" * 100001, tokens=5)

    def test_to_dict(self):
        """Test conversion to dictionary."""
        prompt = PromptInfo(text="Hello", tokens=5)
        d = prompt.to_dict()
        assert d["text"] == "Hello"
        assert d["tokens"] == 5
        assert d["embedding"] is None


class TestResponseInfo:
    """Tests for ResponseInfo class."""

    def test_creation(self):
        """Test basic creation."""
        response = ResponseInfo(text="Hi there!", tokens=10, finish_reason="stop")
        assert response.text == "Hi there!"
        assert response.tokens == 10
        assert response.finish_reason == "stop"
        assert response.embedding is None

    def test_validation_negative_tokens(self):
        """Test validation fails for negative tokens."""
        with pytest.raises(ValueError, match="tokens must be >= 0"):
            ResponseInfo(text="Hi", tokens=-1, finish_reason="stop")

    def test_to_dict(self):
        """Test conversion to dictionary."""
        response = ResponseInfo(text="Hi", tokens=10, finish_reason="stop")
        d = response.to_dict()
        assert d["text"] == "Hi"
        assert d["tokens"] == 10
        assert d["finish_reason"] == "stop"


class TestTelemetryEvent:
    """Tests for TelemetryEvent class."""

    def test_creation(self):
        """Test basic event creation."""
        prompt = PromptInfo(text="Hello", tokens=5)
        response = ResponseInfo(text="Hi", tokens=10, finish_reason="stop")

        event = TelemetryEvent(
            service_name="test-service",
            model="gpt-4",
            prompt=prompt,
            response=response,
            latency_ms=150.5,
            cost_usd=0.001
        )

        assert event.service_name == "test-service"
        assert event.model == "gpt-4"
        assert event.prompt == prompt
        assert event.response == response
        assert event.latency_ms == 150.5
        assert event.cost_usd == 0.001
        assert isinstance(event.event_id, UUID)
        assert isinstance(event.timestamp, datetime)

    def test_validation_negative_latency(self):
        """Test validation fails for negative latency."""
        prompt = PromptInfo(text="Hello", tokens=5)
        response = ResponseInfo(text="Hi", tokens=10, finish_reason="stop")

        with pytest.raises(ValueError, match="latency_ms must be >= 0"):
            TelemetryEvent(
                service_name="test-service",
                model="gpt-4",
                prompt=prompt,
                response=response,
                latency_ms=-1.0,
                cost_usd=0.001
            )

    def test_validation_negative_cost(self):
        """Test validation fails for negative cost."""
        prompt = PromptInfo(text="Hello", tokens=5)
        response = ResponseInfo(text="Hi", tokens=10, finish_reason="stop")

        with pytest.raises(ValueError, match="cost_usd must be >= 0"):
            TelemetryEvent(
                service_name="test-service",
                model="gpt-4",
                prompt=prompt,
                response=response,
                latency_ms=150.5,
                cost_usd=-0.001
            )

    def test_has_errors(self):
        """Test has_errors method."""
        prompt = PromptInfo(text="Hello", tokens=5)
        response = ResponseInfo(text="Hi", tokens=10, finish_reason="stop")

        event = TelemetryEvent(
            service_name="test-service",
            model="gpt-4",
            prompt=prompt,
            response=response,
            latency_ms=150.5,
            cost_usd=0.001
        )

        assert not event.has_errors()

        event.errors.append("Some error")
        assert event.has_errors()

    def test_total_tokens(self):
        """Test total_tokens calculation."""
        prompt = PromptInfo(text="Hello", tokens=5)
        response = ResponseInfo(text="Hi", tokens=10, finish_reason="stop")

        event = TelemetryEvent(
            service_name="test-service",
            model="gpt-4",
            prompt=prompt,
            response=response,
            latency_ms=150.5,
            cost_usd=0.001
        )

        assert event.total_tokens() == 15

    def test_error_rate(self):
        """Test error_rate calculation."""
        prompt = PromptInfo(text="Hello", tokens=5)
        response = ResponseInfo(text="Hi", tokens=10, finish_reason="stop")

        event = TelemetryEvent(
            service_name="test-service",
            model="gpt-4",
            prompt=prompt,
            response=response,
            latency_ms=150.5,
            cost_usd=0.001
        )

        assert event.error_rate() == 0.0

        event.errors.append("Error")
        assert event.error_rate() == 1.0

    def test_to_dict(self):
        """Test conversion to dictionary."""
        prompt = PromptInfo(text="Hello", tokens=5)
        response = ResponseInfo(text="Hi", tokens=10, finish_reason="stop")

        event = TelemetryEvent(
            service_name="test-service",
            model="gpt-4",
            prompt=prompt,
            response=response,
            latency_ms=150.5,
            cost_usd=0.001,
            metadata={"key": "value"}
        )

        d = event.to_dict()
        assert d["service_name"] == "test-service"
        assert d["model"] == "gpt-4"
        assert d["latency_ms"] == 150.5
        assert d["cost_usd"] == 0.001
        assert d["metadata"] == {"key": "value"}
        assert isinstance(d["event_id"], str)
        assert "T" in d["timestamp"]

    def test_from_dict(self):
        """Test creation from dictionary."""
        data = {
            "event_id": "12345678-1234-1234-1234-123456789012",
            "timestamp": "2024-01-01T00:00:00.000000Z",
            "service_name": "test-service",
            "model": "gpt-4",
            "prompt": {"text": "Hello", "tokens": 5, "embedding": None},
            "response": {"text": "Hi", "tokens": 10, "finish_reason": "stop", "embedding": None},
            "latency_ms": 150.5,
            "cost_usd": 0.001,
            "trace_id": None,
            "span_id": None,
            "metadata": {},
            "errors": []
        }

        event = TelemetryEvent.from_dict(data)
        assert event.service_name == "test-service"
        assert event.model == "gpt-4"
        assert event.prompt.text == "Hello"
        assert event.response.text == "Hi"
