# CF Private Relay: Architecture, Access Control, and Mobile Connectivity

Research date: 2026-06-02
Sources: `/home/devuser/workspace/dreamlab-ai-website/forum-config/` and
         `/home/devuser/workspace/nostr-rust-forum/crates/nostr-bbs-relay-worker/`

---

## 1. Relay Identity

### Deployed URL

```
wss://dreamlab-nostr-relay.solitary-paper-764d.workers.dev
```

Source: `dreamlab.toml:relay.url` and
`forum-config/deploy/relay-worker.wrangler.toml:name = "dreamlab-nostr-relay"`.
The same URL is re-exported as a constant in
`forum-config/src/workers.rs:163` (`deployed_urls::RELAY_WORKER`).

The relay also doubles as an HTTP endpoint for NIP-11 relay info (at `/` with
`Accept: application/nostr+json`) and a REST management API under `/api/`.

### Cloudflare Infrastructure

The relay is a Cloudflare Worker (`nostr-bbs-relay-worker`, Rust compiled to
`wasm32-unknown-unknown` via `worker-build`). Real-time WebSocket sessions are
handled by a **Durable Object** class `NostrRelayDO`, instantiated as a single
named instance `"main"` (`relay_do/mod.rs:167`). The HTTP fan-out worker
upgrades all WebSocket connections and forwards them to this single DO.

All WebSocket connections therefore share one in-memory session map. The DO
survives Cloudflare hibernation by serialising session auth state and
subscriptions to DO transactional storage (`session.rs:119-133`).

### D1 Schema (Cloudflare D1 / SQLite)

Two D1 databases are bound:

**`DB` — `dreamlab-relay` (database_id: `97c77d23-0e24-4325-ada7-1747eab4095b`)**
(`relay-worker.wrangler.toml:24-27`)

Core tables created inline in `lib.rs:ensure_schema()`:

| Table | Purpose |
|-------|---------|
| `events` | All stored Nostr events (`id`, `pubkey`, `kind`, `created_at`, `content`, `tags`, `sig`, `d_tag`, `received_at`) |
| `whitelist` | Per-pubkey admission table with cohorts, trust, suspension, admin flag |
| `channel_zones` | Maps NIP-29 channel IDs to zone names |
| `admin_log` | Append-only audit trail of admin actions |
| `settings` | Key-value store for configurable thresholds |
| `reports` | NIP-56 report events projected into structured rows |
| `hidden_events` | Events soft-deleted by moderation |
| `moderation_actions` | Mirror of ban/mute events (kinds 30910/30911) for ingress gating |
| `profiles` | Projection of latest kind-0 per pubkey (Sprint v10) |
| `agent_registry` | Pubkeys authorised to publish governance events (31400-31405) |
| `broker_cases` | HITL governance case aggregate |
| `broker_decisions` | Per-case decisions (append-only) |
| `broker_roles` | Human broker role assignments |

Base schema for `events` and `whitelist` from `SETUP.md:65-84`:

```sql
CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    pubkey TEXT NOT NULL, kind INTEGER NOT NULL,
    created_at INTEGER NOT NULL, content TEXT NOT NULL,
    tags TEXT NOT NULL, sig TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS whitelist (
    pubkey TEXT PRIMARY KEY,
    cohorts TEXT NOT NULL DEFAULT '["members"]',
    added_at INTEGER NOT NULL,
    added_by TEXT NOT NULL DEFAULT 'auto-registration',
    is_admin INTEGER NOT NULL DEFAULT 0
);
```

**`REPLAY_DB` — `dreamlab-auth` (database_id: `e3981999-e8f0-4c07-9e4b-2e50859b8524`)**
(`relay-worker.wrangler.toml:43-46`)

Shared with the auth-worker; holds the `nip98_replay` table for NIP-98
cross-worker replay protection (`auth.rs:31`, `REPLAY_DB` constant).

---

## 2. Supported NIPs

From the NIP-11 document (`nip11.rs:36`):

```
supported_nips: [1, 9, 11, 16, 17, 29, 33, 40, 42, 45, 50, 56, 59, 65, 90, 98]
```

The health endpoint (`lib.rs:234`) also includes NIP-45 (COUNT) in its `nips`
array. NIP-17 (DM protocol) is listed but **not yet enforced at the kind
level** — see Section 4 below.

