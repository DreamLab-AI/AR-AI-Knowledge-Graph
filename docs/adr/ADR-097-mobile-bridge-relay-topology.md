# ADR-097 — Mobile Bridge Relay Topology and Phased Federation

| Field | Value |
|-------|-------|
| Status | Deferred — frozen by closeout 2026-07-03 (was Proposed 2026-06-02). Mobile-bridge relay topology deferred with the cluster. |
| Drives | PRD-017 §5.3 (F10-F11, F14), §8 (phasing) |
| Companion ADRs | ADR-093 (substrate), ADR-094 (permissioning), ADR-009 (gift-wrap deferral), PRD-010 (mesh federation) |
| Affected repos | `agentbox` (embedded relay exposure, relay-consumer), `nostr-rust-forum` (CF relay-worker, federation forwarder) |
| Evidence | `docs/integration-research/nostr-mobile-bridge/02-agentbox-nostr-substrate.md`, `03-cf-private-relay.md` |

## Context

Two relays exist: the agentbox embedded `nostr-rs-relay` 0.9.0 (port 7777), which can communicate with the local `did:nostr` agent substrate; and the DreamLab CF private relay (`wss://dreamlab-nostr-relay.solitary-paper-764d.workers.dev`), a whitelist-gated Cloudflare Worker + Durable Object with a complete-but-dormant mesh-federation framework (`MESH_MODE = "standalone"`, `peer_relays = []`, research 03 §7).

The user's constraint — "this does NOT communicate with the rust forum AT THIS TIME but will do in future" — implies phasing: get phone↔agent working on the nearest relay now; route via the CF relay and (eventually) the forum later. The CF relay has no outbound fetch on event ingest today (research 03 §7) — a real bridge needs a forwarder built.

## Decision

### D1 — Phase 1 transport is the agentbox embedded relay (direct)

The phone connects directly to the agentbox embedded relay (port 7777). Rationale: shortest path, fewest moving parts, no dependency on CF federation that does not yet exist. The agentbox relay already speaks to the local agent substrate; this is the minimum viable phone→agent→reply path (research 02 §"minimum viable").

Phase 1 work items on this relay:
- **Expose + allowlist.** The relay must be reachable from the phone and must allowlist the phone pubkey for NIP-42 AUTH. Exposure SHOULD be via a private overlay (Tailscale/WireGuard or a Cloudflare Tunnel), NOT public internet (PRD-017 open question 1; minimises threat surface).
- **Unwrap kind-1059 (wire the call-site).** Today the relay-consumer stores gift wraps but never decrypts them (ADR-009:262 deferral). The fix is to invoke the existing `nostr-bbs-core` unwrap path (`unwrap_gift`/`nip44_decrypt`, e2e-tested per ADR-093 D6) at the dispatch boundary — wiring a call-site, not building crypto. This is the critical-path Phase 1 integration, but the cryptographic substrate already exists.
- **Recipient isolation.** The embedded relay MUST match the CF relay's kind-1059 `#p`-isolation semantics (research 03 §3) so NFR-3 holds in Phase 1.
- **No durable chat store in agentbox.** The agent unwraps and dispatches; it does NOT build a durable chat-transcript store. The Phase 1 durable record is the kind-30840 session summary + pod resource (ADR-095, ADR-096), not the raw chat. Where durable chat is wanted (Phase 2), the CF relay's Durable Object (`nostr-bbs-relay-worker/src/relay_do/`) already persists kind-1059 transactionally; NIP-17 DMs are conventionally ephemeral, so DO retention of kind-1059 is a config flag, not a build.

### D2 — Phase 2 routes via the CF private relay (federation, config-driven)

Activate the dormant mesh framework rather than redesign. Switch the CF relay `MESH_MODE` standalone→federated, add the agentbox relay URL to `peer_relays`, add the agentbox `did:nostr` to `allowed_remote_dids` / `MESH_ALLOWED_REMOTE_DIDS`, and ensure NIP-17/59 kinds and kind-30840 are in `federated_kinds` (research 03 §7). The current `federated_kinds` already includes 1, 1059, and the governance band — kind 14/15 and 30840 must be added.

### D3 — Build the federation forwarder (Phase 2, net-new)

The CF relay has no outbound HTTP/WS calls on ingest (research 03 §7, confirmed absent). Federation requires either extending `handle_event` (`nip_handlers.rs`) to POST matching events to the peer relay after D1 storage, or a separate CF Worker cron that subscribes as a client and republishes. This is net-new in `nostr-rust-forum`. It is the only protocol-level Phase 2 build; everything else is config (D2).

### D4 — Forum interop is Phase 3, out of scope here

No reads/writes to `nostr-rust-forum` BBS in Phase 1 or 2. The kind allocation (ADR-095 D4) reserves forum-bound kinds so Phase 3 is additive. (User constraint.)

### D5 — Provisioning the phone pubkey

In Phase 1 the phone pubkey is added to the agentbox embedded relay's allowlist by the operator. In Phase 2, to also write to the CF relay, an admin must call `POST /api/whitelist/add` with a NIP-98 admin token (research 03 §6) — invite redemption does NOT auto-add to the relay whitelist (research 03 §6, the two D1 tables are separate). Provisioning is explicit, admin-driven.

## Consequences

**Positive:**
- Phase 1 ships without depending on CF federation that does not exist yet.
- Phase 2 is mostly config flips on an already-built (dormant) framework — low protocol risk.
- Phasing honours the user's explicit "no forum yet" constraint while keeping the path open.

**Negative / risks:**
- The embedded relay must be exposed to the phone — the chief Phase 1 threat-surface decision (open question 1). Private overlay strongly preferred; public exposure of a privileged control relay is a hard no without further hardening.
- The Phase 2 forwarder (D3) is genuinely net-new in the forum repo and the only place Phase 2 touches `nostr-rust-forum` — scope it as forum-repo work, not agentbox.
- Two relays mean two allowlist provisioning steps (D5); operator onboarding must cover both when Phase 2 lands.
