---
title: Protocol Reference
description: Complete specification for all VisionClaw communication protocols
category: reference
difficulty-level: intermediate
tags:
  - protocols
  - websocket
  - mcp
updated-date: 2025-01-29
---

# Protocol Reference

Complete technical specification for all VisionClaw communication protocols.

---

## Protocol Overview

| Protocol | Transport | Use Case | Documentation |
|----------|-----------|----------|---------------|
| **Binary WebSocket** | WebSocket | Real-time graph updates | [binary-websocket.md](../websocket-binary.md) |
| **MCP** | TCP | Agent orchestration | [mcp-protocol.md](./mcp-protocol.md) |
| **REST HTTP** | HTTP/HTTPS | CRUD operations | [REST API](../rest-api.md) |
| **Solid/LDP** | HTTP | Decentralized data | [REST API](../rest-api.md) |

---

## Binary WebSocket Protocol

### Protocol Versions

| Version | Status | Bytes/Node | Use Case |
|---------|--------|------------|----------|
| **V2** | **Current** | 36 | Production standard |
| V3 | Stable | 48 | Analytics extension |
| V4 | Experimental | 16 | Delta encoding |
| V1 | Deprecated | 34 | Legacy (ID limit: 16383) |

### Quick Reference

**V2 Wire Format** (36 bytes/node):
```
[0]      Protocol Version (u8) = 2
[1-4]    Node ID (u32) with type flags
[5-16]   Position X/Y/Z (3xf32)
[17-28]  Velocity X/Y/Z (3xf32)
[29-32]  SSSP Distance (f32)
[33-36]  SSSP Parent (i32)
```

**Performance** (100K nodes @ 60 FPS):
- Binary V2: 3.6 MB, 0.8ms parse, 80% smaller than JSON
- JSON (deprecated): 18 MB, 12ms parse

See [binary-websocket.md](../websocket-binary.md) for complete specification.

---

## MCP Protocol

### Connection

**Transport**: TCP
**Port**: 9500 (configurable via `MCP_TCP_PORT`)

### Message Format

**Structure**: JSON-RPC 2.0

```json
{
  "jsonrpc": "2.0",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "method": "method_name",
  "params": {
    "key": "value"
  }
}
```

### Key Methods

| Method | Description |
|--------|-------------|
| `swarm_init` | Initialize multi-agent swarm |
| `agent_spawn` | Spawn agent in swarm |
| `task_orchestrate` | Orchestrate task across swarm |

See [mcp-protocol.md](./mcp-protocol.md) for complete specification.

---

## Protocol Comparison

### WebSocket vs REST

| Aspect | WebSocket | REST |
|--------|-----------|------|
| **Connection** | Persistent | Stateless |
| **Direction** | Bidirectional | Request/Response |
| **Overhead** | Low (no headers after handshake) | High (headers every request) |
| **Real-time** | Excellent | Poor (polling required) |
| **Use Case** | Real-time updates | CRUD operations |

### Binary vs JSON

| Aspect | Binary | JSON |
|--------|--------|------|
| **Size** | 36 bytes/node | 180+ bytes/node |
| **Parse Time** | 0.8 ms (100K nodes) | 12 ms (100K nodes) |
| **Human Readable** | No | Yes |
| **Bandwidth** | **80% less** | Baseline |
| **Use Case** | High-frequency updates | Metadata, control |

### MCP vs REST

| Aspect | MCP | REST |
|--------|-----|------|
| **Transport** | TCP (raw sockets) | HTTP |
| **Format** | JSON-RPC 2.0 | JSON |
| **Overhead** | Very low | Moderate (HTTP headers) |
| **Firewall** | May be blocked | Usually allowed |
| **Use Case** | Agent orchestration | General API |

---

## Protocol Stack

```
+------------------+
|  Application     |
+------------------+
|  Delta Encoding  |
+------------------+
|  Binary Protocol |
+------------------+
|  WebSocket       |
+------------------+
|  TCP/TLS         |
+------------------+
```

---

## Related Documentation

- [WebSocket Binary Protocol](../websocket-binary.md)
- [REST API](../rest-api.md)
- [Error Codes](../error-codes.md)
