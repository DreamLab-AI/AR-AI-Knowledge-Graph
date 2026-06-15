---
id: ADR-074-D2′
title: did:nostr Multikey convergence — single canonical DID-document form (supersedes ADR-074 §D2 only)
status: accepted
date: 2026-06-15
type: contract
author: Lead architect (convergence consolidation)
supersedes_clause: ADR-074 §D2 (the SchnorrSecp256k1VerificationKey2019 / publicKeyHex document shape) and its dependent clauses §D3/§D4/§D13 where they pin the 2019 multibase/tier/anti-drift shape. ADR-074 §D1 is RETAINED UNCHANGED.
ecosystem_aliases: agentbox ADR-033 (did-nostr-multikey-convergence); project ADR-125 (did-nostr-multikey-convergence). This document is the canonical lead-architect statement; ADR-033/ADR-125 are the per-repo enactments and remain in force.
depends_on: [ADR-074 (D1 retained), ADR-013, ADR-009, ADR-010, ADR-012]
ground_truth: melvincarvalho/create-agent index.js; did-nostr Community Group spec (nostrcg.github.io/did-nostr)
review_trigger: the did-nostr CG spec publishes a new verificationMethod shape; create-agent index.js changes its emitted document; or any change to a DID-doc emitter (solid-pod-rs did_nostr_types::render_did_document, agentbox s04-did.js, sovereign-bootstrap.py inline DID-doc block, forum nostr-bbs-core/src/did.rs)
---

# ADR-074 §D2′ — did:nostr Multikey convergence (single canonical DID-document form)

**This supersedes ADR-074 §D2 ONLY. ADR-074 §D1 stays.**

## TL;DR

The DID document for `did:nostr:<hex>` previously carried a
`SchnorrSecp256k1VerificationKey2019` verification method with a `publicKeyHex`
field and a `z…` base58btc multibase (ADR-074 §D2/§D3). The did-nostr Community
Group spec and the canonical reference implementation
(`melvincarvalho/create-agent` `index.js`) instead emit a **single `Multikey`**
verification method whose `publicKeyMultibase` is `fe70102` + the 64-char
lowercase x-only hex. This ADR adopts **that single form ecosystem-wide and
drops the 2019 suite — we do NOT dual-publish.** The agent's identity (the
`did:nostr:<hex>` string, the BIP-340 x-only even-y hex pubkey) is **unchanged**.
Auth (NIP-98) verifies the raw pubkey in the signed event and never reads the
verification method, so this is a pure re-encoding of the same key.

## 1. The canonical single form (THE target — no dual-publish)

This is the **only** DID document the ecosystem emits. Every emitter — `solid-pod-rs`
`did_nostr_types::render_did_document` (the single source of truth that the forum
and VisionClaw delegate to), agentbox `s04-did.js` (once corrected, §8.2), agentbox
`sovereign-bootstrap.py` (its inline DID-doc block, once corrected, §8.2) — must
produce byte-identical output:

```jsonld
{
  "@context": ["https://w3id.org/did", "https://w3id.org/nostr/context"],
  "id": "did:nostr:<hex>",
  "type": "DIDNostr",
  "verificationMethod": [{
    "id": "did:nostr:<hex>#key1",
    "type": "Multikey",
    "controller": "did:nostr:<hex>",
    "publicKeyMultibase": "fe70102<hex>"
  }],
  "authentication": ["#key1"],
  "assertionMethod": ["#key1"],
  "service": []
}
```

- `<hex>` is the 64-char lowercase BIP-340 x-only even-y pubkey. The same hex
  appears in three places: the DID body (`id`), the VM `controller`, and the
  tail of `publicKeyMultibase`.
- `type: "DIDNostr"` is a **top-level** field (not only on the VM).
- The VM fragment is `#key1` (not `#key-0`, not `#nostr-schnorr`).
- `authentication` / `assertionMethod` are **relative** fragment refs `["#key1"]`.
- `service: []` is the canonical reference output. Populated `service[]` entries
  (`SolidStorage`, `NostrRelay`, `SolidWebID`) are **agentbox/DreamLab
  extensions**, layered by callers/manifest gates, never "the create-agent form".
