# ADR-131: Documentation-Drift Reconciliation Sweep (2026-07-22)

## Status

Accepted

## Date

2026-07-22

## Supersedes-in-part

Banner-level corrections to ADR-061, PRD-007, PRD-014, PRD-023, ADR-121. These
records are **not** rewritten — each carries a dated correction banner and its
body is retained for history, per the append-only culture.

## Context

A six-axis adversarial doc-drift audit (neo4j-migration, phantom-actors,
binary-protocol, xr-client, missing-docs, deferred-vaporware — 13 agents, 47
claims verified: 22 ACCURATE, 22 DOCS_STALE, 4 CODE_GAP, 4 BOTH_DRIFTED; full
plan in [`docs/audit-doc-drift-2026-07-22.md`](../audit-doc-drift-2026-07-22.md))
found the **code correct and consistent on the load-bearing facts** — Oxigraph +
SQLite are the sole store; no BrokerActor / ServerNostrActor / Neo4j on `main`;
the 52-byte V3 position wire is live — but the **doc corpus** carrying:

- **(a)** a fabricated completion checkbox for a database that never shipped
  (PRD-014:248 `[x] Neo4j daily backup`);
- **(b)** an un-narrated Neo4j-removal decision undercited to an auth ADR
  (the `ADR-11` Phase-11 shorthand colliding with the `ADR-011` auth file);
- **(c)** seven shipped-but-undocumented ontology ADRs (ADR-113, ADR-115..120);
- **(d)** a live desktop-as-VR bug the docs claimed was already retired
  (WebXR `enterVR()` never set `isXRMode`);
- **(e)** narrowly-scoped banners leaving dead prescriptive Cypher / dead-file
  instructions live below them.

Crucially, **the code fixes and the doc sweep landed together on 2026-07-22** —
this ADR records reconciliation that has already executed, not a scheduled plan.

## Decision

1. **Neo4j-as-live claims** are corrected by dated banners (never rewritten); the
   originating removal decision is recorded as **ADR-132**
   (`ADR-132-neo4j-removal-oxigraph-adoption.md`), and the mislinks at
   `docs/README.md:29`, `docs/reference/graph-schema.md:14,485` and
   `docs/reference/configuration.md:250` are re-pointed to it. Root cause of the
   mislink: `ADR-11` (= Phase 11) shorthand collided with the `ADR-011` auth file.

2. **ADR-113 / ADR-115..120 backfilled** from shipping code; ADR-112 §5 register
   rows flipped `proposed → written`.

3. **ADR-061 & PRD-007 marked Superseded-by-ADR-102** — the 52-byte
   analytics-inline V3 wire is canonical; the 28-byte-pure design (28B/node,
   `9 + 28*N`) is dead design intent, retained for history.

4. **WebXR guard-or-delete fork RESOLVED by deletion (LANDED 2026-07-22).** The
   `client/src/immersive/` tree (`VRGraphCanvas`, `ImmersiveApp`, et al.) was
   **deleted** and its `App.tsx` wiring removed — ADR-071 Phase 3 executed rather
   than writing the never-written ADR-130 guard. `tsc` is clean. The residual XR
   surface — `quest3AutoDetector.ts` (the live `setXRMode` caller via
   `navigator.xr` immersive-ar), the vircadia services, and the XR settings-schema
   entries — is **deferred to the final-mile sprint**; only the immersive React
   tree is gone now. The desktop-as-VR bug can no longer ship because the button
   that triggered it no longer exists.

5. **Dead-code deletions LANDED 2026-07-22**, not merely scheduled:
   - the two byte-identical 28-byte **V3F0 encoders** (`src/protocol/v3_frame.rs`
     + the `crates/visionclaw-protocol` copy + their re-exports) are **deleted**;
   - the **V4 delta scaffolding** (`DeltaNodeData`, `MessageType::PositionDelta`,
     `PROTOCOL_V4`) is **removed** from `src/utils/binary_protocol.rs`;
   - the client `frameTypes.ts` default is now **`PROTOCOL_V3`** with accurate
     comments. Live wire: `0x03` / 52-byte V3 positions + `0x23` bare-tag agent
     actions.

6. **Final-mile code debt — CLOSED 2026-07-22:**
   - **ADR-119 telemetry no-op FIXED** — new
     `agentbox/mcp/servers/lib/ontology-telemetry.js` (JSONL sink + counters +
     boot canary, fail-open); `fail_open_count` is now observable via the
     `ontology_health` field `_agentbox_ontology_ask_telemetry` and
     `getTelemetrySnapshot()`; 20/20 node tests pass.
   - **ADR-117 server-side SPARQL clamp SHIPPED** — `clamp_sparql_limit` +
     `cap_result_rows` in `src/handlers/ontology_handler.rs` (default LIMIT
     injection 10000, hard row cap 10000, 8 MiB byte cap, explicit `truncated`
     flag); `tests/ontology_sparql_clamp.rs` 7/7 pass. The `/ontology/query`
     response shape is now `{results, rowCount, truncated}` (no consumer used the
     old bare array).
   - **ADR-113 condensation scheduler SHIPPED** —
     `scripts/ontology-condense-scheduler.mjs` (staleness-driven, jittered,
     locked, fail-open) + flock/mkdir lock in `ontology-condense-refresh.sh` +
     `[skills.ontology.condense]` manifest knobs + supervisord/flake staging
     (activation needs the next image rebuild).

7. **PRD-015 three-way identifier collision** dispositioned per §1f: the archived
   `archive/visionclaw-process/PRD-015-ecosystem-code-hygiene.md` (ecosystem
   code hygiene — cited by ADR-087/088/089/091/104), the **agentbox PRD-015**
   (consumer broadcast economy, Lightning-first — cited by ADR-124), and PRD-014's
   forward-reference to a **never-written productionisation PRD-015** are now
   distinguished by qualified banners rather than renumbering in place.

8. **AUTH-001 RESOLVED banner-only.** The four-tier enterprise RBAC middleware
   (`src/middleware/enterprise_auth.rs`, Admin > Broker > Auditor > Contributor,
   `Nip98RoleResolver`, `X-Enterprise-Role`) lives on the `jss-cut-scaffold`
   branch and is **not on `main`**. `main` carries only the NIP-98 primitives
   (`src/utils/nip98.rs`) and a coarser `AccessLevel` enum in
   `src/middleware/auth.rs`. `KNOWN_ISSUES.md` AUTH-001 is banner-corrected to
   state this; the merge decision (adopt to `main` vs. leave on branch) is
   **deferred to the final-mile sprint pending operator decision** — enterprise
   auth stays on its branch until then.

## Consequences

Doc authority chain restored; grep-truth aligned to prose (the dead V3F0 / V4 /
Neo4j-adapter greps that previously found phantom substrate now return nothing on
`main`). Falsification evidence for each shipped closure is filed under
`docs/gap-close-evidence/`. The only honest residual is the deferred final-mile
XR surface (quest3AutoDetector / vircadia / XR settings schema) and the AUTH-001
merge decision — both explicitly recorded above and not hidden by any completion
claim.

Companion record: **ADR-132** (`ADR-132-neo4j-removal-oxigraph-adoption.md`) — the
originating Neo4j-removal decision that this sweep re-points the mislinks to.
