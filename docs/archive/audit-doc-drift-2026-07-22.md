# Doc-Drift Audit — 2026-07-22 (13-agent adversarial sweep)

> **Remediation completed — banners landed, ADR-132 Accepted**

> Provenance: Fable-orchestrated workflow — 6 Sonnet doc-researchers, 6 Opus code-verifiers (one per drift axis: neo4j-migration, phantom-actors, binary-protocol, xr-client, missing-docs, deferred-vaporware), 1 Opus remediation planner. 47 claims verified against source: 22 ACCURATE, 22 DOCS_STALE, 4 CODE_GAP, 4 BOTH_DRIFTED. Raw findings journal retained in session transcript. ADR-131/132 below are PROPOSED pending operator acceptance.


Honours the append-only culture: **corrections land as dated banners, never rewrites; new decisions land as new dated ADRs; closures land as gap-close evidence files with Falsification blocks.** Nothing below rewrites shipped history.

Legend: **P0** = actively misleading (fabricated completion, live-shipping bug, security-relevant) · **P1** = will misdirect an implementer or leaves shipped work undocumented · **P2** = hygiene / consistency.

---

## 1. DOCS-ONLY fixes

### 1a. Fabricated / falsified completion claims (P0)

| File / line | Action | Proposed one-line banner (append, do not delete original) |
|---|---|---|
| `docs/prd/PRD-014-ecosystem-productionisation.md:248` | Uncheck the false `[x]`; banner | `> Correction (2026-07-22 doc-drift audit): this item is VOID — no Neo4j ships (ADR-130:83, P1-REC-4.md:51); the '994b200 Neo4j daily backup' never guarded live data. SQLite stores (data/{kpi,enrichment,settings}.sqlite3) have no backup workstream yet — see ADR-131.` |
| `docs/prd/PRD-014-ecosystem-productionisation.md:331-333` | Banner the dead `neo4j_adapter.rs:55-59/:720-750` citations | `> Correction (2026-07-22): src/adapters/neo4j_adapter.rs does not exist on main (only in _archive/2026-07-10 worktrees). Live KPI store is src/adapters/sqlite_kpi_repository.rs.` |

### 1b. Unbannered target-architecture PRDs asserting a live Neo4j substrate (P1)

Each gets a top-of-file dated correction banner (append-only). These are all category (b): present-tense "Neo4j-backed" claims contradicting `docs/README.md:29`.

| File | Key lines | Proposed banner |
|---|---|---|
| `docs/prd/PRD-005-graph-cognition-platform.md` | 16, 28, 44, 65, 148, 157, 566, 606, 821-823, 1103 | `> Correction (2026-07-22): "Neo4j layer"/"Neo4j Cypher"/"migrations/2026-05_typed-schema.cypher" are obsolete. Live store is Oxigraph named graphs (urn:ngm:graph:knowledge) + SQLite; retarget merge/migration to SPARQL per ADR-101. This PRD remains Draft (unimplemented) — see ADR-131.` |
| `docs/adr/rvf-integration-prd.md` (+ `-afd.md`, `-ddd.md`) | prd:85; afd:24-25,93-94; ddd:15,35,55,207,364 | `> Correction (2026-07-22): "Neo4j remains the source of truth" is false. Ontology/graph source of truth is Oxigraph (oxigraph_ontology_repository.rs). RVF supplements Oxigraph, not Neo4j. Status still Draft, ~5mo stale — see KNOWN_ISSUES AGENT-001 and ADR-131.` |
| `docs/prd/PRD-003-contributor-ai-support-stratum.md` | 105, 357, 415-417, 646 | `> Correction (2026-07-22): "Rust + Actix + Neo4j stack" → Oxigraph stack. BC18/BC19 write projections target Oxigraph named graphs or a SQLite projection table via src/ports/knowledge_graph_repository.rs, not Neo4j.` |
| `docs/prd/PRD-008-xr-godot-replacement.md` | 37, 78 | `> Correction (2026-07-22): "authoritative graph state lives in Neo4j + RuVector + GraphStateActor" → Oxigraph + RuVector + GraphStateActor. No Neo4j component exists.` |
| `docs/prd/PRD-010-did-nostr-mesh-federation.md` | 481 | `> Correction (2026-07-22): consensus-threshold write persists to Oxigraph/SQLite (sqlite_enrichment_repository.rs), not Neo4j.` |
| `docs/prd/prd-insight-migration-loop.md` | 54 | `> Correction (2026-07-22): the constraint "Not graph-database-agnostic. Neo4j + OWL only" is obsolete — OWL runs on Oxigraph RDF; delete or restate as "Oxigraph RDF + OWL".` |
| `docs/prd/PRD-013-solid-git-ingest-surface.md` | 32, 50, 230, 338, 456, 499, 520, 672-674 | **Second banner** (existing one only covers BrokerActor transport): `> Correction 2 (2026-07-22): enrichment data and DecisionHistoryEntry audit trail live in SQLite (sqlite_enrichment_repository.rs, WS-9/WS-12), NOT "trapped in Neo4j". No Neo4j audit store exists.` |

