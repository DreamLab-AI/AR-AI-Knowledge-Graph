---
title: "PRD-017: Nostr Mobile Agent Bridge"
status: Proposed
date: 2026-06-02
author: jjohare
priority: P1
---

# PRD-017: Nostr Mobile Agent Bridge

## 1. Problem

The agentbox mobile interaction surface is Telegram-only. Claude Telegram Mirror (CTM) mirrors agent activity to a Telegram supergroup and injects replies back into the running Claude Code session via `tmux send-keys`. This works but binds the operator's most privileged control channel to a centralised, non-sovereign messaging provider whose only authorisation gate is a single Telegram `chat_id` match (`telegram_handlers.rs:11-15`). The DreamLab ecosystem is otherwise built on a `did:nostr` pubkey identity model, self-sovereign Solid pods, and a private Nostr relay — none of which the mobile bridge uses.

Three concrete deficits:

1. **Identity divergence.** CTM authorises by Telegram chat membership, not by the `did:nostr` keys that gate every other ecosystem surface. The `IdentityStore` already maps `did:nostr:<pubkey>` → Telegram ID with roles (`identity.rs:91-107`) but `is_allowed()` is never called in the message path (`telegram_handlers.rs`) — auth is "anyone in the group".
2. **No sovereign session record.** CTM generates no session-level summaries (it only forwards Claude Code's own `transcript_summary` from `Stop` hooks — `hook.rs:360-393`). There is no durable, user-owned record of what an agent did during a session. "Manage sessions via summaries" is not a feature that exists today.
3. **No path to the ecosystem relay.** The agentbox embedded `nostr-rs-relay` (port 7777) stores kind-1059 gift wraps but never decrypts them (deferred at ADR-009:262). The DreamLab CF private relay (`wss://dreamlab-nostr-relay.solitary-paper-764d.workers.dev`) is whitelist-gated and federation-capable but runs `MESH_MODE = "standalone"` with no active bridge.

This PRD specifies a Nostr-native mobile bridge: a rich interactive surface from an Android Nostr client, through the DreamLab relay substrate, into agentbox agent intelligence — permissioned against the agentbox admin pubkey, with sessions recorded as user-owned summaries on the Solid pod.

## 2. Goals

| ID | Goal | Success Metric |
|----|------|----------------|
| G1 | Operator can hold an ad-hoc chat with an agentbox agent from an off-the-shelf Android Nostr client | Round-trip message (phone → agent → phone reply) completes over NIP-17 on the agentbox embedded relay |
| G2 | Every chat session produces a durable, user-owned summary | A kind-30840 summary event is published AND a JSON-LD summary is written to the operator's Solid pod `/sessions/` on session end |
| G3 | All inbound control is authorised against the admin pubkey | No message from a non-admin/non-delegated pubkey reaches agent dispatch; enforced at the relay-consumer boundary, signature-verified |
| G4 | Phone key custody does not expose the agentbox identity key | Phone signs with its own keypair under a NIP-26 delegation; admin nsec never leaves the agentbox |
| G5 | Android client choice imposes zero Solid requirement | Recommended client speaks only Nostr (NIP-17/44/59/42/55); the agent writes the pod, never the phone |
| G6 | The bridge is forward-compatible with the CF private relay and forum | Phase 2 federation requires config changes only (no protocol redesign); kind allocation reserves forum-bound kinds |

## 3. Non-Goals

- **No forum integration in this phase.** The bridge does NOT read or write the `nostr-rust-forum` (Rust BBS) running on the DreamLab website. Forum interop is explicitly deferred to a later PRD. (User constraint, verbatim: "this does NOT communicate with the rust based forum … AT THIS TIME but will do in future.")
- **No replacement of CTM in Phase 1.** CTM remains operational. The Nostr bridge runs alongside it. CTM teardown is a Phase 2+ decision once parity is proven.
- **No NIP-46 remote signer (bunker) implementation.** NIP-46 is absent across all four codebases (research 04 §6.3); building it is out of scope. NIP-55 (Android signer intent via Amber) provides equivalent key isolation without the engineering cost.
- **No tmux-injection control parity in Phase 1.** Free-text chat and structured tasks are in scope. The synchronous tool-approval gate, `stop`/`kill` keys, and slash-command forwarding (CTM capabilities 9-12) are Phase 2 — they require a co-located injector and a blocking RPC over Nostr (research 01, "minimum viable" §).
- **No multi-operator / multi-tenant access.** Single admin pubkey (plus its delegated phone key) only. Multi-user is deferred.
- **No Android app development.** We adopt an existing client, not build one.

## 4. Personas & Journeys

### Persona: The Operator (Dr John O'Hare)

Holds the agentbox admin pubkey. Works away from the desk and wants to (a) ask an agent to do something ad-hoc, (b) watch it work, (c) get a coherent summary of what happened that is his to keep.

### Journey A — Ad-hoc chat

1. Operator opens Amethyst on Android, selects the agentbox agent's npub.
2. Types: "Check the build status on the visionclaw main branch and summarise failures."
3. Amethyst hands the unsigned event to Amber (NIP-55 intent); Amber signs a NIP-17 gift-wrapped DM with the phone's delegated key and publishes to the agentbox embedded relay.
4. agentbox relay-consumer decrypts the gift wrap, verifies the sender is the admin (directly or via NIP-26 delegation), and dispatches the prompt to an agent.
5. The agent works; turn output streams back as NIP-17 DMs to the phone.
6. Operator reads the reply in Amethyst. Conversation continues ad-hoc.

### Journey B — Session summary & management

1. At session end (or checkpoint), the agent composes a structured summary: title, what was done, tool-call count, tokens, outcome.
2. The agent publishes a kind-30840 addressable session-summary event (`d` = session id) to the relay — readable by any Nostr client.
3. The agent writes the same summary as JSON-LD to the operator's Solid pod at `/sessions/<iso-date>-<session-id>.jsonld` under its mandate (NIP-98).
4. Operator browses recent sessions in Amethyst (kind-30840 events render as a feed) and, when at a desktop, has the canonical durable record in his pod.

## 5. Functional Requirements

### 5.1 Android client & signer

| ID | Requirement |
|----|-------------|
| F1 | **Recommended stack: Amethyst (client) + Amber (NIP-55 signer).** Amethyst is the most feature-complete Android Nostr client with full NIP-17/44/59 support; Amber holds the key and signs via Android intents, so the key never enters the client process (research 06). |
| F2 | **Fallback stack: 0xchat + Amber.** 0xchat is DM-first with strong NIP-17 support; same Amber signer. Selected if Amethyst's broad feed surface is undesirable for a focused control channel. |
| F3 | The client MUST support NIP-17 (kind 14/15), NIP-44 v2 encryption, NIP-59 gift wrap (kind 1059), NIP-42 relay AUTH (kind 22242), and NIP-55 external signing. All three candidate clients do. |
| F4 | No Solid/WAC/LDP capability is required of the client (research 05 §6). |

### 5.2 Messaging substrate (NIP scheme)

| ID | Requirement |
|----|-------------|
| F5 | **Ad-hoc chat uses NIP-17** (kind 14 chat / kind 15 file), sealed (kind 13) and gift-wrapped (kind 1059) per NIP-59, encrypted with NIP-44 v2. This is the primary transport. |
| F6 | **Structured tasks use NIP-90** (DVM: kind 5xxx request / 6xxx result / 7000 feedback) where the operator wants a typed job rather than free chat. Optional in Phase 1; the chat path (F5) is the MVP. |
| F7 | **Admin control panels use ACSP** (NIP-33 addressable kinds 31400-31405: PanelDefinition/State/ActionRequest/Response/PanelUpdate/PanelRetired). Phase 2 — provides the structured approve/reject surface that replaces CTM's inline-button approval gate. |
| F8 | **Session summaries use kind-30840** (NIP-33 addressable, `d` = session id). Schema in §5.4. |
| F9 | NIP-04 legacy DMs (kind 4) MUST NOT be used — no per-recipient isolation on the CF relay (research 03 §4) and deprecated by NIP-17. |

### 5.3 Relay topology & permissioning

| ID | Requirement |
|----|-------------|
| F10 | **Phase 1 transport is the agentbox embedded relay** (`nostr-rs-relay` 0.9.0, port 7777). The phone connects directly. This relay MUST be reachable from the phone (today it is loopback/internal — exposure is a Phase 1 work item) and MUST allowlist the phone pubkey for NIP-42 AUTH. |
| F11 | **The agentbox relay-consumer MUST decrypt inbound kind-1059 gift wraps.** Today they are stored but never decrypted (deferred at ADR-009:262). The decryption primitives already exist, implemented and e2e-tested, in `nostr-bbs-core` (`gift_wrap.rs` `unwrap_gift`, `nip44.rs` `decrypt`/`conversation_key`; roundtrip covered by `e2e_auth_flow.rs`) and are exposed to JavaScript via `wasm_bridge.rs`. The net-new work is the **agentbox call site** that invokes them at the dispatch boundary — not the cryptography. |
| F12 | **Inbound authorisation is admin-pubkey gated.** A message is dispatched to an agent only if the unwrapped rumor's author pubkey equals `AGENTBOX_X_ONLY_PUBKEY_HEX`, an entry in `admin_pubkeys` (`agentbox.toml:159`), or a key bearing a valid NIP-26 delegation from one of those. Enforced after signature verification, before dispatch. |
| F13 | **Phone key custody is NIP-26 delegated (Option B, research 04 §7.2).** The admin key signs a delegation token (`kind=14&kind=1059&created_at>T0&created_at<T1`) authorising the phone's independent keypair. The phone gets its own `did:nostr` and is a first-class mesh identity; the admin nsec stays on the agentbox. Delegation validation (`validate_delegation_tag`, `nip26.rs:258`) MUST be wired into the relay-consumer dispatch path (currently unwired — research 04 §4.2). |
| F14 | **Phase 2 routes via the CF private relay.** Switch `MESH_MODE` from `standalone` to `federated`, add the agentbox relay URL to `peer_relays`, add the agentbox `did:nostr` to `allowed_remote_dids`, and ensure NIP-17/59 kinds and kind-30840 are in `federated_kinds` (research 03 §7). No protocol change; config + a federation forwarder only. |

### 5.4 Session summary

| ID | Requirement |
|----|-------------|
| F15 | On session end, the agent MUST publish a kind-30840 event and write a pod JSON-LD summary. Both carry the same logical content. |
| F16 | kind-30840 event schema (addressable; `d`-tag dedupes per session): |

```jsonc
{
  "kind": 30840,
  "pubkey": "<agent_pubkey>",
  "tags": [
    ["d", "<session_id>"],
    ["p", "<admin_pubkey>"], ["p", "<phone_pubkey>"],
    ["agent", "<agent_pubkey>"],
    ["relay", "<wss://relay_url>"],
    ["start", "<unix_ts>"], ["end", "<unix_ts>"],
    ["status", "active|complete|failed"],
    ["t", "session-summary"],
    ["alt", "Agent session summary"]
  ],
  "content": "{\"title\":...,\"summary\":...,\"tool_calls\":<int>,\"tokens_used\":<int>,\"outcome\":...}"
}
```

| ID | Requirement |
|----|-------------|
| F17 | The pod summary is a JSON-LD resource at `/sessions/<iso-date>-<session-id>.jsonld` in the operator's pod, carrying `owner_did: did:nostr:<admin_pubkey>`, an `action_urn` to the agentbox activity record, start/end timestamps, structured work summary, and URNs of resources created/modified (research 05 §7). |
| F18 | The agent writes the pod under its OWN `did:nostr` via NIP-98 (`buildPodNip98`, `pod-signer.js:76-80`), backed by a one-time user-installed mandate ACL on `/sessions/` (`mandate.js:137-152`). The operator's nsec is never used or exposed. |
| F19 | Summary content SHOULD be generated by an LLM pass over the session transcript/tool record (CTM has the raw material but no session-summary generator — research 01 §5). For Phase 1, Claude Code's own `transcript_summary` from the `Stop` hook is an acceptable source. |

### 5.5 Outbound mirroring (agent → phone)

| ID | Requirement |
|----|-------------|
| F20 | Agent turn output (assistant text) MUST be delivered to the phone as NIP-17 DMs. This is the parity-critical outbound capability (CTM capability 3). |
| F21 | Session start/end notifications MUST be delivered (CTM capabilities 6). |
| F22 | Tool-activity mirroring (start/result one-liners) and sub-agent notifications are OPTIONAL in Phase 1 (verbose mode), behind a per-session mute toggle. |

## 6. Solid Pod Interaction Decision

**Decision: Hybrid (research 05 §5, Option C). Nostr is the live transport; the Solid pod is the durable record. The agent writes the pod; the phone never touches Solid.**

Rationale:
- **Durability is already provided by the relay, not the pod.** The DreamLab CF relay is a Cloudflare Worker + Durable Object (`nostr-bbs-relay-worker/src/relay_do/`); a Durable Object is transactional, durable storage, so chat history persists on the relay. Agentbox does NOT need its own message store. (NIP-17 is conventionally ephemeral/pruned — retaining kind-1059 on the DO is a config flag, not a build.)
- The pod's value is therefore **self-sovereign ownership and export**, not mere durability: WAC-enforced, user-owned, queryable, exportable session history with no vendor lock-in (research 05 §5 Option A). The operator owns the pod; the CF relay is operator-controlled infrastructure but not the user's sovereign store.
- Requiring the Android client to speak Solid would constrain client choice to a near-empty set (research 05 §6). The agent already has a `did:nostr`, NIP-98 signing, and a mandate-backed WAC grant — it is the correct writer.

Persistence and decryption are orthogonal: a kind-1059 gift wrap is encrypted **to the agent pubkey**, so only the agent (holding the nsec) can unwrap it — the CF relay stores it but cannot read it. Storage location (CF Durable Object) does not move the decryption (agent-side, agentbox).

This decision is what decouples the Android client choice from the sovereignty requirement: **the client choice is purely a Nostr-capability question.**

## 7. Non-Functional Requirements & Threat Model

Full detail in research report 08; the load-bearing NFRs and the blocking prerequisites are reproduced here as acceptance gates.

### 7.1 Blocking security prerequisites (MUST fix before ship)

All three primitives already exist, implemented and tested, in `nostr-bbs-core` / `solid-pod-rs`. The "blocking" work is **consuming and wiring** them on the agentbox side and proving interop — not building cryptography. Each row below names the existing implementation and the residual integration task.

| ID | Prerequisite | Existing implementation | Residual (net-new) work |
|----|--------------|-------------------------|-------------------------|
| BLOCK-1 | **NIP-44 v2 HKDF correctness** — spec-conformant ciphertext (HKDF salt/info, ChaCha20 + HMAC-SHA256). | `nostr-bbs-core/nip44.rs` (`encrypt`/`decrypt`/`conversation_key`), benched in `benches/bench_nip44.rs`, roundtrip in `e2e_auth_flow.rs`, NIP-44 v2 conformance fixtures pinned. Exposed to JS as `nip44_encrypt`/`nip44_decrypt` (`wasm_bridge.rs`). | Consume the crate; prove a live Amethyst/Amber round-trip against the published vectors. No crypto to author. |
| BLOCK-2 | **NIP-98 payload-hash + zero-pubkey hardening** — `payload` tag = SHA-256(body); reject the all-zero pubkey. | `solid-pod-rs/.../auth/nip98.rs` (`verify_schnorr_signature`, `verify_nip98_proof`) + `wasm_bridge.rs` `create_nip98_token`/`verify_nip98_token`; payload-hash + mismatch paths asserted in `e2e_auth_flow.rs:180-278`. | Route the agentbox NIP-98 path through the canonical verifier instead of structural-only checks. |
| BLOCK-3 | **Governance/delegation signer verification** — NIP-26 delegation signature-checked before any admin action. | `nostr-bbs-core/nip26.rs` (`validate_delegation_tag`, unit-tested). | Wire the existing validator into the relay-consumer dispatch path. Wiring, not crypto. |

### 7.2 Key NFRs

| ID | NFR | Target |
|----|-----|--------|
| NFR-1 | Inbound message → agent dispatch latency | < 2 s p95 (relay decrypt + auth + dispatch) |
| NFR-2 | All inbound events signature-verified before any side effect | 100%; no unsigned/forged event reaches dispatch |
| NFR-3 | Kind-1059 reads are recipient-isolated | Phone receives only events with its pubkey in `#p` (relay-enforced for CF; agentbox relay must match) |
| NFR-4 | Delegation token revocation | Effective within the token's bounded window; compromise mitigated by short windows + reissue |
| NFR-5 | Pod write auditability | Every summary write is a NIP-98-signed, pod-logged action traceable to the agent `did:nostr` |
| NFR-6 | No secret material in logs | Token/key scrubbing on all log paths (CTM's `ScrubWriter` is the pattern) |

### 7.3 QE / test strategy

- **15-row auth matrix (AUTH-01..15)** and **12 negative tests (NEG-01..12)** from research 08 are the acceptance suite for F12/F13 (admin gating, delegation validity/expiry, forged signatures, replay).
- **3 BDD feature blocks** (Ad-hoc Chat, Session Summary Generation, Admin Permission Enforcement) from research 08 are the executable specification.
- **Mobile-surface tool allowlist/denylist:** the agent dispatched from a phone message runs under a restricted tool set (no destructive ops without Phase 2 approval gate). Defined in research 08.
- **NIP-44 interop vectors:** test against the published NIP-44 v2 test vectors AND a live Amethyst/Amber round-trip before BLOCK-1 is closed.

## 8. Phased Rollout

### Phase 1 — Agentbox-direct bridge (this PRD)

```
Android (Amethyst + Amber, NIP-26 delegated key)
   │  NIP-17 gift-wrapped DM
   ▼
agentbox embedded relay (port 7777, phone pubkey allowlisted, NIP-42 AUTH)
   │
   ▼
relay-consumer  ── unwrap kind-1059 via nostr-bbs-core (WIRE CALL-SITE)  ── verify sig + admin/delegation auth
   │
   ▼
agent dispatch (AGENTBOX_INTENT_COMMAND)
   │
   ├─► NIP-17 reply DMs → phone
   ├─► kind-30840 session summary → relay
   └─► JSON-LD summary → operator Solid pod (NIP-98, agent mandate)
```

Build items (the crypto already exists in `nostr-bbs-core`/`solid-pod-rs`; these are integration tasks):
1. **Expose + allowlist** the embedded relay for the phone (private overlay — open question 1).
2. **Wire the kind-1059 unwrap call-site** (F11): invoke `nostr-bbs-core` `unwrap_gift`/`nip44_decrypt` at the dispatch boundary, consumed via the WASM bridge. Add the one missing `#[wasm_bindgen]` shim for `gift_wrap`/`unwrap_gift` (impl exists; not yet re-exported).
3. **Wire NIP-26 delegation validation** (F13): call the existing `validate_delegation_tag` in the dispatch path.
4. **Build the session-summary generator + dual writer** (F15-F19) — the genuinely new component; the publish/sign/pod-write primitives exist.
5. **Verify BLOCK-1/2/3** as interop/wiring acceptance (§7.1), not crypto authoring.

### Phase 2 — CF private relay onward bridge

Activate mesh federation (F14): `MESH_MODE=federated`, agentbox relay in `peer_relays`, agentbox `did:nostr` in `allowed_remote_dids`, NIP-17/59 + kind-30840 in `federated_kinds`. Add the synchronous tool-approval gate via ACSP panels (F7) for CTM control parity. Build the relay-to-relay forwarder (the CF relay has no outbound fetch today — research 03 §7).

### Phase 3 — Forum interop (deferred, separate PRD)

Bridge into the `nostr-rust-forum`. Out of scope here; the kind allocation in this PRD reserves forum-bound kinds so this is additive.

## 9. Open Questions

1. **Embedded relay exposure.** How is the agentbox relay (port 7777) reached from the phone in Phase 1 — Tailscale/WireGuard, a Cloudflare Tunnel, or direct? Affects NFR-1 and the threat surface. (Recommendation: private overlay network, not public exposure.)
2. **Inbound injection vs. fresh dispatch.** Does a phone message resume an existing Claude Code session (CTM's tmux-injection model, requires a co-located injector) or spawn a fresh agent task (`AGENTBOX_INTENT_COMMAND`)? Phase 1 favours fresh dispatch; tmux-injection parity is Phase 2.
3. **Summary generation model.** Use Claude Code's `transcript_summary` (free, present today) or a dedicated Haiku summarisation pass (richer, costs tokens)? Phase 1 defaults to `transcript_summary`.

## 10. References

- Research evidence base: `docs/integration-research/nostr-mobile-bridge/01-08`
- Companion ADRs: ADR-092 (client+signer), ADR-093 (messaging substrate), ADR-094 (admin permission + delegation), ADR-095 (session summary), ADR-096 (Solid pod boundary), ADR-097 (relay topology)
- Companion DDD: `docs/ddd-nostr-mobile-bridge-context.md`
- Prior art: ADR-009 (gift-wrap deferral), ADR-074 (did:nostr canonicalisation), ADR-017 (pod path convention), PRD-010 (mesh federation)
