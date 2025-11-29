package main

import (
	"context"
	"errors"
	"log"

	"github.com/llm-dev-ops/sentinel-sdk-go/sentinel"
)

func main() {
	// Create client
	client, err := sentinel.NewClient(
		sentinel.WithBrokers([]string{"localhost:9092"}),
		sentinel.WithTopic("llm.telemetry"),
	)
	if err != nil {
		log.Fatalf("Failed to create client: %v", err)
	}
	defer client.Close()

	// Example 1: Validation Error
	log.Println("Example 1: Validation Error")
	invalidEvent := sentinel.NewEventBuilder().
		Service(""). // Empty service name will fail validation
		Model("gpt-4").
		Prompt("Test", 1).
		Response("Test", 1, "stop").
		Latency(100).
		Cost(0.0001).
		Build()

	err = client.Send(context.Background(), invalidEvent)
	if err != nil {
		var validationErr *sentinel.ValidationError
		if errors.As(err, &validationErr) {
			log.Printf("Validation error detected!")
			log.Printf("  Field: %s", validationErr.Field)
			log.Printf("  Message: %s", validationErr.Message)
			log.Printf("  Error code: %s", validationErr.Code())
		}
	}

	// Example 2: Handling Send Errors
	log.Println("\nExample 2: Send Error Handling")
	// Create client with invalid broker to simulate send error
	badClient, err := sentinel.NewClient(
		sentinel.WithBrokers([]string{"invalid-broker:9092"}),
		sentinel.WithTopic("llm.telemetry"),
		sentinel.WithMaxRetries(2),
	)
	if err != nil {
		log.Printf("Failed to create client: %v", err)
	} else {
		defer badClient.Close()

		event := sentinel.NewEventBuilder().
			Service("test-service").
			Model("gpt-4").
			Prompt("Test", 1).
			Response("Test", 1, "stop").
			Latency(100).
			Cost(0.0001).
			Build()

		err = badClient.Send(context.Background(), event)
		if err != nil {
			var sendErr *sentinel.SendError
			if errors.As(err, &sendErr) {
				log.Printf("Send error detected!")
				log.Printf("  Topic: %s", sendErr.Topic)
				log.Printf("  Event ID: %s", sendErr.EventID)
				log.Printf("  Retries: %d", sendErr.Retries)
				log.Printf("  Error code: %s", sendErr.Code())
			}
		}
	}

	// Example 3: Handling Multiple Error Types
	log.Println("\nExample 3: Comprehensive Error Handling")
	err = sendEvent(client)
	if err != nil {
		handleError(err)
	}

	// Example 4: Recording Errors in Events
	log.Println("\nExample 4: Recording Application Errors")
	eventWithErrors := sentinel.NewEventBuilder().
		Service("error-prone-service").
		Model("gpt-4").
		Prompt("Test prompt", 2).
		Response("Partial response", 2, "length").
		Latency(5000.0). // High latency
		Cost(0.01).
		Error("Request timeout after 5s").
		Error("Retried 3 times").
		AddMetadata("error_type", "timeout").
		Build()

	err = client.Send(context.Background(), eventWithErrors)
	if err != nil {
		log.Printf("Failed to send error event: %v", err)
	} else {
		log.Println("Event with errors recorded successfully!")
		log.Printf("  Has errors: %v", eventWithErrors.HasErrors())
		log.Printf("  Error count: %d", len(eventWithErrors.Errors))
		log.Printf("  Errors: %v", eventWithErrors.Errors)
	}

	log.Println("\nError handling examples completed!")
}

func sendEvent(client *sentinel.Client) error {
	event := sentinel.NewEventBuilder().
		Service("test-service").
		Model("gpt-4").
		Prompt("Test", 1).
		Response("Test", 1, "stop").
		Latency(100).
		Cost(0.0001).
		Build()

	return client.Send(context.Background(), event)
}

func handleError(err error) {
	// Type assertion for different error types
	var validationErr *sentinel.ValidationError
	var sendErr *sentinel.SendError
	var connErr *sentinel.ConnectionError
	var configErr *sentinel.ConfigError

	switch {
	case errors.Is(err, sentinel.ErrClientClosed):
		log.Println("Error: Client is closed")
		log.Println("Action: Recreate the client")

	case errors.Is(err, sentinel.ErrContextCanceled):
		log.Println("Error: Context was canceled")
		log.Println("Action: Retry with a new context or extend timeout")

	case errors.Is(err, sentinel.ErrInvalidEvent):
		log.Println("Error: Event validation failed")
		log.Println("Action: Check event fields for correctness")

	case errors.As(err, &validationErr):
		log.Printf("Validation Error:")
		log.Printf("  Field: %s", validationErr.Field)
		log.Printf("  Message: %s", validationErr.Message)
		log.Printf("  Value: %v", validationErr.Value)
		log.Println("Action: Fix the invalid field and retry")

	case errors.As(err, &sendErr):
		log.Printf("Send Error:")
		log.Printf("  Topic: %s", sendErr.Topic)
		log.Printf("  Event ID: %s", sendErr.EventID)
		log.Printf("  Retries attempted: %d", sendErr.Retries)
		log.Printf("  Underlying error: %v", sendErr.Err)
		log.Println("Action: Check Kafka broker connectivity")

	case errors.As(err, &connErr):
		log.Printf("Connection Error:")
		log.Printf("  Brokers: %v", connErr.Brokers)
		log.Printf("  Error: %v", connErr.Err)
		log.Println("Action: Verify broker addresses and network connectivity")

	case errors.As(err, &configErr):
		log.Printf("Configuration Error:")
		log.Printf("  Field: %s", configErr.Field)
		log.Printf("  Message: %s", configErr.Message)
		log.Println("Action: Fix the configuration and recreate client")

	default:
		log.Printf("Unexpected error: %v", err)
		log.Println("Action: Check logs for more details")
	}
}
