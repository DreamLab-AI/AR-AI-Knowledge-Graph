# ADR-125 — DID:Nostr Multikey Convergence (Supersedes ADR-074 §D2, §D3, §D4, §D13 only)

| Field | Value |
|-------|-------|
| Status | Accepted (2026-06-15) |
| Supersedes | **ADR-074 §D2, §D3, §D4, §D13 ONLY** (the 2019-suite `publicKeyHex` doc shape, multibase encoding, Tier-1/Tier-3 split, and anti-drift CI assertions). **ADR-074 §D1 STAYS.** All other ADR-074 sections (D5–D12, delegation grammar, kind-30033, WAC) are unchanged. |
| Keeps intact | ADR-074 §D1 (x-only hex = canonical identity), §D5–D12; PRD-010 G1/G5/G6; all NIP-98 / NIP-26 / NIP-42 auth paths |
| Drives | PRD-010 P1 (DID convergence), re-converges forum / agentbox / VisionClaw / solid-pod-rs / dreamlab-ai-website |
| Ground truth | `melvincarvalho/create-agent` `index.js`; did:nostr CG spec `nostrcg.github.io/did-nostr`; W3C `did-core`; Multikey / Multicodec / Multibase specs; BIP-340 |
| Companion | ADR-074 (parent), ADR-076 (upstream `nostr` crate absorption), ADR-124 build-out (companion plan, web-contract substrate) |

## 1. Context

ADR-074 §D2 mandated `SchnorrSecp256k1VerificationKey2019` as the single `verificationMethod.type` across forum, agentbox, VisionClaw, and `solid-pod-rs` (it resolved the C3 three-suite split: `…2019` vs `NostrSchnorrKey2024` vs the non-existent `…2022`). That decision converged the *drift*, but it converged it on the **wrong target**: a deprecated 2019 cryptosuite shape that the canonical did:nostr emitter ecosystem (`melvincarvalho/create-agent`, the did:nostr CG spec) does **not** use.

The canonical did:nostr DID-document is now the **`DIDNostr` / `Multikey` / `publicKeyMultibase` "fe70102…"** single form. There is no dual-publish, no 2019 suite, no `publicKeyHex` field, no `secp256k1-2019/v1` context, no Tier-1/Tier-3 split.

**Lineage observation (drift, not redesign).** The Solid infrastructure was ported JSS (JavaScriptSolidServer) → `solid-pod-rs` → agentbox. The WebID / DID-Document / NIP-98 / `.well-known` surfaces are therefore *already highly conformant* to the did:nostr + Solid specs. Every divergence from the create-agent / did-nostr-CG target is treated as **DRIFT to correct**, not a new design. The identity primitive, key derivation, and auth path are unchanged; only the **presentation shape of the DID document** moves.

**The single non-trivial mechanical fact** is the multibase encoding (I2). It is verified byte-for-byte below.

## 2. The canonical target document (THE single form — do NOT dual-publish)

This is the **only** DID-document `did:nostr:<hex>` resolves to. Drop the 2019 suite. Drop `publicKeyHex`. Drop the Tier-1/Tier-3 distinction. Drop `alsoKnownAs`-as-primary (it moves to a `service` entry per §2.3).

```jsonld
{
  "@context": [
    "https://w3id.org/did",
    "https://w3id.org/nostr/context"
  ],
  "id":   "did:nostr:<hex>",
  "type": "DIDNostr",
  "verificationMethod": [{
    "id":                 "did:nostr:<hex>#key1",
    "type":               "Multikey",
    "controller":         "did:nostr:<hex>",
    "publicKeyMultibase": "fe70102<hex>"
  }],
  "authentication":  ["#key1"],
  "assertionMethod": ["#key1"],
  "service": []
}
```

Where `<hex>` is the **same 64-char lowercase BIP-340 x-only (even-y) hex pubkey** in every position (it is the identity per ADR-074 §D1, unchanged).

### 2.1 The `fe70102<hex>` multibase string (I2 ground truth — verified)

`publicKeyMultibase` MUST equal the **literal string** `"fe70102"` concatenated with the **same 32-byte x-only hex** that appears in `did:nostr:<hex>`. Decomposition (each segment verified by round-trip):

| Segment | Bytes / chars | Meaning |
|---|---|---|
| `f` | multibase prefix | **base16** (lowercase hex) multibase code |
| `e701` | `0xe7 0x01` | **varint** of multicodec `0xe7` = `secp256k1-pub`. `varint(0xe7)` = `0xe7 0x01` because `0xe7 ≥ 0x80` → continuation byte `0xe7` then high byte `0x01`. |
| `02` | `0x02` | compressed-point **even-y parity prefix** (BIP-340 keys are always even-y, so parity is always `02`, never `03`) |
| `<hex>` | 64 hex chars | the **32-byte x-only pubkey hex** — byte-identical to the `did:nostr:<hex>` body |

