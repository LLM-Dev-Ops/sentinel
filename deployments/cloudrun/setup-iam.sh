#!/bin/bash
# =============================================================================
# LLM-Sentinel Cloud Run IAM Setup
# =============================================================================
#
# This script sets up the required IAM permissions for LLM-Sentinel deployment.
# Run this once before deploying the service.
#
# Usage:
#   ./setup-iam.sh <PROJECT_ID> <REGION>
#
# Example:
#   ./setup-iam.sh agentics-dev us-central1

set -euo pipefail

PROJECT_ID="${1:-}"
REGION="${2:-us-central1}"

if [ -z "$PROJECT_ID" ]; then
    echo "Usage: $0 <PROJECT_ID> [REGION]"
    echo "Example: $0 agentics-dev us-central1"
    exit 1
fi

echo "=== LLM-Sentinel IAM Setup ==="
echo "Project: $PROJECT_ID"
echo "Region: $REGION"
echo ""

# Service account name
SA_NAME="llm-sentinel-sa"
SA_EMAIL="${SA_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"

# =============================================================================
# Step 1: Create service account
# =============================================================================
echo "Step 1: Creating service account..."

if gcloud iam service-accounts describe "$SA_EMAIL" --project="$PROJECT_ID" &>/dev/null; then
    echo "  Service account already exists: $SA_EMAIL"
else
    gcloud iam service-accounts create "$SA_NAME" \
        --project="$PROJECT_ID" \
        --display-name="LLM-Sentinel Service Account" \
        --description="Service account for LLM-Sentinel Cloud Run service"
    echo "  Created service account: $SA_EMAIL"
fi

# =============================================================================
# Step 2: Grant required IAM roles (least privilege)
# =============================================================================
echo ""
echo "Step 2: Granting IAM roles..."

# Cloud Run invoker (for internal service-to-service calls)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/run.invoker" \
    --condition=None \
    --quiet

# Secret Manager accessor (for secrets)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/secretmanager.secretAccessor" \
    --condition=None \
    --quiet

# Cloud Trace agent (for distributed tracing)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/cloudtrace.agent" \
    --condition=None \
    --quiet

# Cloud Monitoring metric writer (for metrics)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/monitoring.metricWriter" \
    --condition=None \
    --quiet

# Cloud Logging writer (for logs)
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$SA_EMAIL" \
    --role="roles/logging.logWriter" \
    --condition=None \
    --quiet

echo "  Granted roles:"
echo "    - roles/run.invoker"
echo "    - roles/secretmanager.secretAccessor"
echo "    - roles/cloudtrace.agent"
echo "    - roles/monitoring.metricWriter"
echo "    - roles/logging.logWriter"

# =============================================================================
# Step 3: Create Artifact Registry repository
# =============================================================================
echo ""
echo "Step 3: Creating Artifact Registry repository..."

if gcloud artifacts repositories describe llm-sentinel \
    --location="$REGION" \
    --project="$PROJECT_ID" &>/dev/null; then
    echo "  Repository already exists: llm-sentinel"
else
    gcloud artifacts repositories create llm-sentinel \
        --repository-format=docker \
        --location="$REGION" \
        --project="$PROJECT_ID" \
        --description="LLM-Sentinel container images"
    echo "  Created repository: llm-sentinel"
fi

# =============================================================================
# Step 4: Create required secrets (placeholders)
# =============================================================================
echo ""
echo "Step 4: Creating secret placeholders..."

for secret in ruvector-service-url ruvector-api-key observatory-endpoint; do
    if gcloud secrets describe "$secret" --project="$PROJECT_ID" &>/dev/null; then
        echo "  Secret already exists: $secret"
    else
        echo "placeholder" | gcloud secrets create "$secret" \
            --project="$PROJECT_ID" \
            --data-file=- \
            --replication-policy="automatic"
        echo "  Created secret: $secret (placeholder value - update before deployment)"
    fi
done

# =============================================================================
# Step 5: Grant Cloud Build permissions
# =============================================================================
echo ""
echo "Step 5: Granting Cloud Build permissions..."

PROJECT_NUMBER=$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')
CLOUDBUILD_SA="${PROJECT_NUMBER}@cloudbuild.gserviceaccount.com"

# Cloud Build needs to deploy to Cloud Run
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$CLOUDBUILD_SA" \
    --role="roles/run.admin" \
    --condition=None \
    --quiet

# Cloud Build needs to push to Artifact Registry
gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:$CLOUDBUILD_SA" \
    --role="roles/artifactregistry.writer" \
    --condition=None \
    --quiet

# Cloud Build needs to act as the service account
gcloud iam service-accounts add-iam-policy-binding "$SA_EMAIL" \
    --project="$PROJECT_ID" \
    --member="serviceAccount:$CLOUDBUILD_SA" \
    --role="roles/iam.serviceAccountUser" \
    --quiet

echo "  Granted Cloud Build permissions"

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "=== Setup Complete ==="
echo ""
echo "Service Account: $SA_EMAIL"
echo "Artifact Registry: ${REGION}-docker.pkg.dev/${PROJECT_ID}/llm-sentinel"
echo ""
echo "Next steps:"
echo "1. Update secrets with real values:"
echo "   gcloud secrets versions add ruvector-service-url --data-file=- <<< 'https://ruvector-service.run.app'"
echo "   gcloud secrets versions add ruvector-api-key --data-file=- <<< 'your-api-key'"
echo "   gcloud secrets versions add observatory-endpoint --data-file=- <<< 'https://llm-observatory.run.app/api/v1/ingest'"
echo ""
echo "2. Deploy the service:"
echo "   gcloud builds submit --config cloudbuild.yaml --substitutions=_ENV=dev,_REGION=$REGION"
echo ""
