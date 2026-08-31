# ADR-096 — Solid Pod Persistence Boundary for the Mobile Bridge

| Field | Value |
|-------|-------|
| Status | Deferred — frozen by closeout 2026-07-03 (was Proposed 2026-06-02). Mobile-bridge pod persistence boundary deferred with the cluster. |
| Drives | PRD-017 §6, §5.4 (F17-F18) |
| Companion ADRs | ADR-095 (summary scheme), ADR-094 (permissioning), ADR-017 (pod path convention) |
| Affected repos | `agentbox` (pod-signer, mandate, summary writer), `solid-pod-rs` (pod server) |
| Evidence | `docs/integration-research/nostr-mobile-bridge/05-solid-pod-interaction.md` |

## Context

The ecosystem is built on self-sovereign Solid pods (one pod per `did:nostr`, WAC default-deny, research 05 §1). The question is whether — and how — the mobile bridge uses the pod, and specifically whether the Android client must speak Solid. NIP-17 DMs are ephemeral (relays prune them, research 05 §5 Option B); they are a transport, not a persistence substrate. Conversely, off-the-shelf Android Nostr clients implement zero Solid Protocol/WAC/LDP (research 05 §6) — requiring it would shrink client choice to nearly empty.

## Decision

### D1 — Hybrid: Nostr transports, the pod owns

Live chat is Nostr (ADR-093). Durable session records are Solid pod resources. This is research 05 §5 Option C. Note the durability axis is *also* covered by the CF relay's Durable Object (it persists kind-1059 chat and kind-30840 summaries transactionally) — so the pod is not chosen for durability alone. The pod's decisive, non-substitutable property is **self-sovereign ownership**: the operator owns, controls (WAC), exports, migrates, and deletes pod resources, which a relay-operated Durable Object does not provide. On ownership, queryability, and offline resilience the pod remains superior for a production deployment; on raw durability the relay already suffices.

### D2 — The agent writes the pod; the phone never does

The pod-write boundary is on the agent side. The agent already holds a `did:nostr`, NIP-98 signing capability (`buildPodNip98`, `pod-signer.js:76-80`), and (via a one-time user mandate) a WAC grant on the operator's `/sessions/` container. The phone has NO Solid responsibility. This is what decouples the Android client choice (ADR-092) from the sovereignty requirement — client choice becomes a pure Nostr-capability question (research 05 §6).

### D3 — Agent writes under its OWN identity, mandate-backed

The agent signs pod writes with its own nsec, not the operator's (`loadSigner(stack)`). The operator installs, once, a mandate ACL granting `acl:agent <did:nostr:AGENT_PUBKEY>` Read/Write/Append on `/sessions/` (`mandate.js:137-152`). The operator's nsec is never used or exposed by the agentbox process (research 05 §7). The mandate is a NIP-33 event (kind 30078), revocable by re-publishing with `revoked: true`; the WAC ACL is the enforcement point.

### D4 — Path and shape

Session summaries land at `/sessions/<iso-date>-<session-id>.jsonld` (research 05 §7), carrying `owner_did: did:nostr:<admin_pubkey>`, `action_urn` (link to the agentbox activity record), start/end timestamps, structured work summary, and URNs of resources created/modified. The `/sessions/` container is registered in the operator's `privateTypeIndex.jsonld` (private by default; `publicTypeIndex.jsonld` if the operator chooses to share session history).

### D5 — Optional read-back to Nostr

If the operator wants to browse session history from the phone (a Nostr client cannot read the pod directly), the agent already publishes the kind-30840 event (ADR-095 D1), which any Nostr client renders. The pod remains canonical; the relay event is the phone-readable projection. No Solid-aware mobile app is needed.

## Consequences

**Positive:**
- User owns, controls (WAC), can export/migrate/delete session history — full self-sovereignty (research 05 §7).
- Android client choice is unconstrained by Solid (the central insight of research 05 §6).
- Reuses existing agentbox pod-signer + mandate machinery; no new pod-server work.

**Negative / risks:**
- Requires the one-time mandate install by the operator before the agent can write. If absent, pod writes 403 (WAC default-deny). Onboarding must include mandate provisioning.
- The pod server (solid-pod-rs at loopback :8484, or JSS) must be reachable from the agent at session end. Already true in agentbox.
- Dual-write partial failure (shared with ADR-095) — retry is safe via deterministic path.
