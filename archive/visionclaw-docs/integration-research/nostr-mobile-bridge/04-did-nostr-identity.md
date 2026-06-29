# 04 — did:nostr Identity Architecture for the Mobile Bridge

| Field | Value |
|-------|-------|
| Author | Research agent (Claude) |
| Date | 2026-06-02 |
| Scope | did:nostr key tiers, DID document shape, NIP-26/NIP-98/Signer, mobile key options |
| Sources | sovereign-bootstrap.py, solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs, solid-pod-rs/crates/solid-pod-rs-nostr/src/did.rs, nostr-bbs-core/src/{nip26,nip98,signer,nip19}.rs, ADR-074, agentbox/agentbox.toml, ddd-mesh-federation-context.md |

---

## 1. The did:nostr Method

### 1.1 DID Derivation

A `did:nostr` DID is derived deterministically from a secp256k1 public key via BIP-340
x-only encoding:

```
DID = "did:nostr:" + hex(x_only_pubkey_32_bytes)
```

The hex encoding is always 64 lowercase characters. No bech32, no checksum, no prefix
other than `did:nostr:`. The regex is `^did:nostr:[0-9a-f]{64}$`.

Sources:
- `sovereign-bootstrap.py:201` — `did = f"did:nostr:{identity['x_only_pubkey_hex']}"`
- `solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs:59-61` — `fn did_nostr_uri(pk: &NostrPubkey) -> String { format!("did:nostr:{}", pk.to_hex()) }`
- `ADR-074 D1` — "Wire identity: 64-char lowercase hex (no 0x, no checksum)"

The DID document is served at a well-known path derived from the same pubkey:

```
/.well-known/did/nostr/<64-hex>.json
```

Source: `solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs:65-67` —
`fn well_known_path(pk: &NostrPubkey) -> String { format!("/.well-known/did/nostr/{}.json", pk.to_hex()) }`

### 1.2 Canonical DID Document Shape

There are two tiers. Both share the same verificationMethod structure; only the presence
of service entries and `alsoKnownAs` (WebID binding) distinguishes them.

**Tier-1 (minimum-viable):**

```jsonld
{
  "@context": [
    "https://www.w3.org/ns/did/v1",
    "https://w3id.org/security/suites/secp256k1-2019/v1"
  ],
  "id": "did:nostr:<64-hex>",
  "alsoKnownAs": [],
  "verificationMethod": [{
    "id": "did:nostr:<hex>#nostr-schnorr",
    "type": "SchnorrSecp256k1VerificationKey2019",
    "controller": "did:nostr:<hex>",
    "publicKeyHex": "<64-hex>",
    "publicKeyMultibase": "z<base58btc(0xe7 0x01 || pk_32)>"
  }]
}
```

Source: `solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs:109-126` —
`fn render_did_document_tier1`.

**Tier-3 (production, with services):**

```jsonld
{
  "@context": [
    "https://www.w3.org/ns/did/v1",
    "https://w3id.org/security/suites/secp256k1-2019/v1"
  ],
  "id": "did:nostr:<64-hex>",
  "alsoKnownAs": ["<webid_url>"],
  "verificationMethod": [{
    "id": "did:nostr:<hex>#nostr-schnorr",
    "type": "SchnorrSecp256k1VerificationKey2019",
    "controller": "did:nostr:<hex>",
    "publicKeyHex": "<64-hex>",
    "publicKeyMultibase": "z<base58btc(0xe7 0x01 || pk_32)>"
  }],
  "authentication": ["did:nostr:<hex>#nostr-schnorr"],
  "assertionMethod": ["did:nostr:<hex>#nostr-schnorr"],
  "service": [
    { "id": "...#solid-pod",   "type": "SolidStorage",  "serviceEndpoint": "<pod_base>" },
    { "id": "...#nostr-relay", "type": "NostrRelay",    "serviceEndpoint": "<wss_url>" },
    { "id": "...#webid",       "type": "SolidWebID",    "serviceEndpoint": "<webid_url>" }
  ]
}
```

Source: `solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs:135-183` —
`fn render_did_document_tier3`.

