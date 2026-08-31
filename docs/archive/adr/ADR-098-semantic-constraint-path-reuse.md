# ADR-098 — Semantic Constraint Path: Reuse the Live ConstraintData Buffer, Hold the SimParams ABI

| Field | Value |
|-------|-------|
| Status | Accepted (2026-06-05) |
| Drives | PRD-018 §5 WS-3, §6.1 |
| Companion ADRs | ADR-072 (autordf2gml per-edge-type params — deferred), ADR-099 (reasoner posture), ADR-100 (canonical IRI), ADR-101 (triple-store migrations) |
| Affected paths | `crates/visionclaw-gpu/src/cuda_sources/visionclaw_unified.cu` (add SEPARATION branch + enum const; delete dead `apply_semantic_forces`), `crates/visionclaw-gpu/src/cuda_sources/ontology_constraints.cu` (delete), `crates/visionclaw-gpu/src/ptx_loader.rs` (remove `OntologyConstraints` module), `src/utils/unified_gpu_compute/{execution.rs,memory.rs,ontology.rs}`, `src/actors/gpu/{force_compute_actor.rs,ontology_constraint_actor.rs}`, `src/models/constraints.rs`, OWL→Constraint mapper (new) |
| Evidence | `visionclaw_unified.cu:89,99,475-500`, `src/models/constraints.rs:16,76,80`, PRD-018 reuse inventory |

## Context

The OWL ontology has almost no effect on the rendered graph today. The only live "semantic" force is a six-bucket hardcoded domain string-match driving repulsion scaling, and that is itself inert because `source_domain` is NULL for ~100% of nodes (the empty-MetadataStore bug, addressed by WS-0).

The PRD research established a decisive, counter-intuitive fact about the GPU layer: **the constraint pipeline is not missing — it is disconnected.** Verified 2026-06-05:

- The **live** `force_pass_kernel` already contains a complete constraint-application loop with node-role detection, per-constraint `weight`, and a `constraint_ramp_frames` progressive-activation ramp (`visionclaw_unified.cu:475-500+`). It runs whenever `feature_flags & ENABLE_CONSTRAINTS` is set (`:475`).
- `struct ConstraintData { int kind; int count; int node_idx[4]; float params[8]; float weight; int activation_frame; }` exists on the device (`:99`) **and** has a Rust mirror with serialisers `to_gpu_format()` / `to_gpu_data()` (`src/models/constraints.rs:16,76,80`).
- `ENABLE_CONSTRAINTS` is bit 4 of `feature_flags` (`:89`) — allocated, never set from Rust.
- The Rust upload handlers `UpdateConstraints` and `UploadConstraintsToGPU` in `src/actors/gpu/force_compute_actor.rs` are no-op log stubs.
- The separate `ontology_constraints.cu` kernels (`apply_disjoint_classes_kernel`, `apply_subclass_hierarchy_kernel`, `apply_sameas_colocate_kernel`, `apply_inverse_symmetry_kernel`, `apply_functional_cardinality_kernel`) duplicate, less generally, what the live inline loop already does.

### Verified topology (rev. 2, 2026-06-05) — the disconnect is sharper than first recorded

Direct source verification refined the picture; the seam has **five** concrete breaks, not one:

1. **`ENABLE_CONSTRAINTS` is never set.** `execution.rs:782-803` builds `feature_flags` from `repel_k`/`spring_k`/`center_gravity_k`/`use_sssp` only. Bit 4 (`ENABLE_CONSTRAINTS`, `simulation_params.rs:85`) is never OR'd in by any Rust path, so the live `force_pass_kernel` constraint loop (`visionclaw_unified.cu:475`) is permanently gated off. **This is the keystone wire.**
2. **The live loop handles only `DISTANCE`(0) and `POSITION`(1).** No `SEPARATION` branch exists in `force_pass_kernel` (`:506,529`). Disjointness needs a one-sided min-distance push that the live loop cannot currently express — a small **additive** kernel branch is required (so "no kernel change beyond deletions" is revised).
3. **Two divergent `ConstraintKind` enums.** Domain `ConstraintKind` (`crates/visionclaw-domain/.../constraints.rs:18`) = `{FixedPosition=0, Separation=1, …, Semantic=10}`; the live kernel's enum = `{DISTANCE=0, POSITION=1, ANGLE=2, SEMANTIC=3, TEMPORAL=4, GROUP=5}`. `ConstraintData.kind = domain_kind as i32`, so a domain `Separation`(1) is read by the kernel as `POSITION`(1) and `Semantic`(10) matches no live branch. The mapper MUST emit `kind` integers in the **live kernel's** numbering, not the domain enum's.
4. **`upload_constraints` (memory.rs:583) is lossy.** It round-trips constraints through a flat 7-float layout that preserves only `node_idx[0]`, `params[0..3]`, `weight` — `node_idx[1..3]` (the *other* endpoints of a pairwise constraint!) are dropped, and `activation_frame` is not stamped. The correct buffer writer is `set_constraints` (memory.rs:535), which preserves all four `node_idx`, all eight `params`, and stamps `activation_frame` so the ramp engages. The ontology path must route through `set_constraints`, not `upload_constraints`.
5. **A parallel dead-end actor path exists.** `OntologyConstraintActor.execute_ontology_constraints` (`ontology_constraint_actor.rs:436` → `unified_gpu_compute/ontology.rs:94`) launches the five `ontology_constraints.cu` kernels — so they are **not** zero-call-site, but they are a parallel path that the live `force_pass_kernel` supersedes once wired. The `apply_semantic_forces` kernel in `visionclaw_unified.cu:1688` is compiled but has **no host launch site** (genuinely dead).

