# ADR-124 — Smart-Contract Features for the DreamLab / VisionClaw Ecosystem (Web-Contracts on a Single-Use-Seal Through-Line)

- **Status**: Proposed
- **Date**: 2026-06-14
- **Deciders**: DreamLab AI / VisionClaw architecture (lead architect synthesis); web-contract pattern lineage from Melvin Carvalho (team).
- **Builds on**: ADR-059 *provenance primitives — block-trails & git-marks* (**solid-pod-rs**: `crates/solid-pod-rs/docs/adr/ADR-059-provenance-primitives-block-trails-git-marks.md` — NOT the project ADR-059, which is *bidirectional-agent-channel-server*); ADR-032 *402 scheme grammar* (**agentbox**: `agentbox/docs/reference/adr/ADR-032-402-scheme-grammar.md` — NOT project ADR-032, which is *embed-solid-pod-rs-library*); ADR-041 (Judgment Broker), ADR-081 (federation key custody / rotation), ADR-110 (ACSP control surfaces), ADR-121/122/123 (writeback loop / two-speed governance routing / voice sign-off).
- **Feeds**: PRD-015 (consumer broadcast economy, Lightning-first), PRD-020.
- **Chosen approach**: **C — Progressive-Trust Web-Contract with a single-use-seal through-line** (a *planned* through-line at L0–L1, see §2.2), grafting A's ship-fast adoption posture and B's trustless endgame onto one upgradeable spine for the rungs we can actually build today.

---

## 1. Context

### 1.1 The clue: the worldcup web-contract pattern
The "Melvo Predicts" worldcup pool (the `webcontracts.org` pattern; the working artefact tree was present in a prior synthesis run at `/tmp/worldcup` and is the template reference throughout — not re-resolvable this run, so all worldcup `file:line` citations below are from that prior run and are *templates to port*, not load-bearing local code) is a fully worked **web contract**: a JSON state machine in **four layers**.

| Layer | Worldcup artefact (prior-run template) | Role |
|---|---|---|
| **Contract** | `scripts/validate.js` (type checker) + `scripts/score.js` / `scripts/ledger.js settle()` (pure reducers) | `validate()` + `transition()` — public, immutable, deterministic |
| **State** | `data/*.json`, `pool/pool.json`, `pool/entries/<pubkey>.json`, each with `schema/*.schema.json`, WAC-gated by `.acl` | schema-validated JSON on a Solid pod |
| **Ledger** | `pool/ledger.json` (`@context https://w3id.org/webledgers`), 1 share = 1000 tbtc4 sats | who owns what |
| **Trail** | `blocktrails.json` (`@type Blocktrail`, `profile gitmark`, chain `tbtc4`): `pubkeyBase` + `states[]` (commit SHAs) + `txo[]` (UTXO chain) | proof it happened — a BIP-341 taproot anchor chain |

Three validation **gates** share one schema (browser before write → `ship.js` before anchor via `npm test` → CI `validate-cli.js`); the deploy **ritual** is *edit → validate → commit → git-mark → push*, audited by a dual `verify.js` (recompute reducer, replay ledger, assert git-clean, confirm trail tip is a confirmed Bitcoin tx). The **trust model** (worldcup `AGENTS.md` §8) is **honest-or-caught**: push all trust into one signed + Bitcoin-anchored oracle; contract-correctness is already zero-trust (public reducer + anchored log, anyone replays). The operator is **catchable** (anchored log proves divergence) and **reducible** (escrow/multisig). The named upgrade seam to *trustless* contracts is the **single-use-seal chain** — the through-line to client-side-validation / RGB and DLCs.

### 1.2 Our substrate already implements three of the four layers in production Rust
`solid-pod-rs` (`jss run --git --nostr --pay`), keyed by `did:nostr` end to end, implements the lower three layers — **including the Bitcoin write-side**:

- **Contract engine**: `mrc20::jcs` (RFC-8785 JCS canonicalisation, `crates/solid-pod-rs/src/mrc20.rs:32`), `validate_mrc20_state` (`mrc20.rs:134`), `verify_state_link` (SHA-256 hash-chain + sequence enforcement, `mrc20.rs:151`), `verify_mrc20_deposit` (`mrc20.rs:196`).
- **State gate**: `acl:PaymentCondition` / `acl:costSats` (agentbox ADR-032 402 grammar), LDP/WAC surface, `did_nostr_types`.
- **Ledger**: `payments::WebLedger` sats-only `credit`/`debit` (`crates/solid-pod-rs/src/payments.rs:144,158`); **multi-currency** settlement via `trading.rs` `get_currency_balance`/`credit_currency`/`debit_currency` (`crates/solid-pod-rs/src/trading.rs:34,43,87`); constant-product AMM `x*y=k` (`trading.rs:320,450`) + order-book `execute_swap` (`trading.rs:240`).
- **Trail (verify-side)**: `mrc20::bt_derive_chained_pubkey` (`mrc20.rs:349`), `bt_address` (bech32m P2TR, `mrc20.rs:473`), `verify_mrc20_anchor` (live UTXO lookup, `mrc20.rs:508`); `provenance::ProvenanceLog::record` (`provenance.rs:363`) + `AnchorPolicy{Never,Always,HighValue,Epoch}` (`provenance.rs:219`) + `EpochAccumulator` Merkle batching with `verify_inclusion` + `prov_ttl` PROV-O sidecar.
- **Trail (write-side — IN PRODUCTION, integration-tested)**: `bitcoin_tx::build_transaction` (from-scratch BIP-341 taproot tx builder, `bitcoin_tx.rs:289`, golden-tested byte-for-byte against JSS), `bitcoin_tx::anchor_state` (`bitcoin_tx.rs:818`), `MempoolBroadcast::broadcast_tx` (`crates/solid-pod-rs-server/src/mempool.rs:238`), `MempoolBlockAnchorer::anchor` (`mempool.rs:300`), `BlockAnchorer::anchor` wired to `POST /{pod}/_prov/anchor` (NIP-98, 402-gated; `crates/solid-pod-rs-server/src/handlers/prov.rs`; tested in `tests/pay_phase4_routes.rs`, `tests/prov_phase5_routes.rs`).

