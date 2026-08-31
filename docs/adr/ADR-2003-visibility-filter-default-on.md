---
id: ADR-2003
title: The pubkey visibility filter defaults ON
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e78e958fa
owner: jjohare
review_trigger: introduction of per-node visibility metadata at ingest, or a deployment relying on anonymous access to private nodes
repo: visionclaw
---

# ADR-2003 — The pubkey visibility filter defaults ON

## Context

The fail-closed private-node drop filter (`visibility_filter.rs`) was fully
implemented but inert: `PUBKEY_VISIBILITY_FILTER` defaulted off and no
deployment set it, so unauthorised clients received private node positions by
default. The 2026-08-31 review sense-check rated this the highest-severity live
finding. Public nodes are unaffected by the filter, so the flip is
behaviour-neutral for all-public deployments.

## Decision

Absence of `PUBKEY_VISIBILITY_FILTER` means **filtered** (secure by default).
Opt-out requires an explicit falsy value (`0`/`false`/`off`/`no`,
case-insensitive); unrecognised values fail safe to ON. Parsing is a pure
function (`parse_visibility_flag`) behind a `OnceLock` wrapper — the posture is
captured at first use and runtime env mutations are ignored. The compose file
states the posture explicitly (`${PUBKEY_VISIBILITY_FILTER:-1}`).

## Consequences

- Deployments with private or owner-scoped nodes are protected without any
  configuration; this is the invariant "absence of a security flag must widen
  nothing" (`docs/SECURITY-profiles.md`).
- Anonymous/non-owner sessions in deployments carrying private-marked nodes
  will now see those nodes drop — intended, but a visible migration change.
- `RBAC_PUBLIC_READS=1` + `PUBKEY_VISIBILITY_FILTER=0` is a recorded illegal
  combination (full disclosure).

## Verification

Pure truth-table test `parse_visibility_flag_defaults_on_and_honours_opt_out`
green at `e78e958fa`; both read sites (`position_updates.rs`, `types.rs:332`)
share the single helper (grep confirms no residual `unwrap_or(false)` reader);
compose env block carries the explicit default.
