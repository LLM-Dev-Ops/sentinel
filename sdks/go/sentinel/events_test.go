package sentinel

import (
	"testing"
)

func TestNewTelemetryEvent(t *testing.T) {
	event := NewTelemetryEvent("test-service", "gpt-4")

	if event.ServiceName != "test-service" {
		t.Errorf("Expected service name 'test-service', got '%s'", event.ServiceName)
	}

	if event.Model != "gpt-4" {
		t.Errorf("Expected model 'gpt-4', got '%s'", event.Model)
	}

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

func TestTelemetryEvent_HasErrors(t *testing.T) {
	event := NewTelemetryEvent("test-service", "gpt-4")

	if event.HasErrors() {
		t.Error("Expected no errors initially")
	}

	event.AddError("test error")

	if !event.HasErrors() {
		t.Error("Expected HasErrors to return true after adding error")
	}
}

func TestTelemetryEvent_TotalTokens(t *testing.T) {
	event := NewTelemetryEvent("test-service", "gpt-4")
	event.Prompt = PromptInfo{Text: "test", Tokens: 10}
	event.Response = ResponseInfo{Text: "test", Tokens: 20, FinishReason: "stop"}

	total := event.TotalTokens()
	if total != 30 {
		t.Errorf("Expected total tokens 30, got %d", total)
	}
}

func TestTelemetryEvent_ErrorRate(t *testing.T) {
	event := NewTelemetryEvent("test-service", "gpt-4")

	rate := event.ErrorRate()
	if rate != 0.0 {
		t.Errorf("Expected error rate 0.0, got %f", rate)
	}

	event.AddError("error")
	rate = event.ErrorRate()
	if rate != 1.0 {
		t.Errorf("Expected error rate 1.0, got %f", rate)
	}
}

func TestTelemetryEvent_SetMetadata(t *testing.T) {
	event := NewTelemetryEvent("test-service", "gpt-4")
	event.SetMetadata("key1", "value1")

	if event.Metadata["key1"] != "value1" {
		t.Errorf("Expected metadata key1=value1, got %s", event.Metadata["key1"])
	}
}

func TestTelemetryEvent_AddError(t *testing.T) {
	event := NewTelemetryEvent("test-service", "gpt-4")
	event.AddError("error1")
	event.AddError("error2")

	if len(event.Errors) != 2 {
		t.Errorf("Expected 2 errors, got %d", len(event.Errors))
	}

	if event.Errors[0] != "error1" {
		t.Errorf("Expected first error to be 'error1', got '%s'", event.Errors[0])
	}

	if event.Errors[1] != "error2" {
		t.Errorf("Expected second error to be 'error2', got '%s'", event.Errors[1])
	}
}
