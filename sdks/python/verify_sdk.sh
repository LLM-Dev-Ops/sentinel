#!/bin/bash
# Verification script for LLM Sentinel Python SDK

echo "=== LLM Sentinel Python SDK Verification ==="
echo

echo "1. Checking directory structure..."
test -d sentinel_sdk && echo "✓ sentinel_sdk/ exists" || echo "✗ sentinel_sdk/ missing"
test -d examples && echo "✓ examples/ exists" || echo "✗ examples/ missing"
test -d tests && echo "✓ tests/ exists" || echo "✗ tests/ missing"
echo

echo "2. Checking core modules..."
for file in __init__ types events exceptions config builders client; do
    test -f sentinel_sdk/${file}.py && echo "✓ ${file}.py exists" || echo "✗ ${file}.py missing"
done
echo

echo "3. Checking documentation..."
for file in README.md QUICKSTART.md API.md OVERVIEW.md CHANGELOG.md; do
    test -f ${file} && echo "✓ ${file} exists" || echo "✗ ${file} missing"
done
echo

echo "4. Checking packaging files..."
for file in pyproject.toml setup.py requirements.txt requirements-dev.txt MANIFEST.in; do
    test -f ${file} && echo "✓ ${file} exists" || echo "✗ ${file} missing"
done
echo

echo "5. Checking examples..."
for file in basic_usage async_usage builder_pattern openai_integration; do
    test -f examples/${file}.py && echo "✓ ${file}.py exists" || echo "✗ ${file}.py missing"
done
echo

echo "6. Checking tests..."
for file in test_events test_types test_builders; do
    test -f tests/${file}.py && echo "✓ ${file}.py exists" || echo "✗ ${file}.py missing"
done
echo

echo "7. Code statistics..."
echo "Total files: $(find . -type f | wc -l)"
echo "Python files: $(find . -name '*.py' | wc -l)"
echo "Lines of Python code: $(find . -name '*.py' -exec wc -l {} + | tail -1 | awk '{print $1}')"
echo "Documentation files: $(find . -name '*.md' | wc -l)"
echo

echo "8. Module imports check..."
python3 -c "from sentinel_sdk import SentinelClient, AsyncSentinelClient, TelemetryEventBuilder, SentinelConfig" 2>/dev/null && echo "✓ All imports successful" || echo "✗ Import errors (dependencies may not be installed)"
echo

echo "=== Verification Complete ==="
