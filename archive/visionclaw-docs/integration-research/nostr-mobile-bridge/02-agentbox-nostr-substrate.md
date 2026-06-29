# 02 — Agentbox Nostr Substrate Research

**Date:** 2026-06-02
**Scope:** How agentbox exposes AI agents over Nostr; what a phone→relay→agent bridge requires.
**Audience:** Integration architect designing a phone DM → agentbox agent → reply flow.

---

## 1. Embedded Nostr Relay

### Decision record

ADR-009 (`docs/reference/adr/ADR-009-embedded-nostr-relay.md`) ratifies the embedding of `nostr-rs-relay` 0.9.0 (Apache-2.0, from nixpkgs) as a supervisord-managed program gated on `[sovereign_mesh.relay].enabled`.

### Runtime

| Property | Value | Source |
|---|---|---|
| Binary | `nostr-rs-relay` 0.9.0 (Nix-pinned) | ADR-009 §Components |
| Default port | **7777** (`ws://127.0.0.1:7777`) | `agentbox.toml` manifest schema, ADR-009 §Manifest |
| Default bind | `127.0.0.1` (loopback) | ADR-009 §Manifest |
| External exposure | opt-in via `expose = true` | `docs/user/nostr-relay.md:129` |
| Persistence | SQLite under `/var/lib/nostr-relay` | ADR-009 §Components |
| NIP-42 AUTH | mandatory when `ingress_policy != "open"` | ADR-009 §NIP support matrix |

### Accepted kinds (default `allowed_kinds` list)

| Kind | NIP | Purpose |
|---|---|---|
| 1 | NIP-01 | General notes |
| 1059 | NIP-17 | Sealed gift-wrap DMs (recommended inbound channel) |
| 27235 | NIP-98 | HTTP auth events (bridge handles out-of-band) |
| 30078 | NIP-33 | Parameterised replaceable agent state |
| 31400 | NIP-33 | ACSP — PanelDefinition |
| 31401 | NIP-33 | ACSP — PanelState |
| 31402 | NIP-33 | ACSP — ActionRequest |
| 31403 | NIP-33 | ACSP — ActionResponse |
| 31404 | NIP-33 | ACSP — PanelUpdate |
| 31405 | NIP-33 | ACSP — PanelRetired |
| 38000-38099 | reserved | Agent-intent (inbound request for agent action) |
| 38100-38199 | reserved | Agent-response (reply to agent-intent) |
| 38200 | — | Job cost estimate (payment) |
| 38201 | — | Job settlement receipt |

Source: `docs/user/nostr-relay.md:161-175`, `mcp/nostr-bridge/relay-consumer.js:149-165`.

Note: kind 4 (unencrypted legacy DMs) is **off by default** (`allow_nip04 = false`). Enabling it triggers validator warning E031. Source: `docs/user/nostr-relay.md:188`.

### Gate condition

The relay and the bridge are only active when `[sovereign_mesh.relay].enabled = true`. When the flag is false, no relay process starts and the bridge falls back to the pre-ADR-009 outbound-only mode (public relays via `NOSTR_RELAYS`). Source: ADR-009 §Components, `docs/user/nostr-relay.md:236-238`.

---

## 2. The Nostr Bridge: Architecture and Event Routing

### Two components in-process with management-api

The bridge is composed of two modules, both loaded in-process (same rationale as noted in `mcp/servers/nostr-bridge.js:1-26`: avoids IPC latency and keeps key material in one process):

1. **`mcp/servers/nostr-bridge.js`** — core library: `RelayConnection`, `NostrBridge`, `loadSigner`, `verifyNip98`, `buildNip98Header`, payment event helpers.
2. **`mcp/nostr-bridge/relay-consumer.js`** — the ADR-009 runtime: `RelayConsumer` class that connects to the loopback relay, verifies signatures, and dispatches to pod inbox / orchestrator / governance directories.

### Inbound event routing (the full trace)