### 1c. Narrowly-scoped banners to widen (P1)

The banner is correct but leaves post-banner prescriptive Cypher / dead-file instructions live:

- `docs/prd/PRD-006-visionclaw-agentbox-uri-federation.md:175-209,270-337` — banner: `> Correction (2026-07-22): the Cypher-SELECT extension instructions below target neo4j_graph_repository.rs, which does not exist. Live equivalent: src/adapters/oxigraph_graph_repository.rs (SPARQL, named-graph routing).`
- `docs/prd/prd-bead-provenance-upgrade.md:299-301,362,385` — banner: `> Correction (2026-07-22): "Neo4j Schema Reference" is no longer doc-of-record; graph-schema.md is Oxigraph-scoped. Re-point :Bead/:BeadLearning schema refs accordingly.`
- `docs/explanation/ddd-bounded-contexts.md:11-14` — extend existing ADR-11 banner scope to the whole document (the box-below wording leaves downstream Neo4j prose live).

### 1d. Stale ADR/PRD status flips & self-contradictions (P1/P2)

| File | Action |
|---|---|
| `docs/adr/ADR-061-binary-protocol-unification.md` **(P1)** | Mark **Superseded-by-ADR-102** in the Status header (append-only); banner D1 (51-67) and the telemetry formula (187-191): `> Superseded (2026-07-22): 28B/node and 9+28*N are the pre-centrality design. Live wire is 52B WireNodeDataItemV3 (ADR-102 §2). Body retained for history.` Do **not** rewrite the body. |
| `docs/prd/PRD-007-binary-protocol-unification.md` **(P1)** | Flip `Status: Draft → Superseded` with banner: `> Superseded (2026-07-22): the 28-byte-pure goal (G1) was abandoned in favour of the 52B analytics-inline wire (ADR-031/ADR-102). Dead design intent, not a pending task.` |
| `docs/prd/PRD-023-gap-close-visionclaw.md:82` **(P1)** | Banner: `> Correction (2026-07-22): src/domain/broker/* is NOT crashbug-only — it shipped to main via c9f2e3539. Only src/actors/broker_actor.rs + src/adapters/neo4j_broker_adapter.rs remain crashbug-only. (ElevationActor gate now defaults ON post-REC-2.)` |
| `docs/prd/PRD-012-dreamlab-ai-website-kit-adoption.md:5` **(P2)** | Banner: `> Correction (2026-07-22): cutover is DEFERRED per ADR-083 (frozen 2026-07-03, outside standalone-first scope), not merely "pending".` |
| `docs/adr/ADR-121-self-improving-ontology-writeback-loop.md:3` (+122/123 scope note) **(P1)** | Banner: `> Correction (2026-07): Tier W0 (derived materialisation) SHIPPED and is route-registered (src/main.rs:906,973; ontology_derived_handler.rs; oxigraph_ontology_repository.rs:753), git-dated 2026-07-10 — reconciling with ADR-114:55. "Unbuilt loop" now scopes only to ADR-122 (two-speed routing) and ADR-123 (voice sign-off).` |
| `docs/how-to/operations/troubleshooting.md:996-1005` **(P1, also §3)** | The SUPERSEDED banner is the *wrong one*. Replace-by-append: `> Correction (2026-07-22): the WebXR tree was NOT removed — client/src/immersive/ still exists and still mounts (App.tsx:187). Deprecation/deletion is PLANNED (ADR-071 Phase 3), not done. Line 33's HTTPS-for-WebXR row is therefore still live.` Leave line 33 untouched (coincidentally accurate). |
| `docs/how-to/operations/troubleshooting.md:33` **(P2)** | No change (accurate while path ships); note reconciliation in the banner above. |

