---
id: ADR-2080
title: The browser client consumes the settingsUpdated broadcast so peers converge without a reload
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a new settings category gaining a server-side emitter, or any change to the settingsUpdated payload shape fixed by ADR-2047
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: the client half of vc-core's ADR-2047, which fixed the server contract and unified the emitter; this ADR supplies the consumer that made ADR-2047 observable
---

# ADR-2080 — The browser client consumes the `settingsUpdated` broadcast

## Context

The server's settings-change broadcast had **no consumer at all**. vc-core found the asymmetry
(physics did not broadcast, rendering did) and the larger fact under it: the server emits
`settingsUpdated` (camelCase) while the only client-side knowledge of such an event was
`client/src/utils/validation.ts:274`'s `case 'settings_update':` (snake_case, expecting a
different shape). Server messages fell through to the dispatcher's `default:` arm, were logged as
"Unknown message type" at debug level, and were acted on by nothing. Two wire contracts that had
never met.

The visible cost: two people with the graph open see **different physics** until one reloads,
because physics is the category whose divergence is immediately obvious — the layout moves.

`settings_update` is confirmed dead on the wire. `grep -rn "settings_update\b" src/` returns only
Rust function and schema names (`EndpointRateLimits::settings_update()`, `validate_settings_update`);
the only WS senders are `src/settings/api/settings_routes.rs:445,982`, both emitting
`settingsUpdated`.

## Decision

The client handles `settingsUpdated` on the real dispatch path
(`client/src/store/websocket/textMessageHandler.ts`), against the contract ADR-2047 fixed:
`{ type, category, updatedBy, timestamp, settings? }`.

- **`nodeFilter` applies the supplied `settings` directly.** The server sends the resolved
  thresholds precisely so no follow-up read is needed; honouring that is the point of the richer
  payload.
- **Every other category triggers a re-read of that category**, via the table the codebase already
  uses for on-demand section loading: `settingsStoreUtils.getSectionPaths(category)` →
  `settingsApi.getSettingsByPaths(paths)` → `set(path, value, true)`. No new endpoint is invented.
  `ensureLoaded` is deliberately *not* reused: it short-circuits on paths already in `loadedPaths`,
  which is true for every essential category after `initialize()`, and would silently no-op the
  very re-read this feature exists to perform.
- **A client ignores the echo of its own write** (`updatedBy` equal to the current pubkey).
  Without this, a local save round-trips and can fight the user's in-flight edits.
- **A stale message is ignored**: per-category last-applied timestamps, and an incoming
  `timestamp` at or below the last applied is dropped. Broadcast order is not guaranteed.
- Values written from a broadcast use the store's skip-autosave path — they are reflected server
  state, not a local edit, and must not be echoed back.
- `validation.ts`'s `settings_update` case is **left in place** with a comment naming
  `settingsUpdated` as authoritative. It is dead, but removing a validator arm is not this ADR's
  business and its presence is now explained rather than misleading.

## Consequences

- Two viewers converge on physics, rendering and node-filter changes without a reload. This is the
  first time any settings broadcast has done anything.
- ADR-2047 can move from `staged` to live: its emitter now has a consumer. vc-core deliberately
  held it partial until this landed, which was the right call.
- Categories vc-core left silent (`constraints`, `quality-gates`, `visual`, the profile routes)
  remain silent by design. They are one line each on the server once wanted; the consumer added
  here is generic and will handle any category present in `getSectionPaths`.
- A category broadcast but absent from `getSectionPaths` logs a warning and does nothing. That is
  the intended failure: a server emitter without a client path table entry is a bug worth seeing,
  not worth guessing around.
- Echo suppression depends on the client knowing its own pubkey. An unauthenticated client
  suppresses nothing and simply re-reads — correct, if marginally chattier.

## Verification

Verification ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`
and must be re-run at the landing commit.

- `cd client && ./node_modules/.bin/tsc --noEmit` → `TSC_EXIT=0`.
- `cd client && npm test` (`vitest run`) → `Test Files 71 passed (71)`, `Tests 789 passed (789)`.
- New suite `client/src/store/websocket/__tests__/textMessageHandler.test.ts` → 4 tests in
  isolation: `nodeFilter` applies the supplied settings with no re-read; `physics` triggers
  `getSettingsByPaths` and writes the resolved value back; a message whose `updatedBy` is the
  current user is ignored; a stale-timestamp message after a fresher one is ignored.
- Dead-contract check: `grep -rn "settings_update\b" src/` → Rust identifiers only, no wire sender.
  `grep -rn '"settingsUpdated"' src/` → `src/settings/api/settings_routes.rs:445,982`.
