"""Custom exceptions for Sentinel SDK.

This module defines the exception hierarchy for the SDK, providing
specific error types for different failure scenarios.
"""


class SentinelError(Exception):
    """Base exception for all Sentinel SDK errors.

    All SDK-specific exceptions inherit from this base class,
    allowing users to catch all SDK errors with a single except clause.
    """

    def __init__(self, message: str, cause: Exception | None = None) -> None:
        """Initialize SentinelError.

        Args:
            message: Error message
            cause: Optional underlying exception that caused this error
        """
        super().__init__(message)
        self.message = message
        self.cause = cause

    def __str__(self) -> str:
        """Return string representation of the error."""
        if self.cause:
            return f"{self.message} (caused by: {self.cause})"
        return self.message


class ConnectionError(SentinelError):
    """Exception raised when connection to Kafka fails.

    This includes errors during initial connection, reconnection attempts,
    and connection loss during operation.
    """

    pass


class ValidationError(SentinelError):
    """Exception raised when event validation fails.

    This occurs when an event does not meet the required schema,
    has invalid field values, or fails validation constraints.
    """

    pass


class SendError(SentinelError):
    """Exception raised when sending an event fails.

    This includes errors during message serialization, producer errors,
    and timeout errors while sending to Kafka.
    """

    pass


class ConfigurationError(SentinelError):
    """Exception raised when SDK configuration is invalid.

    This occurs when required configuration is missing, invalid,
    or incompatible with the SDK requirements.
    """

    pass


class TimeoutError(SentinelError):
    """Exception raised when an operation times out.

    This occurs when sending events or performing health checks
    takes longer than the configured timeout.
    """

    pass


__all__ = [
    "SentinelError",
    "ConnectionError",
    "ValidationError",
    "SendError",
    "ConfigurationError",
    "TimeoutError",
]
