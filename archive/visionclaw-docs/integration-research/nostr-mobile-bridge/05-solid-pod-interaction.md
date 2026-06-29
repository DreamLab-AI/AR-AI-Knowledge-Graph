# Solid Pod Interaction for the Nostr Mobile Agent-Chat Bridge

**Date:** 2026-06-02  
**Related:** ADR-017, ADR-009, ADR-010, PRD-014  
**Sources:** solid-pod-rs 0.4.0-alpha.15, JavaScriptSolidServer (JSS), agentbox management-api

---

## 1. The Solid Pod Model in This Ecosystem

### Pod-per-did:nostr-identity

Every user gets exactly one pod, keyed by their Nostr public key. The pod URL
is derived deterministically from a 64-character lowercase hex pubkey:

```
pod root:  {base_url}/pods/{pubkey_hex}/
WebID doc: {base_url}/pods/{pubkey_hex}/profile/card
WebID URI: {base_url}/pods/{pubkey_hex}/profile/card#me
```

Source: `solid-pod-rs/crates/solid-pod-rs/src/webid.rs:23-35` — functions
`pod_root_url`, `webid_document_url`, and `webid_url` implement this derivation.
The convention is bech32-free: raw 64-char hex is used on disk and in all URN
paths, matching the ADR-017 path convention (`pods/<did:nostr:pubkey-hex>/`) and
keeping the filesystem free of bech32 decode dependencies during cold-path
operations such as boot and dirent traversal.

The DID URI is `did:nostr:<pubkey-hex>`. The WebID profile document records the
Nostr pubkey as a CID v1 `verificationMethod` (`publicKeyMultibase: "feb<hex>"`)
and cross-links to the DID via `"schema:identifier": "did:nostr:<pubkey>"`. This
makes the pod its own authoritative DID resolver: the JSS
`/.well-known/did/nostr/<pubkey>.json` endpoint serves a DID document with
`alsoKnownAs: [<webId>]` generated on the fly from the local account index
(`JavaScriptSolidServer/src/idp/well-known-did-nostr.js:459-477`).

The Rust port (`solid-pod-rs`) replicates this exactly. `webid.rs:64-114`
generates the HTML/JSON-LD profile that includes the `did:nostr` cross-link and
the multibase-encoded secp256k1 pubkey. solid-pod-rs README line 78-81 states
that the crate implements both Tier 1 (pubkey to DID document) and Tier 3
(DID to WebID cross-verification via `alsoKnownAs`/`owl:sameAs`) resolution.

### WAC / ACL Access Control

Every resource on a Solid pod is protected by Web Access Control. The access
model is:

- **Deny by default** — no ACL document means no access. The WAC evaluator in
  `solid-pod-rs/crates/solid-pod-rs/src/wac.rs` implements this.
- **Modes:** `acl:Read`, `acl:Write`, `acl:Append`, `acl:Control`.
  `Write` is a strict superset of `Append`.
- **Agent matchers:**
  - `acl:agent <did:nostr:PUBKEY>` — grants a specific WebID (which resolves to
    a specific did:nostr keypair) a set of modes.
  - `acl:agentClass foaf:Agent` — public (unauthenticated) access.
  - `acl:agentClass acl:AuthenticatedAgent` — any verified identity.
  - `acl:agentGroup <group-url>` — group membership.
- **Inheritance:** ACLs walk up the path for `.acl` sidecars; `acl:default`
  propagates to all descendants of a container.
- **Default pod isolation (ADR-017):** Each pod's root ACL grants `read` and
  `write` on `pods/<X>/...` to `did:nostr:<X>` and `read` to admin pubkeys.
  Cross-pod access requires an explicit ACL grant on the target pod; agentbox
  provides no cross-pod ambient authority.

The agentbox mandate subsystem (`management-api/lib/mandate.js:137-152`) renders
WAC Turtle fragments that users PUT to `<container>/.acl` to grant a specific
agent write/append authority:

```turtle
@prefix acl: <http://www.w3.org/ns/auth/acl#> .
<#agent-mandate> a acl:Authorization ;
    acl:agent <did:nostr:AGENT_PUBKEY_HEX> ;
    acl:accessTo <CONTAINER_PATH/> ;
    acl:default <CONTAINER_PATH/> ;
    acl:mode acl:Read, acl:Write, acl:Append .
```