The verificationMethod fragment identifier is `#nostr-schnorr` in solid-pod-rs but
`#key-0` in agentbox's sovereign-bootstrap.py (line 248: `"id": f"{did}#key-0"`). ADR-074
D2 standardises `#key-0` as the cross-system canonical name and requires all emitters to
align. The `solid-pod-rs` crate test at `crates/solid-pod-rs-nostr/src/did.rs:98` asserts
`format!("did:nostr:{PK_HEX}#nostr-schnorr")` — this divergence is a pre-ADR-074
implementation gap, flagged in the ADR as a required fix.

### 1.3 verificationMethod.type — the canonical value

The canonical and only valid type identifier is:

```
SchnorrSecp256k1VerificationKey2019
```

This is the only published W3C suite for secp256k1 Schnorr verification keys. ADR-074 D1
states explicitly: "SchnorrSecp256k1VerificationKey2022 was a spec-drift fabrication" and
sovereign-bootstrap.py had previously emitted it (line 239 comment in the current file
records this fix). The `secp256k1-2019/v1` context URL
`https://w3id.org/security/suites/secp256k1-2019/v1` must appear in `@context` for the
term to resolve under JSON-LD — both tiers require it.

Source: `ADR-074 D1`; `sovereign-bootstrap.py:236-247` (inline comment explains the fix);
`solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs:99-108` (module docstring).

### 1.4 publicKeyMultibase Encoding

`publicKeyMultibase` = `'z'` + base58btc(`0xe7` `0x01` || `pk_32_bytes`).

- Multicodec `0xe7` = `secp256k1-pub`
- `0x01` = varint-1 marker
- `pk_32` = 32-byte BIP-340 x-only pubkey

Source: `solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs:192-198` —
`fn format_multibase_schnorr`.

### 1.5 Does a Phone Key Automatically Get a DID?

Yes. Every secp256k1 keypair has a canonical did:nostr DID by construction — the DID
is the hex pubkey with a prefix. Any device that holds a 32-byte secp256k1 private key
(or the corresponding x-only public key) trivially derives its DID without interaction
with any external service. The DID document is only resolvable if the device publishes
it to a relay or a well-known endpoint, but the DID URI itself exists unconditionally.

---

## 2. Key Tiers

### 2.1 The Tier Scheme in nostr-bbs-config

The tier classification comes from `nostr-bbs-config/src/schema.rs:209-215` (struct
`Custody`, field `operator`) and is validated in
`nostr-bbs-config/src/validate.rs:85-89`. The four tiers per ADR-079 §4:

| Tier | Label | Custody Model |
|------|-------|---------------|
| tier-1 | Self-host | Operator runs their own hardware. Key persisted on-device. |
| tier-2 | CF Workers Secrets | Key in Cloudflare Workers Secrets (encrypted at rest, injected at runtime). Operator does not hold key in a file. |
| tier-3 | Managed PaaS | Provider-managed environment; key lifecycle handled by operator tooling. |
| tier-4 | Turnkey hosted | Full SaaS; operator key provisioned and rotated by the hosting platform. |

Source: `nostr-rust-forum/crates/nostr-bbs-config/src/schema.rs:212-213`.
Default in the test fixture: `tier-2` (`nostr-bbs-config/src/validate.rs:159`).

These tiers describe the **forum operator's** key custody. They should not be confused
with the DID document Tier-1 / Tier-3 content levels, which are a different naming
collision in the codebase.

### 2.2 Agentbox Sovereign Agent Key

The agentbox generates or loads a per-container secp256k1 keypair via
`sovereign-bootstrap.py:125-182` (function `ensure_identity`). The key lifecycle:

1. **Priority 1**: env var `AGENTBOX_PRIVKEY_HEX` (64-char hex) or `AGENTBOX_NSEC` (bech32
   nsec1) — set in `.env` for a stable signing identity. Used for the agentbox container
   identity (the "sovereign agent").
2. **Priority 2**: persisted identity file at `/var/lib/agentbox/identities/<agent_id>.json`.
   Generated on first boot; immutable thereafter (A-Inv-04 in ddd-mesh-federation-context.md).
3. **Migration path**: older identity files that encoded npub from the 64-byte SEC1 pubkey
   instead of the 32-byte x-only pubkey are detected and corrected in place
   (`sovereign-bootstrap.py:155-171`).

Runtime env vars written to `/run/agentbox/identity.env` (`sovereign-bootstrap.py:258-275`):
- `AGENTBOX_DID` = `did:nostr:<x_only_pubkey_hex>`
- `AGENTBOX_X_ONLY_PUBKEY_HEX` = 64-char hex x-only pubkey
- `AGENTBOX_NPUB` = bech32 npub (for Nostr-internal paths only)
- `AGENTBOX_NSEC` = bech32 nsec

