# ADR-130: Gap-Close Sprint — VisionClaw Decisions

**Status:** Proposed
**Date:** 2026-07-08
**Deciders:** jjohare, VisionClaw platform team
**Governed by:** [ADR-004 Gap-Close Sprint Governance](../../../VisionFlow/docs/ADR-004-gap-close-sprint-governance.md)
**Child PRD:** [PRD-023 Gap-Close VisionClaw](../prd/PRD-023-gap-close-visionclaw.md)
**Child DDD:** [ddd-gap-close-visionclaw-context](../ddd/ddd-gap-close-visionclaw-context.md)
**Related:** ADR-071 (Godot XR replacement), ADR-102 (XR transport completion), ADR-110 (ACSP control surfaces), ADR-041 (judgment broker workbench), ADR-043 (KPI lineage model), ADR-125 (did:nostr Multikey convergence)

## Context

The VisionClaw slice of the gap register (PRD-023) closes 17 gaps and five commitments across desktop, mixed reality and voice. Six of those closures cannot proceed without a decision that the register does not make, because each forces a choice between two live architectures or between a documented design and the code that contradicts it. This ADR records those six decisions with the alternatives weighed, so a reviewer can check the choice against the evidence rather than the prose.

Two facts from the recon pass frame most of the decisions. First, the codebase carries two coexisting XR client stacks: the WebXR immersive tree under `client/src/immersive/` (which ADR-071 marks for deletion, Phase 3 still `PLANNED`) and the Godot native client under `xr-client/` (production-grade, the canonical target). Second, a `BrokerActor` described as live infrastructure in five committed documents exists only on the unmerged `crashbug` branch, tied to a Neo4j store the codebase does not run, while `main` deliberately took a different architecture (ADR-110 ACSP, accepted 2026-06-12, after `crashbug`'s HEAD of 2026-05-15).

## Decision 1 — Mixed reality lands in the Godot client; the WebXR tree is deprecated in place, not fixed and not yet deleted

M1–M4 and M6 implement in the Godot client (`xr-client/`) per ADR-071 and the copresence research brief. The WebXR immersive tree is neutralised, not repaired: the VR entry button (`VRGraphCanvas.tsx:66`) gains a deprecation guard that shows a "install the APK" notice instead of rendering a broken desktop-as-VR view, and the tree's deletion stays on the ADR-071 Phase 3 track outside this sprint.

The register consequence is explicit. M6 ("`enterVR()` never sets `isXRMode`; XR renders as desktop") is a WebXR-only defect (`platformManager.ts`, `VRGraphCanvas.tsx:66`). The Godot client has no such defect: `xr_boot.gd:18` sets `get_viewport().use_xr = true` synchronously in `_ready()`. So M6 closes at `integrated` against the Godot client (verified via the xr-runtime sidecar), and the WebXR `isXRMode` bug is retired by deprecation, its locus recorded as `planned` for deletion under ADR-071 Phase 3. M4's "targeting ray at world-origin" bug is likewise a WebXR defect (`useVRHandTracking.ts:93` defaults hand refs to `(0,0,0)`); the Godot equivalent sources the ray correctly from `XRController3D.global_position` (`graph_scene.gd:303`), so M4 in Godot is a gaze-fallback addition plus a live-session verification, not a bug fix.

> **Update 2026-07-15:** `useVRHandTracking.ts` (the M4 defect locus) was deleted ahead of
> ADR-071 Phase 3, together with the rest of the legacy agent-action renderer chain
> (`VRAgentActionScene`, `VRActionConnectionsLayer`, `useVRConnectionsLOD`; see the
> ADR-059 addendum). The M4 WebXR defect is therefore retired by removal, not merely
> deprecation; the Godot-side gaze-fallback work is unaffected.

### Alternatives considered (Decision 1)

| Alternative | Verdict | Rationale |
|---|---|---|
| Complete the ADR-071 Phase 3 cutover in this sprint (delete WebXR + Vircadia, land LiveKit AAR) | Rejected | Out of the register's owned-item scope, carries the LiveKit Android AAR dependency ADR-071 Phase 2 left open, and risks the stable desktop build for a deletion that does not close any gap. |
| Fix the M4/M6 bugs in the WebXR tree in place | Rejected | Repairs code ADR-071 marks for deletion; a fixed `isXRMode` in a tree that is about to be removed is wasted work and a false maturity signal. |
| Implement MR in Godot; deprecate the WebXR VR entry; defer deletion to ADR-071 Phase 3 (chosen) | Chosen | Closes M1–M4/M6 against the canonical surface, stops the WebXR entry from misleading a user, and keeps the large deletion on its own track. The register records M6 `integrated` (Godot) with the WebXR locus `planned`. |

## Decision 2 — Supersede the `crashbug` BrokerActor transport; cherry-pick its storage-agnostic domain kernel onto the ACSP case queue

The `crashbug` branch carries a 625-LOC `BrokerActor` (`src/actors/broker_actor.rs`) that persists to Neo4j (`src/adapters/neo4j_broker_adapter.rs`, 337 LOC, 10 Neo4j references) and broadcasts `broker:new_case`/`broker:case_decided`. That branch is a freeze-debugging branch: its log is a run of `EXPERIMENT`/`Revert` commits chasing a client-side re-render freeze, and it diverged before `main` adopted ADR-110. Its Neo4j dependency targets a store the codebase does not run (the graph store is Oxigraph plus SQLite).

The domain kernel underneath the actor is worth keeping and the transport is not. Cherry-pick the storage-agnostic `src/domain/broker/{broker_case,broker_decision,precedent_registry,mod}.rs` (936 LOC of `BrokerCase`, `DecisionOrchestrator`, `DecisionOutcome`, `PrecedentRegistry` — the decision invariants and the graduated-outcome model) onto `main` to give the ACSP case queue real domain invariants. Leave the Neo4j adapter and the actor transport behind. Surface the queue against the existing ACSP producer (`src/services/acsp/`) and `broker_inbox_handler` (`GET /api/broker/inbox`), publishing `broker:new_case`/`broker:case_decided` over the existing multiplexed graph socket, and enable `ElevationActor` by default in a dev/staging profile.

Correct the documentation the same change touches: ADR-041 is marked superseded-in-part by ADR-110 plus this ADR, and every document that still names `crashbug`'s `BrokerActor` as live `main` code (CHANGELOG, ADR-033, `docs/explanation/ecosystem-convergence.md`, `docs/reference/rest-api.md`) is corrected to describe the ACSP forum-hosted case queue.

### Alternatives considered (Decision 2)

| Alternative | Verdict | Rationale |
|---|---|---|
| Cherry-pick or rebase the whole `crashbug` broker (actor + Neo4j adapter) onto `main` | Rejected | Brings a dead storage dependency (no Neo4j runs), reintroduces a transport ADR-110 deliberately replaced, and pulls the freeze-debugging branch's reverted fixes into an architecture two commits-of-history downstream of it. |
| Supersede `crashbug` entirely and build a fresh case queue from scratch | Rejected | Discards 936 LOC of tested decision invariants (`DecisionOrchestrator` delegate/promote/precedent, `DecisionOutcome` graduated model) that the ACSP queue needs anyway; the kernel is storage-agnostic and directly reusable. |
| Supersede the transport, cherry-pick the domain kernel, surface against ACSP (chosen) | Chosen | Keeps the decision invariants, drops the dead Neo4j transport, closes REC-2/D3 on the architecture `main` already committed to (ADR-110), and forces the doc corrections that stop `crashbug`'s `BrokerActor` reading as shipped fact. |

## Decision 3 — The LivenessHarness is a central live-traffic observer, registrable from any repository, that fires only on observed traffic

The `LivenessHarness` (RES-a) is a service in `visionclaw-server` that observes live wires and records a `CanaryFired` only when real traffic crosses a registered wire. It is not a synthetic prober. RES-a's own finding is that a passive `/api/health` and a `/healthz` probe already exist (`consolidated_health_handler.rs:63`, `main.rs:865`) and prove nothing about whether any loop carries traffic; a green ping is exactly the false closure ADR-004 forbids.

The service exposes three surfaces. `POST /api/canary/register` accepts a canary declaration (`canary_id`, wire descriptor, fire predicate, owner repository, wave) from any repository, backed by a shared `canary-manifest.json` schema each repository commits. `POST /api/canary/observe/{canary_id}` records a fire from a repository that reaches it over HTTP; a Nostr-relay tap records fires from repositories that speak only Nostr (forum, solid-pod), by subscribing to the wires they already emit. `GET /api/canary/status` returns per-canary `{armed, fired, last_fired_at, observation_count, sha_at_registration}`. A KG-backend watchdog (tokio interval, self-polling `/api/health`) drives a `kg_backend_up` gauge and raises `CANARY-VC-RESA-KG` on loss rather than failing open silently. Standing-versus-one-shot durability resolves per item in PRD-023's canary table: correctness fixes fire once; KPI-feeding and embodiment loops are standing monitors that must stay green for a wave to remain promoted (DDD Gap-Close open issue 2).

### Alternatives considered (Decision 3)

| Alternative | Verdict | Rationale |
|---|---|---|
| A synthetic prober that pings each repository's endpoints on a schedule | Rejected | A successful ping proves the endpoint answers, not that the loop carries traffic — the "built, and unwired" false closure the sprint exists to prevent. RES-a shows the passive probes already exist and are insufficient. |
| Per-repository local canaries with no central registry | Rejected | Produces no cross-repo score and lets each repository judge its own closure — the drift ADR-004's one-register decision forbids. |
| A central observer that taps live wires and accepts registrations + fired-observations from any repository (chosen) | Chosen | Registrable across repositories, fires only on observed live traffic, and holds the one place the four-surface score reads its liveness evidence. |

## Decision 4 — Copresence adopts minimal legible avatars and head-gaze-primary selection per the research brief

The Godot copresence design follows `scratchpad/xr-copresence-research-brief.md`. Agents render as minimal geometric cores (orb or prism) with an explicit gaze cone, a screen-facing name/DID badge, and state shown by colour and motion (idle bob/dim, working pulse, awaiting-approval saturated colour), not by a humanoid skeleton. User selection unifies head-gaze and eye-gaze behind one gaze-ray abstraction: head-gaze is the primary path (calibration-free, and Quest 3, the floor device, has no eye-tracking hardware), eye-gaze a progressive enhancement gated on `OpenXRInterface.is_eye_gaze_interaction_supported()` after OpenXR init. Proxemics apply Hall's zones as radii with a 1.5–2.5 m social-band default, agents arranged on a forward arc (±60°) with equal angular spacing, re-solved on locomotion by a lightweight Rust solver. Three selection resolvers (controller ray via godot-xr-tools, hand pinch detected from `XRHandModifier3D` joint distances, gaze-dwell at 400–800 ms with a charging reticle to mitigate Midas touch) feed one arbiter. Presence replicates at 10–20 Hz with client-side interpolation and a reliable channel for discrete state. The Rust-owns-sim / GDScript-owns-rig split follows the gdext interop guidance.

Body tracking and face tracking are out of scope and stay out: Quest 3 has no face-tracking hardware and Meta body tracking is upper-body estimated only. M3 therefore targets `scaffolded` for the copresence set and may reach `integrated` only on the geometric-avatar + gaze-cone + proxemics-solver subset that Quest 3 can actually run; any sub-feature not instantiated is labelled `scaffolded`, never folded into a closed M3.

### Alternatives considered (Decision 4)

| Alternative | Verdict | Rationale |
|---|---|---|
| Humanoid full-body avatars with IK and body/face tracking | Rejected | Quest 3 lacks eye and face hardware and estimates only upper body; the research favours minimal legible forms, which give stronger co-presence than botched realism and dodge the uncanny-valley trust penalty. |
| Eye-gaze-primary selection | Rejected | The floor device (Quest 3) returns false from `is_eye_gaze_interaction_supported()`; an eye-gaze-primary design does not run on the target hardware. |
| Minimal legible avatars, head-gaze primary with eye-gaze progressive enhancement, three resolvers into one arbiter (chosen) | Chosen | Runs on Quest 3, matches the empirical proxemics and gaze-cue findings, and keeps the deferred body/face work honestly labelled `scaffolded`. |

## Decision 5 — Resurrect ADR-043 with its storage re-targeted from Neo4j to SQLite plus an Oxigraph lineage graph

ADR-043 (accepted 2026-04-14) specifies four KPIs with a Neo4j `OrganisationalMetricSnapshot` and `DERIVED_FROM` lineage edges, and three months later carries zero implementation. The storage assumption is stale: the codebase runs Oxigraph plus SQLite, not Neo4j, so a literal implementation of ADR-043 would need a redesign before a single line of dashboard code. Re-target the snapshot store to a SQLite metrics table analogous to `sqlite_enrichment_repository.rs`, with lineage held as an optional Oxigraph named graph. Compute Augmentation Ratio and Trust Variance first, from existing sources (`/wss/agent-events` volume, `enrichment_proposals` decision outcomes) without new instrumentation; Mesh Velocity and HITL Precision follow once REC-2's case queue and REC-10's loop supply their source events.

### Alternatives considered (Decision 5)

| Alternative | Verdict | Rationale |
|---|---|---|
| Implement ADR-043 verbatim on Neo4j | Rejected | No Neo4j runs in the stack; the ADR's `DERIVED_FROM` Cypher model has no engine to execute against. |
| Compute KPIs on-demand from event queries, no snapshots | Rejected | ADR-043's own Option 3 rejection stands: live aggregation across thousands of events cannot meet the 2-second dashboard target and gives no historical trend. |
| SQLite snapshots + optional Oxigraph lineage, Augmentation Ratio and Trust Variance first (chosen) | Chosen | Uses stores the codebase actually runs, reuses the existing SQLite repository pattern, and computes two KPIs from sources that already exist, evidencing REC-4 rather than asserting it. |

## Decision 6 — COM-14 verifies a Schnorr signature over a client challenge and matches it to the spawn-payload DID before trust

The COM-14 cross-repo boundary is fixed by the queen: agentbox mints and attaches a `did:nostr` at spawn and includes it in the spawn payload; VisionClaw carries it through the `Agent` struct and verifies before trust. The verification is a challenge/response, not a DID-document read. VisionClaw issues a nonce challenge; the agent (via agentbox) returns a NIP-98-style event signed over the challenge; VisionClaw verifies the BIP-340 Schnorr signature against `event.pubkey` (the `signer.rs` / `nostr_identity_verifier.rs` path already in service) and checks that `did:nostr:{event.pubkey}` equals the spawn-payload DID. This is ADR-125 I3-safe by construction: the auth path reads the raw event pubkey and never resolves or parses the DID-document verificationMethod. A node whose signature fails, or whose derived DID does not match the payload, is not trusted and cannot receive a 31402.

### Alternatives considered (Decision 6)

| Alternative | Verdict | Rationale |
|---|---|---|
| Trust the spawn-payload `did:nostr` on presentation, no signature check | Rejected | A payload field is spoofable; trusting it unverified reproduces the register's identity-blind finding under a new name. |
| Resolve the DID document and verify against its `publicKeyMultibase` verificationMethod | Rejected | Violates ADR-125 I3 (auth must read `event.pubkey`, never the DID-doc VM) and adds a resolution dependency the auth path does not need. |
| Challenge/response Schnorr over `event.pubkey`, match derived DID to the payload (chosen) | Chosen | Proves control of the key behind the claimed DID, reuses the existing raw-pubkey verifier, and stays I3-safe. |

## What This ADR Does Not Decide

- **Implementation of any work package.** PRD-023 owns the acceptance criteria and the falsification statements; this ADR owns the six architectural forks.
- **The `did:nostr` document shape or the mint.** ADR-125 fixes the shape; agentbox owns the mint. This ADR decides only how VisionClaw verifies.
- **The voice-intent producer.** agentbox owns `/v1/voice-intent`; this ADR decides only the consumer-side binding and acknowledgement (PRD-023 WP-5).
- **Wave assignment, ownership, or severity.** Those are canon-register properties (ADR-004 Decision 7); a change to them is a canon edit, not an ADR revision.

## Consequences

### Positive

- The MR work targets one surface (Godot), and the WebXR entry stops misleading users without the cost and risk of the Phase 3 deletion.
- The broker case queue lands on the architecture `main` already committed to, with the decision invariants preserved and the dead Neo4j transport dropped, and the five stale documents corrected in the same change.
- One live-traffic observer holds the sprint's liveness evidence, registrable from every repository, so the four-surface score reads from one place.
- The copresence design runs on the floor device and labels its deferred body/face work honestly.
- REC-4 computes against stores that exist, so the four-KPI dashboard can be evidenced rather than accepted-and-abandoned like ADR-043's first three months.

### Tradeoffs

- Cherry-picking the `crashbug` domain kernel means reconciling 936 LOC written against a different transport with the ACSP producer's serde shapes; the round-trip tests in `src/services/acsp/events.rs` are the contract that reconciliation must hold.
- Deferring the WebXR deletion leaves dead code in the tree for one more track; the deprecation guard is the mitigation against it being mistaken for live.
- The `LivenessHarness` adds a service other repositories depend on; its availability becomes a sprint dependency, mitigated by the fail-open watchdog (a harness outage does not block a wave, it re-arms the affected canaries).

### Risks

- The register's standing risk applies here first: an accepted design that sits unbuilt. Decision 3's live-traffic gate is the structural answer — a work package whose canary never fires registers as `Open`, visibly, exactly as ADR-043 should have for three months.
- The MR half of every finding is verified only through the Monado sidecar; no `godot` binary exists in the container. A closure that claims a live MR session must carry a sidecar receipt, not a static read.

## ADR-trail (implementation notes)

### 2026-07-08 — M2 intervention decide transport (Decision 4 / WP-9 stage 2)

PRD-023 WP-9 M2 directs the in-headset decision to POST "via the existing enrichment/broker decide route through `transport.rs` — add the thin client call if absent". The Godot stage-2 implementation carries the decision over Godot's built-in `HTTPRequest` node with a Rust-signed NIP-98 `Authorization` header (`NostrAuth` in `xr-client/rust/src/signer.rs`, `nip98_http_authorization`), rather than adding a `reqwest`/TLS HTTP client to `transport.rs`. Reasons, per the deviation discipline:

1. `xr-client/rust` carries no HTTP-client dependency (only `tokio-tungstenite` for the two WebSocket streams). Adding `reqwest` + a TLS stack inflates the Quest APK for a single cold, event-driven action.
2. The decide is a one-shot operator action, not hot per-frame data; the brief's "batch across the boundary; don't cross it every frame for hot data" argues against making it a Rust socket call, while it says nothing against a cold GDScript HTTP call.
3. The security-critical half — the signature — stays in Rust: the secret key never crosses the GDExtension boundary; only a single-use signed header does. `NostrAuth.nip98_header(url, "POST")` reuses the same event-builder as the WS `authenticate` envelope, so it interops byte-for-byte with the server's `verify_nip98_auth`.

The route and auth are unchanged from the desktop path: `POST /api/broker/cases/{id}/decide` (`enrichment_proposals_handler::decide_as_operator`) under the `power_user()` gate, which accepts NIP-98 as its primary scheme (`src/settings/auth_extractor.rs:109`). The decision funnels through the same kernel + persistence + `broker:case_decided` core as the desktop operator and the agentbox bridge. This is a transport choice within Decision 4, not a new architectural fork.
