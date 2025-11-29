package sentinel

import (
	"time"

	"github.com/google/uuid"
)

// TelemetryEvent represents telemetry data from an LLM application.
type TelemetryEvent struct {
	// EventID is a unique identifier for the event (auto-generated if not set)
	EventID string `json:"event_id"`

	// Timestamp is the event timestamp in ISO 8601 format (auto-generated if not set)
	Timestamp string `json:"timestamp"`

	// ServiceName identifies the service generating the telemetry
	ServiceName string `json:"service_name"`

	// TraceID for distributed tracing (optional)
	TraceID *string `json:"trace_id,omitempty"`

	// SpanID for distributed tracing (optional)
	SpanID *string `json:"span_id,omitempty"`

	// Model identifier (e.g., "gpt-4", "claude-3-sonnet")
	Model string `json:"model"`

	// Prompt contains information about the input prompt
	Prompt PromptInfo `json:"prompt"`

	// Response contains information about the LLM response
	Response ResponseInfo `json:"response"`

	// LatencyMs is the request latency in milliseconds (must be >= 0)
	LatencyMs float64 `json:"latency_ms"`

	// CostUsd is the cost in USD (must be >= 0)
	CostUsd float64 `json:"cost_usd"`

	// Metadata contains additional key-value pairs for custom data
	Metadata map[string]string `json:"metadata"`

	// Errors contains any error messages encountered during the request
	Errors []string `json:"errors"`
}

// PromptInfo contains information about the input prompt.
type PromptInfo struct {
	// Text is the prompt text (may be truncated for storage, max 100,000 chars)
	Text string `json:"text"`

	// Tokens is the number of tokens in the prompt
	Tokens uint32 `json:"tokens"`

	// Embedding is an optional vector embedding of the prompt
	Embedding []float32 `json:"embedding,omitempty"`
}

// ResponseInfo contains information about the LLM response.
type ResponseInfo struct {
	// Text is the response text (may be truncated for storage, max 100,000 chars)
	Text string `json:"text"`

	// Tokens is the number of tokens in the response
	Tokens uint32 `json:"tokens"`

	// FinishReason indicates why the model stopped generating (e.g., "stop", "length", "content_filter")
	FinishReason string `json:"finish_reason"`

	// Embedding is an optional vector embedding of the response
	Embedding []float32 `json:"embedding,omitempty"`
}

// NewTelemetryEvent creates a new telemetry event with auto-generated ID and timestamp.
func NewTelemetryEvent(serviceName, model string) *TelemetryEvent {
	return &TelemetryEvent{
		EventID:     uuid.New().String(),
		Timestamp:   time.Now().UTC().Format(time.RFC3339Nano),
		ServiceName: serviceName,
		Model:       model,
		Metadata:    make(map[string]string),
		Errors:      make([]string, 0),
	}
}

// HasErrors returns true if the event has any errors.
func (e *TelemetryEvent) HasErrors() bool {
	return len(e.Errors) > 0
}

// TotalTokens returns the total number of tokens (prompt + response).
func (e *TelemetryEvent) TotalTokens() uint32 {
	return e.Prompt.Tokens + e.Response.Tokens
}

// ErrorRate returns the error rate for this event (0.0 or 1.0).
func (e *TelemetryEvent) ErrorRate() float64 {
	if e.HasErrors() {
		return 1.0
	}
	return 0.0
}

// SetMetadata sets a metadata key-value pair.
func (e *TelemetryEvent) SetMetadata(key, value string) {
	if e.Metadata == nil {
		e.Metadata = make(map[string]string)
	}
	e.Metadata[key] = value
}

// AddError adds an error message to the event.
func (e *TelemetryEvent) AddError(err string) {
	if e.Errors == nil {
		e.Errors = make([]string, 0)
	}
	e.Errors = append(e.Errors, err)
}
