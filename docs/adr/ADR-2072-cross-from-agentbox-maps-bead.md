---
id: ADR-2072
title: Map bead structurally in cross_from_agentbox; remove dead metrics scaffolding in agent_visualization_processor
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: adding a new agentbox URN kind on either side of the federation boundary; any change to bead's content-addressing shape in uris.js; reintroduction of unused CPU/memory/token-rate tracking in agent_visualization_processor.rs
repo: visionclaw
domain: IDENTIFIER-taxonomy
---

# ADR-2072 — Map `bead` structurally in `cross_from_agentbox`; remove dead metrics scaffolding in `agent_visualization_processor`

## Context

Phase 1 diagram **VC-23.6** (`docs/diagrams/visionclaw/23-identifiers-urn-did-sha12.md:226-268`)
flagged that `cross_from_agentbox` (`src/uri/mod.rs:672`, was `:650`) mapped `agent`, `activity`
and `thing`, then dropped everything else — including `bead` — into the wildcard `_ => return
None` arm, while agentbox's `bc20-provenance-bridge.js::toVisionclaw` crosses `bead`
**structurally**. `agentbox/management-api/lib/uris.js:106-109` records the intent: bead is
`contentAddressed: true` "to match VisionClaw's converged grammar
(`urn:visionclaw:bead:<pubkey>:<sha256-12>`) so the BC20 bridge can cross beads structurally
instead of dropping them"; `bc20-provenance-bridge.js:88-89,167-183` implements the pass-through,
preserving the agentbox local (already `sha256-12-<12hex>`) rather than re-hashing, unlike the
`thing`/`activity` arms. agentbox ADR-2061 (`agentbox/docs/adr/ADR-2061-federation-kind-map-parity.md`,
`proposed`, estate lead) records the full cross-repo contract and requests the Rust arm from
vc-knowledge; this ADR is that arm, landing the JS-defined semantics on the Rust side.

## Decision

`cross_from_agentbox` gains a `"bead"` arm (`src/uri/mod.rs:700-707`) that crosses structurally,
matching the JS bridge: it extracts the owner pubkey scope and the already-content-addressed
local segment from the agentbox URN and mints `urn:visionclaw:bead:<pubkey>:<sha256-12>` via a
new `bead_with_address(owner_pubkey, content_addr)` constructor (`src/uri/mod.rs:284-296`,
mirroring the existing `kg_with_address`), **not** by re-hashing the whole agentbox URN string.
This preserves content identity across the boundary exactly as agentbox's bridge does — the two
grammars are already `<pubkey>:<sha256-12>`, so there is nothing to translate beyond validating
shape. A missing or malformed 64-hex scope, or a malformed content address, still yields `None`
(closed-map / fail-closed discipline unchanged); `memory` and any other unmapped kind are
untouched and still return `None`.

## Consequences

- The Rust and JS federation translators now agree on `bead`: both cross it, both preserve the
  content address, neither re-hashes. This closes the specific asymmetry agentbox ADR-2061
  describes; the broader ask in that ADR (a shared versioned kind-map artefact plus a
  cross-language round-trip fixture) remains open and cross-repo, owned jointly by vc-knowledge
  (`src/uri/mod.rs`) and ab-identity-governance (`bc20-provenance-bridge.js`) — this ADR does not
  supersede ADR-2061, it discharges one of its four acceptance-test items (item 3's Rust-side
  half) on the visionclaw side.