### 1e. Retroactive ADRs for shipped-but-undocumented decisions (P1)

The code exists; only the governing ADR file is missing. Write these as new dated files pointing at the shipping code (documentation catch-up, not new engineering):

1. **Backfill the seven ontology siblings** into `docs/adr/`, flipping ADR-112 §5 register rows from `proposed → written`:
   - `ADR-113-ontology-condensation-mesh.md` → agentbox/mcp/servers/lib/{ontology-condense,ontology-index-build}.js
   - `ADR-115-turtle-serialisation.md` → ontology-retrieval.js:72-109
   - `ADR-116-tiered-token-budgets.md` → ontology-budget.js:15-21
   - `ADR-117-server-side-sparql-clamp.md` → ontology_handler.rs:720-768 (note: **clamp only half-shipped — see §2/§3**)
   - `ADR-118-load-endpoint-hardening.md` → api_handler/ontology/mod.rs:1361-1378
   - `ADR-119-verifiable-liveness-telemetry.md` → ontology-retrieval.js:172,202 (note: **sink is a no-op — see §2/§3**)
   - `ADR-120-propose-p0-auth.md` → ontology_agent_handler.rs:355-361 + tests/rec1_route_guard.rs
2. **`ADR-132-neo4j-removal-oxigraph-adoption.md`** (P0, closes claim-10 governance gap) — the originating decision that narrates Neo4j removal. Then re-point the mislinks: `docs/README.md:29` and `docs/reference/graph-schema.md:14,485` (currently link `ADR-011-auth-enforcement.md`) → ADR-132; align `docs/reference/configuration.md:250` (currently cites ADR-101). Root cause: the `ADR-11` (=Phase 11) shorthand colliding with the `ADR-011` auth file.

### 1f. Dangling-reference & index sweep (P1/P2)

- `docs/adr/README.md` **(P2)**: add a fold-in mapping table for `ADR-001..010 / 015..026` (mirror the existing ADR-074 collision table) so `ADR-026` (cited 6× by ADR-057, PRD-020:193) traces to its surviving ADR. Add an index entry acknowledging the ADR-113/115-120 backfill.
- `docs/prd/PRD-015` three-way collision **(P1)**: enforce the `agentbox PRD-015` qualifier at `ADR-124:7`; relink `ADR-087/088/089/091/104` bare `PRD-015` → `archive/visionclaw-process/PRD-015-ecosystem-code-hygiene.md`; and **renumber PRD-014:16,44,224's forward-ref** (`federation mesh / IS-Envelope / tracing / a11y`) to an unused number — it names a never-written productionisation PRD, a genuine dangling forward reference distinct from the two existing docs.
- `archive/README.md:16,28-29` **(P2)**: banner — `> Correction (2026-07-22): the "every internal link resolves" + "non-substantive process notes" claims are stale. PRD-015 (code-hygiene) and PRD-021 (OMB) carry substantive, still-cited findings (O1/PAR-01/02/03, Strategy B) referenced post-archival by ADR-088/124/126. Reclassified substantive-but-archived.`
- `docs/adr/ADR-126-omb-adoption-posture.md:5` **(P2)**: banner noting its evidence base (`PRD-021 Strategy B`) is a non-canonical **Draft pending operator sign-off** — or promote PRD-021 into `docs/prd/`. Leave ADR-126 Proposed (genuine open decision, no code owed).
- `docs/adr/rvf-integration-{prd,afd,ddd}.md` **(P2)**: add back-link to `KNOWN_ISSUES.md AGENT-001` (already covered by 1b banner).
- Anomaly-register N3 & "24B/28B FIXED" — see §5.