### NIP-16 Event Treatment (`broadcast.rs:24-34`)

```
kind 0, 3, 10000-19999  -> Replaceable   (de-dup per pubkey+kind)
kind 20000-29999         -> Ephemeral    (broadcast only, not stored)
kind 30000-39999         -> ParameterizedReplaceable (d-tag de-dup)
all others               -> Regular      (stored as-is)
```

This means:

- **kind 0** (profile metadata): replaceable, stored.
- **kind 3** (contact list): replaceable, stored.
- **kind 1** (text note): regular, stored. Retention: 7,776,000 seconds (~90 days) per NIP-11 `retention` array (`nip11.rs:54`).
- **kind 4** (NIP-04 legacy encrypted DM): regular, stored. **No special handling, no per-recipient access gate.** The relay stores and serves kind-4 events identically to kind-1.
- **kind 7** (reaction): regular, stored. Retention: 2,592,000 seconds (~30 days).
- **kind 40/41/42** (NIP-29 channel create/meta/message): regular, stored.
- **kind 1059** (NIP-59 gift wrap / sealed DM): regular, stored. **AUTH gated for both publish and subscribe** (see Section 3).
- **kind 14/15** (NIP-17 DM/seal): NOT explicitly listed; these are regular events by default treatment (14 and 15 fall below 10000, treated as regular). No explicit per-recipient read gate exists for kind 14 — it would be served to any subscriber with a matching filter. This means NIP-17 DMs (kinds 14/15) receive **weaker isolation than kind-1059**.
- **kind 1984** (NIP-56 report): regular, processed into `reports` table.
- **kind 5** (deletion): regular, own events + TL3+ for others' events.
- **kind 10002** (NIP-65 relay list): replaceable.
- **kinds 30910/30911** (ban/mute): parameterized replaceable; mirrored into `moderation_actions`.
- **kinds 31400-31405** (agent governance): parameterized replaceable; agent-registry gated.

### Kind Allowlist

There is **no global kind allowlist**. The relay accepts any Nostr kind from
whitelisted pubkeys, subject to the per-kind trust gates in Section 3.
The `MESH_FEDERATED_KINDS` env var (`relay-worker.wrangler.toml:20`) applies
only to events from recognised mesh peers when `MESH_MODE != "standalone"`.
In the current DreamLab deployment `MESH_MODE = "standalone"` (`wrangler.toml:18`),
so this allowlist is never evaluated (`nip_handlers.rs:828`).

---

## 3. Access Control

### NIP-42 AUTH Flow

On every new WebSocket connection, the relay immediately sends an `AUTH`
challenge (`mod.rs:141-147`):

```rust
Self::send_auth(&session.ws, &challenge);  // mod.rs:145
```

The challenge is a 32-byte CSPRNG value XOR-mixed with the session ID
(`session.rs:292-297`, using `getrandom` → `crypto.getRandomValues`).

The client responds with a kind-22242 signed event. The relay validates:

1. `event.kind == 22242` (`nip_handlers.rs:484`)
2. Schnorr signature via `nostr_bbs_core::verify_event_strict` (`nip_handlers.rs:490-497`)
3. `challenge` tag matches the session challenge (`nip_handlers.rs:501-513`)
4. Timestamp within 10 minutes (`nip_handlers.rs:516-519`)

On success, `session.authed_pubkey` is set and persisted to DO storage
(`nip_handlers.rs:523-531`). Auth state survives DO hibernation (`session.rs:189-192`).

### Is AUTH Required to Connect?

**No.** The NIP-11 document explicitly declares `"auth_required": false`
(`nip11.rs:47`). A client may connect and send `EVENT` or `REQ` messages
without authenticating. The relay always sends the `AUTH` challenge upon
connect, but it is the client's choice whether to respond.

### Is AUTH Required to Read (REQ)?

**Conditionally.** For **kind-1059** (NIP-59 gift wrap) subscriptions, AUTH
is mandatory (`nip_handlers.rs:407-415`):

```rust
let needs_kind_1059 = filters.iter()
    .any(|f| f.kinds.as_ref().is_some_and(|k| k.contains(&1059)));
if needs_kind_1059 {
    match &session_pubkey {
        None => {
            Self::send_notice(&ws, "auth-required: must authenticate to receive kind-1059 DMs");
            return;
        }
        ...
    }
}
```

