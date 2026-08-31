# ADR-139: Immersive Interaction Adoption Programme — Graph2VR-class Feature Mining

**Status:** Accepted
**Date:** 2026-08-30
**Deciders:** VisionClaw XR/immersive lead, CUDA/physics specialist, platform team
**Related:**
- ADR-071 (Godot + godot-rust + OpenXR native client — the client these features land in)
- ADR-102 (XR client ↔ backend transport — Graph V3 wire the expansion/fold/query reads ride)
- ADR-136 (desktop OpenXR / VIVE Pro validation target — the headset the wave-1 work rendered on)
- ADR-137 (XR render offload, runtime quality dials, full-3D-default — `RenderStore` is the fold application layer)
- ADR-138 (GPU force-channel registry — the pinned bitmask + DAG radial bias mined here land through it)
- PRD-018 / ADR-098 (ontology forces — the Whelk/Oxigraph inference output the asserted/inferred channel reads)
- PRD — Visual Query Builder with Semantic Planes (`docs/prd/prd-visual-query-builder-semantic-planes.md`)
- PRD — Fold-Level Ladder (`docs/prd/PRD-fold-ladder-hierarchical-density.md`)

## TL;DR

Over 2026-08-30 the immersive-interaction programme mined five external
linked-data / graph-visualisation tools for interaction ideas and adopted a
convergent, deduplicated feature set into the Godot/OpenXR client and the GPU
layout engine. This ADR is the **provenance and governance record** for that
mining: what each source is, under which licence it was studied, what was
adopted from it, and — for the one source-available tool with a restrictive
licence (OntoAir) — the **clean-room protocol** under which it was studied
(functional spec only, zero code or asset reuse).

The mining is *ideas-level*, not code adoption: no external source's code or
assets were copied into the tree. Every adopted feature was independently
implemented against VisionClaw's own `RenderStore`, CUDA kernels, and Graph V3
wire. Two sources (Graph2VR LGPL, OntoAir source-available-restrictive) are
copyleft/restrictive and are therefore recorded as **ideas-only / clean-room**;
the MIT and Apache-2.0 sources permitted deeper structural study but were still
re-implemented, not vendored.

## Context

The knowledge graph is ~13k nodes / ~145k edges over a 5,975-class ontology.
The immersive client (ADR-071) had reached on-headset render (ADR-136) but its
interaction vocabulary was thin: grab, locomotion, a world-anchored HUD. The
mature VR/graph tooling ecosystem has spent years solving room-scale linked-data
interaction; rather than reinvent, the programme surveyed the field, mined the
best-of-breed interaction patterns, and adopted the convergent ones — the ideas
that showed up independently across multiple tools, which is the strongest
signal that a pattern is right rather than incidental.

Licences across the surveyed tools vary from permissive (MIT, Apache-2.0)
through weak copyleft (LGPL-3) to non-OSI source-available ("individual research
use only"). Adopting *ideas* from any of these is lawful; copying code is not,
and from the restrictive source even reading-then-reproducing needs a disciplined
firewall. This ADR records the posture taken for each so the provenance is
auditable.

## The five mined sources

| # | Source | Author / stack | Licence | Study depth | Mining record |
|---|--------|----------------|---------|-------------|---------------|
| 1 | **Graph2VR** | molgenis · Unity/C# | **LGPL-3** (weak copyleft) | Ideas-only | `graph2vr-feature-mining-2026-08-30` |
| 2 | **3d-force-graph** | vasturiano · Three.js | **MIT** | Full-depth (permissive) | `3d-force-graph-mining-2026-08-30` |
| 3 | **GraphDBViewerWeb** | web RDF/graph viewer | permissive web viewer | Reference / shortlist | (surveyed alongside the RDF-viewer set) |
| 4 | **OntoAir** | tykimos · macOS QuickLook ontology viewer | **Source-available, restrictive** ("Individual Research Use Only" — not OSI) | **CLEAN-ROOM — functional-spec-only, no derivation** | `ontoair-cleanroom-spec-2026-08-30` |
| 5 | **Ontosphere** | ThHanke / Fraunhofer IWM · React + Reactodia + Konclude-WASM | **Apache-2.0** | Full-depth (permissive) | `ontosphere-mining-2026-08-30` |

Graph2VR is the anchor reference: the peer-reviewed VR linked-data browser
(Kellmann et al., *Database* (Oxford) 2025, DOI 10.1093/database/baaf008) whose
"in-place query variables → live count → materialised result subgraphs on
stacked planes" concept is the origin of the flagship visual query builder.

## Adoption decisions

Each adopted feature is traced to its mined source(s). Where two sources
converged on the same idea, both are cited — convergence was the selection
criterion.

| Adopted feature | Source(s) | Where it landed |
|-----------------|-----------|-----------------|
| **Predicate-count-first expansion** — radial menu lists predicates with result counts *before* fan-out; expansion bounded by a top-k limit | Graph2VR | `GET /api/graph/node/{id}/relations` (predicate counts) + `POST /api/graph/node/{id}/expand` (bounded top-k) — `src/handlers/api_handler/graph/mod.rs`; RadialMenu client wiring |
| **Two-hand pinch manipulation** — both grips scale/rotate/translate the whole graph about its bounding-sphere centroid | Graph2VR (`SphereInteraction.cs` concept) | `xr-client/scripts/graph_scene.gd` `_update_two_hand_manip` (commit f7113226a) |
| **Node pinning `fx/fy/fz`** — wand grab-and-place pins a node; the integrator early-outs on pinned nodes but they still exert forces | 3d-force-graph | Per-node pinned bitmask in the CUDA integrator; drag-end persistent pin; `nodeUnpin` wire message (ADR-138, commit 82abb6776) |
| **DAG radial hierarchy layout** — per-node rank, radial-out bias so the user sits at the root | 3d-force-graph (`dagMode` radialout) | `dag_radial_bias` shell force in both force kernels; `dagBiasK` / `dagLevelDistance` settings (ADR-138) |
| **Radial menus** — arc-wedge circle menu with overflow-slider paging, latch-safe close, submenu state machine | Graph2VR (CircleMenu) | `xr-client/scripts/radial_menu.gd` + `scenes/RadialMenu.tscn` (commit 92b2d9588) |
| **Visual query builder** — mark nodes/edges as `?variables` in-scene, live binding-count preview, one result-subgraph per binding on stacked semantic planes | Graph2VR (origin), Ontosphere (T-Box/A-Box dual-plane) | `POST /api/graph/query/pattern` (server-side match over the live typed graph); `query_builder.gd` + `graph_scene.gd` marking |
| **Fold-level ladder L1→L2→L3→∅** — discrete, steppable density management: hide low-signal → fold subclass chains → fold communities | Ontosphere (discrete fold levels), OntoAir (barycenter/layered fold synergy) | `GET /api/graph/fold` fold-plan endpoint (`src/handlers/api_handler/graph/fold.rs`); `RenderStore` id→representative remap |
| **Asserted vs inferred channel** — solid/normal edges for asserted axioms, amber-dashed + amber-italic for reasoner-entailed | Ontosphere | Wires to VisionClaw's own Whelk/Oxigraph inference output (PRD-018); joins the existing relation-type edge-style table |
| **Force-channel registry** (enabling seam) — named, enumerable force channels replacing ad-hoc scalars | 3d-force-graph (`d3Force(name,fn)` named-force idea) | `src/models/force_channels.rs` (ADR-138) |

Deliberately **not** adopted (ours already exceeds the reference, or off-target):
Graph2VR's physics, labels, and search (VisionClaw's are faster — 13k@90fps vs
their ~17k@14fps ceiling); OntoAir's data-properties-as-callouts and
graph↔source bidirectional highlighting (noted as differentiation space, desktop
candidates, not this wave); the reasoner/repair engines of Ontosphere and the
Dagre/ELK layout stacks (table stakes we solve on GPU).

