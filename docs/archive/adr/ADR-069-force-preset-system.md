# ADR-069: Force-Preset System & Per-Edge-Category Forces

**Status:** Proposed (not started)
**Date:** 2026-05-01
**Deciders:** jjohare, VisionClaw platform team
**Supersedes:** None
**Extends:** Existing GPU dispatch path via `DynamicRelationshipBuffer` + `SemanticTypeRegistry`
**Implements:** PRD-005 §6 Epic I, §13.2 (corrected)
**Threat-modelled:** PRD-005 §19 (R-19 NaN, R-23 force explosion, F-13 mid-relaxation preset switch, F-16 unstable preset, F-29 mask oscillation, F-30 mid-tick edge insertion)

## Context

> **Status correction (2026-08-21 status-truth audit).** The prior "Implementing …
> `crates/graph-cognition-physics-presets/` … 21 tests passing" line was false: no
> such crate exists, `grep ForcePreset` returns nothing, and no `presets/*.toml`
> is present — only the pre-existing plumbing (`SemanticTypeRegistry`,
> `DynamicRelationshipBuffer`) it would extend. The kernel citation was also wrong:
> `semantic_forces.cu` lives at `crates/visionclaw-gpu/src/cuda_sources/`, not
> `src/utils/` (`src/services/semantic_type_registry.rs` is correct as cited).
> Status reset to Proposed (not started); the decision below stands as design.

PRD-005 Epic I introduces named force presets — empirically-tuned constants for known graph topologies (Logseq vault, code repo, research wiki, etc.). It draws on matryca's vis-network experimentation and on the new typed graph schema (ADR-064).

QE quality-analyzer caught a structural error in PRD §13.2 v1: the legacy fields `requires_strength` / `enables_strength` etc. on `SemanticForcesParams` were described as live, but verification against `crates/visionclaw-gpu/src/cuda_sources/semantic_forces.cu:55-64` shows they are explicitly marked legacy and unused. The actual GPU dispatch path uses `DynamicRelationshipBuffer` populated from `SemanticTypeRegistry`. This ADR encodes the corrected plumbing decision and the schedule revision.

QE chaos review flagged three high-severity concerns:

- **R-19 / F-09** — GPU NaN propagates to position broadcast.
- **R-23 / F-28** — Matryca's constants tuned for vis-network pixel space cause force explosion in VC's metre-scale world units.
- **F-13** — Mid-relaxation preset swap creates velocity discontinuity.

## Decision

**Per-edge-category forces land via `SemanticTypeRegistry` + `DynamicRelationshipBuffer` extension. Preset constants are calibrated against VC's coordinate system before adoption. SimParams undergo strict validation. Preset transitions ease over 60 frames.**

### D1 — Plumbing (corrected from PRD v2)

- `SemanticTypeRegistry` (existing, in `src/services/semantic_type_registry.rs`) is extended to register all 35 UA `EdgeKind` variants. Registration is data-driven from a `presets/edge-kinds.toml` file.
- `DynamicRelationshipBuffer` (existing, populated each tick) carries per-edge coefficients for the active preset's 35 edge kinds.
- Existing `apply_relationship_forces_kernel` in `crates/visionclaw-gpu/src/cuda_sources/semantic_forces.cu` reads the buffer; **no new kernel** — but the buffer layout grows.
- The legacy fields on `SemanticForcesParams` (requires_strength, enables_strength, has_part_strength, bridges_to_strength) are marked `#[deprecated]` in this ADR and removed in a follow-up cleanup ADR after the new path is verified at 100% rollout.

This is a **3-4 week** structural change, not the "~8 fields, 1 week" estimate in PRD v2 §13.2. PRD v3 §13.2 is corrected.

### D2 — Force-preset enum + presets crate

`ForcePreset` is a Rust enum:

