---
id: ADR-2025
title: cross_from_agentbox is a closed kind-map that returns None, never a synthetic ID, for unmapped URNs
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: adding a new mapped inbound kind (e.g. memory→concept elevation), or a caller that fabricates an ID on None
repo: visionclaw
domain: IDENTIFIER-taxonomy
lineage: legacy ADR-063 (minted URNs may return null), ADR-105 (convergence); distils agentbox B04 closed-map discipline
---

# ADR-2025 — cross_from_agentbox is a closed kind-map that returns None, never a synthetic ID, for unmapped URNs

## Context

Inbound `urn:agentbox:*` identifiers arrive at the federation boundary and must
become converged VisionClaw IDs. The unsafe reflex is to fabricate a durable ID
for any inbound kind so ingest never blocks — which mints garbage identifiers for
kinds the translator cannot faithfully map (e.g. `memory`→`concept`, which needs
an elevation `{domain, slug}` target absent on the hot path). See
`docs/IDENTIFIER-taxonomy.md` for the federation seam.

## Decision

`cross_from_agentbox` is a closed kind-map. It translates only `agent` →
`did:nostr:<pk>`, `activity` → `urn:visionclaw:execution:<addr>`, and `thing` →
`urn:visionclaw:kg:<pk>:<addr>`, plus an already-converged `did:nostr:*` that
passes through structurally. `memory` and every other unmapped kind return
`None`, so the caller records the raw string plus an unmapped marker rather than
fabricate a durable ID. The `UrnCrossing` struct carries both ends
(`agentbox_urn`, `visionclaw_id`, `owner_did`) so the crossing is stored as a
recoverable translation, not an opaque foreign blob. This forecloses synthesising
an ID for a kind the map does not know.

## Consequences

- No synthetic durable IDs enter the store; every crossing is either a faithful
  translation or an explicit unmapped record with its source preserved.
- Callers must handle `None` (record-raw), not `unwrap`/fabricate; a caller that
  invents an ID on `None` defeats the discipline — hence the `review_trigger`.
- Extending coverage (e.g. `memory`→`concept`) requires threading the elevation
  target to this seam, a deliberate change rather than a silent default.

## Verification

Re-checked at `e0f8cd896`: `src/uri/mod.rs:514-553` — `cross_from_agentbox` with
the closed `match kind` whose `memory`/default arm returns `None` at `:543-545`;
the `did:nostr` pass-through at `:516-525`; `UrnCrossing` at `:490-498` carries
both ends plus `owner_did` for audit recovery.

## Closeout extension — 2026-09-04

CP-01/02/04/05. Owner remains jjohare with agentbox identifier maintainers. The stated Rust closed map remains intact. A paired fixture shows that the JS bridge additionally maps beads, while Rust returns None. This is a supported-kind divergence to decide jointly, not evidence that every incoming kind should receive an ID.

**Acceptance condition:** bind bytes/serialisation, complete address grammar and kind/elevation support to a shared versioned fixture run in both repositories. Test malformed precomputed addresses and persisted round-trip recovery. Preserve explicit unmapped outcomes and existing decision status. Reopen on hash, parser, scope or kind-map changes. See the [identifier review](../../../VisionFlow/docs/estate-review/federation-identifiers.md) and [paired receipt](../../../VisionFlow/docs/estate-review/evidence/federation-identity-probe.json). These are helper-level results, not deployed-ingest certification.
