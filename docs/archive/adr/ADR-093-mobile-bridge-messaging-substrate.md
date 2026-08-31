# ADR-093 — Mobile Bridge Messaging Substrate (NIP-17 / NIP-44 / NIP-59)

| Field | Value |
|-------|-------|
| Status | Deferred — frozen by closeout 2026-07-03 (was Proposed 2026-06-02). Mobile-bridge NIP-17 transport partially landed in agentbox; the cluster is deferred. |
| Drives | PRD-017 §5.2 (F5-F9), §5.5 (F20-F22) |
| Companion ADRs | ADR-092 (client), ADR-094 (permissioning), ADR-095 (summaries), ADR-097 (topology) |
| Affected repos | `agentbox` (relay-consumer call-site, outbound publisher); `nostr-rust-forum` (`nostr-bbs-core` crypto crate — consumed via WASM, one `wasm_bridge.rs` shim) |
| Evidence | `docs/integration-research/nostr-mobile-bridge/07-nip-substrate.md`, `03-cf-private-relay.md`; `nostr-bbs-core/{nip44,gift_wrap,nip26}.rs`, `tests/e2e_auth_flow.rs` |

## Context

The bridge needs a wire protocol for phone↔agent messaging. The candidates are NIP-04 (legacy DM, kind 4), NIP-17 (private DM, kinds 14/15), NIP-28/29 (public/group chat), and NIP-90 (DVM structured jobs). The choice must give per-recipient confidentiality, metadata privacy, and an interactive surface, while remaining forward-compatible with the CF private relay and an eventual admin control panel.

Two facts constrain the choice:
- The CF relay gives kind-4 (NIP-04) NO per-recipient isolation — it is served to any subscriber (research 03 §4). Only kind-1059 (NIP-59 gift wrap) gets AUTH + `#p`-recipient isolation (research 03 §3, `nip_handlers.rs:398-430`).
- The agentbox embedded relay stores kind-1059 but never decrypts it (ADR-009:262 deferral). The decryption *primitives* are NOT net-new: `nostr-bbs-core` already implements and e2e-tests the full NIP-59 gift-wrap roundtrip (`gift_wrap.rs` `unwrap_gift`/`seal_rumor`/`wrap_seal`) over NIP-44 v2 (`nip44.rs`), and the forum ships them to JS via `wasm_bridge.rs`. What is net-new is only the agentbox **call site** that invokes the existing decrypt at the dispatch boundary.

## Decision

### D1 — NIP-17 is the primary chat transport

Ad-hoc chat uses **NIP-17**: kind-14 chat / kind-15 file rumors, sealed in kind-13, gift-wrapped in kind-1059 per **NIP-59**, encrypted with **NIP-44 v2** (ChaCha20 + HMAC-SHA256, HKDF-derived keys). This is the only DM scheme that gives both content confidentiality and sender-metadata privacy, and it is the kind that the CF relay isolates per-recipient. NIP-17 is the MVP path.

### D2 — NIP-04 is forbidden

Kind-4 NIP-04 DMs MUST NOT be used. No per-recipient isolation on the CF relay (research 03 §4), leaks metadata, and is deprecated upstream. The agentbox consumer MUST NOT fall back to NIP-04.

### D3 — NIP-90 for structured tasks (optional, Phase 1)

When the operator wants a typed job rather than free chat, use **NIP-90 DVM** kinds: 5xxx request, 6xxx result, 7000 feedback. This is additive over the chat path and optional in Phase 1; the chat path (D1) is the floor.

### D4 — ACSP control panels for admin actions (Phase 2)

Structured admin control (e.g. the synchronous approve/reject that CTM does with inline buttons) uses **ACSP** over NIP-33 addressable kinds 31400-31405 (PanelDefinition/State/ActionRequest/Response/PanelUpdate/PanelRetired). Deferred to Phase 2; these kinds are already agent-registry-gated on the CF relay (research 03 §4, `nip_handlers.rs:221-232`).

### D5 — Outbound mirroring is NIP-17

Agent→phone turn output, session start/end, and (optional, verbose) tool activity are delivered as NIP-17 DMs to the phone pubkey (PRD-017 F20-F22). One logical conversation per session.

### D6 — Consume the existing decryption path; wire the call-site in the relay-consumer

The agentbox relay-consumer MUST gain a kind-1059 unwrap path: unwrap gift wrap → unseal kind-13 → recover kind-14 rumor → NIP-44 v2 decrypt. This is **not** a from-scratch build. `nostr-bbs-core` already implements every step (`gift_wrap.rs` `unwrap_gift:307`, `seal_rumor:179`, `wrap_seal:226`; `nip44.rs` `decrypt:81`, `conversation_key:97`), benched and roundtrip-tested (`e2e_auth_flow.rs:8` "Gift wrap (NIP-59): encrypt → wrap → unwrap → decrypt for DM relay routing"). The crate is already first-party built and run as a service in agentbox; the forum already consumes the same crate from JS via WASM.

The residual work is integration: (1) consume `nostr-bbs-core` from the relay-consumer — either through the existing `wasm_bridge.rs` exports or a Rust path; (2) add the one missing `#[wasm_bindgen]` shim re-exporting `gift_wrap`/`unwrap_gift` (the impl is `pub` in `gift_wrap.rs` but `wasm_bridge.rs` currently exposes only `nip44_encrypt`/`nip44_decrypt`, schnorr, and NIP-98); (3) invoke it at the dispatch boundary. NIP-44 conformance (PRD-017 BLOCK-1) is satisfied by the existing implementation plus a live Amethyst/Amber interop check — not by writing crypto.

## Consequences

**Positive:**
- Single confidential transport that the CF relay already isolates per-recipient — Phase 2 federation needs no DM-scheme change.
- Layered design: chat (D1) ships first; structured tasks (D3) and admin panels (D4) layer in without reworking transport.

**Negative / risks:**
- NIP-44 v2 correctness is load-bearing and unobservable on the happy path — but it is already implemented, benched, and e2e-tested in `nostr-bbs-core`; BLOCK-1 reduces to consuming it and proving live Amethyst/Amber interop against the published vectors.
- The cryptographic risk is concentrated in a single, already-tested crate rather than spread across a new agentbox implementation — a consequence of consuming `nostr-bbs-core` rather than hand-rolling. The remaining integration risk is at the call-site (correct argument marshalling, error handling) and the WASM boundary.
- Cross-repo coupling: agentbox now depends on `nostr-bbs-core` as a shared library. This is library reuse, not runtime forum interop — the crate is already the shared substrate under the relay worker, auth worker, and forum client, so the PRD-017 "no forum AT THIS TIME" constraint holds.
- The agentbox embedded relay must match the CF relay's kind-1059 `#p`-isolation semantics, or recipient isolation (NFR-3) degrades in Phase 1.