### 2.3 Admin / Operator Pubkey

The agentbox management-api has an `admin_pubkeys` list in `agentbox.toml:159`:

```toml
[sovereign_mesh.multi_user]
admin_pubkeys = []   # operator pubkey auto-added at boot
```

In `strict-nip98` mode (automatically active when `AGENTBOX_SOVEREIGN_MESH_ENABLED=true`,
per `management-api/middleware/auth.js:105-108`), every request to the management-api
must carry a NIP-98 token. The token's `pubkey` field is the authenticated identity; the
`admin_pubkeys` list gates admin-only routes (`management-api/routes/admin-users.js:19`).

The operator pubkey is read from `AGENTBOX_X_ONLY_PUBKEY_HEX` (set by sovereign-bootstrap
at boot). A request is admin-privileged if its NIP-98 `pubkey` matches either the operator
pubkey or an entry in `admin_pubkeys`.

The agentbox's own key (its `did:nostr` identity) is the default admin. Any additional
pubkey that needs admin access to the management-api must be listed in `admin_pubkeys`.

### 2.4 VisionClaw / Forum Operator Keys

- **VisionClaw**: single operator key in env var `SERVER_NOSTR_PRIVKEY` (or transitionally
  `VISIONCLAW_NOSTR_PRIVKEY`). Post-ADR-074 D6, both must resolve to the same bytes or
  startup fails.
- **Forum**: three signing backends unified through the `Signer` trait — passkey-PRF-derived,
  NIP-07 browser extension, or raw nsec. All yield a 32-byte secret and the same
  cryptographic primitive (`nostr-bbs-core/src/signer.rs:74-123`).

---

## 3. Crypto Primitive: BIP-340 Schnorr x-only Pubkeys

### 3.1 x-only vs Compressed

A secp256k1 public key can be represented in three forms:

| Form | Bytes | Contains |
|------|-------|---------|
| Uncompressed SEC1 | 64 (or 65 with 04 prefix) | X coordinate (32 bytes) + Y coordinate (32 bytes) |
| Compressed SEC1 | 33 bytes | 02/03 prefix + X coordinate (32 bytes) |
| BIP-340 x-only | 32 bytes | X coordinate only (even-Y implied) |

NIP-01 and all Nostr-protocol operations use the **32-byte x-only** form exclusively.
`npub` bech32 encodes the 32-byte x-only pubkey — encoding the 64-byte SEC1 form
produces an npub that no Nostr relay or client can verify.

The gotcha: `ecdsa` library's `verifying_key.to_string()` returns 64-byte SEC1 (X||Y);
`public_bytes[:32]` extracts the X coordinate. sovereign-bootstrap.py correctly handles
this at `sovereign-bootstrap.py:81-105` (function `_x_only_pubkey_with_even_y`).

### 3.2 Even-Y Requirement (BIP-340 lift_x)

BIP-340 §3.1 requires that the pubkey used for Schnorr signing has an even Y coordinate.
If the generated pubkey has odd Y, the private key must be negated:

```python
# sovereign-bootstrap.py:95-103
if y_bytes[-1] & 0x01:  # odd y
    n = SECP256K1.order
    d = int.from_bytes(signing_key.to_string(), "big")
    d_neg = (n - d) % n
    signing_key = SigningKey.from_string(d_neg.to_bytes(32, "big"), curve=SECP256K1)
```

Source: `sovereign-bootstrap.py:81-105`.

The Rust side handles this automatically via `k256::schnorr::SigningKey` (which always
normalises to even-Y).

### 3.3 sign_raw / verify_raw

In `nostr-bbs-core/src/nip26.rs`, the NIP-26 delegation token is signed and verified
using the raw BIP-340 message hash (SHA-256 of the delegation string) directly — not
through the NIP-01 event serialisation path:

- `sign_raw`: `signing_key.sign_raw(&hash, &aux)` — `nip26.rs:165`
- `verify_raw`: `verifying_key.verify_raw(&hash, &sig)` — `nip26.rs:190`

