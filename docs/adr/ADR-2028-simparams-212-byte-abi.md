---
id: ADR-2028
title: SimParams is a size-locked 212-byte repr(C) raw-copy ABI, grown tail-append only
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [src/models/simulation_params.rs, crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu]
owner: jjohare
review_trigger: any new SimParams field, or a driver/toolkit change altering the 212-byte size
repo: visionclaw
domain: GPU-wire-abi
lineage: legacy ADR-138 (flat repr(C) SimParams + dual static_asserts), ADR-141 (grew 180→212 in lockstep), ADR-098 D2 (constraints ride a separate buffer, not a struct field); ADR-031/ADR-061 wire sizes stale.
---

# ADR-2028 — SimParams is a size-locked 212-byte repr(C) raw-copy ABI, grown tail-append only

## Context
The Rust host and the CUDA kernel share simulation parameters every physics tick.
There is no serialisation layer: the struct is copied device-ward as raw bytes.
That makes the memory layout itself the wire contract, and any drift between the
two declarations is silent memory corruption rather than a caught error.
Four shipping clients already speak the current byte prefix (ADR-138/ADR-141).
Successive features (FA2/LinLog, DAG radial bias, the ADR-141 layout blocks)
needed to grow the struct without breaking those clients. Constraints deliberately
do not live in the struct — they ride a separate device buffer (ADR-098 D2).

## Decision
SimParams is a flat `#[repr(C)]` struct declared once in Rust and once in CUDA,
transferred by raw POD device copy via `Pod`/`Zeroable` + `DeviceRepr`/`DeviceCopy`.
Its size is frozen at 212 bytes and guarded by twin compile-time assertions — a Rust
`const _: () = assert!(size_of == 212)` and a CUDA `static_assert(sizeof == 212)`.
Every new field is appended at the tail; the existing prefix is never reordered or
resized. This forecloses adding a serde/flatbuffer layer, inserting fields mid-struct,
and changing any prefix field's type or order. Growing the struct requires bumping
both assertions in the same change.

## Consequences
The layout uses memcpy without an encoding layer. The size assertions reject
size drift; they do not detect all field-order or same-width type mismatches. The cost is rigidity: fields cannot be logically grouped or
reordered for readability, deprecated fields cannot be reclaimed without a coordinated
ABI bump, and the 212 magic number must be maintained in two languages. A reviewer can
verify compliance by checking that a new field is at the tail and both assertions moved.

## Verification
Re-checked at e0f8cd896: `src/models/simulation_params.rs` — derives `Pod, Zeroable`
(line 27), `unsafe impl DeviceRepr`/`DeviceCopy` (lines 118–119), size assertion
`== 212` (line 228), and tail fields each carry the comment "Added at the end to
preserve the existing repr(C) prefix layout" (lines 78–112).
`crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu:117` —
`static_assert(sizeof(SimParams) == 212, ...)`. Both assertions present and in
lockstep; the raw-copy path and tail-append discipline are intact.

## Closeout extension — 2026-09-04

CP-01/06/08. Owner remains jjohare with GPU/protocol/release maintainers. Complete/live is retained for the flat representation and paired size guards. The [host layout probe](../../../VisionFlow/docs/estate-review/evidence/simparams-layout-probe.json) matches all 53 field offsets at size 212/alignment 4. A temporary same-size field swap still compiles under the original assertion: size guards do not prove field order or type identity.

**Acceptance condition:** Version field/type/offset and feature-bit manifests; compare actual Rust/CUDA toolchains and loaded device-module identity in CI and release evidence. Include same-size drift negatives, copy-size and old/new consumer cases, and coordinated rollout/rollback before tail growth. Reopen on any field, compiler, precompiled module or consumer change. See [simulation compatibility review](../../../VisionFlow/docs/estate-review/rendered-state.md#simulation-layout-and-force-authority). This pass ran host extracted declarations only, with no CUDA or GPU execution.

## Acceptance progress — 2026-09-05

**Implemented — a versioned field/type/offset manifest.** The closeout showed the
existing guard's blind spot precisely: swapping `dt` and `damping` in a fixture
left `size_of` at 212, so the paired `const _: () = assert!(… == 212)` /
`static_assert` still passed while both offsets had moved. Every field is a 4-byte
scalar, so **same-size drift is the ordinary failure mode**, not an exotic one —
and the size guard is blind to all of it.

Added to `src/models/simulation_params.rs`:

- `SIMPARAMS_MANIFEST` — all 53 fields as `(name, FieldType, offset)`, in
  declaration order.
- `SIMPARAMS_FEATURE_BITS` — the 7 `FeatureFlags` bits. The feature word is as
  much part of the device ABI as the offsets: a bit reassigned on one side only
  silently enables the wrong force term.
- `SIMPARAMS_ABI_VERSION` — the coordination token for rollout and rollback, to be
  bumped on *any* manifest change. A precompiled module or raw-copy consumer can
  be checked against this number instead of inferring compatibility from a size
  that did not happen to change.
- `simparams_actual_layout()` reads the real layout with `offset_of!`;
  `verify_simparams_abi()` compares any candidate against any manifest and returns
  every departure as typed `AbiDrift` (`Name`, `Type`, `Offset`, `FieldCount`,
  `Size`). It is generic over the candidate, which is what makes the negative test
  meaningful rather than a restatement of the manifest.
- `simparams_abi_digest()` — a stable FNV-1a tag over the triples plus the feature
  bits, for binding a shipped artefact to the exact ABI it was compiled against.
  Explicitly non-cryptographic and not a security boundary.

**The same-size drift negative test.** Two tests state the gap and its closure as
a pair: `a_same_size_field_swap_still_passes_the_size_assertion` asserts the
drifted fixture is still 212 bytes (so the existing guard passes), and
`the_manifest_catches_the_same_size_field_swap` asserts the manifest reports a
`Name` drift at `dt`'s slot while reporting **no** `Size` drift. A same-size
retype (`f32` → `u32`, which moves nothing and reinterprets every value) and a
tail append (which preserves every existing offset but is still unsafe for a
shorter allocation or an older device module) are covered separately.

**Tests run.** `cargo test --lib --no-default-features adr_20` — 34 pass, 10 in
`adr_2028_abi_manifest`: real struct matches the manifest; frozen size 212 and
alignment 4; dense ascending offsets; unique field names; the two same-size drift
tests above; same-size retype; tail-append growth; digest stability, equality with
the real layout and sensitivity to a same-size reorder; single-bit uniqueness
across all 7 feature bits; and a cross-check that the fields ADR-2029's dispatch
word reads are all present.

**Governed paths changed.** `src/models/simulation_params.rs`.

**Open — this is host-side only.** No CUDA compilation, driver load, device copy
or shipped-PTX comparison ran, and the manifest is not yet compared against the
CUDA declaration by an automated check (the closeout's Python probe does that
comparison out of tree). Matching the loaded device module's identity to the host
binary, and coordinated rollout/rollback across every raw-copy consumer before
tail growth, remain open. `implementation_status` is unchanged: complete/live
still refers to the scoped flat, size-locked representation.