Decoding `"f" + "e70102" + <hex>`: drop `f`, hex-decode the rest → `0xe7 0x01` (codec) ‖ `0x02 ‖ X` (33-byte compressed secp256k1 point). Lifting the even-y point recovers the **identical** key bytes. **No key bytes change. The `did:nostr:<hex>` string is unchanged.** This is the entire content of I2.

Concretely, for the operator identity `6407eed80e2a8646e41a5ddba0ae6619425fc54af40e2b30482b9623c682425a`:
- `id` = `did:nostr:6407eed80e2a8646e41a5ddba0ae6619425fc54af40e2b30482b9623c682425a`
- `publicKeyMultibase` = `fe701026407eed80e2a8646e41a5ddba0ae6619425fc54af40e2b30482b9623c682425a`

(Note the 7-char `fe70102` prefix immediately followed by the same 64 hex chars; the multibase string is 71 chars total.)

### 2.2 What is DROPPED versus ADR-074 §D2/§D3/§D4

| Dropped (superseded) | Replaced by |
|---|---|
| `@context: ["https://www.w3.org/ns/did/v1", "https://w3id.org/security/suites/secp256k1-2019/v1"]` | `["https://w3id.org/did", "https://w3id.org/nostr/context"]` |
| `type: "SchnorrSecp256k1VerificationKey2019"` | `type: "Multikey"` |
| `publicKeyHex: "<hex>"` field | **removed entirely** (the key now lives only in `publicKeyMultibase`) |
| `publicKeyMultibase: "z<base58btc(0xe7 0x01 ‖ pk_32)>"` (§D3) | `publicKeyMultibase: "fe70102<hex>"` (base16/`f`, with explicit `02` parity byte) |
| fragment `#key-0` | fragment `#key1` |
| `authentication`/`assertionMethod` = `["did:nostr:<hex>#key-0"]` (absolute) | `["#key1"]` (relative fragment) |
| Tier-1 vs Tier-3 split (§D4) | **single canonical form**; no tiers |
| top-level `type` absent | `type: "DIDNostr"` (required) |

### 2.3 `alsoKnownAs` and `service` handling (agentbox extension reconciliation)

The target shows `service: []`. ADR-074 §D2 carried `alsoKnownAs: ["<webid_url>"]` and four `service` entries (SolidStorage / NostrRelay / SolidWebID / DIDNostrMesh), and `sovereign-bootstrap.py` adds the pod profile to `alsoKnownAs`.

Decision (not invariant-bound): the **WebID binding moves into a `service` entry** rather than top-level `alsoKnownAs`, keeping the document to the two-context canonical form:

```jsonld
"service": [
  { "id": "did:nostr:<hex>#webid", "type": "SolidWebID", "serviceEndpoint": "<webid_url>" }
]
```

The existing `solid-pod`, `nostr-relay`, and `mesh` service entries from ADR-074 §D2 remain permitted in the `service[]` array (they were always optional, and consumers tolerate absence — ADR-074 §D2 closing paragraph). `service: []` is the minimal/Tier-1-equivalent form; populated `service[]` is the production form. **This is the only place the document carries optional supersets; the `@context`, `type`, and `verificationMethod` block are fixed.** This does NOT reintroduce a tier split — it is one document with an optionally-populated `service[]`.

## 3. Decision — replacement clauses for ADR-074

### D2′ (supersedes D2) — Canonical DID Document shape

The single canonical `did:nostr` DID document is the form in §2. All emitters produce exactly this shape. There is no dual-publish and no second shape.

### D3′ (supersedes D3) — Multibase encoding

`publicKeyMultibase` = `"fe70102" + <x-only-hex>` (base16 multibase `f`, secp256k1-pub varint `e701`, even-y parity `02`, then the 32-byte x-only hex), per §2.1. It round-trips to the identical pubkey (I2).

### D4′ (supersedes D4) — No tier split

There is one document. `service[]` MAY be empty (minimal) or populated (production). `@context`, `type`, `id`, `verificationMethod`, `authentication`, `assertionMethod` are fixed and always present.

### D13′ (supersedes D13) — Anti-drift CI assertions (POLARITY INVERTED)

