# ADR-128 Build-Out (canonical) — Adopt gitmark/blocktrails Verbatim as the Single Web-Contract Substrate

> Renumbered from a duplicate **ADR-124** to **ADR-128** on 2026-07-03. This is the
> implementation plan (build-out) for the decision recorded in ADR-124
> (`ADR-124-smart-contract-features-web-contracts.md`).

| Field | Value |
|-------|-------|
| Status | Accepted build-out plan (2026-06-15) |
| Extends | **ADR-124** (`docs/adr/ADR-124-smart-contract-features-web-contracts.md`) — the trust-spectrum / web-contract decision. This record is its implementation plan, not a new decision. |
| Decision (single impl) | **Adopt Melvin Carvalho's `gitmark` / `blocktrails` envelope as THE single web-contract substrate. NO parallel design.** Map the 4-layer web-contract (reducer / state / ledger / trail) and the `validate → anchor → verify` ritual onto the existing solid-pod-rs block-trails / git-marks engine (ADR-059 provenance primitives) and the VisionClaw `src/web_contract/` scaffolding. |
| Identity rail | ADR-074 §D2′ (= agentbox ADR-033 / project ADR-125) `did:nostr` Multikey. **No part of this build-out touches I1–I4.** It adds JSON-LD envelopes + a verifier over the existing trail engine; the `agent_did` it carries is the unchanged `did:nostr:<hex>` string. |
| Git surfaces (LIVE) | Two real, anchor-against git surfaces (§4). Build against them; do **not** stub. |
| Ground truth | `webcontracts.org` / "Melvo Predicts" worldcup pattern: `gitmark.json`, `blocktrails.json`, `verify.js`, `validate-cli.js`, `ship.js`; create-agent `microfed/gitmark.json` (the one byte-verifiable artefact). |

---

## 1. The single-substrate decision (no parallel design)

The ecosystem already implements **three of the four web-contract layers in
production Rust** (solid-pod-rs `mrc20`/`payments`/`provenance`/`trail_store` +
the Bitcoin write-side) and the **whole 4-layer projection scaffolding in
VisionClaw** (`src/web_contract/{reducer,state,ledger,trail,ritual}.rs`, which
compiles clean today). The remaining gaps are: (a) the JSON-LD **envelope shape**
on the solid-pod-rs side, (b) the **engine wiring** behind the VisionClaw
scaffolding traits, (c) the **`verify`/`ship` binaries**, (d) the **seal-closing
check**, and (e) writing the artefacts onto the **real per-user-pod git + the
externally-pullable forum pod**.

We adopt Carvalho's envelope as a **JSON-LD projection/serializer over the
existing Rust types** — we do NOT design a second chain, a second state model, or
a second seal mechanism. The single-use-seal through-line (L0→L1) is the existing
`mrc20::bt_derive_chained_pubkey` UTXO chain; `blocktrails.json` `txo[]` is its
serialization. The 4 layers map 1:1; his artefact names are the canonical on-pod
file names.

**Verbatim discipline (C6/C7):** only `gitmark.json` is byte-verifiable against
the create-agent lineage and is **verbatim** — exactly the **five keys**
`{@id, genesis, nick, package, repository}`, and **nothing else** (NO `@context`,
`@type`, `commit`, or `parent`; parent linkage lives in `blocktrails.json`
`states[]`/`txo[]`). `blocktrails.json`, `verify.js`, `validate-cli.js`, `ship.js`
are **reconstructed per the webcontracts.org reference shape** and must be
labelled as reconstructions, not "verbatim".

## 2. The four layers — Carvalho artefact → our substrate