```
External sender
  --> ws://<host>:7777 (nostr-rs-relay)
      NIP-42 AUTH challenge/response
  --> EVENT published (e.g. kind=1059 or kind=38000)
      nostr-rs-relay persists to SQLite
      nostr-rs-relay fans out to subscribers

  --> relay-consumer.js _onInbound(event, relayUrl)   [relay-consumer.js:236]
      1. _verifySig(event)                             [relay-consumer.js:398]
         nostrTools.verifyEvent(event) -- Schnorr
         DDD-003 I01: no pod write without valid sig
      2. _passesIngressPolicy(event)                   [relay-consumer.js:415]
         'allowlist': event.pubkey in _allowedPubkeys
         'signed-only': sig present
         'open': always pass
      3. _findRecipientNpub(event)                     [relay-consumer.js:429]
         scan event.tags for ['p', <npub>] matching a local npub
         reject if no match (I10)
      4. dedup check: <inbox>/<event.id>.json exists?  [relay-consumer.js:257]
      5. _formatAsLdn(event, recipient, relayUrl)       [relay-consumer.js:485]
         AS2 JSON-LD with x:nostrEvent + x:envelope extension
      6. atomic write: pods/<npub>/events/inbox/<event.id>.json   [relay-consumer.js:268]
      7. adapters.events.dispatch({direction:'inbound', event})   [relay-consumer.js:282]
      8. Branch on kind:
         a. _isPaymentEvent (38200/38201) -> payments/ subdir         [relay-consumer.js:296]
         b. _isGovernanceEvent (31400-31405) -> governance/ subdir    [relay-consumer.js:303]
            -> kind 31403 (ActionResponse) -> orchestrator.handleGovernanceDecision [relay-consumer.js:311]
         c. _isAgentIntent (38000-38099) -> intent-queue/ marker      [relay-consumer.js:321]
            -> if intentSpec + orchestrator: orchestrator.spawnAgent(spec) [relay-consumer.js:357]
```

### Is there an inbound to agent to reply loop today?

**Conditionally yes**, with two prerequisites:

**Prerequisite A — governance path (ActionRequest/Response round-trip):**
An external party sends kind 31403 (ActionResponse) to the relay. The relay-consumer calls `orchestrator.handleGovernanceDecision(event)` at `relay-consumer.js:312`. The orchestrator adapter routes it to VisionClaw's `BrokerActor`. The agent can then write a reply to `events/outbox/`; the outbox flusher signs and publishes it back to the relay. This is a working loop but it depends on BrokerActor being connected.

**Prerequisite B — agent-intent path (direct agent dispatch):**
An external party sends kind 38000 (agent-intent). The relay-consumer calls `orchestrator.spawnAgent(spec)` at `relay-consumer.js:357`. This path requires:
1. `opts.intentSpec` function to be provided to `RelayConsumer` at construction (`relay-consumer.js:125`).
2. The default intent spec (`mcp/nostr-bridge/default-intent-spec.js:63`) only activates when `AGENTBOX_INTENT_COMMAND` env var is set.
3. `opts.adapters.orchestrator.spawnAgent` to be wired.

The spawned agent inherits `NOSTR_EVENT_ID`, `NOSTR_EVENT_KIND`, `NOSTR_EVENT_PUBKEY`, `NOSTR_RECIPIENT_NPUB`, `NOSTR_EVENT_JSON` in its environment (`relay-consumer.js:349-355`). The agent writes a response event to `pods/<npub>/events/outbox/<id>.json`; the outbox flusher (`relay-consumer.js:600`) picks it up, signs via `loadSigner()`, and publishes to the relay (and to `NOSTR_RELAYS` if fanout is on).

**Without `AGENTBOX_INTENT_COMMAND` set:** no agent is spawned on an inbound intent event. Only a durable marker file is written to `intent-queue/`. A downstream poller must pick it up. There is no automatic reply.

**For ad-hoc DMs (kind 1059):** the relay-consumer subscribes to kind 1059 (`relay-consumer.js:152`) and will persist the event to the pod inbox, but there is **no automatic agent dispatch for kind 1059**. It is not in the agent-intent range (38000-38099) and is not a governance event (31400-31405). The inbox write is the full pipeline today for kind 1059. A separate poller or intent-router would need to detect new inbox entries and trigger an agent.

---

## 3. Agent Control Surface Protocol (ACSP)

### Kind semantics

Defined in `mcp/servers/nostr-bridge.js:54-71` (canonical `kinds` const) and `management-api/lib/agent-control-surface.js`.

| Kind | Label | Direction | Purpose |
|---|---|---|---|
| 31400 | PanelDefinition | Agent to relay | Agent declares an interactive control panel schema (fields, actions, layout) |
| 31401 | PanelState | Agent to relay | Agent publishes current data snapshot for a panel |
| 31402 | ActionRequest | Agent to relay | Agent requests a human governance decision |
| 31403 | ActionResponse | Human to relay | Human responds to an action request |
| 31404 | PanelUpdate | Agent to relay | Agent publishes an incremental state diff for a panel |
| 31405 | PanelRetired | Agent to relay | Agent signals that a panel is permanently retired |

