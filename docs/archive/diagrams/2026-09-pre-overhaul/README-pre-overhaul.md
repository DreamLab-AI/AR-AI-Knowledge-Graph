# VisionClaw Diagrams

Hero diagrams for the project README and docs. Two-step workflow:

1. **Diagram-as-code** — structural precursor in Mermaid, rendered via `mmdc`. Source of truth for layout, labels, relationships.
2. **Nano Banana Pro upgrade** — precursor PNG passed as `--reference-image` to Gemini 3 Pro Image (`gemini-3-pro-image-preview`) with the VisionClaw aesthetic prompt, producing a publication-quality 2K image in the project's brand style.

## Layout

| File | Purpose |
|------|---------|
| `src/*.mmd` | Mermaid source (edit here) |
| `src/visionclaw-theme.json` | Mermaid theme config — dark navy, violet/cyan/emerald palette |
| `src/aesthetic-prompt.md` | Canonical Nano Banana prompt prefix for brand consistency |
| `src/batch-generate.sh` | Regenerate all five diagrams in one pass |
| `rendered/*.png` | Mermaid precursor renders (reference inputs for nano banana) |
| `upgraded/*.png` | Nano Banana Pro outputs (also copied to repo root for stable links) |
| `*.png` | Published images referenced from README and docs |

## Diagrams

| # | Diagram | Lives In |
|---|---------|----------|
| 01 | Three-Layer Mesh (hero) | README — "The Three Layers of the Dynamic Mesh" |
| 02 | Insight Ingestion Cycle | README — "The Insight Ingestion Loop" |
| 03 | Four-Plane Voice Architecture | README — Layer 1 voice routing details |
| 04 | MCP Tools Radial | README — Layer 2 MCP tools section |
| 05 | System Architecture (hexagonal) | README — "Architecture" |

## Brand Aesthetic

- **Background**: deep midnight navy `#0A1020` with subtle atmospheric haze
- **Governance layer**: violet `#8B5CF6` glow (top / human judgment)
- **Orchestration layer**: cyan `#00D4FF` glow (middle / agents & reasoning)
- **Discovery layer**: emerald `#10B981` glow (bottom / knowledge & ingestion)
- **Trust hubs**: amber `#F59E0B` glow (sparingly, for critical central nodes)
- **Typography**: clean sans-serif, off-white `#E8F4FC`, all labels verbatim from source
- **Style**: cinematic sci-fi UI concept art meets engineering blueprint — no hand-drawn wobble, no cartoon, no watercolour

## Regenerating

Prerequisites: `mmdc` (Mermaid CLI), `bun`, Chrome/Chromium for mmdc headless, `GOOGLE_API_KEY` set.

```bash
# 1. Edit the Mermaid source
vim src/01-three-layer-mesh.mmd

# 2. Render the precursor
cd docs/diagrams
mmdc -i src/01-three-layer-mesh.mmd \
     -o rendered/01-three-layer-mesh.png \
     -t dark -C src/visionclaw-theme.json \
     -w 2400 -H 1600 -b '#0A1020' --scale 2

# 3. Upgrade via Nano Banana Pro (batch all five)
./src/batch-generate.sh

# 4. Promote the chosen render
cp upgraded/01-three-layer-mesh.png ./01-three-layer-mesh.png
```

The `generate-image` tool lives at `~/.claude/skills/art/tools/generate-image.ts` and is invoked via `bun run` with `--reference-image` for image-to-image style transfer, `--model nano-banana-pro` (`gemini-3-pro-image-preview`), `--size 2K`, `--aspect-ratio 16:9`.

## Technical diagrams (diffable Mermaid → SVG, no Nano-Banana)

Diagrams 13+ are **technical**, not marketing heroes: committed `.mmd` source under
`src/`, rendered to **SVG** via the browsercontainer sidecar
(`agentbox/scripts/mmdc-sidecar.sh -i src/<n>.mmd -o <n>.svg -t dark`). They are the
diffable source of truth and are edited/re-rendered directly — no image upscaling
step. Old small PNGs for 13–20 moved to `../` (2026-08, Lane C; this whole subtree was archived 2026-09-05 — see `docs/diagrams/README.md` for the live tree).

| # | Diagram | Source | SVG | ADR |
|---|---------|--------|-----|-----|
| 13 | ADR-057 actor supervision tree | `src/13-adr057-share-state-transitions.mmd` | `13-adr057-share-state-transitions.svg` | ADR-057 |
| 14 | Skill lifecycle state machine | `src/14-adr057-skill-lifecycle.mmd` | `14-adr057-skill-lifecycle.svg` | ADR-057 |
| 15 | BC18/BC19 context map (+ACL) | `src/15-ddd-context-map.mmd` | `15-ddd-context-map.svg` | DDD ctx |
| 16 | Cross-context relationship map | `src/16-ddd-acl-flow.mmd` | `16-ddd-acl-flow.svg` | DDD ctx |
| 17 | Mesh-promotion sequence | `src/17-ddd-mesh-promotion-sequence.mmd` | `17-ddd-mesh-promotion-sequence.svg` | DDD ctx |
| 18 | Skill retirement sequence | `src/18-skill-retirement-sequence.mmd` | `18-skill-retirement-sequence.svg` | dojo |
| 19 | Skill Dojo publish topology | `src/19-skill-dojo-topology.mmd` | `19-skill-dojo-topology.svg` | dojo |
| 20 | Skill evaluation lifecycle | `src/20-skill-eval-lifecycle.mmd` | `20-skill-eval-lifecycle.svg` | dojo |
| 21 | Force-channel registry + pin mask | `src/21-force-channel-pin-mask.mmd` | `21-force-channel-pin-mask.svg` | ADR-138 |
| 22 | Layout-mode engine registry | `src/22-layout-mode-engine-registry.mmd` | `22-layout-mode-engine-registry.svg` | ADR-141 |

Diagrams **21–22** are the 2026-08 gap-analysis additions (Lane C): the ADR-138
physics substrate (10 named force channels, feature-flag/strength gating, and the
GPU pinned-node mask that anchors dragged nodes — the substrate for the
`interface-sequences.md §5a` drag/pin flow) and the ADR-141 layout programme
(`SetLayoutMode`/`SetRadialMode` → `engine_for()` five-engine registry). Chosen as
highest-value because both underpin the landed Graph2VR/layout wave yet had no
architecture diagram; verified against `src/models/force_channels.rs`,
`src/actors/gpu/force_compute_actor.rs`, and `src/physics/engines/mod.rs`.

Interaction/swarm **sequence** diagrams (node drag→GPU pin, agent-beam `0x23`
broadcast, visual query builder) live inline in
[`interface-sequences.md`](interface-sequences.md) §5 as diffable mermaid blocks.

## When to Add a New Diagram

Add a new diagram when a README section has high conceptual density but no visual aid — e.g., a new architectural pattern, a new governance flow, a new subsystem with ≥4 components. Do **not** add a diagram for every list or table; they should earn their space by communicating structure that prose cannot.
