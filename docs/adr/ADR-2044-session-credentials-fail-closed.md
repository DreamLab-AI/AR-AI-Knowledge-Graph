---
id: ADR-2044
title: Session credentials fail closed and expire identically on every transport
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a new WebSocket endpoint, or a concrete out-of-tree consumer needing the dev-gated ?token= path widened
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: ADR-2009 named the two request-auth realms; ADR-2026 set the fail-closed posture; legacy ADR-011 required header-only WS auth; ADR-2058 closes the query path on the socket_flow/fastwebsockets seams.
---

# ADR-2044 — Session credentials fail closed and expire identically on every transport

## Context

ADR-2009 names two request-auth realms: NIP-98 request signing, and session
bearer tokens. Phase 1 (diagrams VC-03.2, VC-03.5, VC-05.3) found the session
realm was not one credential but two, with different lifetimes and different
strength, depending on transport:

- `NostrService::get_session` — the lookup every WebSocket upgrade uses — scanned
  users for a matching `session_token` and returned the user with **no expiry
  check at all**. `validate_session`, used by the REST `X-Nostr-Token` path, did
  check `AUTH_TOKEN_EXPIRY`. A token was therefore bounded on REST and valid
  until logout or overwrite on a socket.
- `/ws/client-messages` checked only that a token was **non-empty**, never that
  it named a live session, so `?token=x` opened the socket.
- The same any-non-empty-string check exists in `mcp_relay_handler` and
  `multi_mcp_websocket_handler`, which reference `NostrService` nowhere at all.

## Decision

A session token means the same thing on every transport. One freshness rule,
`NostrService::session_is_fresh(last_seen, now, token_expiry)`, is shared by
`get_session` and `validate_session`, so the WebSocket and REST realms cannot
drift apart again. An empty token is rejected before any lookup. A `last_seen` in
the future — a clock stepped backwards — is treated as stale rather than as an
unbounded lease, by comparing the age on its absolute value.

Every WebSocket upgrade resolves its token through the session realm and fails
closed. Presence of a token is not authentication: an unknown token, an expired
token and an absent token all produce 401. When no `NostrService` is configured
the socket does **not** open, because there is nothing to validate against.

The `?token=` query parameter is **closed in release builds** and compiled in for
development only, behind `#[cfg(any(debug_assertions, feature = "dev-auth"))]`.
A session token in a query string reaches proxy and access logs, contradicting
legacy ADR-011's header-only stance; with no consumer that needs it, there is no
reason to keep the exposure in a shipped binary.

**This reverses an earlier draft of this record, and the correction matters.**
The first version deprecated the query path for one release on the stated grounds
that "XR and native clients cannot set headers on an upgrade". That premise was
asserted without checking and is false. vc-gpu-wire challenged it with evidence,
which was then verified independently: the Godot client authenticates
**post-connect**, sending `{"type":"authenticate","event":"<base64>"}` via
`nip98_authenticate_json` (`xr-client/rust/src/signer.rs:110-114`); the only
`token=` anywhere under `xr-client/rust/src/` is `signer.rs:323`, a URL literal
inside the test `nip98_event_is_well_formed_and_signature_verifies`, not auth
transport; and `client/src/services/` contains none. PHASE2 policy allows
DEPRECATE only for a real external consumer, and there is none in-tree.

## Consequences

- Sessions idle beyond `AUTH_TOKEN_EXPIRY` (default 3600s) now drop their
  WebSocket on reconnect where they previously persisted indefinitely. This is
  the intended behaviour change and it is visible to users as a re-login.
- `get_session`'s stricter behaviour propagates to every caller for free:
  `solid_proxy_handler`, the `/wss` upgrade in `socket_flow_handler`,
  `filter_auth`, and `agent_events::ingest`. None needed editing; all already
  treat `None` as unauthenticated. Routed to vc-gpu-wire so a suddenly-failing
  reconnect in testing is not mistaken for a regression in their work.
- The estate lead landed the same fix in `mcp_relay_handler.rs` and
  `multi_mcp_websocket_handler.rs` under **ADR-2090**, so the credential defect is
  now closed on every WebSocket path in the tree. Their note is worth recording:
  those two sockets were not merely openable with any string, they would have
  been openable **indefinitely with a real-but-stale token** — the `get_session`
  expiry gap is what turned an inconsistency into something exploitable.
- The legacy ADR-011 divergence is closed for release builds rather than carried.
  A deployment that was passing `?token=` to a release binary will now fail to
  open the socket — intended, and the reason the dev gate exists is so the seam
  survives for local work rather than vanishing.
- Should an out-of-tree consumer of the query path emerge, the fix is to widen
  the existing gate deliberately as an amendment, not to re-open it by default.
  That is this record's review trigger.

## Verification

`src/services/nostr_service.rs`: `session_is_fresh` added and used by both
`get_session` and `validate_session`; both reject an empty token before lookup.
`src/handlers/client_messages_handler.rs`: the upgrade now resolves the token via
`nostr_service.get_session(&token).await` and returns 401 on absent token, absent
service, or a token that names no live unexpired session; the `?token=` fallback
is behind `#[cfg(any(debug_assertions, feature = "dev-auth"))]`. The equivalent
sites in `socket_flow_handler/` and `fastwebsockets_handler.rs` are vc-gpu-wire's
and are closed under **ADR-2058** with the same gate. `mcp_relay_handler.rs` and
`multi_mcp_websocket_handler.rs` are the estate lead's and are fixed under
**ADR-2090**, which adopted the pattern from this record — including the
fail-closed absent-service arm.

```
cargo test -p visionclaw-server --lib session_expiry
    test result: ok. 5 passed; 0 failed; 0 ignored; 1272 filtered out
```

The five tests exercise the shared helper directly — it is pure over
`(last_seen, now, token_expiry)`, so the matrix runs without constructing a
service or touching Redis: inside the window, exactly at the boundary, past the
window, a small forward skew, and a future `last_seen` beyond the window
(asserting skew is rejected rather than granting an unbounded lease).

```
cargo check -p visionclaw-server --lib
    0 errors
```
Re-run after the release gate on `?token=` was added, and after the crate went
green (the four `src/services/owl_extractor_service.rs` errors present during
earlier runs were another lead's in-flight change and have since been fixed).

**Correction to the Phase 1 report this record supersedes:**
`speech_socket_handler.rs` was listed alongside the two defective handlers. That
was wrong — it performs proper NIP-98 verification via `verify_nip98_auth` at
`:230`, and has no `?token=` query path at all. Only `mcp_relay_handler` and
`multi_mcp_websocket_handler` carry the any-non-empty-token defect.

**Verification ran on the uncommitted working tree above
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` and must be re-run at the landing
commit, which sets `verified_paths`.**
