# ADR-092 — Android Nostr Client and Signer for the Mobile Agent Bridge

| Field | Value |
|-------|-------|
| Status | Deferred — frozen by closeout 2026-07-03 (was Proposed 2026-06-02). Android Nostr client/signer deferred: mobile-bridge cluster parked (standalone-first). |
| Drives | PRD-017 §5.1 (F1-F4) |
| Companion ADRs | ADR-093 (messaging substrate), ADR-094 (permissioning), ADR-096 (pod boundary) |
| Affected repos | `agentbox` (relay exposure, consumer); operator device (client install) |
| Evidence | `docs/integration-research/nostr-mobile-bridge/06-android-client-landscape.md` |

## Context

The mobile bridge needs an Android Nostr client to be the operator's interactive surface into agentbox agents. The client must support encrypted DMs (NIP-17), relay AUTH (NIP-42), and — critically — must not require the operator to paste their nsec into an app process that also renders untrusted feed content. The ecosystem is `did:nostr`-keyed; the client is a Nostr citizen, not a Solid citizen (research 05 §6 establishes that the pod-write boundary is on the agent side, so the client has no Solid requirement).

13 Android clients were surveyed (research 06). The discriminators that matter: full NIP-17/44/59 support (not legacy NIP-04), NIP-42 AUTH (the agentbox and CF relays both challenge on connect), and NIP-55 external-signer support (key isolation).

## Decision

### D1 — Primary stack: Amethyst + Amber

Adopt **Amethyst** as the recommended Android client and **Amber** as the NIP-55 external signer. Amethyst has the most complete NIP-17/44/59 implementation of any Android client and native NIP-55 signer delegation. Amber holds the private key in a separate process and signs via Android `Intent`; the key never enters Amethyst's address space.

### D2 — Key isolation via NIP-55, not NIP-46

Use NIP-55 (on-device Android signer intent) for key isolation. Do NOT implement NIP-46 (remote bunker). NIP-46 is absent from all four ecosystem codebases (research 04 §6.3) and would be a from-scratch build with a per-request relay round-trip on the latency path. NIP-55 gives equivalent custody isolation (the key lives in Amber, not the client) with zero new server-side code.

### D3 — Fallback stack: 0xchat + Amber

If a DM-focused surface is preferred over Amethyst's broad social feed, **0xchat** is the sanctioned fallback. It is DM-first with strong NIP-17 support and uses the same Amber signer, so D2 holds unchanged. Coracle (PWA) is a documented browser-based alternative but is not the recommendation (no native NIP-55).

### D4 — No bespoke Android development

We adopt an existing client. No custom Android app is built. This bounds scope to relay/consumer/agent-side work.

## Consequences

**Positive:**
- Key custody is isolated from feed-rendering code (NIP-55).
- Client choice is fully decoupled from Solid (research 05 §6) — no special client capability needed.
- Zero Android engineering; all build effort is server-side (PRD-017 §8 Phase 1 items).

**Negative / risks:**
- Amethyst renders a broad social feed; for a focused control channel this is noise. Mitigated by D3 (0xchat fallback) or by using Amethyst's DM view exclusively.
- We depend on third-party client NIP-44 v2 conformance. This is why PRD-017 BLOCK-1 mandates a live Amethyst+Amber round-trip against the NIP-44 v2 test vectors before ship.
- Amber must be installed and the delegation key provisioned into it (ADR-094 D-series covers the delegation; the operator imports the delegated key into Amber).

## Alternatives considered

- **Phone holds the raw admin nsec in the client** (research 04 §7.1, Option A): rejected — single point of failure for the whole agentbox identity; key loss forces full identity rotation including pod ACLs and DID docs.
- **NIP-46 bunker on the phone** (Option C): rejected per D2 — no ecosystem code, latency cost, not on any roadmap.
- **Damus/Primal/Nostros etc.**: surveyed (research 06); weaker NIP-17 or no NIP-55. Not selected.