Each repo's CI MUST assert, on every build:
- `doc.type == "DIDNostr"` (top-level).
- `doc.verificationMethod[0].type == "Multikey"`.
- `doc.verificationMethod[0].publicKeyMultibase` starts with `"fe70102"` AND equals `"fe70102" + doc.id["did:nostr:".len ..]` (the multibase body is the DID body — I2 round-trip in CI).
- `doc.verificationMethod[0]` does **NOT** contain a `publicKeyHex` field.
- `doc["@context"] == ["https://w3id.org/did", "https://w3id.org/nostr/context"]` (exact order).
- The 2019 suite, the `…2022`/`…2024` suites, `secp256k1-2019/v1` context, and base58btc (`z…`) multibase are **rejected as stale** (the previous anti-drift rule that *required* the 2019 suite is deleted and inverted).
- Pubkey form unchanged: `^[0-9a-f]{64}$` (ADR-074 §D1, retained).

### D1 (UNCHANGED — explicitly retained, I4)

ADR-074 §D1 stays verbatim: identity is the 64-char lowercase BIP-340 x-only hex; `did:nostr:<64-hex>`; npub is wire-only; WAC ACL agent IRI is lowercased `did:nostr:<hex>`. No change.

## 4. Hard invariants (I1–I4) — a change violating ANY of these is WRONG; flag, do not ship

### I1 — Identity is the BIP-340 x-only (even-y) hex; the `did:nostr:<hex>` STRING is UNCHANGED
The DID string, npub, URN (`urn:agentbox:skill:…`), WAC ACL agent IRIs, pod paths, payment rails, and all identity registries are **untouched**. There is **no identity / npub / URN / ACL / pod / payment migration**. Only the *bytes inside the DID document body* are re-encoded.
- **Verification:** `did:nostr:<hex>` is minted from the verified x-only hex (`did = did:nostr:{x_only_pubkey_hex}`); the convergence changes no minting site.

### I2 — `publicKeyMultibase` MUST equal `"fe70102" + <same-32-byte-x-only-hex>`; round-trips to the identical pubkey; no key bytes change
The multibase body is the **same** hex as `did:nostr:<hex>`. Parity is `02` (even-y) always. Decoding recovers `0x02 ‖ X` (33-byte compressed point) which lifts to the identical key (§2.1, verified).
- **WRONG if:** base58btc (`z…`); any parity byte other than `02`; a different/recompressed key; `publicKeyHex` retained; the multibase body differing from the DID body. Any of these violates I2 — flag, do not ship.

### I3 — Auth = NIP-98 Schnorr verification against the RAW pubkey in the event; it MUST NOT read the DID-doc verificationMethod
The auth path verifies `event.pubkey` / `event.verify()` (BIP-340 Schnorr over the event id), never resolving or parsing a DID document. The DID `did:nostr:<hex>` is **derived from** the already-verified event pubkey, not the other way round.
- **Confirmed across the mesh (scout map):** no `.rs`/`.js`/`.ts`/`.py` in any repo parses `verificationMethod`/`publicKeyHex`/`publicKeyMultibase` on the auth path. agentbox `agent-event-auth.js`, `middleware/auth.js`, `services/nostr-pod-bridge` (`ev.pubkey`); solid-pod-rs `auth/nip98.rs:verify_schnorr_signature` (`event.pubkey`); VisionClaw `utils/nip98.rs` + `services/nostr_identity_verifier.rs` (`XOnlyPublicKey::from_slice` then `verify_schnorr`); forum `nostr-bbs-core/src/nip98.rs` (`verify_event`). The two resolvers that *do* deserialize a DID doc (`solid-pod-rs-nostr/src/resolver.rs`, `solid-pod-rs/src/interop.rs`) read **only** `{id, alsoKnownAs}` and discard the VM.
- **Re-encoding the VM cannot reach the auth path.** Re-encoding the VM is therefore invariant-safe by construction.
- **WRONG if:** any code is added that resolves a DID doc to obtain a key for signature verification. The only VM-reading code permitted is the WebID-card *post-auth principal elevation* (`auth/nip98.rs:try_elevate`, `lws-cid` feature) which reads the **WebID profile card** (its own `feb`-prefixed multibase, multicodec `0xeb` bip340-pub), NOT the DID doc and NOT the auth path. That code is untouched by this ADR (see §6 note).

### I4 — ADR-074 §D1 STAYS; only §D2 (and the D3/D4/D13 it implies) is superseded
The x-only hex canonical identity is unchanged. This ADR supersedes exactly the document-shape clauses (D2/D3/D4/D13). D1 and D5–D12 are retained verbatim.

## 5. Key / tagging layout (create-agent alignment)

