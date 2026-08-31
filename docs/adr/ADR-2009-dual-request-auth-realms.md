---
id: ADR-2009
title: Two request-auth realms coexist — NIP-98 signatures and login-derived session bearers
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [src/utils/auth.rs, src/services/nostr_service.rs, src/middleware/rbac_gate.rs, client/src/services/api/authInterceptor.ts]
owner: jjohare
review_trigger: React client migrating to per-request NIP-98 signing, or any multi-tenant deployment where session-bearer mutations are unacceptable
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: "legacy ADR-011, ADR-142; corrects the aspiration that NIP-98 is the sole request-auth realm"
---

# ADR-2009 — Two request-auth realms coexist — NIP-98 signatures and login-derived session bearers

## Context

The original draft of this record claimed NIP-98 was the *sole* request-auth
realm. Adversarial verification refuted it: `verify_access`
(`src/utils/auth.rs`), which `RbacGate` consults for **every** `/api` route
including `WriteGraph` mutations (`rbac_gate.rs:265`), carries an unconditional
legacy fallback authenticating `X-Nostr-Pubkey` + `X-Nostr-Token` headers via
`validate_session` (`src/services/nostr_service.rs:478`). The React client
actively uses this path (`client/src/services/api/authInterceptor.ts` et al.),
so it cannot be removed without breaking the desktop client.

## Decision

Both realms are accepted as live, with an explicit quality ordering. **NIP-98**
(Schnorr per-request signatures + freshness window + single-use replay cache,
ADR-2002) is the primary realm and the only one the XR client and agents use.
**Legacy session bearers** (UUID minted at Schnorr-verified login, expiring
`token_expiry` after `last_seen`, plain-equality check) remain accepted on REST
for the browser client's benefit. This forecloses pretending the estate has
signature-grade non-replayability on the REST surface: a captured session
header pair is replayable until expiry, unlike a NIP-98 token.

## Consequences

- The RBAC lattice binds to the pubkey regardless of realm, so role enforcement
  is uniform; only the *transport credential* strength differs.
- ADR-2002's replay guarantees apply to the NIP-98 realm only — documentation
  and security reviews must not claim them for the whole REST surface.
- Retiring the legacy realm requires the React client to sign per-request
  (NIP-98 or equivalent) first; that migration is the recorded exit path (see
  review_trigger).
- `docs/IDENTITY-authority-chain.md` must present both realms (it does).

## Verification

`src/utils/auth.rs` ~218-260 legacy branch confirmed unconditional and
non-cfg-gated at `e0f8cd896`; `validate_session` expiry window confirmed at
`nostr_service.rs:478-488`; client dependence confirmed across five
`client/src` files (authInterceptor, restClient, endpoints, ldpClient,
contextLoader). Original sole-realm draft deleted by the adversarial verify
pass of wf_0d0794b9-02c; this record replaces it stating the dual-realm truth.
