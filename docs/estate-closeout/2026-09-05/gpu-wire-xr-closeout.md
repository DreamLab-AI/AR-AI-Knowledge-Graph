---
title: GPU, wire-protocol and XR closeout receipt
status: complete
date: 2026-09-05
type: reference
base_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
adrs: [ADR-2018, ADR-2019, ADR-2020, ADR-2024, ADR-2028, ADR-2029, ADR-2030, ADR-2031, ADR-2032, ADR-2033, ADR-2034, ADR-2035, ADR-2036]
---

# GPU, wire-protocol and XR closeout receipt — 2026-09-05

Source-and-test pass against the ADR closeout extensions dated 2026-09-04.
Everything closable **without a headset, GPU driver or Godot runtime** was
implemented with tests. Hardware-bound conditions remain open and are named as
such per ADR.

Scope: GPU, wire protocol, physics/simulation, XR/Godot client, build-PTX. Data,
auth and vault files were out of scope and untouched.

## Test commands and results

All run at `b00c28a0d` plus the changes below.

| Command | Result |
|---|---|
| `cargo test` (in `xr-client/rust`) | **310 pass**, 0 fail — 227 lib + 83 integration |
| `cargo test --lib --no-default-features` (server) | **1225 pass**, 0 fail, 6 ignored |
| `cargo test -p visionclaw-gpu --lib --no-default-features` | **71 pass**, 0 fail |
| `cargo test -p visionclaw-protocol` | **31 pass**, 0 fail |
| `./node_modules/.bin/vitest run src/types/__tests__/binaryProtocol.framePolicy.test.ts` | **15 pass**, 0 fail |
| `cargo check -p visionclaw-gpu` (GPU feature on, real nvcc 12.9) | **exit 0**, 9 PTX modules compiled, validated and recorded |

Baseline before the pass: 218 XR library tests. No pre-existing test was
weakened, and none was deleted; the one test that changed (ADR-2035) was a
documented implementation/test conflict, reconciled toward the ratified contract.

## Per-ADR outcome

| ADR | Closed this pass | Remaining |
|---|---|---|
| 2018 frozen record / V5 envelope | Consumer freshness gate: decreasing, duplicate, full/delta, reconnect resync; shared server/client fixture pinned to the real encoder; wired into the live client | live concurrent full/delta producers; real reconnect to a restarted server |
| 2019 tag-byte registry | Per-opcode malformed/truncated policy documented as a matrix and tested in server, XR and browser decoders; browser sibling-opcode guard added | frames over a live socket; browser verified in jsdom only |
| 2020 agent co-presence `0x44` | Server ingest with permission denial, node correlation, stale removal, independent pose operation; client `RemotePresenceStore` | no `/ws/presence` route publishes it yet; no headset rendered a remote avatar. **Staged activation retained** |
| 2024 26-bit wire id | Untyped encoder branch now remaps and reports in release, not just `debug_assert`; all six classes share one helper | live allocator overflow; per-generation map retirement across clients |
| 2028 SimParams ABI | Versioned field/type/offset + feature-bit manifest, drift detector, digest; same-size drift negative test | CUDA/device comparison; loaded-module identity |
| 2029 force channel flags | Caller inventory; derivation extracted to a pure function and wired into dispatch; final device word tested across residency, SSSP and scalar boundaries | no constraint upload, GPU tick or observed force output |
| 2030 PTX ISA downgrade | Launch vs compiler failure separated; version-token parsing (fixes the `9.10` → `9.00` splice); symbol validation; artefact manifest. Verified on a real toolkit | all runtime-side conditions: driver load, kernel run, rollback, `ptx_loader` selection |
| 2031 withdrawn/reserved | Disposition confirmed — number unreused, excluded from counts, links resolvable | nothing (terminal tombstone) |
| 2032 Godot renderer | Revision matrix specified, Column A populated from source | Columns B and C — **all** hardware |
| 2033 HUD press mode | All 11 construction sites routed through one `_press_fire` helper; 0 unwrapped, 0 stray assignments | runtime press-to-dispatch, drag-off, jitter, duplicate actions |
| 2034 render store | Action/state precedence with wrap-safe timestamps; evidence expiry demoting stale live status; wired into `poll()` | disconnects, folded endpoints, one action visible on a headset |
| 2035 hierarchical label | Stale test reconciled to the ratified contract; subclass, domain-membership, mixed and cyclic fixtures added | producer-side provenance; rank upload and displayed layout |
| 2036 OpenXR boot | Five boot checks specified as matrix rows with required evidence | all of it — no boot receipt on any target |

Ten ADRs advanced with code and tests; 2031 is a tombstone confirmation; 2032 and
2036 received documentation-only receipt specifications because execution is
impossible here.

## Files changed

**Wire protocol**
- `crates/visionclaw-protocol/src/wire_fixtures.rs` (new) — shared, dependency-free
  fixtures; included by the XR workspace via `#[path]`, pinned to the real encoder
  by a server-side equivalence test
- `crates/visionclaw-protocol/src/lib.rs`
- `src/utils/binary_protocol.rs` — `enforce_wire_id_bounds` / `remap_wire_id`;
  ADR-2019 and ADR-2024 test modules
- `client/src/types/binaryProtocol.ts` — `SIBLING_OPCODES` demultiplexer guard
- `client/src/types/__tests__/binaryProtocol.framePolicy.test.ts` (new)

**XR client**
- `xr-client/rust/src/binary_protocol.rs` — `FreshnessGate`, `FrameKind`,
  `Freshness`, sequence-returning decode; live gating, resync on disconnect,
  clock anchor, diagnostics accessors
- `xr-client/rust/src/render_store.rs` — precedence, `ts_is_newer`,
  `expire_stale_agents`, counters
- `xr-client/rust/src/avatar_state.rs` — `RemotePresenceStore`
- `xr-client/rust/tests/wire_freshness_and_frame_policy.rs` (new)
- `xr-client/rust/tests/agent_copresence_roundtrip.rs` (new)
- `xr-client/scripts/hud.gd` — `_press_fire` helper at all 11 sites

**Simulation / GPU**
- `src/models/simulation_params.rs` — ABI manifest, drift detector, digest
- `src/models/force_channels.rs` — `derive_dispatch_feature_flags`
- `src/utils/unified_gpu_compute/execution.rs` — dispatch calls the pure derivation
- `src/actors/gpu/force_compute_actor.rs` — hierarchy test reconciliation (tests only)
- `crates/visionclaw-gpu/src/ptx_policy.rs` (new), `build.rs`, `src/lib.rs`

**Presence**
- `src/actors/presence_actor.rs` — `0x44` ingest, broadcast, sweep, handlers, tests

## Receipts in this directory

- `ptx-build-manifest.txt` — emitted by the rewritten build script on a real nvcc
  12.9 toolkit: 9 modules, all `provenance=compiled`, `isa=8.8`, original and
  rewritten content tags equal (correctly `Unchanged`, since 8.8 is below the 9.0
  target — the closeout's spurious-downgrade-warning finding, demonstrated absent
  on real output)
- `xr-export-runtime-revision-matrix.md` — ADR-2032/2036 receipt specification

## What this pass does not establish

No GPU kernel ran, no CUDA module was loaded by a driver, no headset booted, no
Godot scene was parsed, and no frame crossed a live socket. Host-side parity,
build-phase acceptance and codec/actor integration are useful evidence; none is a
release certificate. Every ADR's remaining column above is genuinely open.

GDScript changes were made without a parser available and are deliberately
conservative: one added function and eleven single-line constructor substitutions,
with no control-flow, signal or layout edits.
