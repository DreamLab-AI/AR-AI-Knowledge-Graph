# P1 — COM-15 / V1 / D6 / M5 (consumer side): the PTT voice-to-selected-actor governed loop

- **Item:** COM-15 (consumer side), V1, D6, M5 — PRD-023 WP-5
- **Canary:** `CANARY-VC-COM15-PTT` (standing, P1)
- **Base SHA:** `c65cd8058a8cc977f2f4c395374d2f029d13ede0` (branch `gap-close/2026-07`)
- **Verified:** 2026-07-08T12:57Z
- **Maturity:** `scaffolded` → `integrated` (D6/M5: selection-scoped PTT binding + the two
  consumerless PTT modules consumed) and the consumer half of `federation-verified`
  (COM-15/V1: build → sign 31402 → mandate-authenticated POST → parse acceptance → Kokoro
  ack, proven against a real fake of the D7 endpoint). The live cross-substrate round-trip
  against the un-gated agentbox producer is **pending-live-session** — honestly labelled, and
  the standing `CANARY-VC-COM15-PTT` fires on that live end-to-end, never on the test path.

## Falsification (PRD-023 WP-5)

*"WP-5 is falsified if PTT remains globally scoped with no target `did:nostr`, if a spoken
command reaches only the settings assistant and never a signed 31402, if `PushToTalkService`
still has zero consumers, or if COM-15 closes without an audible acknowledgement observed in a
live session."*

Each clause is answered below; the last is the honest `pending-live-session` residual the
standing canary gates.

## What landed

### D6 — PTT is bound to the selected agent's did:nostr (no longer globally scoped)

- **Server** `src/services/audio_router.rs`: `UserVoiceSession` gains
  `selected_agent_did: Option<String>`. New methods `set_ptt_with_target`, `bind_selected_agent`,
  `selected_agent_did`. A binding is stored **only** if the claim is a canonical `did:nostr`
  (`validate_target_did` → `uri::parse` → `ParsedUri::DidNostr`); a non-DID clears the binding
  and warns (verify before trust, DDD invariant 2). `AudioRouterStatus.ptt_bound_to_agent`
  counts bound sessions. The previously **consumerless** `AudioRouter` (0 instantiations on
  `main`) is now held on `AppState` and driven by the speech socket — M5 on the server side.
- **Client** `client/src/services/PushToTalkService.ts`: adds `selectedAgentDid` +
  `setSelectedAgentDid` / `getSelectedAgentDid` / `isBoundToAgent`; the server-notify callback
  now carries `(pttActive, selectedAgentDid)` so a PTT-start threads the target. Only a
  canonical `did:nostr` binds (`isCanonicalDid`).

### AC1 — the selected-agent id threads from graph selection through the PTT-start message

- `client/src/features/voice/pttAgentBinding.ts` (new, pure): `resolveSelectedAgentDid`
  extracts the DID from a `visionclaw:node-selected` detail (agent nodes are keyed by their DID
  per COM-14 `agentTrustKey`, or the DID rides node metadata); a non-agent node → `null` → PTT
  unbinds.
- `client/src/features/voice/usePushToTalkAgentBinding.ts` (new, the consumer): activates
  `PushToTalkService`, binds selection → DID, and on every PTT edge sends `set_ptt {active,
  actorDid}` via `VoiceWebSocketService.setPtt`.
- **Server** `src/handlers/speech_socket_handler.rs`: a new `set_ptt` WS message
  (`SetPttRequest`) binds the socket's `AudioRouter` session via `set_ptt_with_target`.

### AC2 — a spoken command builds a SIGNED 31402 targeted at the DID, accepted by /v1/voice-intent

- `src/services/voice_intent_client.rs` (new): `VoiceIntentClient` builds a kind-31402
  `acsp::events::ActionRequest` whose `subject-id` **is** the target `did:nostr`
  (`build_voice_action_request`), signs it with the panel key (same idiom as `AcspClient`), and
  POSTs the ADR-037 D7 contract — additive `actor_did`, optional `actor` label, `transcript`,
  `duration_ms`, plus the signed-31402 summary — with a NIP-98 **mandate** `Authorization`
  header (`utils::nip98`). A non-DID target is refused **before** any signing or HTTP
  (`is_canonical_did`), so a hashed nickname can never be a governed target (ADR-037 D7).
- **Server routing** `speech_socket_handler.rs`: the `voice_command` arm now branches on
  `actor_did` — a bound, canonical DID takes the governed path; **only an unbound command
  reaches the settings assistant**, and a bound command whose dispatch fails is surfaced as a
  `voice_intent_error` (never silently re-routed to settings). This answers the second
  falsification clause.

### AC3 — a Kokoro TTS acknowledgement plays on acceptance

- `speech_socket_handler.rs::process_voice_intent`: on an accepted dispatch it builds
  `voice_intent_client::ack_sentence` (names the agent + the understood intent) and speaks it
  over `speech_service.text_to_speech` (Kokoro), then records a live fire on
  `CANARY-VC-COM15-PTT` via `liveness_harness.observe` — observed traffic, never a synthetic
  probe (DDD invariant 5).

