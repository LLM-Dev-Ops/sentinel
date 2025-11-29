"""LLM Sentinel Python SDK.

This SDK allows you to easily send LLM telemetry events to Sentinel
for anomaly detection and monitoring.

Example Usage:
    >>> from sentinel_sdk import SentinelClient
    >>>
    >>> # Create a client
    >>> client = SentinelClient(brokers=["localhost:9092"])
    >>>
    >>> # Track an LLM request
    >>> client.track(
    ...     service="my-service",
    ...     model="gpt-4",
    ...     prompt="Hello, how are you?",
    ...     response="I'm doing well, thank you!",
    ...     latency_ms=150.5,
    ...     prompt_tokens=5,
    ...     completion_tokens=10,
    ...     cost_usd=0.001
    ... )
    >>>
    >>> # Close the client
    >>> client.close()

Async Usage:
    >>> from sentinel_sdk import AsyncSentinelClient
    >>>
    >>> async with AsyncSentinelClient(brokers=["localhost:9092"]) as client:
    ...     await client.track(
    ...         service="my-service",
    ...         model="gpt-4",
    ...         prompt="Hello",
    ...         response="Hi there!",
    ...         latency_ms=150.5,
    ...         prompt_tokens=5,
    ...         completion_tokens=10,
    ...         cost_usd=0.001
    ...     )

Builder Pattern:
    >>> from sentinel_sdk import SentinelClient, TelemetryEventBuilder
    >>>
    >>> client = SentinelClient(brokers=["localhost:9092"])
    >>>
    >>> event = (TelemetryEventBuilder()
    ...     .service("my-service")
    ...     .model("gpt-4")
    ...     .prompt(text="Hello", tokens=5)
    ...     .response(text="Hi!", tokens=10, finish_reason="stop")
    ...     .latency(150.5)
    ...     .cost(0.001)
    ...     .metadata({"region": "us-east-1"})
    ...     .build())
    >>>
    >>> client.send(event)
    >>> client.close()
"""

from .builders import TelemetryEventBuilder
from .client import AsyncSentinelClient, SentinelClient
from .config import SentinelConfig
from .events import PromptInfo, ResponseInfo, TelemetryEvent
from .exceptions import (
    ConfigurationError,
    ConnectionError,
    SendError,
    SentinelError,
    TimeoutError,
    ValidationError,
)
from .types import AnomalyType, DetectionMethod, Severity

__version__ = "0.1.0"

__all__ = [
    # Client classes
    "SentinelClient",
    "AsyncSentinelClient",
    # Event classes
    "TelemetryEvent",
    "PromptInfo",
    "ResponseInfo",
    # Builder classes
    "TelemetryEventBuilder",
    # Configuration
    "SentinelConfig",
    # Type enums
    "Severity",
    "AnomalyType",
    "DetectionMethod",
    # Exceptions
    "SentinelError",
    "ConnectionError",
    "ValidationError",
    "SendError",
    "ConfigurationError",
    "TimeoutError",
    # Version
    "__version__",
]