These are `k256::schnorr` methods that sign/verify a pre-hashed message without the NIP-01
event wrapper. The NIP-98 verification path uses the NIP-01 event hash instead
(`nostr-bbs-core/src/nip98.rs:446-447` — `verify_event(&event)`), which recomputes the
event id from canonical NIP-01 serialisation before checking the Schnorr signature.

The IdP SSO path (`solid-pod-rs/crates/solid-pod-rs-idp/src/schnorr.rs:359-409`) uses
`k256::schnorr::VerifyingKey::from_bytes(&pub_bytes)` and `vk.verify(&digest, &sig)` —
where `digest` is `SHA-256(token || user_id || pubkey_hex)`.

---

## 4. Delegation (NIP-26): Phone as Delegated Key

### 4.1 Implementation Status

NIP-26 is **fully implemented** in `nostr-rust-forum/crates/nostr-bbs-core/src/nip26.rs`.

Core types and functions:

| Symbol | Location | Purpose |
|--------|----------|---------|
| `Conditions` | `nip26.rs:48-130` | Parses/serialises `kind=N&created_at>T&created_at<T` |
| `DelegationToken` | `nip26.rs:134-224` | Holds delegator/delegatee pubkeys, conditions, sig |
| `DelegationToken::create` | `nip26.rs:150-175` | Signs a delegation token with delegator's secret key |
| `DelegationToken::verify` | `nip26.rs:178-192` | Verifies Schnorr sig over the delegation hash |
| `DelegationTag` | `nip26.rs:228-247` | Wire format: `["delegation", delegator_pk, conditions, sig]` |
| `validate_delegation_tag` | `nip26.rs:258-277` | Full validation: parse + verify + conditions check |

The signing and verification use `k256::schnorr`:
- Signing: `signing_key.sign_raw(&hash, &aux)` at `nip26.rs:165`
- Verification: `verifying_key.verify_raw(&hash, &sig)` at `nip26.rs:190`

The delegation token hash is:
```
SHA-256("nostr:delegation:" || delegatee_pubkey_hex || ":" || conditions_str)
```
Source: `nip26.rs:281-284`.

### 4.2 Wiring Status

The implementation exists but ADR-074 D8 documents that NIP-26 is **not yet wired** into
the three consumer paths:

- `relay-worker/src/relay_do/nip_handlers.rs::handle_event` — not wired (must be added per F8)
- `agentbox/mcp/nostr-bridge/relay-consumer.js::_processEvent` — not wired
- `src/services/mesh_bridge.rs::handle_inbound` — VisionClaw, not yet implemented

ADR-074 also notes (Implementation notes) that post-ADR-076, the forum's own `nip26.rs`
may be deleted in favour of the upstream `nostr` crate's `nostr::nips::nip26` — the test
roundtrip in `nostr-bbs-core/tests/nip26_tests.rs` already passes.

### 4.3 Delegation for a Phone Device Key

A NIP-26 delegation token from the admin key to a phone key would take the form:

```json
["delegation",
  "<admin_pubkey_hex>",
  "kind=27235&created_at>T_start&created_at<T_end",
  "<schnorr_sig_hex>"]
```

Where:
- `kind=27235` restricts the delegation to NIP-98 HTTP auth events only (management-api use case)
- `T_start` / `T_end` bound the validity window
- The phone signs events with its own key and appends this tag

Conditions can be combined; the `Conditions::permits` logic (`nip26.rs:114-129`) ORs
multiple `kind=` clauses and ANDs the time constraints.

---

## 5. NIP-98 HTTP Auth Tokens

### 5.1 Token Shape

NIP-98 is fully implemented in `nostr-rust-forum/crates/nostr-bbs-core/src/nip98.rs`.
It is the canonical ecosystem implementation — "All verification paths converge here;
downstream crates should depend on `nostr_bbs_core::nip98`" (`nip98.rs:8-9`).

An NIP-98 HTTP auth token is a Nostr event (kind 27235) that commits to a specific URL and
HTTP method:

```json
{
  "id":         "<recomputed_event_id>",
  "pubkey":     "<64-hex x-only signer pubkey>",
  "created_at": <unix_seconds>,
  "kind":       27235,
  "tags": [
    ["u",       "<full_request_url>"],
    ["method",  "<HTTP_method>"],
    ["payload", "<sha256_hex_of_body>"]   // optional, when body present
  ],
  "content":   "",
  "sig":        "<64-byte Schnorr sig hex>"
}
```

This JSON is base64-encoded and sent as `Authorization: Nostr <base64>`.