All are NIP-33 parameterised-replaceable events. The `d` tag carries `panelId`; re-publishing the same `panelId` replaces the prior state. Source: `agent-control-surface.js:24-25`, `agent-control-surface.js:66-82`.

### d-tag scheme

`panelId` is operator-chosen and must be a non-empty string. It forms the entire NIP-33 `d` tag. The tuple `(kind, pubkey, d-tag)` uniquely identifies a replaceable record. Source: `agent-control-surface.js:88`.

`ActionRequest` (31402) uses `caseId` (not `panelId`) as the `d` tag, with an `e` tag pointing to the originating panel event. Source: `governance-bridge.js:267-269`.

### Who can publish each kind

The relay's `agent_registry` table gates which pubkeys may publish kinds 31400-31405. Only registered agent pubkeys are admitted. Source: `agent-control-surface.js:9-14`. ActionResponse (31403) is published by human operators through the forum UI (dreamlab-ai-website using nostr-bbs-forum-client). The relay routes 31402 into the `broker_cases` governance inbox.

### Publisher path in agentbox

Two routes exist:
1. `agent-control-surface.js:buildPanelDefinition/buildPanelState/buildActionRequest/buildPanelUpdate/buildPanelRetired` + `publishPanelEvent()` -- builds an unsigned event and publishes via an already-connected `NostrBridge`. Source: `agent-control-surface.js:233-244`.
2. `governance-bridge.js` (MCP server) -- writes unsigned JSON to `pods/<npub>/events/outbox/<panelId>.json`; the outbox flusher adds `pubkey`, `id`, and `sig` before publishing. Source: `governance-bridge.js:232-242`.

### Can ad-hoc chat ride on ACSP?

ACSP has a schema variant called `chat-bridge` (value in `PANEL_SCHEMAS` at `agent-control-surface.js:38`). This hints that ACSP was designed to accommodate chat sessions. However, ACSP is fundamentally a **governance / human-decision** channel, not a free-form messaging channel. The `ActionRequest`/`ActionResponse` pattern is pull-based (agent asks, human decides); it is not well-suited to ad-hoc bidirectional chat initiated by a phone.

**Recommendation:** ad-hoc chat should ride on **kind 38000 (agent-intent) with a reply on kind 38100**, or on **NIP-17 kind 1059** once a decryption + dispatch layer is added. ACSP can complement (by letting agents request clarifications as ActionRequests), but it should not be the primary chat transport.

---

## 4. Identity and Admin-Key Gate

### Key derivation

`scripts/sovereign-bootstrap.py` is the authoritative identity generator.

| Variable | Purpose | How set | Source |
|---|---|---|---|
| `AGENTBOX_PRIVKEY_HEX` | 64-char hex private key seed | Set in `.env`; takes priority over generated key | `sovereign-bootstrap.py:131` |
| `AGENTBOX_NSEC` | bech32 `nsec1...` alternative | Fallback if `AGENTBOX_PRIVKEY_HEX` not set | `sovereign-bootstrap.py:133` |
| `AGENTBOX_PUBKEY` | BIP-340 x-only pubkey hex (runtime env) | Written by bootstrap to `/run/agentbox/identity.env` as `AGENTBOX_X_ONLY_PUBKEY_HEX` | `sovereign-bootstrap.py:267-269` |
| `MANAGEMENT_API_KEY` | Bearer token for management-api | Set in `.env` | `management-api/server.js:29` |
| `NOSTR_RELAYS` | Comma-separated outbound relay URLs | Set in `.env` | `scripts/start-agentbox.sh:995` |
| `SOLID_ADMIN_KEY` | PSK for solid-pod-rs admin provisioning | Set in `.env` | `admin-users.js:29` |

`AGENTBOX_PRIVKEY_HEX` is the operator's stable Nostr signing identity. It seeds the secp256k1 keypair that signs all Nostr events published by agentbox. It is stored on disk as `nostr.key.enc` (AES-256-GCM, PBKDF2-SHA256 100k iterations against `MANAGEMENT_API_KEY`). Source: `mcp/servers/nostr-bridge.js:579-623`.

