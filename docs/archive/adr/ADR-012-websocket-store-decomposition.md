# ADR-012: WebSocket Store Decomposition

**Status**: Accepted
**Date**: 2026-03-07
**Context**: `websocketStore.ts` is a god-object mixing protocol, transport, auth,
graph, Solid, bots, and binary parsing. Module-level mutable state bypasses store lifecycle.

## Decision

Decompose `websocketStore.ts` into bounded services:

1. **`connectionLifecycleService.ts`** - connect, reconnect, heartbeat, auth handshake
2. **`graphWebSocketStore.ts`** - graph-specific message handling, position batching
3. **`binaryProtocolService.ts`** - binary frame parsing, validation, coalescing
4. **`messageQueueService.ts`** - queue management, retry, prioritization
5. **`solidWebSocketStore.ts`** - Solid-specific WS handling (if needed)

### Rules
- All mutable state moves into Zustand stores or encapsulated service instances
- No module-level `let` state outside store definitions
- Each service has explicit `initialize()`, `dispose()`, `reset()` lifecycle
- Binary validation checks version byte + minimum header before scheduling

## Consequences
- Breaking change for any direct `websocketStore` imports
- Re-export facade for backward compatibility during migration
- Tests become possible per service boundary
