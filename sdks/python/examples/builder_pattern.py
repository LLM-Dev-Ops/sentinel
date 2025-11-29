"""Builder pattern example for Sentinel SDK."""

from sentinel_sdk import SentinelClient, TelemetryEventBuilder

def main():
    """Demonstrate builder pattern for event construction."""
    client = SentinelClient(brokers=["localhost:9092"])

    # Build event using fluent builder pattern
    event = (TelemetryEventBuilder()
        .service("example-service")
        .model("gpt-4-turbo")
        .prompt(text="Write a haiku about programming", tokens=12)
        .response(
            text="Code flows like water\nBugs emerge from the shadows\nDebugger saves day",
            tokens=25,
            finish_reason="stop"
        )
        .latency(200.5)
        .cost(0.002)
        .metadata({
            "region": "us-west-2",
            "user_id": "user-123",
            "session_id": "session-456"
        })
        .trace_id("trace-abc-123")
        .span_id("span-xyz-789")
        .build())

    # Send the event
    client.send(event)
    print(f"✓ Event {event.event_id} sent successfully!")

    # Build event with errors
    error_event = (TelemetryEventBuilder()
        .service("example-service")
        .model("gpt-4")
        .prompt(text="Generate a very long response", tokens=10)
        .response(text="", tokens=0, finish_reason="error")
        .latency(5000.0)
        .cost(0.0)
        .add_error("Request timeout")
        .add_error("Max retries exceeded")
        .build())

    client.send(error_event)
    print(f"✓ Error event {error_event.event_id} sent successfully!")

    client.close()


if __name__ == "__main__":
    main()
