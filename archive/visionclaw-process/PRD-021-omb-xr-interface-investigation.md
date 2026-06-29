---
title: "PRD-021 — Open Metaverse Browser (OMB) Adoption for the VisionClaw XR/MR Interface"
status: Draft — Investigation Complete, Recommendation Pending Operator Sign-off
date: 2026-06-16
authors: ruflo hierarchical-mesh (swarm_1781624538033_2505ubb) — 5 specialist agents + coordinator
drives: ADR-126
companion_prds: PRD-008 (xr-godot-replacement), PRD-019 (xr-transport-completion), PRD-QE-002 (xr-godot-quality-engineering)
method: Engineering-with-Quality — risk-based planning, strangler-fig migration, holistic-testing/PACT, adversarial verification
verdict: NO-GO on literal "complete replacement" · CONDITIONAL-GO on additive service-ification + ratified-standards adoption
---

# PRD-021 — OMB Adoption for the VisionClaw XR/MR Interface

> **One-line answer to the brief.** "Completely replace our XR/MR interface with the Open Metaverse Browser stack (omb.wiki)" is **not feasible today and not wise even when it becomes feasible** — OMB has no deployable runtime, its load-bearing pieces are unratified single-vendor proposals, and it is built for a different problem domain (geolocated multi-fabric AR) than VisionClaw (a single abstract knowledge-graph data-space). The intent underneath the brief — *participate in the open metaverse, stop being a walled garden* — is fully served by the **opposite** of replacement: keep our renderer and crown jewels, and **expose VisionClaw as an OMB-compatible spatial fabric** while adopting the *ratified* standards now. This document is the quality-engineered plan for that path.

---

## 0. How this investigation was run

A ruflo-managed hierarchical-mesh (`swarm_1781624538033_2505ubb`, 15-agent capacity, raft) of five specialist agents ran concurrently under a coordinator, each wired to the relevant skills, the codebase, the live OMB source, and RuVector memory:

| Agent | Role | Primary output | RuVector key |
|---|---|---|---|
| OMB-stack-researcher | External maturity assessment | Standards-by-layer TRL table; located the engine repo | `omb-xr-investigation/omb-stack-maturity` |
| VisionClaw-XR-auditor | As-built audit of the 3 XR layers + the seam | Component inventory, exact wire contract, coupling map | `omb-xr-investigation/visionclaw-xr-asbuilt` |
| Migration-architect | Capability-parity + strategy + phased plan | Gap matrix, 3 strategies, strangler-fig phases | `omb-xr-investigation/migration-strategy` |
| QE-planner | Engineering-with-quality spine | NFRs, gates, test strategy, risk register, CI | `omb-xr-investigation/qe-plan` |
| Adversarial-reviewer | Brutal-honesty GO/NO-GO | Premise audit, fatal-risk ranking, tripwires | `omb-xr-investigation/decision-verdict` |

All five independently converged on the same recommendation (Strategy B, below). The adversarial reviewer pushed the premise harder and reframed the verb: **the request's word "replace" is the bug; the right verbs are "expose" and "adopt."**

---

## 1. What OMB actually is (reality check)

**OMB = the Open Metaverse Browser Initiative (OMBI)**, an open project under the **Metaverse Standards Forum**, authored and driven by **RP1 / Metaversal Corporation** (who operate a *closed* commercial prototype, "the world's first metaverse browser," launched ~June 2025). The public material at `omb.wiki` is a **standards + architecture whitepaper**, explicitly "building in public — capture first, organize second."

- **The engine** is "**Sneeze**" — the Metaverse Browser Engine (MBE), positioned as analogous to Blink in Chromium. Repo: `github.com/MetaversalCorp/Sneeze` (Apache-2.0, C++, **352 commits, 4 stars, 1 open issue**, active to 2026-06-16). It builds a **static library**, not an application. The companion browser app ("Artemis") is **not public**. No release tag, no binary, 1–2 h from-source dependency build. Core systems (SOM concurrent writers, RMAP, avatars, spatial-audio codec) are partially **stubbed**.
- **The stack**, by layer and maturity (June 2026):