The solid-pod-rs WAC evaluator parses `acl:agent` (individual grants) and
matches the value against the NIP-98-derived `did:nostr` WebID verified upstream
(`mandate.js:16-19`).

---

## 2. What Is Stored in Pods Today

Provisioning creates the following structure for every user
(`provision.rs:197-300`, `provision.rs:116-122`):

| Path | Content | ACL |
|------|---------|-----|
| `/profile/card` | WebID HTML/JSON-LD with did:nostr cross-link, CID v1 shape | Inherits root |
| `/settings/publicTypeIndex.jsonld` | `solid:TypeIndex + solid:ListedDocument` | Public read, owner write/control |
| `/settings/privateTypeIndex.jsonld` | `solid:TypeIndex + solid:UnlistedDocument` | Owner-only (inherits) |
| `/settings/publicTypeIndex.jsonld.acl` | Explicit ACL for the public type index | — |
| `/.acl` | Root ACL granting owner full control | — |
| `/inbox/` | LDN inbox container | Owner + invited agents |
| `/public/` | World-readable container | foaf:Agent read |
| `/private/` | Owner-only container | Owner-only |

Beyond provisioning scaffolding, the relay-consumer (`mcp/nostr-bridge/relay-consumer.js`)
writes event data directly to the filesystem pod tree:

| Pod path | Written by | Content |
|----------|-----------|---------|
| `pods/<pubkey>/events/inbox/<event-id>.json` | relay-consumer (line 258) | Verified inbound Nostr events |
| `pods/<pubkey>/events/intent-queue/<id>.json` | relay-consumer (line 377) | Agent-intent event markers (kinds 38000-38099) |
| `pods/<pubkey>/events/governance/<event-id>.json` | relay-consumer (line 526) | Governance events (kinds 31400-31405) |
| `pods/<pubkey>/events/payments/<event-id>.json` | relay-consumer (line 565) | Payment events (kinds 38200-38201) |
| `pods/<pubkey>/events/outbox/<id>.json` | management-api | Pending outbound events |

The payments module (`crates/solid-pod-rs/src/payments.rs:102-125`) defines a
`WebLedger` type stored at `/.well-known/webledgers/webledgers.json`. This
supports per-read satoshi micropayments (HTTP 402), multi-chain TXO deposits,
and MRC20 token tracking — the pod carries the user's payment balance.

Type indexes (`solid:TypeIndex`) are the Solid-standard mechanism for apps to
register what data types live where in a pod. These are present from day one but
are currently empty — no chat/session data is registered in them yet.

---

## 3. Server vs Client: Topology Clarified

**JavaScriptSolidServer (JSS)** is the original Node.js pod server — the
canonical Solid Protocol reference implementation. It is a running server that
handles HTTP requests, LDP resources, WAC, Solid-OIDC, NIP-98, and all
pod-level storage.

**solid-pod-rs** is a Rust port of JSS — a complete pod server in its own
right, not a client library. The README states it delivers "~98% strict parity"
with JSS as "a framework-agnostic Rust library and a drop-in server binary"
(`solid-pod-rs/README.md:3`). It ships seven crates: a core library
(`solid-pod-rs`), a server binary (`solid-pod-rs-server`), and five bounded-context
siblings (idp, activitypub, nostr, git, didkey).

The relationship is:

```
JavaScriptSolidServer (Node.js, AGPL-3.0)
        |
        | reference implementation
        v
solid-pod-rs (Rust, AGPL-3.0)  ← Rust port, same protocol
        |
        | embedded as library in
        v
agentbox management-api  ← writes to pod tree via filesystem backend
VisionClaw               ← wires solid-pod-rs into actix-web
nostr-rust-forum         ← wasm32 core-only consumer
```

In the DreamLab ecosystem, JSS and solid-pod-rs serve different deployment
contexts: JSS is the battle-tested Node.js reference server (usable on
Android/Termux per the README); solid-pod-rs is the production Rust binary that
agentbox packages via Nix and runs at `loopback :8484`. Either can serve as
the pod endpoint. There is no client-SDK-vs-server distinction — both are
full servers that also expose a Rust library API for embedding.

