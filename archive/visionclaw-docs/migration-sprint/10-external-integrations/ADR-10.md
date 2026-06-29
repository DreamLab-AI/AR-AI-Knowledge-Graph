# ADR-10 — External Integrations (agentbox / forum / whelk)

Status      : Proposed
Date        : 2026-05-16
Supersedes  : ADR-040, ADR-041, ADR-042, ADR-043, ADR-044, ADR-045
              (enterprise platform ADRs, made obsolete by `c64661e97`)
Supersedes  : ADR-090 (enterprise removal record; this ADR specifies the
              contract with the new home of those features)
Related     : ADR-02 (Binary Protocol — agent telemetry shares no wire
              format with graph positions), ADR-06 (Auth & Security —
              owns the cryptography), ADR-07 (Bots Telemetry — consumer
              side of inbound contract), ADR-08 (Ontology — consumer
              of whelk inferences), ADR-11 (Persistence — Oxigraph
              writes the inferred ABox).

## Context

VisionClaw is reduced to its core capability: visualising knowledge and
agent graphs. Three previously-internal capability clusters now live
outside this codebase:

1. The **agent runtime** (broker, spawn forms, control panel, swarm
   topology UI) lives in agentbox. VisionClaw consumes telemetry and
   forwards user actions.
2. The **enterprise platform** (OIDC, RBAC, KPIs, workflows, connectors,
   decision canvas, policy engine) lives in the Nostr-native forum.
   VisionClaw consumes the subset that affects its rendering and auth.
3. The **OWL reasoner** (whelk-rs) is vendored but logically external —
   used as a pure function.

This ADR fixes the boundaries. The boundary contracts are load-bearing
because both sides will be implemented by different people on different
schedules. Drift here is more expensive than drift inside a single
codebase.

## Decision

### D1. Inbound agent telemetry: WebSocket-only, versioned envelope

Single transport: WebSocket subscription. No SSE, no HTTP long-poll, no
HTTP polling fallback. Polling fallback was rejected in §9 of PRD-10 —
fallbacks become primary paths under stress and accumulate bugs.

Single envelope shape, top-level discriminated union by `type`:

```json
{
  "schema_version": 1,
  "type": "snapshot" | "delta" | "agent_added" | "agent_removed" | "heartbeat" | "communication",
  "session_id": "<agentbox session uuid>",
  "frame_id": 0,
  "timestamp_ms": 1715856000000,
  "payload": { /* type-specific */ }
}
```

Type-specific payloads:

```json
// snapshot — full agent set, sent on connect and after every reconnect
{
  "type": "snapshot",
  "payload": {
    "agents": [
      {
        "agent_id": "agent-abc123",
        "kind": "researcher" | "coder" | "tester" | ...,
        "status": "spawning" | "running" | "idle" | "stopped" | "errored",
        "swarm_id": "<optional uuid>",
        "spawned_at_ms": 1715856000000,
        "last_activity_ms": 1715856001000,
        "metadata": { /* opaque, opaque to VisionClaw */ }
      }
    ]
  }
}

// delta — incremental update for one agent
{
  "type": "delta",
  "payload": {
    "agent_id": "agent-abc123",
    "fields": {
      "status": "idle",
      "last_activity_ms": 1715856002000
    }
  }
}

// agent_added / agent_removed — topology change
{
  "type": "agent_added",
  "payload": { /* same as one snapshot.agents entry */ }
}
{
  "type": "agent_removed",
  "payload": { "agent_id": "agent-abc123" }
}

// heartbeat — agentbox keeps the connection warm; no data
{
  "type": "heartbeat",
  "payload": {}
}

// communication — emitted when one agent communicates with another.
// Maps to DDD-07's `AgentCommunicated` internal event.
{
  "type": "communication",
  "payload": {
    "from_agent_id": "agent-abc123",
    "to_agent_id": "agent-def456",
    "weight": 1.0
  }
}
```