`SOLID_ADMIN_KEY` is used only for HTTP provisioning calls to `solid-pod-rs` via `X-Pod-Admin-Key` header. It is not a Nostr key. Source: `admin-users.js:48`.

### How "permission against the admin pubkey" is enforced today

Three enforcement layers, applied in order:

**Layer 1 — Relay ingress (NIP-42 allowlist)**
`ingress_policy = "allowlist"` in `agentbox.toml` requires every connecting pubkey to complete NIP-42 AUTH and be present in `allowed_pubkeys`. The phone's pubkey must be in this list or the relay will refuse its events. Source: ADR-009 §Validator rules, `relay-consumer.js:415-427`.

**Layer 2 — Bridge p-tag check**
Even if the relay admits the event, the relay-consumer checks that the event's `p` tag matches a local npub (`relay-consumer.js:429-438`). Events addressed to other pubkeys are silently dropped. To reach the agentbox agent, the phone must set `p = <AGENTBOX_PUBKEY>`.

**Layer 3 — management-api auth (NIP-98 or Bearer)**
All routes under management-api (port 9090) are protected by `createAuthMiddleware` (`server.js:183`). In `strict-nip98` mode (auto-elevated when `AGENTBOX_SOVEREIGN_MESH_ENABLED=true`), only NIP-98 is accepted. Source: `management-api/middleware/auth.js:101-111`. The authenticated pubkey is available as `request.auth.pubkey` but there is no secondary check that `auth.pubkey === AGENTBOX_PUBKEY` on most routes — authentication succeeds for any valid NIP-98 signer.

**What is NOT enforced today:** a per-route check that the caller's NIP-98 pubkey matches `AGENTBOX_PUBKEY` or a configured admin list. The relay-consumer's allowlist is the primary per-pubkey gate on the Nostr path. The management-api NIP-98 gate verifies cryptographic identity but accepts any valid signer, not just the operator.

For the phone bridge, the practical admin-gate is: add the phone's pubkey to `allowed_pubkeys` in `agentbox.toml`. Without that, the relay rejects the connection at the NIP-42 AUTH step.

---

## 5. Encryption: NIP-04 / NIP-44 / NIP-17 / NIP-59

### What is supported

| NIP | Status | Detail |
|---|---|---|
| NIP-04 (kind 4, XOR DM) | Off by default; configurable | `allow_nip04 = false` in manifest; validator E031 if enabled. The relay-consumer does NOT decrypt NIP-04 content -- events land in pod inbox as opaque ciphertext. `nostr-relay.md:188` |
| NIP-17 (kind 1059, sealed gift-wrap) | Subscribed + stored; NOT decrypted | `relay-consumer.js:152` subscribes to kind 1059. Events are persisted to pod inbox as LDN/AS2 wrappers, but `_formatAsLdn` stores raw `event.content` without attempting decryption. The ADR-009 follow-up note (`ADR-009:262`) explicitly defers "NIP-17 full decryption path" to a future PR. |
| NIP-44 (versioned encryption) | Not referenced | No import of NIP-44 primitives in any source file reviewed. |
| NIP-59 (gift wrap) | Not explicitly handled | Kind 1059 is the gift-wrap outer event in NIP-17; the bridge stores it but does not unwrap. |
| Plaintext (kind 38000, content unencrypted) | Fully handled | Agent-intent events arrive unencrypted. The relay-consumer reads `event.content` directly. |

### Practical consequence

Today, **all messaging through the bridge is effectively plaintext from the bridge's perspective**. A phone sending a kind 1059 sealed DM will have the outer wrapper persisted, but the bridge cannot read the inner message -- it has no decryption logic. An agent that inspects the inbox entry will see the `x:nostrEvent.content` field as opaque base64 ciphertext.

**The recommended path for encrypted phone to agent DMs requires adding a decryption step:** after the relay-consumer writes the kind 1059 to the inbox, a handler needs to decrypt the outer gift-wrap using the agentbox private key, verify the inner seal, and extract the rumour (the actual DM content). This is the ADR-009 deferred follow-up (`ADR-009:262`).

For the initial phone bridge, **using kind 38000 (agent-intent) with plaintext content** avoids the encryption gap. The phone authenticates over NIP-42 (proving identity), and the event body carries the chat payload unencrypted. Security relies on NIP-42 admission control, not message-layer encryption.

---

## 6. did:nostr Identity Tier (Agentbox-Relevant Subset)

`sovereign-bootstrap.py` runs once on first boot and implements the following:

