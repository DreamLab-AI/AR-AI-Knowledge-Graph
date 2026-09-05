---
id: ADR-2038
title: "Boot-time deployment-profile assertion with illegal-combination abort"
date: 2026-08-31
decision_status: proposed
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: adoption of a production deployment, or any change to the profile env vars (RBAC_PUBLIC_READS, PUBKEY_VISIBILITY_FILTER, RBAC_DEFAULT_ROLE)
repo: visionclaw
domain: SECURITY-profiles
lineage: ADR-2027 (three named profiles, documented-prose gap), ADR-2003 (visibility-filter illegal combination), ADR-2010 (editor-default first-contact write), ADR-2026 (fail-closed posture the assertion enforces)
---

# ADR-2038 — Boot-time deployment-profile assertion with illegal-combination abort

## Context

ADR-2027 ratified three profiles but admits they are "documented prose, **not**
machine-selected — nothing at boot asserts the running env matches a named
profile," so flag drift is possible. The shipped `docker-compose.unified.yml`
realises demo-open (`RBAC_PUBLIC_READS:-1`). ADR-2003 records
`RBAC_PUBLIC_READS=1` + `PUBKEY_VISIBILITY_FILTER=0` as an illegal
full-disclosure combination with no runtime rejection. ADR-2010's shipped editor
default grants first-contact write. The `RBAC_DEFAULT_ROLE=viewer` lever already
landed (commit 8e78a9d19); the missing piece is the boot assertion that binds
these flags to a named, validated posture — not another lever.

## Decision

At boot the backend MUST resolve the active security profile from env — exactly
one of **demo-open**, **single-tenant**, **multi-user-locked** — log the resolved
profile name and its effective flag set, and ABORT with a non-zero exit when the
env is not a supported posture. Abort is mandatory in two cases: (1) the recorded
illegal full-disclosure combination `RBAC_PUBLIC_READS=1` +
`PUBKEY_VISIBILITY_FILTER=0` (ADR-2003), which no profile permits; and (2) any
env whose flags do not match the named invariants of the resolved profile in
`docs/SECURITY-profiles.md`. A production selector (any deployment not explicitly
opting into demo-open or single-tenant) MUST default to the **multi-user-locked**
profile. The assertion runs before the server binds a listener, so an
unsupported or leaky configuration never serves a request. Cross-refs: ADR-2003
(illegal combination), ADR-2010 (editor default the locked profile overrides),
ADR-2026 (fail-closed defaults the assertion enforces), ADR-2027 (the profiles
this asserts).

## Consequences

- An unsupported or leaky configuration fails fast at boot instead of running and
  silently serving over-disclosed data; the illegal full-disclosure combination
  becomes unbootable rather than merely undocumented.
- Operators gain a logged, machine-asserted statement of which profile is live —
  provable in incident review, closing the ADR-2027 drift gap.
- The shipped demo-open compose must now declare its profile explicitly; a bare
  or ambiguous production env resolves to multi-user-locked and will abort if its
  flags contradict that posture, so demo deployments must opt in on purpose.
- Follow-on work: a profile resolver + validator module, a boot-sequence hook
  ahead of listener bind, and a table of per-profile invariant checks kept in
  lockstep with `docs/SECURITY-profiles.md`.

## Verification

None yet — this record is `proposed` / `implementation_status: none` /
`activation_status: inactive`. No boot-time profile assertion exists (grep for a
resolver/abort path returns nothing). On implementation, verification MUST record:
the resolver source path and line, an abort test proving
`RBAC_PUBLIC_READS=1` + `PUBKEY_VISIBILITY_FILTER=0` yields a non-zero exit before
listener bind, and a production-default test proving an unspecified env resolves
to multi-user-locked. Set `verified_commit` and `verified_paths` at that point.

## Closeout extension — 2026-09-04

CP-01/04/06/08. Owner remains jjohare with release/authentication maintainers. Proposed/none/inactive is retained for the complete profile assertion. Existing development-variable hygiene and individual role/filter parsers do not jointly assert a named deployment profile. Extend the acceptance matrix to include RBAC report mode, whose construction-time acknowledgement can enable non-enforcement.

