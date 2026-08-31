# DDD — Nostr Mobile Agent Bridge Bounded Context

| Field | Value |
|-------|-------|
| Status | Draft (2026-06-02) |
| Drives | PRD-017 |
| Companion ADRs | ADR-092, ADR-093, ADR-094, ADR-095, ADR-096, ADR-097 |
| Sibling DDDs | `docs/ddd-mesh-federation-context.md` (BC-MESH-AGENTBOX, BC-MESH-FORUM), `agentbox/docs/reference/ddd/DDD-003-sovereign-messaging-domain.md` |
| Evidence | `docs/integration-research/nostr-mobile-bridge/01-08` |

## Purpose

This document fixes the bounded context for the mobile agent bridge: a new context, **BC-MOBILE-BRIDGE**, that sits between the operator's phone (a Nostr identity) and the agentbox agent substrate. It names the aggregates, their invariants, the domain events that move between them, the ubiquitous language, and — most importantly — the anti-corruption layers at each boundary where one context's model must not leak into another.

The bridge's core domain challenge is not message delivery; it is **trust translation**. A phone-held delegated key, an admin grant, a chat session, and a durable summary are four objects whose relationships must stay consistent: a message authorises agent action only if the delegation chain back to the admin pubkey is intact; a summary attributes work to the right session and the right owner. When a translation drops the delegation or mis-attributes the session, either an unauthorised message acts, or a user loses ownership of their own session record.

This context deliberately does NOT own the forum (BC-MESH-FORUM) — Phase 3, out of scope (ADR-097 D4).

---

## Ubiquitous Language

