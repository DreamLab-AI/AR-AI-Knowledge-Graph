---
id: ADR-2038
title: "Boot-time deployment-profile assertion with illegal-combination abort"
date: 2026-08-31
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit:
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
