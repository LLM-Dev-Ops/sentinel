#!/bin/bash
# =============================================================================
# LLM-Sentinel Cloud Run Deployment Script
# =============================================================================
#
# This script deploys LLM-Sentinel to Cloud Run.
#
# Usage:
#   ./deploy.sh <ENV> [REGION]
#
# Examples:
#   ./deploy.sh dev
#   ./deploy.sh staging us-central1
#   ./deploy.sh prod us-central1

set -euo pipefail

ENV="${1:-dev}"
REGION="${2:-us-central1}"
PROJECT_ID=$(gcloud config get-value project)
SERVICE_NAME="llm-sentinel"

echo "=== LLM-Sentinel Deployment ==="
echo "Environment: $ENV"
echo "Project: $PROJECT_ID"
echo "Region: $REGION"
echo ""

# Validate environment
if [[ ! "$ENV" =~ ^(dev|staging|prod)$ ]]; then
    echo "Error: Invalid environment. Must be dev, staging, or prod."
    exit 1
fi

# =============================================================================
# Step 1: Build and push using Cloud Build
# =============================================================================
echo "Step 1: Building and deploying via Cloud Build..."

cd "$(dirname "$0")/../.."

gcloud builds submit \
    --config cloudbuild.yaml \
    --substitutions="_ENV=${ENV},_REGION=${REGION}" \
    --project="$PROJECT_ID"

# =============================================================================
# Step 2: Get service URL
# =============================================================================
echo ""
echo "Step 2: Retrieving service URL..."

SERVICE_URL=$(gcloud run services describe "${SERVICE_NAME}-${ENV}" \
    --region="$REGION" \
    --project="$PROJECT_ID" \
    --format='value(status.url)')

echo "Service URL: $SERVICE_URL"

# =============================================================================
# Step 3: Run post-deployment verification
# =============================================================================
echo ""
echo "Step 3: Running post-deployment verification..."

echo ""
echo "--- Health Checks ---"
echo "Liveness: $(curl -s -o /dev/null -w '%{http_code}' "${SERVICE_URL}/health/live")"
echo "Readiness: $(curl -s -o /dev/null -w '%{http_code}' "${SERVICE_URL}/health/ready")"
echo "Full Health: $(curl -s -o /dev/null -w '%{http_code}' "${SERVICE_URL}/health")"

echo ""
echo "--- Agent Endpoints ---"
for agent in anomaly drift alerting correlation rca; do
    code=$(curl -s -o /dev/null -w '%{http_code}' "${SERVICE_URL}/api/v1/agents/${agent}/stats")
    echo "${agent}: ${code}"
done

echo ""
echo "--- Query Endpoints ---"
echo "telemetry: $(curl -s -o /dev/null -w '%{http_code}' "${SERVICE_URL}/api/v1/telemetry")"
echo "anomalies: $(curl -s -o /dev/null -w '%{http_code}' "${SERVICE_URL}/api/v1/anomalies")"

echo ""
echo "--- Metrics ---"
echo "metrics: $(curl -s -o /dev/null -w '%{http_code}' "${SERVICE_URL}/metrics")"

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "=== Deployment Complete ==="
echo ""
echo "Service: ${SERVICE_NAME}-${ENV}"
echo "URL: ${SERVICE_URL}"
echo ""
echo "Agent Endpoints:"
echo "  - POST ${SERVICE_URL}/api/v1/agents/anomaly/detect"
echo "  - POST ${SERVICE_URL}/api/v1/agents/drift/detect"
echo "  - POST ${SERVICE_URL}/api/v1/agents/alerting/evaluate"
echo "  - POST ${SERVICE_URL}/api/v1/agents/correlation/correlate"
echo "  - POST ${SERVICE_URL}/api/v1/agents/rca/analyze"
echo ""
echo "CLI Commands:"
echo "  sentinel agent inspect anomaly --endpoint ${SERVICE_URL}"
echo "  sentinel agent diagnose --endpoint ${SERVICE_URL}"
echo ""