### AC4 / M5 — PushToTalkService (and AudioRouter) are consumed, not superseded by a parallel path

- Client `PushToTalkService` (a complete singleton with **zero call sites** on `main`) is now
  consumed by `usePushToTalkAgentBinding`, which is consumed by `VoiceButton` (the voice UI):
  it drives the PTT lifecycle, the selection→DID binding, and the governed dispatch of a final
  transcript (`handleTranscription` fed from `useVoiceInteraction.onTranscription`).
- Server `AudioRouter` (defined but never instantiated on `main`) is now on `AppState` and
  driven by the `set_ptt` message. Both consumerless modules are consumed rather than
  re-implemented.

### Canary seed extended

- `src/services/liveness_harness.rs`: `CANARY_COM15_PTT` const + `P1_CANARIES` + `seed_p1_canaries`;
  seeded at boot from `app_state.rs` next to `seed_p0_canaries`, so `GET /api/canary/status`
  carries `CANARY-VC-COM15-PTT` (standing, P1) from start-up.

## Receipts

```
$ git rev-parse HEAD
c65cd8058a8cc977f2f4c395374d2f029d13ede0
$ date -u '+%Y-%m-%dT%H:%M:%SZ'
2026-07-08T12:57:46Z

# Server: selection-scoped PTT binding + the voice-intent consumer (build/sign/gate)
$ cargo test -p visionclaw-server --lib -- services::audio_router services::voice_intent_client
    Finished `test` profile [optimized + debuginfo] target(s) in 1m 17s
running 9 tests
test services::voice_intent_client::tests::ack_sentence_names_agent_and_intent ... ok
test services::voice_intent_client::tests::canonical_did_gate_matches_uri_primitive ... ok
test services::audio_router::tests::ptt_is_not_globally_scoped ... ok
test services::audio_router::tests::binding_clears_on_deselect_and_survives_bare_toggle ... ok
test services::audio_router::tests::non_did_target_is_refused ... ok
test services::audio_router::tests::ptt_binds_the_selected_agent_did ... ok
test services::voice_intent_client::tests::builds_31402_targeted_at_the_did ... ok
test services::voice_intent_client::tests::signs_31402_verifiably ... ok
test services::voice_intent_client::tests::non_did_target_is_refused_before_http ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 752 filtered out

# Fake-endpoint integration: build → sign 31402 → mandate header → POST → accept → ack
$ cargo test -p visionclaw-server --test voice_intent_roundtrip
    Finished `test` profile [optimized + debuginfo] target(s) in 2m 07s
running 3 tests
test non_did_target_is_refused_before_http ... ok
test producer_rejection_surfaces_as_error ... ok
test dispatch_signs_targets_and_is_accepted ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# Client: selection-scoped PTT state + the pure binding resolver
$ npx vitest run src/features/voice/pttAgentBinding.test.ts src/services/PushToTalkService.selection.test.ts
 ✓ src/features/voice/pttAgentBinding.test.ts (5 tests)
 ✓ src/services/PushToTalkService.selection.test.ts (5 tests)
 Test Files  2 passed (2)
      Tests  10 passed (10)

# Client: no TypeScript errors in any touched file
$ npx tsc --noEmit -p tsconfig.json   # filtered to touched files
NO TS ERRORS IN pttAgentBinding / usePushToTalkAgentBinding / PushToTalkService / VoiceWebSocketService / VoiceButton
```

The `dispatch_signs_targets_and_is_accepted` integration test asserts, as received on a real
socket, that the wire carried: the NIP-98 mandate header (`Authorization: Nostr …`), the D7
additive `actor_did` = the target DID, the transcript, and a signed `kind:31402` whose
`target_did` matches and whose `sig`/`pubkey` are 128/64 hex (a real BIP-340 signature over an
x-only key).

## Honest residual (pending-live-session)

- **The cross-substrate round-trip is not yet live.** The agentbox producer un-gates in this
  same wave (ADR-037 D7); on this branch `agentbox/management-api/routes/voice-intent.js` still
  gates on `[sovereign_mesh].voice_intent` and has no `actor_did`/mandate. The consumer is
  therefore proven against a **local fake** of the D7 contract, and the live
  transcript→31402→`/v1/voice-intent`→Kokoro path is **pending-live-session**. The governed loop
  is `None`-gated when unconfigured (`VoiceIntentClient::from_env` → the settings-assistant
  fallback), so nothing fabricates a dispatch.
- **The audible acknowledgement in a live session** is exactly what `CANARY-VC-COM15-PTT`
  (standing) records — a real fire is written only from `process_voice_intent` on a genuine
  accepted dispatch, never from a test or a synthetic probe. Until the producer is reachable and
  a spoken utterance completes the loop, the canary is `armed`, not `fired` — visibly Open, per
  the wave discipline.
- **Registration for the live session:** set `AGENTBOX_VOICE_INTENT_URL` (or
  `AGENTBOX_MANAGEMENT_URL`) and `ACSP_PANEL_NOSTR_PRIVKEY` to enable `VoiceIntentClient`; the
  panel pubkey it logs must be mandate-authorised on the producer.
