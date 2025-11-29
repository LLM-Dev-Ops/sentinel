"""Fluent builders for constructing telemetry events.

This module provides builder classes that allow for easy and readable
construction of telemetry events using method chaining.
"""

from datetime import datetime
from typing import Dict, List, Optional
from uuid import UUID, uuid4

from .events import PromptInfo, ResponseInfo, TelemetryEvent
from .exceptions import ValidationError


class TelemetryEventBuilder:
    """Fluent builder for constructing TelemetryEvent instances.

    This builder provides a convenient, readable way to construct
    telemetry events using method chaining.

    Example:
        >>> event = (TelemetryEventBuilder()
        ...     .service("my-service")
        ...     .model("gpt-4")
        ...     .prompt(text="Hello", tokens=5)
        ...     .response(text="Hi!", tokens=10, finish_reason="stop")
        ...     .latency(150.5)
        ...     .cost(0.001)
        ...     .build())
    """

    def __init__(self) -> None:
        """Initialize the builder with default values."""
        self._event_id: UUID = uuid4()
        self._timestamp: datetime = datetime.utcnow()
        self._service_name: Optional[str] = None
        self._model: Optional[str] = None
        self._trace_id: Optional[str] = None
        self._span_id: Optional[str] = None
        self._prompt: Optional[PromptInfo] = None
        self._response: Optional[ResponseInfo] = None
        self._latency_ms: Optional[float] = None
        self._cost_usd: Optional[float] = None
        self._metadata: Dict[str, str] = {}
        self._errors: List[str] = []

    def event_id(self, event_id: UUID) -> "TelemetryEventBuilder":
        """Set the event ID.

        Args:
            event_id: UUID for the event

        Returns:
            Self for method chaining
        """
        self._event_id = event_id
        return self

    def timestamp(self, timestamp: datetime) -> "TelemetryEventBuilder":
        """Set the timestamp.

        Args:
            timestamp: Event timestamp

        Returns:
            Self for method chaining
        """
        self._timestamp = timestamp
        return self

    def service(self, service_name: str) -> "TelemetryEventBuilder":
        """Set the service name.

        Args:
            service_name: Name of the service

        Returns:
            Self for method chaining
        """
        self._service_name = service_name
        return self

    def model(self, model: str) -> "TelemetryEventBuilder":
        """Set the model identifier.

        Args:
            model: Model identifier (e.g., "gpt-4", "claude-3")

        Returns:
            Self for method chaining
        """
        self._model = model
        return self

    def trace_id(self, trace_id: str) -> "TelemetryEventBuilder":
        """Set the trace ID for distributed tracing.

        Args:
            trace_id: Trace identifier

        Returns:
            Self for method chaining
        """
        self._trace_id = trace_id
        return self

    def span_id(self, span_id: str) -> "TelemetryEventBuilder":
        """Set the span ID.

        Args:
            span_id: Span identifier

        Returns:
            Self for method chaining
        """
        self._span_id = span_id
        return self

    def prompt(
        self,
        text: str,
        tokens: int,
        embedding: Optional[List[float]] = None,
    ) -> "TelemetryEventBuilder":
        """Set the prompt information.

        Args:
            text: Prompt text
            tokens: Token count
            embedding: Optional embedding vector

        Returns:
            Self for method chaining
        """
        self._prompt = PromptInfo(text=text, tokens=tokens, embedding=embedding)
        return self

    def prompt_info(self, prompt: PromptInfo) -> "TelemetryEventBuilder":
        """Set the prompt information using a PromptInfo object.

        Args:
            prompt: PromptInfo instance

        Returns:
            Self for method chaining
        """
        self._prompt = prompt
        return self

    def response(
        self,
        text: str,
        tokens: int,
        finish_reason: str,
        embedding: Optional[List[float]] = None,
    ) -> "TelemetryEventBuilder":
        """Set the response information.

        Args:
            text: Response text
            tokens: Token count
            finish_reason: Finish reason (e.g., "stop", "length")
            embedding: Optional embedding vector

        Returns:
            Self for method chaining
        """
        self._response = ResponseInfo(
            text=text,
            tokens=tokens,
            finish_reason=finish_reason,
            embedding=embedding,
        )
        return self

    def response_info(self, response: ResponseInfo) -> "TelemetryEventBuilder":
        """Set the response information using a ResponseInfo object.

        Args:
            response: ResponseInfo instance

        Returns:
            Self for method chaining
        """
        self._response = response
        return self

    def latency(self, latency_ms: float) -> "TelemetryEventBuilder":
        """Set the request latency.

        Args:
            latency_ms: Latency in milliseconds

        Returns:
            Self for method chaining
        """
        self._latency_ms = latency_ms
        return self

    def cost(self, cost_usd: float) -> "TelemetryEventBuilder":
        """Set the cost.

        Args:
            cost_usd: Cost in USD

        Returns:
            Self for method chaining
        """
        self._cost_usd = cost_usd
        return self

    def metadata(self, metadata: Dict[str, str]) -> "TelemetryEventBuilder":
        """Set metadata (replaces existing metadata).

        Args:
            metadata: Metadata dictionary

        Returns:
            Self for method chaining
        """
        self._metadata = metadata.copy()
        return self

    def add_metadata(self, key: str, value: str) -> "TelemetryEventBuilder":
        """Add a single metadata entry.

        Args:
            key: Metadata key
            value: Metadata value

        Returns:
            Self for method chaining
        """
        self._metadata[key] = value
        return self

    def errors(self, errors: List[str]) -> "TelemetryEventBuilder":
        """Set errors (replaces existing errors).

        Args:
            errors: List of error messages

        Returns:
            Self for method chaining
        """
        self._errors = errors.copy()
        return self

    def add_error(self, error: str) -> "TelemetryEventBuilder":
        """Add a single error message.

        Args:
            error: Error message

        Returns:
            Self for method chaining
        """
        self._errors.append(error)
        return self

    def build(self) -> TelemetryEvent:
        """Build the TelemetryEvent.

        Returns:
            Constructed TelemetryEvent instance

        Raises:
            ValidationError: If required fields are missing
        """
        # Validate required fields
        if self._service_name is None:
            raise ValidationError("service_name is required")
        if self._model is None:
            raise ValidationError("model is required")
        if self._prompt is None:
            raise ValidationError("prompt is required")
        if self._response is None:
            raise ValidationError("response is required")
        if self._latency_ms is None:
            raise ValidationError("latency_ms is required")
        if self._cost_usd is None:
            raise ValidationError("cost_usd is required")

        return TelemetryEvent(
            event_id=self._event_id,
            timestamp=self._timestamp,
            service_name=self._service_name,
            model=self._model,
            trace_id=self._trace_id,
            span_id=self._span_id,
            prompt=self._prompt,
            response=self._response,
            latency_ms=self._latency_ms,
            cost_usd=self._cost_usd,
            metadata=self._metadata,
            errors=self._errors,
        )


__all__ = ["TelemetryEventBuilder"]
