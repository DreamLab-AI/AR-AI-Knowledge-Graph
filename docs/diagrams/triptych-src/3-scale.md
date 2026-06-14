# Panel 3 — THE SCALE

**Subtitle:** Value transfer, verifiable provenance, the web-contract trust spectrum, the numbers.

## What this panel says
The right panel is economics + proof + scale. Value moves over a sovereign stack;
every decision is anchored to a tamper-evident trail; smart-contract trust is a declared
spectrum, not a binary. Plus the headline numbers.

## Value transfer (sovereign stack — drawn as a flow, burnt-orange)
- **HTTP 402** payment-required gating + **L402** (Lightning).
- **WebLedger** sats-only credit/debit; **MRC20** token rail; **AMM** constant-product `x·y=k`.
- Bitcoin-native, **Lightning-first — no EVM, no coins**.

## Verifiable provenance (drawn as a descending bead chain into an anchor)
- **git-marks** — every decision earns a git-commit mark in its versioned pod.
- **BIP-341 single-use-seal block-trail** — high-value decisions anchored to Bitcoin taproot.
- Traceability is the value: verifiable provenance for human-governed knowledge.

## The web-contract trust spectrum (ADR-124 — drawn as a 4-rung ladder)
Four-layer state machine: **Contract** (pure validate+transition reducer) /
**State** (WAC + 402-gated pod JSON) / **Ledger** (WebLedger + MRC20 + AMM) /
**Trail** (BIP-341 single-use-seal block-trail).
- **L0 — honest-or-caught** (operator-custodial, signed + anchored) · AVAILABLE
- **L1 — reducible custody** (m-of-n multisig, true single-use seal) · AVAILABLE
- **L2 — DLC trustless oracle** (adaptor sigs) · FUTURE, audit-gated
- **L3 — RGB / client-side-validation** (portable assets) · FUTURE
- Through-line: the **single-use-seal chain**.

## The numbers (drawn as big hand-lettered stat blocks)
- **~5,975** ontology classes · **~123k** triples
- **92** CUDA kernels · **55×** GPU speedup vs CPU
- **88** agent skills · **7** MCP ontology tools
- **61µs** HNSW semantic search (1.17M entries)
- **250+** concurrent XR users · **80%** bandwidth cut (binary protocol)

## EXACT LABELS TO RENDER
- Panel title: **THE SCALE**
- Value flow (orange): **HTTP 402 · L402 · WebLedger · MRC20 · AMM (x·y=k)** —
  **Bitcoin-native, Lightning-first, no EVM**
- Provenance: **git-marks → BIP-341 single-use-seal block-trail → Bitcoin**
- Trust ladder rungs: **L0 honest-or-caught**; **L1 reducible custody**;
  **L2 DLC oracle (future)**; **L3 RGB / CSV (future)**
- 4-layer label: **Contract · State · Ledger · Trail**
- Stat blocks: **5,975 classes**; **123k triples**; **92 CUDA kernels**; **55× GPU**;
  **88 skills**; **61µs search**; **250+ XR users**
- Footer: **verifiable provenance for human-governed knowledge — not a crypto project**

## Aesthetic notes
Hand-drawn ladder (4 rungs) on the left, descending bead-chain anchor (burnt-orange) for
the block-trail, big charcoal stat numbers on the right. Teal for trust/provenance,
burnt-orange for value-flow + the Bitcoin anchor. No Solidity, no EVM, no coin clichés.
