"""OpenAI integration example for Sentinel SDK."""

import time
from typing import Optional

from sentinel_sdk import SentinelClient

# Note: This example requires openai package
# pip install openai


def calculate_openai_cost(model: str, prompt_tokens: int, completion_tokens: int) -> float:
    """Calculate cost for OpenAI API usage.

    This is a simplified example. Update with actual pricing.
    """
    pricing = {
        "gpt-4": {"prompt": 0.03 / 1000, "completion": 0.06 / 1000},
        "gpt-3.5-turbo": {"prompt": 0.0015 / 1000, "completion": 0.002 / 1000},
    }

    if model in pricing:
        return (
            prompt_tokens * pricing[model]["prompt"] +
            completion_tokens * pricing[model]["completion"]
        )
    return 0.0


class SentinelTrackedOpenAI:
    """OpenAI client wrapper with Sentinel tracking."""

    def __init__(self, service_name: str, sentinel_brokers: list):
        """Initialize tracked OpenAI client."""
        import openai

        self.openai = openai
        self.service_name = service_name
        self.sentinel = SentinelClient(brokers=sentinel_brokers)

    def chat_completion(self, messages: list, model: str = "gpt-3.5-turbo", **kwargs):
        """Create chat completion with automatic tracking."""
        start_time = time.time()

        try:
            response = self.openai.ChatCompletion.create(
                model=model,
                messages=messages,
                **kwargs
            )

            latency_ms = (time.time() - start_time) * 1000

            # Extract prompt and response
            prompt = "\n".join([msg.get("content", "") for msg in messages])
            response_text = response.choices[0].message.content

            # Track the request
            self.sentinel.track(
                service=self.service_name,
                model=model,
                prompt=prompt,
                response=response_text,
                latency_ms=latency_ms,
                prompt_tokens=response.usage.prompt_tokens,
                completion_tokens=response.usage.completion_tokens,
                cost_usd=calculate_openai_cost(
                    model,
                    response.usage.prompt_tokens,
                    response.usage.completion_tokens
                ),
                finish_reason=response.choices[0].finish_reason,
                metadata={
                    "model_version": response.model,
                    "response_id": response.id,
                }
            )

            return response

        except Exception as e:
            latency_ms = (time.time() - start_time) * 1000

            # Track the error
            self.sentinel.track(
                service=self.service_name,
                model=model,
                prompt="\n".join([msg.get("content", "") for msg in messages]),
                response="",
                latency_ms=latency_ms,
                prompt_tokens=0,
                completion_tokens=0,
                cost_usd=0.0,
                finish_reason="error",
                errors=[str(e)]
            )

            raise

    def close(self):
        """Close Sentinel client."""
        self.sentinel.close()


def main():
    """Example usage of tracked OpenAI client."""
    # Initialize tracked client
    client = SentinelTrackedOpenAI(
        service_name="openai-example",
        sentinel_brokers=["localhost:9092"]
    )

    # Make a tracked request
    messages = [
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "What is the capital of France?"}
    ]

    try:
        response = client.chat_completion(messages, model="gpt-3.5-turbo")
        print(f"Response: {response.choices[0].message.content}")
        print("✓ Request tracked successfully!")
    except Exception as e:
        print(f"Error: {e}")
    finally:
        client.close()


if __name__ == "__main__":
    main()
