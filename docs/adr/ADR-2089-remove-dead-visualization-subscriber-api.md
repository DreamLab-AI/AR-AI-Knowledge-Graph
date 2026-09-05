---
id: ADR-2089
title: Remove the zero-sender MultiMcpVisualizationActor subscriber API
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any future requirement for a subscriber fan-out on this actor's state changes
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2089 — Remove the zero-sender MultiMcpVisualizationActor subscriber API

## Context

Diagram ES-02 (`docs/diagrams/estate/`) and VC-27.6 (`docs/diagrams/visionclaw/27-agent-integration-mcp-relay.md`,
routed from vc-knowledge) flag `MultiMcpVisualizationActor`'s `Unsubscribe` variant as accepted but
handled by a `warn!` no-op, subscriber never removed. Verified with
`grep -rn 'MultiMcpVisualizationMessage::Subscribe\|MultiMcpVisualizationMessage::Unsubscribe'
src/ --include=*.rs`: the only hits anywhere in the tree were the actor's own two match arms
(was `:316`, `:318`) — zero external senders of either variant, confirmed before deleting anything.
`subscribers: Vec<Recipient<AgentVisualizationMessageWrapper>>` (was `:53`) was initialised
`Vec::new()` (`:232`) and pushed only by the unreachable `subscribe()` (`:520`), so it was
permanently empty and the broadcast loop (was `:981-989`) always iterated zero recipients.

## Decision

Remove the entire dead subscriber surface, per Phase 2 policy 3 (zero-sender messages, unreachable
handlers). Removing `Unsubscribe` alone would have left a subscribe-only API that leaks recipients
by construction, so both variants and everything that served only them were removed together:
`Subscribe`/`Unsubscribe` enum variants and their `handle()` match arms; `fn subscribe`; the
`subscribers` field and its `Vec::new()` initialiser; `fn broadcast_message` (including the
`failed_subscribers` bookkeeping it existed to log); the five wrapper functions
(`broadcast_initialization`, `broadcast_position_update`, `broadcast_state_update`,
`broadcast_connection_update`, `broadcast_metrics_update`) whose entire body was "construct a
message, hand it to `broadcast_message`" with no other side effect; the eleven call sites of those
wrappers, each removed as a single line from within a live handler method that does real
state-mutation work besides the broadcast (`initialize`, `update_agent_positions`, `add_agent`,
`remove_agent`, `update_agent_status`, `add_connection`, `remove_connection`,
`update_server_metrics`, `change_layout`, `reset_visualization`, `update_visualization`) — those
methods themselves were kept intact, only the dead broadcast line was cut from each; the
`AgentVisualizationMessageWrapper` struct, confirmed by grep to have no producer or consumer
anywhere else in the tree; and the imports that became unused as a direct result (`warn` from
`log`; `AgentMetrics`, `AgentStateUpdate`, `AgentVisualizationMessage`, `ConnectionUpdateMessage`,
`InitializeMessage`, `MetricsUpdateMessage`, `PositionUpdate`, `PositionUpdateMessage`,
`StateUpdateMessage`, `SwarmMetrics` from `agent_visualization_protocol`; `use
crate::utils::time;` in full, since every call site of `time::timestamp_seconds()` in this file
was inside the deleted code). `update_agent_positions`'s `timestamp: i64` parameter — used only to
feed the deleted broadcast call — was renamed to `_timestamp` rather than removed, since it is
still part of the `UpdateAgentPositions` message variant's wire shape and must stay in the
function signature.

**Caller analysis performed before deleting the broadcast loop** (required by the routing brief):
the function containing the loop was `broadcast_message(&self, message: AgentVisualizationMessage)`.
Its only callers, found by grepping `self.broadcast_message(`, were the five wrapper functions
named above — each one's entire body was message construction followed by that one call, so none
of them "did useful work besides the empty broadcast." Per the decision rule this meant removing
`broadcast_message` and all five wrapper functions outright, not just trimming the subscriber
iteration inside it. Those wrapper functions' own callers (the eleven call sites, one per live
handler method) DO do other useful work in the same method body — every one of those methods was
therefore preserved, with only its single dead `self.broadcast_xxx(...)` line removed.

