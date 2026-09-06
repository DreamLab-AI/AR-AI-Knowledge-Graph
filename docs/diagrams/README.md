# Diagrams as code

Machine-readable coverage of VisionClaw, agentbox and their estate interfaces, verified against the
code and the ADR packs (`docs/adr`, `agentbox/docs/adr` + governing docs). Sequence diagrams first;
no narrative — every fact lives inside a diagram as a participant `path:line`, a message, or a `Note`.

| Path | Contents |
|------|----------|
| `visionclaw/NN-*.md` (`VC-NN`) | Rust server, GPU/wire, knowledge/data, React + Godot clients |
| `agentbox/NN-*.md` (`AB-NN`) | container runtime, ingress/identity/governance, memory/learning/capabilities |
| `estate/NN-*.md` (`ES-NN`) | cross-repo and infrastructure interfaces (VisionFlow estate) |
| `COVERAGE.md` | generated inverted indexes: diagram → file, ADR → files, governing doc → files, source path → files |
| `hero/` | marketing hero images and their Mermaid → Nano Banana pipeline (README images); not part of the coverage tree |
| `rendered/` | mmdc SVG output, gitignored, regenerable |
| `../archive/diagrams/2026-09-pre-overhaul/` | the pre-2026-09-05 technical diagrams and narrative docs (history only) |

## File contract

````
---
id: VC-03                      # <AREA>-<NN>, unique
title: REST request lifecycle
area: visionclaw               # visionclaw | agentbox | estate (= directory)
governing: [docs/IDENTITY-authority-chain.md]
adrs: [ADR-2009, ADR-2011]
sources: [src/main.rs, src/middleware/rbac_gate.rs]   # repo-relative, must exist
verified_commit: b00c28a0d
---
## VC-03.1 GET /api/graph/data — read path
```mermaid
sequenceDiagram
    autonumber
    participant RG as RbacGate<br/>src/middleware/rbac_gate.rs:122
    ...
```
````

Every mermaid block sits under an `## <file-id>.<n> <title>` heading; ids are unique tree-wide; at most
three prose lines per diagram. Notes use the prefixes `INVARIANT:`, `DIVERGENCE:` (governing-doc open
item), `DOC-DRIFT:` (doc says X, code does Y), `EXTERNAL:` (asserted by this repo about another repo),
`see XX-NN.n` (cross-reference).

## Tooling

```bash
node scripts/diagram-index-gen.js docs/diagrams --check              # frontmatter, ids, paths, prose limit
node scripts/diagram-index-gen.js docs/diagrams --check --render     # + parse every block with mmdc → rendered/
node scripts/diagram-index-gen.js docs/diagrams --check --cite-check # + resolve every path:line citation
node scripts/diagram-index-gen.js docs/diagrams                      # regenerate the index below + COVERAGE.md
```

`--cite-check` resolves each `path:line` inside a diagram against the file's own `sources:` list, asserts the
file is long enough, warns when the anchor line is blank or a lone closing brace, and — for a participant
labelled with a function name — warns when the cited line falls outside that function's body. It **warns,
never fails**.

That last check exists because relocating a citation by diff, however carefully, preserves whatever the
citation meant: if it was already pointing at the wrong line, a re-anchoring pass moves the error and stamps a
fresh `verified_commit` on it. Re-derive a citation from the SYMBOL (`grep -n` the name, then read the body),
never from a computed offset.

Its blind spots: a bare `:NNN` continuation (`Dockerfile.unified:255<br/>… ENTRYPOINT:340`), an extensionless
path (`Makefile:12`, `Dockerfile:40` — the matcher requires a `.ext`), a basename two `sources:` entries share
(`ci.yml`), a `governing:` doc that is not also a source, and — unfixably — a citation that resolves to a real,
non-blank line describing behaviour that has since been deleted. Those still need a human reading the code at
each cited line.

One rule follows from all of it: **verify a landing line by reading it, never by adding a shift to the old
one.** A diff shift tells you where a line moved, not whether the citation was pointing at the right line
before it moved — and a wrong citation plus a correct shift is still a wrong citation, now wearing a fresh
`verified_commit`.

