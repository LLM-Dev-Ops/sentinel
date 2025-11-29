package sentinel

import (
	"testing"
)

func TestEventBuilder_Build(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Hello", 1).
		Response("Hi", 1, "stop").
		Latency(100.0).
		Cost(0.001).
		Build()

	if event.ServiceName != "test-service" {
		t.Errorf("Expected service 'test-service', got '%s'", event.ServiceName)
	}

	if event.Model != "gpt-4" {
		t.Errorf("Expected model 'gpt-4', got '%s'", event.Model)
	}

	if event.Prompt.Text != "Hello" {
		t.Errorf("Expected prompt 'Hello', got '%s'", event.Prompt.Text)
	}

	if event.Prompt.Tokens != 1 {
		t.Errorf("Expected prompt tokens 1, got %d", event.Prompt.Tokens)
	}

	if event.Response.Text != "Hi" {
		t.Errorf("Expected response 'Hi', got '%s'", event.Response.Text)
	}

	if event.Response.Tokens != 1 {
		t.Errorf("Expected response tokens 1, got %d", event.Response.Tokens)
	}

	if event.LatencyMs != 100.0 {
		t.Errorf("Expected latency 100.0, got %f", event.LatencyMs)
	}

	if event.CostUsd != 0.001 {
		t.Errorf("Expected cost 0.001, got %f", event.CostUsd)
	}
}

func TestEventBuilder_WithEmbeddings(t *testing.T) {
	promptEmbed := []float32{0.1, 0.2, 0.3}
	responseEmbed := []float32{0.4, 0.5, 0.6}

	event := NewEventBuilder().
		Service("test-service").
		Model("text-embedding-3-small").
		PromptWithEmbedding("Hello", 1, promptEmbed).
		ResponseWithEmbedding("Hi", 1, "stop", responseEmbed).
		Latency(50.0).
		Cost(0.00001).
		Build()

	if event.Prompt.Embedding == nil {
		t.Error("Expected prompt embedding to be set")
	}

	if len(event.Prompt.Embedding) != 3 {
		t.Errorf("Expected 3 prompt embedding values, got %d", len(event.Prompt.Embedding))
	}

	if event.Response.Embedding == nil {
		t.Error("Expected response embedding to be set")
	}

	if len(event.Response.Embedding) != 3 {
		t.Errorf("Expected 3 response embedding values, got %d", len(event.Response.Embedding))
	}
}

func TestEventBuilder_Metadata(t *testing.T) {
	metadata := map[string]string{
		"key1": "value1",
		"key2": "value2",
	}

	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Test", 1).
		Response("Test", 1, "stop").
		Latency(100).
		Cost(0.001).
		Metadata(metadata).
		Build()

	if len(event.Metadata) != 2 {
		t.Errorf("Expected 2 metadata items, got %d", len(event.Metadata))
	}

	if event.Metadata["key1"] != "value1" {
		t.Errorf("Expected metadata key1=value1, got %s", event.Metadata["key1"])
	}
}

func TestEventBuilder_AddMetadata(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Test", 1).
		Response("Test", 1, "stop").
		Latency(100).
		Cost(0.001).
		AddMetadata("key1", "value1").
		AddMetadata("key2", "value2").
		Build()

	if len(event.Metadata) != 2 {
		t.Errorf("Expected 2 metadata items, got %d", len(event.Metadata))
	}
}

func TestEventBuilder_Errors(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Test", 1).
		Response("Test", 1, "stop").
		Latency(100).
		Cost(0.001).
		Error("error1").
		Error("error2").
		Build()

	if len(event.Errors) != 2 {
		t.Errorf("Expected 2 errors, got %d", len(event.Errors))
	}
}

func TestEventBuilder_TraceAndSpan(t *testing.T) {
	event := NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Test", 1).
		Response("Test", 1, "stop").
		Latency(100).
		Cost(0.001).
		TraceID("trace-123").
		SpanID("span-456").
		Build()

	if event.TraceID == nil || *event.TraceID != "trace-123" {
		t.Error("Expected trace ID to be set")
	}

	if event.SpanID == nil || *event.SpanID != "span-456" {
		t.Error("Expected span ID to be set")
	}
}

func TestBuildFromOptions(t *testing.T) {
	opts := TrackOptions{
		Service:          "test-service",
		Model:            "gpt-4",
		Prompt:           "Hello",
		Response:         "Hi",
		LatencyMs:        100.0,
		PromptTokens:     1,
		CompletionTokens: 1,
		CostUsd:          0.001,
		FinishReason:     "stop",
	}

	event := BuildFromOptions(opts)

	if event.ServiceName != "test-service" {
		t.Errorf("Expected service 'test-service', got '%s'", event.ServiceName)
	}

	if event.Model != "gpt-4" {
		t.Errorf("Expected model 'gpt-4', got '%s'", event.Model)
	}

	if event.Response.FinishReason != "stop" {
		t.Errorf("Expected finish reason 'stop', got '%s'", event.Response.FinishReason)
	}
}

func TestBuildFromOptions_DefaultFinishReason(t *testing.T) {
	opts := TrackOptions{
		Service:          "test-service",
		Model:            "gpt-4",
		Prompt:           "Hello",
		Response:         "Hi",
		LatencyMs:        100.0,
		PromptTokens:     1,
		CompletionTokens: 1,
		CostUsd:          0.001,
		// FinishReason not set
	}

	event := BuildFromOptions(opts)

	if event.Response.FinishReason != "stop" {
		t.Errorf("Expected default finish reason 'stop', got '%s'", event.Response.FinishReason)
	}
}
