# ADR-089 — CQRS Dead Bus Removal

> Correction (2026-07-22 doc-drift audit): every bare "PRD-015" in this record
> (§3 isolated code registry, etc.) is the VisionClaw **ecosystem code-hygiene**
> PRD, now archived at
> [`archive/visionclaw-process/PRD-015-ecosystem-code-hygiene.md`](../../archive/visionclaw-process/PRD-015-ecosystem-code-hygiene.md)
> — not the agentbox PRD-015 (consumer broadcast economy, cited by ADR-124) nor
> PRD-014's forward-reference to a never-written productionisation PRD. Three-way
> collision resolved per [ADR-131](ADR-131-doc-drift-reconciliation-2026-07.md) §1f.

| Field | Value |
|-------|-------|
| Status | Accepted (2026-06-11; proposed 2026-05-09) |
| Drives | PRD-015 §3 (isolated code registry), PRD-014 OBS-03 (dead code removal) |
| Companion ADRs | ADR-077 P8 (code quality gates) |
| Companion PRDs | PRD-014, PRD-015 |
| Affected repos | `VisionClaw` |

## 2026-06-11 reclassification

Moved Proposed → **Accepted**. The 2026-06-11 ADR-gap audit re-confirmed this ADR's
premise directly against the tree: no command/query *bus* was ever wired into a
production dispatch path. The sole command path is **direct hexser dispatch** —
`DirectiveHandler` / `QueryHandler` implementations invoked directly (and actor
mailboxes for live state), never routed through an in-process CQRS bus. The
`src/cqrs/` scaffold remained a dead, no-op experiment with zero consumers and zero
coverage. Ratifying the deletion removes the contradiction between the dead scaffold
and the actual direct-dispatch architecture. The companion correction is the README,
whose "114 CQRS handlers" headline implied a bus that never existed; it now describes
direct hexser `DirectiveHandler`/`QueryHandler` dispatch.

## Context

The `src/cqrs/` directory contains a full CQRS (Command Query Responsibility Segregation) event bus implementation spanning **3,200 lines** across 12 files:

```
src/cqrs/
├── mod.rs           # Bus registration, dispatch
├── commands.rs      # Command types (42 variants)
├── queries.rs       # Query types (28 variants)
├── events.rs        # Event types (31 variants)
├── handlers/
│   ├── command_handlers.rs
│   ├── query_handlers.rs
│   └── event_handlers.rs
├── bus.rs           # InMemoryBus implementation
├── middleware.rs    # Bus middleware chain
└── projections.rs  # Read model projections
```

The bus is registered in `main.rs` during startup, but:
1. **All command handlers are no-ops** — they accept the command and return `Ok(())` without side effects.
2. **All query handlers return empty defaults** — e.g. empty vectors, zero counts.
3. **No production code path dispatches through the bus** — handlers call actor mailboxes directly.
4. **QE gap analysis (2026-04-10) confirmed** the bus is dead code with zero coverage.
5. **Zero tests exist** for bus dispatch paths.

The bus was scaffolded as an architectural experiment that was superseded by the Actix actor mailbox pattern. It was never wired to real service implementations.

## Decision

### D1 — Delete `src/cqrs/` entirely

Remove the directory and all 12 files. Update `src/lib.rs` to remove `pub mod cqrs;`.

### D2 — Remove bus registration from `main.rs`

Delete the `CqrsConfig::register()` call and associated imports.

### D3 — Verify no transitive dependencies

Before deletion, grep for `use crate::cqrs` and `cqrs::` across the codebase. Any remaining references are import artifacts — remove them.

### D4 — No replacement

The Actix actor mailbox pattern (`src/actors/`) is the established command dispatch mechanism and has full production coverage. No CQRS replacement is needed.

If event sourcing is required in future (e.g. for audit trails), it should be designed as an append-only event log on Neo4j or a dedicated event store, not an in-process bus. That would be a new ADR.

## Consequences

**Positive:**
- 3,200 lines of dead code removed
- Eliminates confusion for new contributors who might assume the bus is in use
- Reduces compile time (~2-3 seconds for 12 files)
- Removes 42+28+31 = 101 type definitions that shadow actor message types

**Negative:**
- None identified. The bus has zero consumers.

**Risks:**
- Possibility that a branch-in-progress references `cqrs::`. Mitigated by grepping all local branches before merge.
