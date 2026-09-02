---
id: ADR-2011
title: One central RbacGate covers the whole /api scope; writes gate at WriteGraph, not mere Authenticated
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []                   # legacy ADR-011/ADR-142 distilled — not in this tree; see lineage
superseded_by: []
verified_commit: f326a3b1172df4fea8183e6a4344d3f55c575013
verified_paths: [src/middleware/rbac_gate.rs, src/utils/auth.rs]
owner: jjohare
review_trigger: addition of an /api sub-scope with a distinct auth requirement, or any change to the public-prefix allowlist
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: Distils legacy ADR-011 (enforce at scope config not handler) + ADR-142 ('15+ endpoints missing auth' gap).
---

# ADR-2011 — One central RbacGate covers the whole /api scope; writes gate at WriteGraph, not mere Authenticated

## Context

Per-handler auth checks left an audited gap of 15+ `/api` endpoints with no enforcement (ADR-142).
Prefix matching on raw string segments risks `/api/administrator` inheriting `/api/admin`'s
policy. Gating writes at `Authenticated` alone would let a `Viewer` mutate the graph, because an
authenticated Viewer is still authenticated. ADR-011 required enforcement at scope config, not
scattered across handlers.

## Decision

A single middleware (`RbacGate`) computes `required_level` per `(method, whole-segment path)` across
the entire `/api` scope. Matching is whole-`/`-segment, so `/api/administrator` does not inherit
`/api/admin`. The admin surface requires `Admin` for every method; mutations require `WriteGraph`
— satisfied by Editor(→Authenticated)/Admin but refused to a Viewer(→ReadOnly). This forecloses
per-handler auth as the enforcement point and any write path gated only at `Authenticated`.

## Consequences

- New `/api` routes are covered by default; forgetting a per-handler guard no longer opens a hole.
- A public route must be added to the whole-segment allowlist explicitly, or it is gated.
- The gate enforces the level; the role model it maps to lives in ADR-2010 — the two must stay
  consistent (WriteGraph ⇔ Editor+).
- Whole-segment matching means near-miss paths (`/api/admin-x`) are treated as distinct scopes,
  which is the intended behaviour, not a bug.

## Verification

Re-checked at `e0f8cd896`: `src/middleware/rbac_gate.rs:133` `required_level` uses
`has_segment_prefix` over `segments(path)` (`:60-66`); the public allowlist is whole-segment
(`:50`); `has_segment_prefix(&segs, &["api","admin"])` → `Admin` at `:146-147`; mutating methods
→ `Some(AccessLevel::WriteGraph)` at `:159-167`. `src/utils/auth.rs:41` `has_permission`
implements the lattice comparison that refuses WriteGraph to a ReadOnly (Viewer) level.
Governing doc: `docs/IDENTITY-authority-chain.md`.
