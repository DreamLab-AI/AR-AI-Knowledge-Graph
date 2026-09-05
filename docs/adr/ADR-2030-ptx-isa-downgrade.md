---
id: ADR-2030
title: PTX ISA is text-rewritten to .version 9.0 at build time
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [crates/visionclaw-gpu/build.rs]
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
writing the file back. When nvcc runs but exits unsuccessfully it falls back to bundled pre-compiled PTX
from a fixed set of paths; failure to launch nvcc panics before fallback. If neither compilation nor a fallback yields PTX it panics,
and it panics again if the resulting PTX file is empty. This records the chosen header rewrite rather than toolkit pinning. The build
rejects empty output, but its nonempty check does not establish a tested kernel.

## Consequences
The rewrite aims to support older-driver hosts. Driver acceptance and kernel
behaviour still require runtime evidence; failures can occur after the build. The costs: the rewrite is a brittle
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

## Closeout extension — 2026-09-04

CP-01/06/08. Owner remains jjohare with GPU/build/release maintainers. Implementation is partial against the original missing-toolchain and compatibility promises; historical live activation is retained. Six [isolated PTX-phase cases](../../../VisionFlow/docs/estate-review/evidence/ptx-build-probe.json) show absent nvcc panics before fallback, failed nvcc reaches fallback, invalid nonempty output passes and empty output fails. Existing 9.0 content produces a downgrade warning, and an invented two-digit minor exposes fixed-width splicing. Historical verification did not establish those stronger guarantees.

**Acceptance condition:** Test launch failure separately from compiler failure; validate version tokens, required symbols, fallback provenance, native/stub linking and host/device ABI identity. Record the selected runtime module and original/rewritten hashes, then exercise representative kernels on the intended driver with rollback. Reopen on toolkit/driver changes, fallback selection, native linking or runtime loader changes. See [build/runtime review](../../../VisionFlow/docs/estate-review/rendered-state.md#ptx-build-acceptance-and-loaded-artefact-identity). The fixture omits the native phase and uses a fake compiler; no CUDA or driver execution occurred.

## Acceptance progress — 2026-09-05

**Implemented.** The build-time policy is extracted to
`crates/visionclaw-gpu/src/ptx_policy.rs`, which is compiled **twice from one
source**: `include!`d into `build.rs` and declared as `pub mod ptx_policy` in the
library. A build script cannot depend on the crate it builds, which is why the
closeout had to extract a separate probe to test the phase at all — and nothing
guaranteed that probe still matched the script. Now the tests run against the
exact code the build executes. The module is `std`-only, so the include costs
nothing.

Each closeout finding is addressed:

| Finding | Fix |
|---|---|
| `nvcc` absent panicked *before* the fallback was consulted | `NvccOutcome::LaunchFailed` is distinct from `CompilerFailed`; **both** reach the fallback, with different diagnoses. Panicking on a missing toolkit defeated the entire purpose of the bundled PTX. |
| a successful compiler writing `NOT PTX` passed the non-empty gate | `validate_ptx` checks `.version`, `.target`, `.entry` **and required kernel symbols** — the last catches a compiler that succeeded against stale or partial source |
| the fixed 12-byte splice turned `.version 9.10` into `9.00` | `rewrite_ptx_version` locates the version token by span and rewrites exactly those characters, whatever their width. `9.00` is *lower* than either the original or the target — a silent downgrade past the intended floor |
| a "downgrade" warning was emitted on unchanged content | `VersionRewrite::Unchanged` is reported distinctly from `Rewritten` |
| nothing recorded which module was selected, or its identity | `PtxArtefact` records module, source path, provenance, resulting ISA and content tags **before and after** the rewrite; the build writes a manifest to `OUT_DIR` and exports `VISIONCLAW_PTX_MANIFEST` |

Provenance is three-valued — `Compiled`, `FallbackAfterLaunchFailure`,
`FallbackAfterCompilerFailure` — so a manifest line says not just that a fallback
was used but *why*. The panic when no fallback exists now names the diagnosis and
lists every path searched.

**Tests run.**

- `cargo test -p visionclaw-gpu --lib --no-default-features ptx_policy` — 17 pass:
  launch-vs-compiler classification and their distinct diagnoses; success implying
  neither fallback nor validity; fallback provenance per failure; two-digit-minor
  rewrite explicitly asserting `9.00` never appears; unchanged-not-downgraded for
  `9.0`/`8.7`/`1.0`; higher major and minor both downgrading; span-based token
  parsing with tab and multi-space separation; malformed and missing version as
  defects; `9.0` vs `9.10` display; `NOT PTX` and whitespace-only rejected; each
  structural directive required; required symbols checked by name; artefact tag
  divergence on rewrite and equality without.
- **Real toolkit build** — `cargo check -p visionclaw-gpu` (GPU feature on, nvcc
  12.9 present) completed with all 9 modules compiled, validated (including the
  required-symbol check on `visionclaw_unified`) and recorded. Manifest receipt:
  `docs/estate-closeout/2026-09-05/ptx-build-manifest.txt`. The installed toolkit
  emits `.version 8.8`, below the 9.0 target, so every module is correctly
  reported `Unchanged` with `original == rewritten` — no spurious downgrade
  warning, which is the closeout's fourth finding demonstrated on real output.

**Governed paths changed.** `crates/visionclaw-gpu/build.rs`,
`crates/visionclaw-gpu/src/ptx_policy.rs`, `crates/visionclaw-gpu/src/lib.rs`.

**Open — every runtime-side condition.** No driver module was loaded, no kernel
ran, and no rollback was exercised. `ptx_loader`'s *runtime* selection is
untouched: it still prefers modification-time-sorted build outputs, which is not
proof of source revision or ABI identity, and its own `validate_ptx` remains the
weaker three-substring check. Rewriting a declared ISA still does not prove the
instruction body is supported by the target driver. Implementation remains partial
against the original compatibility promise; the build phase is now tested and
recorded, the runtime phase is not.
