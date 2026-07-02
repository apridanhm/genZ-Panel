#!/bin/bash
# Production script - use prod overrides

set -e

echo "🚀 Starting production environment..."

# Check if .env exists
if [ ! -f .env ]; then
    echo "❌ .env file not found!"
    echo "Please create .env file first"
    exit 1
fi

# Load environment
export $(cat .env | grep -v '^#' | xargs)

# Check required variables
if [ -z "$POSTGRES_PASSWORD" ] || [ -z "$JWT_SECRET" ] || [ -z "$REGISTRY" ]; then
    echo "❌ Missing required environment variables!"
    echo "Required: POSTGRES_PASSWORD, JWT_SECRET, REGISTRY"
    exit 1
fi

# Start services with production overrides
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d "$@"

echo "✅ Production environment started!"
echo "Access panel at: https://${PANEL_DOMAIN}"