| Layer | Carvalho artefact | solid-pod-rs engine (existing) | VisionClaw projection (existing) | Status |
|---|---|---|---|---|
| **1 Reducer (Contract)** | `validate.js` + `ledger.js settle()` — pure `validate()` + `transition()`, deterministic, wasm-compiled | engine: `mrc20::jcs` (RFC-8785), `validate_mrc20_state`, `verify_state_link` (SHA-256 hash-chain + seq) | `web_contract::reducer::ContractReducer` trait (validate/transition/replay; integer-only) + `Checks` 3-gate registry — **scaffolded, compiles** | DRIFTED — engine present; the pure trait + wasm↔native byte-parity is the net-new headline risk (R1). |
| **2 State** | `data/*.json`, `pool/pool.json` + `schema/*.schema.json`, WAC-gated | LDP/WAC + `acl:PaymentCondition`/`acl:costSats` (ADR-032 402 grammar); oracle resource ACL-locked to operator `did:nostr` | `web_contract::state::CanonicalState` (`state_hash` = SHA-256 over canonical JSON) — scaffolded | MATCHES — reuse the WAC/402 surface; the canonical-serialisation shim exists. |
| **3 Ledger** | `pool/ledger.json` (`@context https://w3id.org/webledgers`), 1 share = 1000 sats | `payments::WebLedger` credit/debit; multi-ccy `trading.rs`; AMM `x*y=k` | `web_contract::ledger::Ledger` (`SATS_PER_SHARE=1000`, integer sats) — scaffolded | MATCHES (engine) — emit `ledger.json` in the webledgers `@context`. |
| **4 Trail** | `gitmark.json` (5-key verbatim) + `blocktrails.json` (`@type Blocktrail`, `profile gitmark`, BIP-341 single-use-seal chain, `states[]`=commit SHAs, `txo[]`=UTXO chain) | `provenance::GitMark`; `BlockTrailAnchor`; `bt_derive_chained_pubkey`/`bt_address`/`verify_mrc20_anchor`; `trail_store` (`/.well-known/token/{ticker}.json`); `bitcoin_tx`/`mempool` write-side (DONE) | `web_contract::trail::{GitMark (5-key), Blocktrails, TxOut}` — **scaffolded, compiles, well-formed-invariant present** | DRIFTED (envelope) — all data present; emit it in the JSON-LD shape, wire to the real engine. The one capability gap is the seal-closing check (R2). |

**Net: ~85% reuse.** Deliverables are the serializers (`gitmark.json`,
`blocktrails.json`, `ledger.json`), the `verify`/`ship` binaries, the seal-closing
witness check, the engine-wiring behind the VisionClaw traits, and the real-git
wiring.

### 2.1 `gitmark.json` (VERBATIM — five keys, C7)

```jsonld
{ "@id": "gitmark:<commit_sha>:<vout>",
  "genesis": "gitmark:<first-commit-sha>:<vout>",
  "nick": "<short-name>",
  "package": "<pod-relative contract package path>",
  "repository": "./" }
```
Genesis mark: `genesis == @id`. Emitted over solid-pod-rs `provenance::GitMark`
(commit SHA) + the anchoring `vout` from `BlockTrailAnchor`. `genesis`/`nick`/
`package`/`repository` are additive projection fields. **Do NOT add
`@context`/`@type`/`commit`/`parent`.** Mirror type in VisionClaw is
`web_contract::trail::GitMark` (already exactly five keys; `genesis()`/`marked()`
constructors present).

### 2.2 `blocktrails.json` (RECONSTRUCTED reference shape, C6)

```jsonld
{ "@type": "Blocktrail", "profile": "gitmark",
  "chain": "<ticker e.g. tbtc4>",
  "pubkeyBase": "<bt_derive_chained_pubkey base, hex>",
  "states": ["<commit_sha_0>", "<commit_sha_1>", "..."],
  "txo": [ { "txid": "<txid_0>", "vout": 0, "address": "<bech32m P2TR>" },
           { "txid": "<txid_1>", "vout": 0, "address": "<bech32m P2TR>" } ] }
```
`states[]` = commit SHAs (real SHAs in the pod-git repo); `txo[]` = the BIP-341
single-use-seal UTXO chain (`bt_derive_chained_pubkey` + `bt_address`).
`states.len() == txo.len()` (one seal per state — `web_contract::trail::
Blocktrails::is_well_formed`). Mirror type: `web_contract::trail::Blocktrails`.

### 2.3 The deploy ritual (verbatim shape): `edit → validate → commit → git-mark → push; verify`

