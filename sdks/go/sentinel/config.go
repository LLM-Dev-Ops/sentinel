package sentinel

import (
	"time"

	"github.com/kelseyhightower/envconfig"
)

// Config holds the configuration for the Sentinel client.
type Config struct {
	// Brokers is the list of Kafka broker addresses
	Brokers []string `envconfig:"SENTINEL_BROKERS" default:"localhost:9092"`

	// Topic is the Kafka topic to send telemetry events to
	Topic string `envconfig:"SENTINEL_TOPIC" default:"llm.telemetry"`

	// MaxRetries is the maximum number of retry attempts for failed sends
	MaxRetries int `envconfig:"SENTINEL_MAX_RETRIES" default:"3"`

	// RetryBackoff is the initial backoff duration for retries
	RetryBackoff time.Duration `envconfig:"SENTINEL_RETRY_BACKOFF" default:"100ms"`

	// MaxRetryBackoff is the maximum backoff duration for retries
	MaxRetryBackoff time.Duration `envconfig:"SENTINEL_MAX_RETRY_BACKOFF" default:"5s"`

	// WriteTimeout is the timeout for writing messages to Kafka
	WriteTimeout time.Duration `envconfig:"SENTINEL_WRITE_TIMEOUT" default:"10s"`

	// ReadTimeout is the timeout for reading responses from Kafka
	ReadTimeout time.Duration `envconfig:"SENTINEL_READ_TIMEOUT" default:"10s"`

	// BatchSize is the number of messages to batch before sending (0 for no batching)
	BatchSize int `envconfig:"SENTINEL_BATCH_SIZE" default:"0"`

	// BatchTimeout is the maximum time to wait before sending a partial batch
	BatchTimeout time.Duration `envconfig:"SENTINEL_BATCH_TIMEOUT" default:"0s"`

	// Compression is the compression codec to use (none, gzip, snappy, lz4, zstd)
	Compression string `envconfig:"SENTINEL_COMPRESSION" default:"snappy"`

	// RequireAcks is the required number of acks from Kafka (0, 1, or -1 for all)
	RequireAcks int `envconfig:"SENTINEL_REQUIRE_ACKS" default:"-1"`

	// Async enables async mode where sends don't wait for confirmation
	Async bool `envconfig:"SENTINEL_ASYNC" default:"false"`

	// MaxAttempts is the maximum number of connection attempts
	MaxAttempts int `envconfig:"SENTINEL_MAX_ATTEMPTS" default:"3"`

	// EnableValidation enables event validation before sending
	EnableValidation bool `envconfig:"SENTINEL_ENABLE_VALIDATION" default:"true"`

	// TruncateText enables automatic text truncation to prevent oversized messages
	TruncateText bool `envconfig:"SENTINEL_TRUNCATE_TEXT" default:"true"`

	// MaxTextLength is the maximum length of text fields before truncation
	MaxTextLength int `envconfig:"SENTINEL_MAX_TEXT_LENGTH" default:"100000"`
}

// DefaultConfig returns a configuration with sensible defaults.
func DefaultConfig() *Config {
	return &Config{
		Brokers:          []string{"localhost:9092"},
		Topic:            "llm.telemetry",
		MaxRetries:       3,
		RetryBackoff:     100 * time.Millisecond,
		MaxRetryBackoff:  5 * time.Second,
		WriteTimeout:     10 * time.Second,
		ReadTimeout:      10 * time.Second,
		BatchSize:        0,
		BatchTimeout:     0,
		Compression:      "snappy",
		RequireAcks:      -1, // Wait for all in-sync replicas
		Async:            false,
		MaxAttempts:      3,
		EnableValidation: true,
		TruncateText:     true,
		MaxTextLength:    100000,
	}
}

// LoadConfigFromEnv loads configuration from environment variables.
// It starts with default values and overrides them with environment variables.
func LoadConfigFromEnv() (*Config, error) {
	cfg := DefaultConfig()
	err := envconfig.Process("sentinel", cfg)
	if err != nil {
		return nil, NewConfigError("environment", err.Error())
	}
	return cfg, nil
}

// Validate validates the configuration.
func (c *Config) Validate() error {
	if len(c.Brokers) == 0 {
		return NewConfigError("brokers", "at least one broker is required")
	}

	if c.Topic == "" {
		return NewConfigError("topic", "topic cannot be empty")
	}

	if c.MaxRetries < 0 {
		return NewConfigError("max_retries", "must be >= 0")
	}

	if c.RetryBackoff < 0 {
		return NewConfigError("retry_backoff", "must be >= 0")
	}

	if c.MaxRetryBackoff < c.RetryBackoff {
		return NewConfigError("max_retry_backoff", "must be >= retry_backoff")
	}

	if c.WriteTimeout <= 0 {
		return NewConfigError("write_timeout", "must be > 0")
	}

	if c.ReadTimeout <= 0 {
		return NewConfigError("read_timeout", "must be > 0")
	}

	if c.BatchSize < 0 {
		return NewConfigError("batch_size", "must be >= 0")
	}

	if c.BatchTimeout < 0 {
		return NewConfigError("batch_timeout", "must be >= 0")
	}

	if c.RequireAcks < -1 || c.RequireAcks > 1 {
		return NewConfigError("require_acks", "must be -1, 0, or 1")
	}

	if c.MaxAttempts <= 0 {
		return NewConfigError("max_attempts", "must be > 0")
	}

	if c.MaxTextLength <= 0 {
		return NewConfigError("max_text_length", "must be > 0")
	}

	validCompressions := map[string]bool{
		"none":   true,
		"gzip":   true,
		"snappy": true,
		"lz4":    true,
		"zstd":   true,
	}

	if !validCompressions[c.Compression] {
		return NewConfigError("compression", "must be one of: none, gzip, snappy, lz4, zstd")
	}

	return nil
}

// Clone creates a deep copy of the configuration.
func (c *Config) Clone() *Config {
	brokers := make([]string, len(c.Brokers))
	copy(brokers, c.Brokers)

	return &Config{
		Brokers:          brokers,
		Topic:            c.Topic,
		MaxRetries:       c.MaxRetries,
		RetryBackoff:     c.RetryBackoff,
		MaxRetryBackoff:  c.MaxRetryBackoff,
		WriteTimeout:     c.WriteTimeout,
		ReadTimeout:      c.ReadTimeout,
		BatchSize:        c.BatchSize,
		BatchTimeout:     c.BatchTimeout,
		Compression:      c.Compression,
		RequireAcks:      c.RequireAcks,
		Async:            c.Async,
		MaxAttempts:      c.MaxAttempts,
		EnableValidation: c.EnableValidation,
		TruncateText:     c.TruncateText,
		MaxTextLength:    c.MaxTextLength,
	}
}
