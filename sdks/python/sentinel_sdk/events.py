"""Event type definitions for telemetry.

This module defines the core event structures used to send telemetry data
to Sentinel, matching the Rust types from sentinel-core/src/events.rs.
"""

from dataclasses import dataclass, field
from datetime import datetime
from typing import Dict, List, Optional
from uuid import UUID, uuid4


@dataclass
class PromptInfo:
    """Prompt information for LLM requests.

    Attributes:
        text: Prompt text (may be truncated for storage, max 100000 chars)
        tokens: Token count (must be >= 0)
        embedding: Optional embedding vector
    """

    text: str
    tokens: int
    embedding: Optional[List[float]] = None

    def __post_init__(self) -> None:
        """Validate prompt info fields."""
        if self.tokens < 0:
            raise ValueError("tokens must be >= 0")
        if len(self.text) > 100000:
            raise ValueError("text must be <= 100000 characters")

    def to_dict(self) -> Dict:
        """Convert to dictionary for JSON serialization."""
        return {
            "text": self.text,
            "tokens": self.tokens,
            "embedding": self.embedding,
        }


@dataclass
class ResponseInfo:
    """Response information for LLM requests.

    Attributes:
        text: Response text (may be truncated for storage, max 100000 chars)
        tokens: Token count (must be >= 0)
        finish_reason: Finish reason (e.g., "stop", "length", "content_filter")
        embedding: Optional embedding vector
    """

    text: str
    tokens: int
    finish_reason: str
    embedding: Optional[List[float]] = None

    def __post_init__(self) -> None:
        """Validate response info fields."""
        if self.tokens < 0:
            raise ValueError("tokens must be >= 0")
        if len(self.text) > 100000:
            raise ValueError("text must be <= 100000 characters")

    def to_dict(self) -> Dict:
        """Convert to dictionary for JSON serialization."""
        return {
            "text": self.text,
            "tokens": self.tokens,
            "finish_reason": self.finish_reason,
            "embedding": self.embedding,
        }


@dataclass
class TelemetryEvent:
    """Telemetry event from LLM applications.

    This represents a single LLM request/response cycle with associated
    metadata, metrics, and observability information.

    Attributes:
        event_id: Unique event identifier (auto-generated if not provided)
        timestamp: Event timestamp (auto-generated if not provided)
        service_name: Service name identifier
        model: Model identifier (e.g., "gpt-4", "claude-3")
        prompt: Prompt information
        response: Response information
        latency_ms: Request latency in milliseconds (must be >= 0)
        cost_usd: Cost in USD (must be >= 0)
        trace_id: Optional trace ID for distributed tracing
        span_id: Optional span ID
        metadata: Additional metadata key-value pairs
        errors: List of error messages if any
    """

    service_name: str
    model: str
    prompt: PromptInfo
    response: ResponseInfo
    latency_ms: float
    cost_usd: float
    event_id: UUID = field(default_factory=uuid4)
    timestamp: datetime = field(default_factory=datetime.utcnow)
    trace_id: Optional[str] = None
    span_id: Optional[str] = None
    metadata: Dict[str, str] = field(default_factory=dict)
    errors: List[str] = field(default_factory=list)

    def __post_init__(self) -> None:
        """Validate telemetry event fields."""
        if self.latency_ms < 0:
            raise ValueError("latency_ms must be >= 0")
        if self.cost_usd < 0:
            raise ValueError("cost_usd must be >= 0")
        if not self.service_name:
            raise ValueError("service_name cannot be empty")
        if not self.model:
            raise ValueError("model cannot be empty")

    def has_errors(self) -> bool:
        """Check if event has errors.

        Returns:
            True if the event has any errors, False otherwise
        """
        return len(self.errors) > 0

    def total_tokens(self) -> int:
        """Calculate total tokens (prompt + response).

        Returns:
            Total token count
        """
        return self.prompt.tokens + self.response.tokens

    def error_rate(self) -> float:
        """Get error rate (0 or 1 for single event).

        Returns:
            1.0 if event has errors, 0.0 otherwise
        """
        return 1.0 if self.has_errors() else 0.0

    def to_dict(self) -> Dict:
        """Convert to dictionary for JSON serialization.

        Returns:
            Dictionary representation of the event
        """
        return {
            "event_id": str(self.event_id),
            "timestamp": self.timestamp.isoformat() + "Z",
            "service_name": self.service_name,
            "trace_id": self.trace_id,
            "span_id": self.span_id,
            "model": self.model,
            "prompt": self.prompt.to_dict(),
            "response": self.response.to_dict(),
            "latency_ms": self.latency_ms,
            "cost_usd": self.cost_usd,
            "metadata": self.metadata,
            "errors": self.errors,
        }

    @classmethod
    def from_dict(cls, data: Dict) -> "TelemetryEvent":
        """Create TelemetryEvent from dictionary.

        Args:
            data: Dictionary representation of the event

        Returns:
            TelemetryEvent instance
        """
        # Parse nested objects
        prompt = PromptInfo(**data["prompt"])
        response = ResponseInfo(**data["response"])

        # Parse timestamp
        timestamp_str = data["timestamp"]
        if timestamp_str.endswith("Z"):
            timestamp_str = timestamp_str[:-1]
        timestamp = datetime.fromisoformat(timestamp_str)

        # Parse UUID
        event_id = UUID(data["event_id"]) if isinstance(data["event_id"], str) else data["event_id"]

        return cls(
            event_id=event_id,
            timestamp=timestamp,
            service_name=data["service_name"],
            trace_id=data.get("trace_id"),
            span_id=data.get("span_id"),
            model=data["model"],
            prompt=prompt,
            response=response,
            latency_ms=data["latency_ms"],
            cost_usd=data["cost_usd"],
            metadata=data.get("metadata", {}),
            errors=data.get("errors", []),
        )


__all__ = [
    "PromptInfo",
    "ResponseInfo",
    "TelemetryEvent",
]
