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
The layout is trivially fast (memcpy, zero encode cost) and any mismatch fails the
build, not production. The cost is rigidity: fields cannot be logically grouped or
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
