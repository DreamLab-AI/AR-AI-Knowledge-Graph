---
title: Gap-Close Evidence Index
description: Per-item evidence records for the VisionClaw gap-close sprints (PRD-023 / PRD-024), organised by priority band.
---

# Gap-Close Evidence

> [VisionClaw Docs](../README.md) · Gap-Close Evidence

Closed-item evidence records for the gap-close sprints ([PRD-023](../archive/prd/PRD-023-gap-close-visionclaw.md),
[PRD-024](../archive/prd/PRD-024-final-mile-closeout.md)) and the final-mile process
([ADR-133](../archive/adr/ADR-133-final-mile-sprint.md)). Each file pins one closed gap to
its shipped evidence — the code locus, the regression canary, and the honesty
correction. Records are append-only; they are retained as provenance rather than
rewritten. Companion evidence for PRD-scoped items lives under
[`../prd/gap-close-evidence/`](../archive/prd/gap-close-evidence/README.md).

## P0 — correctness / honesty must-fixes

| Record | Item |
|--------|------|
| [P0-ADR071-PHASE3](P0-ADR071-PHASE3.md) | ADR-071 Phase 3: immersive tree deletion (retires the desktop-as-VR bug) |
| [P0-ADR117-CLAMP](P0-ADR117-CLAMP.md) | ADR-117 WS-0: server-side SPARQL result clamp on `/ontology/query` |
| [P0-ADR119-TELEMETRY](P0-ADR119-TELEMETRY.md) | ADR-119: `ontology_ask` liveness telemetry made observable |
| [P0-COM-14](P0-COM-14.md) | COM-14 / D4 / M1: did:nostr keying of agent nodes (consumer side) |
| [P0-D5](P0-D5.md) | D5: fabricated status indicators removed (both loci) |
| [P0-REC-1](P0-REC-1.md) | REC-1a / REC-1b: already-closed correctness + regression canary |
| [P0-REC-2](P0-REC-2.md) | REC-2: Broker case queue on the ACSP architecture |
| [P0-RES-a](P0-RES-a.md) | RES-a: KG liveness watchdog + sprint-wide LivenessHarness |

## P1 — steering / observability surfaces

| Record | Item |
|--------|------|
| [P1-COM-15](P1-COM-15.md) | COM-15 / V1 / D6 / M5: PTT voice-to-selected-actor governed loop (consumer side) |
| [P1-D2](P1-D2.md) | D2: per-agent steering surface (mount + submit-task + interrupt) |
| [P1-D3](P1-D3.md) | D3 / REC-2 (P1 half): control-centre broker case queue + ambient ACSP indicator |
| [P1-D8](P1-D8.md) | D8: swarm observability view (AgentOps panel) |

## P2 — copresence / voice close-out

| Record | Item |
|--------|------|
| [P2-D2](P2-D2.md) | D2-interrupt final close (real join + honest boundary) |
| [P2-D7](P2-D7.md) | D7: pre-action intent legibility (declared intent) |
| [P2-M1](P2-M1.md) | M1: in-headset identity badge (verified did:nostr render) |
| [P2-M2](P2-M2.md) | M2 / COM-18: in-headset intervention affordance |
| [P2-M3](P2-M3.md) | M3: copresence mechanical core (proxemics, avatar state machine, gaze, `0x44` wire) |
| [P2-M4](P2-M4.md) | M4: gaze + targeting resolving a real selection (Godot copresence) |
| [P2-M6](P2-M6.md) | M6: Godot XR session sets `use_xr` |
| [P2-V3](P2-V3.md) | V3: conversational grounding & repair (confidence gate) |
| [P2-V4](P2-V4.md) | V4: voice docs honesty (deprecated voice-to-swarm path corrected) |
| [P2-vive-closeout-2026-08-20](P2-vive-closeout-2026-08-20.md) | VIVE Pro close-out session evidence record (2026-08-20) |

## See also

- [PRD-024 — Final-Mile Closeout](../archive/prd/PRD-024-final-mile-closeout.md)
- [PRD-023 — Gap-Close VisionClaw](../archive/prd/PRD-023-gap-close-visionclaw.md)
- [ADR-136 — Desktop OpenXR VIVE Validation Target](../archive/adr/ADR-136-desktop-openxr-vive-validation-target.md) (cites the P2-M* records)
