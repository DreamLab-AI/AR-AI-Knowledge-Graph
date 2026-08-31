# DDD: Gap-Close VisionClaw Bounded Context

**Status:** Living document
**Date:** 2026-07-08
**Scope:** VisionClaw's slice of the four-surface gap register — desktop, mixed reality, voice, and the sprint-wide liveness harness
**Governed by:** [PRD-023 Gap-Close VisionClaw](../prd/PRD-023-gap-close-visionclaw.md), [ADR-130 VisionClaw Decisions](../adr/ADR-130-gap-close-visionclaw-decisions.md)
**Conformant to:** [DDD Gap-Close (canon)](../../../VisionFlow/docs/DDD-gap-close-context.md), Judgment Broker context (ADR-041/ADR-110), Ecosystem Alignment context (ADR-002)

---

## 1. Bounded Context

This context is VisionClaw's supplier view into the Gap-Close Sprint. It owns the aggregates and events that carry a gap on the desktop, MR and voice surfaces from `scaffolded` to a canary-fired closure, plus the `LivenessHarness` every repository's canaries fire against. It does not redefine the Gap-Close lifecycle (register, waves, closure protocol) — that belongs to the canon context, and this context conforms to it. It does not own the broker decision loop's rules; those belong to the Judgment Broker context, and this context consumes `BrokerCase` and `DecisionOutcome` without a parallel model.

The context sits downstream of three others. Ecosystem Alignment supplies the maturity vocabulary. Gap-Close (canon) supplies the closure protocol — falsification, receipt, canary. Judgment Broker supplies the decision aggregates that REC-2/D3 surface. VisionClaw adds the surface-side aggregates: the embodiment `Agent`, the `VoiceCommand`, the `XRPresenceSession`, the `AgentActionEnvelope`, and the `LivenessCanary` it hosts for the whole sprint.

---

## 2. Context Map

| Context | Relationship | Notes |
|---|---|---|
| **Gap-Close (canon)** | Conformist (upstream) | Closure protocol, waves, canary requirement, maturity tiers — consumed verbatim |
| **Judgment Broker** (ADR-041/ADR-110) | Conformist (upstream) | `BrokerCase`, `DecisionOutcome`, ACSP kinds 31400–31405 consumed as-is; REC-2 surfaces them, does not redefine them |
| **Ecosystem Alignment** (ADR-002) | Conformist (upstream) | Seven-tier maturity vocabulary |
| **agentbox** | Customer/Supplier (upstream supplier) | Mints `did:nostr` at spawn (COM-14), owns `/v1/voice-intent` (COM-15 producer), emits CTC fields (REC-3), supplies the skill-count for RES-d |
| **nostr-rust-forum** | Customer/Supplier (peer) | Hosts the D1 `broker_cases` store the ACSP producer publishes to; fires forum canaries against this context's `LivenessHarness` over the Nostr tap |
| **solid-pod-rs** | Customer/Supplier (peer) | Contributes the provenance trail for REC-11; fires pod canaries via the Nostr tap |
| **VisionFlow (canon)** | Customer/Supplier (downstream) | `DriftCounter` consumes this context's ontology class-count source (RES-d); canon reconciles the closed-item tiers |

### Relationship types

- **Gap-Close → this context:** Conformist. The falsification/receipt/canary protocol is obeyed, not re-authored.
- **Judgment Broker → this context:** Conformist. REC-2/D3 render `BrokerCase`/`DecisionOutcome` through ACSP; the cherry-picked `crashbug` domain kernel (ADR-130 Decision 2) is the same aggregate model, storage-agnostic, not a fork.
- **agentbox → this context:** Customer/Supplier with fixed sub-item boundaries (COM-14 mint vs carry-and-verify; COM-15 producer vs consumer; REC-3 emit vs schema).

---

## 3. Aggregates

