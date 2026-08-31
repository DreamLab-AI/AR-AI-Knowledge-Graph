---
id: ADR-2030
title: PTX ISA is text-rewritten to .version 9.0 at build time
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: host driver gains support for a newer PTX ISA, or nvcc changes its .version emission
repo: visionclaw
domain: GPU-wire-abi
lineage: legacy ADR-070 (CUDA integration hardening — toolkit/driver ISA skew on CachyOS); distils the project-memory PTX-version-mismatch fingerprint.
---

# ADR-2030 — PTX ISA is text-rewritten to .version 9.0 at build time

## Context
CUDA toolkit 13.x emits PTX with `.version 9.x`, but some host drivers (the CachyOS
build environment among them) only accept ISA up to 9.0 and reject anything newer at
load time. Pinning the whole toolkit to an older version to hold the ISA down was the
alternative, but that forfeits newer nvcc and its codegen. The build also has to
tolerate machines with no nvcc at all, where a bundled pre-compiled PTX is the only
option. An empty or missing PTX must fail loudly rather than ship a silently dead kernel.

## Decision
`build.rs` post-processes every compiled `.ptx`: it finds the leading `.version 9.`
token and, if it is not already `.version 9.0`, splices `.version 9.0` in place before
writing the file back. When nvcc is absent it falls back to a bundled pre-compiled PTX
from a fixed set of paths; if neither compilation nor a fallback yields PTX it panics,
and it panics again if the resulting PTX file is empty. This forecloses pinning the
toolchain to control the ISA and forecloses shipping an untested/empty kernel.

## Consequences
Builds run against current nvcc yet load on the older-driver hosts, and a broken build
fails at compile time, not at first kernel launch. The costs: the rewrite is a brittle
string splice keyed on the exact `.version 9.` prefix (a differently formatted or
non-9.x header would slip through), it hard-codes 9.0 as the floor so a genuinely newer
required ISA would need a code change, and the bundled-PTX fallback can mask a missing
toolchain. Compliance means any new kernel goes through this same `build.rs` path.

## Verification
Re-checked at e0f8cd896: `crates/visionclaw-gpu/build.rs` — locates `.version 9.`
(line 165), compares against `.version 9.0` and splices the replacement when different
(lines 168–171), emits a `cargo:warning` on downgrade (lines 172–176); the bundled-PTX
fallback (lines 137–151) and the twin panics on empty/missing PTX (lines 186–187) are
present. The rewrite, fallback and hard-fail behaviours all match the record.