### 5.2 What is Signed

The event `id` is SHA-256 of the canonical NIP-01 serialisation:
```
SHA-256( JSON([ 0, pubkey, created_at, kind, tags, content ]) )
```

The Schnorr signature covers this event id. The verifier recomputes the id from scratch —
it never trusts the client-provided `id` field (`nip98.rs:488-489`).

### 5.3 Verification Checks (in order)

Source: `nip98.rs:1-27` module docstring, `verify_token_full` function at lines 400-496:

1. Strip `Nostr ` prefix
2. Size-gate (max 64 KiB)
3. Base64-decode + JSON-parse to `NostrEvent`
4. Assert `kind == 27235`
5. Validate pubkey format (64 lowercase hex chars)
6. Timestamp freshness (default ±60 seconds)
7. Recompute event id + verify BIP-340 Schnorr signature
8. Match `u` tag to expected URL
9. Match `method` tag to expected HTTP method (case-insensitive)
10. If body present, verify `payload` tag = SHA-256(body)

### 5.4 Agentbox Management-API Integration

When `sovereign_mesh.enabled = true`, the management-api auto-elevates to `strict-nip98`
mode (`management-api/middleware/auth.js:105-108`). Every call to the management-api must
carry a NIP-98 token. The token's `event.pubkey` is the authenticated caller's DID subject.
A phone using NIP-98 presents its own pubkey as the signer — the management-api records
`request.auth.pubkey` as the authenticated identity.

The agentbox management-api uses `NostrBridge.verifyNip98()` from nostr-tools for
verification (`management-api/middleware/auth.js:47-60`). The Rust canonical verifier in
`nostr-bbs-core/src/nip98.rs` is used by the forum CF Workers path.

### 5.5 Replay Protection

The `Nip98ReplayStore` trait (`nip98.rs:80-94`) abstracts the replay-cache backend. The
forum uses a D1/KV-backed implementation; tests use `InMemoryReplayStore` (`nip98.rs:1100`).
The agentbox does not yet wire replay protection at the management-api layer.

---

## 6. Signer Abstraction

### 6.1 The Signer Trait

Defined in `nostr-rust-forum/crates/nostr-bbs-core/src/signer.rs:74-123`:

```rust
#[async_trait(?Send)]
pub trait Signer {
    fn public_key(&self) -> &str;
    async fn sign_event(&self, unsigned: UnsignedEvent) -> Result<NostrEvent, SignerError>;
    async fn nip44_encrypt(&self, recipient_pubkey_hex: &str, plaintext: &str) -> Result<String, SignerError>;
    async fn nip44_decrypt(&self, sender_pubkey_hex: &str, ciphertext: &str) -> Result<String, SignerError>;
    async fn nip04_encrypt(&self, recipient_pubkey_hex: &str, plaintext: &str) -> Result<String, SignerError>;
    async fn nip04_decrypt(&self, sender_pubkey_hex: &str, ciphertext: &str) -> Result<String, SignerError>;
    fn raw_key_bytes(&self) -> Option<[u8; 32]> { None }
}
```

The `raw_key_bytes()` method returns `None` by default. Only `PrfSigner` (locally-held
keypair) returns `Some` — browser extension signers (NIP-07) never expose raw bytes
(`signer.rs:121-123`).

### 6.2 PrfSigner

The only concrete implementation in this crate is `PrfSigner` (`signer.rs:134-219`), backed
by a `Keypair` (typically derived from a WebAuthn PRF output via HKDF). The private key
lives in memory and is zeroized on drop.

The module docstring (`signer.rs:1-8`) says the file covers:
- `PrfSigner` — wraps a local `Keypair` (WebAuthn PRF-derived key)
- `Nip07Signer` — described as delegating to `window.nostr` via WASM (lives in the
  `forum-client` crate, not this module)

### 6.3 NIP-46 (Remote Signer / Bunker)

The signer.rs module docstring references NIP-46 only in the comment title:
`//! NIP-46 / generic Signer trait for nostr-bbs.` (`signer.rs:1`). There is no
implementation of a NIP-46 remote signer (bunker). The forum-client crate auth header
at `nostr-bbs-forum-client/src/auth/nip98.rs:11` mentions "future hardware bunkers" as a
comment about what the `Signer` trait would accommodate.

