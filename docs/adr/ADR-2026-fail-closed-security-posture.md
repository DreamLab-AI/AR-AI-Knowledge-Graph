---
id: ADR-2026
title: Fail-closed security posture — flag absence widens nothing, release aborts on promoted dev-auth
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: f326a3b1172df4fea8183e6a4344d3f55c575013
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
every role-lookup `Err` resolves down to `Viewer`, never up. In non-debug builds without dev-auth,
`enforce_release_env_hygiene()` aborts at boot: `--allow-skip-auth` in argv exits
`1`; any of the three suspect env vars, or `NODE_ENV=development`+`DOCKER_ENV`
together, exits `2`. This forecloses defaulting any auth gate open and forecloses
booting a release binary that carries dev-config fingerprints.

## Consequences

- A misconfigured or empty environment is safe by construction; there is no
  "we forgot the flag, so it opened up" path.
- Operators who genuinely run single-operator/owner-less must opt in visibly
  (`RBAC_PUBLIC_READS=1`, `RBAC_ALLOW_OWNERLESS=1`) — friction is the point.
- A non-debug image without dev-auth that inherits both `NODE_ENV=development`
  and `DOCKER_ENV` will refuse to
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

## Closeout extension — 2026-09-04

CP-01/04/08. Owner remains jjohare with authentication/release maintainers. The scoped default parsers, ownerless boot refusal and non-debug/no-dev-auth hygiene are present. Historical complete/live declarations are retained for those mechanisms, not universal production posture. The release hygiene function is a stub when dev-auth is compiled, role errors resolve to Viewer rather than denying all reads, and the unassigned-signer default remains Editor unless narrowed.

**Acceptance condition:** Define the effective profile across feature set, all bypass/report controls, role fallbacks, peer/proxy handling and public route exceptions. Test missing versus zero-valued variables, report-mode construction/date rollover/restart, ownerless/error paths and production artefact promotion. Bind configuration and binary identity to the same pre-listener acceptance receipt. Reopen on any security-relevant setting, build feature, route exception or fallback. See the [profile review](../../../VisionFlow/docs/estate-review/role-authority.md#profile-claims-and-effective-policy) and [source receipt](../../../VisionFlow/docs/estate-review/evidence/security-profile-snapshot.json). The prior nine helper cases retain matching source hashes; no new live profile or network test ran.

## Acceptance progress — 2026-09-05

**Implemented.** The fail-closed posture gains a boot-time enforcement point.
`enforce_release_env_hygiene()` covered three suspect variables plus the argv
flag; `src/config/security_profile.rs` (see ADR-2038) extends that to the whole
effective profile and runs it **before the listener binds**.

Two gaps in the original hygiene check are closed: `DEV_AUTH_LOOPBACK` — the
ADR-2012 dev-token runtime opt-in — is now in the forbidden set, and
`RBAC_GATE_MODE=report` is now a rejection in a production artefact, which the
env-hygiene list did not cover at all. The presence-not-truthiness rule the ADR
already relied on is made explicit and tested: `SETTINGS_AUTH_BYPASS=0` is a
rejection, not a disabled feature.

**Tests.** `cargo test --lib --no-default-features security_profile` — 29
passed, 0 failed.

**Receipts.** `docs/estate-closeout/2026-09-05/adr-2012-2038-security-profile.txt`.

**Remains open.** The boot abort's exit code and message are asserted only in
the pure evaluator, not by running a binary; `assert_effective_profile_or_exit`
itself is not exercised end to end.