- The `SchnorrSecp256k1VerificationKey2019` type, the `publicKeyHex` field, the
  `z…` base58btc multibase, and the `["https://www.w3.org/ns/did/v1",
  "https://w3id.org/security/suites/secp256k1-2019/v1"]` context are **DROPPED**.

## 2. The `fe70102` mapping (exact, byte-level — the one mechanical fact)

`publicKeyMultibase` = the literal string `"fe70102"` followed by the **same**
64-char lowercase x-only hex that is in the DID body. Total length **71 chars**,
regex `^fe70102[0-9a-f]{64}$`.

Decoded segment-by-segment (every segment is load-bearing):

| Segment | Bytes | Role | Wrong-if |
|---|---|---|---|
| `f` | — | base16-**lower** multibase indicator | `F` (uppercase) is a different, malformed-here form |
| `e701` | `0xe7 0x01` | unsigned-varint of multicodec `0xe7` = `secp256k1-pub`. Two bytes because `0xe7 ≥ 0x80`. | a single-byte `e7` (`fe702…`) is wrong |
| `02` | `0x02` | SEC1 compressed-point **even-y** prefix — the **first byte of the 33-byte multicodec payload**, NOT a separator. BIP-340 `lift_x` always selects even-y, so this is invariantly `02`. | omitting it gives the 67-char `fe701<x>` form that does NOT round-trip |
| `<hex>` | 64 chars | the 32-byte x-only `X`, byte-identical to the `did:nostr:<hex>` body | uppercase, or != the DID body |

The `secp256k1-pub` codec (`0xe7`) is defined over the **33-byte compressed
point** (`02 ‖ X`), never the raw 32-byte x-only value. Length is invariant:
`f`(1) + `e701`(4) + `02`(2) + `X`(64) = **71**. The encoder MUST (a) prepend the
`0x02` parity byte, (b) emit lowercase hex throughout, (c) round-trip to the
identical key.

**The single mechanical assertion every conformance gate enforces:**
`publicKeyMultibase == "fe70102" + document.id["did:nostr:".len ..]`
(i.e. `publicKeyMultibase[7:]` equals the DID-body hex). This is the same key
re-encoded; no key bytes change (I2).

Canonical reference encoder (already landed): `solid-pod-rs`
`did_nostr_types::format_multibase_schnorr` —
`format!("fe70102{}", hex::encode(pk))` — with `MULTIKEY_PREFIX = "fe70102"`,
`MULTIKEY_LEN = 71`, and a strict `parse_multibase_schnorr` ACCEPT-path decoder
that rejects base58 (`z…`), the missing-parity 67-char form, non-`02` parity, and
uppercase-under-`f`.

## 3. Identity storage layout (DreamLab convention)

Agent identity is stored as **two artefacts in the pod-git root**, the layout
`create-agent` inspired (create-agent itself takes `--privkey` on the CLI and
writes the doc to stdout; the file/git-config layout is a DreamLab convention,
additive to the existing `identity.env`, changing no key bytes):

- `git config nostr.privkey <hex>` — the BIP-340 private key (hex), set in the
  pod-repo's git config.
- `<pod-git-root>/agent.did.json` — the canonical §1 Multikey DID document.

The pod **is** the git repository these two artefacts live in the root of (see
ADR-124 §"git surfaces"). **NEW work item** — `sovereign-bootstrap.py` currently
writes the 2019-shape doc to `did-nostr.json` and identity to `identity.env`; no
`agent.did.json` and no `git config nostr.privkey` exist (the cited
`write_agent_repo_identity()` / `build_did_document()` functions do not exist).
The §1 doc + the `git config nostr.privkey` write are net-new (see §8.2).

## 4. VerifiedSkill ↔ `aam skill sign` alignment

