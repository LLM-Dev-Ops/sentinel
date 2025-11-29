package sentinel

import (
	"context"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/segmentio/kafka-go"
	"github.com/segmentio/kafka-go/compress"
)

// Client represents a Sentinel SDK client for sending telemetry events.
type Client struct {
	config *Config
	writer *kafka.Writer
	mu     sync.RWMutex
	closed bool
}

// ClientOption is a functional option for configuring the Client.
type ClientOption func(*Config)

// WithBrokers sets the Kafka broker addresses.
func WithBrokers(brokers []string) ClientOption {
	return func(c *Config) {
		c.Brokers = brokers
	}
}

// WithTopic sets the Kafka topic for telemetry events.
func WithTopic(topic string) ClientOption {
	return func(c *Config) {
		c.Topic = topic
	}
}

// WithMaxRetries sets the maximum number of retry attempts.
func WithMaxRetries(maxRetries int) ClientOption {
	return func(c *Config) {
		c.MaxRetries = maxRetries
	}
}

// WithRetryBackoff sets the initial retry backoff duration.
func WithRetryBackoff(backoff time.Duration) ClientOption {
	return func(c *Config) {
		c.RetryBackoff = backoff
	}
}

// WithWriteTimeout sets the write timeout for Kafka operations.
func WithWriteTimeout(timeout time.Duration) ClientOption {
	return func(c *Config) {
		c.WriteTimeout = timeout
	}
}

// WithCompression sets the compression codec.
func WithCompression(codec string) ClientOption {
	return func(c *Config) {
		c.Compression = codec
	}
}

// WithBatchSize sets the batch size for batched writes.
func WithBatchSize(size int) ClientOption {
	return func(c *Config) {
		c.BatchSize = size
	}
}

// WithBatchTimeout sets the batch timeout for batched writes.
func WithBatchTimeout(timeout time.Duration) ClientOption {
	return func(c *Config) {
		c.BatchTimeout = timeout
	}
}

// WithAsync enables async mode where sends don't wait for confirmation.
func WithAsync(async bool) ClientOption {
	return func(c *Config) {
		c.Async = async
	}
}

// WithValidation enables or disables event validation.
func WithValidation(enabled bool) ClientOption {
	return func(c *Config) {
		c.EnableValidation = enabled
	}
}

// WithConfig sets the entire configuration.
func WithConfig(config *Config) ClientOption {
	return func(c *Config) {
		*c = *config
	}
}

// NewClient creates a new Sentinel client with the given options.
func NewClient(opts ...ClientOption) (*Client, error) {
	config := DefaultConfig()

	// Apply options
	for _, opt := range opts {
		opt(config)
	}

	// Validate configuration
	if err := config.Validate(); err != nil {
		return nil, err
	}

	// Create Kafka writer
	writer, err := createKafkaWriter(config)
	if err != nil {
		return nil, NewConnectionError(config.Brokers, err)
	}

	client := &Client{
		config: config,
		writer: writer,
		closed: false,
	}

	return client, nil
}

// NewClientFromEnv creates a new Sentinel client using environment variables.
func NewClientFromEnv() (*Client, error) {
	config, err := LoadConfigFromEnv()
	if err != nil {
		return nil, err
	}

	return NewClient(WithConfig(config))
}

// createKafkaWriter creates a configured Kafka writer.
func createKafkaWriter(config *Config) (*kafka.Writer, error) {
	// Determine compression codec
	var compression kafka.Compression
	switch config.Compression {
	case "none":
		compression = kafka.Compression(0) // No compression
	case "gzip":
		compression = compress.Gzip
	case "snappy":
		compression = compress.Snappy
	case "lz4":
		compression = compress.Lz4
	case "zstd":
		compression = compress.Zstd
	default:
		compression = compress.Snappy
	}

	// Determine required acks
	var requiredAcks kafka.RequiredAcks
	switch config.RequireAcks {
	case -1:
		requiredAcks = kafka.RequireAll
	case 0:
		requiredAcks = kafka.RequireNone
	case 1:
		requiredAcks = kafka.RequireOne
	default:
		requiredAcks = kafka.RequireAll
	}

	writer := &kafka.Writer{
		Addr:         kafka.TCP(config.Brokers...),
		Topic:        config.Topic,
		Balancer:     &kafka.Hash{}, // Use hash balancing for better distribution
		Compression:  compression,
		RequiredAcks: requiredAcks,
		MaxAttempts:  config.MaxAttempts,
		WriteTimeout: config.WriteTimeout,
		ReadTimeout:  config.ReadTimeout,
		Async:        config.Async,
		BatchSize:    config.BatchSize,
		BatchTimeout: config.BatchTimeout,
	}

	return writer, nil
}

