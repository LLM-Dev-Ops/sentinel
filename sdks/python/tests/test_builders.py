"""Tests for builder classes."""

import pytest

from sentinel_sdk.builders import TelemetryEventBuilder
from sentinel_sdk.exceptions import ValidationError


class TestTelemetryEventBuilder:
    """Tests for TelemetryEventBuilder class."""

    def test_basic_build(self):
        """Test basic event building."""
        event = (TelemetryEventBuilder()
            .service("test-service")
            .model("gpt-4")
            .prompt(text="Hello", tokens=5)
            .response(text="Hi", tokens=10, finish_reason="stop")
            .latency(150.5)
            .cost(0.001)
            .build())

        assert event.service_name == "test-service"
        assert event.model == "gpt-4"
        assert event.prompt.text == "Hello"
        assert event.response.text == "Hi"
        assert event.latency_ms == 150.5
        assert event.cost_usd == 0.001

    def test_with_metadata(self):
        """Test building with metadata."""
        event = (TelemetryEventBuilder()
            .service("test-service")
            .model("gpt-4")
            .prompt(text="Hello", tokens=5)
            .response(text="Hi", tokens=10, finish_reason="stop")
            .latency(150.5)
            .cost(0.001)
            .metadata({"key1": "value1"})
            .add_metadata("key2", "value2")
            .build())

        assert event.metadata == {"key1": "value1", "key2": "value2"}

    def test_with_errors(self):
        """Test building with errors."""
        event = (TelemetryEventBuilder()
            .service("test-service")
            .model("gpt-4")
            .prompt(text="Hello", tokens=5)
            .response(text="", tokens=0, finish_reason="error")
            .latency(150.5)
            .cost(0.0)
            .add_error("Error 1")
            .add_error("Error 2")
            .build())

        assert event.errors == ["Error 1", "Error 2"]

    def test_with_trace_info(self):
        """Test building with trace information."""
        event = (TelemetryEventBuilder()
            .service("test-service")
            .model("gpt-4")
            .prompt(text="Hello", tokens=5)
            .response(text="Hi", tokens=10, finish_reason="stop")
            .latency(150.5)
            .cost(0.001)
            .trace_id("trace-123")
            .span_id("span-456")
            .build())

        assert event.trace_id == "trace-123"
        assert event.span_id == "span-456"

    def test_missing_service_name(self):
        """Test build fails without service name."""
        with pytest.raises(ValidationError, match="service_name is required"):
            (TelemetryEventBuilder()
                .model("gpt-4")
                .prompt(text="Hello", tokens=5)
                .response(text="Hi", tokens=10, finish_reason="stop")
                .latency(150.5)
                .cost(0.001)
                .build())

    def test_missing_model(self):
        """Test build fails without model."""
        with pytest.raises(ValidationError, match="model is required"):
            (TelemetryEventBuilder()
                .service("test-service")
                .prompt(text="Hello", tokens=5)
                .response(text="Hi", tokens=10, finish_reason="stop")
                .latency(150.5)
                .cost(0.001)
                .build())

    def test_missing_prompt(self):
        """Test build fails without prompt."""
        with pytest.raises(ValidationError, match="prompt is required"):
            (TelemetryEventBuilder()
                .service("test-service")
                .model("gpt-4")
                .response(text="Hi", tokens=10, finish_reason="stop")
                .latency(150.5)
                .cost(0.001)
                .build())

    def test_missing_response(self):
        """Test build fails without response."""
        with pytest.raises(ValidationError, match="response is required"):
            (TelemetryEventBuilder()
                .service("test-service")
                .model("gpt-4")
                .prompt(text="Hello", tokens=5)
                .latency(150.5)
                .cost(0.001)
                .build())

    def test_method_chaining(self):
        """Test all methods return self for chaining."""
        builder = TelemetryEventBuilder()

        assert builder.service("test") is builder
        assert builder.model("gpt-4") is builder
        assert builder.latency(150.0) is builder
        assert builder.cost(0.001) is builder
