"""Sentinel SDK client implementations.

This module provides both synchronous and asynchronous clients for sending
telemetry events to Sentinel via Kafka.
"""

import atexit
import json
import logging
import time
from typing import Dict, List, Optional

from .config import SentinelConfig
from .events import PromptInfo, ResponseInfo, TelemetryEvent
from .exceptions import ConnectionError, SendError, TimeoutError, ValidationError

logger = logging.getLogger(__name__)


class SentinelClient:
    """Synchronous client for sending telemetry events to Sentinel.

    This client uses kafka-python to send events to Kafka with retry logic,
    connection pooling, and graceful shutdown support.

    Example:
        >>> client = SentinelClient(brokers=["localhost:9092"])
        >>> client.track(
        ...     service="my-service",
        ...     model="gpt-4",
        ...     prompt="Hello",
        ...     response="Hi there!",
        ...     latency_ms=150.5,
        ...     prompt_tokens=5,
        ...     completion_tokens=10,
        ...     cost_usd=0.001
        ... )
        >>> client.close()
    """

    def __init__(
        self,
        brokers: Optional[List[str]] = None,
        config: Optional[SentinelConfig] = None,
        **kwargs,
    ) -> None:
        """Initialize the Sentinel client.

        Args:
            brokers: List of Kafka broker addresses (e.g., ["localhost:9092"])
            config: SentinelConfig instance (if provided, brokers and kwargs are ignored)
            **kwargs: Additional configuration options passed to SentinelConfig

        Raises:
            ConnectionError: If connection to Kafka fails
        """
        # Determine configuration
        if config is not None:
            self._config = config
        elif brokers is not None:
            self._config = SentinelConfig(brokers=brokers, **kwargs)
        else:
            self._config = SentinelConfig.from_env(**kwargs)

        self._producer: Optional[object] = None
        self._closed = False

        # Register cleanup handler
        atexit.register(self.close)

        logger.info(
            f"Initialized Sentinel client with brokers: {self._config.brokers}, "
            f"topic: {self._config.topic}"
        )

    def _get_producer(self):
        """Get or create the Kafka producer.

        Returns:
            Kafka producer instance

        Raises:
            ConnectionError: If producer creation fails
        """
        if self._producer is None:
            try:
                from kafka import KafkaProducer

                kafka_config = self._config.to_kafka_config()
                self._producer = KafkaProducer(
                    value_serializer=lambda v: json.dumps(v).encode("utf-8"),
                    **kafka_config,
                )
                logger.info("Kafka producer created successfully")
            except ImportError as e:
                raise ConnectionError(
                    "kafka-python is not installed. Install it with: pip install kafka-python",
                    cause=e,
                )
            except Exception as e:
                raise ConnectionError(f"Failed to create Kafka producer: {e}", cause=e)

        return self._producer

    def send(self, event: TelemetryEvent, timeout: Optional[float] = None) -> None:
        """Send a telemetry event to Sentinel.

        Args:
            event: TelemetryEvent to send
            timeout: Optional timeout in seconds (default: use config timeout)

        Raises:
            SendError: If sending fails
            TimeoutError: If sending times out
            ValidationError: If event validation fails
        """
        if self._closed:
            raise SendError("Client is closed")

        try:
            # Validate event
            event_dict = event.to_dict()

            # Get producer
            producer = self._get_producer()

            # Send to Kafka
            timeout_ms = (
                int(timeout * 1000) if timeout else self._config.request_timeout_ms
            )
            future = producer.send(self._config.topic, value=event_dict)

            # Wait for result
            try:
                future.get(timeout=timeout_ms / 1000.0)
                logger.debug(f"Sent event {event.event_id} to Kafka")
            except Exception as e:
                if "timeout" in str(e).lower():
                    raise TimeoutError(f"Sending event timed out after {timeout_ms}ms", cause=e)
                raise SendError(f"Failed to send event: {e}", cause=e)

        except (SendError, TimeoutError, ValidationError):
            raise
        except Exception as e:
            raise SendError(f"Unexpected error sending event: {e}", cause=e)

    def track(
        self,
        service: str,
        model: str,
        prompt: str,
        response: str,
        latency_ms: float,
        prompt_tokens: int,
        completion_tokens: int,
        cost_usd: float,
        finish_reason: str = "stop",
        trace_id: Optional[str] = None,
        span_id: Optional[str] = None,
        metadata: Optional[Dict[str, str]] = None,
        errors: Optional[List[str]] = None,
        prompt_embedding: Optional[List[float]] = None,
        response_embedding: Optional[List[float]] = None,
        timeout: Optional[float] = None,
    ) -> None:
        """Convenience method to track an LLM request.

        This is a simplified interface for sending telemetry events without
        manually constructing event objects.

        Args:
            service: Service name
            model: Model identifier (e.g., "gpt-4", "claude-3")
            prompt: Prompt text
            response: Response text
            latency_ms: Request latency in milliseconds
            prompt_tokens: Number of tokens in prompt
            completion_tokens: Number of tokens in response
            cost_usd: Cost in USD
            finish_reason: Finish reason (default: "stop")
            trace_id: Optional trace ID
            span_id: Optional span ID
            metadata: Optional metadata dictionary
            errors: Optional list of error messages
            prompt_embedding: Optional prompt embedding vector
            response_embedding: Optional response embedding vector
            timeout: Optional timeout in seconds

        Raises:
            SendError: If sending fails
            TimeoutError: If sending times out
            ValidationError: If event validation fails
        """
        event = TelemetryEvent(
            service_name=service,
            model=model,
            prompt=PromptInfo(
                text=prompt,
                tokens=prompt_tokens,
                embedding=prompt_embedding,
            ),
            response=ResponseInfo(
                text=response,
                tokens=completion_tokens,
                finish_reason=finish_reason,
                embedding=response_embedding,
            ),
            latency_ms=latency_ms,
            cost_usd=cost_usd,
            trace_id=trace_id,
            span_id=span_id,
            metadata=metadata or {},
            errors=errors or [],
        )

        self.send(event, timeout=timeout)

    def flush(self, timeout: Optional[float] = None) -> None:
        """Flush pending messages.

        Args:
            timeout: Optional timeout in seconds

        Raises:
            SendError: If flush fails
        """
        if self._producer is None or self._closed:
            return

        try:
            timeout_sec = timeout if timeout else self._config.request_timeout_ms / 1000.0
            self._producer.flush(timeout=timeout_sec)
            logger.debug("Flushed pending messages")
        except Exception as e:
            raise SendError(f"Failed to flush messages: {e}", cause=e)

    def close(self) -> None:
        """Close the client and release resources.

        This method is called automatically on exit, but can also be called
        manually for explicit cleanup.
        """
        if self._closed:
            return

        try:
            if self._producer is not None:
                self._producer.flush()
                self._producer.close()
                self._producer = None
                logger.info("Kafka producer closed")
        except Exception as e:
            logger.error(f"Error closing producer: {e}")
        finally:
            self._closed = True

    def __enter__(self) -> "SentinelClient":
        """Context manager entry."""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Context manager exit."""
        self.close()


class AsyncSentinelClient:
    """Asynchronous client for sending telemetry events to Sentinel.

    This client uses aiokafka to send events to Kafka with async/await support,
    retry logic, connection pooling, and graceful shutdown.

    Example:
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
    """

    def __init__(
        self,
        brokers: Optional[List[str]] = None,
        config: Optional[SentinelConfig] = None,
        **kwargs,
    ) -> None:
        """Initialize the async Sentinel client.

        Args:
            brokers: List of Kafka broker addresses (e.g., ["localhost:9092"])
            config: SentinelConfig instance (if provided, brokers and kwargs are ignored)
            **kwargs: Additional configuration options passed to SentinelConfig

        Raises:
            ConnectionError: If configuration is invalid
        """
        # Determine configuration
        if config is not None:
            self._config = config
        elif brokers is not None:
            self._config = SentinelConfig(brokers=brokers, **kwargs)
        else:
            self._config = SentinelConfig.from_env(**kwargs)

        self._producer: Optional[object] = None
        self._closed = False

        logger.info(
            f"Initialized async Sentinel client with brokers: {self._config.brokers}, "
            f"topic: {self._config.topic}"
        )

    async def start(self) -> None:
        """Start the async producer.

        Raises:
            ConnectionError: If producer creation fails
        """
        if self._producer is not None:
            return

        try:
            from aiokafka import AIOKafkaProducer

            kafka_config = self._config.to_aiokafka_config()
            self._producer = AIOKafkaProducer(
                value_serializer=lambda v: json.dumps(v).encode("utf-8"),
                **kafka_config,
            )
            await self._producer.start()
            logger.info("Async Kafka producer started successfully")
        except ImportError as e:
            raise ConnectionError(
                "aiokafka is not installed. Install it with: pip install aiokafka",
                cause=e,
            )
        except Exception as e:
            raise ConnectionError(f"Failed to start Kafka producer: {e}", cause=e)

    async def send(self, event: TelemetryEvent, timeout: Optional[float] = None) -> None:
        """Send a telemetry event to Sentinel.

        Args:
            event: TelemetryEvent to send
            timeout: Optional timeout in seconds

        Raises:
            SendError: If sending fails
            TimeoutError: If sending times out
            ValidationError: If event validation fails
        """
        if self._closed:
            raise SendError("Client is closed")

        if self._producer is None:
            await self.start()

        try:
            # Validate event
            event_dict = event.to_dict()

            # Send to Kafka
            timeout_sec = timeout if timeout else self._config.request_timeout_ms / 1000.0

            try:
                await self._producer.send_and_wait(
                    self._config.topic,
                    value=event_dict,
                )
                logger.debug(f"Sent event {event.event_id} to Kafka")
            except TimeoutError as e:
                raise TimeoutError(f"Sending event timed out", cause=e)
            except Exception as e:
                raise SendError(f"Failed to send event: {e}", cause=e)

        except (SendError, TimeoutError, ValidationError):
            raise
        except Exception as e:
            raise SendError(f"Unexpected error sending event: {e}", cause=e)

    async def track(
        self,
        service: str,
        model: str,
        prompt: str,
        response: str,
        latency_ms: float,
        prompt_tokens: int,
        completion_tokens: int,
        cost_usd: float,
        finish_reason: str = "stop",
        trace_id: Optional[str] = None,
        span_id: Optional[str] = None,
        metadata: Optional[Dict[str, str]] = None,
        errors: Optional[List[str]] = None,
        prompt_embedding: Optional[List[float]] = None,
        response_embedding: Optional[List[float]] = None,
        timeout: Optional[float] = None,
    ) -> None:
        """Convenience method to track an LLM request.

        This is a simplified interface for sending telemetry events without
        manually constructing event objects.

        Args:
            service: Service name
            model: Model identifier (e.g., "gpt-4", "claude-3")
            prompt: Prompt text
            response: Response text
            latency_ms: Request latency in milliseconds
            prompt_tokens: Number of tokens in prompt
            completion_tokens: Number of tokens in response
            cost_usd: Cost in USD
            finish_reason: Finish reason (default: "stop")
            trace_id: Optional trace ID
            span_id: Optional span ID
            metadata: Optional metadata dictionary
            errors: Optional list of error messages
            prompt_embedding: Optional prompt embedding vector
            response_embedding: Optional response embedding vector
            timeout: Optional timeout in seconds

        Raises:
            SendError: If sending fails
            TimeoutError: If sending times out
            ValidationError: If event validation fails
        """
        event = TelemetryEvent(
            service_name=service,
            model=model,
            prompt=PromptInfo(
                text=prompt,
                tokens=prompt_tokens,
                embedding=prompt_embedding,
            ),
            response=ResponseInfo(
                text=response,
                tokens=completion_tokens,
                finish_reason=finish_reason,
                embedding=response_embedding,
            ),
            latency_ms=latency_ms,
            cost_usd=cost_usd,
            trace_id=trace_id,
            span_id=span_id,
            metadata=metadata or {},
            errors=errors or [],
        )

        await self.send(event, timeout=timeout)

    async def flush(self) -> None:
        """Flush pending messages.

        Raises:
            SendError: If flush fails
        """
        if self._producer is None or self._closed:
            return

        try:
            await self._producer.flush()
            logger.debug("Flushed pending messages")
        except Exception as e:
            raise SendError(f"Failed to flush messages: {e}", cause=e)

    async def close(self) -> None:
        """Close the client and release resources."""
        if self._closed:
            return

        try:
            if self._producer is not None:
                await self._producer.stop()
                self._producer = None
                logger.info("Async Kafka producer stopped")
        except Exception as e:
            logger.error(f"Error stopping producer: {e}")
        finally:
            self._closed = True

    async def __aenter__(self) -> "AsyncSentinelClient":
        """Async context manager entry."""
        await self.start()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        """Async context manager exit."""
        await self.close()


__all__ = ["SentinelClient", "AsyncSentinelClient"]