| Step | Carvalho | Our impl |
|---|---|---|
| **edit** | author edits state JSON | LDP write to the pod (WAC/402-gated) — REUSE |
| **validate** | `validate.js` → `ship.js` → `validate-cli.js` (one schema, 3 gates) | `ContractReducer::validate` + JCS (engine REUSE); the 3-gate `Checks` registry (`web_contract::ritual::Checks` — scaffolded, asserts cross-gate determinism) |
| **commit** | git commit | `ShellGitMarker` commits the pod write, captures the SHA, sets `agent_did` (the ADR-074 §D2′ `did:nostr`) as author (`solid-pod-rs-git/src/mark.rs:145-241`) — REUSE |
| **git-mark** | write `gitmark.json` to repo root | emit §2.1 from the captured SHA + anchor `vout` (NEW small) |
| **anchor** | BIP-341 taproot tx | `bitcoin_tx::anchor_state` + `broadcast_tx` + `MempoolBlockAnchorer::anchor` + `POST /{pod}/_prov/anchor` — DONE |
| **push** | git push | `git push` to the pod-git remote — REUSE |
| **verify** | `verify.js` audit | §2.4 — `web_contract::ritual::verify` (scaffolded) + the solid-pod-rs `verify` binary |

### 2.4 The `verify` audit (reconstructed)

Given a contract package, `verify`:
1. **Recomputes the reducer** — replays `transition()` from `genesis` over the
   recorded event log; asserts stored canonical state hash == replay (JCS
   byte-parity, R1). (`web_contract::ritual::verify` step 1 + `ContractReducer::replay`.)
2. **Replays the ledger** — recomputes `ledger.json` balances from the reducer
   output via the pure `project_ledger` projection; asserts stored == replay.
3. **Asserts git-clean** — working tree matches the last `gitmark.json` commit SHA.
4. **Confirms the trail tip is a confirmed tx** — `verify_mrc20_anchor` on the
   last `txo[]` entry asserts the anchor UTXO exists AND (L1) the prior
   chained-key prevout was spent **exactly once** (the single-use-seal close, R2 —
   the NEW `MempoolLookup` outspend call). VisionClaw consumes this via the
   `AnchorConfirmer` trait (`is_confirmed` / `prevout_spent_once`).
5. **Oracle-anchor-before-deadline** — the oracle resource anchor's
   confirmed-block-time precedes the pick-submission cutoff in `pool.json`, and
   the cutoff was anchored before any entries were accepted.

## 3. Trust model and upgrade seam (honest-or-caught → single-use-seal → trustless)

Adopted verbatim from Carvalho's `AGENTS.md` §8 / ADR-124 §4. Implemented as
`web_contract::ritual::TrustLevel` (scaffolded, with `gate()` hard-refusing L2+):
- **L0** (available): honest-or-caught — public pure reducer + published verifier
  + per-state block-anchor + oracle ACL = operator `did:nostr`. Anchor = notary
  clock (UTXO-exists).
- **L1** (available after the seal-closing check): anchor becomes a true
  single-use seal (spent-exactly-once); m-of-n multisig; Merkle inclusion proofs.
- **L2/L3** (FUTURE, **HARD-REFUSED** by the capability gate until the adaptor-sig
  CET engine is built AND independently audited): RGB/DLC trustless endgame. The
  single-use-seal chain is the upgrade seam — L0→L1 is in-place over the same
  chain; L2→L3 is a layer rewrite.

The trust-level + deadline + currency + cash-out flags are **on-seal immutable
commitments**. A substrate-disablement flag hard-disables
`.swap`/`.pool`/`.withdraw` + cash-out unless trust-level AND owner+legal
Judgment-Broker sign-off authorise.

## 4. Git surfaces — LIVE, build against them (NOT stubs)

Two real git surfaces. The pod **IS** the git repo the Multikey DID doc + key are
committed into; `gitmark.json`/`blocktrails.json` are committed into the **same
pod-git root**, and `states[]` are **real commit SHAs in these repos**.

**(1) Per-user agentbox pods (primary anchor).** Each user's pod is a FULL git
repo (create-agent design). Layout: `agent.did.json` + `git config nostr.privkey`
in the pod-git **root** (written by `sovereign-bootstrap.py`). Init is owned by
solid-pod-rs `alpha.12 GitAutoInit` (gated by `agentbox.toml [sovereign_mesh.git]
.enabled`, which requires `solid_pod=true`); serve/clone/push via
`management-api/routes/pod-git.js` (smart-HTTP, push = NIP-98 owner-only). The
`gitmark`/`blocktrails`/`ledger` artefacts and the `agent.did.json` from
sovereign-bootstrap all live in this root; the deploy ritual's `commit`/`push`
are first-class against it.