Furthermore, the relay enforces `#p` tag isolation: even when authenticated,
the relay rewrites the filter to only return kind-1059 events where the
authenticated pubkey appears as the `p` tag recipient (`nip_handlers.rs:417-430`).
This prevents a client from fetching another user's gift-wrapped DMs.

For all other kinds, REQ requires no AUTH. Any connected client can query any
stored kind without authenticating.

### Is AUTH Required to Write (EVENT)?

**No, but whitelist membership is required.** The write gate is a D1 whitelist
lookup, not AUTH:

```rust
if !self.is_whitelisted(&event.pubkey).await {
    Self::send_ok(ws, &event.id, false, "blocked: pubkey not whitelisted");
    return;
}
```
(`nip_handlers.rs:82-85`)

`is_whitelisted` queries `SELECT 1 FROM whitelist WHERE pubkey = ?1 AND
(expires_at IS NULL OR expires_at > ?2)` (`storage.rs:318`). The check is on
the `pubkey` field of the incoming event, not on the authenticated session
pubkey. This has a subtle implication: **a client can attempt to write events
without completing the NIP-42 handshake**, as long as the event's pubkey is in
the whitelist. The whitelist check operates on the event signature, not the
session identity.

### Ingress Policy Configuration

The DreamLab operator config sets:

```toml
# dreamlab.toml:ingress_policy
ingress_policy = "allowlist"
```

The `nostr-bbs-config` crate validates this field (`validate.rs:43-47`): only
`"allowlist"` or `"open"` are accepted. At `"allowlist"`, **no pubkey can write
to the relay without being in the D1 `whitelist` table**.

---

## 4. Per-Kind Write Rules

| Kind | Who Can Write | Notes |
|------|--------------|-------|
| 0 (profile) | Whitelisted pubkeys | TL0+; auto-profiles projection |
| 1 (text note) | Whitelisted, TL0+, not banned/muted | Moderation cache checked |
| 4 (NIP-04 DM) | Whitelisted pubkeys | No explicit gate beyond whitelist; stored and served to any subscriber |
| 5 (delete) | Own events: TL0+; others': TL3+ | `nip_handlers.rs:188-200` |
| 7 (reaction) | Whitelisted TL0+ | |
| 9024 (registration) | Whitelisted TL0+ | Smaller content cap (8KB vs 64KB) |
| 40 (channel create) | Whitelisted TL2+ | `nip_handlers.rs:137-145` |
| 41 (channel meta) | TL2+ own channel, TL3+ others' | `nip_handlers.rs:148-176` |
| 42 (channel msg) | Whitelisted TL0+, zone access required | Zone enforcement per `channel_zones` table |
| 1059 (gift wrap) | Whitelisted pubkeys | AUTH not required to write; required to read |
| 1984 (NIP-56 report) | Whitelisted TL1+ | `nip_handlers.rs:177-182` |
| 9000-9020, 39000-39002 (NIP-29 admin) | Admin only | `nip_handlers.rs:203-217` |
| 30910/30911 (ban/mute) | Admin only (implicit) | Non-admin sends succeed but mirror is ignored; effectively admin-only impact |
| 31400-31402, 31404-31405 (governance) | Registered agents only | `nip_handlers.rs:221-232`; `agent_registry` table |
| 31403 (action response) | Any whitelisted user | Human governance decision |

### Can a Non-Allowlisted Pubkey Publish a DM to the Admin Pubkey?

**No.** Any EVENT with `pubkey` not in the `whitelist` table is rejected
with `"blocked: pubkey not whitelisted"` before any kind-specific logic runs
(`nip_handlers.rs:82-85`). There is no "open DM to admin" pathway.

### Can a Non-Allowlisted Client Read Replies?

**No writes, therefore no replies to read.** However, for REQ (read), there is
no whitelist check on the subscribing client's pubkey — any connected client
can subscribe to any filter for kinds other than 1059. In practice this means
an unauthenticated, non-whitelisted client can read public messages on the
relay (kind 1, kind 42, etc.), though it cannot publish.

---

## 5. Rate Limits, Message Size, Retention

### Rate Limits

- **Events per second per IP**: 10 (`broadcast.rs:75`, `MAX_EVENTS_PER_SECOND`)
  - Enforced by a sliding 1-second window in-memory in the DO.