## Clean-room governance for OntoAir

OntoAir is **source-available under a restrictive, non-OSI licence** —
"Individual Research Use Only", with organisational, educational, and commercial
use excluded. That licence permits an individual to *study* the tool but does
**not** grant the right to reuse its code or assets in a product such as
VisionClaw. Ideas and functional behaviour are not copyrightable; a specific
implementation is. The firewall between the two must be explicit and recorded.

Protocol applied, and binding on any future OntoAir-derived work:

1. **Functional-spec-only capture.** The mining record
   (`ontoair-cleanroom-spec-2026-08-30`) describes *what the tool does* — the
   observable behaviour and interaction affordances — and contains **no code, no
   derivation, no copied assets**. It is a specification, not a transcription.
2. **Independent implementation.** Every OntoAir-influenced adoption is built
   from that functional spec against VisionClaw's own code, by contributors
   working from the spec, never from OntoAir's source.
3. **No code or asset reuse, ever** — regardless of how small. The restrictive
   licence makes any copied fragment a licence violation.
4. **Auditable provenance.** This ADR plus the tagged memory record are the
   standing evidence that the OntoAir influence is spec-level only. Any future
   feature citing OntoAir must cite this governance section.

The permissive sources (3d-force-graph MIT, Ontosphere Apache-2.0) did not
require this firewall for lawful reuse, but were still re-implemented rather than
vendored, keeping the whole programme's provenance uniformly clean. Graph2VR's
weak-copyleft LGPL-3 was treated as ideas-only for the same reason.

## Consequences

**Positive.**
- The client's interaction vocabulary jumps from grab+locomotion to a full
  linked-data toolkit (expansion, manipulation, pinning, hierarchy layout,
  radial menus, query building, density folding) in three commits plus the
  flagship working tree, every feature validated against a real headset path.
- Convergent selection means the adopted set is the field's best-of-breed, not
  one tool's idiosyncrasies.
- Provenance is fully auditable: five tagged mining records, per-source licence
  posture, and a written clean-room firewall for the one restrictive source.
- The server-side work (expansion, fold, pattern-match endpoints) is
  client-agnostic and transfers to the desktop React/Three client against an
  identical wire contract.

**Costs / risks.**
- The clean-room discipline is a standing obligation, not a one-off: future
  OntoAir-derived work must re-affirm it.
- The flagship query builder and fold ladder are multi-phase; only phase-1
  server surfaces are shipped at the time of this record (see the two PRDs). The
  ADR documents the *programme and its governance*, not a finished feature set.
- Adopted-but-deferred ideas (data-property callouts, graph↔source highlighting)
  create a differentiation backlog that should be tracked, not lost.

## References

- Mining records (RuVector, namespace `patterns`): `graph2vr-feature-mining-2026-08-30`,
  `3d-force-graph-mining-2026-08-30`, `ontoair-cleanroom-spec-2026-08-30`,
  `ontosphere-mining-2026-08-30`.
- Kellmann et al., "Graph2VR", *Database* (Oxford) 2025, DOI 10.1093/database/baaf008.
- Commits `f7113226a` (pinch manipulation), `82abb6776` (pins / DAG / force channels, ADR-138),
  `92b2d9588` (wave-1 expansion API, radial menu, HUD controls).
- Working tree: `src/handlers/api_handler/graph/fold.rs`, `POST /api/graph/query/pattern`
  in `src/handlers/api_handler/graph/mod.rs`, `xr-client/scripts/query_builder.gd`.