| Term | Definition |
|------|------------|
| **Operator** | The human holding the agentbox admin identity (Dr John O'Hare). Single authority in this context. |
| **Admin pubkey** | `AGENTBOX_X_ONLY_PUBKEY_HEX` or an entry in `agentbox.toml admin_pubkeys`. The root of all authorisation. |
| **Phone key** | The operator's device keypair, held in Amber (NIP-55 signer). Has its own `did:nostr`. NOT the admin key. |
| **Delegation** | A NIP-26 token signed by the admin key authorising the phone key to act on its behalf, kind+time bounded. |
| **Chat session** | One ad-hoc conversation between operator (via phone) and an agent. 1:1 with a Claude Code session. |
| **Rumor** | The innermost NIP-17 kind-14/15 message, after gift-wrap unwrap and seal unseal. Carries the real author + content. |
| **Gift wrap** | The kind-1059 NIP-59 outer envelope. What the relay stores and isolates per-recipient. |
| **Dispatch** | Handing a verified, authorised prompt to an agent (via `AGENTBOX_INTENT_COMMAND`). |
| **Session summary** | The kind-30840 event + pod JSON-LD resource describing what an agent did in a session. |
| **Mandate** | The user-installed WAC ACL (NIP-33 kind-30078) granting the agent write access to the operator's `/sessions/`. |

---

## Bounded Context: BC-MOBILE-BRIDGE

**Owner**: agentbox project. **Mission**: provide a `did:nostr`-authenticated, sovereign mobile control surface into agentbox agents, recording each session as a user-owned summary.

### Aggregates

#### `MobileIdentity` (aggregate root: phone pubkey)
{ `phone_pubkey_hex`, `phone_did = did:nostr:<hex>`, `delegation: Delegation`, `allowlisted_on: [relay_url]` }

- The phone's first-class mesh identity. Derived locally from the keypair (research 04 §1.5) — no service dependency to exist.
- Holds the current `Delegation` it operates under.

#### `Delegation` (value object inside MobileIdentity)
{ `delegator_pubkey` (= admin), `delegatee_pubkey` (= phone), `conditions: kind=14&kind=1059&created_at>T0&created_at<T1`, `sig`, `valid_from`, `valid_until` }

- The trust link. Created by `DelegationToken::create` (`nip26.rs:150`), validated by `validate_delegation_tag` (`nip26.rs:258`).

#### `ChatSession` (aggregate root: session_id)
{ `session_id`, `phone_pubkey`, `agent_pubkey`, `relay_url`, `started_at`, `last_activity`, `status: active|complete|failed`, `claude_code_session_id` }

- 1:1 with a Claude Code session (mirrors CTM's session↔topic mapping, research 01 §4, but keyed by `did:nostr` not Telegram thread).
- Owns the conversation; transitions to `complete`/`failed` trigger `SessionSummary` creation.

#### `SessionSummary` (aggregate root: (agent_pubkey, session_id) — the kind-30840 address)
{ `session_id`, `agent_pubkey`, `admin_pubkey`, `phone_pubkey`, `start`, `end`, `status`, `content: {title, summary, tool_calls, tokens_used, outcome}`, `pod_uri`, `action_urn` }

- Addressable (NIP-33 `d`-tag); re-publish replaces in place (ADR-095 D1).
- Dual-homed: kind-30840 relay event (phone-readable) + pod JSON-LD (canonical, owned). Same logical content.

#### `AdminGrant` (aggregate root: (agent_pubkey, container))
{ `agent_pubkey`, `container = /sessions/`, `modes: [Read, Write, Append]`, `mandate_event_id`, `revoked: bool` }

- The pod-write authority. NIP-33 kind-30078 mandate event + WAC ACL on the pod (ADR-096 D3). Operator-installed once.

### Invariants (BC-MOBILE-BRIDGE-Inv)

- **MB-Inv-01**: No `ChatSession` dispatch occurs unless the inbound rumor's signature verifies AND its author is the admin pubkey or a key with a valid `Delegation` from the admin (ADR-094 D1). Signature check precedes authorisation check.
- **MB-Inv-02**: A `Delegation` is honoured only after its delegator Schnorr signature is verified (`nip26.rs:190`) and its conditions permit the event kind and `created_at` (ADR-094 D3 / BLOCK-3). A delegation tag is never trusted unverified.
- **MB-Inv-03**: The admin nsec never leaves the agentbox. The phone holds only its own key + a delegation (ADR-094 D2). Container-key stability (sibling A-Inv-04) is preserved.
- **MB-Inv-04**: Every `SessionSummary` is written to BOTH the relay (kind-30840) and the operator's pod. Partial-failure retry is safe via `d`-tag idempotency (relay) and deterministic path (pod) (ADR-095 D2).
- **MB-Inv-05**: Pod writes are signed by the AGENT's `did:nostr` under an `AdminGrant` mandate — never with the operator's key (ADR-096 D3). The operator's nsec is structurally absent from the write path.
- **MB-Inv-06**: kind-1059 reads delivered to the phone are filtered to `#p == phone_pubkey` (NFR-3). Cross-recipient leakage is structurally impossible (mirrors forum F-Inv-03, `nip_handlers.rs:348-387`).
- **MB-Inv-07**: NIP-04 (kind 4) is never emitted or accepted by this context (ADR-093 D2).
- **MB-Inv-08**: A `ChatSession` maps 1:1 to a Claude Code session; sub-agent activity attributes to the parent session (mirrors CTM parent-child, research 01 §4).

### Domain Events

| Event | Emitted when | Carried payload |
|-------|--------------|-----------------|
| `MobileMessageReceived` | gift wrap unwrapped + verified + authorised | rumor content, phone_pubkey, session_id |
| `MessageRejected` | signature fails, or author not admin/delegated, or delegation invalid/expired | reason, offending pubkey (for audit) |
| `AgentDispatched` | authorised prompt handed to agent | session_id, prompt, agent_pubkey |
| `AgentTurnProduced` | agent emits assistant output | session_id, text → NIP-17 DM to phone |
| `SessionStarted` / `SessionEnded` | Claude Code session start/end | session_id, timestamps |
| `SummaryComposed` | session end / checkpoint | summary content |
| `SummaryPublished` | kind-30840 event published | event_id, address |
| `SummaryPersisted` | pod JSON-LD written | pod_uri, action_urn |
| `DelegationIssued` / `DelegationExpired` | admin issues/lets-expire a phone delegation | delegation token, window |

---

## Context Map

```
            ┌─────────────────────────────────────────────┐
            │           BC-MOBILE-BRIDGE                   │
            │  MobileIdentity · Delegation · ChatSession   │
            │  SessionSummary · AdminGrant                 │
            └───┬───────────────┬──────────────────┬───────┘
                │               │                  │
        ACL-1 (NIP-17/59   ACL-2 (dispatch    ACL-3 (pod write
        decrypt + verify)  to agent)          via NIP-98 mandate)
                │               │                  │
                ▼               ▼                  ▼
   ┌────────────────────┐ ┌──────────────┐ ┌──────────────────────┐
   │ Relay substrate    │ │ BC-MESH-     │ │ Solid Pod context     │
   │ (agentbox embedded;│ │ AGENTBOX     │ │ (solid-pod-rs :8484)  │
   │  CF relay Phase 2) │ │ SovereignAgent│ │ Pod · WAC · WebID     │
   └─────────┬──────────┘ └──────────────┘ └──────────────────────┘
             │ ACL-4 (Phase 2 federation: MESH_MODE, allowed_remote_dids)
             ▼
   ┌────────────────────┐
   │ BC-MESH-FORUM       │  ← Phase 3 only; NO relationship in Phase 1/2
   │ (CF relay + BBS)    │
   └────────────────────┘
```

### Relationships

- **BC-MOBILE-BRIDGE → Relay substrate** (Customer/Supplier): the bridge consumes the relay's transport + per-recipient isolation. Conformist to NIP-01/17/42/59.
- **BC-MOBILE-BRIDGE → BC-MESH-AGENTBOX** (Partnership): the bridge dispatches into the existing `SovereignAgent`/intent-queue machinery; shares the agentbox `did:nostr` identity.
- **BC-MOBILE-BRIDGE → Solid Pod context** (Customer/Supplier): the bridge is a pod client (via the agent) under a mandate; conformist to WAC + NIP-98.
- **BC-MOBILE-BRIDGE ⇏ BC-MESH-FORUM**: **no relationship in this PRD.** Phase 3 only (ADR-097 D4). The kind allocation reserves forum kinds to keep this additive.

---

## Anti-Corruption Layers

### ACL-1 — Relay ingress → bridge (decrypt + trust translation)

The single most important ACL. The relay speaks gift wraps (kind-1059); the bridge needs verified, authorised rumors. This layer:
1. Unwraps kind-1059 → unseals kind-13 → recovers kind-14 rumor (NIP-44 v2 decrypt — net-new, ADR-093 D6, BLOCK-1).
2. Verifies the rumor's Schnorr signature (MB-Inv-01).
3. Resolves the delegation chain to the admin pubkey (MB-Inv-02, BLOCK-3).
4. Emits `MobileMessageReceived` only if all pass; else `MessageRejected`.

This is where Telegram's "chat_id membership" model (research 01 §6) is replaced by `did:nostr` signature + delegation. The CTM `IdentityStore` schema (`identity.rs:91-107`) is reusable as the allow-cache, but the enforcement is signature-based, not Telegram-id-based.

### ACL-2 — Bridge → agent dispatch

Translates a `ChatSession` prompt into the agentbox intent format (`AGENTBOX_INTENT_COMMAND`). Phase 1 spawns a fresh agent task; Phase 2 may resume a tmux-attached Claude Code session (the co-located injector model, research 01 §3 — requires the local injector CTM uses). Keeps the bridge's `ChatSession` model from leaking into the agent's execution model.

### ACL-3 — Agent → pod write (NIP-98 mandate)

Translates a `SessionSummary` into a NIP-98-authenticated pod PUT under the `AdminGrant` mandate (`buildPodNip98`, `pod-signer.js:76-80`). Enforces MB-Inv-05 (agent identity, never operator). Payload-hash binding (BLOCK-2) lives here.

### ACL-4 — Phase 2 federation (agentbox relay ↔ CF relay)

Translates between the two relays' admission models. The CF relay gates writes on its D1 whitelist (research 03 §3) and federation on `allowed_remote_dids` (research 03 §7); the agentbox relay must present matching `did:nostr` provenance. Net-new forwarder (ADR-097 D3). Dormant until Phase 2.

---

## Mapping to existing contexts

| This context | Reuses from sibling DDD |
|--------------|-------------------------|
| `MobileIdentity` / `Delegation` | `nip26.rs` (BC-MESH-FORUM source of truth); admin pubkey from BC-MESH-AGENTBOX `SovereignAgent` |
| `ChatSession` | session-lifecycle pattern from CTM (research 01 §4), re-keyed to `did:nostr` |
| `SessionSummary` pod write | `PodOutbox` write pattern + `pod-signer` from BC-MESH-AGENTBOX |
| `AdminGrant` | `mandate.js` WAC Turtle rendering (research 05 §1) |
| ACL-1 decrypt | the gift-wrap path ADR-009:262 deferred — this context builds it |

This context introduces no new identity primitive; it composes existing ones (`did:nostr`, NIP-26, NIP-98, WAC) into a new trust-translation surface. That composition — not any single primitive — is the new domain.