---

## 2. CODE-ONLY fixes (the final mile)

Ranked by risk. Effort in {trivial · hours · days · sprint}.

### P0 — live-shipping defects

| # | Task | Files | Effort |
|---|---|---|---|
| C1 | **WebXR desktop-as-VR bug ships in every browser build.** `VRGraphCanvas.tsx:43 xrStore.enterVR()` never calls `platformManager.setXRMode(true)` (setter at platformManager.ts:289 unused), so `GraphManager.tsx:61 isXRMode` stays false and an entered session renders desktop-flat. **Decide the fork (see §3):** wire enterVR→setXRMode(true), or gate/delete the button. | client/src/immersive/threejs/VRGraphCanvas.tsx:41-61; services/platformManager.ts:289; features/.../GraphManager.tsx:61 | hours |
| C2 | **ADR-119 liveness telemetry is unwired** (the exact "wired ≠ working" trap it was built to avoid). Sink defaults to no-op `{ record(){} }`; fail_open records vanish unless a dep is injected. No ontology-telemetry module, no startup canary. `fail_open_count`/liveness matrix unobservable. | agentbox/mcp/servers/lib/ontology-retrieval.js:128,172,202 | hours |
| C3 | **ADR-117 SPARQL clamp half-shipped.** `sparql_query` enforces read-only + forbids SERVICE but injects **no default LIMIT / row / byte cap** before calling `sparql_select_json` raw. An authed caller can issue an unbounded SELECT against Oxigraph. Agentbox compensates client-side only. Add the server clamp (WS-0 hard invariant). | src/handlers/ontology_handler.rs:720-768,827-845 | hours |

### P1 — misdirection & dead scaffolding

| # | Task | Files | Effort |
|---|---|---|---|
| C4 | **Delete the dead, duplicated 28-byte V3F0 encoder** (zero production callers; docstring actively lies "broadcast path uses this exclusively"). Delete both copies or wire one and retire the 52B path. | src/protocol/v3_frame.rs (297L); crates/visionclaw-protocol/src/protocol/v3_frame.rs (byte-identical); re-exports src/protocol/mod.rs:9, crates/.../lib.rs:32 | hours |
| C5 | **V4 delta path is dead scaffolding.** `DeltaNodeData` (20B) + `MessageType::PositionDelta=0x04` declared; `decode_node_data` returns Err for V4; no `encode/decode_node_data_delta` exists anywhere. Delete or implement. | src/utils/binary_protocol.rs:68-80,528,1377 | hours |
| C6 | **Client `PROTOCOL_V4` phantom default** — declared "CURRENT: 6-byte header" and set as `PROTOCOL_VERSION` default, but no live path uses a V4 header (positions=0x03/52B, agent-actions=0x23 bare tag). This constant spawned bug 67503fb39. Demote/remove or annotate unused. | client/src/services/binaryProtocol/frameTypes.ts:7-8 | trivial |
| C7 | **ADR-113 condensation index has no scheduler.** Lib + refresh script exist but nothing invokes them on GitHubSync/elevation — no cron/supervisor/agentbox.toml entry. "Triggered incrementally on sync" is unwired; the class index can silently go stale. | agentbox/mcp/servers/lib/{ontology-condense,ontology-index-build}.js; agentbox/scripts/ontology-condense-refresh.sh | hours |