Our `VerifiedSkill` URN — `urn:agentbox:skill:<scope>:<name>:v<n>`, `<scope>` =
the 64-char x-only hex pubkey — is the **functional analogue** of create-agent's
`aam skill sign` (a Schnorr-signed JCS envelope over the agent's nostr key).
Alignment, not adoption-of-his-envelope:

- **Envelope** (align): wrap the VerifiedSkill record in a Schnorr-signed JCS
  envelope under the agent's `nostr.privkey`; the attester is
  `owner_did = did:nostr:<hex>`.
- **Index** (keep): the URN `urn:agentbox:skill:<scope>:<name>:v<n>` stays the
  internal index *inside* the envelope. It is a minted URN kind
  (`management-api/lib/uris.js`, kind `skill`) and is NOT migrated.

The signed envelope is additive (greenfield); the URN is unchanged. No identity,
key, ACL, or URN migration.

## 5. Hard invariants (I1–I4) — a change that violates ANY of these is WRONG; flag, do not ship

- **I1 — identity string unchanged.** Identity is the BIP-340 x-only (even-y)
  hex pubkey. The `did:nostr:<hex>` STRING is **unchanged**. No
  identity/npub/URN/ACL/pod/payment migration is implied or permitted by this
  ADR. Enforce: the `did:nostr:<hex>` body before == after, byte-for-byte.

- **I2 — multibase is the same key re-encoded.** `publicKeyMultibase` MUST equal
  the literal `"fe70102"` + the same 32-byte x-only hex (parity `02` because
  even-y). Regex `^fe70102[0-9a-f]{64}$`, length exactly 71. It round-trips to
  the identical pubkey; **no key bytes change**. Enforce:
  `publicKeyMultibase[7:] == id["did:nostr:".len:]` and
  `parse_multibase_schnorr(publicKeyMultibase).to_hex() == id-body`.

- **I3 — auth never reads the VM.** Auth = NIP-98 Schnorr verification against
  the **RAW pubkey in the signed event** — it MUST NOT read the DID-doc
  `verificationMethod` / `publicKeyMultibase` / `publicKeyHex`. Re-encoding the
  VM cannot touch the auth path. Enforce: the auth gate constructs
  `did:nostr:<verified-event-pubkey>`; it never decodes a multibase back to a key
  for an auth decision. (See §7 for the one auth-ADJACENT site that must stay off
  the auth path.)

- **I4 — ADR-074 §D1 stays.** ADR-074 §D1 (x-only hex = canonical identity,
  regex `^[0-9a-f]{64}$`) is **retained**. Only ADR-074 §D2 (the 2019
  `publicKeyHex` document shape, with §D3/§D4/§D13 where they pin the 2019
  multibase/tier/anti-drift) is superseded.

CI gates (every DID-doc conformance harness) MUST assert: (1)
`^fe70102[0-9a-f]{64}$` on `publicKeyMultibase`; (2) length == 71; (3) reject the
67-char missing-parity `fe701<x>` form; (4) reject any uppercase hex under `f`;
(5) reject `SchnorrSecp256k1VerificationKey2019` / `publicKeyHex` / `z…` base58 /
`secp256k1-2019` context as **invalid** (polarity inverted — these are now the
must-reject negative vectors); (6) `publicKeyMultibase[7:] == doc.id-body`;
(7) retain the §D1 negative vectors (uppercase-hex DID id; controller≠id).

## 6. Migration note

**DID documents are regenerated; identities and keys are unchanged.** There is
**no identity/npub/URN/ACL/pod/payment migration.** The migration surface is
exactly: (a) the rendered DID-document **bytes** (re-emitted on next publish from
the converged emitter), and (b) conformance **fixtures/checksums** (re-baselined
to `fe70102` with the old shapes inverted to negative vectors). No agent has to
re-key, re-register, or re-acquire a pod, ACL, or payment relationship. Existing
`did:nostr:<hex>` strings, npubs, URNs, WACs, and pod paths are all I1-stable.

## 7. Drift assessment (port lineage: JSS → solid-pod-rs → agentbox)

The Solid infra was ported JSS → solid-pod-rs → agentbox, so the
WebID/DID-doc/NIP-98/.well-known surfaces are highly conformant already.
Divergence from the create-agent/did-nostr-CG target is **drift to correct**, not
a redesign. Per-layer status:

| Layer | State | Action |
|---|---|---|
| `did:nostr:<hex>` identity string (ADR-074 §D1) | conformant | **no change** (I1/I4) |
| NIP-98 auth path (raw event pubkey) | conformant — never reads the VM | **no change** (I3) |
| `.well-known` / WebID / Solid pod scaffolding | conformant | **no change** (additive `agent.did.json` only) |
| solid-pod-rs `did_nostr_types::render_did_document` | **AT TARGET** (the single source of truth) | **no change** |
| agentbox `s04-did.js` | **DRIFT (full 2019 shape)** — emits `secp256k1-2019/v1` context, `SchnorrSecp256k1VerificationKey2019`+`publicKeyHex`, `#schnorr-pubkey`, no top-level `DIDNostr`, default-populated `service` | **CORRECT** — rewrite `encode()` to §1 (two-context, top-level `DIDNostr`, single `Multikey` VM, `publicKeyMultibase:"fe70102"+hex`, `#key1`, relative `["#key1"]`, `service:[]`; the `pod`/`relay` services become an explicit gated extension, not default) |
| agentbox `sovereign-bootstrap.py` inline DID-doc block | **DRIFT (full 2019 shape)** — emits `secp256k1-2019/v1`, `…2019`+`publicKeyHex`, `#key-0`; writes `did-nostr.json` not `agent.did.json`; `build_did_document()`/`write_agent_repo_identity()` do not exist | **CORRECT** — rewrite the inline block to §1; **NEW** — add a writer for `<pod-git-root>/agent.did.json` + `git config nostr.privkey` |
| solid-pod-rs `interop::did_nostr_document` happy path | **AT TARGET** (delegates to canonical renderer) | **no change** |
| solid-pod-rs `interop::did_nostr_document` **malformed-hex fallback** | **DRIFT** — emits a `Multikey` VM with NO `publicKeyMultibase` (keyless VM) | **CORRECT** — reject malformed hex rather than emit a keyless VM |
| solid-pod-rs `webid.rs` WebID-card VM | **DRIFT** — uses `feb<hex>` (`f`+`eb`=bip340-pub `0xeb`), a different multicodec from the DID doc's `fe70102` | **ALIGN** — re-encode the WebID-card VM to `fe70102`, or formally bless `feb` as the WebID-side standard (one or the other, documented) |
| solid-pod-rs `auth/nip98.rs try_elevate` needle `feb<hex>` | **I3 TRAP** — the only site that parses a `publicKeyMultibase` (WebID-elevation convenience) | **co-change with the WebID drift; MUST stay off the auth path** (it runs after the pubkey is already cryptographically verified; a mismatch degrades elevation to `urn:nip98:<pubkey>`, never auth) |
| forum `nostr-bbs-core/src/did.rs` renderer | **converged once upstream lands** — delegates 100% to `solid_pod_rs::did_nostr_types`; its tests/fixtures/lint already encode the target | **no change** (inherits the upstream encoder; its 4 ADR-125 assertions go green) |
| forum `contexts/did-v1.jsonld` + `contexts.rs:37` served IRI | **DRIFT** — defines `SchnorrSecp256k1VerificationKey2019` + `publicKeyHex`; serves `secp256k1-2019/v1` | **CORRECT** — drop the 2019 term + `publicKeyHex`; serve `https://w3id.org/nostr/context` |
| VisionClaw runtime (uri/, web_contract/, solid_proxy_handler) | conformant — DID-string / raw-pubkey only; delegates DID-doc to upstream | **no change** |
| VisionClaw / forum / dreamlab-ai-website conformance fixtures + checksums | **DRIFT** — hard-code the 2019 shape | **CORRECT** — re-baseline to `fe70102`, invert old shapes to negatives, recompute checksums |
| dreamlab-ai-website | doc/config-only, already converged in ADR-027/028 prose | **no change** |

### The single VM-parsing site in the whole ecosystem (the I3 watch-point)

`solid-pod-rs auth/nip98.rs:538-555 try_elevate` (feature `lws-cid`) reads a
**profile's** `verificationMethod` (`publicKeyMultibase` needle `feb<hex>`, and
legacy `publicKeyHex`) to map `urn:nip98:<pubkey>` → WebID. This is a WebID
**elevation convenience**, run **after** the pubkey is already Schnorr-verified —
it is NOT the auth gate. It must (a) keep matching the WebID card's chosen
encoding (co-change with the `webid.rs` decision), and (b) NEVER be repointed at
the DID-doc VM for the auth decision. Re-encoding the DID-doc VM (this ADR)
cannot touch it.

