"""Configuration for Sentinel SDK.

This module provides configuration classes and utilities for the SDK,
including support for environment variables and sensible defaults.
"""

import os
from dataclasses import dataclass, field
from typing import Dict, List, Optional

from .exceptions import ConfigurationError


@dataclass
class SentinelConfig:
    """Configuration for Sentinel SDK.

    This class encapsulates all configuration options for the SDK,
    with support for environment variables and sensible defaults.

    Attributes:
        brokers: List of Kafka broker addresses (e.g., ["localhost:9092"])
        topic: Kafka topic name for telemetry events
        client_id: Client ID for Kafka producer
        compression_type: Compression type for messages ("none", "gzip", "snappy", "lz4", "zstd")
        acks: Number of acknowledgments required ("all", "1", "0")
        retries: Number of retry attempts
        retry_backoff_ms: Backoff time in milliseconds between retries
        request_timeout_ms: Request timeout in milliseconds
        linger_ms: Time to wait before sending batch
        batch_size: Maximum batch size in bytes
        max_in_flight_requests: Maximum in-flight requests per connection
        enable_idempotence: Enable idempotent producer
        buffer_memory: Total memory for buffering
        max_request_size: Maximum size of a request
        metadata_max_age_ms: Metadata cache expiry time
        security_protocol: Security protocol ("PLAINTEXT", "SSL", "SASL_PLAINTEXT", "SASL_SSL")
        sasl_mechanism: SASL mechanism if using SASL
        sasl_username: SASL username
        sasl_password: SASL password
        ssl_ca_location: SSL CA certificate location
        ssl_cert_location: SSL certificate location
        ssl_key_location: SSL key location
        additional_config: Additional Kafka configuration
    """

    brokers: List[str]
    topic: str = "llm-telemetry"
    client_id: str = "sentinel-sdk-python"
    compression_type: str = "gzip"
    acks: str = "all"
    retries: int = 3
    retry_backoff_ms: int = 100
    request_timeout_ms: int = 30000
    linger_ms: int = 10
    batch_size: int = 16384
    max_in_flight_requests: int = 5
    enable_idempotence: bool = True
    buffer_memory: int = 33554432  # 32MB
    max_request_size: int = 1048576  # 1MB
    metadata_max_age_ms: int = 300000  # 5 minutes
    security_protocol: str = "PLAINTEXT"
    sasl_mechanism: Optional[str] = None
    sasl_username: Optional[str] = None
    sasl_password: Optional[str] = None
    ssl_ca_location: Optional[str] = None
    ssl_cert_location: Optional[str] = None
    ssl_key_location: Optional[str] = None
    additional_config: Dict[str, str] = field(default_factory=dict)

    def __post_init__(self) -> None:
        """Validate configuration after initialization."""
        if not self.brokers:
            raise ConfigurationError("brokers cannot be empty")
        if not self.topic:
            raise ConfigurationError("topic cannot be empty")
        if self.acks not in ["all", "1", "0", "-1"]:
            raise ConfigurationError(f"Invalid acks value: {self.acks}")
        if self.compression_type not in ["none", "gzip", "snappy", "lz4", "zstd"]:
            raise ConfigurationError(f"Invalid compression_type: {self.compression_type}")
        if self.security_protocol not in ["PLAINTEXT", "SSL", "SASL_PLAINTEXT", "SASL_SSL"]:
            raise ConfigurationError(f"Invalid security_protocol: {self.security_protocol}")

    @classmethod
    def from_env(cls, **kwargs) -> "SentinelConfig":
        """Create configuration from environment variables.

        Environment variables:
            SENTINEL_BROKERS: Comma-separated list of brokers
            SENTINEL_TOPIC: Kafka topic name
            SENTINEL_CLIENT_ID: Client ID
            SENTINEL_COMPRESSION_TYPE: Compression type
            SENTINEL_ACKS: Acknowledgment mode
            SENTINEL_RETRIES: Number of retries
            SENTINEL_SECURITY_PROTOCOL: Security protocol
            SENTINEL_SASL_MECHANISM: SASL mechanism
            SENTINEL_SASL_USERNAME: SASL username
            SENTINEL_SASL_PASSWORD: SASL password
            SENTINEL_SSL_CA_LOCATION: SSL CA certificate location
            SENTINEL_SSL_CERT_LOCATION: SSL certificate location
            SENTINEL_SSL_KEY_LOCATION: SSL key location

        Args:
            **kwargs: Additional configuration overrides

        Returns:
            SentinelConfig instance
        """
        config_dict = {}

        # Parse brokers from environment
        if "brokers" not in kwargs:
            brokers_str = os.getenv("SENTINEL_BROKERS")
            if brokers_str:
                config_dict["brokers"] = [b.strip() for b in brokers_str.split(",")]
            else:
                config_dict["brokers"] = ["localhost:9092"]

        # Parse other environment variables
        env_mappings = {
            "SENTINEL_TOPIC": "topic",
            "SENTINEL_CLIENT_ID": "client_id",
            "SENTINEL_COMPRESSION_TYPE": "compression_type",
            "SENTINEL_ACKS": "acks",
            "SENTINEL_RETRIES": ("retries", int),
            "SENTINEL_SECURITY_PROTOCOL": "security_protocol",
            "SENTINEL_SASL_MECHANISM": "sasl_mechanism",
            "SENTINEL_SASL_USERNAME": "sasl_username",
            "SENTINEL_SASL_PASSWORD": "sasl_password",
            "SENTINEL_SSL_CA_LOCATION": "ssl_ca_location",
            "SENTINEL_SSL_CERT_LOCATION": "ssl_cert_location",
            "SENTINEL_SSL_KEY_LOCATION": "ssl_key_location",
        }

        for env_key, field_info in env_mappings.items():
            if isinstance(field_info, tuple):
                field_name, converter = field_info
            else:
                field_name, converter = field_info, str

            if field_name not in kwargs:
                env_value = os.getenv(env_key)
                if env_value is not None:
                    config_dict[field_name] = converter(env_value)

        # Merge with kwargs
        config_dict.update(kwargs)

        return cls(**config_dict)

    def to_kafka_config(self) -> Dict[str, str]:
        """Convert to kafka-python producer configuration.

        Returns:
            Dictionary of Kafka configuration parameters
        """
        config = {
            "bootstrap_servers": self.brokers,
            "client_id": self.client_id,
            "compression_type": self.compression_type,
            "acks": self.acks,
            "retries": self.retries,
            "retry_backoff_ms": self.retry_backoff_ms,
            "request_timeout_ms": self.request_timeout_ms,
            "linger_ms": self.linger_ms,
            "batch_size": self.batch_size,
            "max_in_flight_requests_per_connection": self.max_in_flight_requests,
            "enable_idempotence": self.enable_idempotence,
            "buffer_memory": self.buffer_memory,
            "max_request_size": self.max_request_size,
            "metadata_max_age_ms": self.metadata_max_age_ms,
            "security_protocol": self.security_protocol,
        }

        # Add SASL configuration if needed
        if self.sasl_mechanism:
            config["sasl_mechanism"] = self.sasl_mechanism
        if self.sasl_username:
            config["sasl_plain_username"] = self.sasl_username
        if self.sasl_password:
            config["sasl_plain_password"] = self.sasl_password

        # Add SSL configuration if needed
        if self.ssl_ca_location:
            config["ssl_cafile"] = self.ssl_ca_location
        if self.ssl_cert_location:
            config["ssl_certfile"] = self.ssl_cert_location
        if self.ssl_key_location:
            config["ssl_keyfile"] = self.ssl_key_location

        # Add additional configuration
        config.update(self.additional_config)

        return config

    def to_aiokafka_config(self) -> Dict[str, str]:
        """Convert to aiokafka producer configuration.

        Returns:
            Dictionary of AIOKafka configuration parameters
        """
        config = {
            "bootstrap_servers": self.brokers,
            "client_id": self.client_id,
            "compression_type": self.compression_type,
            "acks": self.acks,
            "request_timeout_ms": self.request_timeout_ms,
            "linger_ms": self.linger_ms,
            "max_batch_size": self.batch_size,
            "max_request_size": self.max_request_size,
            "security_protocol": self.security_protocol,
        }

        # Add SASL configuration if needed
        if self.sasl_mechanism:
            config["sasl_mechanism"] = self.sasl_mechanism
        if self.sasl_username:
            config["sasl_plain_username"] = self.sasl_username
        if self.sasl_password:
            config["sasl_plain_password"] = self.sasl_password

        # Add SSL configuration if needed
        if self.ssl_ca_location:
            config["ssl_context"] = {
                "cafile": self.ssl_ca_location,
                "certfile": self.ssl_cert_location,
                "keyfile": self.ssl_key_location,
            }

        return config


__all__ = ["SentinelConfig"]