`mmdc` is the Nix-installed Mermaid CLI (11.16). For a visual check, copy an SVG from `rendered/` to
`/home/devuser/gui-tools/` and open `file:///home/devuser/exchange/<name>.svg` in the browsercontainer
sidecar (chrome-devtools MCP `browser-gpu`); `agentbox/scripts/mmdc-sidecar.sh` renders through the same
sidecar. Hero images regenerate with `hero/src/batch-generate.sh` (Nano Banana Pro, `GOOGLE_API_KEY`).

## Diagram index

<!-- BEGIN GENERATED DIAGRAM INDEX -->
_71 topic files, 841 diagrams. Regenerate with_ `node scripts/diagram-index-gen.js docs/diagrams`.

### visionclaw

| ID | Topic | Diagrams | Kinds | Governing | ADRs |
|----|-------|----------|-------|-----------|------|
| VC-01 | [Server boot, AppState construction and the full route table](visionclaw/01-boot-and-app-state.md) | 14 | sequenceDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2004, ADR-2005, ADR-2007, ADR-2008, ADR-2026, ADR-2037, ADR-2038, ADR-2045, ADR-2053 |
| VC-02 | [Actor supervision tree, GraphServiceSupervisor routing and peer actor surfaces](visionclaw/02-actor-supervision.md) | 20 | flowchart, sequenceDiagram, stateDiagram-v2, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2005, ADR-2007, ADR-2045 |
| VC-03 | [Request lifecycle, identity and the RBAC lattice](visionclaw/03-request-lifecycle-and-rbac.md) | 16 | sequenceDiagram, flowchart | [IDENTITY-authority-chain.md](../../docs/IDENTITY-authority-chain.md), [SECURITY-profiles.md](../../docs/SECURITY-profiles.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2002, ADR-2003, ADR-2009, ADR-2010, ADR-2011, ADR-2012, ADR-2013, ADR-2026, ADR-2039, ADR-2043, ADR-2044 |
| VC-04 | [Handler internals — graph, state, and domain route families](visionclaw/04-handlers-graph-and-state.md) | 29 | sequenceDiagram, flowchart, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2005, ADR-2007, ADR-2011 |
| VC-05 | [Governance and identity handler families](visionclaw/05-handlers-governance-and-identity.md) | 18 | sequenceDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [IDENTITY-authority-chain.md](../../docs/IDENTITY-authority-chain.md) | ADR-2006, ADR-2010, ADR-2011, ADR-2013, ADR-2016 |
| VC-06 | [Settings round trip — REST, actors, SQLite adapter and generated client types](visionclaw/06-settings-round-trip.md) | 10 | flowchart, sequenceDiagram, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2005, ADR-2011, ADR-2041, ADR-2046, ADR-2047, ADR-2080 |
| VC-07 | [Hexagonal ports, adapters, the CQRS application layer and the crate split](visionclaw/07-hexagonal-ports-and-crates.md) | 11 | flowchart, sequenceDiagram, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2004, ADR-2005, ADR-2016 |
| VC-08 | [Observability, liveness canaries, health composition and the dev/production build loop](visionclaw/08-observability-health-and-dev-loop.md) | 12 | sequenceDiagram, flowchart, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [SECURITY-profiles.md](../../docs/SECURITY-profiles.md) | ADR-2008, ADR-2026, ADR-2037, ADR-2038, ADR-2049 |
| VC-09 | [Configuration loading, boot-time profile assertion and the environment-flag register](visionclaw/09-config-and-env-flags.md) | 15 | sequenceDiagram, flowchart, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [SECURITY-profiles.md](../../docs/SECURITY-profiles.md) | ADR-2012, ADR-2026, ADR-2037, ADR-2038, ADR-2039, ADR-2041, ADR-2043, ADR-2046, ADR-2094 |
| VC-10 | [GPU supervision and context bus](visionclaw/10-gpu-supervision-and-context-bus.md) | 10 | flowchart, sequenceDiagram, stateDiagram-v2, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [GPU-wire-abi.md](../../docs/GPU-wire-abi.md) | ADR-2007, ADR-2053 |
| VC-11 | [Physics step and force channels](visionclaw/11-physics-step-and-force-channels.md) | 9 | sequenceDiagram, flowchart, classDiagram, stateDiagram-v2 | [GPU-wire-abi.md](../../docs/GPU-wire-abi.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2007, ADR-2028, ADR-2029, ADR-2055, ADR-2060 |
| VC-12 | [SimParams ABI, GPU buffers and PTX](visionclaw/12-simparams-abi-and-ptx.md) | 7 | classDiagram, sequenceDiagram, flowchart | [GPU-wire-abi.md](../../docs/GPU-wire-abi.md) | ADR-2028, ADR-2030, ADR-2055, ADR-2056 |
| VC-13 | [Position broadcast pipeline and WebSocket](visionclaw/13-broadcast-pipeline-and-websocket.md) | 8 | sequenceDiagram, stateDiagram-v2 | [PROTOCOL-registry.md](../../docs/PROTOCOL-registry.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2003, ADR-2018, ADR-2009, ADR-2002 |
| VC-14 | [Wire frames and tag registry](visionclaw/14-wire-frames-and-tag-registry.md) | 9 | classDiagram, flowchart, sequenceDiagram | [PROTOCOL-registry.md](../../docs/PROTOCOL-registry.md), [GPU-wire-abi.md](../../docs/GPU-wire-abi.md) | ADR-2018, ADR-2019, ADR-2020, ADR-2024, ADR-2057, ADR-2060 |
| VC-15 | [GPU analytics kernels and pathfinding](visionclaw/15-gpu-analytics.md) | 13 | sequenceDiagram, classDiagram, flowchart | [GPU-wire-abi.md](../../docs/GPU-wire-abi.md) | ADR-2007, ADR-2053, ADR-2054, ADR-2061 |
| VC-16 | [Interaction — drag, pin, layout, constraints and agent beams](visionclaw/16-interaction-drag-pin-layout.md) | 7 | sequenceDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [PROTOCOL-registry.md](../../docs/PROTOCOL-registry.md) | ADR-2020, ADR-2029, ADR-2055 |
| VC-17 | [XR presence crate and co-presence](visionclaw/17-xr-presence-crate.md) | 6 | sequenceDiagram, stateDiagram-v2, classDiagram | [PROTOCOL-registry.md](../../docs/PROTOCOL-registry.md), [XR-client.md](../../docs/XR-client.md) | ADR-2019, ADR-2020 |
| VC-18 | [Analytics support handlers and the analytics WebSocket](visionclaw/18-analytics-support-handlers.md) | 9 | flowchart, sequenceDiagram, classDiagram | [PROTOCOL-registry.md](../../docs/PROTOCOL-registry.md), [GPU-wire-abi.md](../../docs/GPU-wire-abi.md) | ADR-2007, ADR-2009, ADR-2059 |
| VC-20 | [Ontology pipeline - OWL extraction, Oxigraph, Whelk reasoning, governed mutation](visionclaw/20-ontology-pipeline-oxigraph-whelk.md) | 12 | sequenceDiagram, erDiagram, classDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2004, ADR-2071, ADR-2064, ADR-2066, ADR-2068 |
| VC-21 | [Corpus ingest (GitHub/local vault) and the vault-migrate converter](visionclaw/21-corpus-ingest-and-vault.md) | 12 | sequenceDiagram, flowchart, stateDiagram-v2, classDiagram | [VAULT-corpus-format.md](../../docs/VAULT-corpus-format.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2014, ADR-2040, ADR-2041, ADR-2042, ADR-2070 |
| VC-22 | [Data authority, provenance and erasure](visionclaw/22-data-authority-provenance-erasure.md) | 11 | flowchart, erDiagram, sequenceDiagram | [DATA-authority-erasure.md](../../docs/DATA-authority-erasure.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2004, ADR-2015, ADR-2016, ADR-2017, ADR-2069, ADR-2070 |
| VC-23 | [Identifier taxonomy — typed URN, did:nostr, sha256-12, federation crossing, wire node-id](visionclaw/23-identifiers-urn-did-sha12.md) | 10 | classDiagram, sequenceDiagram, flowchart | [IDENTIFIER-taxonomy.md](../../docs/IDENTIFIER-taxonomy.md) | ADR-2021, ADR-2022, ADR-2023, ADR-2024, ADR-2025, ADR-2070, ADR-2072 |
| VC-24 | [ACSP — governed decision/elevation pipeline](visionclaw/24-acsp-decision-elevation.md) | 11 | stateDiagram-v2, sequenceDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2006, ADR-2101 |
| VC-25 | [Insight loop, KPI, briefing, NLQ and semantic classification](visionclaw/25-insight-kpi-nlq-semantics.md) | 13 | sequenceDiagram, erDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2014, ADR-2040, ADR-2004, ADR-2063, ADR-2065 |
| VC-26 | [Solid Pod integration — embedded pod, proxy, client stack](visionclaw/26-solid-pod-and-jss.md) | 12 | flowchart, sequenceDiagram | [DATA-authority-erasure.md](../../docs/DATA-authority-erasure.md), [IDENTITY-authority-chain.md](../../docs/IDENTITY-authority-chain.md) | ADR-2067, ADR-2068, ADR-2070 |
| VC-27 | [Agent estate integration — MCP relay, discovery, monitoring, ingest](visionclaw/27-agent-integration-mcp-relay.md) | 13 | sequenceDiagram, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [IDENTIFIER-taxonomy.md](../../docs/IDENTIFIER-taxonomy.md) | ADR-2025 |
| VC-28 | [External services — outbound integrations](visionclaw/28-external-services.md) | 9 | sequenceDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2066 |
| VC-30 | [React client boot sequence and state layer](visionclaw/30-client-boot-and-state.md) | 12 | sequenceDiagram, classDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2074, ADR-2077 |
| VC-31 | [R3F/Three.js graph render pipeline and WASM scene effects](visionclaw/31-client-graph-render-pipeline.md) | 10 | flowchart, sequenceDiagram, classDiagram | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) |  |
| VC-32 | [Client WebSocket transport and binary position protocol](visionclaw/32-client-websocket-and-binary.md) | 16 | sequenceDiagram, classDiagram, flowchart | [PROTOCOL-registry.md](../../docs/PROTOCOL-registry.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2002, ADR-2019, ADR-2020, ADR-2047, ADR-2057, ADR-2078, ADR-2080 |
| VC-33 | [Browser-client identity — NIP-07, NIP-98, passkeys, RBAC gating](visionclaw/33-client-auth-and-identity.md) | 9 | sequenceDiagram, classDiagram | [IDENTITY-authority-chain.md](../../docs/IDENTITY-authority-chain.md), [SECURITY-profiles.md](../../docs/SECURITY-profiles.md) | ADR-2002, ADR-2009, ADR-2011, ADR-2012, ADR-2074, ADR-2075 |
| VC-34 | [Client feature directories — API and WebSocket surface](visionclaw/34-client-features.md) | 21 | sequenceDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2041, ADR-2006, ADR-2074, ADR-2077 |
| VC-35 | [Voice end to end — PTT, STT, intent, TTS](visionclaw/35-voice-end-to-end.md) | 12 | stateDiagram-v2, sequenceDiagram, classDiagram, flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [IDENTITY-authority-chain.md](../../docs/IDENTITY-authority-chain.md) | ADR-2002, ADR-2039, ADR-2075 |
| VC-36 | [Godot + gdext OpenXR immersive client](visionclaw/36-godot-xr-client.md) | 18 | sequenceDiagram, flowchart, stateDiagram-v2, classDiagram | [XR-client.md](../../docs/XR-client.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2032, ADR-2033, ADR-2034, ADR-2035, ADR-2036, ADR-2039, ADR-2076, ADR-2079 |
| VC-37 | [Browser XR surface and desktop spatial input](visionclaw/37-browser-xr-and-desktop-input.md) | 8 | sequenceDiagram, flowchart, stateDiagram-v2 | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [XR-client.md](../../docs/XR-client.md) | ADR-2032, ADR-2081 |

### agentbox

| ID | Topic | Diagrams | Kinds | Governing | ADRs |
|----|-------|----------|-------|-----------|------|
| AB-01 | [Nix flake composition and apply-class gates](agentbox/01-nix-flake-composition.md) | 10 | flowchart, stateDiagram-v2, sequenceDiagram, classDiagram | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2003, ADR-2006, ADR-2029, ADR-2039 |
| AB-02 | [Boot sequence, supervision tree and readiness](agentbox/02-boot-sequence-and-readiness.md) | 18 | sequenceDiagram, flowchart, stateDiagram-v2 | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2003, ADR-2007, ADR-2028, ADR-2029, ADR-2034, ADR-2063 |
| AB-03 | [Management API request lifecycle and route table](agentbox/03-management-api-request-lifecycle.md) | 17 | sequenceDiagram, flowchart | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md), [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md) | ADR-2005, ADR-2013, ADR-2003 |
| AB-04 | [Five-slot adapter spine, dispatch middleware and connect lifecycle](agentbox/04-adapter-spine.md) | 16 | flowchart, classDiagram, sequenceDiagram, stateDiagram-v2 | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2004, ADR-2005, ADR-2035, ADR-2036, ADR-2037, ADR-2064 |
| AB-05 | [Manifest gate catalogue, vault path authority and the agentbox.sh CLI](agentbox/05-manifest-gates-and-cli.md) | 10 | sequenceDiagram, flowchart, stateDiagram-v2 | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2003, ADR-2028, ADR-2029, ADR-2036, ADR-2037, ADR-2038, ADR-2039 |
| AB-06 | [Compose overlays, sidecar topology and the loopback-publish invariant](agentbox/06-sidecars-and-compose-overlays.md) | 8 | flowchart, sequenceDiagram | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2013, ADR-2003, ADR-2040 |
| AB-07 | [Daemon classes, argv-boundary reaping, cron and backups](agentbox/07-daemons-reapers-cron-backups.md) | 9 | flowchart, stateDiagram-v2, sequenceDiagram | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2032, ADR-2003, ADR-2039, ADR-2040 |
| AB-08 | [Claude Code hook pipeline and its handlers](agentbox/08-hooks-pipeline.md) | 14 | flowchart, sequenceDiagram | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2015, ADR-2026, ADR-2007 |
| AB-09 | [MCP registry, boot projector and the server catalogue](agentbox/09-mcp-servers-catalogue.md) | 7 | flowchart, sequenceDiagram | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2008, ADR-2003, ADR-2039 |
| AB-10 | [Ingress — nip98-proxy and the AoE door](agentbox/10-ingress-nip98-proxy-and-aoe.md) | 12 | flowchart, sequenceDiagram, stateDiagram-v2 | [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md), [SECURITY-profiles.md](../../agentbox/docs/SECURITY-profiles.md) | ADR-2002, ADR-2009, ADR-2010, ADR-2011, ADR-2013, ADR-2047 |
| AB-11 | [Identity — DID, URN, mandate, authority](agentbox/11-identity-did-mandate-authority.md) | 16 | classDiagram, sequenceDiagram, flowchart, stateDiagram-v2 | [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md), [PROTOCOL-registry.md](../../agentbox/docs/PROTOCOL-registry.md) | ADR-2011, ADR-2025, ADR-2027, ADR-2064 |
| AB-12 | [tab0-bridge and the interaction plane](agentbox/12-tab0-bridge-and-interaction-plane.md) | 14 | flowchart, sequenceDiagram, classDiagram | [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md), [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md) | ADR-2009, ADR-2010, ADR-2011, ADR-2047 |
| AB-13 | [Nostr — relay, gateway, pod bridge, session mirror](agentbox/13-nostr-relay-gateway-bridge-mirror.md) | 17 | flowchart, sequenceDiagram, classDiagram, stateDiagram-v2 | [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md), [SECURITY-profiles.md](../../agentbox/docs/SECURITY-profiles.md), [PROTOCOL-registry.md](../../agentbox/docs/PROTOCOL-registry.md) | ADR-2012, ADR-2025, ADR-2026, ADR-2061 |
| AB-14 | [Governance — journal, action pipeline, approvals](agentbox/14-governance-journal-actions-approvals.md) | 13 | flowchart, stateDiagram-v2, sequenceDiagram, classDiagram | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md), [SECURITY-profiles.md](../../agentbox/docs/SECURITY-profiles.md) | ADR-2022, ADR-2027 |
| AB-15 | [Capability gating, spend caps and consultants](agentbox/15-capability-gating-spend-consultants.md) | 13 | flowchart, sequenceDiagram, stateDiagram-v2 | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md), [SECURITY-profiles.md](../../agentbox/docs/SECURITY-profiles.md) | ADR-2020, ADR-2031, ADR-2033 |
| AB-16 | [Secrets custody, seccomp and runtime profiles](agentbox/16-secrets-custody-seccomp-profiles.md) | 11 | flowchart, classDiagram, sequenceDiagram, stateDiagram-v2 | [SECURITY-profiles.md](../../agentbox/docs/SECURITY-profiles.md), [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md) | ADR-2007, ADR-2026, ADR-2027, ADR-2033 |
| AB-17 | [Agent events and the BC20 provenance bridge](agentbox/17-agent-events-and-provenance-bridge.md) | 10 | classDiagram, sequenceDiagram | [PROTOCOL-registry.md](../../agentbox/docs/PROTOCOL-registry.md), [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md) | ADR-2011, ADR-2022, ADR-2025, ADR-2061 |
| AB-20 | [RuVector memory path — every MCP memory tool end to end](agentbox/20-ruvector-memory-path.md) | 12 | sequenceDiagram, flowchart, stateDiagram-v2, erDiagram | [LEARNING-memory.md](../../agentbox/docs/LEARNING-memory.md) | ADR-2014, ADR-2018, ADR-2019, ADR-2051 |
| AB-21 | [Learning loop — capture, judge, distil, consume](agentbox/21-learning-loop.md) | 10 | sequenceDiagram, classDiagram, stateDiagram-v2, flowchart | [LEARNING-memory.md](../../agentbox/docs/LEARNING-memory.md) | ADR-2015, ADR-2016, ADR-2017, ADR-2018, ADR-2051, ADR-2052 |
| AB-22 | [Skills estate — discovery, lint gate, routing, harness/precedent MCP bridges](agentbox/22-skills-and-routing.md) | 13 | sequenceDiagram, stateDiagram-v2, flowchart | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md) | ADR-2020, ADR-2021, ADR-2028, ADR-2056, ADR-2057 |
| AB-23 | [Dream machine — nightly cycle, gates and acceptance path](agentbox/23-dream-engine.md) | 10 | stateDiagram-v2, sequenceDiagram, classDiagram, flowchart | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md) | ADR-2024, ADR-2053 |
| AB-24 | [Ontology Loom facade and the model-swap seam](agentbox/24-loom-facade.md) | 9 | flowchart, sequenceDiagram, classDiagram, stateDiagram-v2 | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md) | ADR-2023, ADR-2053, ADR-2055 |
| AB-25 | [Ontology tools and governed writes](agentbox/25-ontology-tools-and-governed-writes.md) | 10 | flowchart, sequenceDiagram, classDiagram, stateDiagram-v2 | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md) | ADR-2022, ADR-2023, ADR-2028, ADR-2054 |
| AB-26 | [Headroom compression, the beads work-DAG, typed spawn and RuvNet grounding](agentbox/26-headroom-beads-spawn-brain.md) | 9 | sequenceDiagram, classDiagram, stateDiagram-v2, flowchart | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md), [LEARNING-memory.md](../../agentbox/docs/LEARNING-memory.md) | ADR-2004, ADR-2005, ADR-2020 |
| AB-27 | [Media and GPU capability services — manifest-gated dispatch](agentbox/27-media-gpu-capability-services.md) | 12 | sequenceDiagram, flowchart, classDiagram | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md), [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md) | ADR-2006, ADR-2020, ADR-2057 |
| AB-28 | [Agentbox service crates and the manifest binary](agentbox/28-agentbox-services-and-manifest-binary.md) | 9 | flowchart, sequenceDiagram, classDiagram | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2030, ADR-2031, ADR-2032 |