| Layer | Standard | Body | Status | TRL | Usable open impl? | Relevance to VisionClaw |
|---|---|---|---|---|---|---|
| XR devices | **OpenXR 1.1** | Khronos | Ratified | 9 | Yes (Meta, Monado, Godot) | **Direct** — we reach it via WebXR + Godot |
| GPU bytecode | **SPIR-V 1.6** | Khronos | Ratified | 9 | Yes | Direct (our compute path) |
| Content | **glTF 2.0** | Khronos | Ratified | 9 | Yes (three.js/Godot/Babylon) | **Direct** — easiest alignment win |
| Service logic | **WASM / Wasmtime** | W3C / BA | Ratified/prod | 8–9 | Yes | Indirect (only under R1 runtime) |
| Identity | **W3C DID v1.0 / VC 2.0 / FIDO2** | W3C / DIF / FIDO | Ratified | 8–9 | Yes | **We exceed it** — Nostr NIP-98 = `did:nostr` |
| Render abstraction | **ANARI 1.0** | Khronos | Ratified | 6–7 | Yes (OSPRay/VisRTX); Halogen (RP1) TRL 4 | Indirect — only under R1 |
| Positioning | GeoPose 1.0 (+ proposed abstraction) | OGC | Ratified / **proposed** | 6–7 / 2 | Partial | **N/A** — abstract graph has no geolocation |
| Scene model | **SOM** (Scene Object Model) | OMBI (RP1) | **Proposed, single-author** | 3 | No (header stubs only) | Conceptually aligned, not adoptable |
| Service connectivity | **RMAP** (Remote Model Access Protocol) | RP1 → intended SDO | **Proposed, single-author, no public wire spec** | 3 | No | The one net-new artefact worth shaping |
| Spatial audio | custom 24 kHz codec | OMBI (RP1) | **Proposed, new** | 2 | No | Better target than our pending LiveKit, later |

**Overall OMB-stack TRL: 3–4.** The commodity *standards* underneath it (OpenXR, glTF, SPIR-V, DID) are TRL 9 and already largely in our stack. The OMB-*specific* glue (Sneeze MBE, RMAP, SOM, positioning abstraction, audio codec) is genesis-stage and single-vendor.

**Critical category fact.** In OMB's own model, **the metaverse browser is the universal client and a world/service is the content.** OMB is engineered for *geolocated, proximity-based, multi-fabric real-world AR* (airports, hospitals, retail; GPS/VPS anchoring; hundreds of simultaneous fabrics). VisionClaw is a *single abstract data-space* — a force-directed knowledge graph of gems/orbs/capsules driven by CUDA physics over a 5 Hz custom binary stream. OMB's three headline value props (proximity discovery, multi-fabric multiplexing, GPS/VPS anchoring) **collapse to no-ops or to things we already have for free** in an abstract single-application graph. Therefore VisionClaw is, in OMB terms, a **spatial fabric / content source** — not a candidate to *be* a browser.

---

## 2. What we have today (as-built)

The "XR/MR interface" is **three layers consuming one clean seam**, plus two sidecars. The backend is untouched by any of it.

### 2.1 The three client layers