| Aggregate | Root | Description |
|---|---|---|
| `Agent` (embodiment node) | Yes | The actor node on every surface. Keyed by `did:nostr`, not `task_id`. Consistency boundary: a node is trusted only after its `did:nostr` survives a `uri::did_nostr()` round-trip and a Schnorr challenge (WP-1, ADR-130 Decision 6). |
| `BrokerCase` | No (member; root in Judgment Broker) | Consumed from the upstream context. Surfaced by REC-2/D3 through the ACSP producer and `broker_inbox_handler`; not re-rooted here. |
| `VoicePttSession` | Yes | A push-to-talk session bound to one human and one selected `Agent`. Consistency boundary: a spoken command dispatches a 31402 only when a selected-agent `did:nostr` is bound (WP-5). |
| `XRPresenceSession` | Yes | A Godot copresence session: the room membership, the avatar set, gaze targets and proxemics placement. Consistency boundary: an avatar renders a verified identity badge only when the `signer.rs` Schnorr path confirms the DID (WP-9). |
| `AgentActionEnvelope` | Yes | The `/wss/agent-events` envelope (`schema.rs:24`). Carries the typed CTC fields (REC-3) and the identity fields; the wire schema VisionClaw owns and agentbox emits. |
| `LivenessCanary` | Yes | A registered probe against the `LivenessHarness`. Consistency boundary: it transitions `armed`→`fired` only on observed live traffic matching its predicate, and re-arms when stale (WP-11). |
| `KpiSnapshot` | Yes | A point-in-time KPI value with its lineage to source events, stored in SQLite with an optional Oxigraph lineage graph (REC-4, ADR-130 Decision 5). |

---

## 4. Entities

| Entity | Identity | Owner aggregate |
|---|---|---|
| `AgentNode` | `did:nostr:<hex>` | `Agent` |
| `IdentityBadge` | `did:nostr` + verification state | `Agent` / `XRPresenceSession` |
| `SelectedAgentBinding` | (session id, `did:nostr`) | `VoicePttSession` |
| `AvatarPresence` | `did:nostr` + pose frame | `XRPresenceSession` |
| `GazeTarget` | resolver id + targeted node id | `XRPresenceSession` |
| `CaseView` | `BrokerCase` id | `BrokerCase` (rendered) |
| `CanaryRegistration` | `canary_id` + owner repo + registration SHA | `LivenessCanary` |
| `MetricLineageEdge` | (snapshot id, source event id) | `KpiSnapshot` |

---

## 5. Value Objects

| Value Object | Fields | Notes |
|---|---|---|
| `DidNostr` | `did:nostr:<64-hex>` | ADR-125 canonical; identity is the BIP-340 x-only hex (I1); verification reads `event.pubkey`, never the DID-doc VM (I3) |
| `ChallengeResponse` | nonce, signed event, `event.pubkey` | The Schnorr proof binding a node to its claimed DID (Decision 6) |
| `BeamFrame` | `0x23` `AGENT_ACTION` binary frame | Decoded at `binaryProtocol.ts:420` into `transientBeamStore` |
| `PoseFrame` | head + optional hand transforms, `0x43` wire | 90 Hz nominal; gaze vector and body joints absent in v1 (`types.rs:198`) |
| `ProxemicsBand` | intimate <0.45 m, personal 0.45–1.2 m, social 1.2–3.6 m (default 1.5–2.5 m), public >3.6 m | Hall's zones per the research brief |
| `GazeRay` | origin, direction, source (head/eye/controller) | Head-gaze primary, eye-gaze progressive (Decision 4) |
| `AcspKind` | 31400–31405 | Panel definition/state/request/response per ADR-110 |
| `CtcFields` | `handoff_count`, `token_burden`, `verification_outcome` | Typed optional envelope members (REC-3) |
| `MaturityTier` | ADR-002 seven-tier | Target and current stated per item |
| `CanaryMode` | one-shot, standing | Correctness fires once; loop canaries stay green (open issue 2) |

---

## 6. Domain Events