1. Reads `AGENTBOX_PRIVKEY_HEX` (or `AGENTBOX_NSEC`) from environment. If neither is set, generates a fresh secp256k1 keypair. Source: `sovereign-bootstrap.py:129-175`.

2. Applies BIP-340 even-y normalisation (`_x_only_pubkey_with_even_y`): if the derived public key has an odd y coordinate, the private key is negated so the canonical identity satisfies Schnorr's `lift_x` requirement. Source: `sovereign-bootstrap.py:81-105`.

3. Writes the identity JSON to `/var/lib/agentbox/identities/<agent_id>.json` with fields: `private_key_hex`, `public_key_hex`, `x_only_pubkey_hex`, `nsec`, `npub`. Source: `sovereign-bootstrap.py:141`.

4. Creates pod directory tree: `pods/<npub>/{memory/{episodic,semantic},system/{adrs,prds},events/{inbox,outbox}}`. Source: `sovereign-bootstrap.py:186-196`.

5. Writes a WAC ACL doc with subject `did:nostr:<x_only_pubkey_hex>` (64-char hex, not bech32). Source: `sovereign-bootstrap.py:201`.

6. Writes a DID document at `pods/<npub>/did-nostr.json` using `SchnorrSecp256k1VerificationKey2019` type with `publicKeyHex = x_only_pubkey_hex`. This is the Tier 1 + Tier 3 DID doc that solid-pod-rs serves at `GET /did:nostr:<hex>`. Source: `sovereign-bootstrap.py:228-255`.

7. Exports runtime env vars to `/run/agentbox/identity.env`: `AGENTBOX_NPUB`, `AGENTBOX_NSEC`, `AGENTBOX_PUBKEY_HEX`, `AGENTBOX_X_ONLY_PUBKEY_HEX`, `AGENTBOX_DID`. Source: `sovereign-bootstrap.py:258-275`.

**For the phone bridge:** the phone needs the agentbox's `AGENTBOX_X_ONLY_PUBKEY_HEX` to address events (set as the `p` tag). The agentbox DID is `did:nostr:<AGENTBOX_X_ONLY_PUBKEY_HEX>`. The DID document is publicly readable from `GET /.well-known/did.json` (auth-exempt per `server.js:210`) or `solid-pod-rs:8484/did:nostr:<hex>`.

---

## 7. Security Posture Table

Each finding from the prior security audit evaluated against the current code.

### Finding 1: NIP-98 payload-hash verification dropped

**Prior claim:** `verifyNip98(authHeader, method, url)` takes no body param -- body-substitution replay possible.

**Current status: FIXED with residual gap**

`NostrBridge.verifyNip98` at `mcp/servers/nostr-bridge.js:330` now performs full Schnorr signature verification via `nostrTools.verifyEvent(event)` (line 382). The `buildNip98Header` method at `mcp/servers/nostr-bridge.js:415` adds a `['payload', sha256hex(body)]` tag when a body is supplied (lines 428-434).

The **residual gap**: `verifyNip98` (line 330) takes no `body` parameter and does not verify the `payload` tag against the actual request body. The caller at `management-api/middleware/auth.js:43` does not pass the request body. So the server confirms the event's Schnorr signature is valid but does not confirm that the signed `payload` tag matches `sha256(actual body received)`. Body-substitution replay against signed events that omit the `payload` tag (or where the tag is present but unchecked) remains possible.

| Sub-finding | Status | File:line |
|---|---|---|
| Schnorr forgery | FIXED | `mcp/servers/nostr-bridge.js:382` |
| Body-hash check (server side) | PARTIAL -- tag present in builder but not verified in middleware | `management-api/middleware/auth.js:43` |

### Finding 2: Fail-open null identity (all-zero pubkey fallback)

**Prior claim:** `pubkey = req.nip98?.pubkey || process.env.AGENTBOX_PUBKEY || '0'.repeat(64)` in auth middleware.

**Current status: FIXED**

`management-api/middleware/auth.js:34-64` now fails closed: if `verifyNip98Header` returns `null` (failed or absent), and Bearer is also invalid, the request is rejected 401. No fallback pubkey is assigned. The `'0'.repeat(64)` pattern appears only in unrelated non-auth contexts (`broker-bridge.js:54`, `stdio-bridge.js:115`) as a fallback for outbound event signing identity, not for authentication.

