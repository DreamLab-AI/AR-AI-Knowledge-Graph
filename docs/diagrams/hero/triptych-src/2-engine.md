# Panel 2 — THE ENGINE

**Subtitle:** Inside VisionClaw — semantic knowledge, GPU/XR, pervasive ontology augmentation, governed agents.

## What this panel says
The middle panel is the architecture. A formal ontology sits at the centre; everything
else attracts to it. The NEW pervasive augmentation lets every agentbox AI call query
that ontology. Agents act, humans govern, the GPU renders it all in immersive XR.

## Core stack (drawn as stacked / nested layers)
- **Oxigraph RDF store + Whelk-rs OWL-EL reasoning** — ~5,975 ontology classes,
  ~123k triples, materialised inference, EL++ consistency checking. The semantic core.
- **GPU physics** — 92 CUDA kernels, force-directed layout where `subClassOf` → attraction,
  `disjointWith` → repulsion. 55× vs CPU.
- **Godot-native XR client** — Godot 4 + godot-rust + OpenXR Quest 3 (NOT Babylon, NOT Vircadia),
  multi-user presence, live V3 binary wire.

## The NEW capability — pervasive ontology augmentation (PRD-020)
- **One retrieval brain** (`@agentbox/ontology-retrieval`, shared in-process library — not a service).
- **PUSH channel** — per-turn hook breadcrumb, ≤80 tokens, synchronous, no network.
- **PULL channel** — `ontology_ask` MCP tool + consultant seam, budget-bounded Turtle.
- **Offline Haiku condensation mesh under a Sonnet lead** compresses the corpus into Class Summaries.
- Read pervasive, **write governed** — only `propose → Whelk gate → human merge` asserts truth.

## Governance + agentic mesh
- **ACSP producer** — agents publish decision panels (Nostr kinds 31400–31405);
  humans approve/reject with signed events. Judgment Broker for the 10% that need a human.
- **Embodied agent loop** — agent actions render as coloured beams in the graph (gluon attractive force deferred; see ADR-059 addendum).

## EXACT LABELS TO RENDER
- Panel title: **THE ENGINE**
- Centre node (teal): **Oxigraph KG + Whelk-rs OWL-EL · ~5,975 classes**
- Around it: **GPU physics — 92 CUDA kernels, semantic forces**;
  **Godot-native XR client (OpenXR Quest 3)**;
  **ACSP governance — Nostr 31400–31405, human sign-off**
- The new binding (burnt-orange highlight box): **PERVASIVE ONTOLOGY AUGMENTATION (PRD-020)**
  with sub-labels **PUSH: per-turn hook ≤80 tok** · **PULL: ontology_ask MCP tool** ·
  **one retrieval brain** · **read pervasive / write governed (Whelk + human merge)**
- Agentic mesh label: **agents act → beams render → humans govern**
- Footer: **every AI call gains the option to query the formal ontology**

## Aesthetic notes
Hand-drawn nested-layer / hub-and-spoke diagram. Ontology core in teal at centre.
The augmentation binding (PUSH/PULL arrows into the brain) is the burnt-orange action element.
Render actual arrows labelled PUSH and PULL. No stock AI brain, no neon, no glowing cubes.
