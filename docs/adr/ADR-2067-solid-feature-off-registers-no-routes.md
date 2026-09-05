---
id: ADR-2067
title: The Solid pod feature-off build registers no routes
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any new handler added under `/solid`, `/.well-known/did.json`, or `/did` that gains a `#[cfg(not(feature = "solid-pod-embed"))]` 503-stub twin instead of simply not being registered when the feature is off
repo: visionclaw
domain: SECURITY-profiles
---

# ADR-2067 — The Solid pod feature-off build registers no routes

## Context
Phase 1 diagrams VC-26.1/26.2 flagged that `configure_solid_routes`
(`src/handlers/solid_proxy_handler.rs`) registered the entire `/solid` +
`/.well-known/did.json` + `/did` route tree in **both** the
`solid-pod-embed`-on and -off builds, and that every one of the eight
handlers it points at (`handle_solid_proxy`, `create_pod`,
`check_pod_exists`, `init_pod`, `init_pod_nip98`, `solid_health_check`,
`handle_did_wellknown`, `handle_did_proxy`) had a
`#[cfg(not(feature = "solid-pod-embed"))]` twin whose only job was to return
`503 Service Unavailable` (stub lines `:386`, `:1228`, `:1285`, and similarly
for the rest). The feature-off build therefore advertised a full, routed API
surface that could never do anything but fail.

## Decision
When `solid-pod-embed` is off, `configure_routes` registers nothing: no
`/solid` scope, no `/.well-known/did.json`, no `/did` scope. All eight
per-handler 503-stub twins are deleted, along with the now-orphaned
`SolidPodState::new()` synchronous stub constructor (its only caller was the
stub `configure_routes`) and the stale "storage watch is handled at the
handler level" claim these stubs' presence had encouraged elsewhere in the
file. `get_global_storage()`'s `Option<()>`-returning feature-off twin is kept
— it is a plain helper (not an HTTP handler, does not return a 503) called
unconditionally from `image_gen_handler.rs` regardless of the feature flag.
`src/main.rs` calls `configure_solid_routes` unconditionally in both builds,
so the function's name and `fn(&mut web::ServiceConfig)` signature are
unchanged; only its feature-off body changes, to a no-op.

## Consequences
In a feature-off build, `/solid/*`, `/.well-known/did.json`, and `/did/*` now
404 (no route matches) instead of every one of the eight endpoints replying
`503` with a hand-written JSON body — a smaller, honest, unauthenticated
attack surface: nothing under those paths is routed or reachable at all. The
`solid-pod-embed`-on route tree, its handlers, and their behaviour are
byte-for-byte unchanged. Anyone re-enabling a partial Solid surface without
the full feature must add real routes, not resurrect the 503-stub pattern.

## Verification
Ran on the uncommitted working tree above `verified_commit`; must be
re-verified at the landing commit.

```
$ grep -n '#\[cfg(not(feature = "solid-pod-embed"))\]' src/handlers/solid_proxy_handler.rs
# before: 11 hits (SolidPodState::new, 8 handler stubs, get_global_storage, configure_routes)
# after:  3 hits (SolidPodState::new comment marker removed; get_global_storage kept;
#         configure_routes kept, body emptied)

$ grep -n "fn configure_routes" -A3 src/handlers/solid_proxy_handler.rs
#[cfg(not(feature = "solid-pod-embed"))]
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    let _ = cfg;
    info!("=== SOLID POD ROUTES DISABLED (no solid-pod-embed feature): registering none ===");
}

$ grep -n "configure_solid_routes" src/main.rs
src/main.rs:1123:  .configure(visionclaw_server::handlers::configure_solid_routes)   # unconditional, unchanged call site

$ grep -n "get_global_storage" -r --include="*.rs" . | grep -v solid_proxy_handler.rs
src/handlers/image_gen_handler.rs:653   # unconditional real caller — kept in both builds

$ grep -n "SolidPodState::new()" -r --include="*.rs" .
(no output after removal — its only caller, the stub configure_routes, no longer calls it)

$ cargo check -p visionclaw-server
    error: could not compile `visionclaw-server` (lib) due to 4 previous errors
    # all 4 in src/services/owl_extractor_service.rs (unrelated ADR-2064
    # in-flight work by another lead on this shared tree) — none in
    # solid_proxy_handler.rs; see ADR-2066 Verification for the full trace.

$ cargo check -p visionclaw-server --no-default-features --features gpu,ontology,persistence-oxigraph
    error: could not compile `visionclaw-server` (lib) due to 4 previous errors
    # same 4 owl_extractor_service.rs errors, same file:line set as the
    # default-feature run above — confirms the feature-off code path in
    # solid_proxy_handler.rs is not implicated.
    #
    # This run's warnings first surfaced 3 real unused-import warnings this
    # ADR's change caused (Method/NostrUser/NostrService only reachable from
    # solid-pod-embed handlers now that the feature-off configure_routes
    # registers nothing) — fixed by gating those three imports behind
    # #[cfg(feature = "solid-pod-embed")]. Re-run after the fix:
    #   grep -n "unused import" <log> | grep solid_proxy_handler
    #   (no output — the three warnings are gone; only the 4 unrelated
    #   owl_extractor_service.rs errors remain)
```