Receiver rules: unknown `type` → log once, ignore, continue. Schema
version skew → close frame `4001 schema_version_unsupported`. Missing
required fields → drop frame, increment `telemetry_malformed_count`.

Back-pressure is **drop, never queue**, mirroring ADR-02 D3. Agent state
is eventually consistent — the next `delta` or `snapshot` corrects it.

### D2. Telemetry reconnection: exponential backoff, snapshot first

On disconnect: (1) mark agent nodes `stale = true` after 5 s without a
frame; rendering desaturates them (Section 4). (2) Reconnect with
backoff 1s, 2s, 4s, 8s, 16s, cap 30 s, jitter ±20% to avoid thundering
herd. (3) On reconnect, agentbox sends `snapshot` first; client
reconciles add/remove against local state and clears stale. (4) Client
never replays local state to agentbox — agentbox is authoritative.

### D3. Outbound action: click → `AgentActionEnvelope`

When the user clicks an agent node in VisionClaw, the renderer
dispatches an `AgentActionEnvelope`. The envelope shape,
BroadcastChannel constant, deep-link template, and
`AgentActionTargetOrigin` allowlist type are defined in
`crates/visionclaw-contracts/src/agent_action.rs` and generated as
`client/src/types/contracts/agent-action.d.ts`. The full schema is
reproduced in `_resolutions/T4-T6-T7-api-contracts.md` §T7. Receivers
MUST verify `type === "visionclaw:agent-action"` and
`schema_version === 1`; postMessage receivers additionally enforce
`event.origin` against the allowlist. Unknown `kind` values are no-ops
(forward-compatible). This envelope supersedes ADR-07 D8's
`RequestAgentControlSurface` intent.

```typescript
// Source of truth: crates/visionclaw-contracts/src/agent_action.rs
// Generated:       client/src/types/contracts/agent-action.d.ts
//                  also published as @visionclaw/contracts for agentbox/forum

/**
 * AgentActionEnvelope — outbound message from VisionClaw to agentbox/forum
 * dispatched when the user interacts with an agent node in the 3D scene.
 *
 * Transport selection happens once per session at handshake time:
 *   - same-origin: BroadcastChannel(AGENT_ACTION_CHANNEL)
 *   - cross-origin: window.open(deep-link)
 *   - embedded (window.parent !== window): window.parent.postMessage
 *
 * Receivers MUST:
 *   1. Verify `type === "visionclaw:agent-action"`
 *   2. Verify `schema_version === 1` (refuse with structured log otherwise)
 *   3. For postMessage delivery: verify event.origin against allowlist
 *   4. Treat any unknown `kind` as no-op (forward-compatible)
 *
 * Receivers MUST NOT:
 *   - Re-broadcast the envelope (one-way contract)
 *   - Trust `issued_by_pubkey` as auth (informational only; auth is
 *     established via the ADR-10 D4 bridge JWT)
 */
export interface AgentActionEnvelope {
  /** Discriminator. Always exactly this literal. */
  readonly type: "visionclaw:agent-action";

  /** Schema version. Bumped per ADR-10 D8 when payload semantics change. */
  readonly schema_version: 1;

  /** UUID v4, generated per click. Receivers MAY use this for dedup. */
  readonly message_id: string;

  /** Unix milliseconds at click time. */
  readonly issued_at_ms: number;

  /** Pubkey of the clicking user, npub format. Informational only —
   *  receiver verifies authorisation via its own bridge session. */
  readonly issued_by_pubkey: string;

  /** Action kind. Forward-compatible: receivers ignore unknowns. */
  readonly kind:
    | "open_panel"     // primary — open the agent's control panel
    | "show_logs"      // open the agent's log view
    | "show_swarm"     // open the parent swarm overview
    | "show_lineage";  // open the agent's parent-chain trace

  /** Agent identity. Required for every kind. */
  readonly agent_id: string;

  /** Swarm identity, when known. Not all agents belong to swarms. */
  readonly swarm_id?: string;

  /** Class flag bits from the V3 node id (ADR-08 §D6). Lets the receiver
   *  short-circuit if the click target is not actually an agent. */
  readonly node_class: "agent" | "knowledge" | "ontology";

  /** Click modifiers, for receivers that distinguish primary/secondary
   *  actions. All optional; receivers default to "primary" semantics. */
  readonly modifiers?: {
    readonly shift?: boolean;
    readonly ctrl?: boolean;
    readonly alt?: boolean;
    readonly meta?: boolean;
    readonly button?: 0 | 1 | 2; // 0=primary, 1=middle, 2=secondary
  };

  /** Cursor in scene world-space at click time. Used by the receiver to
   *  position popovers when rendering inside the same browser tab. */
  readonly cursor_world_position?: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };

  /** Bridge session id from the ADR-10 D4 auth flow. Receivers MAY use
   *  this to correlate the click with the bridge session that issued
   *  the Authorization for the originating VisionClaw tab. */
  readonly bridge_id?: string;
}

/** BroadcastChannel constant. Both sides import this literal. */
export const AGENT_ACTION_CHANNEL = "visionclaw:agent-actions" as const;

/** Deep-link template. Receivers parse incoming requests at this path.
 *  Deep-link is structurally untrusted (URL bar); receivers validate
 *  every field as if it arrived via BroadcastChannel. The `bridge_id`
 *  is the only field linking the click to an authenticated session;
 *  if absent or invalid, the receiver SHOULD challenge for re-auth. */
export const AGENT_ACTION_DEEP_LINK_TEMPLATE =
  "/agents/{agent_id}?source=visionclaw&kind={kind}" +
  "&issued_at={issued_at_ms}&issued_by={issued_by_pubkey}" +
  "&message_id={message_id}&node_class={node_class}" +
  "&bridge_id={bridge_id?}&swarm_id={swarm_id?}";

/** Allowed postMessage target origins. The bridge handshake (ADR-10 D4)
 *  establishes this list at session start; the receiver enforces it.
 *  Empty list => BroadcastChannel-only mode. */
export type AgentActionTargetOrigin =
  | "https://agentbox.example.com"
  | "https://forum.example.com";
```