// Send sends a telemetry event to Sentinel.
func (c *Client) Send(ctx context.Context, event *TelemetryEvent) error {
	c.mu.RLock()
	if c.closed {
		c.mu.RUnlock()
		return ErrClientClosed
	}
	c.mu.RUnlock()

	// Sanitize event
	SanitizeEvent(event, c.config)

	// Validate event if enabled
	if c.config.EnableValidation {
		if err := Validate(event); err != nil {
			return err
		}
	}

	// Serialize event to JSON
	data, err := json.Marshal(event)
	if err != nil {
		return fmt.Errorf("failed to marshal event: %w", err)
	}

	// Create Kafka message
	msg := kafka.Message{
		Key:   []byte(event.EventID),
		Value: data,
		Time:  time.Now().UTC(),
	}

	// Send with retry logic
	return c.sendWithRetry(ctx, msg, event.EventID)
}

// sendWithRetry sends a message with exponential backoff retry logic.
func (c *Client) sendWithRetry(ctx context.Context, msg kafka.Message, eventID string) error {
	var lastErr error
	backoff := c.config.RetryBackoff

	for attempt := 0; attempt <= c.config.MaxRetries; attempt++ {
		// Check context cancellation
		select {
		case <-ctx.Done():
			return ErrContextCanceled
		default:
		}

		// Attempt to send
		err := c.writer.WriteMessages(ctx, msg)
		if err == nil {
			return nil
		}

		lastErr = err

		// Don't retry on last attempt
		if attempt == c.config.MaxRetries {
			break
		}

		// Wait with exponential backoff
		select {
		case <-ctx.Done():
			return ErrContextCanceled
		case <-time.After(backoff):
			// Double the backoff for next retry, up to max
			backoff *= 2
			if backoff > c.config.MaxRetryBackoff {
				backoff = c.config.MaxRetryBackoff
			}
		}
	}

	return NewSendError(c.config.Topic, eventID, lastErr, c.config.MaxRetries)
}

// SendBatch sends multiple telemetry events in a single batch.
func (c *Client) SendBatch(ctx context.Context, events []*TelemetryEvent) error {
	c.mu.RLock()
	if c.closed {
		c.mu.RUnlock()
		return ErrClientClosed
	}
	c.mu.RUnlock()

	if len(events) == 0 {
		return nil
	}

	messages := make([]kafka.Message, 0, len(events))

	for _, event := range events {
		// Sanitize event
		SanitizeEvent(event, c.config)

		// Validate event if enabled
		if c.config.EnableValidation {
			if err := Validate(event); err != nil {
				return fmt.Errorf("validation failed for event %s: %w", event.EventID, err)
			}
		}

		// Serialize event to JSON
		data, err := json.Marshal(event)
		if err != nil {
			return fmt.Errorf("failed to marshal event %s: %w", event.EventID, err)
		}

		// Create Kafka message
		msg := kafka.Message{
			Key:   []byte(event.EventID),
			Value: data,
			Time:  time.Now().UTC(),
		}

		messages = append(messages, msg)
	}

	// Send all messages
	return c.writer.WriteMessages(ctx, messages...)
}

// Track is a convenience method for sending telemetry using a simpler API.
func (c *Client) Track(ctx context.Context, opts TrackOptions) error {
	event := BuildFromOptions(opts)
	return c.Send(ctx, event)
}

// Close closes the client and releases resources.
func (c *Client) Close() error {
	c.mu.Lock()
	defer c.mu.Unlock()

	if c.closed {
		return nil
	}

	c.closed = true

	if c.writer != nil {
		return c.writer.Close()
	}

	return nil
}

// Stats returns statistics about the Kafka writer.
func (c *Client) Stats() kafka.WriterStats {
	return c.writer.Stats()
}

// Config returns a copy of the client's configuration.
func (c *Client) Config() *Config {
	return c.config.Clone()
}

// IsClosed returns true if the client has been closed.
func (c *Client) IsClosed() bool {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.closed
}
