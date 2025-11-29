# Changelog

All notable changes to the LLM-Sentinel TypeScript SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-01-XX

### Added
- Initial release of LLM-Sentinel TypeScript SDK
- `SentinelClient` for sending telemetry events to Kafka
- Type-safe event structures matching Rust implementation
- Fluent `TelemetryEventBuilder` for constructing events
- Comprehensive validation of telemetry events
- Environment variable configuration support
- Custom error classes: `ValidationError`, `ConnectionError`, `SendError`, `ConfigurationError`
- Automatic connection management with retry logic
- Batch sending support for high-throughput scenarios
- Compression support (gzip, snappy, lz4)
- SSL/TLS and SASL authentication support
- TypeScript type definitions for all event types
- Graceful shutdown and connection pooling
- Debug logging mode
- Complete documentation with examples
- Example code for common use cases

### Features
- **Event Types**: TelemetryEvent, PromptInfo, ResponseInfo, AnomalyEvent, AlertEvent
- **Enums**: Severity, AnomalyType, DetectionMethod
- **Client Methods**: connect(), disconnect(), send(), sendBatch(), track(), flush()
- **Configuration**: Kafka config, retry config, SSL/SASL, compression
- **Validation**: Client-side validation before sending to Kafka
- **Error Handling**: Rich error types with detailed context
- **Performance**: Connection pooling, batching, compression

### Documentation
- Comprehensive README with API reference
- TypeScript API documentation
- Usage examples (basic, builder, batch, error-handling)
- Configuration guide
- Troubleshooting guide
- Best practices

## [Unreleased]

### Planned Features
- Metrics collection and reporting
- Circuit breaker for Kafka connection failures
- Event sampling for high-volume scenarios
- Schema registry integration
- Dead letter queue support
- Observability hooks (OpenTelemetry integration)
- Browser support (via WebSocket gateway)
- Middleware/plugin system
- Performance benchmarks
- Integration tests with test containers

---

[0.1.0]: https://github.com/yourusername/llm-sentinel/releases/tag/sdk-typescript-v0.1.0
