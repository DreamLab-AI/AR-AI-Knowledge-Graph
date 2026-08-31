---
id: ADR-2022
title: Identity is a did:nostr hex pubkey — no agent URN kind, hex x-only canonical, npub display-only
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: a proposal to add an Agent URN kind, or to persist a bech32 npub inside any durable identifier
repo: visionclaw
domain: IDENTIFIER-taxonomy
lineage: legacy ADR-074/ADR-074-D2 (cross-system did:nostr canonicalisation), ADR-125 (did:nostr multikey convergence), ADR-050 (superseded owner-scoped npub path); converges with agentbox ADR-2011 hex-canonical ingress
---

# ADR-2022 — Identity is a did:nostr hex pubkey — no agent URN kind, hex x-only canonical, npub display-only

## Context

An agent needs a durable identity across the VisionClaw/agentbox federation
boundary. Two temptations exist: a dedicated `urn:visionclaw:agent:<pk>` kind,
and persisting the human-friendly bech32 `npub1...` form. Both would fork
identity into competing canonical strings and break byte-for-byte cross-substrate
joins. See `docs/IDENTIFIER-taxonomy.md` for the identity grammar.

## Decision

An agent's identity *is* its `did:nostr:<hex-pubkey>`; there is deliberately no
`Agent` variant in the `Kind` enum, and the agentbox translator collapses
`urn:agentbox:agent:<pk>` straight to `did:nostr:<pk>`. Every persisted URN, DID,
and owner scope uses the 64-char lowercase-hex BIP-340 x-only pubkey
(`is_pubkey_hex`); `did_nostr()` rejects any non-hex input. `npub1...` exists
only as a UI rendering (`UserContext.user_id`) and is never mixed into a
persisted identifier or an owner scope. This forecloses a second identity kind
and forecloses bech32 leaking into the durable store.

## Consequences

- Owner scopes, DIDs, and cross-repo joins compare as raw hex — no bech32
  decode step, no ambiguity about the canonical form.
- Any UI or API that only holds an npub must decode to hex before it can mint or
  scope; the decode is the caller's responsibility, not the minter's.
- Adding a genuine agent-as-resource URN later would require reopening this
  decision, not merely extending the enum.

## Verification

Re-checked at `e0f8cd896`: `src/uri/mod.rs:51-69` — `Kind` enum with no `Agent`
variant; `:136-140` `is_pubkey_hex`; `:185-190` `did_nostr` rejects non-hex;
`cross_from_agentbox` `:534-537` maps `agent` → `did_nostr(pk)`.
`src/services/nostr_identity_verifier.rs:65` derives the DID from the verified
`event.pubkey`, gated at `:100-108`. `src/types/user_context.rs:16-20` documents
`user_id` (npub) as the display identifier with hex `pubkey` for verification.
