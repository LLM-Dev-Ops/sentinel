package sentinel

import (
	"strings"
	"unicode/utf8"
)

// Validate validates a telemetry event and returns an error if invalid.
func Validate(event *TelemetryEvent) error {
	if event == nil {
		return NewValidationError("event", "event cannot be nil", nil)
	}

	// Validate required fields
	if event.EventID == "" {
		return NewValidationError("event_id", "cannot be empty", event.EventID)
	}

	if event.Timestamp == "" {
		return NewValidationError("timestamp", "cannot be empty", event.Timestamp)
	}

	if event.ServiceName == "" {
		return NewValidationError("service_name", "cannot be empty", event.ServiceName)
	}

	if event.Model == "" {
		return NewValidationError("model", "cannot be empty", event.Model)
	}

	// Validate numeric fields
	if event.LatencyMs < 0 {
		return NewValidationError("latency_ms", "must be >= 0", event.LatencyMs)
	}

	if event.CostUsd < 0 {
		return NewValidationError("cost_usd", "must be >= 0", event.CostUsd)
	}

	// Validate prompt
	if err := validatePromptInfo(&event.Prompt); err != nil {
		return err
	}

	// Validate response
	if err := validateResponseInfo(&event.Response); err != nil {
		return err
	}

	// Validate text lengths (max 100,000 characters as per Rust implementation)
	const maxTextLength = 100000

	if utf8.RuneCountInString(event.Prompt.Text) > maxTextLength {
		return NewValidationError("prompt.text", "exceeds maximum length of 100,000 characters", len(event.Prompt.Text))
	}

	if utf8.RuneCountInString(event.Response.Text) > maxTextLength {
		return NewValidationError("response.text", "exceeds maximum length of 100,000 characters", len(event.Response.Text))
	}

	return nil
}

// validatePromptInfo validates the prompt information.
func validatePromptInfo(prompt *PromptInfo) error {
	if prompt == nil {
		return NewValidationError("prompt", "cannot be nil", nil)
	}

	if prompt.Text == "" {
		return NewValidationError("prompt.text", "cannot be empty", prompt.Text)
	}

	if prompt.Tokens == 0 {
		return NewValidationError("prompt.tokens", "must be > 0", prompt.Tokens)
	}

	return nil
}

// validateResponseInfo validates the response information.
func validateResponseInfo(response *ResponseInfo) error {
	if response == nil {
		return NewValidationError("response", "cannot be nil", nil)
	}

	if response.Text == "" {
		return NewValidationError("response.text", "cannot be empty", response.Text)
	}

	if response.Tokens == 0 {
		return NewValidationError("response.tokens", "must be > 0", response.Tokens)
	}

	if response.FinishReason == "" {
		return NewValidationError("response.finish_reason", "cannot be empty", response.FinishReason)
	}

	return nil
}

// TruncateText truncates text fields to the specified maximum length.
func TruncateText(event *TelemetryEvent, maxLength int) {
	if event == nil {
		return
	}

	event.Prompt.Text = truncateString(event.Prompt.Text, maxLength)
	event.Response.Text = truncateString(event.Response.Text, maxLength)
}

// truncateString truncates a string to the specified maximum length.
// It ensures the truncation happens at a valid UTF-8 boundary.
func truncateString(s string, maxLength int) string {
	if utf8.RuneCountInString(s) <= maxLength {
		return s
	}

	// Convert to runes to handle multi-byte UTF-8 characters correctly
	runes := []rune(s)
	if len(runes) <= maxLength {
		return s
	}

	truncated := string(runes[:maxLength])
	return truncated + "...[truncated]"
}

// SanitizeEvent ensures the event is valid and ready to send.
// It auto-generates missing fields and truncates text if needed.
func SanitizeEvent(event *TelemetryEvent, config *Config) {
	if event == nil {
		return
	}

	// Auto-generate event ID if missing
	if event.EventID == "" {
		event.EventID = generateEventID()
	}

	// Auto-generate timestamp if missing
	if event.Timestamp == "" {
		event.Timestamp = generateTimestamp()
	}

	// Initialize metadata if nil
	if event.Metadata == nil {
		event.Metadata = make(map[string]string)
	}

	// Initialize errors if nil
	if event.Errors == nil {
		event.Errors = make([]string, 0)
	}

	// Truncate text if enabled
	if config != nil && config.TruncateText {
		TruncateText(event, config.MaxTextLength)
	}

	// Sanitize finish reason
	if event.Response.FinishReason == "" {
		event.Response.FinishReason = "unknown"
	}
}

// NormalizeServiceName normalizes a service name by converting to lowercase
// and replacing spaces with hyphens.
func NormalizeServiceName(name string) string {
	normalized := strings.ToLower(name)
	normalized = strings.ReplaceAll(normalized, " ", "-")
	normalized = strings.ReplaceAll(normalized, "_", "-")
	return normalized
}

// NormalizeModelName normalizes a model name by converting to lowercase.
func NormalizeModelName(name string) string {
	return strings.ToLower(name)
}