Proton Drive SDK (`proton-drive-sdk/README.md`) is entirely separate: it is a
Rust SDK and TUI for accessing Proton Drive via SRP login. It is NOT part of
the self-sovereign Solid pod story — it is a personal cloud client for a
centralised third-party service and has no Solid, WAC, or did:nostr integration.

---

## 4. Pod Write Auth: Tracing One Write Path

The authoritative write path is NIP-98 HTTP authentication. Here is a complete
trace for an agent writing a resource to a user's pod:

**Step 1 — Mandate issuance (user action, one-time)**

The user creates a mandate via `lib/mandate.js:createMandate()`, specifying the
agent's pubkey and the target container (e.g. `/sessions/`). The mandate is
rendered as WAC Turtle (`mandateToAclTurtle()`) and signed as a NIP-33
parameterised-replaceable event (kind 30078). The user PUTs the WAC Turtle to
`<pod>/sessions/.acl` using their own NIP-98 credential. This grants the agent
`acl:Read`, `acl:Write`, `acl:Append` on `/sessions/` and all its descendants
via `acl:default`.

**Step 2 — NIP-98 token construction (per request)**

The agent constructs a kind-27235 Nostr event:
```json
{
  "kind": 27235,
  "created_at": <now>,
  "tags": [
    ["u", "https://<pod-host>/pods/<pubkey>/sessions/session-abc.jsonld"],
    ["method", "PUT"],
    ["payload", "<SHA-256 of request body>"]
  ]
}
```
The event is Schnorr-signed with the agent's secp256k1 nsec, then
base64-encoded. The `Authorization: Nostr <token>` header is sent with the
HTTP request. This is assembled by `buildPodNip98()` in
`management-api/lib/pod-signer.js:76-80`.

**Step 3 — Pod-side verification**

The pod server (solid-pod-rs or JSS) receives the `PUT` request. The auth
middleware:
1. Extracts the base64 token from the `Authorization: Nostr` header.
2. Decodes the kind-27235 event.
3. Verifies the Schnorr signature via BIP-340 `verify_raw()` — the raw 32-byte
   event ID as the message, no tagged pre-hash (`solid-pod-rs/README.md:239`).
4. Checks timestamp tolerance (±60 s).
5. Confirms the `u` tag matches the request URL and `method` tag matches the
   HTTP method.
6. Derives the acting identity: `did:nostr:<verified_pubkey>`.

**Step 4 — WAC evaluation**

The WAC evaluator resolves the ACL for `/pods/<user>/sessions/session-abc.jsonld`:
1. Looks for `/pods/<user>/sessions/session-abc.jsonld.acl` (resource ACL) —
   not present.
2. Walks up to `/pods/<user>/sessions/.acl` — finds the mandate-generated ACL.
3. Checks `acl:default` — yes, this applies to descendants.
4. Matches `acl:agent <did:nostr:AGENT_PUBKEY>` against the verified did:nostr.
5. Grants `acl:Write` — request proceeds.

If no ACL is found, or the agent's identity does not match any grant: HTTP 403.
The `WAC-Allow` header on 403 responses reports which modes are available
(`solid-pod-rs/README.md:219`).

---

## 5. Decision Input: Where Should Chat Sessions, Summaries, and Grants Live?

### Option A — Solid Pod (durable, self-sovereign, WAC-controlled)

**Capabilities the pod infrastructure actually supports:**

- Structured Turtle/JSON-LD resources with strong ETags and conditional requests.
- WAC-enforced per-resource and per-container access control. The user controls
  exactly which agents can read their session history.
- Solid Notifications 0.2 (WebSocket and Webhook channels) for reactive
  downstream consumers (`solid-pod-rs/README.md:291-298`).
- Type indexes (`solid:TypeIndex`) for Solid app discovery — a chat app could
  register session summaries so other Solid-aware apps find them.
- WebLedger payment conditions (`acl:ClientCondition`) for metered access to
  session records (if desired).
- `PATCH` support (N3 Patch, SPARQL-Update) allowing incremental append to an
  existing session document rather than full replacement.

**Constraints:**

- Writing requires either the user's credential (problematic for autonomous
  agents) or a mandate-backed agent credential — the mandate system is already
  implemented (`lib/mandate.js`).
- The pod server must be reachable from the writer. Agentbox already has this
  at `loopback :8484`.