**Acceptance condition:** Bind image digest, source, feature closure and effective profile to a receipt. Test production rejection before listener bind, including forbidden variables set to zero, and prevent promotion of a dev-auth artefact. Exercise full REST and WebSocket paths, report-mode interaction, network reachability and sentinel attribution separately from helper parsing. Preserve the distinction between the peer-agnostic full bypass and loopback dev-token mechanism. Reopen on build features, boot sequencing, bypass branches or profile policy. See the [review](../../../VisionFlow/docs/estate-review/role-authority.md#development-bypass-and-release-identity), [reproducer](../../../VisionFlow/docs/estate-review/evidence/dev-auth-probe.py) and [receipt](../../../VisionFlow/docs/estate-review/evidence/dev-auth-probe.json). No full image, listener, HTTP or headset execution ran.

### Implementation status update — 2026-09-05 (ADR-2038)

`implementation_status` moves `none` → **complete** and `activation_status`
`inactive` → **staged**. `decision_status` deliberately stays `proposed`: the
implementation exists but has never landed in a commit, and promoting the
decision on the strength of untracked code would make this record assert
something a clean checkout cannot reproduce.

**The implementation.** `src/config/security_profile.rs` (1038 lines) is a pure
evaluator over `(EnvSnapshot, BuildIdentity, today)`:
`assert_effective_profile_or_exit` (`:528`) is called from `src/main.rs:873`,
inside the block at `:868-878`, **before** `HttpServer::new` (`:893`) and `.bind()`
(`:1146`). A production artefact with any finding prints each one and
`std::process::exit(2)`; a development build logs and continues
(`may_bind_listener`, `:358`). The boot receipt — `summary()` plus
`observed_flags` — is logged at `src/main.rs:879-883`.

`DeploymentProfile` (`:79`) implements ADR-2027's three profiles with
`expected_flags()` (`:120-136`) reproducing the table in
`docs/SECURITY-profiles.md` exactly, and `VISIONCLAW_SECURITY_PROFILE` (`:55`) is
the selector. Findings cover the forbidden dev variables (`FORBIDDEN_DEV_VARS`,
`:60-66` — a superset of ADR-2026's `SUSPECT_ENVS`, adding `DEV_AUTH_LOOPBACK`),
the `NODE_ENV=development` + `DOCKER_ENV` fingerprint, `--allow-skip-auth` in
argv, a `dev-auth` artefact (ADR-2037), report mode (ADR-2012), declared-profile
drift (ADR-2027), an unknown declared profile, and — added by **ADR-2043** — the
ADR-2003 full-disclosure flag pair.

**Why `staged` and not `live`.** Both the module and its call site are
**uncommitted**: `git status` reports `src/config/security_profile.rs` as
untracked (`??`), and `git show HEAD:src/main.rs | grep -c
assert_effective_profile_or_exit` returns `0`. A clean checkout of
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` therefore contains neither, and
re-verifying this record from that SHA will not find the assertion. `verified_commit`
is set to that SHA per the Phase 2 convention that verification ran on the
uncommitted working tree above it; **both `verified_commit` and `verified_paths`
must be restored at the landing commit**, at which point `decision_status` can
move to `accepted` and `activation_status` to `live`.

**Not implemented.** The Decision text says a production selector defaults to
`multi-user-locked`. The code implements no implicit profile: an undeclared
deployment whose flags match no ratified profile is classified `Unnamed` and
still binds. ADR-2043 closes the highest-risk case (the full-disclosure pair)
unconditionally, but the general gap stays open and is recorded in
`docs/SECURITY-profiles.md` and the estate lead's ES-10.7.

**Provenance.** The implementation predates this remediation pass — the file's
mtime is 2026-09-05 13:10 and it has never been committed — and its author is not
established. It was found during Phase 1 diagram work (VC-09.4, VC-09.5, VC-09.6)
rather than written for it.

## Acceptance progress — 2026-09-05

**Implemented.** This record's subject now exists:
`src/config/security_profile.rs`, called from `src/main.rs` before
`HttpServer::new`/`bind`.

The gap the closeout named — "existing development-variable hygiene and
individual role/filter parsers do not jointly assert a named deployment profile"
— is closed by a single pure function,
`evaluate_effective_profile(&EnvSnapshot, BuildIdentity, today) ->
EffectiveProfile`, which joins: forbidden development variables (by presence,
including `DEV_AUTH_LOOPBACK`), the `NODE_ENV=development` + `DOCKER_ENV`
container fingerprint, `--allow-skip-auth` in argv, the dev-auth artefact
identity, **RBAC report mode** (the extension this record asked for — its
construction-time acknowledgement can otherwise enable non-enforcement), and
drift from the declared ADR-2027 profile. `EffectiveProfile` carries the
observed value of every profile flag as the boot receipt, and
`may_bind_listener()` is the decision: a production artefact must have no
findings; a development build always may.

Because the function takes an injected environment snapshot and date rather than
reading the process environment and clock, the entire matrix runs as ordinary
unit tests with no global state and no ordering hazard.

**Tests.** `cargo test --lib --no-default-features security_profile` — 29
passed, 0 failed. Missing versus zero-valued variables, report-mode construction
and date rollover, restart re-evaluation, artefact identity, argv, profile
drift, classification, and the binding decision are each covered.
The gate shares this module's acknowledgement rule (ADR-2011), so the two cannot
drift: `cargo test --lib --no-default-features rbac` — 13 passed, 0 failed.

**Receipts.** `docs/estate-closeout/2026-09-05/adr-2012-2038-security-profile.txt`.

**Remains open.** Image digest and feature closure are not bound to the receipt.
`assert_effective_profile_or_exit` is not exercised end to end (it calls
`process::exit`); only the pure evaluator it wraps is tested. Status left
`none`/`inactive`: the assertion exists but no deployment has run it.