```rust
pub enum ForcePreset {
    Default,           // existing tuning, retained as baseline
    LogseqSmall,       // ≤1k nodes Logseq vault
    LogseqLarge,       // 1k–100k Logseq vault, matryca-derived after calibration
    CodeRepo,          // source-code-derived graph
    ResearchWiki,      // Karpathy-pattern wiki
}
```

`crates/graph-cognition-physics-presets` is a **data-only** crate holding TOML preset files. Each preset defines:

- Global SimParams (gravity, central_gravity, damping, etc.).
- Per-edge-kind coefficients (35 entries: spring_k, rest_length, repulsion_strength).
- Per-node-kind coefficients (21 entries: mass, charge, max_velocity).
- Stability detection thresholds (velocity epsilon, force epsilon, max iterations).

Presets are versioned. Switching preset version produces a `physics.preset_change` event in audit log.

### D3 — Coordinate-scale calibration (closes R-23 / F-28)

Matryca's constants (`gravity=-50`, `spring_strength=0.08`, `damping=0.4`, `spring_length=100`) are tuned for vis-network's coordinate space (typically pixels in [0, 1000]). VC's world is in metres at typical scales of ~[-50, +50]. Adopting matryca constants verbatim produces order-of-magnitude force explosion.

**Calibration step**: Before any preset is added, a calibration test runs:

1. Load preset against a canonical 100-node graph (committed fixture).
2. Run 1,000 simulation iterations.
3. Measure: post-relaxation AABB extent, peak kinetic energy, edge-crossing count.
4. **Acceptance gate**: AABB ≤ ±1,000 in all axes; KE never exceeds 100× initial; converges within 1,000 iterations.
5. If failed, automatically scale-tune `spring_strength` / `gravity` / `rest_length` by powers of 10 until acceptance, persist scaling factor.

Calibration runs in CI. New presets cannot land without passing.

For the matryca-derived `LogseqLarge` preset specifically: the calibration step is expected to scale `gravity` from -50 (matryca px) to ≈-0.5 (VC m), and `spring_length` from 100 px to ≈1.0 m, etc. **Document the calibration factor explicitly** so that future readers understand the matryca constants are *inspirational*, not directly applied.

### D4 — Strict SimParams validation (closes R-19 partial)

`SimParams::validate()` is called before every GPU upload. Rejects (returns `Err`):

- `damping <= 0.0` or `damping >= 1.0`.
- Any `rest_length <= 0.0` for any edge kind.
- Any `spring_k < 0.0`.
- `gravity` magnitude > 100.0 (sanity ceiling).
- `max_velocity <= 0.0`.
- Any field is `NaN` or `Inf`.

A failed validation is non-fatal: the GPU upload is skipped, the previous valid SimParams stay in place, and a `physics.invalid_simparams` audit event records the rejection cause. UI shows "preset rejected, falling back to last valid".

### D5 — Per-iteration NaN guard (closes R-19 fully)

Every N iterations (default 32), a small reduction kernel checks GPU output for any non-finite position or velocity. On detection:

- Halt physics integration.
- Restore last-known-good positions from a 2-frame ring buffer.
- Emit `physics_nan_detected{iter, nodes_affected}` metric.
- Surface user banner: "Physics encountered numerical instability; layout reverted."
- Force preset → `Default` until user re-selects.

### D6 — Preset transition: 60-frame ease-in (closes F-13)

When preset changes mid-run:

1. Old SimParams snapshot stored.
2. Over 60 consecutive frames (≈1 second at 60Hz), interpolate every numeric SimParam linearly between old → new values.
3. During ease, `damping` is forcibly set to `min(old.damping, new.damping, 0.9)` to absorb transient kinetic energy.
4. After 60 frames, snap to new params.
5. Debounce: any preset change within 1s of a previous change is queued and coalesces.

Closes F-13 (mid-relaxation preset jolt) and F-16 partially (helps with unstable transition).

### D7 — Per-preset stability detection (closes F-16 fully)

Existing stability detection (`force_compute_actor.rs`) compares residual velocity against a single global threshold. Per-preset thresholds replace this:

