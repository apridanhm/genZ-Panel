#!/bin/bash

set -e

echo "Updating Panel Hosting..."

# Pull latest images
docker compose pull

# Restart services with new images
docker compose up -d

# Run database migrations
docker compose exec core-api /app/migrate

echo "Update complete!"