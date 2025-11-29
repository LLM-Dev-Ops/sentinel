# Changelog

All notable changes to the LLM Sentinel Python SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-01-01

### Added

#### Core Features
- Initial release of the LLM Sentinel Python SDK
- Synchronous client (`SentinelClient`) with kafka-python
- Asynchronous client (`AsyncSentinelClient`) with aiokafka
- Complete event schema matching sentinel-core Rust implementation
- Fluent builder pattern for event construction (`TelemetryEventBuilder`)

#### Event Types
- `TelemetryEvent`: Main telemetry event structure
- `PromptInfo`: Prompt information with text, tokens, and embeddings
- `ResponseInfo`: Response information with text, tokens, and finish reason
- Auto-generated UUIDs and timestamps
- Distributed tracing support (trace_id, span_id)
- Metadata and error tracking

#### Configuration
- `SentinelConfig`: Comprehensive configuration dataclass
- Environment variable support for all configuration options
- Kafka producer configuration with sensible defaults
- SSL/TLS encryption support
- SASL authentication (PLAIN, SCRAM)
- Configurable compression (none, gzip, snappy, lz4, zstd)
- Retry logic with exponential backoff

#### Type System
- `Severity` enum: LOW, MEDIUM, HIGH, CRITICAL with ordering
- `AnomalyType` enum: 12+ anomaly types
- `DetectionMethod` enum: 11+ detection methods
- Full type hints (PEP 484)
- PEP 561 compliance with py.typed marker

#### Exception Hierarchy
- `SentinelError`: Base exception
- `ConnectionError`: Kafka connection failures
- `ValidationError`: Event validation failures
- `SendError`: Message sending failures
- `TimeoutError`: Operation timeouts
- `ConfigurationError`: Configuration errors
- Cause tracking for debugging

#### Client Features
- Context manager support for automatic cleanup
- Graceful shutdown with resource cleanup
- Connection pooling
- Automatic message batching
- Flush support for pending messages
- Configurable timeouts
- Track convenience method for simple usage
- Send method for pre-built events

#### Documentation
- Comprehensive README with examples
- Quick start guide (QUICKSTART.md)
- Complete API reference (API.md)
- Architecture overview (OVERVIEW.md)
- Integration examples for OpenAI, Anthropic, LangChain

#### Examples
- Basic synchronous usage example
- Asynchronous usage with concurrency
- Builder pattern demonstration
- OpenAI integration with error tracking
- Context manager usage patterns

#### Testing
- Unit tests for events module
- Unit tests for types module
- Unit tests for builders module
- Pytest configuration
- Coverage reporting setup

#### Development Tools
- Modern packaging with pyproject.toml
- Black code formatting configuration
- isort import sorting configuration
- Mypy type checking configuration
- Flake8 linting configuration
- Requirements files for core and dev dependencies

### Technical Details

#### Dependencies
- kafka-python >= 2.0.2 (core)
- aiokafka >= 0.8.0 (optional, for async support)
- pytest >= 7.0.0 (dev)
- pytest-asyncio >= 0.21.0 (dev)
- black, isort, mypy, flake8 (dev)

#### Python Support
- Python 3.8+
- Python 3.9
- Python 3.10
- Python 3.11
- Python 3.12

#### Metrics
- 8 core modules
- 2,285+ lines of Python code
- 4 comprehensive example scripts
- 3 test modules
- 4 documentation files
- 100% type hint coverage

### Performance
- Default batch size: 16KB
- Default compression: gzip
- Default timeout: 30 seconds
- Default buffer: 32MB
- Linger time: 10ms

### Security
- SSL/TLS support
- SASL authentication support
- Text truncation (100K chars max)
- No credentials in code

## [Unreleased]

### Planned Features
- Metrics collection and reporting
- OpenTelemetry integration
- Event sampling support
- Local buffering for offline scenarios
- Schema registry support (Avro/Protobuf)
- Batch sending APIs
- Success/failure callbacks
- Request/response interceptors
- Additional LLM provider examples (LangChain, LlamaIndex)

---

[0.1.0]: https://github.com/yourusername/sentinel/releases/tag/v0.1.0
