# P0 — COM-14 / D4 / M1 (consumer side): did:nostr keying of agent nodes

- **Item:** COM-14, D4 (desktop), M1 (identity root) — PRD-023 WP-1
- **Canary:** `CANARY-VC-COM14-DID` (standing, P0)
- **Base SHA:** `6f4eb1b0aeaa2b30f6959c9caa3c0a3eb485424d` (branch `gap-close/2026-07`)
- **Verified:** 2026-07-08T10:57Z
- **Maturity:** `scaffolded` → `integrated` (struct field carried end-to-end + verify-before-trust seam unit-tested). The live challenge/response round-trip against a running agentbox is **pending-live-session** — honestly labelled, not claimed closed.

## Falsification (PRD-023 WP-1)

*"WP-1 is falsified if any surface still keys an agent by `task_id`, if a `did:nostr` is trusted without a verified signature over a challenge, or if the Godot avatar renders a nameplate but no verifiable identity while `graph_scene.gd:431` still holds the DID in metadata."* (The Godot/M1-HUD render clause is WP-9 P0/P2 territory; this file covers the desktop consumer carry + the verifier seam.)

## What landed

### 1. `Agent` carries `did_nostr: Option<String>` in BOTH definitions (ADR-130 D6, AC1)
- `crates/visionclaw-domain/src/types/mcp_responses.rs:98` — `#[serde(default, skip_serializing_if = "Option::is_none")] pub did_nostr: Option<String>`.
- `src/services/bots_client.rs:38` — same field on the services-layer `Agent`, populated in `From<MultiMcpAgentStatus>` via the round-trip gate `validate_did_nostr()` (line ~52).

### 2. Populated from the agentbox agent record; validated through `src/uri/mod.rs` before storing
- `src/services/agent_visualization_protocol.rs` — `MultiMcpAgentStatus` gains `#[serde(default)] pub did_nostr`.
- `src/utils/mcp_tcp_client.rs:458` — `parse_single_agent` extracts `did_nostr`/`didNostr` from every agentbox agent record (the live carry to `/api/bots/agents` is the 2 s agent-list poll → `Agent::from` → cache → `get_bots_agents`; the dead `spawn_agent_mcp` helper is NOT the live path).
- `src/services/bots_client.rs` `validate_did_nostr()` accepts a claim only if `uri::parse` yields `ParsedUri::DidNostr` AND `uri::did_nostr(pubkey)` re-mints the exact claim (ADR-125 I1). Malformed → `None`, warn-logged.

### 3. Verifier seam per ADR-130 Decision 6 (I3-safe)
`src/services/nostr_identity_verifier.rs` `verify_did_matches_challenge(payload_did, challenge, verifier)`:
1. Gate 1 (I1): `payload_did` must be a canonical `did:nostr` (rejects before any crypto).
2. Gate 2 (I3-safe): BIP-340 Schnorr verify over `(nonce || timestamp_us)` against `event.pubkey` (`claimed_pubkey_hex`) via the existing `NostrIdentityVerifier` — the raw event pubkey, **never** a DID-document verificationMethod.
3. `did:nostr:{event.pubkey}` must equal `payload_did`, else `RoomError::InvalidDid`.

### 4. Client: DID is the trust key, task_id the fallback
- `client/src/features/visualisation/components/agentIdentity.ts` (new) — `agentTrustKey()` (DID when non-empty, else `id`), `isDidKeyed()`, `shortDid()`.
- `AgentNodesLayer.tsx` — `AgentNode` gains `did_nostr?: string`; the node `key` is `agentTrustKey(agent)`; a short-DID nameplate renders on both the WebGPU (`Html`) and non-WebGPU (`Text`) label paths.
- `client/src/features/bots/types/BotsTypes.ts` — `BotsAgent.did_nostr?: string`.

### 5. Canary
`CANARY-VC-COM14-DID` is already registered by the previous commit's harness seed (`src/services/liveness_harness.rs:42` `P0_CANARIES`). Registered + fired-in-test (the verifier unit tests exercise the fire predicate: a verified `did:nostr` at selection). Live Schnorr round-trip against a running agentbox: **pending-live-session**.

## Receipts

```
$ git rev-parse HEAD
6f4eb1b0aeaa2b30f6959c9caa3c0a3eb485424d
$ date -u '+%Y-%m-%dT%H:%M:%SZ'
2026-07-08T10:56:26Z

$ cargo test -p visionclaw-domain did_nostr
running 1 test
test types::mcp_responses::tests::agent_carries_optional_did_nostr ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 190 filtered out

$ cargo test -p visionclaw-server --lib services::nostr_identity_verifier
    Finished `test` profile [optimized + debuginfo] target(s) in 1m 14s
running 4 tests
test services::nostr_identity_verifier::tests::rejects_non_did_payload_before_crypto ... ok
test services::nostr_identity_verifier::tests::rejects_tampered_signature ... ok
test services::nostr_identity_verifier::tests::accepts_matching_did_over_real_schnorr_signature ... ok
test services::nostr_identity_verifier::tests::rejects_payload_did_for_a_different_key ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 717 filtered out

$ npx vitest run src/features/visualisation/components/agentIdentity.test.ts
 ✓ src/features/visualisation/components/agentIdentity.test.ts (3 tests) 4ms
 Test Files  1 passed (1)
      Tests  3 passed (3)

$ npx tsc --noEmit   # filtered to touched files
NO TYPE ERRORS IN TOUCHED FILES (agentIdentity.ts / AgentNodesLayer.tsx / BotsTypes.ts)
```

The full `visionclaw-server` monolith compiled to a finished `test` profile (1m 14s), proving all five root-crate edits (`bots_client.rs`, `agent_visualization_protocol.rs`, `mcp_tcp_client.rs`, `agent_monitor_actor.rs`, `bots_visualization_handler.rs`) plus the verifier type-check across the whole crate.

## Honest residual

- The verifier is unit-tested with a real secp256k1 keypair and a real Schnorr signature (accept + key-mismatch + tampered-sig + non-DID cases). The end-to-end challenge/response against a **live** agentbox that mints a DID at spawn is **pending-live-session** — the standing `CANARY-VC-COM14-DID` fires on that live selection, not on the unit test.
- Verification badge state in the client HUD shows the DID identity (short pubkey suffix); a client-side "verified" affirmation depends on the live challenge wire and is not asserted here.
