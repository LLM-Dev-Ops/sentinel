package main

import (
	"context"
	"log"
	"time"

	"github.com/llm-dev-ops/sentinel-sdk-go/sentinel"
)

func main() {
	// Create client with default configuration
	client, err := sentinel.NewClient(
		sentinel.WithBrokers([]string{"localhost:9092"}),
		sentinel.WithTopic("llm.telemetry"),
	)
	if err != nil {
		log.Fatalf("Failed to create client: %v", err)
	}
	defer client.Close()

	// Example 1: Simple Track API
	log.Println("Example 1: Simple Track API")
	err = client.Track(context.Background(), sentinel.TrackOptions{
		Service:          "example-service",
		Model:            "gpt-4",
		Prompt:           "What is the capital of France?",
		Response:         "The capital of France is Paris.",
		LatencyMs:        150.5,
		PromptTokens:     8,
		CompletionTokens: 7,
		CostUsd:          0.001,
		FinishReason:     "stop",
		Metadata: map[string]string{
			"user_id": "user-123",
			"region":  "us-east-1",
		},
	})
	if err != nil {
		log.Printf("Failed to send event: %v", err)
	} else {
		log.Println("Event sent successfully!")
	}

	// Example 2: Builder Pattern
	log.Println("\nExample 2: Builder Pattern")
	event := sentinel.NewEventBuilder().
		Service("example-service").
		Model("gpt-4").
		Prompt("Tell me a joke", 4).
		Response("Why did the chicken cross the road? To get to the other side!", 14, "stop").
		Latency(200.0).
		Cost(0.0015).
		AddMetadata("category", "humor").
		AddMetadata("environment", "production").
		Build()

	err = client.Send(context.Background(), event)
	if err != nil {
		log.Printf("Failed to send event: %v", err)
	} else {
		log.Println("Event sent successfully!")
	}

	// Example 3: With Distributed Tracing
	log.Println("\nExample 3: With Distributed Tracing")
	tracedEvent := sentinel.NewEventBuilder().
		Service("example-service").
		Model("claude-3-sonnet").
		Prompt("Explain quantum computing", 3).
		Response("Quantum computing uses quantum mechanics...", 50, "stop").
		Latency(300.0).
		Cost(0.002).
		TraceID("4bf92f3577b34da6a3ce929d0e0e4736").
		SpanID("00f067aa0ba902b7").
		Build()

	err = client.Send(context.Background(), tracedEvent)
	if err != nil {
		log.Printf("Failed to send event: %v", err)
	} else {
		log.Println("Event sent successfully!")
	}

	// Example 4: Batch Send
	log.Println("\nExample 4: Batch Send")
	events := []*sentinel.TelemetryEvent{
		sentinel.NewEventBuilder().
			Service("example-service").
			Model("gpt-4").
			Prompt("Hello", 1).
			Response("Hi there!", 2, "stop").
			Latency(100).
			Cost(0.0001).
			Build(),
		sentinel.NewEventBuilder().
			Service("example-service").
			Model("gpt-4").
			Prompt("How are you?", 3).
			Response("I'm doing well, thank you!", 5, "stop").
			Latency(120).
			Cost(0.0002).
			Build(),
		sentinel.NewEventBuilder().
			Service("example-service").
			Model("gpt-4").
			Prompt("Goodbye", 1).
			Response("See you later!", 3, "stop").
			Latency(110).
			Cost(0.00015).
			Build(),
	}

	err = client.SendBatch(context.Background(), events)
	if err != nil {
		log.Printf("Failed to send batch: %v", err)
	} else {
		log.Printf("Batch of %d events sent successfully!", len(events))
	}

	// Example 5: With Timeout
	log.Println("\nExample 5: With Timeout")
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	err = client.Track(ctx, sentinel.TrackOptions{
		Service:          "example-service",
		Model:            "gpt-4",
		Prompt:           "Test with timeout",
		Response:         "Response",
		LatencyMs:        100,
		PromptTokens:     3,
		CompletionTokens: 1,
		CostUsd:          0.0001,
	})
	if err != nil {
		log.Printf("Failed to send event: %v", err)
	} else {
		log.Println("Event sent successfully!")
	}

	// Display statistics
	log.Println("\nClient Statistics:")
	stats := client.Stats()
	log.Printf("  Messages written: %d", stats.Writes)
	log.Printf("  Messages sent: %d", stats.Messages)
	log.Printf("  Bytes written: %d", stats.Bytes)
	log.Printf("  Errors: %d", stats.Errors)

	log.Println("\nAll examples completed!")
}
