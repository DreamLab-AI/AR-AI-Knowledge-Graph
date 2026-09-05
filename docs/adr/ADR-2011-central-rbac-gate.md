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

## Closeout extension — 2026-09-04

CP-01/04/05/08. Owner remains jjohare with identity/runtime maintainers. The central gate is installed on /api, with explicit public-prefix exceptions and a report mode that forwards denials. The complete/live declaration remains scoped to that implementation, not unconditional enforcement. Release builds can activate report mode using the current-date acknowledgement; construction captures the mode without per-request expiry. The whoami handler inherits the stricter central admin-prefix requirement.

**Acceptance condition:** Verify caller/target authority under concurrent changes, removal versus effective revocation, immutable/recoverable audit outcomes and response-loss retries. Exercise the composed route policy, public-prefix methods, whoami, missing identity services and report-mode lifecycle. Bind accepted behaviour to the release profile and effective process settings. Reopen on role fallback, transaction, prefix, middleware ordering or report-mode changes. See the [authority review](../../../VisionFlow/docs/estate-review/role-authority.md) and [source receipt](../../../VisionFlow/docs/estate-review/evidence/rbac-snapshot.json). This pass is source-only: no role mutation, HTTP request or race test ran.

## Acceptance progress — 2026-09-05

**Implemented.** The shared half of the ADR-2010/2011 acceptance.
`src/middleware/rbac_gate.rs` no longer carries its own copy of the report-mode
dated-acknowledgement rule: `GateMode::from_env` and `report_acknowledged` now
call `config::security_profile::{report_mode_requested, report_mode_acknowledged}`,
so the gate and the new boot-time assertion cannot drift apart. The
construction-time capture is unchanged, but reaching it in a production artefact
is now impossible: the ADR-2038 assertion rejects `RBAC_GATE_MODE=report` before
the listener binds, acknowledged or not.

The caller-authority and removal-versus-revocation work is recorded under
ADR-2010; the central gate is the surface that composes it.

**Tests.** `cargo test --lib --no-default-features rbac` — 13 passed, 0 failed
(existing gate tests, unchanged behaviour). `cargo test --lib --no-default-features
security_profile` — 29 passed, 0 failed, covering the shared acknowledgement
rule including the date rollover.

**Receipts.** `adr-2010-2011-rbac-caller-freshness.txt`,
`adr-2012-2038-security-profile.txt`.

**Remains open.** The composed route policy, public-prefix methods, whoami and
missing identity services are still not exercised through real routes.
