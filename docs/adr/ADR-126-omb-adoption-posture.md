---
title: "ADR-126 — XR/MR Interface: Open Metaverse Browser (OMB) Adoption Posture"
status: Proposed (awaiting operator ratification)
date: 2026-06-16
drives: PRD-021
companion_prds: PRD-008, PRD-019, PRD-QE-002
companion_adrs: ADR-102 (xr presence transport)
affected_repos: DreamLab-AI/VisionClaw
decision: NO-GO on full client replacement · CONDITIONAL-GO on additive service-ification + ratified-standards adoption (strangler-fig)
---

# ADR-126 — XR/MR Interface: OMB Adoption Posture

## Status
**Proposed.** NO-GO on full client replacement; CONDITIONAL-GO on additive service-ification + ratified-standards adoption via strangler-fig (PRD-021 Strategy B).

## Context
A proposal to **"completely replace VisionClaw's XR/MR interface with the Open Metaverse Browser (OMB) stack"** (omb.wiki) was investigated by a ruflo hierarchical-mesh (`swarm_1781624538033_2505ubb`) of five specialist agents (research / as-built audit / migration architecture / quality engineering / adversarial review), all of which converged independently.

Established facts:

- **OMB is a standards + architecture whitepaper, not a runtime.** It is an Open Metaverse Browser Initiative (OMBI) project under the Metaverse Standards Forum, authored by RP1 / Metaversal Corporation, whose only working browser is a **closed** prototype. The open engine, "Sneeze" (MBE, ~Blink-analogue), is `github.com/MetaversalCorp/Sneeze` — a C++ **static library** (Apache-2.0, 352 commits, 4 stars), with **no public companion application, no release, no runnable browser**. Overall stack TRL **3–4**.
- **The load-bearing OMB-specific pieces are unratified and single-author.** **RMAP** (service connectivity) and the **SOM** (Scene Object Model) ownership/security model are new RP1 proposals with no public wire spec, no second implementation, and no SDO ratification. The commodity standards beneath OMB (OpenXR, glTF, SPIR-V, DID) are TRL 9 and **already largely in our stack**.
- **Domain mismatch.** OMB targets geolocated, proximity-based, multi-fabric real-world AR (GPS/VPS anchoring; hundreds of simultaneous fabrics). VisionClaw is a single **abstract** knowledge-graph data-space (CUDA force-directed physics, 5 Hz custom binary stream). OMB's headline value props (proximity discovery, multi-fabric multiplexing, geolocation) **collapse to no-ops** for us. In OMB's own model the **browser is the universal client and VisionClaw is content / a spatial fabric** — so "replace our client with OMB" is a category error.
- **The thing we'd "replace" isn't finished.** Quest on-device validation (XR #27) and LiveKit voice were still **open** as of 2026-06-12; and the existing path carries P0 security debt (unauth ontology save/query S1, flat-privilege mutation S2, unauth settings reads S3, CPU-fallback unwired G1).
- **The seam is clean and the moat is real.** The three XR client layers (Godot/gdext native; `visionclaw-xr-presence` V3 protocol — production-grade; browser WebXR — deprecated) consume one well-defined seam (`/wss` V3 52-byte stream · `initialGraphLoad` · `/ws/presence` · NIP-98 · `/api/*`). The backend (CUDA physics, Oxigraph ontology, position pipeline, Nostr NIP-98) is the moat and **already satisfies OMB's definition of a spatial fabric**.

## Decision
1. **Reject full client replacement (R1).** There is no deployable target; the domain is mismatched; the core is single-vendor and unratified; the cost is ~30–45 eng-months and **irreversible**.
2. **Adopt a strangler-fig posture (PRD-021 Strategy B)** behind the existing, frozen seam, all work **additive** and feature-flagged:
   - **(R3) Adopt ratified standards now** — OpenXR (via Godot), glTF asset export, formalise Nostr → `did:nostr`. Low risk, reversible, mostly hygiene.
   - **(R2) Expose VisionClaw as an OMB-compatible spatial fabric** — publish an additive **RMAP manifest that *describes* the existing `/api` + `/wss` + V3 seam** and label our topology/position stream as a SOM-style data-space fabric. Backend emits identical bytes; **crown jewels untouched**.
3. **Sequence hardening first.** Close XR #27 and LiveKit voice and fix the P0 security items **before** any genesis-stage OMB phase (P4 SOM / P5 RMAP / P6 render-abstraction), which remain in **shadow / deferred**.
4. **Crown jewels are out of scope for modification:** CUDA force-directed physics, Oxigraph ontology, the V3 52-byte binary protocol and broadcast cadence, and the Nostr NIP-98 auth event. If a phase needs to touch one, the phase is wrong — wrap instead.

## Consequences
**Positive.** Open-metaverse-consumable **without betting production on vapour**; fully reversible per phase; the moat (CUDA / Oxigraph / V3 / NIP-98) preserved; engineering effort bounded to single-digit eng-months, mostly hardening that we owe anyway; the same artefacts (glTF assets, DID doc, RMAP manifest, OpenXR alignment) serve our own thin client today, so there is no speculative-only work; if RP1 abandons OMB we flip the flags off and lose nothing.

**Negative / accepted.** We are **not** a generic-OMB-browser client and forgo the "we replaced our client" narrative; we carry one new artefact (the RMAP manifest) to maintain as the spec drifts; keeping the thin client on the legacy renderer indefinitely (if Sneeze ANARI never matures) is an explicitly-accepted terminal state.

## Alternatives considered
- **A — Full replacement (R1).** *Rejected:* no runtime exists; domain mismatch; single-vendor unratified core; irreversible ~30–45 eng-months; would delete a working-ish client and two near-done features to depend on a non-existent runtime.
- **C — Do nothing OMB-specific, harden only.** *Viable for ~2 quarters* and its hardening priorities are adopted wholesale; rejected as the *sole* strategy only because one cheap additive RMAP manifest buys open-metaverse positioning for a fraction of an eng-month.

## Revisit trigger
Re-open the client-replacement (R1) decision **only when ALL FOUR hold** (AND, not OR):
1. Sneeze/MBE reaches a runnable open-source **v0.1 with a working OpenXR backend** (a real Quest binary);
2. **RMAP and SOM reach draft ratification** via Metaverse Standards Forum / OMBI (no longer single-author);
3. a **second, independent OMB browser implementation** exists;
4. that browser **renders an abstract, non-geolocated graph at visual + interaction parity** with our current client.

If (1)–(3) hold but (4) fails, **fabric-exposure (R2) is the ceiling**, not client replacement. Review annually or on any OMB GA announcement, whichever comes first.

---
*Decision derived by ruflo mesh `swarm_1781624538033_2505ubb`; independently re-derived by an adversarial reviewer. Evidence: PRD-021; RuVector namespace `omb-xr-investigation`.*
