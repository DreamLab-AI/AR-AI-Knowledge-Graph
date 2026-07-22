# Documentation Archive (retired)

> Correction (2026-07-22 doc-drift audit): the "non-substantive process notes"
> characterisation of `visionclaw-process/` (below) and the "every internal
> forward/back link resolves" claim (bottom of file) are **stale**.
> `PRD-015-ecosystem-code-hygiene.md` and `PRD-021-omb-xr-interface-investigation.md`
> carry substantive, still-cited findings — PRD-015's O1/PAR-01/02/03 hygiene
> findings are referenced post-archival by ADR-087/088/089/091/104, and PRD-021's
> Strategy B underpins [ADR-126](../docs/adr/ADR-126-omb-adoption-posture.md).
> Both are **substantive-but-archived**, not process ephemera. See
> [ADR-131](../docs/adr/ADR-131-doc-drift-reconciliation-2026-07.md) §1f.

This directory holds the **previous** VisionClaw and agentbox documentation systems,
retired in the clean-room documentation rebuild. Nothing here is part of the published
docs. It is kept only as a recoverable historical record; everything is also in git
history. The canonical, maintained documentation now lives in:

- VisionClaw — [`../docs/README.md`](../docs/README.md)
- agentbox — [`../agentbox/docs/README.md`](../agentbox/docs/README.md)

## What was retired and why

| Path | Contents | Reason |
|:-----|:---------|:-------|
| `visionclaw-docs/` | Ephemeral process docs: `audit/`, `audits/`, `control-surface-audit/`, `data-sprint/`, `migration-sprint/`, `eval/`, `design/`, `integration-research/`, `qe/`, `testing/`, `use-cases/`, the old `architecture/` map, `how-to/infrastructure/`, the consumed merge-source files (`binary-protocol.md`, `gpu-physics-architecture.md`, `security.md`, `xr-godot-*.md`, `neo4j-schema-unified.md`, `websocket-binary.md`, …) and the obsolete Neo4j guides | Sprint/audit artefacts whose durable decisions already landed as ADRs; merge sources folded into the new canonical docs; Neo4j removed per ADR-11 (store is Oxigraph + SQLite) |
| `visionclaw-process/` | Process PRD/DDD notes (`PRD-001-pipeline-alignment`, `PRD-013-closeout`, `PRD-014-addendum-qe-fleet-validation`, `PRD-QE-001/002`, `PRD-agent-orchestration-improvements`, `ddd-code-hygiene-context`, `ddd-qe-traceability-graph-context`, …) | Closeout / addendum / QE-sprint planning notes. The substantive PRD/DDD records were kept under `../docs/prd/` and `../docs/ddd/` |
| `agentbox-docs/` | `regen-2026-06-14/`, `reference/.claude-flow/`, `00-anomaly-register.md`, superseded images | One-shot regeneration / tooling-state artefacts |
| `media/` | Large videos (`complexGraph.mp4` 153 MB, `complexGraph-web.mp4` 65 MB, `complexGraph-tiny.mp4`, `visionclaw.mp4`) | Unreferenced large binaries removed from the docs tree to keep the repo lean; host externally if needed |

## Recovering an item

```bash
git mv archive/<path>/<file> docs/<destination>/
# or inspect prior state:
git log --follow -- archive/<path>/<file>
```

The clean-room rebuild verified **every** Mermaid diagram compiles for GitHub and
**every** internal forward/back link resolves across both published packs.
