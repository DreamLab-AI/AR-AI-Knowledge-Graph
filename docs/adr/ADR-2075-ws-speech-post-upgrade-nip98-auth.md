---
id: ADR-2075
title: Authenticate /ws/speech with a post-upgrade NIP-98 frame and drop query-token auth
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a browser gaining the ability to set WebSocket upgrade headers, or a decision to move voice onto the multiplexed graph socket
repo: visionclaw
domain: XR-client
lineage: applies the graph socket's authenticate-frame contract (socket_flow_handler/filter_auth.rs) to the voice socket; consumes ADR-2002's NIP-98 replay cache and ADR-2039's dev-mode gating
---

# ADR-2075 — Authenticate `/ws/speech` with a post-upgrade NIP-98 frame and drop query-token auth

## Context

`/ws/speech` gated its upgrade on a credential it never verified: it accepted any non-empty
`Authorization: Bearer` value **or** any non-empty `?token=` query parameter, checking only
that the string was not empty. Browsers cannot set headers on a `WebSocket` constructor, and
`VoiceWebSocketService` sent neither a header nor a query token — so the browser voice client
was rejected outright at upgrade while an arbitrary attacker-chosen string was accepted. The
socket carries command-bearing frames (`voice_command`, `tts`, `stt`, `set_ptt`) that spawn
agents and speak, so it is a write surface. The estate already has a contract for this: the
graph socket authenticates **after** the upgrade with
`{"type":"authenticate","event":"<base64 NIP-98>"}` (`socket_flow_handler/filter_auth.rs:10`).
Exposed by diagram VC-35.4.

## Decision

`/ws/speech` performs no credential check at upgrade time and accepts no `?token=` query
parameter. The socket opens unauthenticated and holds a `pubkey: Option<String>` that is
`None` until an `authenticate` frame is accepted.

- `{"type":"authenticate","event":"<base64>"}` is verified with
  `NostrService::verify_nip98_auth(header, connection_url, "GET", None)`, where
  `connection_url` is the HTTP-equivalent of this socket's own upgrade URL — the same `u`-tag
  derivation the graph socket uses. Success sets `pubkey` and replies `authenticate_success`;
  failure replies `authenticate_error` and leaves the socket unauthenticated.
- `authenticate` is the **only** frame accepted while `pubkey` is `None`. Every other typed
  frame is refused with an `error` telling the client to authenticate first. Ping/pong remain
  free so heartbeats work pre-auth.
- A socket that has not authenticated within `AUTH_DEADLINE` (30s) is closed, so an
  unauthenticated peer cannot hold an audio/transcription broadcast subscription open.
- Dev relaxations mirror the graph socket exactly and are compiled out of release builds
  (`#[cfg(any(debug_assertions, feature = "dev-auth"))]`): the LAN-local full bypass when
  `dev_full_bypass_active()` (ADR-2039), and the literal `dev-session-token` only when the
  handshake marked the connection dev-bypass-eligible (`DEV_AUTH_LOOPBACK=1` **and** a
  loopback peer). Neither is ever accepted ungated.
- `VoiceWebSocketService` sends the `authenticate` frame on `onopen`, signing the HTTP
  equivalent of its own socket URL, and handles `authenticate_success` / `authenticate_error`.

## Consequences

- Voice works in the browser for the first time on this path: previously every browser
  connection was 401'd at upgrade.
- Unverified bearer strings and `?token=` query auth are gone from this surface, so voice
  credentials no longer appear in URLs, proxy logs or referrers.
- An unauthenticated client can still open the socket and receive the `connected` greeting;
  it can do nothing else and is closed after 30s. This is deliberate — it lets the client
  distinguish "server unreachable" from "not authorised".
- `VoiceMessage.type` gains `authenticate_success` and `authenticate_error`.
- Follow-on: `/ws/speech` still uses its own socket rather than the multiplexed graph socket.
  Consolidating them is a separate decision.

## Verification

Verification ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`
and must be re-run at the landing commit.

- `cargo check -p visionclaw-server` → **`Finished dev profile`, exit 0**, whole crate clean.
  (Earlier in the session the crate carried 10 pre-existing errors in other domains' files —
  `src/services/owl_extractor_service.rs` and the `quic_transport_handler` re-exports in
  `src/handlers/mod.rs`. Those were fixed by their owners at 18:58 and the crate now compiles,
  so this change is verified against a fully type-checked crate rather than a partial one.)
- `cargo test -p visionclaw-server --lib speech` → `ok. 2 passed; 0 failed` (the voice actor
  tests: `elevation_voice::harvest_finds_multiword_concepts_in_speech`,
  `voice_interface_actor::ordinary_speech_does_not_trigger`).
- `cargo test -p visionclaw-server --lib auth` → `ok. 40 passed; 0 failed`.
- `cargo test -p visionclaw-server --lib nip98` → `ok. 39 passed; 0 failed`. The NIP-98 validator
  and the auth realm this handler now delegates to are unchanged and still pass.
- No test exercises the new socket state machine end to end: `SpeechSocket` is an actix
  `StreamHandler` and the crate has no WS harness for it. The auth deadline, the
  refuse-until-authenticated gate and the `authenticate` verify path are verified by inspection
  and by their structural identity with `socket_flow_handler/filter_auth.rs`, not by a test.
  That gap is real and is the first thing to close if this handler changes again.
- `cd client && ./node_modules/.bin/tsc --noEmit` → exit 0, no output.
- `cd client && npm test` (`vitest run`) → `Test Files 69 passed (69)`, `Tests 773 passed (773)`.
- `grep -n "form_urlencoded" src/handlers/speech_socket_handler.rs` → no output (the query-string
  parser is gone). `grep -n "?token=" …` returns exactly one hit, line 986, inside the comment
  that records why the old path was removed — no code reads a query token.
- `grep -rn "dev_full_bypass_active\|dev_bypass_permitted" src/handlers/speech_socket_handler.rs`
  → both sites present and both inside `#[cfg(any(debug_assertions, feature = "dev-auth"))]`.
