# Emitting ACSP Governance Panels from VisionClaw

How a VisionClaw-side agent or service publishes **Agent Control Surface
Protocol (ACSP)** events — Nostr kinds **31400-31405** — so that interactive
control panels render on the DreamLab forum governance page
(`/community/governance`).

The canonical schema reference (exact JSON per kind, field_type catalogue,
dos/don'ts, troubleshooting) is maintained in agentbox:
[`docs/developer/agent-control-surface-panels.md`](https://github.com/DreamLab-AI/agentbox/blob/main/docs/developer/agent-control-surface-panels.md)
(mirrored locally under `docs/agentbox-docs/`). This page covers only the
VisionClaw-specific picture.

## Current state: VisionClaw IS a panel producer (ADR-110, 2026-06-12)

`src/services/acsp/` is the VisionClaw ACSP producer:

- `events.rs` — serde-exact wire types + unsigned-event builders, with
  round-trip tests locking the consumer contract (kebab-case panel enums,
  snake_case content keys, `["d", id]` first tag, broker-projection fields as
  tags).
- `client.rs` — `nostr_sdk::Client`-backed signing/publishing to
  `FORUM_RELAY_URL` plus the long-lived kind-31403 decision subscription that
  routes human responses back to the owning actor by case-id namespace.
- First agentic actor: `src/actors/elevation_actor.rs` — the "Knowledge
  Elevation" panel (`d = vc-elevation`, cases `vc-elev-*`,
  `knowledge_enrichment` category). Frontier ontology concepts become broker
  cases; an `approve` decision commits a draft Class page to the corpus as a
  PR via `GitHubPRService`. Env gate: `ELEVATION_ACTOR_ENABLED=1` +
  `FORUM_RELAY_URL` + `ACSP_PANEL_NOSTR_PRIVKEY` (falls back to
  `VISIONCLAW_NOSTR_PRIVKEY`).
- **Operational prerequisite:** register the panel pubkey (logged at startup)
  in the relay's `agent_registry` or every publish is rejected with
  `blocked: pubkey not in agent registry`.

Separately, `src/services/nostr_bridge.rs` remains the **bead-provenance
bridge** (kind 30001 → kind 9 NIP-29 group messages) — an audit trail, not a
decision mechanism.

## The pipeline

| Hop | Component | Behaviour |
|-----|-----------|-----------|
| Producer | `src/services/acsp/` (+ your actor) | Builds + signs 31400-31405 events |
| Relay | nostr-rust-forum `nostr-bbs-relay-worker` | Accepts the agent kinds **only from pubkeys in its `agent_registry` D1 table** (`active = 1`); rejects others with `OK false "blocked: pubkey not in agent registry"`. Kind 31403 (human responses) is admin-only — never publish it. 31402 events are projected into the `broker_cases` governance inbox |
| Consumer | nostr-rust-forum `nostr-bbs-forum-client` (`panel_registry` + `GovernancePage`) | Strict-serde parses content and renders panels/action rows at `/governance` |
| Website | dreamlab-ai-website | Serves the forum SPA at `/community/`, so panels appear at `/community/governance` |

The reference producer implementation is agentbox's
`management-api/lib/agent-control-surface.js`; its jest contract suite
(`tests/sovereign/agent-control-surface.test.js`) locks the exact wire
shapes. A Rust producer must match the consumer serde structs in
`nostr-rust-forum/crates/nostr-bbs-core/src/governance.rs` byte-for-byte
semantics.

## What a panel-emitting VisionClaw integration must do

1. **Get the pubkey registered.** Derive the 64-char hex x-only pubkey from
   `VISIONCLAW_NOSTR_PRIVKEY` (or mint a dedicated panel keypair — cleaner
   for rate-limiting and revocation) and have a relay admin register it:
   `POST /api/governance/agents/register` (NIP-98 admin-gated) with
   `{ "pubkey": "<64 hex>", "name": "visionclaw-governance", "description": "...", "rate_limit_per_min": 60 }`.
   Identity is `did:nostr:<hex-pubkey>` — the same mesh identity used by
   bead provenance; registration + the Schnorr signature is the entire
   authorisation.

2. **Build serde-exact events.** Every event is NIP-33
   parameterised-replaceable with a non-empty `["d", panelId]` tag.
   Content keys are **snake_case** (`field_type`, `refresh_secs`,
   `context_url`); enum values are **kebab-case** (`action-inbox`,
   `inbox-table`, `primary`, …). A kind-31400 PanelDefinition content:

   ```json
   {
     "title": "VisionClaw Graph Health",
     "description": "Physics convergence and sync status",
     "version": "1.0.0",
     "schema": "status-board",
     "fields": [
       { "name": "iteration", "field_type": "int", "label": "Physics iteration" },
       { "name": "last_sync", "field_type": "timestamp", "label": "Last GitHub sync" }
     ],
     "actions": [
       { "id": "force-resync", "label": "Force resync", "style": "destructive" }
     ],
     "layout": "card-grid",
     "capabilities": ["filter"],
     "refresh_secs": 30
   }
   ```

   Unknown enum values or camelCase keys fail the consumer parse *silently*
   (the relay still says OK). Domains: schema ∈ {`action-inbox`,
   `dashboard`, `config-form`, `status-board`, `chat-bridge`}; layout ∈
   {`inbox-table`, `kanban`, `card-grid`, `split-detail`}; field_type ∈
   {`string`, `int`, `float`, `bool`, `json`, `enum`, `timestamp`}; style ∈
   {`primary`, `secondary`, `destructive`}.

3. **Put ActionRequest (31402) metadata in TAGS, not content.** The relay's
   broker-case projection reads tags only: `["priority", "high"]`
   (`critical|high|medium|low`), plus optional `["category", ...]`,
   `["subject-kind", ...]`, `["subject-id", ...]`, `["title", ...]`.
   Content is just
   `{"fields": {...}, "reasoning": "...", "context_url": "..."}` with
   explicit `null` for absent optionals. Use the case's `d` tag as the
   broker case id and a VisionClaw URN (minted via `src/uri/`, e.g.
   `urn:visionclaw:execution:<sha256-12>`) as `subject-id`.

4. **Sign and publish directly to the forum relay.** Unlike bead provenance,
   panels are NOT bridged through kind-9 group messages — publish the
   governance kinds as-is to `FORUM_RELAY_URL`. With `nostr_sdk` (already a
   dependency of `nostr_bridge.rs`): build the event with
   `EventBuilder::new(Kind::from(31400), content).tags([Tag::identifier(panel_id)])`,
   sign with the registered `Keys`, send `["EVENT", ...]` over the relay
   WebSocket. Maintain the connection as a long-running task like
   `NostrBridge::run()` does — reconnect-with-backoff, not
   connect-per-publish.

5. **Listen for human decisions.** Subscribe to kind **31403** filtered on
   your case `d` tags. Content is
   `{"action": "approve" | "reject" | ..., "reasoning": "..."}` with the
   request's event id in an `e` tag. The relay guarantees the responder is
   an admin.

6. **Maintain the panel lifecycle.** 31401 = full JSON-object snapshot
   (replaceable per `d`); 31404 = shallow-merged top-level diff; 31405 =
   retire (empty content). Retire a given `panelId` at most once — core
   treats repeated 31405 `d` tags as append-only audit replays. Re-publish
   the 31400 definition to resurrect a panel. Keep events under the relay's
   64 KiB content / 2000 tag / 1024 B-per-tag-value limits.

## Where to put the code

Follow the existing pattern: a `src/services/` module spawned from startup
as a background task (like `NostrBridge`), gated by env
(`FORUM_RELAY_URL` + signing key present). Do not extend
`nostr_bridge.rs`'s forwarding loop to emit panels — provenance forwarding
and panel production have different keys, kinds, and failure semantics.
Reuse the BC20 anti-corruption layer conventions for any URN that crosses
the federation boundary (`urn:visionclaw:*` stays VisionClaw-side;
`subject-id` is an opaque string to the relay).

## Quick troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| `OK false "blocked: pubkey not in agent registry"` | The signing pubkey is not registered (or revoked) — step 1 |
| `OK false "blocked: admin-only governance action response"` | You published 31403; don't — subscribe to it instead |
| Relay OK but no panel on `/community/governance` | Content failed strict serde: snake_case keys, kebab-case enums, all required keys (`title`, `description`, `schema`, `fields`, `actions`, `layout`) |
| Action renders with priority `medium` | Priority must be a tag, not a content key |
| Panel vanished | Same `(kind, pubkey, d)` replaced it, or a 31405 retired it |

Full table: agentbox reference §13.