- `docs/IDENTIFIER-taxonomy.md`'s cross-substrate mapping table and "Minted URNs may return null"
  divergence bullet are updated in the same change (see that document's `## Remediation —
  2026-09-05` section, which already named this ADR).
- `docs/diagrams/` is out of scope for this implementer (file-ownership boundary for this task);
  VC-23.6's `DIVERGENCE ADR-2025 closeout` note is not rewritten here and should be updated to
  `RESOLVED ADR-2072` by whichever lead next touches that diagram file.

## Verification

Ran on the uncommitted working tree above `verified_commit`
(`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`); must be re-run at the landing commit.

```
$ grep -n 'fn bead\b\|fn bead_with_address\|fn cross_from_agentbox\|"bead" =>' src/uri/mod.rs
89:            "bead" => Kind::Bead,
265:pub fn bead(owner_pubkey: &str, content: impl AsRef<[u8]>) -> Result<String, UriError> {
284:pub fn bead_with_address(owner_pubkey: &str, content_addr: &str) -> Result<String, UriError> {
672:pub fn cross_from_agentbox(agentbox_urn: &str) -> Option<UrnCrossing> {
701:        "bead" => {

$ cargo check -p visionclaw-server --lib
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.75s
    # exit 0

$ cargo test -p visionclaw-server --lib uri::
running 33 tests
test uri::tests::bc20_crosses_bead_structurally_preserving_content_address ... ok
test uri::tests::bc20_bead_crossing_rejects_invalid_scope ... ok
test uri::tests::bc20_crosses_agentbox_kinds_per_closed_map ... ok
... (30 more)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 1241 filtered out; finished in 0.00s
```

New tests added (`src/uri/mod.rs` `mod tests`):
- `bc20_crosses_bead_structurally_preserving_content_address` — a bead URN with a valid 64-hex
  scope crosses to a well-formed `urn:visionclaw:bead:<pubkey>:<sha256-12>` that (a) preserves the
  exact input content address unchanged, and (b) round-trips through `parse()` to
  `ParsedUri::Bead { pubkey, address }`.
- `bc20_bead_crossing_rejects_invalid_scope` — a bead URN with an invalid (non-64-hex, or
  too-short) scope returns `None`.
- `memory` and unknown-kind → `None` were already covered by the pre-existing
  `bc20_crosses_agentbox_kinds_per_closed_map` test (`:975,977`) and re-confirmed unchanged by the
  full-suite run above.

---

## Second decision — remove dead metrics scaffolding in `agent_visualization_processor.rs`

### Context

Phase 1 diagram **VC-27.11** (`docs/diagrams/visionclaw/27-agent-integration-mcp-relay.md:492-532`)
found that the `/api/visualization/agents/ws` init/refresh path always sends an **empty** agent
list, and that the `pause_updates`/`resume_updates` client opcodes are no-ops. Investigating the
file named for this task (`src/services/agent_visualization_processor.rs`) established that
**neither of those two specific bugs lives in this file**:

```
$ grep -n 'pause\|resume' src/services/agent_visualization_processor.rs src/services/agent_visualization_protocol.rs
(no output in either file)

$ grep -n 'pause_updates\|resume_updates\|fn send_init_state\|Vec::new()' src/handlers/bots_visualization_handler.rs
41:    fn send_init_state(&self, ctx: &mut ws::WebsocketContext<Self>) {
42:        let agents: Vec<visionclaw_domain::types::claude_flow::AgentStatus> = Vec::new();
165:                        "pause_updates" => {
168:                        "resume_updates" => {
```

Both bugs live entirely in `src/handlers/bots_visualization_handler.rs:41-51,165-170` — the
`send_init_state` handler hard-codes `Vec::new()` regardless of a nearby (also dead,
`#[allow(dead_code)]`) `get_real_agent_data()` returning `vec![]` with the comment "No agents
connected yet"; and `pause_updates`/`resume_updates` are bare `debug!()` calls with no effect on
`start_position_updates`'s 16ms interval sender. That file is `src/handlers/bots*`, owned by
**estate** per the Phase 2 file-ownership table, not vc-knowledge — it is out of scope for this
task and was **not touched**. This finding is routed to the estate lead with the file:line
evidence above (also recorded in `scratchpad/reports/vc-knowledge.md`).

Establishing ground truth in the file actually assigned to this task
(`src/services/agent_visualization_processor.rs`) surfaced a distinct, genuine dead-code cluster:
a full sysinfo-based per-process CPU/memory sampler (`get_real_system_metrics`) plus a rolling
token-rate tracker (`calculate_token_rate`, `get_agent_token_usage`) and their backing state
(`SYSTEM` static, `token_history`, `process_map`, `metrics_call_count`, `evict_dead_processes`,
`is_pid_alive`, `EVICTION_INTERVAL`) — all marked `#[allow(dead_code)]` and never called:

```
$ grep -n 'calculate_token_rate\|get_agent_token_usage\|get_real_system_metrics\|evict_dead_processes' \
    src/services/agent_visualization_processor.rs
# (pre-removal) only their own definitions and internal cross-calls between each other —
# no call from process_agents(), create_visualization_packet(), or anywhere else in the file.

$ grep -rn 'calculate_token_rate\|get_agent_token_usage\|get_real_system_metrics' src/ --include=*.rs \
    | grep -v src/services/agent_visualization_processor.rs
(no output — zero external callers)
```

`process_agents()` (the only place per-agent numbers are assembled) takes `cpu_usage`,
`memory_usage`, `tokens` and `token_rate` directly from the caller-supplied
`visionclaw_domain::types::claude_flow::AgentStatus` (a real value deserialized from the
claude-flow TCP integration, per `crates/visionclaw-domain/src/types/claude_flow.rs:40-60`) — it
never calls the sysinfo-based estimators. There is no Invariant requiring a second, local,
process-scanning source of truth for numbers the upstream integration already supplies, and
nothing in the codebase reads it.

### Decision

Per PHASE2 policy rule 3 (dead, zero-caller code is deleted, not stubbed or ported), removed
outright from `src/services/agent_visualization_processor.rs`:
- Methods: `evict_dead_processes`, `calculate_token_rate`, `get_agent_token_usage`,
  `get_real_system_metrics`.
- Free function `is_pid_alive`, const `EVICTION_INTERVAL`, static `SYSTEM`.
- Struct fields `token_history`, `process_map`, `metrics_call_count` on
  `AgentVisualizationProcessor` (and their initialisers in `new()`).
- Now-unused imports: `once_cell::sync::Lazy`, `std::collections::hash_map::DefaultHasher`,
  `std::hash::{Hash, Hasher}`, `std::sync::{Arc, Mutex}`, `sysinfo::{Pid, System}`.

Not removed: the `_performance_history` field and `get_performance_history()` — the latter
generates synthetic sine-wave history rather than reading `_performance_history`, which is a real
but separate finding (fabricated performance-history data sent to clients) not named in this
task's brief; flagged here for a future pass rather than folded in unreviewed. The `sysinfo` crate
dependency itself is retained — it is still used by `src/physics/stress_majorization.rs` and
`src/handlers/consolidated_health_handler.rs`.

### Consequences

- No behaviour change: none of the removed code was reachable, so real agent visualisation data
  (which already flows from claude-flow via `AgentStatus`) is unaffected.
- The two live VC-27.11 bugs (empty init list, no-op pause/resume) remain unfixed — they are
  estate's to remediate in `src/handlers/bots_visualization_handler.rs`, which this ADR does not
  and cannot touch.
- Follow-on work (not done here, out of this task's scope): estate wiring `send_init_state` to a
  real agent source and giving `pause_updates`/`resume_updates` an actual effect on the position-
  update interval; a separate look at `get_performance_history()`'s synthetic data.

### Verification

Ran on the uncommitted working tree above `verified_commit`
(`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`); must be re-run at the landing commit.

```
$ grep -rn 'calculate_token_rate\|get_agent_token_usage\|get_real_system_metrics\|evict_dead_processes\|SYSTEM\b' \
    src/services/agent_visualization_processor.rs
(no output — fully removed)

$ cargo check -p visionclaw-server --lib
    Finished `dev` profile [optimized + debuginfo] target(s) in 26.41s
    # exit 0, warning count unchanged/lower (no new dead_code warnings introduced)
```