## 8. Per-repo, file-level change list

`cargo check` is used in-loop for all Rust (it passes on both substrate crates at
baseline; full rebuild on operator demand).

### 8.1 solid-pod-rs — the single source of truth (3 drifts to correct, 1 greenfield)

| Work item | File / lines | Type | I-impact |
|---|---|---|---|
| Renderer at target | `crates/solid-pod-rs/src/did_nostr_types.rs:139-158` `render_did_document` | REUSE — no change | I1/I2/I4 ✓ |
| `MULTIKEY_PREFIX`/`format_multibase_schnorr`/`parse_multibase_schnorr` | `did_nostr_types.rs:218-274` | REUSE — no change | I2 ✓ |
| **D-drift-1**: malformed-hex fallback emits keyless VM | `interop.rs:341-367` (fallback branch of `did_nostr_document`) | CORRECT (small) | reject malformed hex (return canonical doc only for valid 64-hex) OR emit `service:[]` with `verificationMethod:[]` — NEVER a `Multikey` VM lacking `publicKeyMultibase`. I2 (a keyless VM is an I2 hole). |
| **D-drift-2**: WebID-card VM uses `feb` not `fe70102` | `webid.rs:96` (`format!("feb{pubkey}")`) + ctx `:87` | ALIGN | re-encode to `format!("fe70102{pubkey}")` so DID doc and WebID card agree; OR add a doc comment formally blessing `feb`=bip340-pub as the WebID-side standard. Pick one. No key bytes change (I2). |
| **D-drift-3 (I3 trap)**: `try_elevate` needle hard-codes `feb` | `auth/nip98.rs:543` (`format!("feb{pubkey_hex}")`) | ALIGN — co-change with D-drift-2 | if D-drift-2 re-encodes to `fe70102`, change the needle in lockstep (or accept both forms). MUST stay off the auth path (I3). |
| Auth gate (raw-pubkey Schnorr) | `auth/nip98.rs verify_at*` (`:130`, `:316-344`) | REUSE — do NOT touch | I3 ✓ |
| Resolvers (`id` + `alsoKnownAs` only) | `nostr/resolver.rs:199-204`, `interop.rs:371-376` | REUSE — no change | I1 ✓ |
| **G-greenfield**: `agent.did.json` + `git config nostr.privkey` at provisioning | `key_provisioning.rs` (currently writes `/private/privkey.jsonld`, no root DID doc) | NEW (additive) | emit the canonical §1 doc to `<pod-git-root>/agent.did.json` + store privkey in `git config nostr.privkey`. Must NOT rewrite npub/URN/ACL/pod layout (I1). |
| Land the cited ADRs | `crates/solid-pod-rs/docs/adr/` (only ADR-059 present) | DOC | land this ADR + the ADR-124 build-out so `did_nostr_types.rs:111` / `interop.rs:309-315` citations are first-class. |

