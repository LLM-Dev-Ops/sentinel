package sentinel

import (
	"time"

	"github.com/google/uuid"
)

// EventBuilder provides a fluent interface for building telemetry events.
type EventBuilder struct {
	event *TelemetryEvent
}

// NewEventBuilder creates a new event builder with auto-generated ID and timestamp.
func NewEventBuilder() *EventBuilder {
	return &EventBuilder{
		event: &TelemetryEvent{
			EventID:   generateEventID(),
			Timestamp: generateTimestamp(),
			Metadata:  make(map[string]string),
			Errors:    make([]string, 0),
		},
	}
}

// Service sets the service name.
func (b *EventBuilder) Service(name string) *EventBuilder {
	b.event.ServiceName = name
	return b
}

// Model sets the model identifier.
func (b *EventBuilder) Model(model string) *EventBuilder {
	b.event.Model = model
	return b
}

// Prompt sets the prompt text and token count.
func (b *EventBuilder) Prompt(text string, tokens uint32) *EventBuilder {
	b.event.Prompt = PromptInfo{
		Text:   text,
		Tokens: tokens,
	}
	return b
}

// PromptWithEmbedding sets the prompt text, token count, and embedding.
func (b *EventBuilder) PromptWithEmbedding(text string, tokens uint32, embedding []float32) *EventBuilder {
	b.event.Prompt = PromptInfo{
		Text:      text,
		Tokens:    tokens,
		Embedding: embedding,
	}
	return b
}

// Response sets the response text, token count, and finish reason.
func (b *EventBuilder) Response(text string, tokens uint32, finishReason string) *EventBuilder {
	b.event.Response = ResponseInfo{
		Text:         text,
		Tokens:       tokens,
		FinishReason: finishReason,
	}
	return b
}

// ResponseWithEmbedding sets the response text, token count, finish reason, and embedding.
func (b *EventBuilder) ResponseWithEmbedding(text string, tokens uint32, finishReason string, embedding []float32) *EventBuilder {
	b.event.Response = ResponseInfo{
		Text:         text,
		Tokens:       tokens,
		FinishReason: finishReason,
		Embedding:    embedding,
	}
	return b
}

// Latency sets the request latency in milliseconds.
func (b *EventBuilder) Latency(latencyMs float64) *EventBuilder {
	b.event.LatencyMs = latencyMs
	return b
}

// Cost sets the cost in USD.
func (b *EventBuilder) Cost(costUsd float64) *EventBuilder {
	b.event.CostUsd = costUsd
	return b
}

// TraceID sets the distributed tracing trace ID.
func (b *EventBuilder) TraceID(traceID string) *EventBuilder {
	b.event.TraceID = &traceID
	return b
}

// SpanID sets the distributed tracing span ID.
func (b *EventBuilder) SpanID(spanID string) *EventBuilder {
	b.event.SpanID = &spanID
	return b
}

// Metadata sets the entire metadata map, replacing any existing metadata.
func (b *EventBuilder) Metadata(metadata map[string]string) *EventBuilder {
	b.event.Metadata = metadata
	return b
}

// AddMetadata adds a single metadata key-value pair.
func (b *EventBuilder) AddMetadata(key, value string) *EventBuilder {
	if b.event.Metadata == nil {
		b.event.Metadata = make(map[string]string)
	}
	b.event.Metadata[key] = value
	return b
}

// Error adds an error message to the event.
func (b *EventBuilder) Error(err string) *EventBuilder {
	if b.event.Errors == nil {
		b.event.Errors = make([]string, 0)
	}
	b.event.Errors = append(b.event.Errors, err)
	return b
}

// Errors sets the entire errors slice, replacing any existing errors.
func (b *EventBuilder) Errors(errors []string) *EventBuilder {
	b.event.Errors = errors
	return b
}

// EventID sets a custom event ID (useful for deduplication).
func (b *EventBuilder) EventID(id string) *EventBuilder {
	b.event.EventID = id
	return b
}

// Timestamp sets a custom timestamp (defaults to current time).
func (b *EventBuilder) Timestamp(timestamp string) *EventBuilder {
	b.event.Timestamp = timestamp
	return b
}

// Build returns the constructed telemetry event.
func (b *EventBuilder) Build() *TelemetryEvent {
	return b.event
}

// TrackOptions contains options for the simple Track() API.
type TrackOptions struct {
	// Service identifies the service generating the telemetry
	Service string

	// Model is the model identifier (e.g., "gpt-4", "claude-3-sonnet")
	Model string

	// Prompt is the input prompt text
	Prompt string

	// Response is the LLM response text
	Response string

	// LatencyMs is the request latency in milliseconds
	LatencyMs float64

	// PromptTokens is the number of tokens in the prompt
	PromptTokens uint32

	// CompletionTokens is the number of tokens in the response
	CompletionTokens uint32

	// CostUsd is the cost in USD
	CostUsd float64

	// FinishReason is the reason the model stopped generating (default: "stop")
	FinishReason string

	// TraceID for distributed tracing (optional)
	TraceID string

	// SpanID for distributed tracing (optional)
	SpanID string

	// Metadata contains additional key-value pairs
	Metadata map[string]string

	// Errors contains any error messages
	Errors []string

	// PromptEmbedding is an optional vector embedding of the prompt
	PromptEmbedding []float32

	// ResponseEmbedding is an optional vector embedding of the response
	ResponseEmbedding []float32
}

// BuildFromOptions creates a telemetry event from TrackOptions.
func BuildFromOptions(opts TrackOptions) *TelemetryEvent {
	builder := NewEventBuilder().
		Service(opts.Service).
		Model(opts.Model).
		Latency(opts.LatencyMs).
		Cost(opts.CostUsd)

	// Set prompt
	if opts.PromptEmbedding != nil {
		builder.PromptWithEmbedding(opts.Prompt, opts.PromptTokens, opts.PromptEmbedding)
	} else {
		builder.Prompt(opts.Prompt, opts.PromptTokens)
	}

	// Set response
	finishReason := opts.FinishReason
	if finishReason == "" {
		finishReason = "stop"
	}

	if opts.ResponseEmbedding != nil {
		builder.ResponseWithEmbedding(opts.Response, opts.CompletionTokens, finishReason, opts.ResponseEmbedding)
	} else {
		builder.Response(opts.Response, opts.CompletionTokens, finishReason)
	}

	// Set optional fields
	if opts.TraceID != "" {
		builder.TraceID(opts.TraceID)
	}

	if opts.SpanID != "" {
		builder.SpanID(opts.SpanID)
	}

	if opts.Metadata != nil {
		builder.Metadata(opts.Metadata)
	}

	if opts.Errors != nil {
		builder.Errors(opts.Errors)
	}

	return builder.Build()
}

// Helper functions for generating IDs and timestamps

func generateEventID() string {
	return uuid.New().String()
}

func generateTimestamp() string {
	return time.Now().UTC().Format(time.RFC3339Nano)
}