- No native full-text search — pod query is LDP container listing + client-side
  SPARQL over fetched resources, or an external triplestore.

### Option B — Ephemeral Nostr Relay Events Only

NIP-17 direct messages are sealed, gift-wrapped events that the relay stores
ephemerally. Most relays prune events over time. There is no guaranteed durability,
no structured query surface, and no per-user ownership concept at the storage
layer. Chat sessions and summaries stored ONLY as NIP-17 DMs will be lost when
relays prune or when the user switches relays.

NIP-17 is the correct transport for live message delivery. It is not a
persistence substrate.

### Option C — Hybrid (Nostr Transport + Pod Durable Record) — RECOMMENDED

This is the natural architecture given the existing infrastructure:

1. **Live chat transport:** NIP-17 encrypted DMs over the Nostr relay mesh.
   The phone sends messages; the agentbox agent receives and responds. No Solid
   involvement at this layer.
2. **Durable session summary:** When the session ends (or at configurable
   checkpoints), the agentbox agent writes a structured summary document to the
   user's pod at a path such as:
   ```
   /sessions/<iso-date>-<session-id>.jsonld
   ```
   This is a single PUT or PATCH operation authenticated with NIP-98 under the
   agent's mandate.
3. **Grant records:** Admin grants (e.g. "allow agent X to write to /projects/")
   are WAC ACL documents PUT to the pod by the user. The mandate record itself
   is signed as a NIP-33 event so it can be revoked by re-publishing with
   `revoked: true` — the pod WAC ACL is the enforcement point; the Nostr event
   is the user-owned revocation audit trail.

**Tradeoffs:**

| Criterion | Pod only | Relay only | Hybrid |
|-----------|----------|-----------|--------|
| Durability | High | Low (prune risk) | High |
| User ownership | Full WAC control | Relay-dependent | Full WAC control |
| Real-time delivery | Via Notifications | Native | Nostr for live, pod for archive |
| Android client complexity | Requires Solid auth | NIP-17 only | NIP-17 only (agent side writes pod) |
| Queryable history | Yes (LDP + SPARQL) | No | Yes |
| Offline resilience | Pod must be reachable | Relay must be reachable | Both; graceful degradation |

The hybrid option is superior on every meaningful criterion for a production
deployment. The live transport uses Nostr because that is what it is designed
for; the durable record uses the pod because that is what it is designed for.

---

## 6. Android Client Constraint Analysis

**Does persisting to a Solid pod require the Android Nostr client to do anything
special?**

No. This is the central insight.

Standard Android Nostr clients (Amethyst, Nos, Primal, Damus for Android, etc.)
support the Nostr protocol: keypair management, NIP-17 sealed DMs, relay
connections, and basic NIP-98 HTTP authentication for third-party APIs. None of
them implement Solid Protocol, WAC, LDP, Solid-OIDC, or WebID-OIDC. Adding
that requirement to the Android client would be a significant development burden
and would constrain client choice to a near-empty set.

**The correct boundary is: the agentbox agent writes to the pod, not the phone.**

The mobile flow is:

```
Android phone                   agentbox                    Solid pod
     |                              |                            |
     |-- NIP-17 DM (chat msg) ----> |                            |
     |                              |-- agent processes msg      |
     |<-- NIP-17 DM (response) --   |                            |
     |          ...                 |                            |
     | (session ends)               |                            |
     |                              |-- NIP-98 PUT /sessions/--> |
     |                              |   summary.jsonld           |
     |                              |<-- 201 Created             |
```

The agentbox agent already has:
1. A did:nostr identity (`AGENTBOX_STACK` / `AGENTBOX_PROFILE`).
2. NIP-98 signing capability (`lib/pod-signer.js:buildPodNip98`).
3. A mandate-backed WAC ACL grant on the user's `/sessions/` container
   (created once by the user; the agent writes under its own did:nostr).
4. Filesystem access to the pod tree OR HTTP access to solid-pod-rs at
   `loopback :8484`.

The phone need only know how to:
- Connect to a Nostr relay via WebSocket.
- Sign NIP-17 gift-wrapped DMs with the user's nsec.
- Optionally verify the agent's event signatures (NIP-01 verifyEvent).

