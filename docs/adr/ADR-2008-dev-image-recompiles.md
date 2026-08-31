---
id: ADR-2008
title: The development image recompiles the Rust backend on container start
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [scripts/dev-entrypoint.sh, docker-compose.unified.yml]
owner: jjohare
review_trigger: a dev-loop turnaround that makes on-start compilation intolerable, or a move to pre-baked dev binaries by default
repo: visionclaw
domain: BASELINE-architecture
lineage: No dedicated legacy ADR; distils the dev/prod build-path divergence (project_dev_prod_build_sync.md).
---

# ADR-2008 — The development image recompiles the Rust backend on container start

## Context

The project mount is a host bind-mount: developers edit source on the host and
expect changes to take effect without an image rebuild. A pre-baked binary in
the image would silently run stale code against edited source. The dev and prod
build paths therefore diverge deliberately. Lineage: the dev/prod build-path
divergence note (`project_dev_prod_build_sync.md`).

## Decision

The dev container (`target: development`) rebuilds on entry with
`cargo build --release --features gpu,dev-auth`, so host bind-mount edits take
effect on restart with no image rebuild, and it **exits non-zero on build
failure** rather than serving a stale binary. The pre-baked binary is opt-in via
`SKIP_RUST_REBUILD=true`. Production pre-compiles in the image build and does not
recompile on start.

## Consequences

- Edit-on-host, restart-container is the dev loop; no `docker build` per change.
- Container start pays a full `--release` compile (minutes) unless
  `SKIP_RUST_REBUILD=true` — the cost of never running stale code by accident.
- A broken tree fails the container loudly at boot instead of masking the error
  behind an old binary.

## Verification

`scripts/dev-entrypoint.sh` guards the rebuild on `SKIP_RUST_REBUILD` (~:103),
runs `cargo build --release --features gpu,dev-auth` (~:108) and `exit 1`s on
failure (~:113). `docker-compose.unified.yml` sets `target: development` (:52)
and `BUILD_TARGET: ${BUILD_TARGET:-development}` (:55), with the production stage
distinct (`BUILD_TARGET: production`, :157). Verified at `e0f8cd896`.
