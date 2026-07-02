#!/bin/bash
# Development script - auto-load override

set -e

echo "🚀 Starting development environment..."

# Load environment
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

# Start services
docker compose up "$@"

