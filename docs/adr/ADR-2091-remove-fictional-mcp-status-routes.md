---
id: ADR-2091
title: Remove the two multi-MCP REST routes that returned fabricated state
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a client needing multi-MCP server status or a manual discovery trigger, which must then be implemented against services/multi_mcp_agent_discovery.rs rather than reinstated from this ADR
repo: visionclaw
domain: BASELINE-architecture
lineage: ADR-2083 (the real discovery cadence these routes purported to expose), ADR-2090 (the credential fix on the sibling WebSocket route in the same file)
---

# ADR-2091 — Remove the two multi-MCP REST routes that returned fabricated state

## Context

`src/handlers/multi_mcp_websocket_handler.rs` served two live REST routes under
the `/multi-mcp` scope that reported state they never gathered:

- `GET /multi-mcp/status` → `get_mcp_server_status` returned a **hardcoded JSON
  literal**: a two-server list naming `claude-flow` at `localhost:9500` with
  `"is_connected": true` and `"agent_count": 4`, and `ruv-swarm` at `:9501` with
  `"is_connected": false`, plus `"total_agents": 4`. Only the timestamp was
  computed. It never consulted the discovery service.
- `POST /multi-mcp/refresh` → `refresh_mcp_discovery` logged *"Manual MCP
  discovery refresh requested"* and returned `{"success": true, "message":
  "Discovery refresh initiated"}` without initiating anything.

Both took `_app_state` — bound, underscore-prefixed, unused — which is the
signature of a handler written against an intent that was never wired.

The `"is_connected": true` is the part that makes this worse than dead code. A
monitoring consumer polling `/multi-mcp/status` is told a claude-flow server is
connected with four agents, regardless of whether any MCP server is running. A
route that returns nothing is a gap; a route that returns confident fiction is a
false negative in an operator's health picture.

Found by vc-core during Phase 2 and routed to this lead as owner of the file.

## Decision

Both routes and both handlers are **removed**, along with their registrations in
`configure_multi_mcp_routes`. The `/multi-mcp` scope keeps only `/ws`.

A REST surface reports state it actually gathered, or it does not exist. A
handler that fabricates a plausible response is not a placeholder for a future
implementation — it is a defect that is harder to find than the missing feature
would have been, because nothing fails.

If a client later needs multi-MCP server status or a manual discovery trigger,
it is implemented against `src/services/multi_mcp_agent_discovery.rs`, which
holds the real `discovered_agents` / `server_statuses` state and, since ADR-2083,
polls on the configured per-server cadence. It is not reinstated from this
record.

## Consequences

- `GET /multi-mcp/status` and `POST /multi-mcp/refresh` now 404. No caller
  exists, so no consumer breaks — see Verification.
- The estate loses a route that could have been mistaken for MCP health
  monitoring during an incident. That is the point of the removal.
- `crate::ok_json` became unused in this file with `refresh_mcp_discovery` gone
  and its import was removed, so the change introduces no new warnings.
- The sibling WebSocket route `/multi-mcp/ws` in the same file is untouched here;
  its credential defect was fixed separately under ADR-2090. Keeping the two
  changes in separate records keeps the security fix reviewable on its own.

## Verification

Ran on the **uncommitted working tree** above SHA
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; `verified_paths` is empty because the
tree is uncommitted, and verification must be re-run at the landing commit.

- **No caller anywhere.**
  `grep -rn 'get_mcp_server_status\|refresh_mcp_discovery' src/ client/ xr-client/`
  before the change returned exactly four hits, all inside this one file: the two
  definitions (`:842`, `:871`) and the two registrations (`:885`, `:886`). No
  client, XR-client, test or other Rust module referenced either.
- **No client route reference.**
  `grep -rn 'mcp-server-status\|mcp/status\|refresh.*discovery\|refresh-discovery' client/src xr-client`
  → no matches.
- **Registration is in this file, not `main.rs`.** The routes were registered by
  `configure_multi_mcp_routes` (this file), which `src/main.rs:1108` merely
  `.configure(...)`s. `src/main.rs` belongs to another lead and was **not**
  touched; the scope now registers `/ws` alone.
- `cargo check -p visionclaw-server --message-format=short` →
  `grep -cE '^src.*error'` → **0**, and no warning attributable to this file
  (the transient `unused import: crate::ok_json` my removal created was fixed in
  the same change).
- `cargo test -p visionclaw-server --lib multi_mcp` → **ok. 5 passed; 0 failed**
  (the ADR-2083 discovery-interval suite, unaffected and still green).
