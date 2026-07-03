# VisionClaw End-to-End Test — Pristine Container Run

**Purpose**: validate the full pipeline after a clean rebuild with empty databases, against commit `1862c2d2f` (post diagnostic dial-back).

> **STALE — REMOVED PERSISTENCE LAYER (flagged 2026-07-03).** This checklist
> predates the persistence-layer cutover. **Neo4j has been fully removed** — the
> graph store is now Oxigraph + SQLite (see the "Graph Store" row in the root
> `README.md`) — and the **JSS sidecar has been removed** (Solid Pod is now the
> embedded `solid-pod-rs` library, ADR-032 / ADR-053). Every `visionclaw-neo4j*`
> volume, the `7474`/`7687` (Bolt) ports, the Cypher `MATCH (n) RETURN count(n)`
> query and the `visionclaw-jss` container referenced below are **obsolete**. The
> Layer 1–2 steps must be rewritten against the Oxigraph/SQLite store before this
> checklist is run again. Struck-through items below are retained only for history.

## Pre-conditions

- [x] Containers stopped via `./scripts/launch.sh down dev` or `compose down`
- [x] Volumes wiped: ~~`visionclaw-neo4j-data`, `visionclaw-neo4j-logs`~~ (REMOVED — Neo4j gone), `visionclaw-data`, `visionclaw-logs`
- [x] Build caches preserved: `visionclaw-cargo-*`, `visionclaw-npm-cache`
- [x] Diagnostic log gates removed (commit `1862c2d2f`)
- [x] RUST_LOG default demoted to `warn,webxr=info,...`
- [x] Rebuild triggered: `./scripts/launch.sh up dev`

## Layer 1 — Infrastructure

- [ ] `docker ps` shows `visionclaw_container` (healthy) ~~, `visionclaw-neo4j` (healthy), `visionclaw-jss` (healthy or at least running)~~ (REMOVED — Neo4j and the JSS sidecar are gone; Solid Pod is embedded)
- [ ] ~~`visionclaw-neo4j` reachable at port 7474 (HTTP) and 7687 (Bolt)~~ (REMOVED — Neo4j gone; the graph store is Oxigraph/RocksDB with no external port)
- [ ] `visionclaw_container` serving on port 3001 (nginx) and 4000 (direct backend)
- [ ] No unexpected `warn!`/`error!` entries in first 60s of `docker logs visionclaw_container`

## Layer 2 — Data Ingestion

- [ ] GitHub ontology sync kicks off on startup — look for `GithubSyncActor` logs
- [ ] Logseq pages processed into `KGNode` rows ~~in Neo4j~~ in the Oxigraph triple store
- [ ] OWL ontology assembler → converter → Whelk reasoner pipeline executes
- [ ] ~~Neo4j node count > 0 after ingestion (query: `MATCH (n) RETURN count(n)`)~~ (REMOVED — Neo4j gone; verify via an Oxigraph SPARQL count instead)
- [ ] `iri_to_id` map populated (logs: `ONT-001: Built iri_to_id map — N KGNode nodes have owl_class_iri`)
- [ ] Ontology edges loaded (logs: `Loaded M ontology edges (SUBCLASS_OF + RELATES)`)

## Layer 3 — Real-time Pipeline

- [ ] WebSocket upgrade succeeds at `/wss` (101 Switching Protocols)
- [ ] Binary frames arrive at client — opcode `0x42`, 24-byte/node payload (position + velocity)
- [ ] `broadcast_sequence` increments monotonically
- [ ] No `BroadcastPositions#` diagnostic logs in server (confirms dial-back applied)
- [ ] Physics simulation running at ~60 Hz — `ForceComputeActor` emitting frames
- [ ] Client receives position updates — graph nodes animate in browser

## Layer 4 — Interactive

- [ ] Frontend loads at `http://localhost:3001` (no 5xx, no JS console errors)
- [ ] Graph renders with >0 nodes
- [ ] Sliders move the live graph (physics parameter changes apply without hard-refresh)
  - Attraction slider (0–10)
  - Dual Graph Separation (0–500)
  - Flatten to Planes (0–0.1)
- [ ] Enterprise drawer opens on Ctrl+Shift+E
- [ ] Settings PUT via enterprise drawer persists

## Layer 5 — Observability

- [ ] Log volume under `warn,webxr=info` is reasonable (not flooding)
- [ ] No boundary-stuck node rescues firing repeatedly (indicates stable physics)
- [ ] FastSettle either converges or falls back to Continuous cleanly
- [ ] `/api/health` returns healthy with physics simulation running

## Known-out-of-scope

- RuVector PostgreSQL NOT wiped (shared with other workspace projects — separate concern)
- Solid Pod data NOT wiped (`visionclaw-jss-data` volume preserved)
- Build caches preserved (`visionclaw-cargo-*`, `visionclaw-npm-cache`)

## Rollback

If the E2E fails, the previous stable commit is `fcfc1a166` (the physics unblock commit before the documentation session). The logging change can be reverted with `git revert 1862c2d2f`.
