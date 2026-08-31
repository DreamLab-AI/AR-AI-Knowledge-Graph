---
id: ADR-2010
title: Four-tier pubkey-bound RBAC lattice (Editor default, fail-closed to Viewer) with atomic role mutations
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []                   # legacy ADR-142/ADR-094 distilled — not in this tree; see lineage
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: adoption of a multi-user-locked deployment, or any change to default_authenticated() or the last-Owner guard
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: Distils legacy ADR-142 (multi-user RBAC bound to NIP-98 pubkeys), ADR-131 §3 (branch-only enterprise_auth rejected), ADR-094 (admin-pubkey privilege intent); hardened per Codex round-2 (three checks folded into one transaction).
---

# ADR-2010 — Four-tier pubkey-bound RBAC lattice (Editor default, fail-closed to Viewer) with atomic role mutations

## Context

Authority must attach to the NIP-98 pubkey (ADR-142), not to a branch build flag (ADR-131 §3
rejected `enterprise_auth`). Pre-RBAC `main` granted every authenticated user read+write-graph;
demoting them silently on RBAC rollout would break existing users. Naive check-then-write role
mutations carry TOCTOU risk: two concurrent grants could bypass the `can_assign` privilege check
or the last-Owner guard, permitting escalation or sole-Owner lockout.

## Decision

Persisted per-pubkey roles form an `Owner(4) > Admin(3) > Editor(2) > Viewer(1)` lattice.
An authenticated-but-unassigned pubkey resolves to `Editor` (`default_authenticated()`), preserving
the pre-RBAC grant; any lookup error — including an unparseable stored role — **fails closed to
`Viewer`**, never up. `assign_checked`/`revoke_checked` fold the current-role read, the
`can_assign` privilege check, and the last-Owner guard into a single SQLite transaction, so
escalation and sole-Owner lockout are structurally impossible (TOCTOU-free). This forecloses
role logic scattered across handlers and any code path that reads then writes a role non-atomically.

## Consequences

- New authenticated pubkeys can write the graph immediately (Editor); a locked-down deployment
  must explicitly assign `Viewer` — flagged for reconsideration under a multi-user-locked profile.
- A role-store outage denies rather than grants (Viewer), trading availability for safety.
- Role mutations run inside a transaction, so a busy store can contend; the guarantee is atomicity,
  not throughput.
- Also absorbs the SECURITY-profiles Editor-default facet — cross-ref SECURITY ADR-2027.

## Verification

Re-checked at `e0f8cd896`: `src/models/rbac.rs:68-69` `default_authenticated()` returns `Editor`
with the rationale comment `:62-66`; `src/services/role_store.rs:194` `effective_role` returns
`default_authenticated()` on an empty row (`:201`) and `Viewer` on `Err` (`:206`);
`assign_checked` (`:243`) enforces `can_assign` at `:266`/`:272` and `LastOwner` at `:287`;
`revoke_checked` (`:310`) enforces `can_assign` `:330` and `LastOwner` `:342`, all inside one
transaction folded through `TxOutcome`. `src/utils/auth.rs:60` `resolve_access_level` maps roles
to access levels. Governing doc: `docs/IDENTITY-authority-chain.md`.