- **WebSocket connections per IP**: 20 (`mod.rs:47`, `MAX_CONNECTIONS_PER_IP`)
- **HTTP REST routes** (per `dreamlab.toml:[ratelimit]` and `workers.rs`):
  - `/api/profiles/batch`: 60 req/min
  - `/.well-known/nostr.json`: 60 req/min
  - `/api/exports/*`: 6 req/min
  - default: 120 req/min

### Message / Event Size Caps (`nip_handlers.rs:30-34`)

| Limit | Value |
|-------|-------|
| `MAX_CONTENT_SIZE` | 65,536 bytes (64 KB) |
| `MAX_REGISTRATION_CONTENT_SIZE` (kind 0, 9024) | 8,192 bytes (8 KB) |
| `MAX_TAG_COUNT` | 2,000 tags |
| `MAX_TAG_VALUE_SIZE` | 1,024 bytes per tag value |
| `MAX_TIMESTAMP_DRIFT` | 604,800 seconds (7 days) |
| `max_message_length` (NIP-11) | 65,536 |
| `max_subscriptions` (NIP-11) | 20 |
| `max_filters` (NIP-11) | 10 |
| `max_limit` (NIP-11) | 1,000 |
| `max_subid_length` (NIP-11) | 64 |

### Retention (NIP-11 `retention` field, `nip11.rs:51-59`)

| Kind | Retention |
|------|-----------|
| 0 (profile) | Indefinite |
| 3 (contacts) | Indefinite |
| 1 (text note) | 90 days (7,776,000 s) |
| 7 (reaction) | 30 days (2,592,000 s) |
| 9024 (registration) | 24 hours (86,400 s) |
| 10000-19999 (replaceable) | Indefinite |
| 30000-39999 (param. replaceable) | Indefinite |

NIP-40 event-level expiration tags are enforced both on ingest (rejected before
storage) and on query (filtered out at query time) (`nip_handlers.rs:61-68`,
`storage.rs:288-295`).

---

## 6. The "Private" Boundary

### What Makes This Relay Private

The relay is "private" in the sense that **writing requires explicit whitelist
membership** set by an admin. It is NOT private in the sense of a closed-read
system: unauthenticated clients can connect and subscribe to most event kinds
without restriction. The only strictly enforced privacy boundary is on event
ingress (writes).

The mechanisms are:

1. **D1 `whitelist` table gating on EVENT** (`storage.rs:310-326`). Every
   EVENT message is blocked unless `pubkey` is in the whitelist with a
   non-expired entry. This is enforced before any kind-specific logic.

2. **NIP-59 per-recipient isolation** (`nip_handlers.rs:398-430`,
   `broadcast.rs:43-67`). Kind-1059 gift-wrap events are delivered only to
   authenticated sessions whose `authed_pubkey` matches the `p` tag recipient.
   This is the relay's strongest per-user privacy guarantee.

3. **Zone cohort enforcement on kind-42 channel messages** (`nip_handlers.rs:234-247`).
   Channel messages are only served to authenticated sessions whose pubkey has
   the matching zone cohort in the `whitelist.cohorts` column.

4. **CORS headers** (`lib.rs:77-93`). The relay sets `Access-Control-Allow-Origin`
   to the allowlist from `ALLOWED_ORIGINS`. However, CORS is enforced by
   browsers, not by the relay itself. A WebSocket upgrade from a native client
   or a non-browser tool that does not send an `Origin` header bypasses CORS
   entirely. The relay does **not reject WebSocket upgrades based on the Origin
   header** (`lib.rs:166-169`): the upgrade path simply forwards to the DO
   without any origin check. CORS therefore provides no security boundary
   against native clients or relay testing tools.

### Can a Standard Android Nostr Client Connect with a Plain nsec/npub?

**YES — conditionally.**

A standard Android Nostr client (Amethyst, 0xchat, Nostros, etc.) using a
plain keypair (nsec/npub) can:

- Establish a WebSocket connection to
  `wss://dreamlab-nostr-relay.solitary-paper-764d.workers.dev`.
- Receive the NIP-42 AUTH challenge.
- Sign and return a kind-22242 AUTH event with the nsec (standard NIP-42;
  all major Android clients support this).
- Subscribe to and receive events for most kinds.

