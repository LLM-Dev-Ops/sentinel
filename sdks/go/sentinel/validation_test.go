package sentinel

import (
	"strings"
	"testing"
)

func TestValidate_ValidEvent(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Hello", 1).
		Response("Hi", 1, "stop").
		Latency(100.0).
		Cost(0.001).
		Build()

	err := Validate(event)
	if err != nil {
		t.Errorf("Expected no validation error, got: %v", err)
	}
}

func TestValidate_NilEvent(t *testing.T) {
	err := Validate(nil)
	if err == nil {
		t.Error("Expected validation error for nil event")
	}
}

func TestValidate_EmptyEventID(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Hello", 1).
		Response("Hi", 1, "stop").
		Latency(100.0).
		Cost(0.001).
		EventID(""). // Empty event ID
		Build()

	err := Validate(event)
	if err == nil {
		t.Error("Expected validation error for empty event ID")
	}
}

func TestValidate_EmptyServiceName(t *testing.T) {
	event := NewEventBuilder().
		Service(""). // Empty service
		Model("gpt-4").
		Prompt("Hello", 1).
		Response("Hi", 1, "stop").
		Latency(100.0).
		Cost(0.001).
		Build()

	err := Validate(event)
	if err == nil {
		t.Error("Expected validation error for empty service name")
	}
}

func TestValidate_EmptyModel(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model(""). // Empty model
		Prompt("Hello", 1).
		Response("Hi", 1, "stop").
		Latency(100.0).
		Cost(0.001).
		Build()

	err := Validate(event)
	if err == nil {
		t.Error("Expected validation error for empty model")
	}
}

func TestValidate_NegativeLatency(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Hello", 1).
		Response("Hi", 1, "stop").
		Latency(-10.0). // Negative latency
		Cost(0.001).
		Build()

	err := Validate(event)
	if err == nil {
		t.Error("Expected validation error for negative latency")
	}
}

func TestValidate_NegativeCost(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Hello", 1).
		Response("Hi", 1, "stop").
		Latency(100.0).
		Cost(-0.001). // Negative cost
		Build()

	err := Validate(event)
	if err == nil {
		t.Error("Expected validation error for negative cost")
	}
}

func TestValidate_EmptyPromptText(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("", 1). // Empty prompt text
		Response("Hi", 1, "stop").
		Latency(100.0).
		Cost(0.001).
		Build()

	err := Validate(event)
	if err == nil {
		t.Error("Expected validation error for empty prompt text")
	}
}

func TestValidate_ZeroPromptTokens(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Hello", 0). // Zero tokens
		Response("Hi", 1, "stop").
		Latency(100.0).
		Cost(0.001).
		Build()

	err := Validate(event)
	if err == nil {
		t.Error("Expected validation error for zero prompt tokens")
	}
}

func TestTruncateText(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt(strings.Repeat("a", 200), 200).
		Response(strings.Repeat("b", 200), 200, "stop").
		Latency(100.0).
		Cost(0.001).
		Build()

	TruncateText(event, 100)

	if len([]rune(event.Prompt.Text)) > 115 { // 100 + "...[truncated]"
		t.Errorf("Expected prompt text to be truncated to ~115 runes, got %d", len([]rune(event.Prompt.Text)))
	}

	if len([]rune(event.Response.Text)) > 115 {
		t.Errorf("Expected response text to be truncated to ~115 runes, got %d", len([]rune(event.Response.Text)))
	}
}

func TestSanitizeEvent(t *testing.T) {
	event := &TelemetryEvent{
		ServiceName: "test-service",
		Model:       "gpt-4",
		Prompt:      PromptInfo{Text: "Hello", Tokens: 1},
		Response:    ResponseInfo{Text: "Hi", Tokens: 1},
		LatencyMs:   100.0,
		CostUsd:     0.001,
	}

	config := DefaultConfig()
	SanitizeEvent(event, config)

	if event.EventID == "" {
		t.Error("Expected event ID to be auto-generated")
	}

	if event.Timestamp == "" {
		t.Error("Expected timestamp to be auto-generated")
	}

	if event.Metadata == nil {
		t.Error("Expected metadata to be initialized")
	}

	if event.Errors == nil {
		t.Error("Expected errors to be initialized")
	}
}

func TestNormalizeServiceName(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"MyService", "myservice"},
		{"my-service", "my-service"},
		{"my_service", "my-service"},
		{"My Service", "my-service"},
		{"MY_SERVICE", "my-service"},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := NormalizeServiceName(tt.input)
			if result != tt.expected {
				t.Errorf("NormalizeServiceName(%s) = %s, expected %s", tt.input, result, tt.expected)
			}
		})
	}
}

func TestNormalizeModelName(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"GPT-4", "gpt-4"},
		{"Claude-3-Sonnet", "claude-3-sonnet"},
		{"text-embedding-3-small", "text-embedding-3-small"},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := NormalizeModelName(tt.input)
			if result != tt.expected {
				t.Errorf("NormalizeModelName(%s) = %s, expected %s", tt.input, result, tt.expected)
			}
		})
	}
}