**NIP-46 is absent across the entire ecosystem.** Searching all Rust and TypeScript source
files across nostr-rust-forum, solid-pod-rs, and project/agentbox yields only:
- `signer.rs:1` — title comment only
- `forum-client/src/auth/nip98.rs:11` — "future hardware bunkers" prose comment
- `nostr-rust-forum/docs/phase1-impact-assessment.md` — forward-looking prose only
- No implementation: no `NIP46Signer`, no `nostrconnect://` URI handling, no bunker
  protocol message types, no remote signer state machine.

---

## 7. Mobile Device Key: Options Analysis

### 7.1 Option A — Phone holds the raw admin nsec

The phone holds the agentbox's own 32-byte secret key (the same key that is stored in
`/var/lib/agentbox/identities/<agent_id>.json` and exported as `AGENTBOX_NSEC`).

The phone derives `did:nostr:<x_only_pubkey_hex>` from this key and signs NIP-98 tokens
directly. This is the simplest path — the management-api recognises the pubkey as the
operator identity unconditionally.

**How it fits:**
- Immediate compatibility: the management-api in strict-nip98 mode already accepts any
  NIP-98 token whose pubkey matches `AGENTBOX_X_ONLY_PUBKEY_HEX`.
- No additional wiring required.

**Risks:**
- The admin nsec is a single point of failure for the entire agentbox identity. If the
  phone is lost, stolen, or compromised, the agentbox identity is compromised and cannot
  be recovered without a full identity rotation.
- Key rotation requires re-provisioning the pod ACLs (which are keyed to
  `did:nostr:<x_only_pubkey_hex>`), the DID document, and any downstream mesh trust
  records.
- This violates the principle that the container key should be stable
  (DDD A-Inv-04 in ddd-mesh-federation-context.md).

### 7.2 Option B — Phone holds a NIP-26-delegated key

The agentbox admin key signs a NIP-26 delegation token authorising the phone's key to
sign NIP-98 events on its behalf. The phone holds its own independent keypair (phone-DID)
and attaches the delegation tag to every NIP-98 event it sends.

**Token creation (on the agentbox or a trusted admin tool):**
```python
# conditions: restrict to NIP-98 HTTP auth events, bounded time window
# nostr-bbs-core/src/nip26.rs:150-175 (DelegationToken::create)
conditions = "kind=27235&created_at>T_start&created_at<T_end"
token = DelegationToken::create(admin_sk, phone_pubkey_hex, conditions)
```

Wire format appended to every phone NIP-98 event:
```json
["delegation", "<admin_pubkey_hex>", "kind=27235&created_at>T&created_at<T", "<sig>"]
```

**How it fits:**
- `DelegationToken::create` and `validate_delegation_tag` are fully implemented in
  `nostr-bbs-core/src/nip26.rs`.
- The management-api would need to unwrap the delegation tag and accept the event if the
  delegator pubkey matches the operator pubkey (or an `admin_pubkeys` entry).
- ADR-074 D11 specifies exactly this pattern for WAC ACL delegation acceptance.
- The phone's key gets its own `did:nostr` DID. It is a first-class identity in the mesh.

**Risks:**
- NIP-26 is not yet wired into the management-api auth path. The `auth.js` middleware
  currently only verifies the direct NIP-98 Schnorr signature; it does not inspect
  `["delegation", ...]` tags.
- Wiring required: after `verifyNip98Header` succeeds, the middleware must detect a
  delegation tag, call `validate_delegation_tag`, and check that the delegator pubkey is
  in the admin set.
- Delegation tokens have a time window — the token must be refreshed before expiry. The
  agentbox (or an operator tool) must re-issue tokens.
- Revocation: NIP-26 has no explicit revocation mechanism other than waiting for the
  expiry timestamp. If the phone is compromised, the shortest revocation is to let the
  token expire (or issue a new token with a `created_at<now` condition to immediately
  invalidate it by making the window impossible to satisfy).

### 7.3 Option C — Phone as a NIP-46 Remote Signer (Bunker)

In this model, the phone acts as a hardware bunker — it holds the admin key and responds
to signing requests from the agentbox or other clients via the NIP-46 protocol over
Nostr relays. Clients ask the phone to sign events; the phone approves and returns the
signature.

**How it fits:**
- NIP-46 is **absent** from the ecosystem. There is no bunker protocol implementation,
  no `nostrconnect://` handling, no remote signing RPC message types, and no NIP-46
  client in any of the four codebases.
