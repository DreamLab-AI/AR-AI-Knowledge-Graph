# ADR-013: Zero-Allocation Render Loop

**Status**: Accepted
**Date**: 2026-03-07
**Context**: Per-frame allocations in GraphManager and BotsVisualization cause GC pressure

## Decision

All code in `useFrame` callbacks and render paths MUST be zero-allocation:

### Rules
1. No `new Vector3()`, `new Color()`, etc. in render paths - preallocate in refs
2. No `.slice()`, `.map()`, `.filter()` on arrays in frame loops
3. No `Date.now()` in render - use `state.clock.elapsedTime` from useFrame
4. `GlassEdges.updatePoints` accepts `(buffer, length)` not sliced arrays
5. Instanced meshes use preallocated capacity with overflow detection
6. `useMemo` for side effects replaced with `useEffect` + cleanup

### Patterns
```typescript
// WRONG: allocates every frame
const points = edges.slice(0, count);

// RIGHT: view over preallocated buffer
edgesRef.current.updateFromBuffer(buffer, count);
```

## Consequences
- GraphManager edge generation refactored to buffer-view pattern
- BotsVisualization particle positions preallocated in refs
- Measurable frame time improvement on 1000+ node graphs
