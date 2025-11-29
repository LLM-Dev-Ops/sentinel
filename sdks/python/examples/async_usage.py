"""Async usage example for Sentinel SDK."""

import asyncio
from sentinel_sdk import AsyncSentinelClient

async def main():
    """Demonstrate async SDK usage."""
    async with AsyncSentinelClient(brokers=["localhost:9092"]) as client:
        # Track multiple requests concurrently
        tasks = []

        for i in range(5):
            task = client.track(
                service="async-example-service",
                model="gpt-4",
                prompt=f"Request {i + 1}",
                response=f"Response {i + 1}",
                latency_ms=100.0 + i * 10,
                prompt_tokens=5,
                completion_tokens=10,
                cost_usd=0.001,
                metadata={"request_id": str(i + 1)}
            )
            tasks.append(task)

        # Wait for all requests to complete
        await asyncio.gather(*tasks)
        print(f"✓ Sent {len(tasks)} events concurrently!")


async def main_with_error_handling():
    """Demonstrate async usage with error handling."""
    from sentinel_sdk import SendError, TimeoutError, ValidationError

    async with AsyncSentinelClient(brokers=["localhost:9092"]) as client:
        try:
            await client.track(
                service="async-example-service",
                model="gpt-4",
                prompt="Test request",
                response="Test response",
                latency_ms=150.0,
                prompt_tokens=5,
                completion_tokens=10,
                cost_usd=0.001,
                timeout=5.0  # 5 second timeout
            )
            print("✓ Event sent successfully!")

        except ValidationError as e:
            print(f"✗ Validation error: {e}")
        except TimeoutError as e:
            print(f"✗ Timeout error: {e}")
        except SendError as e:
            print(f"✗ Send error: {e}")


if __name__ == "__main__":
    print("Example 1: Concurrent requests")
    asyncio.run(main())

    print("\nExample 2: With error handling")
    asyncio.run(main_with_error_handling())