- Implementing NIP-46 from scratch would require: bunker state machine, NIP-44 encrypted
  request/response messages, relay connectivity on the phone, key approval UI.

**Risks:**
- Not viable in the short term. Building a NIP-46 bunker is a significant engineering
  commitment with no existing ecosystem code to build on.
- Introduces a latency dependency: every request to the management-api would require a
  round-trip to the phone's relay connection before the auth token can be produced.
- NIP-46 is not part of any PRD or ADR in the current roadmap.

---

## 8. Summary of Critical Question Answers

### Is NIP-26 delegation actually implemented?

Yes. `nostr-rust-forum/crates/nostr-bbs-core/src/nip26.rs` contains a complete
implementation: `DelegationToken::create` at line 150, `DelegationToken::verify` at
line 178, `validate_delegation_tag` at line 258. The `sign_raw` call is at line 165;
`verify_raw` at line 190. The implementation is test-covered
(`nostr-bbs-core/tests/nip26_tests.rs`).

It is **not yet wired** into the three consumer ingress paths — that wiring is specified
as ADR-074 D8 / PRD-010 F8 work items.

### Is NIP-46 present anywhere in the ecosystem?

Absent. The only references are:
- `nostr-bbs-core/src/signer.rs:1` — title comment only, no implementation.
- `nostr-bbs-forum-client/src/auth/nip98.rs:11` — future-looking prose comment.

No NIP-46 types, protocol handlers, or bunker implementations exist in any of the four
codebases.

### What is the canonical did:nostr verificationMethod type?

`SchnorrSecp256k1VerificationKey2019`

Confirmed in:
- `solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs:118` and line 175
- `solid-pod-rs/crates/solid-pod-rs-nostr/src/did.rs:73` (test assertion)
- `sovereign-bootstrap.py:244` (inline comment explaining the correction from the
  non-existent `SchnorrSecp256k1VerificationKey2022`)
- `ADR-074 D1` — mandated as the cross-system canonical value

### Does a phone key automatically get a DID?

Yes. `did:nostr:<x_only_pubkey_hex>` is computed locally from any secp256k1 keypair with
no external service dependency. See section 1.5 above.

---

## 9. References

| Source | Path | Relevance |
|--------|------|-----------|
| sovereign-bootstrap.py | `agentbox/scripts/sovereign-bootstrap.py` | Key derivation, even-Y, DID doc generation, pod ACL |
| did_nostr_types.rs | `solid-pod-rs/crates/solid-pod-rs/src/did_nostr_types.rs` | Canonical Tier-1/Tier-3 renderers, multibase encoding |
| did.rs (nostr crate) | `solid-pod-rs/crates/solid-pod-rs-nostr/src/did.rs` | Re-export layer, test assertions |
| nip26.rs | `nostr-rust-forum/crates/nostr-bbs-core/src/nip26.rs` | DelegationToken, sign_raw/verify_raw, conditions |
| nip98.rs | `nostr-rust-forum/crates/nostr-bbs-core/src/nip98.rs` | NIP-98 token creation and full 10-step verification |
| signer.rs | `nostr-rust-forum/crates/nostr-bbs-core/src/signer.rs` | Signer trait, PrfSigner, NIP-46 absence noted |
| nip19.rs | `nostr-rust-forum/crates/nostr-bbs-core/src/nip19.rs` | npub/nsec bech32 encode/decode |
| schnorr.rs | `solid-pod-rs/crates/solid-pod-rs-idp/src/schnorr.rs` | Nip07SchnorrSso, BIP-340 verify via k256 |
| ADR-074 | `docs/adr/ADR-074-cross-system-did-nostr-canonicalisation.md` | Canonical type, D1/D2/D8/D11/D12, wiring status |
| schema.rs | `nostr-rust-forum/crates/nostr-bbs-config/src/schema.rs:209-215` | tier-1/tier-2/tier-3/tier-4 operator custody |
| agentbox.toml | `agentbox/agentbox.toml:159` | admin_pubkeys, strict-nip98 auto-elevation |
| auth.js | `agentbox/management-api/middleware/auth.js:105-108` | strict-nip98 mode, NIP-98 verification path |
| ddd-mesh-federation-context.md | `docs/ddd-mesh-federation-context.md` | A-Inv-04, BC-MESH-AGENTBOX invariants |
