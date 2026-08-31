---
id: ADR-2037
title: "Production release images are built without the dev-auth cargo feature, asserted in CI"
date: 2026-08-31
decision_status: proposed
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit:
verified_paths: []
owner: jjohare
review_trigger: any change to the production Dockerfile build line, the dev-auth feature gates, or enforce_release_env_hygiene
repo: visionclaw
domain: SECURITY-profiles
---

# ADR-2037 — Production release images are built without the dev-auth cargo feature, asserted in CI

## Context

`src/main.rs:169` `enforce_release_env_hygiene()` is a no-op stub under
`#[cfg(any(debug_assertions, feature="dev-auth"))]`; the real boot-abort only compiles when
neither holds. ADR-2012's `Bearer dev-session-token` fence is likewise dev-auth-gated. ADR-2008
records the DEV image building `cargo build --release --features gpu,dev-auth` — a `--release`
binary that nonetheless carries the hygiene abort stubbed out AND the bypass fence's compile-time
gate open. No record currently forbids promoting such a dev-auth-featured release binary to
production, so a mis-targeted pipeline could ship stubbed hardening silently.

## Decision

Production and release images MUST be compiled without the `dev-auth` cargo feature; the release
build line carries only production features (e.g. `--features gpu`), never `dev-auth`. A CI or
image-build assertion MUST verify the shipped binary contains neither the
`enforce_release_env_hygiene` no-op stub nor the dev-session-token bypass fence — via a build-time
symbol/feature check on the emitted binary, or a boot self-test proving the hygiene abort is the
real (non-stub) implementation. A release artefact that fails this assertion fails the build; it is
never published. The dev image is exempt and retains `dev-auth` for local work.

## Consequences

- The dev image stays as-is; local development keeps the auth shortcut and the stubbed hygiene.
- The release pipeline gains a gate: a mis-built binary (dev-auth leaked into a release image)
  fails CI rather than silently shipping with the boot-abort stubbed and the bypass gate open.
- Closes the promotion gap between ADR-2008 (dev image recompiles) and ADR-2012 / ADR-2026
  (fences and boot-abort that only bite when dev-auth is absent) — those defences now have a
  build-time guarantee that release binaries actually compile them in.
- Cost: the release pipeline must add and maintain the assertion; a feature-set change to the
  production build line now requires updating this gate in lockstep.

## Verification

None yet — this record is `proposed` / `implementation_status: none`. Implementation lands the CI
or image-build assertion (symbol/feature check or boot self-test) and, once green, updates
`verified_commit` and `verified_paths` to the build config and check that were inspected.
Cross-ref ADR-2008 (dev image recompiles), ADR-2012 (dev bypass triple-gated), ADR-2026
(fail-closed boot-abort). Governing doc: `docs/SECURITY-profiles.md`.
