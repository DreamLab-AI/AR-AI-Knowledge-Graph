# Ghost / Dead-Wiring Register — 2026-06-14

**Purpose:** root out dead wiring per operator directive ("remove the ghosts wherever you find them"; "dead wiring should be rooted out"). This is the evidence-backed **find-and-disposition** deliverable. Nothing destructive is executed by this document — removals are either staged into PRD-020 workstreams (coupled to replacements) or await go-ahead (irreversible/standalone), consistent with the operator's "once we're fully confident we should implement and test everything" gate.

**Classification:**
- **A — safe-delete now** (zero consumers, pure orphan, low/no risk)
- **B — superseded** (a PRD-020 workstream replaces it; delete *with* the replacement, not before — premature deletion breaks live code)
- **C — intended-but-unbuilt** (a real gap; **do NOT delete** — wire it or take an explicit kill decision; several are filled by PRD-020)
- **D — orphaned data** (large/irreversible; confirm before deleting)
- **NOT-A-GHOST** (looked dead, verified live — recorded so nobody deletes it)

---

## B — Superseded by PRD-020 (delete during the named workstream)

| Ghost | Evidence | Disposition |
|---|---|---|
| **`writeback_triggered` flag + `DECISION_LOG` (in-memory `Mutex<Vec>` posing as durable) + `WRITEBACK_DECISIONS` const** — the canonical ghost: claims to write back to the KG, performs no write; doc-comment admits "process-global decision_log" | `src/handlers/enrichment_proposals_handler.rs:37,102,116,167,234` | **ADR-121 / PRD-020 WS-9** deletes all three; replace with the durable `EnrichmentProposal` store + real write. **Do NOT delete before WS-9** — the `/decide` endpoint depends on it until the replacement lands. Grep-gate proves removal at WS-9 close. |

## C — Intended-but-unbuilt (gaps — wire, don't delete)

| Gap | Evidence | Disposition |
|---|---|---|
| **`GET /api/broker/inbox` missing** — broker/voice surfaces have no pending-case list | (absence; `broker-bridge.js:219` GETs it; no Rust route) | **Built by ADR-123 / WS-12.** Not a deletion — a gap the voice-governance work fills. |
| **`workspace_actor.rs` workspace-change broadcast = no-op** | `src/actors/workspace_actor.rs:41` "no-op implementation" | Decision needed: wire the WS broadcast or remove the handler. **Out of PRD-020 scope** — flag for owner. Not auto-deleted (a consumer may await the message). |
| **Cluster hull store "not wired; hulls will not render"** | `src/actors/gpu/clustering_actor.rs:1291` | Decision needed: wire the store or remove the hull path. Flag for owner; not auto-deleted. |
| **`system.network.enableMetrics` promises a metrics endpoint that "is not yet implemented"** | `src/app_state.rs:742` | Either implement the endpoint or remove the setting + warning. Flag for owner. |
| **Duplicate `web::scope("/ontology")`** (indeterminate `/load` gating) | `src/handlers/ontology_handler.rs:913` **and** `src/handlers/api_handler/ontology/mod.rs:1344` | **PRD-020 WS-0** resolves via route-dump + collapse to one scope with the strictest gate. Governance-relevant; not standalone deletable. |

## D — Orphaned data (confirm before deleting; reclaims disk)

| Item | Evidence | Disposition |
|---|---|---|
| **`qdrant_data/` — 7.9 GB, ZERO Rust references** (Qdrant never wired into the backend; last write Nov 2025) | `du -sh qdrant_data` = 7.9G; `grep -rin qdrant src crates Cargo.toml` = 0 | ✅ **DELETED 2026-06-14** (operator go-ahead). Pre-delete safety verified: 0 refs in compose/env/scripts, not a mountpoint, not git-tracked. 7.9 GB reclaimed. |
| **`.agentic-qe/sessions/*.jsonl`** — stale session logs (May 28); one is untracked in git status | `ls .agentic-qe/sessions/` | **Recommend delete** (transient). Low risk. |
| **`ruvector.db` (548 K), `agentdb.rvf` (4 K)** — local sqlite/rvf artifacts; canonical memory is RuVector **PostgreSQL** (`ruvector-postgres:5432`), so these are likely stale local debris | repo root | **Verify-then-delete** — confirm nothing opens them locally (a debug path may). Flag, don't auto-delete. |