| Event | Trigger | Publisher | Consumer |
|---|---|---|---|
| `AgentSpawned` | agentbox spawns an agent with a `did:nostr` | agentbox | `Agent` (carry) |
| `DidVerified` | Schnorr challenge over `event.pubkey` matches the payload DID | VisionClaw | `Agent`, `IdentityBadge`, `CANARY-VC-COM14-DID` |
| `BeamFrameDecoded` | A `0x23` frame decodes with non-empty agent data | client `binaryProtocol.ts` | `TransientBeamsLayer`, `CANARY-VC-D1-BEAM` |
| `CaseQueued` | ACSP publishes a 31402; queue receives `broker:new_case` | ACSP producer | control-centre case queue, `CANARY-VC-REC2-CASE` |
| `CaseDecided` | A human decision publishes 31403; queue receives `broker:case_decided` | forum relay → VisionClaw | `KpiSnapshot` (HITL), `CANARY-VC-REC2-CASE` |
| `VoiceIntentBound` | A selected-agent `did:nostr` binds to a PTT session | VisionClaw | `VoicePttSession` |
| `ActionRequestSigned` | A spoken command builds a signed 31402 at the bound agent | VisionClaw → agentbox `/v1/voice-intent` | ACSP path, `CANARY-VC-COM15-PTT` |
| `TtsAcknowledged` | Kokoro TTS plays an acknowledgement on acceptance | VisionClaw | `CANARY-VC-COM15-PTT` |
| `StatusTransitionObserved` | The WS dot flips on a real socket drop | client | `CANARY-VC-D5-WS` |
| `CtcEnvelopeEmitted` | An envelope carries a populated typed CTC field | agentbox → VisionClaw schema | `CANARY-VC-REC3-CTC` |
| `KpiSnapshotComputed` | A KPI computes from real source events | VisionClaw | dashboard, `CANARY-VC-REC4-KPI` |
| `AvatarJoined` | A remote avatar joins the Godot room with its DID | presence wire | `XRPresenceSession`, `CANARY-VC-M1-HUD` |
| `GazeResolved` | A gaze or controller ray resolves a non-origin selection | Godot client | `CANARY-VC-M4-RAY` |
| `KgLivenessTransition` | The watchdog flips `kg_backend_up` | `LivenessHarness` | `CANARY-VC-RESA-KG` |
| `CanaryRegistered` | A repository registers a canary declaration | any repo | `LivenessHarness`, SprintWave |
| `CanaryFired` | The harness records live traffic on a registered wire | `LivenessHarness` | SprintWave, ClosureEvidence |

---

## 7. Invariants

1. **No surface keys an agent by `task_id`.** Every surface reads `Agent.did_nostr`. A node presented with only a `task_id` is out of process (WP-1). The register's D4/M1 findings are the standing counter-examples this invariant forbids.

2. **Identity is verified before trust.** An `Agent` is trusted only after a Schnorr signature over a client challenge verifies against `event.pubkey` and `did:nostr:{event.pubkey}` matches the spawn payload (Decision 6). Reading the DID-document verificationMethod for a key is forbidden (ADR-125 I3).

3. **No status indicator is decoupled from ground truth.** Every status field derives from a real check. A hardcoded `websocketStatus="connected"` (D5, `ControlCenter.tsx:139`) or a literal `mcp_status: "not_configured"` (`consolidated_health_handler.rs:190`) violates this invariant.

4. **No loop item closes without its canary fired.** A loop-closing item whose `LivenessCanary` has not fired in a live session is `Open`, regardless of other evidence — the canon Gap-Close invariant applied to this slice.

5. **A canary fires only on observed live traffic.** A synthetic probe never stands in for a `CanaryFired`. The `LivenessHarness` observes wires; it does not poke them into looking alive (Decision 3).

6. **Deferred copresence sub-features are labelled `scaffolded`.** M3 is claimed above `scaffolded` only on the geometric-avatar + gaze-cone + proxemics subset Quest 3 can run; body and face tracking are out of scope and never folded into a closed M3 (Decision 4).

7. **The broker decision model is consumed, not forked.** REC-2/D3 render `BrokerCase`/`DecisionOutcome` from the cherry-picked storage-agnostic kernel through ACSP; no parallel Neo4j-backed broker aggregate is reintroduced (Decision 2).

8. **The register is immutable; corrections chain forward.** This context conforms to the canon `GapRegister`; a wave or ownership change is a canon edit, not a local re-scope.

---

## 8. Ubiquitous Language