### P2 — hygiene / trap removal

| # | Task | Files | Effort |
|---|---|---|---|
| C8 | **Delete/archive the `crashbug` branch (tip d1f7f254).** Standing trap: carries the full Neo4j-dependent BrokerActor stack (broker_actor.rs 625L, server_nostr_actor.rs, neo4j_broker_adapter.rs 337L). Any merge/cherry-pick reintroduces a Neo4j dependency into an Oxigraph+SQLite stack. | git branch (crashbug) | trivial |
| C9 | **Delete stale archive worktree Neo4j adapters** (9 copies) + the jss-cut-scaffold copy — they keep grep/agents "finding" Neo4j. | _archive/2026-07-10/visionflow-worktrees/*/src/adapters/neo4j_adapter.rs; .claude/worktrees/jss-cut-scaffold/... | trivial |
| C10 | **Delete orphan RVF artifacts** — no reader exists. (`./agentdb.rvf.lock` is currently dirty in git status.) | ./agentdb.rvf, ./agentdb.rvf.lock, .claude/worktrees/jss-cut-scaffold/agentdb.rvf | trivial |

### Cleanly-deferred engineering (record, do not force now)
XR APK cross-build (rustup + aarch64-linux-android + cargo-ndk; **sprint**), LiveKit Android AAR JNI voice bridge (PRD-008 §5.5; **sprint**), physical Quest 3 on-device validation of the Monado-only canaries (**days**). These are honestly self-reported gaps — see §4.

---

## 3. BOTH — doc + code must land together

| Item | Code | Doc | Priority |
|---|---|---|---|
| **WebXR guard-or-delete decision** | C1: either write the ADR-130 "install the APK" guard into VRGraphCanvas.tsx:41-61 **or** execute ADR-071 Phase 3 (delete client/src/immersive/ + App.tsx:11,187 wiring + platformManager XR surface). | If guard: correct `ADR-130:19-21` D1 to state a guard was *actually* written (it never was — grep `deprecat\|apk` in immersive/ = 0 hits). If delete: gap-close evidence file `docs/gap-close-evidence/P?-ADR071-PHASE3.md` with Falsification block; then troubleshooting.md's "removed" banner becomes true. | **P0** |
| **Neo4j-removal decision record** | C8/C9: delete crashbug branch + archive adapters so grep-truth matches the doc. | 1e-2: write `ADR-132-neo4j-removal-oxigraph-adoption.md`; re-point README:29 / graph-schema:14,485 / configuration:250. | **P0** |
| **Anomaly N3 reconciliation** | C4 (delete dead V3F0) + C6 (client V4 phantom). | Rewrite N3 to inventory `{52B V3 live, 0x23 agent-action live, 28B V3F0 dead×2}`, drop "server never emits V4"; append reference to commit 67503fb39 (server DOES emit 0x23 via encode_agent_actions:1233). | **P1** |
| **ADR-117 clamp** | C3 (add server LIMIT/row/byte cap). | ADR-117 backfill (1e-1) records the clamp as the WS-0 hard invariant *after* it actually ships, not before. | **P1** |
| **ADR-119 telemetry** | C2 (inject real sink + startup canary). | ADR-119 backfill (1e-1) + a `docs/gap-close-evidence/` file with a Falsification block asserting `fail_open_count` is observable. | **P1** |
| **AUTH-001 RBAC** | Decide: merge `sprint-3/jss-cut-scaffold`'s `src/middleware/enterprise_auth.rs` (four-tier Admin>Broker>Auditor>Contributor, Nip98RoleResolver, X-Enterprise-Role) to main, **or** leave on branch. | `KNOWN_ISSUES.md:24` currently cites `enterprise_auth.rs` as if on main — it is not. If not merging: banner AUTH-001 that only NIP-98 primitives (src/utils/nip98.rs) are on main; main's src/middleware/auth.rs uses a coarser AccessLevel enum. | **P1** |
| **ADR-120 propose-auth** | Already shipped (ontology_agent_handler.rs:355-361 + passing tests/rec1_route_guard.rs). | Write ADR-120 (1e-1) citing the code + test as closure evidence. Documentation-only closure of a shipped decision. | **P1** |

---

## 4. Explicitly NOT recommended (honest-residual culture already handles these)

Do **not** generate churn on:

- **Grep-count drift** (BrokerActor 38→44, ServerNostrActor 18→22): P0-REC-2.md explicitly frames these as point-in-time; only the *zero-unqualified-live-voice* invariant is load-bearing. No edit. (Optional one-liner in P0-REC-2.md that live counts drift — but not required.)
- **The append-only self-correction inside P0-REC-2.md** ("46-all-legit → incomplete → 38"): this is the correction culture working as designed. Leave intact.
- **Cleanly-deferred frozen ADRs** ADR-073 (relay mesh — code matches: nostr_bridge.rs is one-way, no NIP-42), ADR-078-085 (frozen, standalone-first), and the genuinely-still-unbuilt ADR-122/123. Code matches doc; no drift.
- **ADR-074/075/076/077** — correctly NOT frozen; only the *narrative* "073..085 whole range frozen" overstates. Fix the narrative if it appears, but do not touch the ADR headers.
- **KNOWN_ISSUES AGENT-001** (rvf backend) — honest "not implemented", accurate. No change beyond the back-link (1f).
- **PRD-005 §19.4 blockers, ADR-041 dual banners, ADR-102 wire spec, xr_boot.gd use_xr, useVRHandTracking deletion, Godot APK/LiveKit/on-device gaps** — all verified ACCURATE and honestly self-reported. Do not re-litigate.
- **CHANGELOG.md / KNOWN_ISSUES.md:69 historical Neo4j entries** — correctly past-tense/append-only; not forward-looking claims.
- **Rewriting ADR-061 D1 body or ADR-121 body prose** — forbidden by append-only culture. Banner only.

---

## 5. Proposed ADR-131 + anomaly-register entry set

### New ADR: `docs/adr/ADR-131-doc-drift-reconciliation-2026-07.md`

```
# ADR-131 — Documentation-Drift Reconciliation Sweep (2026-07-22)
Status: Accepted — 2026-07-22
Supersedes-in-part: banner-level corrections to ADR-061, PRD-007, PRD-014, PRD-023, ADR-121

## Context
A six-axis adversarial doc-drift audit (neo4j-migration, phantom-actors,
binary-protocol, xr-client, missing-docs, deferred-vaporware) found the code
correct and consistent on the load-bearing facts (Oxigraph+SQLite sole store;
no BrokerActor/ServerNostrActor/Neo4j on main; 52B V3 wire live) but the doc
corpus carrying: (a) a fabricated completion checkbox for a deleted DB, (b) an
un-narrated Neo4j-removal decision undercited to an auth ADR, (c) seven
shipped-but-undocumented ontology ADRs, (d) a live desktop-as-VR bug the docs
claim was retired, (e) narrowly-scoped banners leaving dead prescriptive paths.

## Decision
1. All Neo4j-as-live claims corrected by dated banners (never rewritten); the
   originating removal decision recorded as ADR-132.
2. ADR-113/115-120 backfilled from shipping code; ADR-112 §5 register flipped
   proposed→written.
3. ADR-061 & PRD-007 marked Superseded-by-ADR-102 (52B wire canonical).
4. WebXR: guard-or-delete fork resolved via ADR-071 Phase 3 gap-close (§3).
5. Dead code (V3F0 ×2, V4 delta, crashbug branch, archive/orphan artifacts)
   scheduled for deletion; unwired ADR-119 telemetry / ADR-117 clamp / ADR-113
   scheduler tracked as the final-mile code debt.
6. PRD-015 three-way identifier collision and PRD-021/ADR-126 provenance gap
   dispositioned per §1f.

## Consequences
Doc authority chain restored; grep-truth aligned to prose after C8/C9/C10.
Falsification evidence for each shipped closure filed under docs/gap-close-evidence/.
```

Companion new file: **`ADR-132-neo4j-removal-oxigraph-adoption.md`** (the missing originating decision — see 1e-2).

### Anomaly-register updates (`docs/diagrams/00-anomaly-register-...md`)

**Close (with evidence pointer):**
- **N-neo4j-substrate** → CLOSED: Oxigraph+SQLite verified sole store (Cargo.toml, oxigraph_graph_repository.rs); doc banners land via ADR-131.
- **Phantom-actor axis** → CLOSED on substance (already accurate); only PRD-023:82 edit outstanding.

**Downgrade / re-word (do not mark FIXED):**
- **"24B/28B vs 52B wire width — FIXED"** → re-label **"banner-only; ADR-061 D1 body + 9+28*N formula still 28B; ADR-102 docs-alignment pass outstanding."** The register overstated closure.

**Open (new entries):**
- **N-webxr-desktop-vr (Sev HIGH, Open):** enterVR() never sets isXRMode; live desktop-as-VR bug ships; ADR-130 D1 claims a guard that was never written. Files: VRGraphCanvas.tsx:43, platformManager.ts:289, GraphManager.tsx:61.
- **N-ontology-telemetry-noop (Sev HIGH, Open):** ADR-119 liveness sink defaults to no-op; fail_open_count unobservable. File: ontology-retrieval.js:128.
- **N-sparql-clamp-halfshipped (Sev MED, Open):** no server-side LIMIT/row/byte cap on read-only SPARQL. File: ontology_handler.rs:827-845.
- **N-v3f0-dead-encoder (Sev LOW, Open):** two byte-identical dead 28B V3F0 encoders + a lying docstring. Files: src/protocol/v3_frame.rs, crates/visionclaw-protocol/src/protocol/v3_frame.rs.
- **N-crashbug-branch-trap (Sev MED, Open):** unmerged crashbug branch carries a Neo4j-dependent actor stack; deletion pending.
- **N3 (rewrite, keep Open until C4/C6):** correct to `{52B V3 live, 0x23 agent-action live server-emitted, 28B V3F0 dead×2}`; drop "V4 header server never emits" (falsified by commit 67503fb39).

---

## Consolidated priority ladder

**P0 (do first):** PRD-014:248 false checkbox banner · ADR-132 + mislink repoint · WebXR guard-or-delete (C1 + doc) · ADR-119 telemetry no-op (C2) · ADR-117 unbounded SPARQL (C3).
**P1:** all §1b/1c Neo4j banners · ADR-061/PRD-007 supersede · PRD-023:82 · ADR-121 W0 banner · seven retroactive ADRs · C4/C5/C7 · AUTH-001 reconciliation · PRD-015 collision · troubleshooting.md banner swap.
**P2:** ADR-026 fold-in map · archive/README banner · ADR-126/PRD-021 provenance · PRD-012 status · C6/C8/C9/C10 · anomaly-register re-word.

Files central to this plan (absolute): `/home/devuser/workspace/project/docs/prd/PRD-014-ecosystem-productionisation.md`, `/home/devuser/workspace/project/docs/adr/ADR-061-binary-protocol-unification.md`, `/home/devuser/workspace/project/docs/adr/ADR-121-self-improving-ontology-writeback-loop.md`, `/home/devuser/workspace/project/client/src/immersive/threejs/VRGraphCanvas.tsx`, `/home/devuser/workspace/project/src/protocol/v3_frame.rs`, `/home/devuser/workspace/project/agentbox/mcp/servers/lib/ontology-retrieval.js`, `/home/devuser/workspace/project/src/handlers/ontology_handler.rs`, and new `/home/devuser/workspace/project/docs/adr/ADR-131-doc-drift-reconciliation-2026-07.md` + `ADR-132-neo4j-removal-oxigraph-adoption.md`.