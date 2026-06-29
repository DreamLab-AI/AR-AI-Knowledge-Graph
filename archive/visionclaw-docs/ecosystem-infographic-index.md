# Ecosystem Infographic Index — regen targets (rebuilt 2026-06-14)

Authoritative, **scoped** list for the infographic-revamp sprint. Scope (per operator
2026-06-14): **infographics in READMEs + websites across the ecosystem only**. **Live-system
screenshots and photography are EXCLUDED** (do not touch). Supersedes the broad ADR-111 §3
inventory for *this* sprint; ADR-111 remains the verdict/prompt spec.

Two regen modes:
- **NB** = Nano Banana Pro 4K regen (`gemini-3-pro-image`, GA — most performant for text-in-image). Generator: `agentbox/skills/art/tools/nb-generate.cjs` (no-dep REST; access confirmed). 512px Flash preview → 4K Pro final. Each replacement is **before/after visually QA'd in-model** and kept only on real improvement (git-reversible).
- **MMD** = re-render the technical diagram from its mermaid source at high DPI (AI regen would garble precise structure/text). Some also need **content updates** (drift).

---

## VisionClaw (`project`) — README

| Image | Mode | Notes |
|---|---|---|
| `docs/diagrams/linkedInEcosystem.png` | **NB (author-update)** | Dense 3-panel hand-drawn ecosystem infographic; very text-heavy → faithful AI-regen is hard. Update content (add ontology-augmentation + smart-contract/web-contract capabilities) then regen, or re-author panels. Highest-value hero. |
| `docs/diagrams/01-three-layer-mesh.png` | **MMD** | `.mmd` at `docs/diagrams/src/01-*.mmd`; an `upgraded/` NB-Pro final exists — point README at it or re-render. |
| `docs/diagrams/04-mcp-tools-radial.png` | **MMD + content** | Says "7 ontology tools" — **drifted**: `ontology_ask` (PRD-020) added. Update the `.mmd` then re-render. |
| `docs/diagrams/05-architecture-hexagonal.png` | **MMD** | `.mmd` present; re-render high-DPI. |
| **NEW** §4.6 ontology-augmentation, §4.7 web-contract trust-spectrum | **MMD** | New diagrams from ADR-111 §4.6/§4.7 (PRD-020/ADR-124). |
| `graph-*`, `logseq*`, `ChloeOctave.jpg` | **KEEP** | live captures / photo — excluded. |

## agentbox — README
| Image | Mode | Notes |
|---|---|---|
| `docs/agentbox.png` | **NB** | Hero infographic (2026 sovereign stack); refresh for new features (ontology binding, web-contracts). |
| `docs/images/setup-wizard-overview.png`, `setup-dashboard.png`, `setup-wizard-sections.png` | **KEEP** | live SPA screenshots/mockups — excluded. |

## VisionFlow (`../VisionFlow`) — README + website (the marketing site; richest target)
| Image | Mode | Notes |
|---|---|---|
| `assets/heroes/{cyber-infrastructure, decentralised-agents, dreamlab-hero, visionflow-power-user}.webp` | **NB** | Marketing heroes → 4K Pro regen depicting the real stack (Oxigraph KG/Godot XR, 402/MRC20/block-trails, did:nostr, web-contracts). |
| `assets/generated/{coordination-topology, evolution-line, five-substrates, identity-spine, judgment-broker}.png` | **MMD** | Mermaid-rendered — re-render high-DPI. |
| `assets/diagrams/{ecosystem-overview, agentbox-overview, solid-pod-rs-architecture, wardley-map}.png` | **MMD** | Strategy/arch — re-render (wardley from source). |
| `assets/upstream/jss-architecture.svg` | **MMD/KEEP** | upstream SVG. |
| `assets/screenshots/visionclaw-graph-live.png` | **KEEP** | live capture. |

## dreamlab-ai-website (`../dreamlab-ai-website`) — programme heroes
| Image | Mode | Notes |
|---|---|---|
| `public/images/heroes/{ai-commander-week, visionflow-power-user, decentralised-agents, cyber-infrastructure, xr-innovation-intensive, engineering-visualisation, neural-content-creation, virtual-production-master, corporate-immersive, creative-technology-fundamentals}.webp` | **NB** | ~10 programme heroes built from 2024 generic stock → 4K Pro regen to the real tooling (ADR-111 §3.5 prompts §5.1-5.6). Purge implied legacy (Babylon/Vircadia/Neo4j). |
| `heroes/{minimoonoir, dreamlab, family, business, digital-human-mocap, spatial-audio, lake-district-dawn}.webp`, `docs/images/screenshots/*` | **KEEP** | venue/team/timeless/photographic + forum screenshots — excluded. |

## nostr-rust-forum (`../nostr-rust-forum`)
| Image | Mode | Notes |
|---|---|---|
| `docs/screenshots/forum-*.webp` (3) | **KEEP** | live forum screenshots — excluded. |
| `docs/diagrams/*` (73+ mermaid blocks) | **MMD (if stale)** | exemplar repo, all diagram-as-code + current; re-render only if drifted. |

## solid-pod-rs (`../solid-pod-rs`)
| Image | Mode | Notes |
|---|---|---|
| `crates/solid-pod-rs/docs/diagrams/rendered/01-08*.png` | **MMD** | 8 architecture diagrams; PNG renders stale vs `.mmd` (ADR-111 §3.4) — re-render high-DPI. README itself embeds none. |
| NEW: 402/webledger, block-trails+git-marks, did:nostr (ADR-111 §4.1-4.3) | **MMD** | gap-fill diagrams. |

---

## Totals (this sprint)
- **NB (Nano Banana Pro 4K regen):** ~15-16 — VisionFlow heroes (4) + dreamlab-ai-website programme heroes (~10) + agentbox.png (1) + linkedInEcosystem (1, author-update). **before/after vision-QA, replace-if-improved.**
- **MMD (re-render high-DPI):** ~25 — VisionClaw diagrams + new §4.6/§4.7, VisionFlow generated/diagrams, solid-pod-rs 8 + 3 gaps, forum (if stale).
- **KEEP (excluded):** all live screenshots, graph captures, logseq, setup SPA, venue/team/timeless photos, forum screenshots.

**Execution:** best run as a dedicated opus mesh — one agent per NB hero (generate 4K Pro → read before+after → keep on real improvement) + a mermaid re-render pass. Generator + model + in-model vision-QA loop are proven (see this ADR-111 update block + `nb-generate.cjs`).
