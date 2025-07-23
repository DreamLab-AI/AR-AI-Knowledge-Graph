#!/bin/bash

# Script to start the Rust backend with Claude Flow MCP support

echo "Starting backend with Claude Flow MCP support..."

# Ensure we're in the right directory
cd /workspace/ext || exit 1

# Check if npm/npx is available
if ! command -v npx &> /dev/null; then
    echo "npx not found. Installing Node.js..."
    curl -fsSL https://deb.nodesource.com/setup_lts.x | bash -
    apt-get install -y nodejs
fi

# Test Claude Flow availability
echo "Testing Claude Flow availability..."
if npx claude-flow@alpha --version; then
    echo "Claude Flow is available"
else
    echo "Claude Flow test failed, but continuing..."
fi

# Start the Rust backend
echo "Starting Rust backend..."
cd /app || cd /workspace/ext || exit 1

# Use the binary if it exists, otherwise try cargo
if [ -f "./target/release/visionflow" ]; then
    echo "Running pre-built binary..."
    ./target/release/visionflow
elif command -v cargo &> /dev/null; then
    echo "Building and running with cargo..."
    cargo run --release --features gpu
else
    echo "ERROR: Neither pre-built binary nor cargo found!"
    echo "Please ensure the Rust backend is built or cargo is installed."
    exit 1
fi