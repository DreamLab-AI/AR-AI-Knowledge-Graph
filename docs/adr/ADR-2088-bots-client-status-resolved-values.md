---
id: ADR-2088
title: BotsClient::get_status reports resolved runtime values, never literals
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any future status/health field added to BotsClient::get_status, or a change to how host/port/connection state are resolved
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2088 — BotsClient::get_status reports resolved runtime values, never literals

## Context

Diagram ES-02.8 (`docs/diagrams/estate/02-agent-events-agentbox-to-visionclaw.md`) and VC-27.1
(`docs/diagrams/visionclaw/27-agent-integration-mcp-relay.md`, routed from vc-knowledge) both flag
`BotsClient::get_status` (`src/services/bots_client.rs`, was `:227`): `"connected": connected`
where `let connected = true;` was a constant regardless of MCP reachability; `"host":
"agentic-workstation"` and `"port": 9090` were literals, while the poller actually resolves host
from `CLAUDE_FLOW_HOST`/`MCP_HOST` (default `"multi-agent-container"`) and port from
`MCP_TCP_PORT` (default **9500**) in `BotsClient::new()` (`:115-121` before this change). The
status endpoint misreported the live topology to every consumer on all three fields.

## Decision

`get_status()` reports resolved runtime state, never a literal, as a general policy for any
status/health surface: a literal cannot drift-detect a topology change and silently misreports it
forever, whereas a resolved-field read stays correct as the underlying config or state changes.
Concretely: `host`/`port` now read `self.mcp_client.host`/`self.mcp_client.port` — the same
resolved fields `connect()` already logs (`"Initializing MCP connection to {}:{}"`). `connected`
now reads a new `connected: Arc<AtomicBool>` field on `BotsClient`, set `true` only on a successful
`test_connection()` inside `connect()` and set `false` on an unreachable server or a connection-test
error — never a hardcoded value. The flag starts `false` at construction (`BotsClient::new()`), so
a client on which `connect()` has never run correctly reports itself as not connected.

## Consequences

- `/api/bots/agents`-adjacent status consumers now see the real host/port/connection state instead
  of a fixed lie; any dashboard or health check built on this endpoint becomes trustworthy.
- `connected` reflects only the last `connect()` outcome, not a live per-tick reachability probe —
  a transient `query_agent_list()` failure inside the 2s poll loop does not flip it back to `false`
  (that loop only logs and keeps the stale snapshot; see `bots_client.rs` `start_polling`). This is
  a deliberate scope boundary, not a new gap: `connect()` is the only place this ADR wires the flag,
  matching the FIX's minimal-risk footprint. A future ADR may extend the flag to per-poll
  reachability if that finer granularity is ever needed.
- Follow-on: none required immediately; this closes the finding cleanly.

## Verification

- `bots_client.rs:115-123` (host/port env resolution, default port 9500) and `:178` (poll interval)
  — read and confirmed before editing.
- Fix applied: added `connected: Arc<AtomicBool>` field (initialised `false` in `new()`), set on
  the three real branches of `connect()`'s `test_connection()` match (`true` on `Ok(true)`, `false`
  on `Ok(false)` and on `Err`), and `get_status()` now builds `"connected"` from
  `self.connected.load(Ordering::SeqCst)`, `"host"` from `self.mcp_client.host`, `"port"` from
  `self.mcp_client.port`.
- Added two `#[tokio::test]`s in a new `mod tests` at the end of `bots_client.rs`:
  `get_status_reports_resolved_host_and_port_not_literals` (asserts the reported host/port equal
  the client's own resolved fields and are NOT the old `"agentic-workstation"`/`9090` literals) and
  `get_status_reflects_actual_connection_state_not_a_constant` (asserts a freshly constructed
  client reports `connected: false`, then flips the tracked `AtomicBool` and asserts the report
  changes to `true`) — following the brief's guidance to assert against the client's resolved
  fields rather than mutate process env (no existing test-serialisation convention was present in
  this file to follow instead).
- `cargo check -p visionclaw-server` (ran at `verified_commit`, uncommitted tree): no errors or
  warnings attributable to `bots_client.rs`. The crate currently fails to compile for reasons
  entirely unrelated to this file — `src/actors/multi_mcp_visualization_actor.rs:18` and
  `src/services/multi_mcp_agent_discovery.rs:18` hit an unresolved `GlobalPerformanceMetrics`
  import, and `src/services/owl_extractor_service.rs` hits an undeclared `AnnotatedOntology` type —
  both are in-flight edits by other leads (vc-knowledge), confirmed by the queen, and out of this
  ADR's scope. `cargo test -p visionclaw-server bots` could not run to completion while the crate
  fails to compile for those unrelated reasons; re-run both commands at the landing commit once
  those are resolved. Working-tree caveat: verification ran on the uncommitted working tree above
  `verified_commit`.

### Test verification completed — 2026-09-05 (later the same day)

The blockage recorded above was another lead's concurrent in-flight edit
(`GlobalPerformanceMetrics` removed from `src/services/agent_visualization_protocol.rs`,
plus `AnnotatedOntology` in `owl_extractor_service.rs`), never this change. Both
cleared, and the crate now compiles:

```
cargo check -p visionclaw-server --message-format=short   → 0 errors
cargo test -p visionclaw-server --lib bots                → ok. 4 passed; 0 failed; 0 ignored
    services::bots_client::tests::get_status_reports_resolved_host_and_port_not_literals ... ok
    services::bots_client::tests::get_status_reflects_actual_connection_state_not_a_constant ... ok
```

**Status: implementation complete and test-verified** on the uncommitted working
tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`. Re-run at the landing
commit per standard practice.