**Origin-check requirement (normative)**:

| Transport | Required verification |
|-----------|----------------------|
| `BroadcastChannel` | `data.type === "visionclaw:agent-action"` AND `data.schema_version === 1`. Browser guarantees same-origin. |
| `window.postMessage` | All of the above PLUS `event.origin` matches `AgentActionTargetOrigin`. Allowlist established at bridge handshake. |
| Deep-link (URL) | All envelope checks PLUS treat every field as untrusted user input. `bridge_id` is the only authenticated link; if absent/invalid, challenge for re-auth before honouring `kind`. |

Delivery selection at session start (one transport per session, no
runtime fallback):

- If `BroadcastChannel` is available *and* the forum/agentbox window is
  detected as same-origin (via `window.open` handle returned from forum
  during auth handshake), the action is published on a
  `BroadcastChannel(AGENT_ACTION_CHANNEL)`. Agentbox/forum subscribes
  and renders the panel.
- Otherwise, the click opens a deep-link URL composed from
  `AGENT_ACTION_DEEP_LINK_TEMPLATE` via `window.open(url, 'agentbox')`.
  The browser focuses the existing agentbox tab if present, otherwise
  opens a new one.
- If VisionClaw is itself iframed (`window.parent !== window` at load),
  the click dispatches via `window.parent.postMessage(envelope, origin)`
  where `origin` is selected from the `AgentActionTargetOrigin`
  allowlist. This is not a fallback — it is the embedded-case primary
  transport. Selection happens once at session start.

Iframe embedding of agentbox UI inside VisionClaw is explicitly
rejected (PRD-10 §9). VisionClaw does not render the agent panel under
any circumstance.

`crates/visionclaw-contracts` is the source-of-truth Rust crate created
in the implementation phase; it owns this envelope and every other
cross-boundary contract (telemetry, enterprise events, binary-protocol
header constants). `ts-rs` generates the `.d.ts` consumed by the client
and published as `@visionclaw/contracts` for agentbox/forum.