### estate

| ID | Topic | Diagrams | Kinds | Governing | ADRs |
|----|-------|----------|-------|-----------|------|
| ES-01 | [Estate topology — substrates, network fabric, service ports, compose networks](estate/01-estate-topology.md) | 6 | flowchart | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2023, ADR-2013, ADR-2027, ADR-2025, ADR-2009, ADR-2012, ADR-2062 |
| ES-02 | [Agent-event path — agentbox action to rendered beam, plus legacy paths](estate/02-agent-events-agentbox-to-visionclaw.md) | 11 | sequenceDiagram, classDiagram | [PROTOCOL-registry.md](../../docs/PROTOCOL-registry.md), [GPU-wire-abi.md](../../docs/GPU-wire-abi.md), [PROTOCOL-registry.md](../../agentbox/docs/PROTOCOL-registry.md) | ADR-2020, ADR-2015, ADR-2083, ADR-2084, ADR-2085, ADR-2088, ADR-2089, ADR-2090, ADR-2091 |
| ES-03 | [Cross-repo federation contract (agentbox <-> VisionClaw)](estate/03-cross-repo-federation-contract.md) | 10 | classDiagram, flowchart, sequenceDiagram | [IDENTIFIER-taxonomy.md](../../docs/IDENTIFIER-taxonomy.md), [PROTOCOL-registry.md](../../docs/PROTOCOL-registry.md), [PROTOCOL-registry.md](../../agentbox/docs/PROTOCOL-registry.md), [DATA-authority-erasure.md](../../docs/DATA-authority-erasure.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2023, ADR-2025, ADR-2061 |
| ES-04 | [did:nostr identity mesh — signing, verification, custody](estate/04-identity-mesh-did-nostr.md) | 6 | flowchart, classDiagram, sequenceDiagram | [IDENTITY-authority-chain.md](../../docs/IDENTITY-authority-chain.md), [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md), [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md), [SECURITY-profiles.md](../../docs/SECURITY-profiles.md) | ADR-2002, ADR-2009, ADR-2010, ADR-2011, ADR-2013, ADR-2026 |
| ES-05 | [Human-approval governance loop across the estate](estate/05-governance-loop-across-estate.md) | 10 | flowchart, classDiagram, sequenceDiagram, stateDiagram-v2 | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md), [BASELINE-architecture.md](../../docs/BASELINE-architecture.md) | ADR-2006 |
| ES-06 | [Ontology Loom and the email privacy path](estate/06-loom-and-email-privacy-path.md) | 9 | flowchart, sequenceDiagram, stateDiagram-v2 | [GOVERNANCE-capabilities.md](../../agentbox/docs/GOVERNANCE-capabilities.md), [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2023 |
| ES-07 | [RuVector memory and embedding estate](estate/07-memory-and-embedding-estate.md) | 9 | flowchart, sequenceDiagram, stateDiagram-v2 | [LEARNING-memory.md](../../agentbox/docs/LEARNING-memory.md), [DATA-authority-erasure.md](../../docs/DATA-authority-erasure.md) | ADR-2014, ADR-2015, ADR-2016 |
| ES-08 | [Solid-pod estate — four deployments, write identity, access control](estate/08-solid-pod-estate.md) | 10 | flowchart, sequenceDiagram, classDiagram, stateDiagram-v2 | [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md), [DATA-authority-erasure.md](../../docs/DATA-authority-erasure.md), [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md) | ADR-2015, ADR-2016, ADR-2017, ADR-2064, ADR-2068 |
| ES-09 | [Build, deploy and CI estate — source to running container, every gate](estate/09-build-deploy-and-ci-estate.md) | 19 | flowchart, sequenceDiagram, stateDiagram-v2 | [BASELINE-architecture.md](../../docs/BASELINE-architecture.md), [BASELINE-container.md](../../agentbox/docs/BASELINE-container.md) | ADR-2008, ADR-2037, ADR-2013, ADR-2028 |
| ES-10 | [Deployment and security profiles across the estate](estate/10-deployment-and-security-profiles.md) | 10 | flowchart, sequenceDiagram, stateDiagram-v2 | [SECURITY-profiles.md](../../docs/SECURITY-profiles.md), [SECURITY-profiles.md](../../agentbox/docs/SECURITY-profiles.md), [INGRESS-identity.md](../../agentbox/docs/INGRESS-identity.md), [IDENTITY-authority-chain.md](../../docs/IDENTITY-authority-chain.md) | ADR-2003, ADR-2010, ADR-2012, ADR-2013, ADR-2026, ADR-2027, ADR-2037, ADR-2038, ADR-2039, ADR-2062, ADR-2086, ADR-2087 |
<!-- END GENERATED DIAGRAM INDEX -->
