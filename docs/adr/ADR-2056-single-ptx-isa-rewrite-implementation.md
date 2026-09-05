---
id: ADR-2056
title: Route every PTX ISA downgrade through the single span-parsed rewrite
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: Any new PTX acquisition path, or any change to the .version rewrite
repo: visionclaw
---

# ADR-2056 — Route every PTX ISA downgrade through the single span-parsed rewrite

## Context

ADR-2030 established a build-time PTX ISA downgrade: `crates/visionclaw-gpu/src/ptx_policy.rs`
parses the `.version` directive's span and rewrites it so emitted PTX loads on a driver
older than the toolkit that produced it. A second, independent implementation existed at
runtime — `downgrade_ptx_isa_if_needed` in `ptx_loader.rs` — using `String::find(".version ")`
plus `replacen`, first match only. That is the same fixed-window, first-substring approach
ADR-2030 replaced, reintroduced one layer down. `GPU-wire-abi.md`'s own PTX closeout already
noted that "runtime loading separately selects and rewrites PTX; its structural substring
check is not driver or required-symbol validation". Diagram VC-12.5 carried the DIVERGENCE.

The obvious remediation — delete the runtime copy — turned out to be wrong, and
investigation before deletion is why. Two PTX acquisition paths never pass through
`build.rs`/`ptx_policy` at all: the checked-in pre-shipped `.ptx` files under
`crates/visionclaw-gpu/src/ptx/` and legacy `src/utils/ptx/`, reachable via
`load_precompiled_ptx`'s fallback scan; and the runtime `nvcc` fallback
(`compile_ptx_fallback_sync_module`). Deleting the runtime downgrade would have left those
two paths loading un-downgraded PTX — reintroducing the bug ADR-2030 fixed, for the exact
cases where the build-time policy cannot help.

## Decision

There is exactly one implementation of the `.version` rewrite: the span-parsed
`ptx_policy::rewrite_ptx_version`. Every path that can produce or load PTX uses it.

The runtime entry point `downgrade_ptx_isa_if_needed` is retained as an entry point but
carries no rewrite logic of its own: it determines the driver's maximum supported ISA and
delegates the rewrite to `ptx_policy`. Its companion `detect_max_ptx_isa` is retained
because it has a genuine runtime-only duty the build-time constant cannot serve — querying
the installed driver — and a build-time `TARGET_PTX_ISA` cannot know the driver on the
machine the artefact eventually runs on.

A substring scan is not an acceptable substitute for span parsing anywhere in this
codebase, including in a path that "only" handles pre-shipped artefacts.

## Consequences

Build-time and runtime downgrades can no longer disagree, because there is one rewrite to
disagree about. The pre-shipped and runtime-`nvcc` paths gain the correct span-parsed
behaviour they never had.

`ptx_loader` now depends on `ptx_policy`, which is the right direction (loader depends on
policy, not the reverse) but does couple the two modules; the review trigger exists so that a
future third acquisition path is routed through the same rewrite rather than growing a third
implementation.

This ADR does not address the other half of the PTX closeout's concern: the loader still does
not perform driver capability or required-symbol validation on the module it selects. That
remains open and is out of scope here.

## Verification

`cargo check -p visionclaw-gpu` — clean, exit 0.

`cargo test -p visionclaw-gpu --lib ptx_loader` — **27 tests pass**, including
`ptx_with_no_version_directive_is_returned_unchanged` and
`ptx_version_not_downgraded_when_within_max`, confirming behaviour parity across the
delegation change rather than merely that it compiles.

`nvcc` **is** present in this container, so `build.rs` exercised the real build-time path
rather than its fallback: all 9 CUDA kernels compiled via `ptx_policy` at `isa=8.8`, which
needed no rewrite on this GPU. The rewrite path itself is therefore covered by the unit
tests rather than by this build.

The two un-policied acquisition paths were established by reading `load_precompiled_ptx`'s
fallback scan and `compile_ptx_fallback_sync_module` before any deletion, and are the reason
the runtime entry point was retained rather than removed.

**Tracking caveat:** `crates/visionclaw-gpu/src/ptx_policy.rs`, which this ADR relies on, is UNTRACKED in the owner's working
tree and absent from `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` — a clean checkout of that SHA
will not contain it. The landing commit must add it, or this ADR describes a file that does not
exist in the repository.

Verification ran on the uncommitted working tree above the recorded SHA; `verified_paths` is
empty for that reason.
