---
title: Data Authority & Erasure
doc_id: VC-DATA
version: 0.1.1
status: draft-for-ratification
verified_commit: 73540faa0
changelog:
  - 0.1.1 — fix GRAPH_PROVENANCE citation 53→55; widen named-graph range 50-55→46-55; add RBAC_ALLOW_OWNERLESS file:line citations
sources:
  - crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs
  - src/services/role_store.rs
  - src/middleware/rbac_gate.rs
  - crates/visionclaw-domain/src/utils/visibility_filter.rs
  - src/handlers/socket_flow_handler/position_updates.rs
  - scripts/backup-sqlite.sh
  - src/bin/sync_github.rs
  - src/services/file_service.rs
date: 2026-08-31
---

# Data Authority & Erasure

## Purpose

Defines the single authoritative owner for each data class in the estate, the
store that actually wins today, and how dual-writes linearise. Erasure and
backup are covered honestly, including where they do not yet exist.

## Current State

### Ownership matrix

Each data class has exactly one authoritative store. "Wins today" is what the
running code reads back as truth, regardless of what legacy ADR prose claims.

| Data class | Authoritative owner (today) | Store / mechanism | Linearisation point |
|---|---|---|---|
| Authored content | GitHub `public:: true` markdown | `src/bin/sync_github.rs`, `src/services/file_service.rs` | Sync run commit SHA; last-writer = the GitHub push |
| Visibility intent | Node `visibility` column + owner pubkey | SQLite (settings/node metadata), projected by `visibility_filter.rs` | Write to the node metadata row |
| Graph projection | Oxigraph named graphs | `oxigraph_ontology_repository.rs` (`GRAPH_KNOWLEDGE`) | Oxigraph transaction commit |
| Ontology (asserted + inferred) | Oxigraph | `GRAPH_ONTOLOGY` / `GRAPH_ONTOLOGY_INFERRED` | Oxigraph transaction commit; inferred is derived, never primary |
| Operational state | SQLite (WAL) | `data/{kpi,enrichment,settings,liveness}.sqlite3` | SQLite commit |
| Vector / agent memory | RuVector (external Postgres) | `mcp__claude-flow__memory_*` → `ruvector-postgres:5432` | Embedding-pipeline insert |
| Event journal / provenance | Oxigraph append-only | `GRAPH_PROVENANCE` (PROV-O reification), `oxigraph_ontology_repository.rs:55,641,2568` | Append; never mutated in place |
| Audit evidence (RBAC/auth) | SQLite role store | `src/services/role_store.rs` | Role-store write |
| Credentials | `.env` plaintext | filesystem | N/A — see divergences (SOPS never executed) |

### Store of record

The triple store is embedded Oxigraph (RocksDB backend, SPARQL 1.1); Neo4j is
100% removed (legacy ADR-132, no `neo4rs` in tree). Named-graph layout is fixed
in code: `GRAPH_ONTOLOGY`, `GRAPH_ONTOLOGY_INFERRED`, `GRAPH_KNOWLEDGE`,
`GRAPH_AGENT`, `GRAPH_SHAPES`, `GRAPH_PROVENANCE`
(`oxigraph_ontology_repository.rs:46-55`). Non-triple state lives in four
SQLite databases in WAL mode inside the `visionclaw-data` Docker volume at
`/app/data` (`scripts/backup-sqlite.sh:10-16`).

### Conflict resolution per class

- **Authored content vs graph projection.** GitHub is upstream; the Oxigraph
  `GRAPH_KNOWLEDGE` projection is derived from a sync run and is regenerated,
  never hand-edited. On conflict the sync overwrites the projection.
- **Ontology asserted vs inferred.** `:assert` is authored; `:inferred` is
  whelk-derived and rebuilt on each reasoner run — never a source of truth.
  The `:derived` graphs (`:summary`, `:observed`) are the ONLY writeback path
  through `/api/ontology/derived`; `:assert`/`:inferred` are never writable
  there (`oxigraph_ontology_repository.rs:56-65`).
- **Visibility intent.** The SQLite metadata row wins. The wire encoder applies
  it fail-closed: anonymous or unknown-owner callers are dropped to public-only
  (`visibility_filter.rs:11-14,74`).
- **Audit/RBAC.** Role store resolves the effective role with precedence
  explicit-assignment → power-user-Admin → default `Editor`, failing closed to
  `Viewer` on any error (`role_store.rs:188-199`).

### Linearisation of dual-writes