### 5.1 Agent identity storage — adopt the create-agent layout

The canonical create-agent layout stores the agent identity as:
- **`git config nostr.privkey`** = the 32-byte secret as hex, in the repo's git config.
- **`agent.did.json`** in the repo root = the canonical DID document (§2 shape) for the agent.
- The served copy at `/.well-known/did.json` (or `/.well-known/did/nostr/<hex>.json`) mirrors `agent.did.json`.

agentbox currently writes the secret to `identity.env` (`AGENTBOX_NSEC`, `AGENTBOX_BRIDGE_SK`) and the document to `did-nostr.json`. Adopting the create-agent git-config layout is **greenfield additive** (ADR-124 build-out item; see the companion plan). It does NOT migrate the existing env-var path under I1 (the secret bytes are unchanged; only an additional storage location is offered). The bootstrap emits `agent.did.json` in the §2 shape; the `git config nostr.privkey` write is the build-out item.

### 5.2 VerifiedSkill ↔ `aam skill sign` alignment

create-agent's `aam skill sign` produces a Schnorr-signed skill envelope keyed by the agent's nostr key. Our analogue is the **`VerifiedSkill`** (`urn:agentbox:skill:<scope>:<name>:v<n>`, `ontology_type: ex:VerifiedSkill`, `owner_did: did:nostr:<hex>`):
- The **URN stays the internal index** (no change — I1). It is not a cryptographic field.
- The current `VerifiedSkill` envelope carries no Schnorr signature (the `signature` field in `verify-and-store.py` is a Python function-signature string, not a key — untouched by I2).
- **Alignment (greenfield, ADR-124 build-out):** add a signed envelope mirroring `aam skill sign` — a Schnorr signature over the JCS-canonical skill record under the agent's nostr key, with `owner_did = did:nostr:<hex>` as the attester. The URN remains the index; the signature is the create-agent-aligned attestation. This is additive; it changes no existing identity or key.

## 6. Migration note (DID docs are REGENERATED; identities and keys are UNCHANGED)

**Nothing is migrated except the rendered DID-document bytes.** There is no key rotation, no identity re-mint, no npub/URN/ACL/pod/payment change.

- **Regenerate, do not migrate:** every emitter is changed to render the §2 shape. On next resolution / next bootstrap / next `.well-known` serve, the document is emitted in the new shape from the **same** key. Cached old-shape documents expire by TTL (ADR-074 §D5: `min(cache_max_age, kind30033_d_tag_ttl, 600s)`); no forced cache flush is required because the auth path never consumed the document (I3).
- **Identities/keys unchanged:** the 32-byte secrets, the x-only hex pubkeys, the `did:nostr:<hex>` strings, the npubs, the WAC ACL agent IRIs, the pod filesystem paths, and the payment rails are byte-identical before and after. The `dreamlab-nostr-identity-roster` (11 identities) is untouched.
- **No dual-publish window:** because no consumer reads the VM (I3), there is no compatibility window to honour. Emitters cut over directly to the §2 shape. Old-shape consumers do not exist.
- **Per-repo regeneration:** documents are produced on demand by the emitter; the change is purely in the emitter code (plus its pinned conformance fixtures, schemas, checksums, and docs). See §7.

**Note on the WebID-card `feb` multibase (out of scope, flagged):** the WebID profile card (`solid-pod-rs/src/webid.rs`) emits its own `Multikey` with `publicKeyMultibase: "feb<hex>"` (multicodec `0xeb` = bip340-pub, no parity byte) and fragment `#nostr-key`, and `auth/nip98.rs:try_elevate` compares against `"feb"+pubkey_hex`. This is a **deliberately different multicodec** from the DID-doc target (`e70102` secp256k1-pub + parity). It is the WebID-card encoding, not the DID-doc encoding, and is **NOT changed by this ADR**. A future "unify all multibase encodings" task may reconcile them; until then, do not assume one encoding. (I3-safe: the elevation read is post-auth principal mapping, not signature verification.)

## 7. Per-repo, file-level change list

All edits regenerate the DID-document shape and re-pin the conformance/CI artefacts. The canonical encoder change lives **once** in `solid-pod-rs` (`did_nostr_types.rs`); every downstream that delegates to it inherits the fix. Edits marked **[encoder]** produce key bytes; all others are shape relabel + fixture/checksum/doc re-baseline.

### 7.1 solid-pod-rs (canonical source of truth — `cargo check` in-loop)

