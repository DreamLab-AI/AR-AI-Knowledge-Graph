---
id: ADR-2003
title: The pubkey visibility filter defaults ON
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e78e958fa
owner: jjohare
review_trigger: introduction of per-node visibility metadata at ingest, or a deployment relying on anonymous access to private nodes
repo: visionclaw
---

# ADR-2003 — The pubkey visibility filter defaults ON

## Context

The fail-closed private-node drop filter (`visibility_filter.rs`) was fully
implemented but inert: `PUBKEY_VISIBILITY_FILTER` defaulted off and no
deployment set it, so unauthorised clients received private node positions by
default. The 2026-08-31 review sense-check rated this the highest-severity live
finding. Public nodes are unaffected by the filter, so the flip is
behaviour-neutral for all-public deployments.

## Decision

Absence of `PUBKEY_VISIBILITY_FILTER` means **filtered** (secure by default).
Opt-out requires an explicit falsy value (`0`/`false`/`off`/`no`,
case-insensitive); unrecognised values fail safe to ON. Parsing is a pure
function (`parse_visibility_flag`) behind a `OnceLock` wrapper — the posture is
captured at first use and runtime env mutations are ignored. The compose file
states the posture explicitly (`${PUBKEY_VISIBILITY_FILTER:-1}`).

## Consequences

- Deployments with private or owner-scoped nodes are protected without any
  configuration; this is the invariant "absence of a security flag must widen
  nothing" (`docs/SECURITY-profiles.md`).
- Anonymous/non-owner sessions in deployments carrying private-marked nodes
  will now see those nodes drop — intended, but a visible migration change.
- `RBAC_PUBLIC_READS=1` + `PUBKEY_VISIBILITY_FILTER=0` is a recorded illegal
  combination (full disclosure).

## Verification

Pure truth-table test `parse_visibility_flag_defaults_on_and_honours_opt_out`
green at `e78e958fa`; both read sites (`position_updates.rs`, `types.rs:332`)
share the single helper (grep confirms no residual `unwrap_or(false)` reader);
compose env block carries the explicit default.

## Closeout extension — 2026-09-04

CP-02/04/06/08. Owner remains jjohare with graph/identity/rendering maintainers. Current initial graph and position-stream source both filter by public/owner metadata; initial edges require visible endpoints. Six native domain filter tests pass. The complete/live declaration is retained for the default-on decision, without certifying all outputs or deployed private-data handling. Stale nearby default-off comments do not describe current executable flag parsing.

**Acceptance condition:** Trace authority for visibility/owner metadata through ingest and updates, normalise owner identity, and inventory every output path. Exercise anonymous/owner/non-owner callers, public-to-private transitions, owner changes, reauthentication, concurrent sync and cached client state. Confirm node/edge/label/analytics behaviour together. First-use flag caching requires release/process evidence for opt-out changes; later environment edits do not change the captured value. Reopen on metadata schema, ingest authority, output routes or client retention changes.

See the [review](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/rendered-state.md#visibility-defaults-and-output-coverage) and [receipt](https://github.com/DreamLab-AI/VisionFlow/blob/main/docs/estate-review/evidence/visibility-snapshot.json). Pure filter tests do not exercise a socket, browser, metadata mutation or live revocation. Dependencies are CP-02 metadata integrity, CP-04 identity and CP-08 release evidence.

## Acceptance progress — 2026-09-05

**Implemented.** `crates/visionclaw-domain/src/utils/visibility_filter.rs`.
The three caller classes are now exercised against one corpus (public,
private-owned-by-caller, private-owned-by-another, private-unowned):
anonymous sees public only; an owner sees public plus their own; a non-owner
sees public plus *their* own and never another's. Also covered:
public-to-private transition, owner change, re-authentication (the drop set is a
pure function of corpus and session pubkey, with no retained state), the
both-endpoints-visible edge rule asserted against the same drop set the socket
path uses, and the drop set applied to position, agent-node and label payloads.

**Output-path inventory** (the acceptance item) is a receipt, not code:
`docs/estate-closeout/2026-09-05/visibility-output-paths.sh` emits
`adr-2003-output-path-inventory.json`. It shows the two socket paths filtered
and four REST surfaces (`graph_export_handler`, `graph_state_handler`,
`bots_visualization_handler`, the analytics handlers) carrying **no** visibility
filter at all.

**Tests.** `cargo test -p visionclaw-domain --lib visibility` — 17 passed,
0 failed (11 new).

**Receipts.** `adr-2003-visibility-tests.txt`,
`adr-2003-output-path-inventory.json`, `visibility-output-paths.sh`.

**Remains open.** The unfiltered REST surfaces the inventory names are a real
coverage gap and need handler work with a caller identity threaded through;
that is not closable without touching the graph/analytics handlers. Socket,
browser and live-revocation behaviour, cached client state, and concurrent sync
remain unexercised. The first-use flag caching still needs release evidence.
