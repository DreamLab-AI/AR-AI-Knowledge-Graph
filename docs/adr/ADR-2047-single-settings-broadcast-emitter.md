---
id: ADR-2047
title: Emit settings-change broadcasts from one place, on a contract a client can consume
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a new settings category needing a broadcast, or the retirement of the settings WebSocket channel
repo: visionclaw
domain: BASELINE-architecture
lineage: Phase 1 diagram VC-06.4 recorded the per-category asymmetry; investigating the fix found the wider contract mismatch.
---

# ADR-2047 — Emit settings-change broadcasts from one place, on a contract a client can consume

## Context

Phase 1 (diagram VC-06.4) found that settings writes were split between
categories that told other sessions and categories that did not: `rendering`
(`settings_routes.rs:837-838`) and `node-filter` (`:972`) each carried their own
copy-pasted `BroadcastMessage` block, while `physics`, `constraints`,
`quality-gates`, `visual` and the profile routes were silent. A second open
session kept stale physics — the most visible category, since the layout moves —
until it happened to re-read.

Fixing that surfaced a larger fact the diagram had not: **nothing consumes any of
these broadcasts.** `grep -rn "settingsUpdated" client/src xr-client` returns
nothing. The React client's message validator knows a `settings_update` case
(`client/src/utils/validation.ts:274`, snake_case, expecting a `data` object)
while the server emits `settingsUpdated` (camelCase, carrying
`category`/`updatedBy`/`timestamp` and no `data`). Server messages therefore fall
through to the validator's `default:` arm, are logged as
`Unknown message type` at debug, and are acted on by nothing.

## Decision

The server emits settings-change broadcasts from exactly one function,
`broadcast_settings_change(state, category, updated_by)`, and `physics` now emits.
The emitter is fire-and-forget: a settings write must never fail because a
broadcast could not be serialised or the coordinator's mailbox was full, so
failures are logged and swallowed.

`node-filter` keeps its own richer payload rather than being flattened into the
generic one. It carries the resolved filter thresholds and mode, which a client
needs to recompute visibility without a follow-up read; collapsing it to
`{category, updatedBy, timestamp}` would remove information for the sake of
uniformity. The generic emitter is the default, not a mandate.

The wire name stays `settingsUpdated`. The client/server mismatch is resolved on
the **client** side, by adding a handler for the name the server actually emits,
because renaming the server event would silently change a published wire contract
for any consumer outside this repository. vc-clients did so under ADR-2080.

## Consequences

- Physics changes now announce themselves, closing the asymmetry VC-06.4 found.
- The duplicated broadcast blocks are gone; a future category needs one line.
- The channel is live end-to-end as of **ADR-2080** (vc-clients), which added the
  consumer: `nodeFilter` applies its payload directly, other categories re-read via
  `getSectionPaths`/`getSettingsByPaths`, and write-echo and stale-timestamp
  messages are dropped. This record moved `partial`/`staged` → `complete`/`live`
  only once that landed — it was deliberately held at `staged` while the server
  emitted into a void, because marking it complete would have claimed a feature no
  user could observe.
- The contract this record fixed is what ADR-2080 implements: type
  `settingsUpdated`, fields `category` (`physics`, `rendering`, `nodeFilter`, and
  any later addition), `updatedBy` (hex pubkey of the writer), `timestamp` (epoch
  ms), plus for `nodeFilter` only a `settings` object with the resolved
  thresholds.
- `constraints`, `quality-gates`, `visual` and the profile routes are still
  silent. That was the right call while no consumer existed; now that ADR-2080 has
  a generic re-read path for any category, each is one line and the constraint is
  only that vc-clients confirms which categories their handler should act on.

## Verification

`src/settings/api/settings_routes.rs`: `broadcast_settings_change` added above the
physics routes; called from `update_physics_settings` after persistence and from
`update_rendering_settings` in place of its inline block; `update_node_filter_settings`
left on its richer payload by design.

```
cargo check -p visionclaw-server --lib
    0 errors
```

Evidence for the dead channel **as it stood when this record was written**, before
ADR-2080 landed the consumer:

```
grep -rn "settingsUpdated" client/src xr-client        -> no matches
grep -rn "case 'settings"  client/src --include=*.ts   -> validation.ts:274: case 'settings_update':
```

`client/src/utils/validation.ts:270-284` shows the switch: `node_position_update`,
`settings_update`, `error`, then `default:` logging `Unknown message type`.

**Verification ran on the uncommitted working tree above
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` and must be re-run at the landing
commit, which sets `verified_paths`.**
