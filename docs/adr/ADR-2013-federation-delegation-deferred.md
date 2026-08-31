---
id: ADR-2013
title: Federated (OIDC) and delegated-agent (NIP-26) identity are both deferred; the secp256k1 signature chain is the sole realm
date: 2026-08-31
decision_status: accepted
implementation_status: none
activation_status: inactive
supersedes: []                   # legacy ADR-040/ADR-094/ADR-081 distilled — not in this tree; see lineage
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: an enterprise deployment requiring OIDC/SAML, or a decision to wire NIP-26 agent-on-behalf-of-user delegation
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: Distils legacy ADR-040 (enterprise identity — OIDC-alongside-Nostr, half superseded by ADR-142, half unbuilt) + ADR-094 (NIP-26 phone delegation, frozen 2026-07-03) + ADR-081 (federation key custody, frozen same date).
---

# ADR-2013 — Federated (OIDC) and delegated-agent (NIP-26) identity are both deferred; the secp256k1 signature chain is the sole realm

## Context

ADR-040 imagined OIDC alongside Nostr; ADR-142 superseded half of it, the rest was never built.
ADR-094 (NIP-26 phone delegation) and ADR-081 (federation key custody) were both frozen
2026-07-03. There is no enterprise-federation realm and no agent-on-behalf-of-user attribution
today. The forum bridge re-signs events under its own key, carrying no original-user authority.
Leaving these as unstated gaps risks code assuming a server-minted or IdP-federated identity.

## Decision

No OIDC/SAML/LDAP/SCIM auth realm exists, and NIP-26 delegation is unwired. Enterprise
federation and agent-on-behalf-of-user attribution are deliberately absent: identity is a
secp256k1 pubkey only until later phases land. This deferral is itself a constraint — no code may
assume a server-minted or IdP-federated identity, nor treat a bridged/agent-signed event as
carrying the original user's authority. It forecloses building features that depend on federated
claims or delegated attribution before those realms are designed and ratified.

## Consequences

- Enterprise SSO is unavailable; onboarding is pubkey-only, which limits large-org adoption until
  a federation ADR lands.
- The forum bridge's events are attributable to the bridge key, not to a user; downstream code
  must not infer user identity from a bridged event.
- When delegation is built, NIP-26 tags must be verified and mapped to authority explicitly —
  this ADR marks that as not-yet-done, not as forbidden forever.
- Merges IDENTITY B5 (OIDC parked) + B6 (NIP-26 deferred).

## Verification

Re-checked at `e0f8cd896`: no OIDC auth verifier exists in `src/` — a grep for
`OidcAuthVerified`/OIDC auth constructs finds only the `solid:oidcIssuer` RDF predicate string in
`src/handlers/solid_proxy_handler.rs:1015` (a Turtle literal, not a verifier) and a stale
claude-flow log, no verifier code. `src/services/nostr_bridge.rs:169` signs via
`sign_with_keys(&self.keys)` where `keys` is the bridge's own `Keys` (`:29`, constructed `:65`),
confirming the bridge carries no original-user authority. Governing doc:
`docs/IDENTITY-authority-chain.md` marks OIDC parked and delegated-agent-signed NOT IMPLEMENTED.