This means **any off-the-shelf Android Nostr client that supports NIP-17 works.**
The user's client-side choice is completely decoupled from the pod persistence
layer. The Android client constraint is purely a Nostr capability question, not
a Solid question.

**Edge case — user-initiated pod reads from the phone:**

If in future the user wants to browse their session history from their phone,
this would require either:
- A Solid-aware mobile app (not an off-the-shelf Nostr client), OR
- A Nostr relay integration where the agent publishes summary events back to
  the user's relay so they can be read by any Nostr client.

The hybrid approach can serve both: the pod is the canonical durable store; the
agent optionally publishes a NIP-23 long-form note (or a NIP-99 classified
listing) back to the relay so the phone can read summaries without Solid support.
This is an additive capability that does not change the write architecture.

---

## 7. Self-Sovereignty Principle

The user's emphasis on self-sovereign Solid pods translates to the following
requirements for the chat/summary feature:

**The user owns their session history.** Session summaries written to the pod
land at a path under the user's own pod root, controlled by the user's own WAC.
No session data is stored in a central agentbox database, a shared relay, or
any third-party service.

**The user controls agent access.** The mandate system (`lib/mandate.js`) makes
the grant model explicit: the user signs a NIP-33 event authorising a specific
agent to write to a specific container. The mandate is revocable by re-publishing
with `revoked: true`. The WAC ACL is the enforcement point — the pod server
evaluates it on every request, so revoking the mandate means updating the ACL
document, and the agent loses write access immediately.

**The user can export, migrate, or delete.** All session summaries are standard
Solid resources (Turtle or JSON-LD). The pod exports cleanly to any Solid
server. No proprietary format, no vendor lock-in.

**The agent writes under its own identity, not the user's.** The pod-signer
module signs requests with the agent's own nsec (via `loadSigner(stack)`), not
the user's. The WAC ACL's `acl:agent <did:nostr:AGENT>` grant permits
specifically the agent's identity — the user's nsec is never exposed to or used
by the agentbox process.

**Auditability.** Every pod write is authenticated (NIP-98 signature), logged by
the pod server, and traceable to a specific `did:nostr` identity. Governance
events (kinds 31400-31405) are written to `pods/<user>/events/governance/` and
can be included in session summaries as provenance annotations. This aligns with
the BC20 provenance bridge (`management-api/lib/bc20-provenance-bridge.js`) and
the PROV-O aligned activity records described in the agentbox CLAUDE.md.

**Practical recommendation:** The session summary document should be a
JSON-LD resource at `/sessions/<date>-<session-id>.jsonld` in the user's pod.
It should carry `owner_did: did:nostr:<user_pubkey>`, an `action_urn` linking to
an agentbox activity record, the session start/end timestamps, a structured
summary of work done, and references to any resource URNs created or modified
during the session. The `/sessions/` container should be registered in the user's
`publicTypeIndex.jsonld` (if session history is intentionally shareable) or
`privateTypeIndex.jsonld` (if it is private by default) so Solid-aware apps can
discover it.

---

## Summary Reference

**Pod URL derivation:** `{base_url}/pods/{64-char-hex-pubkey}/` — deterministic,
bech32-free. Source: `solid-pod-rs/crates/solid-pod-rs/src/webid.rs:23-35`.

**WAC enforcement:** default-deny; `acl:agent <did:nostr:HEX>` is the grant
primitive; inheritance via `acl:default` on containers.

**Provisioned artifacts today:** `/profile/card`, `/settings/publicTypeIndex.jsonld`,
`/settings/privateTypeIndex.jsonld`, root `.acl`, `/inbox/`, `/public/`, `/private/`.
Relay-consumer also writes `events/{inbox,intent-queue,governance,payments}/`.

**Topology:** JSS = Node.js reference server; solid-pod-rs = Rust port and
drop-in replacement server, also usable as embedded library. Both are servers.
Proton Drive SDK is unrelated (centralised cloud).

**Auth write path:** NIP-98 kind-27235 Schnorr-signed event in
`Authorization: Nostr <base64>` header, verified against did:nostr; mandate
renders WAC Turtle the user installs once; agent writes under its own identity.

**Recommended architecture for mobile bridge:** Hybrid — Nostr (NIP-17) for live
transport, pod write by agentbox agent at session end. Android client has zero
Solid requirement.
