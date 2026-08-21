#!/bin/bash
set -e

echo "[STARTUP] Starting production environment..."

# Function to log messages with timestamps
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1"
}

# Check GPU availability
log "Checking GPU availability..."
if command -v nvidia-smi &>/dev/null; then
    GPU_INFO=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -n1 || true)
    if [ -n "$GPU_INFO" ]; then
        log "GPU detected: $GPU_INFO"
    fi
fi

# Create necessary directories
mkdir -p /app/logs /var/log/nginx /var/run/nginx

# Verify the pre-built production binary exists
if [ ! -x /app/visionclaw-server ]; then
    log "ERROR: Production binary /app/visionclaw-server not found or not executable!"
    exit 1
else
    log "Using pre-built production binary"
fi

# Verify PTX files are in place (copied during Docker build)
if [ -f /app/src/utils/ptx/visionclaw_unified.ptx ]; then
    log "PTX file present"
else
    log "WARNING: PTX file not found - GPU features may not work"
fi

# Start Rust backend on the loopback-only upstream port used by nginx.
log "Starting Rust backend on port 4001..."
SYSTEM_NETWORK_PORT=4001 RUST_LOG=${RUST_LOG:-info} /app/visionclaw-server &
BACKEND_PID=$!

terminate_backend() {
    kill "$BACKEND_PID" 2>/dev/null || true
    wait "$BACKEND_PID" 2>/dev/null || true
}
trap terminate_backend EXIT INT TERM

log "Waiting for backend to start..."
for _ in {1..60}; do
    if nc -z 127.0.0.1 4001; then
        log "Backend is accepting connections on port 4001"
        break
    fi
    if ! kill -0 "$BACKEND_PID" 2>/dev/null; then
        log "ERROR: Backend crashed during startup"
        wait "$BACKEND_PID"
        exit 1
    fi
    sleep 1
done

if ! nc -z 127.0.0.1 4001; then
    log "ERROR: Backend did not bind port 4001 within 60 seconds"
    exit 1
fi

# nginx stays in the foreground as PID 1; the EXIT trap stops the backend.
log "Starting nginx on port 3001..."
nginx -g "daemon off;"
