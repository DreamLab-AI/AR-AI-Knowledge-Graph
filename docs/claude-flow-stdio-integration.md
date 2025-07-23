# Claude Flow Stdio Integration

This document describes how the Rust backend has been modified to work with Claude Flow's stdio-based MCP interface.

## Overview

Claude Flow's MCP server runs in stdio mode, not as a WebSocket server. We've modified the Rust connector to spawn the Claude Flow process and communicate via stdin/stdout using JSON-RPC.

## Changes Made

### 1. New Stdio Transport (`src/services/claude_flow/transport/stdio.rs`)

- Spawns `npx claude-flow@alpha mcp start` as a subprocess
- Communicates via stdin/stdout using JSON-RPC protocol
- Handles asynchronous message parsing and response routing
- Manages process lifecycle (spawn/terminate)

### 2. Updated Client Builder

The `ClaudeFlowClientBuilder` now supports three transport types:
- `Http` - HTTP REST API
- `WebSocket` - WebSocket for bidirectional communication  
- `Stdio` - Process-based stdio communication (default)

### 3. Updated ClaudeFlowActor

The actor now uses stdio transport by default:

```rust
let mut client = ClaudeFlowClientBuilder::new()
    .use_stdio()  // Use stdio transport
    .build()
    .await
    .expect("Failed to build ClaudeFlowClient");
```

## Usage in Docker Container

### Prerequisites

The container needs Node.js/npm installed to run Claude Flow. The startup script handles this automatically.

### Starting the Backend

Use the provided startup script:

```bash
/workspace/ext/scripts/start-backend-with-claude-flow.sh
```

This script:
1. Checks for Node.js/npm and installs if needed
2. Tests Claude Flow availability
3. Starts the Rust backend

### Manual Start

If you need to start components manually:

```bash
# From inside the container
cd /app

# Ensure npm is available
which npx || (curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - && apt-get install -y nodejs)

# Test Claude Flow
npx claude-flow@alpha --version

# Start the Rust backend
./target/release/visionflow
# or with cargo
cargo run --release --features gpu
```

## How It Works

1. When `ClaudeFlowActor` starts, it creates a stdio transport client
2. The client spawns `npx claude-flow@alpha mcp start` as a subprocess
3. Communication happens via JSON-RPC over stdin/stdout
4. The client handles request/response correlation and notifications
5. When the actor stops, it terminates the Claude Flow process

## Benefits

- No need for separate WebSocket server
- Direct process communication (lower latency)
- Automatic process lifecycle management
- Works with Claude Flow's standard MCP interface

## Troubleshooting

### Claude Flow Not Found

If you see "command not found" errors:
```bash
# Install Node.js
curl -fsSL https://deb.nodesource.com/setup_lts.x | bash -
apt-get install -y nodejs
```

### Process Communication Issues

Check Claude Flow is working:
```bash
npx claude-flow@alpha mcp start
# Should see initialization messages
```

### Debug Logging

Enable debug logging in Rust:
```bash
RUST_LOG=debug ./target/release/visionflow
```

## Future Improvements

1. Connection pooling for multiple Claude Flow instances
2. Automatic reconnection on process failure
3. Performance metrics for stdio communication
4. Support for Claude Flow configuration options