| Layer | Tech | Maturity | Notes |
|---|---|---|---|
| **L1 — `xr-client/`** native Quest 3 app | Godot 4.3 + Rust gdext (`visionclaw_xr_gdext`) | **Code-complete vertical slice; NOT device-validated** | 5 gdext classes (`binary_protocol.rs` 710 / `presence.rs` 918 / `transport.rs` 417 / `signer.rs` 289 / `interaction.rs` 331 / `lod.rs` 309 / `webrtc_audio.rs` 345), real tokio-tungstenite sockets, real BIP-340 signer, 48 tests. Renders typed nodes (cap 4000) + edges (cap 3000), presence avatars, ray/pinch grab, NIP-98-gated drag-pin, haptics, HUD, unbounded reconnect. **Open:** Quest on-device validation (#27 — only ever run headless / on the Monado desktop sidecar, which exercises *core OpenXR only*, no Quest vendor extensions); LiveKit AAR voice (routing maths exist, media transport absent). |
| **L2 — `crates/visionclaw-xr-presence/`** | Rust (bytes/serde/proptest/cargo-fuzz) | **PRODUCTION-GRADE — KEEP** | The shared multiplayer contract, imported by **both** server and client. `wire.rs` (0x43 pose), `room.rs` (one-DID-one-avatar invariant), `delta.rs` (per-slot delta), `validate.rs` (velocity/bounds/quat/monotonic/NaN gates). Property + adversarial + fuzz + bench. This is the most mature artefact in the whole XR surface. |
| **L3 — `client/src/features/visualisation/WebXRScene.tsx` + `client/src/immersive/`** browser WebXR | React + @react-three/xr + three.js | **ASPIRATIONAL / DEPRECATED** | Renders only the agent-action viz in VR; `useImmersiveData.ts` pin/drag/position writes are explicit **TODO no-ops ("Phase 5")** that never landed. Lowest-value layer to preserve. |

Sidecars: `agentbox/xr-runtime/` (Monado + Godot desktop OpenXR test rig — useful CI/dev infra, core-OpenXR only); `vircadia-world/` (**effectively empty** — one stray `AI-SHACL.ttl`, zero runtime code — greenfield, not an existing integration).

### 2.2 The seam (the frozen interface any client — ours or OMB's — must speak)

This is the single contact surface between clients and the backend crown jewels. It is **clean**: no backend code imports any client.

- **`/wss`** binary position stream — version byte `0x03` + N × **52-byte** little-endian records: `id:u32(+flag bits)` · `pos[3]:f32` · `vel[3]:f32` · `sssp_distance:f32` · `sssp_parent:i32` · `cluster_id:u32` · `anomaly:f32` · `community_id:u32` · `centrality:f32`. ~5 Hz, full-snapshot (delta forbidden by PRD-007 §3; late-joiners need full state), rate-limited 200 ms. Node-type flag bits: `0x80000000` agent / `0x40000000` knowledge / `0x1C000000` ontology-mask; `NODE_ID_MASK=0x03FFFFFF`.
- **`initialGraphLoad`** text frame — `{nodes, edges:[{source_id,target_id,weight}], timestamp}`, snake_case, ids carry flag bits.
- **`/ws/presence`** — JSON challenge/auth/joined handshake, then 0x43 binary pose (outbound single-pose) / sibling-broadcast (inbound) per `visionclaw-xr-presence`.
- **NIP-98 auth** — `{type:"authenticate", event:<base64 kind-27235 event>}`, BIP-340 Schnorr over the signed URL; unlocks server-authoritative mutations.
- **REST** — `/api/graph/data?graph_type=knowledge|ontology|agent`, `/api/settings/*`.

### 2.3 Crown jewels (out of scope for modification — the moat)

Rust graph domain · **CUDA force-directed physics** (`visionclaw-gpu`) · **Oxigraph ontology** (`visionclaw-ontology`) · position-broadcast pipeline (`force_compute_actor` → `broadcast_optimizer`) · **Nostr NIP-98 auth**. These are *more evolved than anything OMB ships* — our GPU layout already satisfies OMB's own definition of a "spatial fabric" (a server-side mapped 3D coordinate space with presence). Do not genesis-ify them by chasing OMB-native rewrites.

---

## 3. Capability-parity gap analysis

Columns: does the OMB stack provide it · maturity of that piece · gap · what we'd build.

| VisionClaw XR capability | OMB component | Maturity | Gap | What we build |
|---|---|---|---|---|
| Node render (gem/orb/capsule) | glTF + SOM branch | glTF commodity / SOM genesis | **small** | Export 3 geometries to a glTF asset library + binding table |
| Edge render | SOM topology (no first-class edge) | genesis | **small–large** | Edges are *our* domain concept; encode as SOM child nodes/attributes |
| **GPU-streamed positions (V3 52 B, 5 Hz)** | **SOM streaming + RMAP channel** | genesis | **large** | **Wrap, don't replace:** publish an RMAP model *describing* the V3 channel; backend emits identical bytes (see §3.1) |
| Presence (spatial state) | OMB presence | concept ratified, no impl | **small** | We're *ahead* — reshape `visionclaw-xr-presence` to OMB presence semantics |
| Avatars | parametric kB avatars | proposed | **small–med** | Adopt parametric descriptor; wire-format change, not a capability gap |
| Grab/ray + drag-pin | OpenXR input + SOM branch ownership | OpenXR commodity / ownership genesis | **medium** | Map drag-pin to SOM per-branch ownership; logic stays in actors |
| Analytics overlays (centrality/community/anomaly/SSSP) | none — domain-specific | n/a | **ours to keep** | Carried as opaque SOM per-node metadata; **moat, not gap** |
| HUD | none (MBE is UI-agnostic) | n/a | **ours** | Stays client-side |
| Voice | server-side spatial audio | proposed | **medium** | Defer; OMB's server-side mix is a *better* target than client LiveKit |
| Haptics | OpenXR haptics | commodity | **none** | Already aligned |
| **Auth (Nostr NIP-98)** | DID / FIDO | ratified | **we're ahead** | `did:nostr`; publish a DID doc, keep NIP-98 |
| Settings control (`/api/settings/*`) | RMAP service model | genesis | **large** | Describe as RMAP service methods; backend untouched |
| **The coordinate space itself** | spatial fabric | concept ratified, no runtime | **none conceptually** | We already *satisfy the definition*; gap is the published interface |

### 3.1 The hard mapping — GPU-streamed 5 Hz binary → SOM streaming + RMAP

The trap is rewriting the 52-byte binary into a SOM-native wire format. **Don't.** Our single position stream *is* a single-source SOM branch mutated every frame. **Wrap, don't replace:** publish an **RMAP model definition that describes the V3 channel** — stride (52 B), field layout, cadence, full-snapshot semantics. A future OMB browser reads the *model*, then consumes the *same bytes off the same socket*. The binary protocol becomes a *described* transport rather than a *replaced* one. **Backend emits identical frames; zero crown-jewel change.** "Proximity" for this stream = virtual proximity (graph-distance / cluster region) inside the abstract coordinate space, **not** GPS.

---

## 4. Strategy options & recommendation

| | **A — Full replacement now (R1)** | **B — Strangler-fig: expose + adopt, track Sneeze (R2+R3)** | **C — Track / defer only** |
|---|---|---|---|
| Scope | Retire Godot + WebXR clients; a generic OMB browser renders us | Behind the existing seam, emit OMB-shaped artefacts; keep a thin client; track Sneeze for eventual optional R1 | Spikes only |
| Cost | ~30–45 eng-months **+ wait for a runtime that doesn't exist** | **~6–9 eng-months**, P6 deferred indefinitely | ~1–2 eng-months |
| Risk | **Extreme** — depends on non-existent runtime (TRL 3–4), single point of total failure | **Low–moderate, bounded per phase** | Very low, but low value |
| Reversibility | **None** once clients retired | **Full, per phase** (feature-flagged) | Total |
| Backend impact | High (conform renderer to ANARI) | **Zero to crown jewels** (additive descriptors only) | Zero |

**Recommendation: Strategy B.** We already own three of OMB's hardest, most genesis-stage pieces — a server-side mapped 3D coordinate space with presence (= a spatial fabric, via GPU physics), a working presence/avatar wire protocol, and DID-grade identity (Nostr NIP-98, which *exceeds* the OMB DID baseline). Strategy A throws away working renderers to chase a runtime that doesn't exist. Strategy C under-uses a strong hand. **Strategy B's deliverables are Strategy C's hardening plus one published manifest** — the same road, one stop further — and every artefact *also* serves our own thin client today, so there is no speculative-only work.

---

## 5. Phased migration plan (Strategy B) — strangler-fig / branch-by-abstraction

**Framing.** The existing seam (`visionclaw-protocol` V3 · `/api/*` · `/wss`) is the abstraction boundary. Every phase adds an OMB-shaped facade *behind that seam*, flag-gated, defaulting OFF. The legacy path stays authoritative until a phase passes all its gates. Nothing is removed until its replacement has carried production traffic.

> **CROWN-JEWEL GUARDRAIL (absolute).** No change to the 52-byte wire, the broadcast cadence, the CUDA kernels, Oxigraph, or the NIP-98 event. **All migration work is ADDITIVE.** If a phase needs to touch a crown jewel, the phase is wrong — wrap instead.

Phase order: **presence → assets → identity → SOM → RMAP → render-abstraction.**

| Phase | Ships | Flag (default off) | OMB-maturity dependency |
|---|---|---|---|
| **P0 — RMAP manifest spike** | A *descriptive-only* RMAP model document of the existing seam. No code-path change. Validated against current behaviour. | — (doc) | none |
| **P1 — Presence/avatars** | Reshape `visionclaw-xr-presence` to OMB presence semantics + parametric-avatar descriptor; dual-emit. | `omb.presence.v1` | concept ratified |
| **P2 — Assets (glTF)** | Export gem/orb/capsule to a glTF asset library + node→asset binding; client renders glTF or procedural. | `omb.asset.gltf` | glTF ratified |
| **P3 — Identity (DID)** | Publish a `did:nostr` DID document; formalise pubkey↔DID. **NIP-98 unchanged.** | `omb.identity.did=bridge` | DID/FIDO ratified |
| **P4 — SOM-branch formalisation** | Label topology + position stream as a SOM data-space fabric (single branch, ownership = drag-pin, virtual-proximity regions); **shadow mode**. | `omb.transport.som=shadow` | SOM genesis — stay in shadow |
| **P5 — RMAP service wiring** | Promote P0's descriptive manifest to a live RMAP service model; discovery endpoint additive; **52 B stream unchanged**; shadow. | `omb.transport.rmap=shadow` | RMAP genesis — stay in shadow |
| **P6 — Render-abstraction / optional R1 (DEFERRED)** | ANARI/SPIR-V; consumption by a generic OMB browser. **Tracking spike only** until tripwires (§7) hold. | `omb.render.anari` / `omb.r1.browser` | **blocks here** — Sneeze must exist |

P0–P5 are decoupled from OMB maturity (they emit *artefacts*, not *runtime dependence*) and can ship entirely against our own thin client. **P6 is the only phase that hard-depends on an external runtime** and stays a flagged spike. If OMB stalls or RP1 abandons it, we flip the flags off and have lost nothing — we keep glTF assets, a DID doc, OpenXR alignment, and a clean published seam, all of which stand alone.

---

## 6. Engineering-with-quality spine

The single highest-leverage QE asset is the **seam contract**: because OMB is pre-production, we anchor regression to the *frozen seam*, not the new engine. Everything new is validated *against* byte-exact golden fixtures of the V3 layout before it is allowed to replace anything. House standard applies: validate end-to-end in the **browsercontainer GPU sidecar** (visuals + live data streams), **never** assert success from HTTP 200.

### 6.1 NFRs (gate-blocking floors in bold)

| NFR | Target | Floor |
|---|---|---|
| Quest 3 fps | 90 | **72 sustained** (<1% frames below over 60 s) |
| Desktop fps | 60 | 60, p99 frame ≤ 16.6 ms |
| Motion-to-photon | ≤ 20 ms | ≤ 25 ms p95 |
| Position-stream e2e latency | ≤ 120 ms p50 | **≤ 200 ms p95** |
| Bandwidth/client | ≤ 100 KB/s | **≤ 150 KB/s** (≤ 2 MB connect burst) |
| Cold-start to first live frame | ≤ 2 s | **≤ 3 s p95** |
| Concurrent presence peers | ≥ 64 design | **≥ 16 load-tested green** before any presence cutover |
| Quest heap | ≤ 384 MB | **≤ 512 MB** (no OOM) |
| Accessibility (2D surfaces) | WCAG 2.2 **AA** | no Level A violations |
| WASM service sandbox | WASI capability-based, no ambient FS/net | no un-sandboxed service ships |
| Endpoint auth | **NIP-98 on every endpoint** | zero unauth mutating routes; `SETTINGS_AUTH_BYPASS` removed |
| Render correctness | populated graph (not blank/collapsed/single-sphere) | visual-diff SSIM ≥ 0.95 |

### 6.2 Quality gates (entry → exit, green only in the real-browser+device harness)

- **P0 Seam baseline (blocks everything):** byte-exact golden fixtures for the V3 52 B layout, `initialGraphLoad` schema, `/api/graph/data` shape; replay harness renders a live frame from golden identically; old client passes 100% contract tests.
- **P1 Presence parity:** wire round-trips identically; N7 ≥ 16 peers; N3 on Quest; visual baseline match; reconnect/half-open chaos green.
- **P2 glTF parity:** Khronos glTF-validator 100% clean; geometry visual parity; no bandwidth regression.
- **P3 DID parity:** DID resolution conformance; FIDO E2E; NIP-98↔DID bridge preserves auth on **every** endpoint.
- **P4 SOM (shadow):** SOM stream frame-equivalent to V3 (diffed against golden); latency within budget; 24 h shadow-vs-primary divergence below threshold. **`=on` forbidden until ratified.**
- **P5 RMAP (shadow):** contract conformance; abstract-graph↔GeoPose mapping documented + visual parity; shadow soak clean.
- **P6 Render-abstraction:** ANARI visual parity across the device matrix; SPIR-V conformance; fps floors held. **Deferred indefinitely if Sneeze ANARI is immature — keeping the thin client on the legacy renderer is an explicitly-acceptable terminal state.**

Gate dependency: P0 → P1 → P2 → P3 → {P4 → P5}(shadow) → P6. P4–P6 may park indefinitely without blocking shipping.

### 6.3 Test strategy

Unit (Rust crates / gdext / TS — lift client coverage off the **5.6%** baseline to ≥ 60% on changed code; close `139 unwrap()` exposure). **Contract tests** = the strangler regression guard: byte-exact V3 fixtures + `initialGraphLoad` schema + `/api/graph/data` shape, run against **both** clients. Integration (seam↔adapter, NIP-98↔DID bridge, CUDA-context-on-spawn-blocking guard). E2E in the GPU sidecar (visuals + live streams). **Device matrix** (Quest 3 / phone-AR / desktop / PCVR) — gate-blocking before each cutover. **Visual regression** (SSIM ≥ 0.95, catches the recurring blank/collapsed/single-sphere mode). **Performance/load** including **settle-time** (connect a client *after* convergence + the 600-frame warmup, assert populated positions within N6 — the periodic-full-broadcast guard). **Conformance** (glTF-validator, OpenXR CTS subset, DID resolution, SPIR-V, SOM/RMAP shadow contracts). **Chaos** targeting known bug classes: reconnect/half-open socket; GPU circuit-breaker benign-skip conflation (the "froze layout to a static sphere" regression must be a *standing* test); `unified_compute` mutex lock-starvation deadlock.

### 6.4 Risk register (L×I, 1–5)

| ID | Risk | L | I | Score | Mitigation |
|---|---|---|---|---|---|
| **R1** | **OMB pre-implementation — RMAP/SOM unratified, Sneeze may never ship** | 4 | 5 | **20** | Strangler behind the stable seam; shadow-only until ratified; legacy stays shippable; track quarterly; P4–P6 may park forever |
| R3 | Quest perf regression (< 72 fps) | 4 | 5 | 20 | Device-matrix perf gate blocks every cutover; perf-budget CI; frame-budget SLO |
| R5 | Abstract-graph vs geo-AR model mismatch | 4 | 4 | 16 | Documented coordinate mapping (P5); abstract default, geo additive |
| R2 | Device fragmentation | 4 | 4 | 16 | Device-matrix CI gate-blocking; per-device baselines |
| R6 | Team capacity (Rust+gdext+TS+new standards) | 4 | 4 | 16 | Phase scoping; shadow mode; defer P4–P6 |
| R4 | Presence-protocol breaking change | 3 | 5 | 15 | Byte-exact V3 golden fixtures; both clients conform; change fails build |
| R7 | RP1 abandons OMB/Sneeze | 3 | 5 | 15 | No hard runtime dependency; adopt only ratified standards; seam isolates |
| R8 | Standard churn | 4 | 3 | 12 | Pin versions; conformance CI; adapters absorb change behind seam |
| R-stream | Broadcast-cadence/node-settling regression | 3 | 4 | 12 | Standing settle-time test + periodic-full-broadcast assertion |
| R-gpu | Circuit-breaker conflation / lock-starvation recurs | 3 | 4 | 12 | Standing chaos tests for benign-skip + clustering deadlock |
| R-sec | Auth bypass / un-sandboxed WASM slips through | 2 | 5 | 10 | NIP-98-on-every-endpoint gate; WASM capability audit |

### 6.5 Rollback & CI/CD

Every phase reversible; legacy is the shipping default until a phase passes all gates. Flags are server- and client-evaluated and cohort-targetable; SOM/RMAP cut over only after a **≥ 24 h shadow soak** with divergence below threshold; automatic rollback on any NFR breach, visual-diff < 0.95, reconnect spike, or circuit-breaker DEGRADED latch. New gate-blocking CI jobs: `contract-conformance`, `gltf-validate`, `visual-diff`, `device-matrix`, `perf-budget`, `chaos-resilience`, `conformance-standards`, `auth-coverage`, `wasm-sandbox-audit`.

---

## 7. Decision & tripwires (drives ADR-126)

**Verdict: NO-GO on the literal "complete replacement"; CONDITIONAL-GO on the salvaged intent (Strategy B).**

Sequence ahead of any OMB-specific work, the existing open debt that can actually hurt us: **close XR #27 (Quest on-device validation) and LiveKit voice; fix the P0 security items** (unauth ontology save/query S1, flat-privilege mutation S2, unauth settings reads S3, CPU-fallback unwired G1). Then do P0–P3 (cheap, ratified, reversible). Hold P4–P6 in shadow/deferred.

**Re-open the client-replacement (R1) decision only when ALL FOUR hold (AND, not OR):**
1. Sneeze/MBE reaches a runnable open-source **v0.1 with a working OpenXR backend** (a real Quest binary, not a design doc);
2. **RMAP and SOM reach draft ratification** through Metaverse Standards Forum / OMBI governance (no longer single-author);
3. a **second, independent OMB browser implementation** exists (bus-factor > 1);
4. that browser **renders an abstract, non-geolocated graph at visual + interaction parity** with our current client.

If (1)–(3) hold but (4) fails, OMB consuming us as a **fabric (R2)** is the ceiling — not client replacement. Review annually or on any OMB GA announcement, whichever first.

---

## 8. Success metrics

- M1 — Zero modifications to crown jewels across P0–P5 (audit: `git log` touches only additive paths).
- M2 — Contract-conformance CI green on `main` for the full migration (byte-exact V3 fidelity never regresses).
- M3 — XR #27 + voice closed and P0 security items fixed **before** P4 starts.
- M4 — A published RMAP manifest + glTF asset library + `did:nostr` document exist and validate, with the legacy client still default-on.
- M5 — Every phase demonstrably reversible (flag-off returns byte-identical behaviour to pre-phase golden).

---

## 9. References

- omb.wiki `/standards` (Open Standards for the Metaverse), `/sneeze` (MBE Architecture, Appendices A1–A4, B1–B7), `/w3cdeck` (PDF: `cdn.rp1.com/decks/MSF-Open-Metaverse-Browser-Initiative-W3C.pdf`).
- `github.com/MetaversalCorp/Sneeze` (engine, Apache-2.0); companion repos Halogen (ANARI+Filament), Vox (SPIR-V compute), MSF_Map_Svc/Db.
- Metaverse Standards Forum — OMBI announcement; BusinessWire (RP1 metaverse-browser launch, 2025-06).
- Khronos: OpenXR 1.1, glTF 2.0, ANARI 1.0, SPIR-V 1.6. W3C DID v1.0 / VC 2.0. FIDO2. OGC GeoPose. OMI Group glTF extensions.
- Internal: PRD-008, PRD-019, PRD-QE-002, ddd-xr-godot-context, xr-godot-threat-model; RuVector namespace `omb-xr-investigation` (5 mesh keys) + `patterns/visionclaw-production-hardening-2026-05-28` (S1/S2/S3, G1).

---
*Produced by ruflo mesh `swarm_1781624538033_2505ubb` (raft, hierarchical-mesh). Planning only — no code modified. Findings cross-verified by an adversarial reviewer that independently re-derived the verdict.*