**Verification:** `cargo check -p solid-pod-rs -p solid-pod-rs-nostr` after the
D-fixes (passes at baseline). D-drift-2/3 cross the `lws-cid` feature gate — full
`cargo build` with `did-nostr`/`lws-cid`/`nip98-schnorr` on operator demand.

### 8.2 agentbox — already at target (no change) + the VerifiedSkill envelope

| Work item | File | Type | Notes |
|---|---|---|---|
| S4 emitter | `management-api/middleware/linked-data/surfaces/s04-did.js` | **CORRECT** | emits the full 2019 shape (`secp256k1-2019/v1`, `SchnorrSecp256k1VerificationKey2019`+`publicKeyHex`, `#schnorr-pubkey`, no top-level `DIDNostr`, default-populated `service`). Rewrite `encode()` to §1; `pod`/`relay` services become a gated extension, not default. |
| Bootstrap DID writer | `scripts/sovereign-bootstrap.py` inline DID-doc block (L228-255) | **CORRECT** | emits the 2019 shape (`secp256k1-2019/v1`, `…2019`, `publicKeyHex`, `#key-0`). Rewrite the inline block to §1. `build_did_document()` does not exist — the doc is inline. |
| Identity layout | `scripts/sovereign-bootstrap.py` (new writer) | **NEW** | sovereign-bootstrap writes `did-nostr.json` + `identity.env` today; no `agent.did.json`, no `git config nostr.privkey`, and `write_agent_repo_identity()` does not exist. Add a writer for `<pod-git-root>/agent.did.json` (canonical §1) + `git config nostr.privkey <hex>` in the pod-git root. |
| Auth path | `management-api/lib/agent-event-auth.js:46-76`; `routes/pod-git.js:236-239` | REUSE — do NOT touch | constructs `did:nostr:${verified.pubkey}`; never reads VM (I3 ✓). |
| Conformance corpus | `tests/contract/upstream_vectors/did-doc-conformance.json` + fixtures | **CORRECT** | corpus is the 2019 shape (0 × `fe70102`; 21 × `publicKeyHex`/`SchnorrSecp256k1VerificationKey2019`/`secp256k1-2019`). Re-baseline to `fe70102`; invert the 2019/`publicKeyHex`/`z…` vectors to must-reject negatives. |
| VerifiedSkill URN kind | `management-api/lib/uris.js` (kind `skill`) | REUSE — no change | URN stays the internal index. |
| **VerifiedSkill signed envelope** (`aam skill sign` analogue, §4) | `mcp/voyager/verify-and-store.py` | NEW (greenfield, additive) | Schnorr-signed JCS envelope under the nostr key; `owner_did=did:nostr:<hex>` attester; URN kept inside. No migration. |