The four web-contract layers map **1:1** onto these modules. **Two things are genuinely net-new** (see §2 and the component table): (a) a pure `validate()+transition()` reducer *trait*, and (b) the *seal-closing-witness* check (spent-exactly-once). Everything in the trail write-path is already built.

### 1.3 Ontology lineage (dogfood — live RuVector HNSW over `ns:ontology-classes`)
The semantic index reproduces the clue's crypto lineage verbatim and grades it:

- `urn:ngm:class:proof-of-publication` **[established]** — *enables* Single-Use-Seals, Client-Side-Validation, Block-Trails; the formal name for *honest-or-caught catchability*.
- `urn:ngm:class:single-use-seals` **[established]** — *enables* Client-Side-Validation, RGB, Block-Trails; the through-line root.
- `urn:ngm:class:client-side-validation` **[established]** — *enables* RGB, ZK-Proof, Block-Trails, Web-Contracts.
- `urn:ngm:class:web-contracts` **[emerging]** — *implements* Client-Side-Validation, *uses* Block-Trails + did:nostr — the direct analogue of worldcup's four layers.
- `urn:ngm:class:rgb-and-client-side-validation` **[established]** — *hasPart* AluVM, Bifrost, Consignment, Contract-Schema, Contractum — the trustless endgame. (`alu-vm`, `rgb`, `rgb-protocol` are tagged **[emerging]**.)
- `urn:ngm:class:discreet-log-contracts` **[draft]** — *enables* Smart-Contracts via signed outcomes, no trusted intermediary — the trustless-oracle seam.
- Settlement primitives cited as-is: `trustless-settlement` **[established]**, `amm-algorithm` **[established]** (cite this, **not** `automated-market-maker` **[draft]** — the parent grade is inverted), `escrow-system`, `delivery-versus-payment`, `prediction-markets`, `liquidity-pool`, `blockchain-oracle`, `verifiable-computation`, `verifiable-credential-surface` **[established]** (Schnorr-over-`did:nostr` — our existing attestation rail), `taproot`, `schnorr-signature`, `sovereign-keyset`/`sovereign-mesh`/`nostr-protocol`.
- **Regulatory cluster (RICH, not thin):** `uk-mlr-2017` **[established]**, `aml` **[mature]**, `anti-money-laundering` **[mature]**, `kyc-aml`, `transaction-monitoring`, `licensing-requirements` **[emerging — MiCA / FCA licence / money-transmitter]**, `financial-derivative` **[established]**, `compliance-verification`, `know-your-customer`. §7 is re-grounded on these IRIs.

**The decision is therefore adoption + seam, not greenfield.**

---

## 2. Decision

We **name the web-contract pattern as a first-class DreamLab/VisionClaw primitive** over `solid-pod-rs`, and commit to a **single block-trail / single-use-seal substrate** across **four trust levels**. **Only L0 and L1 are declarable today**; **L2 and L3 are research / audit-gated FUTURE levels** the capability gate must HARD-REFUSE (see §2.4, §4).

### 2.1 The four layers (canonical mapping)
A **VisionClaw web-contract** is:

1. **Contract** — a pure `ContractReducer { validate(state)→Vec<Error>; transition(state, event)→State }`, deterministic, no I/O, **compiled to wasm so it runs client-side** (browser computes, pod is a dumb store). Dual-reducer shape (scoring + money) mirroring worldcup's `score.js` + `ledger.js settle()`. *Grounds:* `urn:ngm:class:web-contracts`, `client-side-validation`. **Determinism is enforceable, not aspirational** — see §2.5.
2. **State** — schema-validated JSON on the pod, WAC + `acl:PaymentCondition`-gated (agentbox ADR-032 402 grammar). The oracle resource (`results.json`-equivalent) is **ACL-locked to the operator `did:nostr`**, never `acl:AuthenticatedAgent`.
3. **Ledger** — `payments::WebLedger` + the multi-currency `trading.rs` primitives (`get_currency_balance`/`credit_currency`/`debit_currency`, `trading.rs:34,43,87`) for settlement and share→cash-out. *Grounds:* `amm-algorithm` [established], `liquidity-pool`, `delivery-versus-payment`.
4. **Trail** — every settled state git-marked always + Bitcoin-anchored per `AnchorPolicy`; `Epoch` Merkle batching + per-state inclusion proofs for high-frequency contracts. *Grounds:* `block-trails`, `git-mark`, `single-use-seals`, `proof-of-publication`.

Plus a **published verifier** (port of `verify.js`), a **3-gate CHECKS registry** (port of `validate-cli.js`), and a **reusable ship command** (the extraction-ready generic core of `ship.js`), so one schema is enforced everywhere with zero drift.

### 2.2 The through-line claim (and its honest scope)
> The block-trail chain (`mrc20::bt_derive_chained_pubkey` `mrc20.rs:349` + `provenance::ProvenanceLog`) is the substrate shared across L0 and L1. Across those two rungs, upgrading a contract's trust changes **how the chain is *closed*** — not the chain itself.

- **L0**: the anchor is a **notary clock** (timestamp/order state) — UTXO-exists today (see §2.3).
- **L1**: the anchor becomes a **single-use seal** once the *seal-closing-witness* (spent-exactly-once) check lands.

