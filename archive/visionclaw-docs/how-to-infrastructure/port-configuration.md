---
title: Multi-Agent Docker Port Configuration
description: This document explains the port allocation and purpose for the Multi-Agent Docker Environment.
category: how-to
tags:
  - tutorial
  - api
  - docker
updated-date: 2025-12-18
difficulty-level: intermediate
---


# Multi-Agent Docker Port Configuration

## Overview

This document explains the port allocation and purpose for the Multi-Agent Docker Environment.

## Port Map

### Core Application Ports

| Port | Service | Protocol | Access | Purpose |
|------|---------|----------|--------|---------|
| **3000** | Claude Flow UI | HTTP | Public | Web interface for Claude Flow |
| **3002** | MCP WebSocket Bridge | WebSocket | Public | WebSocket-to-stdio MCP bridge |

### MCP Server Ports

| Port | Service | Protocol | Access | Purpose | Status |
|------|---------|----------|--------|---------|--------|
| **9500** | MCP TCP Server | TCP/MCP | Public | **PRIMARY** - Shared claude-flow MCP server | ✅ **CRITICAL** |
| **9502** | Claude-Flow TCP Proxy | TCP/MCP | Public | Isolated claude-flow sessions (one per client) | ✅ Optional |
| **9503** | CF-TCP Health Check | HTTP | Localhost | Health endpoint for port 9502 | ✅ Optional |

### GUI Container Ports (gui-tools-service)

| Port | Service | Protocol | Access | Purpose |
|------|---------|----------|--------|---------|
| **5901** | VNC Server | VNC | Public | Remote desktop access to GUI tools |
| **9876** | Blender MCP | TCP/MCP | Internal | Blender 3D modeling bridge |
| **9877** | QGIS MCP | TCP/MCP | Internal | QGIS geospatial analysis bridge |
| **9878** | PBR Generator | TCP/MCP | Internal | PBR texture generation service |
| **9879** | Playwright MCP | TCP/MCP | Internal | Browser automation service |

## Port Details

### Port 9500 - MCP TCP Server (PRIMARY)

**Status**: ✅ **CRITICAL - ALWAYS REQUIRED**

**Service**: `mcp-tcp-server` (managed by supervisord)
**Script**: `/app/core-assets/scripts/mcp-tcp-server.js`
**Listen Address**: `0.0.0.0:9500`

**Purpose**:
- Main MCP TCP server for external system integration
- Provides JSON-RPC 2.0 over TCP for MCP protocol
- Spawns and manages shared claude-flow instance
- All external clients connect through this port

**Usage**:
```bash
# Test connection
echo '{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}' | nc localhost 9500
```

**Environment Variables**:
- `MCP-TCP-PORT` - Default: 9500
- `MCP-TCP-AUTOSTART` - Default: true

---

### Port 9502 - Claude-Flow TCP Proxy (Optional)

**Status**: ✅ Optional (for isolated sessions)

**Service**: `claude-flow-tcp` (managed by supervisord)
**Script**: `/app/core-assets/scripts/claude-flow-tcp-proxy.js`
**Listen Address**: `0.0.0.0:9502`

**Purpose**:
- Provides **isolated claude-flow processes** per TCP connection
- Each client gets their own separate claude-flow instance
- Prevents state sharing between different external projects
- Max concurrent sessions configurable (default: 10)

**Difference from Port 9500**:
- **9500**: All clients share **one** claude-flow instance (shared state)
- **9502**: Each client gets **their own** claude-flow instance (isolated state)

**When to use**:
- Multiple independent external projects
- Need session isolation and separate state
- Running tests that shouldn't interfere

**Environment Variables**:
- `CLAUDE-FLOW-TCP-PORT` - Default: 9502
- `CLAUDE-FLOW-MAX-SESSIONS` - Default: 10

**Usage**:
```bash
# Connect to get isolated session
nc localhost 9502
{"jsonrpc":"2.0","id":"1","method":"initialize","params":{}}
```

---

### Port 9503 - Claude-Flow TCP Health (Optional)

**Status**: ✅ Optional (monitors 9502)

**Service**: Built into `claude-flow-tcp-proxy.js`
**Listen Address**: `127.0.0.1:9503` (localhost only)

**Purpose**:
- HTTP health check endpoint for port 9502
- Returns JSON with active sessions and capacity
- Automatically serves on `CLAUDE-FLOW-TCP-PORT + 1`

**Response Format**:
```json
{
  "active-sessions": 2,
  "max-sessions": 10,
  "port": 9502
}
```

**Usage**:
```bash
# Check health
curl http://localhost:9503
```

---

## Removed Ports

### Port 9501 - MCP Health Check (REMOVED)

**Status**: ❌ **Removed** (not implemented)

**Reason**:
- Was supposed to be HTTP health endpoint for port 9500
- Never implemented in `mcp-tcp-server.js`
- Caused Docker health checks to fail
- Replaced by disabling health check in docker-compose.yml

---

## Configuration Changes Made

### docker-compose.yml Updates:

1. **Removed port 9501** from port mappings
2. **Removed `MCP-HEALTH-PORT=9501`** environment variable
3. **Disabled health check** (commented out)
4. **Updated port comments** with clearer descriptions

### Minimal Configuration:

If you only need basic MCP functionality, you can disable optional services:

```yaml
# In supervisord.conf, comment out:
# [program:claude-flow-tcp]  # Disables ports 9502/9503
```

Then remove from docker-compose.yml:
```yaml
ports:
  - "3000:3000"    # Claude Flow UI
  - "3002:3002"    # WebSocket Bridge
  - "9500:9500"    # MCP TCP Server - ONLY THIS IS REQUIRED
```

---

## Resource Configuration

Both containers default to:
- **Memory**: 16GB (configurable via `DOCKER-MEMORY`)
- **CPU**: 4 cores (configurable via `DOCKER-CPUS`)

Configure in `.env`:
```bash
DOCKER-MEMORY=16g
DOCKER-CPUS=4
```

---

## Logging

All services now log to stdout/stderr for Docker logs visibility:

```bash
# View all logs
docker logs multi-agent-container

# Follow logs
docker logs -f multi-agent-container

# View last 50 lines
docker logs --tail 50 multi-agent-container

# View supervisor service logs
docker exec multi-agent-container supervisorctl status
```

---

---

## Related Documentation

- [Deployment Guide](../deployment-guide.md)
- [Pipeline Operator Runbook](../operations/pipeline-operator-runbook.md)
- [Troubleshooting Guide](troubleshooting.md)
- [XR Architecture](../../explanation/xr-architecture.md)

## Quick Reference

### Essential Ports (Cannot Disable)
- **9500** - MCP TCP Server

### Optional Ports (Can Disable If Not Needed)
- **3000** - Claude Flow UI (if not using web interface)
- **3002** - WebSocket Bridge (if only using TCP)
- **9502** - Isolated sessions (if shared state is acceptable)
- **9503** - Health check (if not monitoring 9502)

### External Dependencies (GUI Container)
- **5901** - VNC (for visual access to GUI tools)
- **9876-9879** - GUI tool bridges (Blender, QGIS, PBR, Playwright)

---