| File | Lines | Change |
|---|---|---|
| `crates/solid-pod-rs/src/did_nostr_types.rs` | 109–126 `render_did_document_tier1` | Emit §2 shape: `@context ["https://w3id.org/did","https://w3id.org/nostr/context"]`, add `type:"DIDNostr"`, VM `type:"Multikey"`, drop `publicKeyHex`, `publicKeyMultibase:"fe70102"+hex`, fragment `#key1`, `authentication/assertionMethod ["#key1"]`, `service:[]`. |
| `crates/solid-pod-rs/src/did_nostr_types.rs` | 135–183 `render_did_document_tier3` | Collapse into the single canonical form (no tier split — D4′); `authentication/assertionMethod` → relative `"#key1"`; WebID → `service[]` SolidWebID entry (§2.3). |
| `crates/solid-pod-rs/src/did_nostr_types.rs` **[encoder]** | 192–198 `format_multibase_schnorr` | Replace `'z' + base58btc(0xe7 0x01 ‖ pk)` with `"f" + "e70102" + hex(x_only_pk)` (I2). MUST round-trip to identical key bytes — assert in a unit test. |
| `crates/solid-pod-rs/src/did_nostr_types.rs` | 202–230 `base58_encode` | Dead after the encoder change; remove. |
| `crates/solid-pod-rs/src/did_nostr_types.rs` | 266–407 (tests) | Rewrite to assert §2 shape + I2 round-trip; delete 2019-suite/`z…`/`publicKeyHex` assertions. |
| `crates/solid-pod-rs/src/interop.rs` | 325–354 `did_nostr::did_nostr_document` | Delegates to `render_did_document_tier1` (inherits fix). Rewrite the **inline fallback (340–353)** to the §2 shape, or emit `verificationMethod:[]` when the pubkey does not parse (target has no `publicKeyHex` fallback). Update doc comments 308–321 (ADR-074 D2 → this ADR). |
| `crates/solid-pod-rs/src/interop.rs` | 358–363 `DidNostrDoc` resolver | **No change** ( `{id, alsoKnownAs}` only — I3-safe). |
| `crates/solid-pod-rs/src/auth/nip98.rs` | 316–346 `verify_schnorr_signature`; 83–200 | **DO NOT TOUCH** — I3 guardrail (raw `event.pubkey` Schnorr). |
| `crates/solid-pod-rs/src/auth/nip98.rs` | 516–585 `try_elevate` (line 543 `"feb"+pubkey_hex`) | **No change**; flag only (WebID-card `feb` encoding, §6 note). |
| `crates/solid-pod-rs/src/webid.rs` | 94–114 | **No change**; flag only (WebID-card `Multikey`/`feb`/`#nostr-key`, distinct from DID-doc — §6). |
| `crates/solid-pod-rs-nostr/src/resolver.rs` | 199–204 `DidNostrDoc` | **No change** (`{id, alsoKnownAs}` only). |
| `crates/solid-pod-rs-nostr/src/did.rs` | 15–18 re-export; 20–134 tests | Re-export inherits fix; rewrite duplicated tests to §2 shape. |
| `crates/solid-pod-rs-server/src/lib.rs` | 1450–1464 `handle_well_known_did_nostr` | **No change** (serves whatever the emitter returns; confirm WebID stays in `service[]`). |
| Tests | `tests/did_nostr_resolver.rs:58–81`, `tests/nip05_endpoint_smoke.rs:40` | Rewrite to §2 shape. |

**Verification:** `cargo check -p solid-pod-rs --features did-nostr-types,did-nostr,nip98-schnorr,lws-cid && cargo check -p solid-pod-rs-nostr && cargo check -p solid-pod-rs-server --features did-nostr`, then targeted test rewrite. Full rebuild on operator demand. (No identity/auth code is in the `cargo check` blast radius — the change is confined to `did_nostr_types.rs` + the `interop.rs` fallback; everything else inherits.)

### 7.2 agentbox (JS emitter + Python emitter + conformance fixtures — JS/Python, no cargo)