**Scope correction:** the "one spine, four strengths, no rewrite" framing applies to **L0–L1 only**. **L2→L3 is a rewrite, not a tightening:** L3/RGB replaces the Contract layer (Rust reducer → AluVM bytecode) and the State layer (schema-validated JSON → strict-encoded consignment), and pulls a heavy `rgb-core`/AluVM external dependency that conflicts with our `k256`-only, zero-rust-bitcoin-dep posture. There is **zero RGB/AluVM/`rgb-core` in the tree** and the maturity is **emerging**. L3 is therefore "additive seam" only in that the anchor chain is RGB-*shaped*; the layers above it are net-new.

### 2.3 What the substrate provides TODAY: an anchor, not (yet) a single-use seal
`verify_mrc20_anchor` proves **UTXO-exists**: it derives the taproot address and asserts `address_utxos(...)` is non-empty (`mrc20.rs:546-547`). It does **not** prove the *prior* chained-key UTXO was **spent-exactly-once** — there is **no spent-status helper anywhere in `crates/`** (a repo-wide search for `outspend`/spent-check returns zero hits). So:

> **Today = single-use-*anchor* (UTXO-exists). The seal-CLOSING property (spent-exactly-once) is a P0/P1 deliverable.**

This is *decoupled* from the write-side, which is **done**: anchor production (`anchor_state` `bitcoin_tx.rs:818`, `broadcast_tx` `mempool.rs:238`, `MempoolBlockAnchorer::anchor` `mempool.rs:300`, `POST /{pod}/_prov/anchor`) is in production and integration-tested. The single remaining trail gap is the **lookup transport** for spent-status (an `/outspend` or `/address/{a}/txs` mempool call) plus a small extension to `verify_mrc20_anchor` that asserts the prevout was consumed exactly once. **It is the cheapest, highest-leverage change in this ADR.** Do not call the current substrate a "single-use seal" in code, docs, or marketing until that check exists.

### 2.4 Declared trust level + capability gate
Each contract declares `trust_level: L0|L1|L2|L3` in its `pool.json`-equivalent config, **anchored on-seal** (immutable commitment — see §2.5 / Q4). The **capability gate** is a hard pre-condition on `transition()` commit:

- It enforces the level's invariants (§4) *within* a level.
- It **HARD-REFUSES** any declaration of **L2 or above** until the adaptor-signature CET engine is *implemented AND independently audited* (§4, R3). An operator must not be able to obtain the "trustless" marketing/regulatory framing of L2 while the actual security is unaudited, self-rolled `k256` crypto that **does not exist yet** (confirmed absent in `bitcoin_tx.rs`/`mrc20.rs`; unground-able in the ontology — there is no `adaptor-signature` class).

### 2.5 Reducer determinism (enforceable)
`verify_state_link` (`mrc20.rs:151`) hash-chains the JCS of the canonical state. JCS canonicalises only the *hash input*, not the *computation*. So determinism must be enforced two ways:

1. **Lift all clock/UUID/random inputs out of the canonical state.** The worldcup JS template seeds non-determinism in exactly this place (e.g. `settle.js` / `gift.js` `new Date()` in the prior-run tree); those must be passed as explicit **event parameters**, never stored in the hash-chained state. Otherwise non-deterministic metadata leaks into the chain and the "anyone replays" property breaks at the chain level, not merely the leaderboard level.
2. **Byte-parity controls for reducer ARITHMETIC**: integer-only share math (no floats), fixed `serde` derive, deterministic map ordering, canonical integer division/rounding. The wasm↔native golden test asserts byte-parity on the **canonical state only** (the JCS input), mirroring the `bitcoin_tx` golden pattern — noting that the `bitcoin_tx` golden only works by **pinning `aux_rand = 0`** (`bitcoin_tx.rs:29,398`), i.e. it *sidesteps* cross-impl signature divergence rather than solving it; the reducer dual has no such escape hatch and must be genuinely byte-identical.

---

## 3. Component table

