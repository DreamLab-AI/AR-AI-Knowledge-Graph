#!/bin/bash

# MCP WebSocket Relay Stop Script

PID_FILE="/tmp/mcp-ws-relay.pid"

if [ ! -f "$PID_FILE" ]; then
    echo "MCP WebSocket relay is not running (no PID file found)"
    exit 0
fi

PID=$(cat "$PID_FILE")

if ps -p "$PID" > /dev/null 2>&1; then
    echo "Stopping MCP WebSocket relay (PID: $PID)..."
    kill "$PID"
    sleep 2
    
    # Force kill if still running
    if ps -p "$PID" > /dev/null 2>&1; then
        echo "Force killing..."
        kill -9 "$PID"
    fi
    
    rm "$PID_FILE"
    echo "MCP WebSocket relay stopped"
else
    echo "MCP WebSocket relay is not running (process not found)"
    rm "$PID_FILE"
fi