---
id: ADR-2086
title: "CI asserts release/production builds exclude the dev-auth cargo feature"
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any change to Dockerfile.production, Dockerfile.unified, scripts/{prod,dev}-entrypoint.sh, or the dev-auth-release-gate job in .github/workflows/ci.yml
repo: visionclaw
domain: SECURITY-profiles
---

# ADR-2086 — CI asserts release/production builds exclude the dev-auth cargo feature

## Context

Diagrams ES-09.5 and ES-10.2 exposed: `src/main.rs:169`
`enforce_release_env_hygiene()` is a no-op stub under
`#[cfg(any(debug_assertions, feature="dev-auth"))]`; the real boot-abort only
compiles when neither holds. ADR-2037 (`docs/adr/ADR-2037-production-build-excludes-dev-auth.md`,
`decision_status: proposed`, `implementation_status: none`, `verified_paths: []`)
already decided production/release images must exclude `dev-auth`, and records
that a mis-targeted pipeline could promote a dev-auth-featured release binary
silently — but ADR-2037 records the decision only, with no CI assertion
implementing it. This ADR implements that decision; it does not supersede
ADR-2037.

## Decision

`.github/workflows/ci.yml` gains a new **blocking** job,
`dev-auth-release-gate`, that fails the build if a production/release build
path would carry the `dev-auth` cargo feature. It is a hermetic shell/grep
step (no cargo invocation, no docker build, no network) that:

1. Greps `Dockerfile.production` and `Dockerfile.unified` for any `cargo
   build`/`cargo install` line containing `--release`, and fails if any such
   line names `dev-auth`.
2. Additionally fails if the literal string `dev-auth` appears anywhere in
   either Dockerfile at all — both files select cargo features directly in
   their build-time `RUN` lines with no `ARG FEATURES` plumbing that could
   carry `dev-auth` in from outside, so a whole-file check is the correct,
   simpler invariant matching how these files actually build.
3. Fails if `scripts/prod-entrypoint.sh` (the production container's runtime
   rebuild path, `cargo build --release --features gpu` at its line 9)
   references `dev-auth`.
4. Fails — as a **positive control** — if `dev-auth` is *not* found in
   `scripts/dev-entrypoint.sh` (the dev-only runtime rebuild path,
   `cargo build --release --features gpu,dev-auth`): its disappearance from
   there too would mean checks 1-3 pass vacuously (the feature vanished
   everywhere) rather than because it is correctly confined to dev.

The failure message names ADR-2037, explains what compiles in (`dev-session-token`
bypass, stubbed `enforce_release_env_hygiene`), and says exactly which three
files are the only place `dev-auth` may legitimately appear:
`scripts/dev-entrypoint.sh`, `scripts/rust-backend-wrapper.sh`, and the
dev-environment branch of `scripts/launch.sh`.

## Consequences

- Closes ADR-2037's implementation gap: production/release images now have a
  CI-enforced guarantee, not just a documented intention, that `dev-auth`
  never reaches them. ADR-2037's `implementation_status` can move from `none`
  to `complete` once this lands at a real commit (that edit belongs to
  ADR-2037's owner/the queen, not this record).
- The check is text-based, not a build-time symbol/feature-closure check on
  the emitted binary (the stronger alternative ADR-2037 also names). It runs
  in seconds and needs no CUDA toolchain, docker, or network, at the cost of
  trusting that the Dockerfiles' build lines are the only place features are
  selected — true today (verified: no `ARG FEATURES` exists in either
  Dockerfile) but a future refactor that introduces build-arg-driven feature
  selection into a *production* Dockerfile stage would need this gate
  extended, which `review_trigger` above covers.
- Cost: any future legitimate change to how these files select cargo features
  must keep this gate's assumptions in mind (see review_trigger).

## Verification

Commands run locally against the uncommitted working tree above commit
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` (HEAD at time of writing; must be
re-run at the landing commit):

1. Real tree, expect pass:
   ```
   for f in Dockerfile.production Dockerfile.unified; do
     grep -nE 'cargo (build|install)[^&]*--release' "$f" | grep -q 'dev-auth' && echo FAIL
     grep -q 'dev-auth' "$f" && echo FAIL
   done
   grep -q 'dev-auth' scripts/prod-entrypoint.sh && echo FAIL
   grep -q 'dev-auth' scripts/dev-entrypoint.sh || echo FAIL
   ```
   Result: no `FAIL` printed — `PASS: no production/release build path
   includes dev-auth (ADR-2037/ADR-2086).`

2. Deliberately broken input #1 — injected `,dev-auth` into
   `Dockerfile.production`'s `cargo build --release && \` line (copy in
   scratchpad, not the real file): both the `--release`-line grep and the
   whole-file grep detected it; gate would exit 1.

3. Deliberately broken input #2 — injected `,dev-auth` into
   `Dockerfile.unified`'s first `cargo build --release --features gpu && \`
   line (the `rust-deps`/`rust-builder` stage that feeds the production
   image's binary via `COPY --from=rust-builder`, copy in scratchpad): both
   the `--release`-line grep and the whole-file grep detected it; gate would
   exit 1. (An earlier design that only scanned text *after* the `FROM ... AS
   production` marker missed this — the binary-feeding stages sit textually
   before that marker — which is why the final check scans the whole file,
   not a stage slice.)

4. Deliberately broken input #3 — injected `,dev-auth` into
   `scripts/prod-entrypoint.sh`'s `cargo build --release --features gpu` line
   (copy in scratchpad): detected; gate would exit 1.

5. YAML validity: `node -e "require('js-yaml').load(fs.readFileSync('.github/workflows/ci.yml','utf8'))"`
   parsed successfully and listed all six jobs including the new
   `dev-auth-release-gate`, confirming the workflow file is still valid YAML
   after the edit.

All scratch copies used for the broken-input tests were deleted after use;
the real `Dockerfile.production`, `Dockerfile.unified`, and
`scripts/*-entrypoint.sh` were never modified by this verification.
