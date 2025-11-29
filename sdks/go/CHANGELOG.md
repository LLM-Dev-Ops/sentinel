# Changelog

All notable changes to the Sentinel Go SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of Sentinel Go SDK
- Core client with Kafka integration using segmentio/kafka-go
- Event builder with fluent API
- Comprehensive validation and sanitization
- Retry logic with exponential backoff
- Batch send support
- Context-based cancellation and timeouts
- Environment variable configuration support
- Custom error types for better error handling
- Built-in text truncation for large payloads
- Support for distributed tracing (trace_id, span_id)
- Support for embeddings in prompts and responses
- Comprehensive test suite
- Example code demonstrating common use cases
- Full documentation with integration examples

### Configuration
- Configurable Kafka brokers
- Configurable topic (default: llm.telemetry)
- Configurable retry behavior
- Configurable compression (snappy, gzip, lz4, zstd, none)
- Configurable batching
- Async mode support

### Documentation
- Comprehensive README with usage examples
- API documentation via godoc
- Example programs for basic usage and error handling
- Integration examples for OpenAI and Anthropic Claude

## [0.1.0] - TBD

### Added
- First public release
