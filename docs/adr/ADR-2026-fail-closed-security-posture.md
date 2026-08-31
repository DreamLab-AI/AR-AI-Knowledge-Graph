---
id: ADR-2026
title: Fail-closed security posture — flag absence widens nothing, release aborts on promoted dev-auth
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [src/middleware/rbac_gate.rs, src/main.rs, src/services/role_store.rs]
owner: jjohare
review_trigger: any new security-relevant env flag, or a request to soften the release boot-abort to a warning
repo: visionclaw
domain: SECURITY-profiles
lineage: legacy ADR-011 (no log-and-allow, fail closed), ADR-142 (fail-closed flag defaults), T2-auth-gating V3 resolution
---

# ADR-2026 — Fail-closed security posture — flag absence widens nothing, release aborts on promoted dev-auth

## Context

Two failure modes leak access. First, a security-relevant env flag that defaults
open means a forgotten var silently widens the surface. Second, dev-auth bypass
knobs (`SETTINGS_AUTH_BYPASS`, `ALLOW_INSECURE_DEFAULTS`, `VISIONCLAW_DEV_MODE`,
`--allow-skip-auth`) are `#[cfg]`-stripped from release builds, so they cannot
grant bypass — but their presence at deploy time signals a dev config was
promoted to production and must not pass silently. Legacy ADR-011 forbade
log-and-allow; ADR-142 set the flag lattice; the T2-auth-gating V3 resolution
made env promotion a hard boot failure. This merges those two facets.

## Decision

The absence of any security-relevant flag grants nothing more. `RBAC_PUBLIC_READS`
and `RBAC_ALLOW_OWNERLESS` both `unwrap_or(false)`; an owner-less store aborts
boot with `PermissionDenied` unless `RBAC_ALLOW_OWNERLESS=1` is set explicitly;
every role-lookup `Err` resolves down to `Viewer`, never up. In release builds,
`enforce_release_env_hygiene()` aborts at boot: `--allow-skip-auth` in argv exits
`1`; any of the three suspect env vars, or `NODE_ENV=development`+`DOCKER_ENV`
together, exits `2`. This forecloses defaulting any auth gate open and forecloses
booting a release binary that carries dev-config fingerprints.

## Consequences

- A misconfigured or empty environment is safe by construction; there is no
  "we forgot the flag, so it opened up" path.
- Operators who genuinely run single-operator/owner-less must opt in visibly
  (`RBAC_PUBLIC_READS=1`, `RBAC_ALLOW_OWNERLESS=1`) — friction is the point.
- A release image that inherits a stray `NODE_ENV=development` will refuse to
  boot even though it is not actually vulnerable; the fix is to clean the env,
  not the binary. Exit codes `1`/`2` must be understood by orchestration.
- Role-lookup errors degrade to `Viewer`, which overlaps the RBAC record
  ADR-2010 (cross-ref) rather than being re-litigated here.

## Verification

Re-checked at `e0f8cd896`: `public_reads_enabled` `unwrap_or(false)` at
`src/middleware/rbac_gate.rs:121-128`; owner-less `PermissionDenied` return at
`src/main.rs:729-739` (with `RBAC_ALLOW_OWNERLESS` opt-out at 717-719); role
`Err(e) => UserRole::Viewer` at `src/services/role_store.rs:204-208`;
`enforce_release_env_hygiene` argv `exit(1)` and `SUSPECT_ENVS`/`NODE_ENV+DOCKER_ENV`
`exit(2)` at `src/main.rs:117-163`, gated `#[cfg(not(any(debug_assertions,
feature = "dev-auth")))]` with a no-op dev stub. All four constructs are live.