| Sub-finding | Status | File:line |
|---|---|---|
| Auth path null pubkey | FIXED | `management-api/middleware/auth.js:34-64` |
| `'0'.repeat(64)` in outbound signing | Present but not an auth bypass | `management-api/routes/broker-bridge.js:54` |

### Finding 3: Governance decisions consumed without signer verification

**Prior claim:** governance decisions consumed without signer verification at `governance-bridge.js:336`.

**Current status: FIXED at the consumption point; admin-pubkey list absent**

Line 336 of `governance-bridge.js` is currently inside `governance_list_decisions` (reading files already written by the relay-consumer). It is NOT a consumption point for inbound decisions.

Inbound governance events (including ActionResponse 31403) are consumed by `relay-consumer.js:_onInbound`. The relay-consumer verifies Schnorr signature at `relay-consumer.js:237-242` before any write or dispatch. The `event.pubkey` (Schnorr-verified) is stored in governance records at `relay-consumer.js:546`.

The remaining gap: neither the relay-consumer nor the governance bridge checks that the 31403 signer is specifically an admin. Any NIP-42-admitted pubkey can submit an ActionResponse. The only gate is the relay's `allowed_pubkeys` list.

| Sub-finding | Status | File:line |
|---|---|---|
| Schnorr verification of governance events | FIXED | `relay-consumer.js:237-242` |
| Admin-list check on ActionResponse signer | ABSENT | N/A -- no such check exists |

---

## 8. Gap Analysis: What Is Missing for Phone -> Agent -> Reply

### G1 — Relay accessibility from phone

**Status: Missing (configuration)**

Default bind is `127.0.0.1:7777`. The phone cannot reach this. `expose = true` must be set in manifest AND `bind = "0.0.0.0"` (or the docker port must be mapped). Source: ADR-009 §Manifest, `nostr-relay.md:129`.

**Required:** Set `bind = "0.0.0.0"`, `expose = true`, add docker port mapping for 7777.

### G2 — Phone pubkey admission (NIP-42 allowlist)

**Status: Missing (configuration)**

`ingress_policy = "allowlist"` requires the phone's pubkey in `allowed_pubkeys`. An empty list means only the operator can post. Source: `relay-consumer.js:419-426`.

**Required:** Add the phone's hex pubkey to `allowed_pubkeys` in `agentbox.toml`.

### G3 — Event kind choice for chat

**Status: Partial**

Kind 1059 (NIP-17) is accepted by the relay and stored in the inbox but the bridge does not decrypt it or dispatch any agent (`relay-consumer.js:152`, no dispatch branch for kind 1059). Kind 38000 (agent-intent) triggers agent dispatch IF `AGENTBOX_INTENT_COMMAND` is set.

**Required:** Either use kind 38000 with plaintext (fastest path), or implement the deferred NIP-17 decryption path (ADR-009:262).

### G4 — intentSpec / agent dispatch wiring

**Status: Missing (configuration)**

`RelayConsumer` opts.intentSpec defaults to `null` unless `AGENTBOX_INTENT_COMMAND` env var is set. Without it, agent-intent events only write a marker file to `intent-queue/` -- no agent is spawned. Source: `relay-consumer.js:125`, `default-intent-spec.js:63`.

**Required:** Set `AGENTBOX_INTENT_COMMAND` to the command that invokes the agent runtime (e.g. `claude`). Optionally set `AGENTBOX_INTENT_ARGS` and `AGENTBOX_INTENT_CWD`.

### G5 — Agent reply path: outbox to Nostr event

**Status: Present when outbox is written correctly**

If the agent writes a reply to `pods/<npub>/events/outbox/<id>.json` with `status = "pending"`, the outbox flusher at `relay-consumer.js:600` will sign and publish it. The outbound event must include a `p` tag targeting the phone's pubkey. The agent receives the phone pubkey from `NOSTR_EVENT_PUBKEY` env var (`relay-consumer.js:352`).

**Required:** Agent must write outbox event with `p = <phone-pubkey>`. Phone must subscribe to the relay for events tagged with its own pubkey.

### G6 — Encryption for DMs

**Status: Missing (code change required)**

The bridge has no NIP-17 / NIP-44 decryption logic. Kind 1059 content lands in the pod inbox as raw ciphertext. Neither `nostr-bridge.js` nor `relay-consumer.js` imports any NIP-17/NIP-44 primitives. ADR-009:262 explicitly defers this.

