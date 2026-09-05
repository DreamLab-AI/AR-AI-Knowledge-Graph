---
id: ADR-2100
title: Consolidate the Solid/JSS notification clients onto one socket
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b0bc275f6501aae7751b85a72ce15fe1e730e7e8
verified_paths: []
owner: jjohare
review_trigger: A third consumer needing JSS notifications, a change to the `VITE_JSS_WS_URL` wiring, or any proposal to give a consumer its own reconnect policy.
repo: visionclaw
---

# ADR-2100 — Consolidate the Solid/JSS notification clients onto one socket

## Context
The browser client opened **two** independent WebSockets to the same
`VITE_JSS_WS_URL`. `services/solidPod/podNotifications.ts`
(`PodNotificationManager`, registry name `solid-pod`, 5 reconnect attempts)
served `SolidPodService`; `store/websocket/solidWebSocket.ts` (registry name
`solid-store`, 10 attempts) served the Zustand store, reached through
`serviceCompat` by `features/ontology/services/jss/inferenceClient.ts`. Both
spoke solid-0.1, both maintained their own subscription map, and each sent its
own `sub`/`unsub` for overlapping resources. The pod therefore saw two
subscriber sets for one logical client, two divergent backoff ladders, and two
`WebSocketRegistry` entries. Diagram
`docs/diagrams/visionclaw/26-solid-pod-and-jss.md:399` recorded the divergence.
Only the store's client handled the server's `error ` frame; only it emitted
`solid-*` store events.

## Decision
There is **one** JSS notification client: the `podNotificationManager` singleton
exported from `services/solidPod/podNotifications.ts`, registered once as
`solid-pod`. Both consumers bind to it — `SolidPodService` holds the singleton
rather than constructing its own, and `store/websocket/solidWebSocket.ts` is
reduced to a thin store adapter over it.

Retry policy is a single pair of exported constants,
`SOLID_MAX_RECONNECT_ATTEMPTS = 5` and `SOLID_RECONNECT_DELAY_MS = 1000`, owned
by the manager. No consumer carries a ladder of its own; the store adapter's
`resetSolidReconnect` delegates.

The manager gains the surface the store's client uniquely had, so nothing is
lost in the merge: `error ` frame handling, a `protocol` handshake signal, and
per-subscriber error containment. These reach the store through a
connection-level `onLifecycle` channel, which the adapter mirrors into
`isSolidConnected` / `solidSocket` and re-emits as the same `solid-connected`,
`solid-disconnected`, `solid-error`, `solid-protocol` and
`solid-resource-changed` events store consumers already listen for. The store's
public surface (`connectSolid`, `disconnectSolid`, `subscribeSolidResource`,
`unsubscribeSolidResource`, `isSolidWebSocketConnected`,
`getSolidSubscriptions`) is unchanged.

`state.solidSubscriptions` survives as a **bookkeeping mirror only** — it backs
`getSolidSubscriptions()` and is never dispatched through, so a callback
registered via the store fires exactly once.

## Consequences
- One socket, one subscriber registry, one backoff ladder, one registry entry.
  The `solid-store` registry name is retired.
- Reconnect behaviour changes for the store's consumers: 5 attempts, not 10.
  This is the deliberate reconciliation — one endpoint cannot have two retry
  policies. Callers needing a longer ladder change the shared constant.
- The store no longer owns a socket, so `connectSolidWebSocket` /
  `disconnectSolidWebSocket` lost their `get` and reconnect-callback parameters;
  `store/websocket/index.ts` was updated accordingly.
- Either consumer connecting satisfies both. The adapter re-syncs its mirror when
  it finds the socket already open, so store state stays truthful when
  `SolidPodService` opened it first.
- Cost: the two consumers are now coupled through one lifecycle. A disconnect by
  either tears down the shared socket — correct for one logical connection, but
  it means `SolidPodService.disconnect()` also drops the store's subscriptions.
  That was already true of any shared-endpoint design and is now explicit.

## Verification
Verified on the working tree at `b0bc275f6501aae7751b85a72ce15fe1e730e7e8` (the
tree carried other agents' uncommitted edits; the paths this record touches were
inspected directly).

- Consumer inventory established by reference search across `client/src` before
  the change: `podNotifications` → `SolidPodService.ts`; store client →
  `serviceCompat.ts` → `inferenceClient.ts:46,100`.
- `src/store/websocket/__tests__/solidWebSocketConsolidation.test.ts` (12 tests)
  pins the contract against a fake WebSocket: exactly one socket is constructed
  when both consumers connect, registration only ever uses `solid-pod`, a `pub`
  reaches a store subscriber exactly once, `sub`/`unsub` traverse the one socket,
  the handshake resubscribes, a throwing subscriber does not starve its peers,
  container subscribers fire once, and the shared constants are 5/1000.
- `npx tsc --noEmit` → exit 0.
- `eslint src --ext ts,tsx` → 0 errors.
- `vitest run` → 73 files, 809 tests passed (789 before). No regressions.
