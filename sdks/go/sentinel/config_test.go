package sentinel

import (
	"testing"
	"time"
)

func TestDefaultConfig(t *testing.T) {
	config := DefaultConfig()

	if len(config.Brokers) != 1 || config.Brokers[0] != "localhost:9092" {
		t.Errorf("Expected default broker 'localhost:9092', got %v", config.Brokers)
	}

	if config.Topic != "llm.telemetry" {
		t.Errorf("Expected default topic 'llm.telemetry', got '%s'", config.Topic)
	}

	if config.MaxRetries != 3 {
		t.Errorf("Expected max retries 3, got %d", config.MaxRetries)
	}

	if config.RetryBackoff != 100*time.Millisecond {
		t.Errorf("Expected retry backoff 100ms, got %v", config.RetryBackoff)
	}

	if config.Compression != "snappy" {
		t.Errorf("Expected compression 'snappy', got '%s'", config.Compression)
	}

	if config.RequireAcks != -1 {
		t.Errorf("Expected require acks -1, got %d", config.RequireAcks)
	}

	if !config.EnableValidation {
		t.Error("Expected validation to be enabled by default")
	}

	if !config.TruncateText {
		t.Error("Expected text truncation to be enabled by default")
	}
}

func TestConfig_Validate(t *testing.T) {
	config := DefaultConfig()

	err := config.Validate()
	if err != nil {
		t.Errorf("Expected default config to be valid, got error: %v", err)
	}
}

func TestConfig_Validate_EmptyBrokers(t *testing.T) {
	config := DefaultConfig()
	config.Brokers = []string{}

	err := config.Validate()
	if err == nil {
		t.Error("Expected validation error for empty brokers")
	}
}

func TestConfig_Validate_EmptyTopic(t *testing.T) {
	config := DefaultConfig()
	config.Topic = ""

	err := config.Validate()
	if err == nil {
		t.Error("Expected validation error for empty topic")
	}
}

func TestConfig_Validate_NegativeMaxRetries(t *testing.T) {
	config := DefaultConfig()
	config.MaxRetries = -1

	err := config.Validate()
	if err == nil {
		t.Error("Expected validation error for negative max retries")
	}
}

func TestConfig_Validate_InvalidCompression(t *testing.T) {
	config := DefaultConfig()
	config.Compression = "invalid"

	err := config.Validate()
	if err == nil {
		t.Error("Expected validation error for invalid compression")
	}
}

func TestConfig_Validate_InvalidRequireAcks(t *testing.T) {
	config := DefaultConfig()
	config.RequireAcks = 5

	err := config.Validate()
	if err == nil {
		t.Error("Expected validation error for invalid require acks")
	}
}

func TestConfig_Clone(t *testing.T) {
	config := DefaultConfig()
	config.Brokers = []string{"broker1:9092", "broker2:9092"}
	config.Topic = "custom-topic"

	cloned := config.Clone()

	// Verify values are copied
	if cloned.Topic != "custom-topic" {
		t.Error("Expected topic to be cloned")
	}

	// Verify it's a deep copy
	cloned.Brokers[0] = "modified:9092"
	if config.Brokers[0] == "modified:9092" {
		t.Error("Expected brokers to be deep copied")
	}
}

func TestConfig_Validate_ValidCompressionCodecs(t *testing.T) {
	codecs := []string{"none", "gzip", "snappy", "lz4", "zstd"}

	for _, codec := range codecs {
		t.Run(codec, func(t *testing.T) {
			config := DefaultConfig()
			config.Compression = codec

			err := config.Validate()
			if err != nil {
				t.Errorf("Expected compression '%s' to be valid, got error: %v", codec, err)
			}
		})
	}
}

func TestConfig_Validate_MaxRetryBackoff(t *testing.T) {
	config := DefaultConfig()
	config.RetryBackoff = 5 * time.Second
	config.MaxRetryBackoff = 1 * time.Second

	err := config.Validate()
	if err == nil {
		t.Error("Expected validation error when max retry backoff < retry backoff")
	}
}