| Component | Reuse / NEW | Responsibility | Anchor (file:line / IRI) |
|---|---|---|---|
| `ContractReducer` trait (Contract) | **NEW** | Pure `validate()+transition()`, wasm+native byte-parity; dual reducer (scoring + money); integer-only arithmetic | models `mrc20::verify_state_link` `mrc20.rs:151`; template worldcup `scripts/{validate,score,ledger}.js`; `urn:ngm:class:web-contracts` |
| Contract **engine** (JCS + hash-chain) | **REUSE** | Deterministic canonicalisation + SHA-256 hash-chain + seq enforcement the reducer plugs into | `mrc20.rs:32,134,151,196` |
| `TrustLevel` + capability gate | **NEW** | Hard pre-condition on `transition()`; HARD-REFUSE L2+ until CET engine built+audited; on-seal level commitment | `single-use-seals`→`client-side-validation`→`rgb`/`dlc` |
| State layer (pod LDP + WAC 402) | **REUSE** | Schema-validated JSON, per-resource paywall, **oracle ACL lock** to operator did:nostr | `acl:PaymentCondition`; agentbox ADR-032 402 grammar |
| Ledger layer (WebLedger + multi-ccy + AMM) | **REUSE** + wiring | did:nostr multi-currency settlement; reducer-output→`credit_currency` cash-out mirror | `payments.rs:144,158`; `trading.rs:34,43,87,240,320,450`; `amm-algorithm`; **NOTE no-escrow `trading.rs:179`** |
| Trail write-side (anchor production) | **REUSE — DONE** | Build+broadcast+persist the BIP-341 anchor tx | `bitcoin_tx::{build_transaction:289, anchor_state:818}`; `mempool.rs:{238,300}`; `POST /{pod}/_prov/anchor` (`handlers/prov.rs`) |
| Trail verify-side (chained-key) | **REUSE** | Derive chained taproot key/address; verify UTXO-exists | `mrc20.rs:349,473,508,546`; solid-pod-rs ADR-059 |
| `AnchorPolicy` + Merkle inclusion | **REUSE** | *When* to anchor; per-state inclusion proof binds state to anchored root | `provenance.rs:219`; `EpochAccumulator::verify_inclusion`; `prov_ttl` |
| **Seal-closing-witness check** | **NEW (small, P0/P1)** | Upgrade *UTXO-exists* (`mrc20.rs:546`) → *prevout spent-exactly-once* = true single-use seal. Needs a spent-status lookup transport (outspend endpoint) — broadcast already wired | extends `verify_mrc20_anchor`; new `MempoolLookup` outspend call |
| Published verifier + 3-gate CHECKS + ship cmd | **NEW (mostly port)** | Replay audit (recompute reducer, assert stored==replay, git-clean, confirmed UTXO, **oracle-anchor-before-deadline §7**); one-schema-everywhere | ports `verify.js`, `validate-cli.js`, `ship.js`; reuses `verify_mrc20_anchor` + `verify_inclusion` |
| DLC oracle-attestation + adaptor-sig CETs (L2, FUTURE) | **NEW self-audited crypto** | Net-new secp256k1 protocol crypto: s-value/encrypted-nonce arithmetic hand-rolled on `k256::Scalar`/`ProjectivePoint`; existing signer is full BIP-340 `sign_raw` and CANNOT produce adaptor sigs | scalar/point arith exists `mrc20.rs:310-391`; existing signer `bitcoin_tx.rs:400` (`sign_raw`); `discreet-log-contracts`; **NO `adaptor-signature` ontology class** |
| RGB consignment import/validate (L3, deferred) | **NEW (heavy, last)** | Reducer→AluVM, State→consignment — a layer rewrite, not a tightening | dep `rgb-core` (absent from tree); `rgb-and-client-side-validation` |
| Governance / dispute / sign-off | **REUSE** (no new code) | Disputes/overrides/refunds = Judgment Broker cases; two-speed lane routing; voice sign-off; PROV-O per transition | ADR-041, ADR-110 (Nostr 31400-31405), ADR-122, ADR-123 |
| **Substrate capability-disablement flag** | **NEW (config/deploy gate)** | Hard-disable `.swap`/`.pool`/`.withdraw` + cash-out unless trust_level AND owner+legal sign-off authorise (see §7) | gates `trading.rs`, `/pay/.*` routes, `bitcoin_tx::anchor_state` cash-out leg |

**Net: ~85% reuse.** Genuinely new today: the reducer trait, the trust-level gate, the seal-closing check, the substrate-disablement flag. Genuinely new and FUTURE/audit-gated: adaptor-sig CETs (L2), RGB import (L3).

---

## 4. The trust spectrum and how a contract declares its level

The trust knob has four detents; each removes one trust bucket (oracle / contract-correctness / operator-custody). **L0–L1 are buildable and declarable now. L2–L3 are research / audit-gated — the gate refuses to declare them until their crypto exists and is audited.**

| Level | Status | Trust model | Custody | Oracle | Capability-gate invariants | Stakes tier | Ontology IRI |
|---|---|---|---|---|---|---|---|
| **L0** | **available** | honest-or-caught | operator-custodial | signed+anchored, catchable | public pure reducer **+** published verifier **+** per-state block-**anchor** **+** oracle ACL = operator `did:nostr` **+** oracle-anchor-before-deadline (§7) **+** per-ledger atomicity (§7) | T0 symbolic / testnet / friends | `proof-of-publication`, `web-contracts` |
| **L1** | **available (after seal-closing check)** | honest-or-caught, reducible-custody realised | m-of-n multisig | locked *before* outcome knowable | L0 **+** **seal-closing-witness (spent-exactly-once)** **+** Merkle inclusion proof **+** key-role separation (ADR-081) **+** m-of-n multisig | T1 small real value / friends | + `escrow-system` |
| **L2** | **FUTURE — HARD-REFUSED until CET engine built + independently audited** | honest-or-irrelevant | non-custodial (DLC), *only the DLC-settled leg* | did:nostr Schnorr attester, fund-less | L1 **+** pre-signed CET set + adaptor-sigs keyed to oracle attestation points **+** **timelocked refund CET** (oracle withholding) **+** m-of-n oracle for real value | T2 real value / public | `discreet-log-contracts`, `trustless-settlement` |
| **L3** | **FUTURE — deferred, layer rewrite** | honest-or-can't | none (portable CSV assets) | mis-attestation only, attributable | L2 **+** consignment validation (invalid-if-reducer-violated) — *new Contract+State layers* | T3 strangers / derivatives | `rgb-and-client-side-validation`, `client-side-validation` |

**Decision criterion (one sentence):** *pick the least-trust mechanism whose oracle + custody profile matches the stakes, and upgrade BEFORE scaling, never after a loss* — catching a dishonest operator does not return stolen sats.

**On the L2 crypto, precisely:** "scriptless on BIP-340 built on the existing signer" is **wrong and must not be claimed**. The existing path is `SigningKey::sign_raw` (full BIP-340, `bitcoin_tx.rs:400`), which **cannot** produce adaptor signatures. Adaptor sigs require hand-rolled s-value / encrypted-nonce arithmetic on raw `k256::Scalar` / `ProjectivePoint` (these types *do* exist — `mrc20.rs:310-391`). P2 is therefore **net-new, self-audited secp256k1 protocol crypto**, audit-dominated (R3), not "wiring an existing signer".

---

## 5. Reuse map