### D4. Auth bridge: signed challenge, never shared cookies

The forum issues the user a Nostr identity (npub/nsec) per its own
registration flow (passkey-PRF-derived in current designs, per
`c75305b95`). To bridge that identity into VisionClaw:

```text
1. User clicks "Open VisionClaw" in the forum.
2. Forum opens VisionClaw at https://visionclaw.tld/?bridge=1.
3. VisionClaw generates a random 32-byte challenge, stores it in
   sessionStorage keyed by a bridge_id.
4. VisionClaw posts the challenge to forum via
   window.opener.postMessage({type:'visionclaw:challenge', bridge_id, challenge})
   OR via BroadcastChannel('visionclaw:auth').
5. Forum signs the challenge with the user's Nostr key (NIP-07 window.nostr
   if available, or its server-side keypair) and replies:
   { type:'visionclaw:challenge_response', bridge_id, npub, sig, challenge }
6. VisionClaw verifies sig against npub over challenge. If valid, it
   issues a session JWT (HS256, server-side secret) and stores it in
   sessionStorage (not localStorage, not cookies).
7. VisionClaw's REST API and WebSocket subscribe accept the JWT in the
   Authorization header.
```

Properties: no cookie sharing (disjoint origins permitted);
replay-resistant (single-use challenge, 60s server-side window);
forum-only signer (VisionClaw verifies, never sees private keys);
coarse RBAC label (`reader|operator|admin`) flows in the challenge
response and is enforced at VisionClaw's API gateway, not re-derived.
ADR-06 owns the JWT cryptography and gateway enforcement; ADR-10 owns
only the bridge contract.

### D5. Enterprise events: inbound-only subset, dedicated WebSocket

VisionClaw does **not** subscribe to the forum's full enterprise event
stream (workflow state, KPI updates, decision canvas activity). It
subscribes to a narrow projection of events that affect its rendering
or auth posture. The forum is responsible for emitting this
projection; VisionClaw is responsible for refusing to consume the
broader stream.

Envelope:

```json
{
  "schema_version": 1,
  "type": "membership_change" | "role_change" | "session_revoked",
  "issued_at_ms": 1715856020000,
  "payload": { /* type-specific */ }
}
```

Type-specific:

```json
// membership_change — user joined or left a org/workspace
{
  "type": "membership_change",
  "payload": {
    "npub": "npub1...",
    "org_id": "<uuid>",
    "action": "joined" | "left"
  }
}

// role_change — coarse RBAC label changed
{
  "type": "role_change",
  "payload": {
    "npub": "npub1...",
    "new_role": "reader" | "operator" | "admin"
  }
}

// session_revoked — forum invalidated a bridge JWT
{
  "type": "session_revoked",
  "payload": { "bridge_id": "<uuid>" }
}
```

Rendering/auth effects:

- `membership_change` may filter the graph (org-scoped subgraphs); the
  consumer hook lives in Section 8 (graph data access).
- `role_change` updates the JWT claim and disables operator-only UI
  affordances within ≤2s.
- `session_revoked` forces the consumer to drop the JWT and prompt
  re-authentication.

Anything beyond these three types is forum's internal business and
must not appear on this channel.

### D6. Whelk-rs: pure function, no I/O

The contract:

```rust
pub struct WhelkInferenceRequest {
    pub tbox_axioms: Vec<TBoxAxiom>,
    pub abox_assertions: Vec<ABoxAssertion>,
    pub iri_prefix_map: BTreeMap<String, String>,
}

pub struct WhelkInferenceResponse {
    pub inferred_assertions: Vec<ABoxAssertion>,
    pub diagnostics: WhelkDiagnostics,
}

pub trait WhelkReasoner: Send + Sync {
    fn infer(&self, req: WhelkInferenceRequest) -> Result<WhelkInferenceResponse>;
}
```

