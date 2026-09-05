---
id: ADR-2083
title: "Multi-MCP discovery loop honours the configured discovery_interval_ms"
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any change to McpServerConfig, its discovery_interval_ms defaults, or the discovery loop in src/services/multi_mcp_agent_discovery.rs
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2083 — Multi-MCP discovery loop honours the configured discovery_interval_ms

## Context

Diagram ES-02.9 exposed: `McpServerConfig::discovery_interval_ms` (declared
`src/services/multi_mcp_agent_discovery.rs:30`) is set per-server in
`initialize_default_servers` (defaults 5000/3000/3000ms) but never read — the
discovery loop's `tokio::spawn` in `start_discovery` slept a hardcoded
`Duration::from_millis(1000)` after every `join_all(discovery_futures)`
regardless of any server's configured cadence. Per-server values are a
deliberate, sensible knob (PHASE2 policy 4: built-but-unwired config → WIRE
when an Invariant needs it), so the field is connected rather than removed.

## Decision

The discovery loop's inter-cycle sleep is derived from the configured
per-server `discovery_interval_ms` via a new pure function
`select_discovery_interval_ms`: the **minimum** `discovery_interval_ms` across
currently **enabled** servers (so every server is polled at least as often as
it asked to be), clamped to a floor of `MIN_DISCOVERY_INTERVAL_MS = 250`ms so
a misconfigured `0` cannot spin the loop into a busy-poll. Disabled servers
never influence the result. When no server is enabled (empty or all-disabled
server set), the loop falls back to the named constant
`DEFAULT_DISCOVERY_INTERVAL_MS = 5000`ms rather than a bare literal, matching
`McpServerConfig::default()`. The interval is recomputed every cycle (from the
same `servers_config` snapshot already read for that cycle's discovery), so
adding/removing/disabling a server via `add_server`/`remove_server` takes
effect on the next sleep without a restart.

## Consequences

- Operators configuring `discovery_interval_ms` per server (e.g. a
  fast-changing DAA swarm at 3000ms vs. a stable Claude Flow server at 5000ms)
  now get that cadence honoured, rather than a silently-ignored hardcoded
  1000ms for every server regardless of configuration.
- A misconfigured `discovery_interval_ms: 0` no longer busy-polls the discovery
  loop — it is floored at 250ms.
- Slight behaviour change versus the prior hardcoded 1000ms: with the shipped
  defaults (3000/3000/5000ms) the loop now sleeps 3000ms between cycles
  instead of 1000ms, i.e. up to 3x fewer discovery cycles/sec against MCP
  servers under default config — this is the intended fix (the field's whole
  purpose), not a regression, but it changes observed discovery latency for
  any code relying on the old undocumented 1s cadence.
- Follow-on work (not done here, out of scope for this ADR): no runtime API
  exists yet to change `discovery_interval_ms` post-`add_server` without a
  full `add_server` re-insert; not needed by any current caller.

## Verification

Commands run against the uncommitted working tree above commit
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` (HEAD at time of writing; this
verification must be re-run at the landing commit):

- `cargo check -p visionclaw-server` — **initially passed** clean (only
  pre-existing warnings unrelated to this change). Subsequent runs in the same
  session failed, at different times, with errors in three files, none of
  which are this ADR's file and none of which reference anything this ADR
  added:
  - a transient unresolved import of `GlobalPerformanceMetrics` from
    `crate::services::agent_visualization_protocol` at
    `src/services/multi_mcp_agent_discovery.rs:18` (a pre-existing import used
    by this file's unrelated `get_global_performance_metrics()`, not
    introduced by this change) — caused by another lead's (vc-knowledge)
    in-flight edit to `src/services/agent_visualization_protocol.rs`, which
    later restored the symbol; confirmed resolved on a later run;
  - `error[E0425]`/`error[E0433]`: `AnnotatedOntology` not found,
    `error[E0425]`: `read_functional` not found in scope, all in
    `src/services/owl_extractor_service.rs` (a horned_functional API-version
    mismatch, `git status` confirms this file is concurrently modified by
    another lead right now);
  - `error[E0425]`: `public_reads_enabled_in`, `visibility_filter_enabled_in`
    not found in scope, in a third file elsewhere in the crate.
  None of `src/services/agent_visualization_protocol.rs`,
  `src/actors/multi_mcp_visualization_actor.rs`,
  `src/services/owl_extractor_service.rs`, or the RBAC-visibility file above
  are in this ADR's scope or were touched here. The crate did not reach a
  clean `cargo check` in this session despite repeated retries over ~15
  minutes; this is ongoing concurrent Phase 2 editing across the shared
  working tree by other leads, not a defect in this change.
Once the shared tree stabilised, both commands were re-run to completion and
are green:

- `cargo check -p visionclaw-server` — clean: `Finished \`dev\` profile
  [optimized + debuginfo] target(s) in 11.30s`, exit code 0 (only
  pre-existing warnings unrelated to this change).
- `cargo test -p visionclaw-server --lib multi_mcp` — exit code 0:
  ```
  running 5 tests
  test services::multi_mcp_agent_discovery::discovery_interval_tests::falls_back_to_default_when_no_server_is_enabled ... ok
  test services::multi_mcp_agent_discovery::discovery_interval_tests::disabled_servers_are_ignored ... ok
  test services::multi_mcp_agent_discovery::discovery_interval_tests::floor_is_applied_to_a_too_small_configured_value ... ok
  test services::multi_mcp_agent_discovery::discovery_interval_tests::value_above_floor_is_unchanged ... ok
  test services::multi_mcp_agent_discovery::discovery_interval_tests::selects_minimum_across_enabled_servers ... ok

  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 1272 filtered out
  ```

**Status: implementation complete and test-verified** on the uncommitted
working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`. Both commands
must still be re-run at the landing commit per standard practice, but this is
no longer blocked — the transient failures recorded above were entirely two
other leads' concurrent in-flight edits to files outside this ADR's scope,
not this change.