However, to **write any events**, the pubkey (derived from the nsec) must first
be in the relay's D1 `whitelist` table. Without whitelist membership, every
EVENT is rejected with `"blocked: pubkey not whitelisted"`.

The relay does **not use WebAuthn/passkeys for the WebSocket/Nostr protocol
layer**. WebAuthn is used exclusively by the web forum-client (`forum-client`
WASM SPA) for its own session management and key derivation. The relay's
WebSocket protocol knows nothing about WebAuthn — it uses standard NIP-42
Schnorr-signed AUTH events only.

### How a Phone Client Gets Authorised

Currently the only documented provisioning paths are:

1. **Admin adds the pubkey via `POST /api/whitelist/add`** with a NIP-98 admin
   token (`whitelist.rs:277-342`). This is the standard path for onboarding
   known pubkeys.

2. **First-user-is-admin**: if the `whitelist` table is empty, the first kind-0
   event accepted triggers `auto_whitelist()` promoting that pubkey to admin
   (`storage.rs:336-392`). Note: `auto_whitelist` is defined but is **never
   called from `handle_event`** — it has no call sites in the relay's
   production code path. It is a dead-code utility that would need to be wired
   into the kind-0 ingest hook to activate.

3. **Invite redemption** in the auth-worker (`invites.rs:509-515`) inserts the
   pubkey into the `members` table (auth D1), not directly into the relay's
   `whitelist` table. There is no code in the auth-worker that calls
   `/api/whitelist/add` on the relay after invite redemption. The `whitelist`
   table is in the relay D1 (`RELAY_DB`); the auth-worker has a separate bound
   `RELAY_DB` binding for admin queries only. The two tables are separate; new
   invite-registered users are **not automatically added to the relay whitelist**.

In practice, onboarding a mobile client's pubkey requires an admin to call the
relay management API with a NIP-98 signed token.

---

## 7. Bridge-Onward Readiness

### Existing Federation Mechanism

The relay has a complete **mesh federation framework** (`nip_handlers.rs:813-865`,
`relay-worker.wrangler.toml:17-20`). When `MESH_MODE != "standalone"`, the
relay checks whether an event's pubkey is in `MESH_ALLOWED_REMOTE_DIDS`
and filters inbound events against the `MESH_FEDERATED_KINDS` allowlist. The
`dreamlab.toml:[mesh]` section (`dreamlab.toml:mesh`) defines peer relay URLs
and federated kinds:

```toml
[mesh]
mode                    = "standalone"
peer_relays             = []
federated_kinds         = [1, 1059, 30001, 30050, 30910, 31400, 31401, 31402, 31403, 31404, 31405]
allowed_remote_dids     = []
```

However, in the current deployment `mode = "standalone"`, `peer_relays = []`,
and `allowed_remote_dids = []`. The federation bridge **does not exist yet** —
the code and config scaffolding are present but all activation flags are off.
`MESH_PEER_RELAYS` in the wrangler manifest is an empty string
(`relay-worker.wrangler.toml:19`).

### Where a Future Bridge Would Attach

A bridge from this relay to the agentbox relay would attach at two points:

1. **Outbound (CF relay → agentbox)**: the relay would need to be switched from
   `MESH_MODE = "standalone"` to `"federated"` or `"client"`, with the
   agentbox relay's `wss://` URL added to `peer_relays` and the agentbox
   relay's `did:nostr:<pubkey>` added to `allowed_remote_dids`. The
   `federated_kinds` list controls which event kinds cross the boundary.
   The CF Worker relay would then forward matching events to the agentbox relay
   by publishing signed relay-to-relay events.

2. **Inbound (agentbox → CF relay)**: the agentbox relay's pubkey would need to
   be added to `MESH_ALLOWED_REMOTE_DIDS`, and the agentbox relay's events
   would be filtered against `MESH_FEDERATED_KINDS` before admission.

No subscriber-push or webhook mechanism exists. The current DO has no outbound
fetch calls on event ingest. A real bridge implementation would require either:
- Extending `handle_event` in `nip_handlers.rs` to POST matching events to a
  peer relay URL after saving to D1; or
- A separate CF Worker cron job that subscribes to this relay as a client and
  republishes events to the agentbox relay.

**Confirmed absent**: there are no outbound HTTP/WebSocket calls from the relay
to any external endpoint in the current codebase. The `peer_relays` field is
parsed by the config crate but never read by the relay worker itself.

