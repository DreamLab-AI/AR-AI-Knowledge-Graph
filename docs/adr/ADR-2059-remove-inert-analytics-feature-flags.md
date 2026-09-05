---
id: ADR-2059
title: Remove the analytics feature flags that gate nothing
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: Any addition to the analytics FeatureFlags struct, or any proposal to persist or per-user scope it
repo: visionclaw
---

# ADR-2059 — Remove the analytics feature flags that gate nothing

## Context

`POST/GET /analytics/feature-flags` exposed a nine-field `FeatureFlags` struct
(`src/handlers/api_handler/analytics/types.rs`) and the GET response shipped a prose
`description` for all nine, each implying it switched a subsystem — "Enable GPU-accelerated
clustering algorithms", "Enable real-time AI insights generation", and so on.

Only one was read as a gate: `ontology_validation`, in
`src/handlers/api_handler/ontology/mod.rs` `check_feature_enabled`, which short-circuits
ontology validation with an error. `sssp_integration` is written by `/sssp/toggle` and
echoed by `/sssp/status` but gates nothing. The other seven — `gpu_clustering`,
`gpu_anomaly_detection`, `real_time_insights`, `advanced_visualizations`,
`performance_monitoring`, `stress_majorization`, `semantic_constraints` — were never read
anywhere outside get/update. An operator could toggle them, observe a `200 OK` and a changed
response body, and conclude a subsystem had been switched. Diagrams VC-18.2 and VC-18.9
carry the DIVERGENCE notes.

## Decision

An API does not advertise a switch it does not act on. The seven flags that are never read
are removed from the `FeatureFlags` struct, from its `Default`, and from the `description`
map in the GET response.

Two are retained, each for a stated reason. `ontology_validation` is retained because it is a
real gate on a live code path. `sssp_integration` is retained because it is part of a live
request/response contract (`/sssp/toggle` writes it, `/sssp/status` reports it) and is
documented here as **display-only state, not a gate** — removing it would break those two
endpoints for no benefit, but a reader must not infer that setting it changes SSSP behaviour.

A future flag added to this struct must have a reader at the time it is added. A flag with no
reader is not a feature toggle, it is a lie with a JSON schema.

## Consequences

The API surface shrinks and stops misleading operators. Any client that POSTed the full
nine-field body now sends unknown fields; serde's default behaviour ignores them, so this is
tolerated rather than breaking, but a client that *reads* the removed fields from the GET
response will find them absent.

Two properties of this endpoint are **deliberately left unchanged and remain open**, recorded
here so they are not mistaken for oversights. `FEATURE_FLAGS` is a process-global
`Lazy<Arc<Mutex<..>>>` (`src/handlers/api_handler/analytics/state.rs`): it is not persisted,
resets to `Default` on restart, and is shared across all users. `update_feature_flags`
performs a wholesale `*flags = request.into_inner()`, so a partial POST silently resets
omitted flags to whatever the body carries. Combined, that means any authenticated caller can
disable ontology validation process-wide for every user, and a careless partial POST can do
it accidentally. That is a fail-open control surface on a validation path; per the estate's
fail-closed posture it deserves its own decision, and it was routed to the vc-knowledge lead
(who owns the reading side) rather than being changed unilaterally here.

## Verification

`grep -n "pub ontology_validation\|pub sssp_integration\|pub gpu_clustering\|pub real_time_insights" src/handlers/api_handler/analytics/types.rs`
— only `ontology_validation` and `sssp_integration` remain in the struct.

Each removed flag was re-verified to have no reader before deletion, by grepping for
`flags.<name>` across `src/`. The two retained flags were confirmed to have live readers:
`ontology_validation` in `src/handlers/api_handler/ontology/mod.rs` `check_feature_enabled`,
and `sssp_integration` in `src/handlers/api_handler/analytics/sssp_handlers.rs` (written by
the toggle handler, read by `get_sssp_status`).

`cargo check -p visionclaw-server` — **exit 0, zero errors**, with every Phase 2 change in the
tree. (An earlier run in this phase was blocked by concurrent breakage in three files owned by
other leads; those were fixed by their owners and the check was re-run clean.)

Verification ran on the uncommitted working tree above the recorded SHA; `verified_paths` is
empty for that reason.
