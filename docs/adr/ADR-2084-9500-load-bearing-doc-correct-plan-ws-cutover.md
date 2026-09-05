---
id: ADR-2084
title: Correct the stale ":9500 deprecated" and "beam+gluon" doc drift; plan the state-snapshot WS cutover
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any change to bots_client.rs polling, or a decision to build the `notifications/agent_state` WS payload
repo: visionclaw
domain: PROTOCOL-registry
---

# ADR-2084 — Correct the stale ":9500 deprecated" and "beam+gluon" doc drift; plan the state-snapshot WS cutover

## Context

Diagram ES-02.8 (`docs/diagrams/estate/02-agent-events-agentbox-to-visionclaw.md`) flags
`src/agent_events/ingest.rs:12-15` calling `:9500` "deprecated" when `bots_client.rs` is live and
load-bearing: constructed `app_state.rs:1226`, boot-polled `app_state.rs:1228-1258` with backoff
`[5,15,60,300]`s, host/port from `CLAUDE_FLOW_HOST`/`MCP_TCP_PORT` default 9500
(`bots_client.rs:115-123`), 2s poll interval (`bots_client.rs:178`). ES-02.2 flags a second drift:
`ingest.rs`/`mod.rs` described a "beam + gluon render actor" as Phase 2b/latent, but
`agent_beam_actor.rs` ships the beam today and only documents gluon (attractive transient edge)
as deferred (`agent_beam_actor.rs:327`, packed-CSR edge layout has no incremental insert path).

## Decision

Two actions, tracked as one ADR because both correct the same two doc comments.

1. **DOC-CORRECT (done, this change).** `src/agent_events/ingest.rs` and `src/agent_events/mod.rs`
   module docs are rewritten to state: the GPU beam render actor is shipped and subscribes to the
   hub today; the gluon attractive edge is a separate, deferred sub-feature (cites
   `agent_beam_actor.rs:327`); and the `:9500` MCP-TCP path is **not deprecated** — it is the sole,
   live source of agent state snapshots, with no replacement built yet.

2. **PLAN (not built): cut `:9500` state snapshots over to `/wss/agent-events`.** Policy for the
   future increment: agentbox's `agent-event-publisher.js` gains a second notification kind,
   `notifications/agent_state`, pushed on the same WS subscriber connection ES-02.2 already
   authenticates, carrying the full `MultiMcpAgentStatus` list `query_agent_list()` returns today
   (id, name, type, status, x/y/z, cpu_usage, health, did_nostr). VisionClaw's `process_frame`
   (`ingest.rs`) gains a branch on this method that maps directly onto `Agent::from(mcp_agent)`
   (`bots_client.rs:54-104`) and calls `GraphServiceSupervisor::do_send(UpdateBotsGraph{agents})` —
   the same sink `bots_client.rs` calls today — instead of adding a second graph-update path.
   Once that branch is live and passes the acceptance test below, `BotsClient::start_polling`
   (`bots_client.rs:174-`), its `tokio::spawn` poll loop, and the boot-poll block in
   `app_state.rs:1228-1258` are deleted, along with `McpTcpClient`'s `query_agent_list` call site
   if nothing else uses it. Until then `bots_client.rs` stays exactly as is.

**Acceptance test for the cutover** (must all hold before removing `bots_client.rs` polling):
- The WS payload is a `notifications/agent_state` JSON-RPC 2.0 envelope whose `params.agents` is a
  `Vec<MultiMcpAgentStatus>`-shaped array identical in field set to what `query_agent_list()`
  returns today (verified by a fixture-diff test comparing one polled TCP response against one
  pushed WS payload for the same underlying agent set).
- `process_frame` routes it to `GraphServiceSupervisor::do_send(UpdateBotsGraph{agents})` with the
  same `Agent` struct shape `Agent::from(mcp_agent)` produces (including the `did_nostr` round-trip
  via `validate_did_nostr`).
- A soak test with the WS path live and the TCP poll disabled shows `/api/bots/agents` returning a
  non-empty, freshness-bounded (≤ 1 push interval stale) agent list for at least one full session —
  proving no state is lost by removing the poll.
- `bots_client.rs`'s `start_polling` tokio::spawn, the `app_state.rs:1228-1258` boot-poll block, and
  the `CLAUDE_FLOW_HOST`/`MCP_TCP_PORT`/9500 env wiring are removed in the same change that flips
  this ADR's `activation_status` to `live`.

## Consequences

- Today: the module docs describe reality — no behavioural change, so no test regressions possible.
- Until the cutover lands, VisionClaw keeps two independent live paths into agent state: the
  `:9500` TCP poll (state) and `/wss/agent-events` (actions) — this is intentional duplication,
  not drift, and is now documented as such.
- The cutover is nontrivial: it requires an agentbox-side change (`agent-event-publisher.js`) that
  is out of this ADR's file ownership (estate owns the VisionClaw side only); a future ADR or a
  cross-lead routed change lands the agentbox half.
- Follow-on: once cutover, `crate::utils::mcp_tcp_client::McpTcpClient` becomes dead code if
  `query_agent_list` is its only caller — REMOVE per Phase 2 policy 3 at that time, not now.

## Verification

Doc-only change; verified by reading, not by test assertion of behaviour:
- `bots_client.rs:115-123` (host/port env + default 9500), `:178` (`Duration::from_secs(2)` poll
  interval) — opened and confirmed at `verified_commit`.
- `app_state.rs:1226` (`BotsClient::with_graph_service` construction), `:1228-1258` (boot-poll
  spawn block with `[5,15,60,300]` backoff) — opened and confirmed.
- `agent_beam_actor.rs:327-` (`GLUON (attractive force) — DEFERRED` doc anchor, packed-CSR
  argument citing `unified_gpu_compute::memory::initialize_graph`/`upload_edges_csr`) — opened
  and confirmed; this file was NOT edited (owned by another lead; already correct).
- `cargo check -p visionclaw-server` — ran at `verified_commit` (uncommitted tree): finished with
  only pre-existing unrelated warnings (dead_code lints in unrelated modules), no errors. Must be
  re-run at the landing commit per the working-tree caveat below.
- Working-tree caveat: verification ran on the uncommitted working tree above `verified_commit`;
  re-run `cargo check -p visionclaw-server` at the landing commit before relying on this record.