| Web-contract layer | solid-pod-rs / worldcup / ontology / governance asset |
|---|---|
| **Contract** | `mrc20::{jcs:32, validate_mrc20_state:134, verify_state_link:151, verify_mrc20_deposit:196}`; template worldcup `scripts/{validate,score,ledger}.js`; new `ContractReducer` trait |
| **State** | `acl:PaymentCondition` (agentbox ADR-032 402 grammar); LDP/WAC surface; `did_nostr_types`; `jss run --git --nostr --pay` |
| **Ledger** | `payments::WebLedger` (`credit:144`, `debit:158`, `pubkey_to_did:450`); multi-currency `trading.rs:{get_currency_balance:34, credit_currency:43, debit_currency:87}`; AMM `trading.rs:{320,450}` + order-book `execute_swap:240`; closes worldcup §7 share→`jss --pay` cash-out |
| **Trail (write)** | `bitcoin_tx::{build_transaction:289, anchor_state:818}`; `mempool::{MempoolBroadcast::broadcast_tx:238, MempoolBlockAnchorer::anchor:300}`; `POST /{pod}/_prov/anchor` (`handlers/prov.rs`, `tests/pay_phase4_routes.rs`, `tests/prov_phase5_routes.rs`) |
| **Trail (verify)** | `mrc20::{bt_derive_chained_pubkey:349, bt_address:473, verify_mrc20_anchor:508/546}`; `provenance::{ProvenanceLog::record:363, AnchorPolicy:219, EpochAccumulator::verify_inclusion, prov_ttl}`; **solid-pod-rs ADR-059** (`crates/solid-pod-rs/docs/adr/ADR-059-provenance-primitives-block-trails-git-marks.md`) |
| **Verifier / gates / ship** | port `verify.js`, `validate-cli.js` (CHECKS registry), `ship.js` generic core; reuse `MempoolHttpClient` (`mempool.rs:56`) for live UTXO + outspend checks |
| **Identity** | `urn:ngm:class:sovereign-keyset`/`sovereign-mesh`/`nostr-protocol`/`verifiable-credential-surface` (all **established**) — every VisionClaw agent holds a BIP-340 Schnorr key |
| **Governance** | ADR-041 Judgment Broker (6 `DecisionOutcome` variants Approve/Reject/Amend/Delegate/Promote/Precedent, append-only `DecisionHistory`, no self-review); ADR-110 ACSP (Nostr 31400-31405, **rejects bespoke REST approval**); ADR-122 three lanes by epistemic class (L1 human-gated / L2 auto, default-off); ADR-123 voice sign-off; PROV-O `prov_ttl` per transition |

---

## 6. Phased delivery

- **P0 — Name & adopt (L0, ~1–1.5 eng-weeks).** Define and golden-test the `ContractReducer` trait (**wasm↔native byte-parity is the headline risk**, §2.5). Wire the 3-gate CHECKS registry. Port the `verify.js` replay verifier and the `ship.js` generic core into Rust — **including the oracle-anchor-before-deadline assertion (§7) and per-ledger atomicity (§7)**. **ACL-lock the oracle resource** to operator `did:nostr`. Ship the worldcup parimutuel/leaderboard reducers as the first reference contract, **hard-pinned to tbtc4 with cash-out / `.swap` / `.pool` / `.withdraw` disabled (§7, P21)**. *Exit:* a testnet/friends pool runs end-to-end with independent replay.
- **P1 — Reducible custody + true single-use seal (L0→L1).** Add the **seal-closing-witness check** — extend `verify_mrc20_anchor` (`mrc20.rs:546`) from *UTXO-exists* to *prevout spent-exactly-once*, plus the spent-status lookup transport (outspend endpoint) on `MempoolLookup` (broadcast already wired, `mempool.rs:238`). This is the cheapest, highest-leverage change and is **not** gated on ADR-059 Phase-4 (which has landed). Wire the **share→`credit_currency` cash-out mirror** (tens of lines). Per-state Merkle inclusion proofs (`EpochAccumulator::verify_inclusion`). Key-role separation + at-rest encryption + `Zeroize` per ADR-081. m-of-n multisig custody.
- **P2 — Trustless oracle (L2, FUTURE; gate HARD-REFUSES until audited).** Build the adaptor-signature CET engine (encrypt/decrypt/verify on `k256::Scalar`/`ProjectivePoint` — net-new self-audited crypto) + DLC oracle nonce-pre-commit / outcome-scalar attestation format + **timelocked refund CET** for oracle withholding + m-of-n oracle for any real value. CET payouts settle through the existing WebLedger/AMM. **Resolve Q1 first** (does parimutuel/multi-winner mapping onto enumerated CETs even hold?). *Cost driver is the security audit, not the LOC.*
- **P3 — Trustless correctness (L3, deferred, layer rewrite).** `rgb-core`/`rgb-std` integration; consignment import/validate; AluVM schema authoring. Contract layer → AluVM, State layer → consignment. Heaviest external dependency; strains the sovereign zero-dep posture; sequenced last, optional.
- **P-onto (parallel, ~1 wk; DOGFOOD).** Ontology-augmentation rider (per ADR-112–119): **mint only the genuinely-missing OUR-CODE classes** — `adaptor-signature`, `dlc-oracle-attestation`, `mrc20-token-rail`, `webledger`, `payment-condition-402` (all confirmed absent) — and **fix the inverted `automated-market-maker`[draft] / `amm-algorithm`[established] parent-child grade**. **Do NOT re-grade external protocol classes** (`rgb`, `discreet-log-contracts`) to "production/established" as a deliverable — that asserts external-spec readiness we cannot warrant and conflates spec maturity with our buildability. Re-run the dogfood query set to confirm HNSW-retrievability of our own species.

---

## 7. Governance / security / custody / regulatory / oracle-risk