**Required:** Implement NIP-17 gift-wrap decryption using `nostr-tools` `nip17` / `nip59` primitives. Add decryption step in `relay-consumer.js` before the inbox write when `event.kind === 1059`.

### G7 — No admin-pubkey secondary check on governance decisions

**Status: Not enforced beyond allowlist**

Only the NIP-42 allowlist gates who can post kind 31403. Any allowlisted pubkey can submit an ActionResponse.

**For phone chat:** acceptable. For governance authority, add a check that the 31403 signer is in a dedicated admin list.

### G8 — Phone must know agentbox pubkey

**Status: Discoverable**

The phone must set `p = <AGENTBOX_X_ONLY_PUBKEY_HEX>` on every event it sends. This is available from `GET /.well-known/did.json` (auth-exempt per `server.js:210`) or from `solid-pod-rs:8484/did:nostr:<hex>`.

**Required:** Phone client must fetch the DID document on first connection and extract `verificationMethod[0].publicKeyHex`.

### G9 — NIP-98 body-hash not verified server-side

**Status: Partial (per §7 Finding 1)**

Relevant to any management-api calls from the phone; not relevant to pure relay event submission.

**Required if phone calls management-api:** Pass request body into verify path and check `payload` tag.

### Summary: Minimum viable phone to agent to reply path

To get a working loop today without any code changes (plaintext only):

1. Set `expose = true`, `bind = "0.0.0.0"`, map port 7777 in compose.
2. Add phone's pubkey to `allowed_pubkeys` in `agentbox.toml`.
3. Set `AGENTBOX_INTENT_COMMAND` env var to the agent runner.
4. Phone fetches `/.well-known/did.json`, extracts `AGENTBOX_X_ONLY_PUBKEY_HEX`.
5. Phone completes NIP-42 AUTH with its keypair.
6. Phone sends kind 38000 event, `p = <AGENTBOX_X_ONLY_PUBKEY_HEX>`, plaintext content in `content` field.
7. Agent is spawned, reads `NOSTR_EVENT_JSON`, writes reply to outbox with `p = <phone-pubkey>` (`NOSTR_EVENT_PUBKEY`).
8. Phone subscribes to relay for events tagged to its pubkey (filter `#p: [phone-pubkey]`).
9. Outbox flusher signs and publishes the reply; phone receives it.

Encrypted DMs (kind 1059) require the additional NIP-17 decryption step (G6) and are not available without code changes to `relay-consumer.js`.

---

## Critical Questions — Answered

### Is there a working inbound to agent to reply loop over Nostr today, or only outbound publishing?

**Conditional yes.** The loop exists for agent-intent events (38000-38099) IF `AGENTBOX_INTENT_COMMAND` is set (`default-intent-spec.js:63`) and the orchestrator adapter is wired. Without that env var, the agent-intent path only writes a marker file and no reply is sent. For governance events (kind 31403), the loop reaches `orchestrator.handleGovernanceDecision` (`relay-consumer.js:312`), which connects to VisionClaw's BrokerActor, but that depends on the BrokerActor integration being live.

For kind 1059 (NIP-17 DM), the relay-consumer stores the event in the inbox but dispatches nothing. No reply loop exists for kind 1059 without additional code.

Evidence: `relay-consumer.js:323-367` (agent-intent dispatch path), `relay-consumer.js:311-316` (governance dispatch), `default-intent-spec.js:63` (intent-command gate).

### What is the exact admin-pubkey permission mechanism?

The primary gate is the relay's NIP-42 allowlist (`ingress_policy = "allowlist"`, `allowed_pubkeys` in `agentbox.toml`). Secondary: the bridge's `p`-tag recipient check at `relay-consumer.js:429-438`. There is no secondary check on management-api routes comparing `auth.pubkey` to `AGENTBOX_PUBKEY` or a configured admin list -- auth accepts any valid NIP-98 signer. Evidence: `relay-consumer.js:415-427`, `management-api/middleware/auth.js:113-148`.

### Does the bridge support encrypted DMs, and which NIP?

The bridge subscribes to kind 1059 (NIP-17 outer gift-wrap) but does not decrypt it. NIP-04, NIP-44, and NIP-59 decryption primitives are not imported anywhere in the bridge code. The ADR-009 follow-up note at line 262 explicitly defers "NIP-17 full decryption path" to a future PR. **Effective answer: no encryption support today.** Plaintext agent-intent events (kind 38000) are the viable channel.
