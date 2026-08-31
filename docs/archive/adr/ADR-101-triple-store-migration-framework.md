# ADR-101 — Triple-Store Migration Framework for Oxigraph

| Field | Value |
|-------|-------|
| Status | Accepted (2026-06-05) |
| Drives | PRD-018 §5 WS-0/WS-1, §6.4 |
| Companion ADRs | ADR-100 (canonical IRI — first migration consumer), ADR-098, ADR-099 |
| Affected paths | `crates/visionclaw-adapters/src/{oxigraph_ontology_repository.rs,sparql_migrations.rs}`, `crates/visionclaw-adapters/migrations/sparql/` (new — crate-local so it is inside the dev-container bind-mount scope; repo-root `migrations/` is not mounted), `migrations/sqlite/0001_initial.sql` (discipline parity reference) |
| Evidence | `migrations/sqlite/0001_initial.sql:11-14` (sole CREATE TABLE, CI-enforced), `oxigraph_ontology_repository.rs:44-53` (named graphs), PRD-018 §2.1 |

## Context

The SQLite settings store has a disciplined migration regime: a single `schema_migrations` table, versioned files under `migrations/sqlite/`, and CI enforcement that `CREATE TABLE` appears only in migration `0001` (`0001_initial.sql:11-14`).

The Oxigraph triple store (RocksDB-backed) has **no equivalent**. There is no versioning of the graph schema, no idempotent way to apply a structural change (e.g. re-minting IRIs under ADR-100, or back-filling provenance under ADR-099), and SPARQL is string-concatenated with hand-rolled escaping (an injection surface). ADR-100 needs to rewrite existing subject/object IRIs in one transaction without leaving dangling references — that is precisely a migration, and there is no framework to run it safely or to record that it ran.

The reuse directive: mirror the SQLite discipline that already exists rather than invent a new mechanism, and use Oxigraph's native SPARQL UPDATE rather than a bespoke mutation layer.

## Decision

### D1 — Versioned, idempotent SPARQL migrations under `crates/visionclaw-adapters/migrations/sparql/`

Structural changes to the triple store are expressed as numbered SPARQL UPDATE files (`0001_*.rups`, `0002_*.rups`, …) under `crates/visionclaw-adapters/migrations/sparql/` (crate-local, so inside the dev-container bind-mount scope; the repo-root `migrations/` holds only `sqlite/`), mirroring `migrations/sqlite/`. Each migration is a single logical transaction (Oxigraph applies UPDATE atomically) and MUST be idempotent (guarded by an existence check or `DELETE/INSERT WHERE` that is safe to re-run).

### D2 — Migration ledger in a dedicated named graph

Applied migrations are recorded in `urn:ngm:graph:migrations` (a new named graph alongside the existing `urn:ngm:graph:*` family at `oxigraph_ontology_repository.rs:44-53` — reuse the discipline, add one graph). Each record carries version, checksum, and applied-at timestamp. On startup the repository compares files-on-disk to the ledger and applies pending migrations in order, exactly once. This is the triple-store analogue of `schema_migrations`.

### D3 — Parameterised SPARQL, not string concatenation

Migrations and the repository's mutating queries use parameterised/escaped construction (or Oxigraph's typed term builders) rather than the current hand-rolled string escaping. This closes the injection surface flagged in PRD-018 §2.1. New code MUST NOT concatenate untrusted strings into SPARQL.

### D4 — First migrations are ADR-100's IRI re-mint and ADR-099's provenance back-fill

The framework ships with its first real consumers (as shipped under
`crates/visionclaw-adapters/migrations/sparql/`):
- `0001_canonical_iri_remint.rups` — rewrite existing entity IRIs to `vc:{domain}/{slug}` (ADR-100 D1), rewriting all subject/object positions in one transaction so no references dangle. Idempotent: re-running on already-canonical IRIs is a no-op.
- `0002_bootstrap_shapes_graph.rups` — bootstrap the SHACL shapes named graph (ADR-127 trust layer).
- `0003_bootstrap_provenance_graph.rups` — bootstrap the provenance named graph and tag inferred quads with provenance markers (ADR-099 D3).

### D5 — CI parity

Extend the existing CI discipline: a check that the `migrations/sparql/` ledger logic is the only path that performs structural rewrites, analogous to the SQLite "CREATE TABLE only in 0001" rule. Migrations are append-only; an applied migration file is never edited (a fix is a new migration).

## Consequences

**Positive:**
- ADR-100's re-mint becomes a safe, idempotent, recorded operation instead of an ad-hoc one-off — no dangling references, re-runnable.
- The triple store gains the same auditability the settings store already has.
- Parameterised SPARQL removes the injection surface.
- Pure reuse of an established in-repo pattern; no new dependency.

**Negative / risks:**
- Idempotency is the migration author's responsibility; mitigated by a required dry-run/round-trip test per migration and the checksum ledger detecting drift.
- Large re-mint migrations over a big RocksDB store take time at startup; run once, recorded thereafter, and gated behind the ledger so they never re-run.
- A new named graph (`urn:ngm:graph:migrations`) must be excluded from graph-export/round-trip and from the GPU/inference paths; documented in the named-graph table.

## Verification

- Unit: applying a migration twice is a no-op (idempotency); ledger records version+checksum; pending migrations apply in order exactly once.
- Integration: `0001` re-mint leaves zero dangling subject/object references (SPARQL count of orphaned references = 0); `0002` back-fills provenance on all pre-existing inferred quads.
- CI: structural-rewrite-only-via-migrations check passes; SPARQL injection lint passes.