The structural worry recorded in earlier analysis was the **frozen 172-byte `SimParams` ABI** (dual `static_assert sizeof(SimParams)==172`). That worry does not apply here: `ConstraintData[]` is passed to the kernel as a **separate device-buffer pointer** (`const ConstraintData* __restrict__ constraints`), not as a `SimParams` field. Enabling constraints flips an already-allocated flag bit. Neither touches the struct size.

Two competing mechanisms were on the table (PRD-018 WS-3 options a/b):
- **(a)** Wire the existing live `ConstraintData[]` buffer + `ENABLE_CONSTRAINTS`.
- **(b)** Implement ADR-072's per-edge-type `EdgeTypeForceParams` in GPU constant memory.

The binding directive — *reuse the tooling we have rather than creating entire new* — and the ABI analysis both point the same way.

## Decision

### D1 — Reuse the live `ConstraintData[]` path; do not build new params

WS-3 is implemented by connecting the existing seam, not by adding a new force mechanism:

1. **Anti-corruption mapper (new, small).** A pure function maps each materialised OWL axiom to one or more `Constraint` values. Initial mapping table:

   | Axiom / relation | `ConstraintKind` | Effect | Key `params` |
   |---|---|---|---|
   | `rdfs:subClassOf`, `vc:partOf` | `DISTANCE` (attraction) | shorter rest-length, pull child→parent | rest-length, strength |
   | `owl:disjointWith`, inter-domain | `SEPARATION` | push apart, min-distance clamp | min-distance, strength |
   | `owl:sameAs`, `owl:equivalentClass` | `DISTANCE` (colocate) | near-zero rest-length | rest-length≈0, strength |
   | 8 orphaned relations (`requires`, `enables`, `depends-on`, `relates-to`, `has-part`, `is-part-of`, `bridges-to`, `bridges-from`) | `DISTANCE`/`SEPARATION` | tunable per relation | rest-length, strength |

   Subjects/objects resolve to node indices through the fixed IRI→node map delivered by WS-0 (ADR-100). Axioms whose endpoints do not resolve are counted and logged, never silently dropped.

   The mapper emits `kind` integers in the **live kernel's** numbering (`DISTANCE=0` for `subClassOf`/`partOf` attraction and `sameAs`/`equivalentClass` colocate; `SEPARATION` for `disjointWith`), never the domain enum's discriminants (verified-topology break #3). It writes `node_idx[0..1]`, `count=2`, `params[0]=rest_length`, and `weight`. The natural home is the existing `OntologyConstraintActor`, repointed — reuse the actor that already holds a `constraint_buffer` and is already wired to `ForceComputeActor` (`SetForceComputeAddr`).

2. **Set the keystone flag + route through the lossless writer.** `execution.rs` ORs `ENABLE_CONSTRAINTS` into `feature_flags` whenever `self.num_constraints > 0` (break #1). The buffer is written via `set_constraints` (memory.rs:535), which preserves all four `node_idx`, all eight `params`, and stamps `activation_frame` for the live ramp — **not** the lossy 7-float `upload_constraints` (break #4). `apply_ontology_forces` (`force_compute_actor.rs:1276`, already called every step at `:1457`) is redirected to `set_constraints`. The two no-op stub handlers (`UpdateConstraints`/`UploadConstraintsToGPU`, `:2730,2739`) are implemented to drive this, reusing the existing `cached_constraint_buffer` + `UpdateOntologyConstraintBuffer` seam.

3. **One small additive kernel branch.** Add a one-sided `SEPARATION` branch to the live `force_pass_kernel` (push apart only when `current_dist < params[0]`; no attraction beyond), and a matching `SEPARATION` constant to the live `.cu` `ConstraintKind` enum that does not collide with `DISTANCE`/`POSITION`/`ANGLE`/`SEMANTIC`/`TEMPORAL`/`GROUP` (break #2). This is the only kernel addition; everything else in the loop is consumed as-is.

### D2 — Hold the SimParams ABI

`sizeof(SimParams)` stays 172; the dual `static_assert` stays green. We add **no** fields. The only `SimParams` write is flipping the `ENABLE_CONSTRAINTS` flag bit. Any future need for richer per-axiom parameters uses the `ConstraintData.params[8]` slots (8 floats, currently underused), not new `SimParams` fields.

### D3 — Retire the redundant constraint kernels and their launch path

`ontology_constraints.cu` (five kernels) is superseded by the live inline loop once D1/D2 wire it. It is **not** zero-call-site (verified-topology break #5): it is launched by `OntologyConstraintActor.execute_ontology_constraints` → `unified_gpu_compute/ontology.rs:94`. Removal therefore retires the whole parallel path together, in order:

1. Repoint `OntologyConstraintActor` to emit live-kernel `ConstraintData` via `set_constraints` (D1), so it becomes the **producer** for the live loop rather than a launcher of separate kernels.
2. Delete `execute_ontology_constraints` (`ontology.rs`) and the lossy flat branch in `upload_constraints` (memory.rs:583-642) once nothing calls them.
3. Delete `ontology_constraints.cu` and remove its `ptx_loader.rs` plumbing: the `PTXModule::OntologyConstraints` variant, its five match arms (`file_name`/`env_var`/`option_env`), the `all_modules()` entry, and update the unit test `assert_eq!(PTXModule::all_modules().len(), 10)` → `9` plus the file/env-var uniqueness sets.
4. Delete the genuinely-dead `apply_semantic_forces` kernel in `visionclaw_unified.cu:1688` (compiled into the PTX but no host launch site).

This satisfies the PRD acceptance criterion "no dead semantic kernels remain compiled" and shrinks the CUDA surface that must compile on the host. Each deletion is gated on a grep proving zero remaining references.

### D4 — Defer ADR-072 constant-memory per-edge-type params

ADR-072's `EdgeTypeForceParams` (per-relation rest-length/strength in GPU constant memory) is a **tuning refinement layered on top of D1**, not a replacement for it. It is deferred: D1's `ConstraintData.params[8]` already carries per-constraint rest-length/strength, which covers the WS-3 acceptance bar. Revisit ADR-072 only if profiling shows per-constraint upload bandwidth (not per-type constant memory) is a bottleneck at 10k+ axioms.

### D5 — Safety reuse

Reuse the existing NaN/IMA circuit breaker and `constraint_ramp_frames` ramp rather than adding new guards. Constraints activate gradually; the established `MAX_COORD`/`MAX_VELOCITY` clamps and the bad-frame fallback remain the explosion backstop.

## Consequences

**Positive:**
- Smallest possible change that makes the ontology drive the layout: a mapper + two handler bodies + one flag, all on top of code that already exists and already ramps.
- Zero ABI risk — the `static_assert` is untouched, so the wire protocol and all SimParams call sites are unaffected.
- Net reduction in CUDA code (D3 deletes more than D1/D2 adds).
- Fully GPU-resident solving — satisfies the GPU-only directive; nothing moves to the client.

**Negative / risks:**
- `ConstraintData[]` is uploaded per constraint-set change; very large axiom sets (10k+) mean larger uploads than a constant-memory scheme. Mitigated by uploading only on `OntologyModified`, not per frame, and by D4's deferral note.
- The mapper is the new correctness surface; it is covered by fixture tests (subClassOf attraction, disjointWith separation) under WS-3 acceptance.
- Deleting `ontology_constraints.cu` is irreversible in spirit; recoverable from git. Confirmed zero call sites before deletion.

## Verification

- `cargo check` (in-container gate) + full GPU build on host tmux tab 6.
- Fixture: a two-domain graph with one `subClassOf` and one `disjointWith`; assert the log-signature (`sep_x`/`flatten`) and node-geometry deltas match expected attraction/separation; browser-verify.
- CI assert: `static_assert sizeof(SimParams)==172` present and compiling; grep proves `ontology_constraints.cu` removed and no dangling references.

### Implementation record (2026-06-05, WS-3)

Two text deviations from the plan above, recorded for accuracy:

1. **Single `static_assert`, not dual.** Only one
   `static_assert(sizeof(SimParams) == 172, …)` exists in
   `visionclaw_unified.cu:79`. It is present, intact, and unmodified — the ABI
   freeze holds. The "dual" wording in D2 was a miscount; there is one assert.
2. **`all_modules().len()` went `9 → 8`, not `10 → 9`.** The codebase already
   contained 9 PTX modules (one had been removed earlier) while the unit test
   still asserted `len() == 10` — a stale, already-failing assertion. Removing
   `PTXModule::OntologyConstraints` leaves **8** modules; the test now asserts
   `len() == 8` and is renamed `all_modules_returns_eight_variants`.

Implemented as specified otherwise:

- **Keystone (break #1):** `execution.rs:799-801` ORs `ENABLE_CONSTRAINTS` (bit 4)
  whenever `self.num_constraints > 0`.
- **SEPARATION branch (break #2):** added to the live `force_pass_kernel`
  (`:542`), its telemetry block (`:601`), and the bad-frame fallback (`:2137`);
  `SEPARATION = 6` is now the canonical, non-colliding enum constant (`:121`).
- **Enum match (break #3):** new mapper `src/physics/ontology_constraint_mapper.rs`
  emits `kind` as live-kernel integers directly (`0=DISTANCE`, `6=SEPARATION`)
  via `LiveKernelKind::as_i32()`, never a domain-enum cast.
- **Lossless writer (break #4):** the lossy 7-float branch in
  `upload_constraints` (memory.rs) is deleted; it now delegates to
  `set_constraints`, preserving all `node_idx`/`params` and stamping
  `activation_frame`. `apply_ontology_forces` routes through `set_constraints`.
- **Producer (break #5):** `OntologyConstraintActor` now maps OWL axioms via
  `map_axioms_to_constraints` and uploads through `set_constraints`.
- **D3 retirement:** `ontology_constraints.cu`, the dead `apply_semantic_forces`
  kernel, `unified_gpu_compute/ontology.rs`, and all `PTXModule::OntologyConstraints`
  plumbing (ptx_loader.rs, build.rs, actor PTX loaders) are removed.
- **Mapper tests:** 3 fixtures pass (subClassOf→DISTANCE, disjointWith→SEPARATION,
  sameAs→DISTANCE colocate; unresolved endpoints counted + skipped).
- `cargo check -p visionclaw-server -p visionclaw-gpu` is green.
  **Host PTX build (tmux tab 6) is still required** to compile the new `.cu`
  branches into PTX; the in-container check cannot run nvcc.

### Live verification (2026-06-05) — forces measured end-to-end

The reuse path is proven against the live corpus, not just fixtures:

- **Axiom source fix (was the silent zero):** `get_axioms()` read only reified
  `vc:Axiom` triples and returned **0** for a corpus with 5k+ `subClassOf`. It
  now UNIONs plain structural triples (`rdfs:subClassOf`, `owl:equivalentClass`,
  `owl:disjointWith`, `ObjectPropertyAssertion` for `hasPart`/`partOf`/`sameAs`,
  and SomeValuesFrom restrictions). Result: **11,464 asserted axioms load** (was 0).
- **Mapper coverage:** `hasPart`/`partOf` added as `Attract` (corpus carries
  5,589 `hasPart`, 0 `isPartOf`). SomeValuesFrom `onProperty` mirrored to both
  `predicate` and `property` annotation keys so Whelk's translation resolves.
- **Measured dispatch:** 3,713 classes + 11,464 axioms → 19,318 inferred →
  30,782 materialised → **18,933 live-kernel constraints uploaded** (100% IRI→node
  endpoint resolution across 7,481 `SubClassOf`). Before the fix: 0.
- **Stats truthfulness:** `ingest_domain_axioms` now sets
  `active_ontology_constraints` (the stats endpoint reported `0` despite 18,933
  uploaded — the buffer was filled but the counter never set).
- **Manual re-sync wired:** the `web::Data` `GitHubSyncService` instance used by
  `POST /api/admin/sync` (constructed in `main.rs`, distinct from the
  `AppState`-internal one) is now registered with `GPUManagerActor`. Re-sync
  re-dispatches constraints instead of logging "GPUManagerActor address not
  registered" and pushing nothing.
- **Client read-back:** `GET /api/ontology-physics/constraints` surfaces
  `activeConstraints`, `axiomsProcessed`, and GPU health counters; the client
  Forces panel + System Status box poll it read-only (no client-side solving).