**Governance (reuse, do not reinvent).** Every *non-deterministic* contract event — dispute, oracle override, refund, manual payout, the *caught* branch when the verifier detects divergence — is a **Judgment Broker `BrokerCase`** routed through ACSP (Nostr 31400-31405). **Do not build a bespoke contract-admin REST surface** (ADR-110 explicitly rejects this; the signed `DecisionOutcome` *is* the authorisation; record it in PROV-O and anchor it). The contract **risk-class → governance-lane** mapping is modelled on **ADR-122's three lanes routed by epistemic class**: volatile/low-stakes/anchored-oracle events auto-settle; high-stakes/disputed/real-value-custody-release events are human-gated (L1), exposed to the **ADR-123 voice surface** for did:nostr-signed approve/reject/amend with readback-before-act. ADR-121's hard line ("never auto-write asserted truth") becomes the contract hard line: **never auto-release custodied real-value funds without a gate matching the stakes.**

**Custody (the sharp edge — and the containment retraction).** Today the WebLedger/AMM is **fully operator-custodial**: `trading.rs:179` is explicit — *"the order book does not escrow"* — and `execute_swap` (`trading.rs:240`) is checks-then-debit against an unlocked `&mut WebLedger`. **Retraction of the synthesis claim "trust_level contains the custody/regulatory trigger by construction":** the trust knob gates contract `transition()` **ONLY**. The operator-custodial rails — the WebLedger, the AMM (`trading.rs`), `/pay/.swap`, `/pay/.pool`, `/pay/.withdraw`, and the `bitcoin_tx::anchor_state` cash-out leg — are **substrate-level and BYPASS the trust knob entirely**. The gate does not, by itself, contain anything. **Required containment (architecturally enforced):** a per-deployment / per-contract **substrate capability-disablement flag** that **hard-disables `.swap`/`.pool`/`.withdraw` and the cash-out mirror unless `trust_level` AND owner+legal Judgment-Broker sign-off jointly authorise them**. Without this flag the custody containment is *false*.

**Security / key custody.** ADR-081 documents unresolved CRITICAL hazards (plaintext `private_key_hex`, missing at-rest encryption, no `rotate-keys`, no `Zeroize`). A fund-holding/oracle-signing contract inherits all of them. **Required before T1+:** the oracle-signing key, the custody/settlement key, and the substrate operator key are **distinct** (least privilege); at-rest encryption + `Zeroize` for any value-moving key; m-of-n multisig so a single compromise cannot drain a pool.

**Concurrency / atomicity (custody attack surface).** The `read_ledger → mutate → write_ledger` cycle is a TOCTOU: `PaymentStore` exposes `check_replay` and `record_replay` as **two separate calls** (`payments.rs:441-442`) and `execute_swap` mutates an **unlocked** `&mut WebLedger` (`trading.rs:240`). At L0/L1 (which hold real sats), require: a **per-ledger advisory lock or CAS** on a ledger version/seq around the mutate cycle, and **collapse `check_replay`+`record_replay` into one atomic record-if-absent** operation.

**Oracle (irreducible — concrete gate checks, not prose).** No cryptography removes the oracle (`urn:ngm:class:blockchain-oracle`); it can only be made *signed, timely, anchored, and ideally m-of-n*.
- **L0 capability-gate check (enforceable, in the ported `verify.js`):** the verifier MUST assert that the `results.json` anchor's **confirmed-block-time (or seal-close time) precedes the pool's pick-submission cutoff recorded in `pool.json`**, AND that the cutoff itself was anchored before any picks were accepted. Without this, honest-or-caught is honest-*on-trust*. (Affirmative answer to Q4: the trust-level AND the deadline MUST be **on-seal immutable commitments**.)
- **L2 downgrades the oracle** from *can rewrite/withhold/steal* to *can only publicly mis-call under its own pubkey* — **but not withholding**: a silent oracle can permanently lock funds. So L2 MUST include a **timelocked refund/timeout CET**, and **m-of-n oracle is a stated requirement (not an open question) for any T2+ real-value settlement**, since single-oracle DLC still permits unattributable liveness-griefing by the operator-as-oracle.

**Regulatory (jurisdiction-grounded; ontology is RICH here).** The legal exposure is **decoupled from the cryptographic trust knob** — there are two independent axes:

**Axis A — cryptographic-trust-level:** L0 → L1 → L2 → L3.
**Axis B — legal-product-class:** *gambling/parimutuel* | *securities/derivatives* | *money-transmission/VASP*.

| Legal-product-class \ trust-level | L0 | L1 | L2 | L3/RGB |
|---|---|---|---|---|
| **Gambling / parimutuel** (stakes pool) | Gambling Commission licence if real-stakes/public | same | same | **same — L3 does NOT exempt** |
| **Securities / derivatives** (`prediction-markets`, outcome tokens; `financial-derivative` [established]) | UK **FCA retail crypto-derivatives ban**; potential unregistered securities | same | same | **same — L3 does NOT exempt** |
| **Money-transmission / VASP** (MRC20 issuance + AMM/withdraw) | **MLR-2017** (`uk-mlr-2017` [established]) crypto-exchange registration; `kyc-aml`/`aml`[mature]/`transaction-monitoring`; `licensing-requirements`[emerging — MiCA/FCA/money-transmitter] | same | same | **same — L3 does NOT exempt** |

**The crucial point: a public real-value pool is regulated at EVERY cryptographic level.** L3/RGB removes operator *custody* but does **not** remove any *legal* obligation on any axis. **The only true containment is staying in the testnet / symbolic / no-cash-out corner.** Therefore (P21, architecturally enforced, not aspirational): the reference-contract config **hard-pins currency to tbtc4 (testnet)**, **disables the cash-out mirror and `.withdraw`/`.swap`/`.pool` routes**, the **owner+legal Judgment-Broker sign-off is a BUILD/DEPLOY gate** (not a runtime hope), and the **trust-level + currency + cash-out flags are anchored on-seal** so a deployed contract cannot silently switch from testnet to mainnet currency or enable cash-out post-deployment.