Properties:

- **Pure function**. No reads from disk, no network, no global state.
- **One-shot invocation**. Whelk does not run as an actor with state.
- **Logging through the diagnostics struct**, not through global log
  sinks. Caller decides what to log.
- **Errors are structured**. Inconsistent ontologies, axiom-classification
  failures, and resource exhaustion are distinct error variants.
- **Cancellable via a deadline**. Caller may pass an optional
  `deadline: Instant`; whelk returns a `Timeout` error rather than
  spinning indefinitely on pathological inputs.

Whelk's vendored crate at `whelk-rs/` retains its upstream identity.
Patches go upstream; the in-tree copy tracks tagged upstream commits.
The contract above is implemented by an adapter at
`src/adapters/ontology/whelk_reasoner.rs` that wraps the crate API.

### D7. No enterprise control logic in VisionClaw

The migration is enforced by a CI check that searches the tree for any
of the following names under disallowed paths:

| Disallowed name      | Disallowed paths                                  |
|----------------------|---------------------------------------------------|
| `broker`             | `src/handlers/api_handler/`, `client/src/features/enterprise/` |
| `workflows`          | `src/handlers/api_handler/`                       |
| `connectors`         | `src/handlers/api_handler/`                       |
| `mesh_metrics`       | `src/handlers/api_handler/`                       |
| `policy`             | `src/handlers/api_handler/` (engine; *contract* OK) |
| `decision_canvas`    | anywhere                                          |
| `kpi`                | `src/handlers/api_handler/`                       |
| `EnterpriseDrawer*`  | `client/src/`                                     |
| `enterprise-standalone` | `client/src/`                                  |

The check is a `cargo xtask check-no-enterprise` step in CI. PRs that
trip it are blocked.

Additionally, scan for re-introduction of deprecated bots control-plane
route names (`/initialize-swarm`, `/spawn-agent`, `/spawn-agent-hybrid`,
`/remove-task`, `/bots/data` POST, `/bots/update` POST) under
`src/handlers/`. The Phase 7b removal date in ADR-07 D12 is the date
these become hard CI failures.

### D8. Versioning policy for the contracts

Every envelope carries `schema_version`. Bump rules:

- **Backwards-compatible field added**: keep `schema_version = 1`.
  Producers may emit; consumers must ignore unknown fields.
- **Backwards-incompatible change** (renamed field, removed field,
  changed enum variants, transport semantics changed): bump to
  `schema_version = 2`. VisionClaw refuses unsupported versions per
  D1. Migrating to a new version requires coordinated deploys, not
  rolling deploys.
- **Both sides keep one back-version of support** so the deploy
  window is non-zero.

Contract test fixtures live in `tests/contracts/external-integrations/`
and are versioned alongside the schema.

### D11. GitHub adapter (Logseq corpus)

Transport: `octocrab` REST client. Auth: `GITHUB_TOKEN` from environment.
Output value object: `ParsedMarkdown` as defined by DDD-08 §"Anti-corruption
layer to Section 10" — Section 10 is the transport, Section 8 owns the
parse and the value-object shape.

Sync gating: `GitHubSyncService::sync_graphs()` SHA1-compares each file's
blob against the cached hash and skips unchanged files. The
`FORCE_FULL_SYNC=1` environment variable bypasses gating and forces full
reparse — used for content-format migrations. Set back to `0` after use.

Parse-error envelope: errors carry
`{ path: string, sha: string, error_kind: "yaml" | "wikilink" | "ontology-block" | "io", message: string }`.
Errors are logged but do not fail the sync; the failed file is retained
at its previous good version in Neo4j and surfaced as an operator metric
`github_sync_parse_errors_total{error_kind}`.

Implementation already exists at `src/services/github_sync_service.rs`;
this decision codifies the existing contract so Section 8 has a named
upstream rather than a phantom one.

### BroadcastChannel naming convention