**Verification:** sovereign test suite (baseline green); contract harness
(`tests/contract/linked-data/surfaces.contract.spec.js` S4 case) for any fixture
touched.

### 8.3 nostr-rust-forum — converges when upstream lands (drift in context only)

| Work item | File | Type | Notes |
|---|---|---|---|
| Renderer | `crates/nostr-bbs-core/src/did.rs` | REUSE — no change | delegates 100% to `solid_pod_rs::did_nostr_types`; flips to target when 8.1 lands. Its 4 currently-failing ADR-125 tests (`tier1_*`, `multibase_is_deterministic_and_canonical_multikey`) go green. |
| Pod-worker re-export + emission | `nostr-bbs-pod-worker/src/did.rs`, `lib.rs:300-488` | REUSE — no change | pure pass-through; inherits the fix. |
| **JSON-LD context drift** | `nostr-bbs-pod-worker/contexts/did-v1.jsonld:7-13` | CORRECT | drop `SchnorrSecp256k1VerificationKey2019` term + `publicKeyHex`; map the target IRIs (`w3id.org/did`, `w3id.org/nostr/context`). |
| **Served-IRI allow-list drift** | `nostr-bbs-pod-worker/src/contexts.rs:37` | CORRECT | serve `https://w3id.org/nostr/context` (currently `secp256k1-2019/v1`). |
| Auth path | `nostr-bbs-core/src/nip98.rs`, `event.rs`, `signer.rs`, `keys.rs` | REUSE — do NOT touch | raw x-only Schnorr; never reads VM (I3 ✓). |
| Conformance fixture + lint | `tests/fixtures/did-doc-conformance.json`, `scripts/anti-drift-lint.sh` | REUSE — no change | already the target + the `^fe70102[0-9a-f]{64}$` guardrail. |

**Cross-layer coupling to watch:** `did.rs` `multibase_matches_upstream`
currently passes only because both forum and upstream emit the wrong `z` form; it
flips the moment one side moves. Move upstream (8.1) and forum together — bump the
`solid-pod-rs` dependency in `nostr-rust-forum/Cargo.toml`, do not inline a second
encoder.

**Verification:** `cargo test -p nostr-bbs-core --lib did::` (4 FAIL at baseline
against the un-converged upstream → all PASS after 8.1 lands and the dep bumps).

