---
id: ADR-2090
title: WebSocket upgrades resolve the token through the session realm and fail closed
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: adding a WebSocket upgrade handler, or any change to NostrService session validation or AUTH_TOKEN_EXPIRY
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: ADR-2044 (request-credential realm fails closed, vc-core — the session-expiry half and the pattern this copies), ADR-2058 (WebSocket auth is header-only in release, vc-gpu-wire — the carrier rule this extends to the two MCP sockets), ADR-2026 (fail-closed posture), ADR-2011 (central RBAC gate)
---

# ADR-2090 — WebSocket upgrades resolve the token through the session realm and fail closed

## Context

Two WebSocket upgrade handlers accepted **any non-empty string** as a
credential. Neither referenced `NostrService` at all — `grep -n
"get_session\|validate_session\|NostrService"` over both files returned nothing.
Each extracted a token from `Authorization: Bearer` or `?token=` and then tested
only `.is_empty()`, so `?token=x` was sufficient to open the socket:

- `src/handlers/mcp_relay_handler.rs` — `/ws/mcp-relay`, registered at
  `src/main.rs:1033`. Its own comment admitted the posture: *"Currently allows
  but logs unauthenticated connections — enforcement will come when all clients
  send tokens."*
- `src/handlers/multi_mcp_websocket_handler.rs` — `/multi-mcp/ws`, same shape.

This contradicts the fail-closed posture (ADR-2026) and the authority chain: a
socket is a longer-lived, higher-value grant than a REST call, and these two were
the weakest credential check in the estate. Found by vc-core and routed to this
lead as owner of both files; recorded here rather than in their ADR-2044 because
the code is ours.

## Decision

A WebSocket upgrade resolves its token through the **session realm** and fails
closed. Presence of a token is not authentication: the token must name a live,
unexpired session via `NostrService::get_session`, which since ADR-2044 enforces
the `AUTH_TOKEN_EXPIRY` window through the shared `session_is_fresh` helper.

Three outcomes are 401, with a distinct log reason each: no token, no
`NostrService` configured, and a token that names no live session. The
absent-service arm is explicitly a **denial**, not a bypass — with no session
store there is nothing to validate against, so the socket must not open. The
"allow but log, enforce later" posture is retired: an upgrade path either
authenticates or refuses.

The `Authorization: Bearer` header is the **only** carrier accepted in a release
build. `?token=` is compiled out of release entirely and survives only behind the
`#[cfg(any(debug_assertions, feature = "dev-auth"))]` gate, logging a `SECURITY:`
warning naming ADR-2058 when used; a release build that receives a `token=` query
parameter logs a `SECURITY:` rejection warning so the operator learns why the
client failed rather than seeing a bare 401. Clients that cannot set headers on an
upgrade authenticate post-connect with the NIP-98 `authenticate` envelope
(kind 27235).

This aligns both sockets with ADR-2058, which established the rule for the four
WebSocket entrances in the GPU/wire domain. A rule applied to four of six
entrances is not a rule — the two MCP sockets were the remaining gap.

## Consequences

- `?token=x` no longer opens either socket. Any client relying on the previous
  behaviour now receives 401 and must present a real session token, which is the
  intended break.
- Both handlers gain a dependency on `AppState`. `mcp_relay_handler` had no
  `app_state` parameter and now takes one; because actix resolves
  `web::Data<AppState>` positionally by type, the route registration at
  `src/main.rs:1033` needed **no** change — that file belongs to another lead and
  was not touched.
- The absent-`NostrService` denial means a deployment that has not configured the
  session store cannot open these sockets at all. That is the correct posture and
  is a behavioural change for any such deployment.
- Expiry now applies to WebSocket tokens, inherited from ADR-2044. Before that
  change `get_session` had no expiry check while `validate_session` did, so WS
  tokens were valid until logout while REST tokens expired.
- `?token=` no longer opens either socket in a release build. There is no in-tree
  consumer of the query path on these two routes (`grep -rn` over `client/` and
  `xr-client/` for either endpoint returns nothing), so nothing breaks; a browser
  client that cannot set headers uses the post-connect `authenticate` envelope.
- The bearer token stops reaching access logs, proxy logs and `Referer` headers
  for these endpoints in production — the log-hygiene concern this ADR previously
  deferred is now closed rather than merely recorded.

## Verification

Ran on the **uncommitted working tree** above SHA
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; `verified_paths` is empty because the
tree is uncommitted, and verification must be re-run at the landing commit.

- Defect confirmed before the change:
  `grep -n "get_session\|validate_session\|NostrService" src/handlers/mcp_relay_handler.rs src/handlers/multi_mcp_websocket_handler.rs`
  → **no matches**. The sole gate was `token.as_deref().unwrap_or("").is_empty()`
  at `mcp_relay_handler.rs:465` and `multi_mcp_websocket_handler.rs:802`.
- Both handlers now call `nostr_service.get_session(&token).await.is_none()` and
  return 401 on `None`, matching the pattern landed by ADR-2044 in
  `src/handlers/client_messages_handler.rs:150-172`.
- `app_state.nostr_service` is `Option<web::Data<NostrService>>`
  (`src/app_state.rs:378`); the `let Some(...) else` arm denies.
- `cargo check -p visionclaw-server --message-format=short` → **0 errors**
  (`grep -cE '^src.*error'` → `0`). The crate had been red on unrelated
  in-flight breakage from another lead; that cleared first and this change did
  not reintroduce it.
- `cargo test -p visionclaw-server --lib multi_mcp` → **ok. 5 passed; 0 failed**.
- `cargo test -p visionclaw-server --lib bots` → **ok. 4 passed; 0 failed**.

### Amendment — query-string carrier dev-gated (2026-09-05)

Queen decision following a vc-gpu-wire report that the `?token=` extraction
survived in both handlers after the first pass. Both now carry the ADR-2058
shape verbatim: a `header_token` binding, a
`#[cfg(any(debug_assertions, feature = "dev-auth"))]` arm that falls back to the
query string with a `SECURITY:` warning, and a
`#[cfg(not(any(debug_assertions, feature = "dev-auth")))]` arm that warns on a
`token=` query and returns the header token alone.

Both `cfg` arms were type-checked, not just the one the dev profile compiles:

- `cargo check -p visionclaw-server --message-format=short` → **0 errors**
  (dev arm).
- `cargo check --release -p visionclaw-server --message-format=short` → **0
  errors** attributable to either handler (release arm; the only warnings are
  pre-existing future-incompat notices for the `quick-xml` dependency).
- `cargo test -p visionclaw-server --lib multi_mcp` → **ok. 5 passed; 0 failed**.

### Deliberately not bundled — now closed by ADR-2091

`multi_mcp_websocket_handler.rs` also carried two live REST routes that returned
fiction: `get_mcp_server_status` served a hardcoded JSON literal and never
queried real state, and `refresh_mcp_discovery` was a no-op stub; both took
`_app_state` unused. They were kept out of this record so a credential change
stayed reviewable on its own, and were **removed under ADR-2091** in the same
pass. Nothing remains open from this ADR.