| Term | Meaning |
|---|---|
| **Agent (embodiment node)** | An actor rendered on a surface, keyed by `did:nostr`, trusted only after a verified challenge |
| **Embodiment join** | The wire from a live agent action to a drawn beam and avatar on screen (D1) |
| **Case queue** | The control-centre surface rendering pending `BrokerCase`s from ACSP (REC-2/D3) |
| **PTT binding** | The selected-agent `did:nostr` bound to a push-to-talk session before a command dispatches (COM-15) |
| **ACSP** | Agent Control Surface Protocol, Nostr kinds 31400–31405 (ADR-110) |
| **Gaze ray** | The unified head-or-eye gaze abstraction for MR selection (Decision 4) |
| **Proxemics band** | Hall's-zones radius placing agents in the 1.5–2.5 m social band (Decision 4) |
| **CTC** | Contextual transaction cost — handoff count, token burden, verification outcome per DAG (REC-3) |
| **Liveness canary** | A probe that fires only on observed live traffic on a wired loop (RES-a) |
| **LivenessHarness** | The service that registers canaries and records `CanaryFired`, hosting the sprint's liveness evidence |
| **Fabricated status** | A status field not derived from a real check — the honesty defect D5 names |

---

## 9. Services

| Service | Responsibility | Owner | Status |
|---|---|---|---|
| `LivenessHarness` | Registers canaries from any repository; records `CanaryFired` on observed traffic; drives the `kg_backend_up` watchdog | VisionClaw | `planned` → `integrated` (WP-11) |
| `AcspCaseSurface` | Renders `BrokerCase`s from ACSP; publishes `broker:new_case`/`broker:case_decided` | VisionClaw | `scaffolded` → `integrated` (WP-4) |
| `IdentityVerifier` | Schnorr challenge/response over `event.pubkey`; DID-payload match | VisionClaw (`signer.rs`, `nostr_identity_verifier.rs`) | `scaffolded` → `integrated` (WP-1) |
| `VoiceIntentConsumer` | Capture → selected-agent binding → signed 31402 → Kokoro ack | VisionClaw | `scaffolded` → `federation-verified` (WP-5) |
| `KpiComputeService` | Computes Augmentation Ratio / Trust Variance from real events; persists SQLite snapshots | VisionClaw | `planned` → `integrated` (WP-8) |
| `ClassCountSource` | Script-queryable ontology class count for the canon `DriftCounter` | VisionClaw | `planned` → `integrated` (RES-d) |
| `CopresenceSolver` | Rust proxemics + gaze-target + selection-arbitration for the Godot client | VisionClaw | `scaffolded` (WP-9) |

---

## 10. Ownership Summary

| Boundary | This context owns | This context does not own |
|---|---|---|
| Identity | Carrying `did:nostr` through the `Agent` struct; verifying before trust | Minting `did:nostr` (agentbox); the DID-document shape (ADR-125) |
| Voice | Capture, selected-agent binding, the 31402 call, the TTS acknowledgement | The `/v1/voice-intent` producer (agentbox) |
| Governance | Rendering `BrokerCase`s; the case-queue surface; `broker:*` WS events | The broker decision rules (Judgment Broker); the forum `broker_cases` store |
| CTC | The `AgentActionEnvelope` schema and its typed CTC fields | Emitting the field values (agentbox) |
| Liveness | The `LivenessHarness` service and every VisionClaw canary | Other repositories' canary predicates (they register their own) |
| MR | The Godot copresence client (M1–M4, M6, copresence) | The WebXR tree (deprecated, deletion on the ADR-071 Phase 3 track) |

---

## 11. Open Issues

1. **Canary durability per item.** Standing-versus-one-shot is resolved in the PRD-023 canary table, but the exact staleness window for standing monitors (30 days vs SHA-bound) inherits the canon default and may need per-loop tuning once the harness runs.
2. **Cherry-pick reconciliation.** The `crashbug` domain kernel (936 LOC) was written against a Neo4j transport; reconciling its `DecisionOutcome` serde with the ACSP `events.rs` shapes is fixed only when the round-trip tests pass, not at design time.
3. **MR verification receipts.** Every MR closure depends on the Monado sidecar for a live receipt; the receipt format the canon accepts for a sidecar session (VNC capture vs structured log) is not yet fixed.
4. **RES-d source-of-truth split.** VisionClaw exposes the ontology class count; agentbox exposes the skill count; the canon `DriftCounter` joins them. The join's failure mode on a partial source (one repo down) is a canon-side decision.