## A — Safe-delete now (committed transient artifacts polluting source)

| Item | Evidence | Disposition |
|---|---|---|
| **Committed agent log dumps inside the source tree** — large headless agent-output `.log` files under `.claude-flow/logs/headless/` (e.g. a 39 KB testgaps dump) committed under `src/` and `client/src/` | `src/.claude-flow/logs/headless/*.log`, `client/src/.claude-flow/logs/headless/*.log` | **Recommend delete + `.gitignore`** the `**/.claude-flow/logs/` path. Transient agent output should never live in `src/`. Low risk (logs, no consumers). Awaiting go-ahead to keep this register non-destructive. |

## NOT-A-GHOST (verified live — do NOT delete)

| Looked dead | Why it's live | Evidence |
|---|---|---|
| `semantic_forces.cu` / `ontology_constraints.cu` / `ENABLE_CONSTRAINTS` / `UploadConstraintsToGPU` | **Wired by PRD-018** (~18,933 GPU constraints uploaded); real `impl Handler<UploadConstraintsToGPU>` | `src/actors/gpu/gpu_manager_actor.rs:493`, `ontology_constraint_actor.rs:191`, `semantic_forces_actor.rs` |
| `cuda_ffi_stubs.c` no-op stubs | Legitimate **fallback** when nvcc can't compile object files — satisfies the linker by design | `src/utils/cuda_ffi_stubs.c:1-3` |
| `main.rs` dev-build "no-op stub"s; `client_filter.rs` / `elevation_actor.rs` "stub" mentions | Dev-build guards and the real domain concept of `linked_page`/`owl_class` **frontier stubs** — not dead code | `src/main.rs:125,174`; `src/actors/client_filter.rs:44`; `src/actors/elevation_actor.rs:263` |
| `semantic_processor_actor.rs` 256-dim hash "embedding" | Not a ghost but a **fake** — recorded in ADR-114 as the reason RuVector (not this) is the substrate; superseded conceptually, not dead code to delete | `src/actors/semantic_processor_actor.rs:402` |

---

## Recommended immediate actions (await go-ahead)

1. **Delete `qdrant_data/`** (7.9 GB) after confirming it is not a mounted sidecar volume.
2. **Delete `.agentic-qe/sessions/*.jsonl`** and **purge `**/.claude-flow/logs/` from source + `.gitignore` it.**
3. **Verify-then-delete** `ruvector.db` / `agentdb.rvf` local debris.

## Staged into PRD-020 (delete/wire with the replacement, not before)

4. `writeback_triggered` / `DECISION_LOG` / `WRITEBACK_DECISIONS` → **WS-9** (durable store replaces it).
5. `GET /api/broker/inbox` → **WS-12** (built).
6. Duplicate `/ontology` scope → **WS-0** (collapse to one strict gate).

## Flagged for owner decision (out of PRD-020 scope)

7. `workspace_actor` no-op broadcast — wire or remove.
8. `clustering_actor` hull store "not wired" — wire or remove.
9. `app_state` metrics endpoint "not yet implemented" — implement or remove the setting.

**Principle applied:** dead wiring coupled to a replacement is removed *with* its replacement (never before, or it breaks live paths); standalone orphans are removed on go-ahead; intended-but-unbuilt features are *gaps to fill*, not ghosts to delete — and the verifiable-liveness telemetry (ADR-119) is the standing mechanism that prevents new dead wiring from going unnoticed.