| File | Lines | Change |
|---|---|---|
| `management-api/middleware/linked-data/surfaces/s04-did.js` | 16–18, 82–89 | `@context` triple/dual → 2-context; add top-level `type:"DIDNostr"`; VM `SchnorrSecp256k1VerificationKey2019`+`publicKeyHex` → `Multikey`+`publicKeyMultibase:"fe70102"+x-only-hex`; fragment `#schnorr-pubkey` → `#key1`; `authentication/assertionMethod` → `["#key1"]`. **Reconcile L42** so the multibase uses the **x-only DID body**, not an arbitrary `pubkeyHex` (the S4 test's `'02'.repeat(33)` 66-byte compressed key path must feed the x-only hex for I2 round-trip). |
| `scripts/sovereign-bootstrap.py` | 228–254 | Same shape change as s04; VM at L244/246 → `Multikey`+`publicKeyMultibase` (uses `x_only_pubkey_hex` at L246 — correct I2 source); fragment `#key-0` → `#key1`; `authentication/assertionMethod` (L249–250) → `["#key1"]`. |
| `scripts/sovereign-bootstrap.py` | 251–253 `alsoKnownAs`; 255 filename | Move pod profile WebID into `service[]` SolidWebID entry (§2.3). Emit `agent.did.json` (repo-root) in addition to `did-nostr.json`; the served `/.well-known/did.json` mirrors it. **Greenfield:** add `git config nostr.privkey` write alongside the existing `identity.env` (§5.1) — additive, no key change. |
| `tests/contract/upstream_vectors/did-doc-conformance.json` | all `valid:true` vectors; `negative-stale-suite-2022/2025` (L104–148); `publicKeyMultibase` `zQ3sh…` (L29, L75) | Rewrite all positive vectors to §2 shape; **fix `zQ3sh…` (base58btc) → `fe70102`+x-only-hex (I2)**; invert the negative-suite polarity (2019/`z…`/`publicKeyHex` are now the *invalid* shapes; new negatives police non-`Multikey`/non-`fe70102`). Keep ≥7 vectors (count gate). |
| `tests/contract/upstream_vectors/fixtures/did-doc-conformance.json` | (duplicate) | Same edits. |
| `tests/contract/upstream_vectors/CHECKSUM.txt` and `CHECKSUMS.txt`; `fixtures/CHECKSUM(S).txt` | L2/L5 etc. | **Recompute SHA-256** after fixture edits (both dirs) or the harness fails on hash mismatch. |
| `schemas/did-doc-conformance.schema.json` | 21 | Add `"type"` to `required`; add `Multikey` + `"fe70102"`-prefix constraint on `verificationMethod[0]`. Recompute its checksum. |
| `tests/contract/upstream_vectors/all_fixtures.test.js` | 50 (`minVectors:7`, `spec:'ADR-074'`) | Keep ≥7 vectors; update `spec` reference to `ADR-125`. |
| `tests/contract/linked-data/surfaces.contract.spec.js` | 56–68 (S4) | Feed an x-only hex (not the 66-byte compressed `'02'.repeat(33)`); assert `type:"DIDNostr"`, VM `Multikey`, `publicKeyMultibase` starts `"fe70102"`, no `publicKeyHex`. |
| `docs/user/solid-pod.md` | 155–165 | Rewrite the example (currently `…2022` + `publicKeyHex` + `did:nostr:npub1…`) to the §2 canonical hex form. |
| `mcp/voyager/verify-and-store.py` | 582, 598, 602, 618, 625 | **No change to URN/envelope under I1/I2.** Greenfield (ADR-124 build-out): add the `aam skill sign`-aligned signed envelope (§5.2); URN stays the index. |
| `management-api/routes/uri-resolver.js`, `routes/well-known.js`, `mcp/servers/nostr-bridge.js`, `agent-event-auth.js`, `middleware/auth.js` | — | **No change** (string/redirect/raw-pubkey only — I3). |

**Verification:** run the contract harness (`tests/contract/upstream_vectors/all_fixtures.test.js` + `surfaces.contract.spec.js`) after fixture + checksum regen.

### 7.3 nostr-rust-forum (thin wrapper over solid-pod-rs — `cargo check`)

| File | Lines | Change |
|---|---|---|
| `crates/nostr-bbs-core/src/did.rs` | 84–91 `render_did_document_tier1`; 16 re-export; 97–148 tier3 | Inherits the encoder/shape fix from upstream `solid_pod_rs::did_nostr_types` once 7.1 lands. Until then, the wrapper must override locally to the §2 shape. `service[]` Tier-3 enrichment stays (target-compatible) with `Multikey` VM. |
| `crates/nostr-bbs-core/src/did.rs` | 154–374 (tests) | Rewrite to §2 shape (`DIDNostr`/`Multikey`/`fe70102`/`#key1`); re-baseline `tier1_superset_of_upstream` after upstream bump. |
| `crates/nostr-bbs-pod-worker/src/did.rs` | 7 re-export; 9–39 tests (L21 `starts_with('z')`) | Logic no change; test L21 → assert `"fe70102"` prefix. |
| `crates/nostr-bbs-auth-worker/src/did.rs` | 20–40 handler; 63–79 tests (L74 `starts_with('z')`) | Handler no change (body inherits); test L74 → `"fe70102"`. |
| `crates/nostr-bbs-pod-worker/contexts/did-v1.jsonld` | 7, 13 | Drop `SchnorrSecp256k1VerificationKey2019`/`publicKeyHex` terms; ensure `DIDNostr`/`Multikey` covered; re-point the `secp256k1-2019/v1` IRI to `https://w3id.org/nostr/context`. |
| `crates/nostr-bbs-core/src/contexts.rs` | 37 | Re-point the `secp256k1-2019/v1` context-IRI map to the new context. |
| `tests/fixtures/did-doc-conformance.json` | all 7 vectors; negatives L103–148; `_meta.upstream_path` | Rewrite to §2 shape; invert negative polarity; re-point `_meta` to ADR-125; update `CHECKSUM(S).txt`. |
| `scripts/anti-drift-lint.sh` | Rule 1 (25–50), Rule 2 (52–76) | **Rewrite to enforce §2 (D13′):** canonical = `Multikey`+`fe70102`+`DIDNostr`+2-context; reject the 2019 suite as stale. This lint currently codifies the drift and **will block the pivot** — highest-risk file. |
| `crates/nostr-bbs-core/src/nip98.rs`, `event.rs`, `keys.rs`, `signer.rs` | — | **DO NOT TOUCH** (raw-pubkey Schnorr — I3/I1). |
| `crates/nostr-bbs-auth-worker/src/pod.rs`, `forum-client/src/auth/passkey.rs` | — | **No change** (DID *string* consumers — I1). |
| `CHANGELOG.md`, `docs/adr/ADR-074-*`, `docs/phase1-impact-assessment.md` | — | Record supersession (point to ADR-125); fix the stale "did.rs Unaffected" line. |

**Verification:** `cargo check -p nostr-bbs-core -p nostr-bbs-pod-worker -p nostr-bbs-auth-worker` after the upstream encoder lands (or the local override is added); run `scripts/anti-drift-lint.sh` to confirm it now passes the §2 shape and fails the 2019 shape.

### 7.4 VisionClaw (doc/fixture only — `cargo check` only at the upstream call site)

VisionClaw does NOT emit the DID-doc body in Rust; the canonical document comes from `solid_pod_rs::interop::did_nostr::did_nostr_document(pubkey, &also_known_as)` (called at `src/handlers/solid_proxy_handler.rs:1737`). The call site is shape-agnostic (serialises whatever the crate returns as `application/did+ld+json`). **Runtime Rust change: none.** Work is doc + fixture + ADR text.

| File | Lines | Change |
|---|---|---|
| `docs/specs/fixtures/did-doc-conformance.json` | 16–58, 63–79 (positive); 84–148 (negatives) | Rewrite positives to §2 shape; `publicKeyMultibase` `zQ3sho…` → `fe70102`+x-only-hex (I2); add top-level `type:"DIDNostr"`, fragment `#key1`; rewrite/delete the stale-suite & missing-secp256k1-context negatives (re-author against `Multikey`/`fe70102`); **keep** `negative-uppercase-hex-id` and `negative-mismatched-controller` (D1/controller negatives survive). Update `_meta.spec`/`vector_count`. |
| `docs/specs/fixtures/schemas/did-doc-conformance.schema.json` | 21, title/$id | Add `"type"` required; re-label ADR-074 D2 → ADR-125. |
| `docs/specs/fixtures/{CHECKSUMS.txt,COVERAGE_MATRIX.md,UPSTREAM_PINS.md,README.md}` | — | Re-sync (ADR-082 fixture-sync web); recompute checksums. **Flag:** the agentbox mirror (`agentbox/tests/contract/upstream_vectors/**`) consumes a checksummed copy — coordinate the two-repo edit or the agentbox `all_fixtures.test.js` CI fails. |
| `docs/adr/ADR-074-cross-system-did-nostr-canonicalisation.md` | 61–89 (D2), 90–104 (D3/D4), 237–244 (D13) | Mark D2/D3/D4/D13 **SUPERSEDED by ADR-125**; replace with the §2 form / `fe70102` multibase / no-tier / inverted anti-drift. **Preserve D1 (46–60) verbatim.** |
| `docs/adr/ADR-124-smart-contract-features-web-contracts.md` | identity refs (27, 46, 146) | Add one line: the `did:nostr` DID-doc shape is now `DIDNostr`/`Multikey` per ADR-125; web-contract identity refs stay consistent. No code change. |
| `src/uri/mod.rs`, `services/nostr_identity_verifier.rs`, `utils/nip98.rs`, `crates/visionclaw-xr-presence/src/types.rs`, `pay_handler.rs`, `broker_inbox_handler.rs`, `agent_events/provenance.rs` | — | **No change** (DID-string / raw-pubkey only — I1/I3). |
| `src/handlers/solid_proxy_handler.rs` | 1690 (`did:web` fallback), 1737 (call site) | **No change.** The `did:web` fallback is a different method (no VM); the `did:nostr` call site is shape-agnostic. |
| narrative docs (`docs/PRD-010-*`, `ddd-mesh-federation-context.md`, `docs/integration-research/05-crypto-gotchas.md`, `docs/ops/solid-pod-rs-runbook.md`, `docs/adr/ADR-077/078/082/086`, `docs/explanation/ecosystem-convergence.md`) | — | Update prose references to the §2 shape opportunistically; non-load-bearing. |

**Verification:** `cargo check -p visionclaw-server --features solid-pod-embed` only to re-confirm the `did_nostr_document(pubkey: &str, also_known_as: &[String]) -> serde_json::Value` signature after the upstream bump. No body logic in this repo.

### 7.5 dreamlab-ai-website (doc/spec only — no cargo, no deployed artefact)

The DID-doc implementation lives in external repos. Work is spec text superseding ADR-074 D2 only.

| File | Lines | Change |
|---|---|---|
| `docs/adr/027-canonical-identity-stack.md` | 48–72 (Tier-1 JSON), 50–53, 58, 60, 64/67 | `@context [did/v1, secp256k1-2019/v1]` + `SchnorrSecp256k1VerificationKey2019` + `publicKeyHex` + `#key-0` → §2 form. Drop the 2019 suite; no dual-publish. |
| `docs/prd/prd-nostr-solid-identity-refactor-v8.0.md` | 747–778 | Same rewrite as 027. |
| `docs/adr/028-solid-pod-rs-agpl-boundary.md` | 125–138 ("v9 alignment" emitting the 2019 suite) | Note supersession: the external `did.rs` emits `Multikey`/`publicKeyMultibase` per ADR-125; the "v9 alignment" was itself drift. |
| `docs/ddd/09-nostr-solid-bridge-context.md` | 253–267 (`DidNostrDocument`), 174–180 (mermaid), 322–327 (invariants), 892 (serialize note) | Spec the VM as `Multikey`/`publicKeyMultibase`; reaffirm invariant 323: `pubkey_hex` round-trips byte-identical (I2). |
| `docs/tranche-1/feature-parity-matrix.md` | 136–138 (P4 "multicodec 0xe701") | Reword to `fe70102` multibase form, parity `02`. |
| `docs/ddd/08-agent-identity-messaging-context.md` | 604, 625 | Cross-ref to §2; shape-neutral. |
| `src/pages/Index.tsx`, `README.md`, `docs/security/AUTHENTICATION.md`, `docs/ddd/01-domain-model.md`, `docs/ddd/05-value-objects.md`, `.nostr-identities.env`, `forum-config/**` | — | **No change** (identifier strings / x-only identity / NIP-98 — I1/I3/I4 already conformant). |

## 8. Consequences

**Positive.** Single canonical did:nostr form ecosystem-wide, matching the create-agent / did-nostr-CG ground truth — external did:nostr clients interoperate. The encoder change is one function in one crate (`did_nostr_types.rs`); everything downstream inherits. No identity/key/auth churn (I1–I4 hold). The fixture/checksum/anti-drift re-baseline is mechanical drift-correction.

**Negative.** Cross-repo fixture sync (ADR-082) requires coordinating the VisionClaw in-tree fixture with the agentbox checksummed mirror or the mirror CI fails. The forum `anti-drift-lint.sh` and the ADR-074 D13 assertions actively codify the old shape and must be inverted in the same change, or they block the pivot.

**Neutral.** No effect on the public Nostr ecosystem (auth is signature-only). No effect on the WebID-card `feb` encoding (deliberately distinct; §6).

## 9. References

- `melvincarvalho/create-agent` `index.js`; did:nostr CG spec `nostrcg.github.io/did-nostr`
- ADR-074 §D1 (retained), §D2/D3/D4/D13 (superseded here); PRD-010 G1/G5/G6, P1
- ADR-076 (upstream `nostr` crate absorption); ADR-082 (fixture-sync web); ADR-081 (key custody)
- ADR-124 (web-contract substrate — companion build-out)
- W3C DID Core; Multibase / Multicodec specs; BIP-340; NIP-19 / NIP-26 / NIP-98