**(2) Externally-pullable forum pod (`nostr-rust-forum`).** The forum's pod can be
git-init'd and pulled externally — a real remote-pullable pod repo. Native /
agentbox builds wire the full git backend (`solid_pod_rs_git::GitHttpService`);
the Cloudflare Workers deployment returns a 501 stub (ADR-089 — CF cannot
subprocess `git-http-backend`). The substrate wires onto the **native build + the
external forum pod**, never the CF 501 stub. Cross-remote clone via
`management-api/routes/git-bridge.js` (NIP-98-gated).

## 5. Per-repo, file-level work-item list

Legend: **REUSE** (no change) · **WIRE** (existing + small glue) · **NEW**
(net-new) · **PORT** (port JS → Rust). All inherit the ADR-074 §D2′ `did:nostr`
Multikey identity rail (no I1–I4 impact). `cargo check` in-loop (passes on
`solid-pod-rs` with `did-nostr-types` and on `visionclaw-server --lib` at
baseline). Full rebuild on operator demand.

### 5.1 solid-pod-rs (the engine — primary build-out)

| Work item | File | Type | Notes |
|---|---|---|---|
| `gitmark.json` serializer | `crates/solid-pod-rs/src/provenance.rs` (over `GitMark`) | NEW (small) | Add `genesis`/`nick`/`package`/`repository` to a `GitMarkDoc` projection; emit §2.1, 5 keys, `@id "gitmark:<sha>:<vout>"`. |
| `blocktrails.json` serializer | `provenance.rs` (over `BlockTrailAnchor`) + `solid-pod-rs-server/src/trail_store.rs` | NEW (small) | Emit §2.2. Data all present; envelope projection. Existing `prov_ttl` PROV-O Turtle stays a sibling sidecar. |
| `ledger.json` serializer | `crates/solid-pod-rs/src/payments.rs` (over `WebLedger`) | NEW (small) | `@context https://w3id.org/webledgers`; 1 share = 1000 sats. |
| `ContractReducer` trait | new module `crates/solid-pod-rs/src/reducer.rs` | NEW | `validate`/`transition`; wasm+native; integer-only; dual reducer (scoring + money). **Byte-parity golden test = R1 headline risk.** Plugs into `verify_state_link`. |
| `TrustLevel` + capability gate | new module | NEW | hard pre-condition on `transition()` commit; HARD-REFUSE L2+ until CET engine built+audited; on-seal commitment. |
| Seal-closing-witness check | `mrc20.rs:508/546` + new `MempoolLookup` outspend (`solid-pod-rs-server/src/mempool.rs`) | NEW (small, P1) | extend `verify_mrc20_anchor` from UTXO-exists → prevout spent-exactly-once. **Cheapest/highest-leverage (R2).** |
| `verify` binary (verify.js port) | `crates/solid-pod-rs/src/bin/verify.rs` | PORT | §2.4 end-to-end. |
| 3-gate CHECKS registry (validate-cli port) | new module | PORT | one schema, 3 gates. |
| `ship` ritual command (ship.js core) | `crates/solid-pod-rs/src/bin/ship.rs` | PORT | edit→validate→commit→git-mark→push; calls `ShellGitMarker` then `bitcoin_tx::anchor_state`, pushing to the §4(1) pod-git remote. |
| Per-ledger atomicity (TOCTOU) | `payments.rs:441-442`, `trading.rs:240` | NEW (small) | advisory lock/CAS; collapse `check_replay`+`record_replay` into record-if-absent (R6). |
| Substrate disablement flag | `trading.rs`, `/pay/.*` routes, `bitcoin_tx::anchor_state` cash-out leg | NEW (config gate) | hard-disable `.swap`/`.pool`/`.withdraw`/cash-out unless trust-level AND owner+legal sign-off. |
| Real-git wiring (forum + pods) | `solid-pod-rs-git/src/{init,config,mark}.rs`, `GitHttpService` | REUSE/WIRE | `ShellGitMarker` already commits real SHAs with `agent_did` author; wire `ship` against the §4 surfaces. Native build only (not CF). |
| Bitcoin write-side | `bitcoin_tx::{build_transaction,anchor_state}`, `mempool.rs`, `handlers/prov.rs` | REUSE — DONE | integration-tested. |

**Verification:** `cargo check -p solid-pod-rs -p solid-pod-rs-server -p solid-pod-rs-git` per module; reducer wasm↔native golden byte-parity; `tests/pay_phase4_routes.rs`/`tests/prov_phase5_routes.rs` for the write-side.

### 5.2 VisionClaw (this repo) — wire the engine behind the scaffolding (no parallel chain)

