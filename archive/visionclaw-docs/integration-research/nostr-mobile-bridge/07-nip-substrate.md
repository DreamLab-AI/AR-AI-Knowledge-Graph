# 07 — NIP Substrate for Mobile→Relay→AI-Agent Chat Bridge

**Date**: 2026-06-02
**Scope**: Protocol substrate selection for encrypted mobile-to-agent chat, session
summaries, and admin permissioning over Nostr.
**Reads across**: internet NIP specs + internal nostr-rust-forum codebase +
agentbox nostr-bridge.js + PRD-010 IS-Envelope design.

---

## 1. NIP Reference Table

| NIP | Purpose | Event Kinds | Encryption | Our Implementation Status | Relevance |
|-----|---------|-------------|------------|---------------------------|-----------|
| **01** | Core event structure: id/pubkey/created_at/kind/tags/content/sig; REQ/EVENT/CLOSE/OK wire protocol; kind ranges (regular, replaceable, ephemeral 20k-29k, addressable 30k-39k) | All | None (structure only) | Full — foundation of all crates | HIGH (mandatory base) |
| **04** | Legacy encrypted DM (AES-256-CBC, ECDH x-coord unshashed, `base64?iv=base64` wire). **Deprecated** in favour of NIP-17. Leaks: both pubkeys visible in plaintext on relay, sender/recipient correlation trivial | 4 | AES-256-CBC | `nostr-bbs-core/src/nip04.rs` — full implementation (upstream-delegated post-ADR-076); `gift_wrap.rs:363` `process_kind4_event` for backward-compat read path; `dm/mod.rs:412` subscribes kind 4 for legacy read | LOW (backward-compat read only; do not write new kind-4 events) |
| **17** | Private DMs: kind 14 chat message, kind 15 file message. Both are "rumors" (unsigned) wrapped in NIP-59 gift wrap. Supersedes NIP-04. Off-the-shelf clients (0xchat, Amethyst, Nostur) natively understand kind 14 + 1059 | 14 (chat), 15 (file) | NIP-44 v2 via NIP-59 gift wrap | Kind 14 rumor used in `gift_wrap.rs:27` `KIND_RUMOR = 14`; `dm/mod.rs:310-386` sends as gift-wrapped kind-14 rumor; kind 10050 DM relay published at first send (`dm/mod.rs:642`) | **HIGH — recommended primary chat surface** |
| **26** | Delegated event signing: `["delegation", delegator_pk, conditions, sig]` tag. Allows a device key to sign on behalf of an admin key with kind+timestamp constraints | N/A (tag on any event) | None (Schnorr delegation proof) | `nostr-bbs-core/src/nip26.rs` — full implementation; `governance.rs` IS-Envelope `delegation` field; `nip26_tests.rs` | HIGH (admin permissioning: phone key delegates from master admin key) |
| **29** | Relay-managed groups: h-tag routing, membership via kind 9000/9001 admin events, join/leave 9021/9022, metadata 39000-39004. No built-in encryption — relies on relay trust model. Groups are relay-scoped. | 9, 11, 9000-9022, 39000-39004 | None (relay enforces access) | `nip_handlers.rs:38` `is_nip29_admin_kind` gates 9000-9020 and 39000-39002 to admin pubkeys only; partial server-side support | MED (session-as-room model; lacks end-to-end encryption) |
| **40/41/42** | NIP-28 public chat channels: kind 40 create, 41 metadata, 42 message, 43/44 hide/mute. Fully public, no encryption. NIP-28 now deprecated in favour of NIP-29. | 40, 41, 42, 43, 44 | None | `nip_handlers.rs:138-174` TL-gated channel creation/metadata; kind-42 zone enforcement at `:234-253` | LOW (not encrypted; public square only; do not use for agent chat) |
| **42** | Relay AUTH: challenge/response using kind 22242 (relay + challenge tags). Relay sends `["AUTH", <challenge>]`; client responds with signed kind-22242. Enables pubkey-allowlist gating of REQ/EVENT per connection. | 22242 | None (auth only) | `relay_do/nip_handlers.rs:481-530` full AUTH handler; `relay_do/session.rs` tracks auth state; `auth.rs` NIP-98 middleware; relay gates kind-1059 REQ behind AUTH at `nip_handlers.rs:398-430` | **HIGH — mandatory for relay-side access control** |
| **44 v2** | Modern encryption: ECDH x-coord → HKDF-extract (salt=`"nip44-v2"`) → 32-byte conversation key → HKDF-expand (L=76) → ChaCha20 key (32B) + nonce (12B) + HMAC-SHA256 key (32B). Power-of-two padding. Wire: `version(1)\|\|nonce(32)\|\|ciphertext\|\|mac(32)` base64. HMAC uses nonce+ciphertext as AAD. | N/A (crypto layer) | Self (ChaCha20 + HMAC-SHA256) | `nostr-bbs-core/src/nip44.rs` — full implementation delegating to `nostr::nips::nip44` (rust-nostr 0.44.x), validated by upstream reference vectors; `benches/bench_nip44.rs` | **HIGH — mandatory crypto for all DMs** |
| **46** | Remote signing / bunker: client generates keypair, connects to signer via `bunker://` URI, calls `sign_event`/`get_public_key`/`nip44_encrypt`/`nip44_decrypt` over encrypted Nostr events. Eliminates private key storage on mobile. | 24133 (request), 24134 (response) | NIP-44 (signer↔client channel) | Not implemented | MED (valuable for mobile key custody; phone holds bunker, agentbox signs via bunker) |
| **59** | Gift wrap: kind 1059 outer (throwaway ephemeral key, p-tag recipient visible), kind 13 seal (sender's real key, NIP-44 encrypted rumor), kind 14 rumor (unsigned). Randomised ±48h timestamps on seal and wrap. Hides: sender identity, recipient in inner layers, message content. Exposes: throwaway pubkey, final recipient pubkey on outer wrap. | 1059 (wrap), 13 (seal), 14 (rumor) | NIP-44 v2 | `nostr-bbs-core/src/gift_wrap.rs` — complete: `create_rumor`, `seal_rumor`, `wrap_seal`, `gift_wrap`, `unwrap_gift`; 17 unit tests; `dm/mod.rs:310-386` production send path; relay gates kind-1059 REQ behind NIP-42 AUTH | **HIGH — mandatory metadata privacy layer for all DMs** |
| **65** | Relay list metadata: kind 10002 with `r` tags and optional `read`/`write` markers. Clients route outbound DMs to recipient's read relays. | 10002 | None | `dm/mod.rs:642-666` publishes kind-10050 (inbox relay pointer, a variant of relay routing); `02-forum-surfaces.md:556` notes kind-10002/10050 discovery gap for DID docs | HIGH (inbox routing — phone must publish its read relay so agent can route DMs) |
| **89** | Application handler discovery: kind 31990 handler info (k-tags for supported kinds, platform URLs), kind 31989 recommendations. Used to advertise DVMs and custom clients. | 31990, 31989 | None | `nip90.rs:31` `KIND_HANDLER_INFO = 31990` for DvmCapabilityAd; `nip11.rs:28` NIP-90 advertised | MED (agent DVM capability advertisement) |
| **90** | Data Vending Machines: kind 5000-5999 job request (i-tag inputs, output, bid, p-tag targeting), kind 6000-6999 job result (request-tag, amount), kind 7000 feedback (status: queued/processing/payment-required/success/error). Competitive marketplace model. Encryption supported (NIP-04 or NIP-44 on content). | 5000-5999, 6000-6999, 7000 | Optional (NIP-44 or NIP-04 on job content) | `nostr-bbs-core/src/nip90.rs` — complete: `DvmJobRequest`, `DvmJobResult`, `DvmJobFeedback`, `DvmCapabilityAd`, `parse_job_inputs`, `is_job_request`, `is_job_result`; relay accepts DVM kinds modulo TL/whitelist gating; marketplace UI placeholder only | HIGH (structured "do work" requests; layered on top of NIP-17 chat) |
| **98** | HTTP Auth: kind 27235 with `u`-tag (URL without query string) and `method`-tag. Used to authenticate HTTP API calls from Nostr identities. | 27235 | None (Schnorr auth) | `nostr-bridge.js` `verifyNip98`/`buildNip98Header` (full Schnorr verify); `nip98.rs` in nostr-bbs-core; replay protection in relay auth.rs | HIGH (management-api auth, pod-worker NIP-98 headers) |
| **NIP-EE / MLS** | Group E2EE using MLS (Message Layer Security). **Status: unrecommended**, superseded by the Marmot Protocol. No merged NIP as of June 2026. | N/A | MLS | Not implemented | LOW (avoid; protocol not standardised in Nostr ecosystem) |

---

## 2. Core Transport Decision: Substrate Analysis

Four candidates for the primary chat channel. Each is analysed as PRIMARY substrate;
the recommendation is a layered combination.

### 2A. NIP-17 (kind 14, gift-wrapped DM) — agent-as-contact model

Every NIP-17 message is a gift-wrapped (kind 1059) NIP-44-encrypted kind-14 rumor
addressed to the agent's pubkey. From any off-the-shelf client's perspective, the
agent is a Nostr contact and the conversation is indistinguishable from a human DM.

**Strengths**:
- Off-the-shelf Android/iOS clients (0xchat, Amethyst, Nostur, Damus) render kind-14
  conversations natively — zero custom client needed for basic chat.
- Encryption is end-to-end by construction: relay sees only a throwaway pubkey and
  the recipient's pubkey; content and sender identity are in nested NIP-44 ciphertexts.
- Our stack already implements the full pipeline: `gift_wrap.rs` (all three layers),
  `nip44.rs` (upstream-delegated), relay AUTH-gating kind-1059 REQ, and `dm/mod.rs`
  production send+receive path.
- p-tag addressing is sufficient for routing — the bridge subscribes to
  `{ kinds: [1059], "#p": [agent_pubkey] }` after NIP-42 AUTH.
- Reply threading via e-tags on the rumor.

**Weaknesses**:
- Strictly pairwise (one sender, one recipient per gift-wrap). Multi-admin scenarios
  require N separate gift-wrap events (one per recipient).
- No native "job status" semantics — progress updates must be inlined as text DMs.
- Session concept is informal: threads are identified by subject-tag convention
  rather than a protocol primitive.

### 2B. NIP-90 (kind 5000-6999, DVM) — agent-as-job-service model

The human posts a kind-5xxx job request to the relay; the agent responds with a
kind-6xxx result and optionally kind-7000 feedback events during processing.

**Strengths**:
- Native job status lifecycle: `queued → processing → partial → success/error`.
- Bidding, pricing, and payment hooks built in.
- Input types (`event`, `job`, `text`, `url`) map cleanly to "pass context to agent" patterns.
- Our `nip90.rs` implements the full type set (request, result, feedback, capability ad).
- Can be encrypted: job content encrypted with NIP-44, `encrypted` tag marker.

**Weaknesses**:
- NIP-90 is a competitive open marketplace: requests are broadcast to any listening DVM.
  Private agent control requires targeting via `p`-tag but standard clients show the
  job as public unless additionally wrapped.
- Off-the-shelf chat clients do NOT render kind-5xxx/6xxx as conversations. Requires
  a custom client or PWA.
- Poor for conversational back-and-forth: designed for discrete single-turn tasks, not
  multi-turn dialogue.
- No metadata privacy by default — job request pubkey is exposed.

### 2C. NIP-29 (relay-managed groups) — session-as-room model

Each agent chat session maps to a relay group. The h-tag identifies the session; admins
manage membership via kind-9000/9001. Messages are kind 9/11 within the group.

**Strengths**:
- Native group membership primitives: add/remove member events.
- Session = room is an intuitive mapping for multi-participant sessions.
- Relay enforces access — only members can send/receive.

**Weaknesses**:
- No encryption whatsoever. Relay operators see all group content in plaintext.
  For an AI-agent chat bridge carrying sensitive instructions, this is unacceptable.
- Groups are relay-scoped — session does not survive relay migration.
- Off-the-shelf clients supporting NIP-29 groups exist (groups.0xchat.com) but are
  less common than NIP-17 DM support.
- Our relay implements NIP-29 admin kind gating but does not implement the full
  group membership protocol.

### 2D. ACSP Kinds 31400-31405 — agent-as-control-surface model

The Agent Control Surface Protocol uses NIP-33 addressable events: kind 31400
(PanelDefinition), 31401 (PanelState), 31402 (ActionRequest), 31403
(ActionResponse), 31404 (PanelUpdate), 31405 (PanelRetired).

**Strengths**:
- Already implemented in both VisionClaw Rust backend and nostr-bbs-core governance.rs.
- Rich structured control: schema definitions, action buttons, state diffs.
- NIP-33 replaceable semantics (keyed by `d`-tag) — latest state always fetchable.
- The relay gates 31400-31405 to admin pubkeys (kind 31403 responses allowed for any
  whitelisted user) — access control is already live.
- Suitable for "admin drives agent" use cases: action request/response cycles.

**Weaknesses**:
- Not chat. ACSP is a control plane, not a conversational channel. It has no message
  threading, no conversation history, no NIP-44 encryption.
- No off-the-shelf client support — only the custom governance UI renders these kinds.
- Conflating chat and control would lose the natural UX of a conversational interface.

### Recommendation: NIP-17 Primary + NIP-90 Structured Tasks + ACSP Control Plane

```
Chat (conversational) ────────── NIP-17 (kind 14 rumor, gift-wrapped kind 1059)
Structured work requests ──────── NIP-90 (kind 5xxx request / 6xxx result / 7000 feedback)
                                   wrapped in NIP-59 gift wrap for privacy
Agent control / admin ─────────── ACSP kinds 31402/31403 (existing, relay-gated to admin)
Session summaries ─────────────── kind 30840 replaceable (new, section 4)
```

Rationale: NIP-17 is the correct primary because off-the-shelf clients understand it,
our stack fully implements it, and it is end-to-end encrypted by construction. NIP-90
handles discrete "do work" requests layered on top — the human sends a NIP-17 DM
containing a NIP-90 job request event-id as context, or the agent publishes a NIP-90
result addressed via p-tag that the same NIP-17 conversation thread can reference.
ACSP remains the privileged admin control surface already live in production.

---

## 3. Encryption Stack

### Mandatory crypto for all DMs

```
Layer 3 — Gift Wrap (kind 1059)
  Signed by:  ephemeral throwaway key (never reused)
  Encrypts:   Seal JSON using NIP-44 v2 (throwaway_sk, recipient_pk)
  Reveals:    ["p", recipient_pubkey] (required for relay routing), randomised timestamp

Layer 2 — Seal (kind 13)
  Signed by:  sender's real key
  Encrypts:   Rumor JSON using NIP-44 v2 (sender_sk, recipient_pk)
  Reveals:    sender's pubkey (inside encrypted gift wrap — only recipient sees it)
  Tags:       empty (no metadata leak)
  Timestamp:  randomised ±48h

Layer 1 — Rumor (kind 14, unsigned)
  Contains:   plaintext content, p-tag (recipient), optional e-tag (reply chain)
  Real timestamp, real sender pubkey — visible only after both decryptions
```

NIP-44 v2 crypto primitive details (from spec, verified against our `nip44.rs`):
- `conversation_key = HKDF-extract(salt="nip44-v2", ikm=secp256k1_ecdh_x_coord)`
- Message keys via `HKDF-expand(conversation_key, nonce, L=76)`:
  - bytes 0-31: ChaCha20 key
  - bytes 32-43: ChaCha20 nonce
  - bytes 44-75: HMAC-SHA256 key
- Padding: power-of-two chunks, minimum 32 bytes, max 65535 bytes
- MAC over `nonce || ciphertext` (constant-time verify)
- Wire: `version(0x02,1B) || nonce(32B) || ciphertext(var) || mac(32B)` → base64

### What our nostr-core implements vs what must be added

**Implemented (production-ready):**
- `nip44.rs`: full encrypt/decrypt/conversation_key — upstream `nostr::nips::nip44`
  delegation, validated by paulmillr reference vectors
  (`tests/upstream_vectors/all_fixtures.rs`)
- `gift_wrap.rs`: full three-layer NIP-59 pipeline (create_rumor, seal_rumor,
  wrap_seal, gift_wrap, unwrap_gift), timestamp jitter ±48h, throwaway key zeroized
- `dm/mod.rs`: production send (gift_wrap → publish) and receive (subscribe kind 1059,
  unwrap_gift, state update) path including kind-10050 DM relay advertisement
- Relay: kind-1059 REQ requires NIP-42 AUTH (`nip_handlers.rs:398-430`)
- `nip04.rs`: legacy AES-256-CBC read path (correct x-coord ECDH, upstream-delegated)

**Must be built / added:**
- NIP-46 bunker client: phone-side key custody without exposing nsec to the bridge.
  The bridge should accept a `bunker://` URI at setup and use kind-24133/24134 for
  all signing operations. Currently `nostr-bridge.js` uses direct key material from
  AES-256-GCM encrypted `nostr.key.enc`; this is acceptable for server-side agents
  but not for phone-originated events.
- NIP-17 multi-recipient fan-out: for messages addressed to admin + agent simultaneously,
  the sender must produce N gift-wrap events (one per recipient). Currently `dm/mod.rs`
  sends to one recipient. A `gift_wrap_multi(recipients: &[&str])` wrapper is needed.
- Kind-10002/10050 relay discovery: `dm/mod.rs:642` publishes kind-10050 (inbox relay),
  but the bridge does not yet read recipient's kind-10002/10050 to route to their
  preferred inbox relay. This is required for the phone→agent path to succeed when
  the agent's relay differs from the phone's default relay.

---

## 4. Session-Summary Event Design

### 4A. Session summary kind: 30840

```jsonc
{
  "kind": 30840,
  // Addressable (NIP-01 30000-39999 range), replaceable per pubkey+kind+d-tag.
  // Latest summary for a given session_id always wins.
  "pubkey": "<agent_pubkey>",
  "created_at": <unix_ts>,
  "tags": [
    ["d",       "<session_id>"],           // UUID or content-addressed session ID
    ["p",       "<admin_pubkey>"],          // admin who owns this session
    ["p",       "<phone_pubkey>"],          // user pubkey (if different from admin)
    ["agent",   "<agent_pubkey>"],          // agent that ran the session
    ["relay",   "<wss://relay_url>"],       // relay where session events live
    ["start",   "<unix_ts_str>"],           // session start timestamp
    ["end",     "<unix_ts_str>"],           // session end timestamp (if closed)
    ["status",  "active|complete|failed"],
    ["t",       "session-summary"],         // discoverability tag
    ["alt",     "Agent session summary"]    // NIP-31 human-readable fallback
  ],
  "content": "{\"title\": \"<short_title>\", \"summary\": \"<markdown>\", \"tool_calls\": <int>, \"tokens_used\": <int>, \"outcome\": \"<text>\"}"
}
```

- **Range choice**: 30840 is in the addressable 30000-39999 range, not yet allocated by
  any standard NIP. It does not collide with ACSP (31400-31405), NIP-90 (31990),
  job estimates (38200-38201), AGENT_STATE (30078), or BRIEF_REF/BEAD_REF (30000-30001).
- **Replaceability**: same agent + same d-tag (session_id) → relay keeps only the latest.
  The agent updates this event as the session progresses (status: active → complete).
- **Privacy**: the summary content is NOT encrypted in this design. It is a structured
  metadata event, not a DM. Sensitive content (tool call arguments, partial results)
  should remain in the NIP-17 DM thread. If summary confidentiality is required, wrap
  the kind-30840 in a NIP-59 gift wrap addressed to the admin pubkey — but this breaks
  the replaceable semantics (each wrap is a new kind-1059, not an addressable event).
  Recommendation: keep summaries non-sensitive by design; log raw tool calls in an
  encrypted NIP-17 DM thread.

### 4B. Session lifecycle events

```
Session start:   Agent publishes kind-30840 with status=active, start=<ts>, end absent.
                 Simultaneously sends NIP-17 DM to admin: "Session <id> started".

Session update:  Agent publishes updated kind-30840 (same d-tag) with incremented
                 tool_calls/tokens_used. Relay retains only the latest (addressable).

Session end:     Agent publishes kind-30840 with status=complete|failed, end=<ts>,
                 final outcome summary. Sends NIP-17 DM to admin with summary text.

Admin queries:   REQ { kinds: [30840], authors: [agent_pubkey], "#p": [admin_pubkey] }
                 Returns all sessions belonging to admin's agents.
                 Single-session fetch: REQ { kinds: [30840], "#d": [session_id] }
```

Alternative considered: writing summaries to a Solid pod with a Nostr pointer event.
This would use a kind-30100 (or similar) addressable event containing the pod URL and
path to the RDF summary document, with the actual content on the Solid pod. This is
appropriate if the summary is large or must be part of the knowledge graph (PRD-013
ingest flow). For the mobile bridge use case, the Solid path adds latency and a Solid
pod dependency; the in-Nostr kind-30840 design is preferred. The two are not mutually
exclusive: the kind-30840 content field can carry a `"pod_url"` key pointing to the
Solid document if one was written.

---

## 5. Admin Permissioning on the Wire

### 5A. Relay-level access control (NIP-42 AUTH)

The relay is the first enforcement layer. Our relay already implements:

```
1. D1 whitelist check: event.pubkey must be in the whitelist table
   (nip_handlers.rs:82). Any unlisted pubkey is rejected with
   "blocked: pubkey not whitelisted".

2. NIP-42 AUTH gate on kind-1059 REQ:
   (nip_handlers.rs:398-430)
   A client requesting kind-1059 events MUST have completed AUTH (kind-22242
   challenge/response). If not authenticated, relay returns:
   "auth-required: must authenticate to receive kind-1059 DMs".
   Post-AUTH, the filter is rewritten to require "#p" = [authenticated_pubkey]
   so a client can only read its own DMs.

3. ACSP governance kind gating:
   (nip_handlers.rs:218-233)
   Kinds 31400-31405 are admin-only EXCEPT kind 31403 (ActionResponse) which
   any whitelisted user may publish. The admin pubkey must be in the D1 whitelist
   with is_admin=true.
```

### 5B. p-tag addressing and admin identity

The recommended identity model:

```
Admin master key:   The keypair stored in /workspace/profiles/<stack>/nostr.key.enc
                    (AES-256-GCM, loaded by nostr-bridge.js loadSigner()).
                    This key is the relay whitelist entry and the ACSP admin key.
                    The agent always signs with this key (or via NIP-46 bunker).

Phone device key:   A per-device keypair generated on the phone. NOT the admin key.
                    Registered in the relay whitelist by the admin out-of-band.
                    The phone user signs NIP-17 DMs and kind-22242 AUTH events
                    with this key.

NIP-26 delegation:  To avoid the phone holding admin signing authority, use NIP-26:
                    admin_key delegates to phone_key for
                    kind=14&created_at>T (session start) conditions.
                    The delegation tag is included in the rumor inside the gift wrap.
                    The bridge validates the delegation before acting on commands.
                    PRD-010 IS-Envelope carries the delegation block at the
                    "delegation" field (envelope spec: §5.3).
```

### 5C. Bridge enforcement flow

```
Phone → Relay:
  1. Phone completes NIP-42 AUTH (kind 22242, relay+challenge tags, phone_key signs).
  2. Phone publishes gift-wrapped kind 1059 addressed to agent_pubkey.
  3. Relay checks: phone_pubkey in whitelist → accept; else → reject.

Relay → Agent Bridge:
  4. nostr-bridge.js subscribes { kinds: [1059], "#p": [agent_pubkey] }
     (AUTH already established at connect time).
  5. Bridge calls unwrap_gift(event, agent_sk) → recovers kind-14 rumor.
  6. Bridge checks:
     a. seal.pubkey (sender) is either admin_pubkey or a NIP-26 delegatee
        of admin_pubkey with valid kind=14 conditions.
     b. If NIP-26 delegation present: verify delegation sig and condition constraints.
     c. Reject if sender not authorised.

Agent → Phone:
  7. Agent calls gift_wrap(agent_sk, agent_pubkey, phone_pubkey, content).
  8. Publishes kind-1059 to relay.
  9. Relay routes to phone_pubkey via existing p-tag subscription.
```

### 5D. Registered-pubkey allowlist

The allowlist approach for the agentbox relay (`nostr-rs-relay` with `allowed_pubkeys`
env config, as per PRD-010 §5.2) provides the first defence: only registered keys can
publish events. The relay config should include:
- `agent_pubkey` (the bridge identity)
- `admin_pubkey` (same as agent_pubkey in single-admin deployments)
- `phone_pubkey` (registered at device setup time via the management API)
- Any NIP-26 delegatee pubkeys (if using per-session ephemeral phone keys)

For multi-admin setups, each admin phone generates its own keypair; the admin registers
it via `POST /api/nostr/register-pubkey` (management-api endpoint backed by NIP-98 auth
with the master admin key).

---

## 6. Concrete Kind Allocation Proposal

| Kind | Name | Direction | Encryption | Replaceability | Purpose |
|------|------|-----------|------------|----------------|---------|
| **4** | Legacy DM | read-only (backward compat) | AES-256-CBC (NIP-04) | Regular | Read legacy encrypted DMs; never write |
| **13** | Seal | internal layer | NIP-44 v2 | Regular | NIP-59 inner layer; never subscribed directly |
| **14** | Chat rumor | phone→agent, agent→phone (inside 1059) | NIP-44 v2 (via wrap) | Regular (never published bare) | Conversational chat message; the actual text lives here |
| **15** | File rumor | phone→agent (inside 1059) | NIP-44 v2 (via wrap) | Regular (never published bare) | File/image transfer to agent (NIP-17 file message) |
| **22242** | Relay AUTH | phone→relay, agent→relay | None (Schnorr) | Ephemeral | NIP-42 challenge response; connection auth |
| **1059** | Gift Wrap | phone→agent, agent→phone | NIP-44 v2 (double layer) | Regular | Transport envelope for all DMs; relay routes by p-tag |
| **5000-5999** | DVM Job Request | phone→agent | Optional NIP-44 | Regular | Structured work request: "summarise this", "analyse that" |
| **6000-6999** | DVM Job Result | agent→phone | Optional NIP-44 | Regular | Result of a NIP-90 job; agent publishes with p-tag to requester |
| **7000** | DVM Feedback | agent→phone | Optional NIP-44 | Regular | Job progress: queued/processing/partial/error |
| **10002** | Relay list | phone→relay, agent→relay | None | Replaceable (per pubkey+kind) | Publish preferred read/write relays for DM routing |
| **10050** | DM relay hint | phone→relay, agent→relay | None | Replaceable (per pubkey+kind) | Inbox relay pointer; already published by `dm/mod.rs` |
| **22242** | NIP-42 AUTH | bidirectional | None | Ephemeral | Relay connection authentication |
| **27235** | NIP-98 HTTP Auth | phone→management-api | None (Schnorr) | Ephemeral | Authenticated HTTP calls to management API |
| **30078** | Agent State | agent→relay | None | Addressable (d=state-id) | Agent internal state events (existing AGENT_STATE kind) |
| **30840** | Session Summary | agent→relay | None (or gift-wrapped to admin if confidential) | Addressable (d=session_id) | NEW: per-session summary; replaces on update; queryable by admin |
| **31400** | ACSP PanelDefinition | agent→relay | None | Addressable (d=panel-id) | Agent publishes governance panel schema (existing, production) |
| **31401** | ACSP PanelState | agent→relay | None | Addressable (d=panel-id) | Agent publishes current panel data snapshot |
| **31402** | ACSP ActionRequest | agent→relay | None | Addressable (d=action-id) | Agent requests human decision (existing, production) |
| **31403** | ACSP ActionResponse | phone/admin→relay | None | Addressable (d=action-id) | Human responds to action request (existing, production) |
| **31404** | ACSP PanelUpdate | agent→relay | None | Addressable (d=panel-id) | Agent publishes incremental state diff |
| **31405** | ACSP PanelRetired | agent→relay | None | Addressable (d=panel-id) | Agent retires a control panel |
| **31990** | DVM Handler Info | agent→relay | None | Addressable (d=handler-id) | Agent's capability advertisement (NIP-89/90) |
| **38200** | Job Estimate | agent→relay | None | Addressable (d=job-id) | Agent job cost estimate (existing payment system) |
| **38201** | Job Settlement | agent→relay | None | Addressable (d=job-id) | Agent job receipt/settlement (existing payment system) |

No new ephemeral kinds are allocated. The 30840 range is confirmed clear of all
existing allocations in this codebase. All standard NIP kinds are reused as-is.

---

## 7. Off-the-Shelf Client Compatibility

### The core tradeoff

| Substrate choice | Off-the-shelf client works? | Custom client needed? | Encrypted? |
|-----------------|-----------------------------|-----------------------|------------|
| **NIP-17 (kind 14 + 1059)** | YES — Amethyst, 0xchat, Nostur, Damus all render gift-wrapped DMs as normal conversations | No (for basic chat) | Yes (mandatory NIP-44+NIP-59) |
| NIP-90 (kind 5xxx/6xxx) | NO — DVM kinds shown as raw events or ignored | Yes — custom PWA required | Optional only |
| NIP-29 (groups) | PARTIAL — some clients (groups.0xchat.com) support it | For full feature set | No (no encryption) |
| ACSP (31400-31405) | NO — completely custom; only the governance UI renders these | Yes | No |

### NIP-17 compatibility implications

If the recommended NIP-17 primary is chosen:

1. **A user can install Amethyst (Android) or Nostur (iOS), add the agent's npub as a
   contact, and send/receive encrypted messages immediately.** No custom PWA or bridge
   app is required for basic chat. The agent appears as a Nostr contact.

2. **Session summaries (kind 30840) are invisible to off-the-shelf clients.** This is
   acceptable — summaries are admin tooling, not end-user chat. The admin accesses them
   via the governance UI or a simple Nostr filter query.

3. **NIP-90 DVM job requests embedded in DM conversations** require either:
   - The off-the-shelf client to send a text DM containing structured parameters
     (the agent parses intent from natural language), OR
   - A thin PWA that wraps the DM UI and adds a "send task" button that publishes a
     kind-5xxx event alongside the NIP-17 DM thread.
   The basic text-DM path works with zero custom client. The structured task path
   requires a PWA — this is the correct place to draw the custom-client boundary.

4. **ACSP kinds (31400-31405) require the governance UI.** These are not part of the
   chat surface; they are the admin control plane. The off-the-shelf client and the
   ACSP plane coexist on the same relay because they use different kinds.

5. **NIP-42 AUTH is transparent to users.** Amethyst and 0xchat both implement
   NIP-42 AUTH automatically. The user sees "sign in" once, then the client handles
   AUTH challenges silently.

### Coupling to client-choice decision

This NIP-17 primary recommendation directly implies:

- **Client priority 1**: Any NIP-17-capable off-the-shelf client (Amethyst, 0xchat)
  works for basic agent chat with zero custom development. This is the minimum viable
  mobile client path.
- **Client priority 2**: A thin PWA (web app) adds structured task submission (NIP-90),
  session summary display (kind 30840 query), and the ACSP action response UI
  (kind 31403 ActionResponse). The PWA extends, not replaces, the off-the-shelf client.
- **Client priority 3**: Native mobile app (React Native / Flutter with nostr-tools /
  rust-nostr FFI) if richer UX is required — but this is not a NIP-substrate question.

The client-choice agent should be aware that the NIP-17 choice deliberately keeps the
minimum client requirement at "any Nostr app that supports encrypted DMs", and the
custom development budget is spent on structured task UI only.

---

## Sources

### NIP Specifications (github.com/nostr-protocol/nips)

- NIP-01: https://github.com/nostr-protocol/nips/blob/master/01.md
- NIP-04: https://github.com/nostr-protocol/nips/blob/master/04.md
- NIP-17: https://github.com/nostr-protocol/nips/blob/master/17.md
- NIP-26: https://github.com/nostr-protocol/nips/blob/master/26.md
- NIP-28: https://github.com/nostr-protocol/nips/blob/master/28.md
- NIP-29: https://github.com/nostr-protocol/nips/blob/master/29.md
- NIP-42: https://github.com/nostr-protocol/nips/blob/master/42.md
- NIP-44: https://github.com/nostr-protocol/nips/blob/master/44.md
- NIP-46: https://github.com/nostr-protocol/nips/blob/master/46.md
- NIP-59: https://github.com/nostr-protocol/nips/blob/master/59.md
- NIP-65: https://github.com/nostr-protocol/nips/blob/master/65.md
- NIP-89: https://github.com/nostr-protocol/nips/blob/master/89.md
- NIP-90: https://github.com/nostr-protocol/nips/blob/master/90.md
- NIP README (full list + NIP-EE status): https://github.com/nostr-protocol/nips/blob/master/README.md

### Internal File References

- `nostr-bbs-core/src/nip04.rs` — NIP-04 AES-256-CBC, upstream-delegated, wire format
- `nostr-bbs-core/src/nip44.rs` — NIP-44 v2 ChaCha20+HMAC-SHA256, upstream-delegated
- `nostr-bbs-core/src/gift_wrap.rs` — NIP-59 full three-layer pipeline (kinds 14/13/1059)
- `nostr-bbs-core/src/nip90.rs` — DVM types (DvmJobRequest, DvmJobResult, DvmJobFeedback, DvmCapabilityAd)
- `nostr-bbs-core/src/nip26.rs` — NIP-26 delegation (Conditions, create_delegation, verify_delegation)
- `nostr-bbs-relay-worker/src/relay_do/nip_handlers.rs` — NIP-01/09/42/45 handlers, ACSP gating, kind-1059 AUTH gate, NIP-29 admin gating
- `nostr-bbs-forum-client/src/dm/mod.rs` — NIP-17 production send/receive, kind-10050 DM relay publication
- `project/agentbox/mcp/servers/nostr-bridge.js` — ACSP kinds 31400-31405, NIP-98 auth, job payment kinds 38200-38201
- `project/docs/PRD-010-did-nostr-mesh-federation.md` §5.3 — IS-Envelope v1 (kind 14 rumor content shape, NIP-26 delegation field, gift wrap layers)
- `project/docs/integration-research/02-forum-surfaces.md` §7 — NIP-90 DVM marketplace status (protocol complete, UI placeholder)
- `project/docs/ddd-mesh-federation-context.md` — ACSP kind table (31400-31405) and governance implementation status
