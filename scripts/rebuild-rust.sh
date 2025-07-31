#!/bin/bash

# Script to rebuild Rust in the container and restart the service

echo "Rebuilding Rust application in Docker container..."

# Rebuild in the container
docker exec logseq_spring_thing_webxr bash -c "cd /app && cargo build --release"

# Restart the container to pick up changes
echo "Restarting container..."
docker-compose -f /workspace/ext/docker-compose.dev.yml restart webxr

echo "Done! Check logs with: docker logs -f logseq_spring_thing_webxr"