**Explicitly not touched, per instruction from the coordinator**: `GlobalPerformanceMetrics`'s
import (`:18` of this file) and its six use sites (struct field, constructor params, two
`::default()` calls, one struct literal) are part of an unrelated in-flight change by another lead
(vc-knowledge, in `src/services/agent_visualization_protocol.rs`) and were left exactly as found.

## Consequences

- The actor's `handle()` no longer accepts `Subscribe`/`Unsubscribe`; any future attempt to send
  either message variant is now a compile error rather than a silent no-op, which is the correct
  failure mode for a removed API.
- The eleven `broadcast_*` call sites are gone, but every method that called them keeps its real
  state mutation (HashMap inserts/removes, physics simulation, metrics recalculation) — no
  behavioural change to the actor's core responsibilities, only to the dead fan-out tail of each.
- If a subscriber fan-out is wanted later, it must be **re-added from scratch with a working
  unsubscribe path and at least one live sender wired in the same change** — it must not be
  resurrected from this dead surface, which never had either.
- `AgentVisualizationMessageWrapper` no longer exists; anything that would have needed to construct
  it must define its own wrapper type when that future fan-out is built.

## Verification

- `grep -rn 'MultiMcpVisualizationMessage::Subscribe\|MultiMcpVisualizationMessage::Unsubscribe' src/
  --include=*.rs` — before deletion: 2 hits, both this actor's own match arms. After deletion: 0 hits.
- `grep -rn "AgentVisualizationMessageWrapper" src/ --include=*.rs` — before deletion: 6 hits, all
  within this one file (struct def, field type, two enum-variant field types, one construction
  site) and no re-export in `src/actors/mod.rs` beyond `MultiMcpVisualizationActor` itself. After
  deletion: 0 hits anywhere in the tree.
- `grep -n "broadcast_\|subscribe\|subscribers\|AgentVisualizationMessageWrapper\|use crate::utils::time\|\bwarn!"
  src/actors/multi_mcp_visualization_actor.rs` — 0 hits after the edit (no orphaned references).
- `cargo check -p visionclaw-server --message-format=short` (ran at `verified_commit`, uncommitted
  tree): 6 errors total, all outside this file's own responsibility —
  `multi_mcp_visualization_actor.rs:18` and `multi_mcp_agent_discovery.rs:18` fail on an unresolved
  `GlobalPerformanceMetrics` import, and `owl_extractor_service.rs` fails on an undeclared
  `AnnotatedOntology` type; all three are in-flight edits by another lead (vc-knowledge), confirmed
  by the queen, and untouched by this ADR. No warning in the full output names anything removed
  here (no unused-import or unused-variable warning was introduced by this change). `cargo test -p
  visionclaw-server multi_mcp` could not run to completion while the crate fails to compile for
  those unrelated reasons; re-run both commands at the landing commit. Working-tree caveat:
  verification ran on the uncommitted working tree above `verified_commit`.

### Test verification completed — 2026-09-05 (later the same day)

The blockage recorded above was another lead's concurrent in-flight edit, never
this removal. Both cleared, and the crate now compiles clean:

```
cargo check -p visionclaw-server --message-format=short   → 0 errors
cargo test -p visionclaw-server --lib multi_mcp           → ok. 5 passed; 0 failed; 0 ignored
```

No dangling reference survived the removal, re-confirmed by the lead: `grep -rn`
for each of the seven removed symbols returns zero hits in the actor. The four
apparent hits elsewhere are unrelated types — `ClientCoordinatorActor::broadcast_message`
(`client_coordinator_actor.rs:449,1582`), `broadcast_position_updates` plural
(`physics_orchestrator_actor.rs:609`) and a doc comment
(`graph_service_supervisor.rs:548`).

**Status: implementation complete and test-verified** on the uncommitted working
tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`. Re-run at the landing
commit per standard practice.