---

## 8. Ontology grounding

**The design rests on these IRIs (verified live via `ns:ontology-classes` HNSW):**

`urn:ngm:class:` — `proof-of-publication` [established], `single-use-seals` [established], `client-side-validation` [established], `web-contracts` [emerging], `block-trails`, `git-mark`, `rgb-and-client-side-validation` [established], `rgb`/`rgb-protocol`/`alu-vm` [emerging], `discreet-log-contracts` [draft], `trustless-settlement` [established], `escrow-system`, `delivery-versus-payment`, `amm-algorithm` [established] (parent `automated-market-maker` [draft] — inverted), `liquidity-pool`, `prediction-markets`, `blockchain-oracle`, `verifiable-computation`, `verifiable-credential-surface` [established], `taproot`, `schnorr-signature`, `sovereign-keyset`/`sovereign-mesh`/`nostr-protocol`; **regulatory:** `uk-mlr-2017` [established], `aml`/`anti-money-laundering` [mature], `kyc-aml`, `transaction-monitoring`, `licensing-requirements` [emerging], `financial-derivative` [established], `compliance-verification`, `know-your-customer`.

**Where the ontology HELPED (strongly):** the cryptographic substrate (`taproot`/`schnorr`/`single-use-seals`/`proof-of-publication` all established and correctly linked); the RGB/CSV lineage (`single-use-seals → client-side-validation → rgb / web-contracts` reproduced verbatim); the `did:nostr` identity cluster (fully established); **and the regulatory cluster** — contrary to the earlier draft's "thin on regulation" verdict, the index is **RICH** here (`uk-mlr-2017`, `aml`/`anti-money-laundering` mature, `transaction-monitoring`, `licensing-requirements`, `financial-derivative` established), which is exactly why §7 can ground UK-specific obligations on real IRIs.

**Where the ontology is THIN (the augmentation rider — the crypto-primitive gaps only):**
1. **`adaptor-signature`** — no class. Only `schnorr-signature` exists. This is the primitive that makes DLCs trustless; the L2 seam currently has **no cryptographic grounding** in the index — reinforcing why L2 is HARD-REFUSED.
2. **`dlc-oracle-attestation`** — `blockchain-oracle`/`chainlink-oracles` are EVM-oriented; the Bitcoin DLC oracle (signs an outcome, no on-chain footprint) has no class.
3. **`mrc20-token-rail`, `webledger`, `payment-condition-402`** — the most surprising gap: the ontology is thinnest exactly where OUR code lives (only `l-402-protocol` [domain:unknown] and `x-402` [stablecoin] exist — neither is our sat/MRC20 402 grammar).
4. **Maturity defect:** inverted `automated-market-maker`[draft] / `amm-algorithm`[established] parent-child grade.

These four are the concrete **DOGFOOD return** to the ADR-112–119 augmentation track (P-onto). We do **not** propose re-grading external protocol specs (`rgb`, `dlc`) as a dogfood deliverable.

---

## 9. Alternatives considered

