# ADR-094 — Admin-Pubkey Permission Model and NIP-26 Phone Delegation

| Field | Value |
|-------|-------|
| Status | Deferred — frozen by closeout 2026-07-03 (was Proposed 2026-06-02). Admin-pubkey / NIP-26 delegation deferred: delegation is implemented but unwired into any ingress. |
| Drives | PRD-017 §5.3 (F12-F13), §7.1 (BLOCK-3) |
| Companion ADRs | ADR-092 (client/signer), ADR-093 (substrate), ADR-074 (did:nostr canonicalisation) |
| Affected repos | `agentbox` (relay-consumer auth, management-api auth.js), `nostr-rust-forum` (`nip26.rs` source) |
| Evidence | `docs/integration-research/nostr-mobile-bridge/04-did-nostr-identity.md`, `01-telegram-bridge-teardown.md` |

## Context

The bridge's control channel is the operator's most privileged surface — a message that reaches agent dispatch can make the agent act. Authorisation must be `did:nostr`-native (not Telegram-style chat membership) and must not require the operator's phone to hold the agentbox's own identity key.

Current state:
- agentbox management-api runs `strict-nip98` when sovereign mesh is enabled (`auth.js:105-108`); admin routes gate on `admin_pubkeys` (`agentbox.toml:159`) or the operator pubkey `AGENTBOX_X_ONLY_PUBKEY_HEX`.
- NIP-26 delegation is fully implemented (`nostr-bbs-core/src/nip26.rs`: `DelegationToken::create:150`, `verify:178`, `validate_delegation_tag:258`) but NOT wired into any consumer ingress path (research 04 §4.2) — including the agentbox relay-consumer `_processEvent`.
- CTM's `IdentityStore` maps `did:nostr:<pubkey>` → role but `is_allowed()` is never called in the message path (research 01 §6).

## Decision

### D1 — Authorisation is admin-pubkey gated, signature-first

An inbound message is dispatched to an agent only if, AFTER Schnorr signature verification, the unwrapped rumor's author pubkey is one of:
1. `AGENTBOX_X_ONLY_PUBKEY_HEX` (the operator/agent identity), or
2. an entry in `agentbox.toml [sovereign_mesh.multi_user] admin_pubkeys`, or
3. a key bearing a valid NIP-26 delegation (D2) from (1) or (2).

The check runs at the relay-consumer dispatch boundary, before any agent side effect. Signature verification precedes authorisation — an unsigned or forged event is rejected before its pubkey is even consulted (NFR-2).

### D2 — Phone holds a NIP-26 delegated key (not the admin nsec)

The phone holds its OWN independent secp256k1 keypair with its own `did:nostr`. The admin key signs a NIP-26 delegation token authorising it:

```
conditions: kind=14&kind=1059&created_at>T_start&created_at<T_end
tag:        ["delegation", "<admin_pubkey_hex>", "<conditions>", "<schnorr_sig>"]
```

The phone appends this delegation tag to its events. The admin nsec stays on the agentbox; it is never imported into the phone or Amber. This satisfies PRD-017 G4 and preserves the agentbox container-key stability invariant (DDD A-Inv-04).

### D3 — Wire the EXISTING delegation validator into the relay-consumer

`validate_delegation_tag` (`nostr-bbs-core/nip26.rs:258`) already implements the full check and is unit-tested: parse the delegation tag, verify the delegator's Schnorr signature over `SHA-256("nostr:delegation:" || delegatee_pubkey_hex || ":" || conditions)` (`nip26.rs:281`), check conditions permit the event kind and timestamp. The net-new work is to **call it** in the agentbox relay-consumer dispatch path and confirm the delegator is in the admin set (D1) — wiring, not cryptography. This is PRD-017 BLOCK-3 — without invoking the existing verifier, the delegation is trusted blind and the whole model collapses; the risk is a missing call, not an unimplemented algorithm.

### D4 — Revocation is window-bounded

NIP-26 has no explicit revocation. Delegation windows MUST be short (operator-tunable; recommend ≤ 7 days) and reissued by an agentbox-side tool. On phone compromise, the immediate mitigation is to stop reissuing and let the window expire; a forced kill is achieved by issuing a token whose window is already closed (research 04 §7.2). For faster revocation, the admin may remove the delegated pubkey from any consumer-side allow-cache.

### D5 — Management-api parity

When the phone calls the management-api (NIP-98, kind-27235), the same delegation acceptance applies: `auth.js` MUST, after `verifyNip98Header` succeeds, detect a delegation tag, validate it (D3), and treat the event as admin if the delegator is in the admin set. The delegation conditions for management-api use restrict to `kind=27235`.

## Consequences

**Positive:**
- `did:nostr`-native auth, consistent with every other ecosystem surface; reuses the existing, test-covered `nip26.rs`.
- Admin identity key never leaves the agentbox; phone is a revocable, first-class delegated identity.
- Closes the long-standing gap where `IdentityStore`/delegation existed but were never enforced (research 01 §6, research 04 §4.2).

**Negative / risks:**
- BLOCK-3 is wiring, not crypto: `validate_delegation_tag` is implemented and unit-tested in `nostr-bbs-core`. The security-critical risk is a *missing or mis-ordered call* — trusting the delegation tag without invoking the existing verifier is a full auth bypass. Covered by the 15-row AUTH matrix + 12 negative tests (research 08).
- Window-bounded revocation means a compromised phone retains access until window expiry unless the consumer-side cache is updated. D4 provides the faster path.
- Delegation reissue is operational overhead; needs an agentbox-side reissue tool.