| Work item | File | Type | Notes |
|---|---|---|---|
| 4-layer scaffolding | `src/web_contract/{mod,reducer,state,ledger,trail,ritual}.rs` | REUSE — present, compiles | the projection types + `verify`/`Checks`/`TrustLevel`/`AnchorConfirmer` traits already exist (1292 LOC, clean). |
| `ContractReducer` engine impl | `src/web_contract/reducer.rs` (impl on a concrete contract) | NEW | implement the trait for the first reference contract (worldcup parimutuel); integer-only; byte-parity with the solid-pod-rs reducer (R1). |
| `AnchorConfirmer` engine impl | `src/web_contract/` + `src/handlers/pay_handler.rs:38` | WIRE | back `is_confirmed`/`prevout_spent_once` with the solid-pod-rs `verify_mrc20_anchor` + `MempoolLookup` (the §5.1 seal-closing check). |
| `gitmark.json`/`blocktrails.json` consumption | `src/agent_events/provenance.rs:51-52`, `adapters/sqlite_enrichment_repository.rs:73,153` | WIRE | these mention trail/blocktrails but emit no envelope; consume the solid-pod-rs serializers (identity-string-only — no I1–I4 impact). |
| Broker decisions on-trail | `src/handlers/broker_inbox_handler.rs`, `src/agent_events/*` | WIRE | governance overrides/refunds = Judgment Broker `BrokerCase` via ACSP (ADR-041/110), recorded PROV-O, anchored. No bespoke REST admin surface. |
| DID-doc delegation | `src/handlers/solid_proxy_handler.rs:1737` | REUSE | doc comes from `solid_pod_rs::interop::did_nostr::did_nostr_document`; consumed shape-agnostically (inherits ADR-074 §D2′). |
| URI minter | `src/uri/mod.rs` (`urn:visionclaw` + `did:nostr`) | REUSE | sha256-12 byte-equal to agentbox; no change. |
| ADR identity-rail note | `docs/adr/ADR-124-smart-contract-features-web-contracts.md` | DOC | one line: DID-doc shape is now `DIDNostr`/`Multikey` per ADR-074 §D2′; `did:nostr` refs stay consistent. |

**Verification:** `cargo check -p visionclaw-server --lib` (passes at baseline,
warnings only); runtime needs a host rebuild via tmux tab 6 (DinD blocks
in-container builds — workspace CLAUDE.md). Passing `cargo check` + rebuild note
is sufficient.

### 5.3 agentbox (402 state gate + ship/verify glue + the create-agent layout)

| Work item | File | Type | Notes |
|---|---|---|---|
| 402 state gate | `acl:PaymentCondition`/`acl:costSats` (ADR-032) | REUSE | per-resource paywall over `gitmark`/`blocktrails`-anchored state. |
| Oracle ACL lock | management-api WAC surface | WIRE | oracle resource ACL-locked to operator `did:nostr`, never `acl:AuthenticatedAgent`. |
| Pod-git init/serve/push | solid-pod-rs `GitAutoInit` (gated) + `routes/pod-git.js` + `routes/git-bridge.js` | REUSE | the §4 live surfaces. agentbox checks `.git` exists; init owned by solid-pod-rs. |
| `agent.did.json` + `git config nostr.privkey` in pod-git root | `scripts/sovereign-bootstrap.py` (inline DID-doc block + new identity writer) | **NEW (greenfield)** | sovereign-bootstrap.py writes neither today (writes `did-nostr.json` + `identity.env`, and `build_did_document()`/`write_agent_repo_identity()` do not exist); add the §2.1 canonical Multikey doc to the pod-git root as `agent.did.json` + `git config nostr.privkey <hex>`. The `states[]`-into-pod-git-root claim is otherwise sound. |
| VerifiedSkill signed envelope (`aam skill sign` analogue) | `mcp/voyager/verify-and-store.py` | NEW (greenfield) | Schnorr-signed JCS envelope under the nostr key; `owner_did=did:nostr:<hex>` attester; URN `urn:agentbox:skill:<scope>:<name>:v<n>` stays the internal index. |
| Ship/verify invocation glue | management-api routes | WIRE | expose the solid-pod-rs `ship`/`verify` binaries for agentbox-hosted contracts. |

**Verification:** sovereign test suite (baseline green); contract harness for any fixture touched.

### 5.4 nostr-rust-forum — the externally-pullable forum pod (native build)