- **A — Web-Contract Layer (honest-or-caught only).** ~90% reuse, fastest to ship, contract-correctness zero-trust from day one. **Rejected as the whole answer** because it permanently retains operator custody (`trading.rs:179`) and irreducible oracle trust — fine for T0, but it forces a re-architecture later. *Scores:* sov 5 / trustless 2 / reuse 5 / ttship 5 / reg 2. **Grafted in** as C's L0/P0 rung.
- **B — Trustless Layer (DLC + RGB straight away).** Removes the oracle/custody trust; correctness valid-by-construction. **Rejected as the starting point** because it skips the cheap-and-shippable rung; highest maturity risk (we'd build on primitives our own graph under-rates and that are absent from the tree); requires self-audited adaptor-sig crypto and a heavy `rgb-core` dependency that strains our zero-rust-bitcoin-dep posture; the DLC enumerated-outcome model is awkward for parimutuel prize splits (Q1). *Scores:* sov 4 / trustless 5 / reuse 3 / ttship 2 / reg 4. **Grafted in** as C's L2/L3 *future* endgame.
- **C — Progressive-Trust through-line (CHOSEN).** ~85% reuse; per-contract declared trust level; the block-trail spine is shared across L0–L1 so upgrade there is in-place (L2→L3 is a rewrite, §2.2). **Chosen** because it dominates: it ships A's L0 *now*; it *enables* (but does not by itself contain — §7) stakes-matched containment of the custody/regulatory trigger via the substrate-disablement flag + owner/legal gate; it reaches B's trustless endgame *without rewriting L0–L1*; and it is the only option that makes the ontology lineage `single-use-seals → CSV → RGB/DLC` an executable roadmap. *Scores:* sov 5 / trustless 4 / reuse 5 / ttship 4 / reg 4 — the strongest aggregate. **Note:** the containment is a *property we must build* (the flag + gate), not a property the trust knob has *by construction*.

**Rejected for now (mirroring ADR-059's EVM rejection):** covenant-enforced vaults (CTV/CSFS BIP-119/BIP-348) — no mainnet activation as of Jun 2026; everything L0–L1 needs works on today's taproot key-path anchors. **EVM/L2** — Bitcoin-only, Lightning-first (PRD-015, solid-pod-rs ADR-059).

---

## 10. Consequences, risks, open questions

**Positive.** One block-trail spine across L0–L1 → trust tightens without rewrite at those rungs. ~85% reuse → adoption not greenfield. Stakes-matched levels **plus the substrate-disablement flag + owner/legal gate** contain the regulatory/custody trigger (a built property, not an automatic one). Governance/provenance fully reused (ADR-041/110/121/122/123 + PROV-O). Bitcoin-only, did:nostr everywhere, no covenant dependency. The Bitcoin write-side is already production.

**Negative / risks.**
- **R1 (reducer drift).** The `ContractReducer` must be byte-identical in wasm (browser) and native (CLI/CI). Worldcup's single shared `validate.js` does **not** cover a Rust-wasm/Rust-native dual; the `bitcoin_tx` golden only works by pinning `aux_rand = 0` (sidesteps, not solves, cross-impl divergence — `bitcoin_tx.rs:29,398`). **Mitigate** with integer-only arithmetic, deterministic serde/map ordering, clock/UUID/random lifted to event params (§2.5), and golden cross-impl tests on the canonical state only.
- **R2 (seal-closing not yet built — but write-side IS).** `verify_mrc20_anchor` proves *UTXO-exists* (`mrc20.rs:546`), not *spent-exactly-once*; **no spent-check helper exists anywhere in `crates/`**. This is the only real trail gap. The **write-side is landed and integration-tested** (`anchor_state` `bitcoin_tx.rs:818`, `broadcast_tx` `mempool.rs:238`, `MempoolBlockAnchorer::anchor` `mempool.rs:300`, `POST /{pod}/_prov/anchor`). The missing piece is a **spent-status lookup transport** (outspend endpoint) + a small `verify` extension — **not** tx-building/broadcast, and **not** gated on ADR-059 Phase-4. Today's substrate is a single-use-*anchor*; L1+ real-value claims require the seal-*closing* check first.
- **R3 (self-audited crypto, L2).** Adaptor signatures are net-new crypto on `k256` (the existing `sign_raw` is full BIP-340 and cannot produce them); a subtle bug loses funds silently. **Audit is the dominant cost.** The capability gate HARD-REFUSES declaring L2 until this engine is built AND independently audited.
- **R4 (key custody).** ADR-081 CRITICALs (plaintext keys, no at-rest encryption, no `Zeroize`, no rotate) are inherited by any oracle-signing/custody key. **Must close before T1+.**
- **R5 (custody NOT contained by the trust knob alone).** The trust knob gates `transition()` only; the custodial WebLedger/AMM/`.swap`/`.pool`/`.withdraw`/cash-out rails bypass it. **Mitigate** with the substrate capability-disablement flag + owner+legal build/deploy sign-off gate (§7).
- **R6 (concurrency/TOCTOU).** Two-call replay guard (`payments.rs:441-442`) + unlocked `&mut WebLedger` in `execute_swap` (`trading.rs:240`) are a custody attack surface at L0/L1. **Mitigate** with per-ledger advisory lock/CAS + atomic record-if-absent.
- **R7 (RGB lift).** `rgb-core` is a heavy external dependency (AluVM, Contractum experimental) absent from the tree; pulls the sovereign zero-dep posture and **rewrites the Contract+State layers**. Sequenced last, optional.

**Open questions.**
- **Q1 (blocking L2 for the flagship).** Does the worldcup parimutuel/leaderboard reducer (continuous multi-winner prize splits) map onto DLC's enumerated-outcome CET model, or do we need a hybrid (many CETs / honest-or-caught settlement with DLC only for a binary jackpot leg)? **Must be resolved before claiming L2 de-custodies the flagship contract.**
- **Q2.** Canonical seal-closing-witness path — full prevout-spend check via an outspend `MempoolLookup` call, or a lighter SPV-style inclusion proof?
- **Q3 (now a requirement, not just a question, for T2+).** Multi-source / m-of-n oracle design over `did:nostr` attesters (`chainlink`/`price-oracle` pattern) for safety-critical settlement.
- **Q4 (answered: yes).** The trust-level declaration, the deadline, the pinned currency, and the cash-out flags MUST be **on-seal immutable commitments** so a deployed contract cannot silently downgrade its level or switch testnet→mainnet after funds are committed.

---

**Delivery note (paths verified this run).** ADR-124 did not previously exist (`docs/adr/ADR-124*` absent). Disambiguation enforced throughout: **solid-pod-rs ADR-059** = provenance primitives (`crates/solid-pod-rs/docs/adr/ADR-059-provenance-primitives-block-trails-git-marks.md`); **project ADR-059** = bidirectional-agent-channel-server (different ADR). **agentbox ADR-032** = 402-scheme-grammar (`agentbox/docs/reference/adr/ADR-032-402-scheme-grammar.md`); **project ADR-032** = embed-solid-pod-rs-library (different ADR). Load-bearing claims re-verified against CODE: write-side production (`anchor_state` `bitcoin_tx.rs:818`, `MempoolBroadcast::broadcast_tx` `mempool.rs:238`, `MempoolBlockAnchorer::anchor` `mempool.rs:300`, `POST /{pod}/_prov/anchor` in `handlers/prov.rs` with `tests/pay_phase4_routes.rs` / `tests/prov_phase5_routes.rs`); anchor = UTXO-exists not spent-once (`mrc20.rs:546-547`); **zero `outspend`/spent-status helper in `crates/`**; existing signer is full BIP-340 `sign_raw` (`bitcoin_tx.rs:400`) with raw `k256::Scalar`/`ProjectivePoint` available (`mrc20.rs:310-391`); no-escrow boundary (`trading.rs:179`); two-call replay guard (`payments.rs:441-442`); `AnchorPolicy{Never,Always,HighValue,Epoch}` (`provenance.rs:219`); reducer engine (`mrc20.rs:32-216`); multi-currency primitives (`trading.rs:34,43,87`). The `/tmp/worldcup` template tree was not resolvable this run; its citations are prior-run templates to port, not local load-bearing code. This document is the body for `docs/adr/ADR-124-smart-contract-features-web-contracts.md`.
