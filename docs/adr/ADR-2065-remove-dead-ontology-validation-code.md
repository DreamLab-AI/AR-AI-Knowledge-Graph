---
id: ADR-2065
title: Remove dead ontology and validation code
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any future PR that re-introduces a CPU-only OWL validator, an empty-graph guard outside file_service.rs, or a graph-cognition-extract crate not registered as a workspace member
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2065 — Remove dead ontology and validation code

## Context
Phase 1 diagram VC-20.10 flagged `src/services/owl_validator_stubs.rs` as an
orphaned second `ValidationConfig`/`PropertyGraph`/`RdfTriple`/`ConstraintSummary`
set with no `mod` declaration anywhere in the repo. Phase 1 diagram VC-25.13
flagged `src/services/empty_graph_check.rs::check_empty_graph` as having zero
call sites — the live empty-graph guard is an unrelated inline check in
`file_service.rs:1174`. `crates/graph-cognition-extract/` contained only a
`.claude-flow/data/pending-insights.jsonl` file — no Rust source, no
`Cargo.toml` — and was never listed in the root `Cargo.toml` `[workspace]
members`.

## Decision
Dead code is deleted, not ported or stubbed. All three are removed outright:
- `src/services/owl_validator_stubs.rs` (no `mod`/`use` referenced it; it was
  never compiled into any build).
- `src/services/empty_graph_check.rs` (its only function had zero callers).
- `crates/graph-cognition-extract/` (not a workspace member; contained no
  source).

## Consequences
No behaviour changes — none of the three was reachable from any compiled
target, so their removal is a pure code-size reduction. The live empty-graph
guard remains the inline check in `file_service.rs:1174`, unaffected. A future
CPU-only OWL validation path (if ever needed for the `ontology` feature being
disabled) must be wired to a real caller and registered via `mod`, not left as
an orphaned file.

## Verification
Ran on the uncommitted working tree above `verified_commit`
(`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`); must be re-verified at the
landing commit.

```
$ grep -rn "owl_validator_stubs" --include="*.rs" .
(no output before deletion — zero mod/use references anywhere in the repo)

$ grep -rn "empty_graph_check\|check_empty_graph" --include="*.rs" .
src/services/empty_graph_check.rs:5:pub fn check_empty_graph(...)   # only its own definition; no callers, no mod line

$ grep -n "mod empty_graph_check\|mod owl_validator_stubs" -r --include="*.rs" .
(no output — neither file was ever part of the module tree)

$ find crates/graph-cognition-extract -type f
crates/graph-cognition-extract/.claude-flow/data/pending-insights.jsonl   # only file present, before deletion

$ grep -n "graph-cognition-extract" Cargo.toml
(no output — not a workspace member)

$ rm src/services/owl_validator_stubs.rs src/services/empty_graph_check.rs
$ grep -n "claude-flow" .gitignore
18:claude-flow
197:.claude-flow/
198:**/.claude-flow/   # the only content graph-cognition-extract/ held was gitignored — never tracked in git history

$ rm -rf crates/graph-cognition-extract

$ cargo check -p visionclaw-server
    error: could not compile `visionclaw-server` (lib) due to 4 previous errors
    # All 4 errors are pre-existing/concurrent, unrelated breakage in
    # src/services/owl_extractor_service.rs (AnnotatedOntology/read_functional
    # horned-owl API drift — tracked separately under ADR-2064, a different
    # lead's in-flight work on this shared working tree). Confirmed by
    # attributing every error to its file:
    #   grep -B1 '^error' <log> | grep -v '^--$'   -> 4x owl_extractor_service.rs, 0x elsewhere
    # None touch owl_validator_stubs.rs, empty_graph_check.rs, or
    # graph-cognition-extract/ — this ADR's removals introduce zero new errors.
```

## Phase 2 addendum (2026-09-05) — remove the dead `SemanticAnalyzer` port

vc-gpu-wire's diagram **VC-15.11** (`docs/diagrams/visionclaw/15-gpu-analytics.md:521-533`)
flagged `src/ports/semantic_analyzer.rs::SemanticAnalyzer` (the `trait`, `:52`) as
`<<trait, DEAD - zero impls>>`, distinct from the live hexagonal port
`visionclaw_domain::ports::gpu_semantic_analyzer::GpuSemanticAnalyzer` with its adapter at
`src/adapters/gpu_semantic_analyzer.rs`. Their ADR-2054 (Delete the dead GPU, analytics and
WebSocket code paths) records the sibling removals in this same subsystem sweep.

Re-verified independently before deleting (own grep, not a repeat of vc-gpu-wire's):

```
$ grep -rn "impl SemanticAnalyzer\|for .*SemanticAnalyzer" src/ crates/ --include=*.rs
src/application/semantic_service.rs:229:    impl GpuSemanticAnalyzer for MockSemanticAnalyzer {
src/adapters/gpu_semantic_analyzer.rs:206:impl GpuSemanticAnalyzerAdapter {
src/services/semantic_analyzer.rs:133:impl Default for SemanticAnalyzerConfig {
src/services/semantic_analyzer.rs:153:impl Clone for SemanticAnalyzer {
src/services/semantic_analyzer.rs:163:impl SemanticAnalyzer {
# All hits are either the unrelated GpuSemanticAnalyzer trait, or the unrelated
# *struct* SemanticAnalyzer in src/services/semantic_analyzer.rs (its own inherent
# impl, Default/Clone for its config/self) — never `impl <the trait>
# ports::SemanticAnalyzer> for <T>`.

$ grep -rn "ports::SemanticAnalyzer\b\|dyn SemanticAnalyzer\b\|: SemanticAnalyzer\b" src/ crates/ --include=*.rs \
  | grep -v "src/ports/semantic_analyzer.rs\|src/services/semantic_analyzer.rs"
src/actors/semantic_processor_actor.rs:174:    semantic_analyzer: Option<SemanticAnalyzer>,
src/actors/semantic_processor_actor.rs:263:        semantic_analyzer: Option<SemanticAnalyzer>,
# Checked the import at semantic_processor_actor.rs:22-24: this SemanticAnalyzer is
# `crate::services::semantic_analyzer::SemanticAnalyzer` (the struct), explicitly
# imported from `services::semantic_analyzer`, not the ports trait. Zero consumers
# of the ports trait confirmed.

$ grep -rn "ports::SemanticAnalyzer\|ports::semantic_analyzer" --include=*.rs .
./src/ports/mod.rs:11:pub mod semantic_analyzer;
./src/ports/mod.rs:20:pub use semantic_analyzer::SemanticAnalyzer;
./src/ports/semantic_analyzer.rs:...
# (only the module's own declaration/re-export — no external caller of the path)
```

Confirmed a real, standalone, zero-implementor, zero-consumer trait. Deleted outright per
policy (dead code removed, not stubbed):

- `src/ports/semantic_analyzer.rs` — removed entirely.
- `src/ports/mod.rs` — removed `pub mod semantic_analyzer;` and
  `pub use semantic_analyzer::SemanticAnalyzer;`. The unrelated `services::semantic_analyzer`
  struct, and the live `gpu_semantic_analyzer` re-exports, are untouched.

### Verification

```
$ cargo check -p visionclaw-server --lib
    Finished `dev` profile [optimized + debuginfo] target(s) in 29.14s
    # exit 0, no new warnings/errors introduced by the removal
```

Ran on the uncommitted working tree above `verified_commit`
(`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`); must be re-verified at the landing commit.