| Work item | File | Type | Notes |
|---|---|---|---|
| Native git backend | `crates/nostr-bbs-pod-worker/src/git.rs` (`GitHttpService` wired on native; CF=501 stub) | WIRE | anchor the trail on the native build + external forum pod, never the CF stub (ADR-089). |
| `agent.did.json` writer at provisioning | `crates/nostr-bbs-pod-worker/src/provision.rs` (`provision_pod`) | NEW (greenfield) | write the canonical Multikey DID doc (ADR-074 §D2′) into the pod-git root + `git config nostr.privkey`. |
| gitmark/blocktrails on forum pod | provision.rs + native git | NEW | commit the §2.1/§2.2 artefacts into the externally-pullable pod root; `states[]` = real forum-pod SHAs. |

**Verification:** `cargo check -p nostr-bbs-core --tests`; native git path only (CF deployment out of scope for anchoring).

### 5.5 dreamlab-ai-website — no web-contract substrate

REUSE — identity/forum-config surfaces only. No gitmark/blocktrails here.

## 6. Phased delivery

- **P0 (L0, ~1–1.5 eng-weeks):** `ContractReducer` trait + golden byte-parity
  test; `gitmark.json`/`blocktrails.json`/`ledger.json` serializers; 3-gate
  CHECKS registry; `verify`/`ship` binaries (incl. oracle-anchor-before-deadline
  + per-ledger atomicity); oracle ACL lock; first reference contract (worldcup
  parimutuel), hard-pinned tbtc4, cash-out/`.swap`/`.pool`/`.withdraw` disabled;
  `ship` wired onto the §4(1) pod-git remote. *Exit:* testnet pool runs
  end-to-end with independent `verify` replay against a real pod-git repo.
- **P1 (L0→L1):** seal-closing-witness check (extend `verify_mrc20_anchor` +
  `MempoolLookup` outspend) — cheapest/highest-leverage; share→cash-out mirror;
  per-state Merkle inclusion proofs; key-role separation + Zeroize + m-of-n
  (ADR-081); forum-pod native git wiring (§5.4).
- **P2 (L2, FUTURE — gate HARD-REFUSES until audited):** adaptor-sig CET engine on
  `k256::Scalar`/`ProjectivePoint`; DLC oracle attestation; timelocked refund CET;
  m-of-n oracle. Audit-dominated.
- **P3 (L3, deferred — layer rewrite):** `rgb-core`/`rgb-std`; consignment
  import/validate; AluVM. Reducer→AluVM, State→consignment.

## 7. Invariant boundary (explicit)

This build-out is **identity-rail-agnostic above the ADR-074 §D2′ `did:nostr`
Multikey layer.** It adds JSON-LD envelopes (`gitmark.json`, `blocktrails.json`,
`ledger.json`), a verifier, a reducer trait, a trust gate, a seal-closing check,
and a disablement flag — **none of which parse or re-encode the DID-document
verificationMethod**, none of which read identity from anything other than the
verified `did:nostr:<hex>` string / raw event pubkey. **I1–I4 hold trivially:** no
identity string changes (I1), no key bytes change (I2), no auth path reads the VM
(I3), ADR-074 §D1 stays (I4). The `agent_did` carried into
`gitmark`/`blocktrails`/`ShellGitMarker` commits is the unchanged `did:nostr:<hex>`
string. A future `verify` that tried to **decode** `publicKeyMultibase` back to a
key for an auth decision would violate I3 — the substrate signs/verifies with the
raw `nostr.privkey`/pubkey, NEVER the DID-doc VM. Flag any such change.

## 8. References
- ADR-124 (decision) — `docs/adr/ADR-124-smart-contract-features-web-contracts.md`
- solid-pod-rs ADR-059 — provenance primitives (block-trails & git-marks)
- agentbox ADR-032 — 402-scheme grammar; ADR-089 — git pods CF Workers limitation
- ADR-074 §D2′ / agentbox ADR-033 / project ADR-125 — did:nostr Multikey identity rail
- ADR-041/110/121/122/123 — Judgment Broker / ACSP / writeback / two-speed / voice sign-off
- ADR-081 — federation key custody / rotation
- `webcontracts.org` / "Melvo Predicts" worldcup pattern; create-agent `microfed/gitmark.json`
- BIP-341 (taproot single-use-seal); RGB / DLC (trustless endgame seam)