---

## 8. Summary: Critical Questions Answered

### Can an off-the-shelf Android Nostr client connect to this relay with a plain nsec/npub?

**Yes, to connect and read. No, to write without prior admin action.**

- Connection: plain WebSocket upgrade with no origin check, no passkey required.
- NIP-42 AUTH: supported and immediately challenged on connect; any client that
  supports NIP-42 (Amethyst, 0xchat, etc.) can authenticate with its nsec.
- Reading: all non-kind-1059 events are readable without AUTH.
- Kind-1059 DM reading: requires AUTH with the recipient pubkey.
- Writing: the pubkey must be in the relay's D1 `whitelist` table. An admin
  must call `POST /api/whitelist/add` with a NIP-98 token before the mobile
  client can publish any events.

### Does the relay accept and serve encrypted DM kinds (4 / 1059 / 14)?

- **Kind 4** (NIP-04 legacy DM): accepted and stored for whitelisted pubkeys;
  served to any subscriber; **no per-recipient isolation**.
- **Kind 1059** (NIP-59 gift wrap): accepted and stored for whitelisted pubkeys;
  served only to AUTH-verified sessions matching the `p` tag recipient.
  This is the recommended encrypted DM kind with proper isolation.
- **Kind 14** (NIP-17 DM): no special handling. Treated as a regular event.
  Stored and served without per-recipient isolation. The NIP-17 flag in
  `supported_nips` (`nip11.rs:36`) reflects intent, not enforcement. A mobile
  client using NIP-17 would receive no access control benefit beyond whitelist
  gating.

### What exactly is the access-control model?

**Write: pubkey-allowlist (D1 whitelist table) + per-kind trust levels.**
**Read: open (no access control) except kind-1059 (AUTH + recipient-match).**

The relay is described as "whitelist-only" (`nip11.rs:29`) which is accurate
for writes. Reads are substantially open to any connected client for most
kinds, with kind-1059 being the one exception with proper per-recipient
isolation. The web forum-client adds a layer of WebAuthn-backed key management,
but that is invisible to the relay's WebSocket protocol — the relay only
speaks NIP-01 and NIP-42.

---

## File Reference Index

| Claim | File:Line |
|-------|----------|
| Relay WSS URL | `dreamlab.toml:relay.url` |
| Relay CF Worker name | `deploy/relay-worker.wrangler.toml:5` |
| D1 database bindings | `deploy/relay-worker.wrangler.toml:23-46` |
| ALLOWED_ORIGINS | `deploy/relay-worker.wrangler.toml:15` |
| NIP-11 supported_nips | `nostr-bbs-relay-worker/src/nip11.rs:36` |
| NIP-11 auth_required = false | `nostr-bbs-relay-worker/src/nip11.rs:47` |
| NIP-11 restricted_writes = true | `nostr-bbs-relay-worker/src/nip11.rs:49` |
| AUTH challenge on connect | `relay_do/mod.rs:141-147` |
| AUTH response handler | `relay_do/nip_handlers.rs:482-535` |
| Whitelist gate on EVENT | `relay_do/nip_handlers.rs:82-85` |
| Kind-1059 AUTH gate on REQ | `relay_do/nip_handlers.rs:398-430` |
| Kind-1059 broadcast isolation | `relay_do/broadcast.rs:43-67` |
| No origin check on WS upgrade | `src/lib.rs:166-169` |
| Rate limit 10 events/sec/IP | `relay_do/broadcast.rs:75` |
| Rate limit 20 conns/IP | `relay_do/mod.rs:47` |
| Content size caps | `relay_do/nip_handlers.rs:30-34` |
| NIP-11 retention | `src/nip11.rs:51-59` |
| Ingress policy = allowlist | `dreamlab.toml:ingress_policy` |
| Zone cohort enforcement | `relay_do/nip_handlers.rs:234-247` |
| Mesh mode = standalone | `dreamlab.toml:mesh.mode` |
| Mesh peer_relays empty | `dreamlab.toml:mesh.peer_relays` |
| MESH_MODE env var | `deploy/relay-worker.wrangler.toml:18` |
| auto_whitelist (dead code) | `relay_do/storage.rs:336-392` |
| Admin pubkeys | `dreamlab.toml:admin.static_pubkeys` |
| Agent pubkeys | `dreamlab.toml:agents` |
