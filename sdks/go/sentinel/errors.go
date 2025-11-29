package sentinel

import (
	"errors"
	"fmt"
)

var (
	// ErrInvalidConfig indicates that the client configuration is invalid
	ErrInvalidConfig = errors.New("invalid configuration")

	// ErrNotConnected indicates that the client is not connected to Kafka
	ErrNotConnected = errors.New("not connected to Kafka")

	// ErrClientClosed indicates that the client has been closed
	ErrClientClosed = errors.New("client is closed")

	// ErrContextCanceled indicates that the context was canceled
	ErrContextCanceled = errors.New("context canceled")

	// ErrInvalidEvent indicates that the event failed validation
	ErrInvalidEvent = errors.New("invalid event")
)

// SentinelError is the interface for all Sentinel SDK errors.
type SentinelError interface {
	error
	// Unwrap returns the underlying error
	Unwrap() error
	// Code returns an error code for categorization
	Code() string
}

// ConnectionError represents an error connecting to Kafka.
type ConnectionError struct {
	Brokers []string
	Err     error
}

func (e *ConnectionError) Error() string {
	return fmt.Sprintf("failed to connect to Kafka brokers %v: %v", e.Brokers, e.Err)
}

func (e *ConnectionError) Unwrap() error {
	return e.Err
}

func (e *ConnectionError) Code() string {
	return "CONNECTION_ERROR"
}

// NewConnectionError creates a new connection error.
func NewConnectionError(brokers []string, err error) *ConnectionError {
	return &ConnectionError{
		Brokers: brokers,
		Err:     err,
	}
}

// ValidationError represents an event validation error.
type ValidationError struct {
	Field   string
	Message string
	Value   interface{}
}

func (e *ValidationError) Error() string {
	return fmt.Sprintf("validation error on field '%s': %s (value: %v)", e.Field, e.Message, e.Value)
}

func (e *ValidationError) Unwrap() error {
	return ErrInvalidEvent
}

func (e *ValidationError) Code() string {
	return "VALIDATION_ERROR"
}

// NewValidationError creates a new validation error.
func NewValidationError(field, message string, value interface{}) *ValidationError {
	return &ValidationError{
		Field:   field,
		Message: message,
		Value:   value,
	}
}

// SendError represents an error sending an event to Kafka.
type SendError struct {
	Topic    string
	EventID  string
	Err      error
	Retries  int
	LastTime string
}

func (e *SendError) Error() string {
	return fmt.Sprintf("failed to send event %s to topic '%s' after %d retries: %v", e.EventID, e.Topic, e.Retries, e.Err)
}

func (e *SendError) Unwrap() error {
	return e.Err
}

func (e *SendError) Code() string {
	return "SEND_ERROR"
}

// NewSendError creates a new send error.
func NewSendError(topic, eventID string, err error, retries int) *SendError {
	return &SendError{
		Topic:   topic,
		EventID: eventID,
		Err:     err,
		Retries: retries,
	}
}

// ConfigError represents a configuration error.
type ConfigError struct {
	Field   string
	Message string
}

func (e *ConfigError) Error() string {
	return fmt.Sprintf("configuration error on field '%s': %s", e.Field, e.Message)
}

func (e *ConfigError) Unwrap() error {
	return ErrInvalidConfig
}

func (e *ConfigError) Code() string {
	return "CONFIG_ERROR"
}

// NewConfigError creates a new configuration error.
func NewConfigError(field, message string) *ConfigError {
	return &ConfigError{
		Field:   field,
		Message: message,
	}
}
