"""Basic usage example for Sentinel SDK."""

from sentinel_sdk import SentinelClient

def main():
    """Demonstrate basic SDK usage."""
    # Create client with explicit broker configuration
    client = SentinelClient(brokers=["localhost:9092"])

    # Track a simple LLM request
    client.track(
        service="example-service",
        model="gpt-4",
        prompt="What is the capital of France?",
        response="The capital of France is Paris.",
        latency_ms=150.5,
        prompt_tokens=8,
        completion_tokens=7,
        cost_usd=0.001
    )

    print("✓ Event sent successfully!")

    # Clean up
    client.close()


def main_with_context_manager():
    """Demonstrate usage with context manager (recommended)."""
    with SentinelClient(brokers=["localhost:9092"]) as client:
        client.track(
            service="example-service",
            model="gpt-4",
            prompt="Explain quantum computing in simple terms.",
            response="Quantum computing uses quantum mechanics...",
            latency_ms=250.0,
            prompt_tokens=15,
            completion_tokens=50,
            cost_usd=0.005,
            metadata={
                "region": "us-east-1",
                "environment": "production"
            }
        )

        print("✓ Event sent successfully with context manager!")


if __name__ == "__main__":
    print("Example 1: Basic usage")
    main()

    print("\nExample 2: With context manager")
    main_with_context_manager()