- `LogseqLarge`: tolerates higher residual velocity (`vel_eps = 0.1`) because force-of-life is acceptable.
- `CodeRepo`: tighter convergence (`vel_eps = 0.01`).
- All presets: hard iteration cap 50,000. Above cap → log `physics_unstable{preset, ke_trajectory}`, fall back to last-converged preset, freeze positions, emit warning.
- Auto-tune: damping multiplied by 1.5 every 10,000 iters past threshold.

### D8 — Auto-selection heuristic

When a graph is loaded, a heuristic selects a default preset:

- `kind = codebase` → `CodeRepo`.
- `kind = knowledge` AND has Karpathy structure → `ResearchWiki`.
- `kind = knowledge` AND >5,000 blocks → `LogseqLarge`.
- `kind = knowledge` AND ≤5,000 blocks → `LogseqSmall`.
- Otherwise → `Default`.

Auto-selection is overridable in the settings panel.

### D9 — Atomic edge-set updates (closes F-30)

Inferred edges or block-parser updates that arrive mid-simulation are queued in a `pending_edges` buffer. Edges are integrated **only at frame boundaries** with a full edge-buffer swap (double-buffered):

```
frame N:    physics reads edge_buffer[0]
frame N→N+1: edge_buffer[1] = edge_buffer[0] + pending_edges; pending_edges.clear()
frame N+1:  physics reads edge_buffer[1]
```

Closes F-30 (out-of-bounds GPU read race).

### D10 — Frame-coherent persona compute mask (closes F-29)

Persona-aware compute mask is **latched at frame start** and applied for the entire frame. Toggles queued, applied at next frame boundary. Debounce ≥100ms between accepted toggles.

### D11 — Temporal Z-axis (Epic I.6) — soft-spring implementation

The Z-axis pinning for journal nodes is implemented as a **strong spring force toward `z_target = (journal_day - epoch) * scale`**, not a hard pin. The spring force is added in addition to existing collision and repulsion. Soft-spring preserves force isotropy and avoids the kernel-side metric break that GQ-03 (PRD §13.9) flagged as P1.

User can toggle temporal axis on/off in settings (default off).

## Consequences

### Positive

- Per-edge-category forces enable visually distinct layouts for codebases, wikis, vaults.
- Calibration step prevents the matryca-constants explosion category of bug.
- NaN guard + SimParams validation eliminates the most catastrophic GPU failure modes.
- Frame-coherent mask prevents persona-mask oscillation.
- Atomic edge-set updates prevent GPU buffer-swap races.

### Negative

- 3–4 week plumbing extension (vs. PRD v2's "1 week" estimate). Phase 2 schedule absorbs this.
- 60-frame ease-in adds ~1s to preset switch UX. Acceptable given how rare preset changes are.
- Calibration test requires a canonical 100-node fixture; one more thing to maintain.

### Risks

- Calibrated scaling factor for matryca presets may need adjustment per node-density profile; presets may not fit all Logseq vault shapes.
- Temporal Z-axis as soft spring competes with normal Z-axis forces; some user-perceived drift is expected. Document as known limitation.
- Per-iteration NaN guard cost: ~1 reduction kernel per 32 iters. Profile and confirm ≤2% overhead before flag flip.

## References

- PRD-005 §6 Epic I, §13.2 (corrected), §13.9 (GPU questions), §19 (R-19, R-23, F-13, F-16, F-29, F-30)
- Matryca: `lib/bindings/utils.js` (force constants), `docs/ARCHITECTURE.md`
- Existing: `crates/visionclaw-gpu/src/cuda_sources/semantic_forces.cu:55-64`, `src/services/semantic_type_registry.rs`, `src/models/simulation_params.rs`
- ADR-061 (Binary Protocol — preserved)
- ADR-031 (Network Backpressure — informs the broadcast cadence interaction)
- ADR-064 (Typed Graph Schema — provides the EdgeKind taxonomy this ADR consumes)