### 8.4 VisionClaw (this project repo) — runtime no-change; fixtures/ADRs re-baseline

| Work item | File / lines | Type | Notes |
|---|---|---|---|
| URI minter | `src/uri/mod.rs` (`did:nostr:<64-hex>`, `is_pubkey_hex` `^[0-9a-f]{64}$`) | REUSE — no change | I1/I4 ✓; never touches the doc shape. |
| web_contract identity refs | `src/web_contract/{mod,state,ledger,reducer,trail,ritual}.rs` | REUSE — no change | `did:nostr:<hex>` opaque; "never parsed (I1)". |
| DID-doc delegation | `src/handlers/solid_proxy_handler.rs:1737` (`did_nostr_document`) | REUSE — no change | shape-agnostic; inherits the 8.1 encoder. |
| Auth/runtime | `src/services/nostr_identity_verifier.rs`, `src/utils/nip98.rs` | REUSE — do NOT touch | raw `XOnlyPublicKey` Schnorr; never reads VM (I3 ✓). |
| **Conformance fixture** | `docs/specs/fixtures/did-doc-conformance.json` | CORRECT | `@context`→`["https://w3id.org/did","https://w3id.org/nostr/context"]`; `type`→`Multikey` + top-level `DIDNostr`; **remove** `publicKeyHex`; multibase `z…`→`"fe70102"+<same hex>`; fragment `#key-0`→`#key1`; absolute auth/assertion → `["#key1"]`; **invert** the `negative-missing-secp256k1-context`/`stale-suite-*` vectors (2019/`z`/`publicKeyHex` are now the INVALID shapes); KEEP `negative-uppercase-hex-id` + `negative-mismatched-controller`; keep `vector_count`. |
| **Conformance schema** | `docs/specs/fixtures/schemas/did-doc-conformance.schema.json` | CORRECT | add `"type"` to document `required`; pin `verificationMethod[0]` to `Multikey` + `^fe70102` prefix. |
| **Checksums** | `docs/specs/fixtures/CHECKSUMS.txt` (+ COVERAGE_MATRIX/UPSTREAM_PINS/README refs) | CORRECT | recompute SHA-256 after the two edits (cross-repo: the agentbox mirror `all_fixtures.test.js` consumes this checksum — coordinate the two-repo edit). |
| **ADR-074 source** | `docs/adr/ADR-074-cross-system-did-nostr-canonicalisation.md` | DOC | mark §D2/§D3/§D4/§D13 SUPERSEDED by this ADR; **preserve §D1 verbatim** (I4). |

**Verification:** `cargo check -p visionclaw-server --lib` (passes at baseline,
warnings only). Fixture/checksum/ADR edits need no compilation (the fixture is
not loaded by any in-tree Rust test).

### 8.5 dreamlab-ai-website — no change

Doc/config-only; ADR-027/028 prose already carries the converged §1 form. Every
`did:nostr` occurrence is a raw-hex identifier string (trust list, SQL key, ACL
`@id`, env) — I1-stable. No DID-doc construction/parsing in-repo. NIP-98 is
token-creation only (I3-safe).

## 9. Consequences

**Positive:** byte-conformance with the did-nostr CG / create-agent reference; one
canonical form; the `02`-parity / 71-char framing is CI-policed, closing the
latent missing-parity ship-bug.
**Negative:** the solid-pod-rs encoder is the single gating dependency — the forum
flips green only when its `solid-pod-rs` dep includes the (already-landed)
`render_did_document`; coordinate the dep bump.
**Neutral:** external Nostr clients are unaffected — they verify `event.pubkey`,
not the DID document.

## References
- ADR-074 — Cross-System did:nostr canonicalisation (§D1 retained, §D2 superseded)
- agentbox ADR-033; project ADR-125 (per-repo enactments — in force)
- did-nostr Community Group spec — https://nostrcg.github.io/did-nostr
- melvincarvalho/create-agent — `index.js` (reference emitter)
- W3C DID Core; W3C Multikey / Multibase / Multicodec; BIP-340