Prefix: `visionclaw:`. Suffix: kebab-case noun describing the channel's
payload. Current channels: `visionclaw:agent-actions` (D3),
`visionclaw:auth` (D4). New channels register here in the same PR that
introduces them.

## Options considered

- **O1. One bidirectional protocol for telemetry + enterprise + actions.**
  Rejected. Mixing channels couples their failure modes; the freeze
  investigation showed that overloaded broadcast channels make
  buffer-pressure analysis intractable.
- **O2. HTTP polling for telemetry with WebSocket upgrade.** Rejected per
  PRD-10 §9 — fallback paths become primary under stress.
- **O3. Shared subdomain cookies for auth.** Rejected. Forum and
  VisionClaw may live on disjoint origins; signed-challenge over
  postMessage is origin-agnostic.
- **O4. Iframe-embed agentbox UI inside VisionClaw.** Rejected. CSP
  boundaries, storage isolation, and click-jacking complicate the
  integration; BroadcastChannel/deep-link matches the real usage pattern.
- **O5. Whelk as a long-running reasoner actor.** Rejected. Whelk is a
  function (TBox + ABox → inferred ABox); actor-wrapping it adds state
  that must be invalidated.
- **O6. Adopt the chosen contracts (this ADR).** Adopted. Each external
  concern gets one well-typed surface; each surface is one-way unless
  explicitly stated; each surface fails predictably.

## Risks

- **R1**: agentbox is built by a different team on a different release
  schedule. Mitigation: contract tests (PRD-10 §7) run against an
  agentbox-emulator harness, so VisionClaw can prove conformance
  without waiting for agentbox availability.
- **R2**: BroadcastChannel API has uneven browser support for less-common
  browsers (Safari < 15.4). Mitigation: deep-link is the canonical
  fallback at session-start (not runtime), with feature detection at
  the bridge handshake step.
- **R3**: The forum's Nostr identity model may evolve faster than
  VisionClaw's auth implementation. Mitigation: ADR-06 owns the JWT
  surface; D4's contract decouples Nostr-identity-lifecycle from
  VisionClaw's session lifecycle by exchanging at bridge time only.
- **R4**: Whelk version bumps may change inferred-triple sets. Mitigation:
  the golden-file inference fixture (PRD-10 §7) flags this at PR time,
  so the ontology team can ratify or revert.
- **R5**: CI check D7 may false-positive on documentation files mentioning
  the disallowed names. Mitigation: limit the scan to source code paths
  (`src/`, `client/src/`), exclude `docs/`, `tests/contracts/`.

## Rejected from main as buggy / unjustified

- The full enterprise platform (commits `a61a9c095`, `1e1303e75`,
  `ed4aac368`, `15216949c`, `c3bea48b3`, `fe5fdb184`, `7d076d93a`,
  `fcfc1a166`, `ea0e2f50f`, `74d503112`). Removed in `c64661e97`; this
  ADR codifies why it does not return: VisionClaw is a visualisation
  client, not a control plane.
- Any inbound contract that requires VisionClaw to maintain an
  authoritative agent state machine. Agentbox is authoritative; D1's
  drop-and-resync policy is sufficient.
- Any bidirectional Nostr publishing from VisionClaw (relay writes,
  event signing). VisionClaw verifies; it does not author. PRD-10 §9.

## Bugs and smells at the reset point (41979d33e)

At baseline:

- `BotsClient` (or equivalent) speaks a JSON stream without
  `schema_version`. D1 introduces the field; migration is a one-time
  envelope shape change at the consumer.
- Nostr login plumbing exists in fragmentary form per the commit log
  (`b8f28117b`, `4b91d3a93`, `81ad98f11`, etc.) but does not yet
  separate the *bridge* from the *session*. D4 splits these.
- Whelk is vendored as a crate but called through a non-pure adapter
  in some code paths. D6's `WhelkReasoner` trait formalises the
  pure-function discipline; existing call sites refactor to use it.
- No CI guard against re-introduction of enterprise modules exists at
  baseline. D7's check is new and is part of the migration's gate.