There is no cross-store two-phase commit. Each class linearises at its own
store's commit point (table above). Where a write fans out to two stores
(content sync → Oxigraph projection; agent action → RuVector + provenance), the
authoritative store commits first and the secondary is best-effort. There is no
distributed transaction and no compensating-action framework in code.

## Known divergences & open items

- **`deleteAgentMemory` tombstone gap.** Deleting agent memory in the Pod path
  emits NO reverse tombstone to RuVector. A delete does not revoke the vector
  memory: the embedding row persists and remains semantically searchable. RuVector
  is an external Postgres store reached only via MCP, with no delete-propagation
  hook. This is the single largest erasure hole.
- **No estate-wide erasure design.** There is no subject-erasure orchestration
  that spans GitHub content, Oxigraph graphs, SQLite state, RuVector vectors and
  the append-only provenance graph. `GRAPH_PROVENANCE` is append-only by design
  (`oxigraph_ontology_repository.rs:55`), so a compliant erasure story must define
  crypto-shredding or redaction for it — neither exists today.
- **Sources-of-truth conflict (legacy).** Legacy ADRs assign primacy to
  Oxigraph (132), Pod write-master (050/052), GitHub `public::true` (051),
  RuVector for agent memory (030) and provenance trails (033/034/124/128). Code
  resolves this as the matrix above; the legacy prose is not reconciled and must
  not be treated as authority.
- **Backup coverage is partial.** `scripts/backup-sqlite.sh` (2026-08-31) takes
  lock-consistent online snapshots of the four SQLite DBs via the SQLite backup
  API and copies them off-volume. **Oxigraph has NO point-in-time backup.** There
  is no cross-store consistent restore and no declared RPO/RTO.
- **Credentials.** SOPS (legacy ADR-109, VisionClaw) was accepted 2026-05-09 but
  NEVER EXECUTED — `.env` is plaintext, no SOPS artifacts in tree. Open.
- **Open-by-default posture.** Compose ships `RBAC_PUBLIC_READS=1`
  (`rbac_gate.rs:119-122`, default on) and `RBAC_ALLOW_OWNERLESS=1`
  (`docker-compose.unified.yml:78-79`, both `${VAR:-1}`; env const
  `role_store.rs:33`, enforced `main.rs:717-737`); an unassigned authenticated
  pubkey resolves to `Editor` (`role_store.rs:188-199`).
  This is a deliberate compatibility trade-off that needs a named security
  profile before ratification.
- **Landing 2026-08-31.** `PUBKEY_VISIBILITY_FILTER` now defaults ON
  (`position_updates.rs:35-42`, `unwrap_or(true)`) — the privacy encoder was
  previously inert. NIP-98 single-use replay cache added (`src/utils/nip98.rs`).
  These are reflected as current state above.
- **Identifier grammars unreconciled.** `vc:{domain}/{slug}` (legacy ADR-100),
  `urn:visionclaw:*` (105), `visionclaw:owner:{npub}/kg/...` (050), agentbox
  hex-canonical/npub-display (053) and minted-URNs-may-return-null (063) coexist.
  Code mints `urn:ngm:*` IRIs (`oxigraph_ontology_repository.rs:20-24`). A single
  identifier grammar is not yet agreed — cross-class joins rely on convention.

## Invariants

These must not silently change:

1. **One authoritative owner per class.** No class may acquire a second
   write-master without an explicit conflict-resolution rule in this table.
2. **`:assert` / `:inferred` / `:provenance` are never writable through the
   derived-writeback path.** Only `:summary` / `:observed` accept writeback.
3. **`GRAPH_PROVENANCE` is append-only.** Erasure of provenance must be by an
   explicit, documented redaction mechanism — never in-place mutation.
4. **Visibility filtering stays fail-closed.** Unknown-owner or anonymous
   callers get public-only; the default must remain ON.
5. **Role resolution fails closed to `Viewer`.** Errors never escalate.
6. **Secondary-store fan-out is best-effort but must be observable.** A dropped
   secondary write (e.g. RuVector tombstone) must be logged, not swallowed —
   the current silent gap is a defect, not the invariant.

## Change process

This is a living document. Amend it in the same commit that changes any
authoritative-owner assignment, conflict rule, or backup/erasure behaviour.
Every load-bearing claim carries a `file:line` citation; update the citation
when the code moves. Bump `version` (semver: patch for wording, minor for a
new class or rule, major for an owner reassignment) and refresh
`verified_commit`. Ratification requires closing the open items above or
explicitly accepting each as a signed-off trade-off with a named owner.
