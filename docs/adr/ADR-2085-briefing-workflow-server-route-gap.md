---
id: ADR-2085
title: The briefing workflow client calls agentbox routes that do not exist
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: b0bc275f6501aae7751b85a72ce15fe1e730e7e8
verified_paths: []
owner: jjohare
review_trigger: any attempt to invoke BriefingService::submit_brief or request_debrief in a live deployment, or a decision to build the agentbox brief routes
repo: visionclaw
domain: BASELINE-architecture
---

# ADR-2085 — The briefing workflow client calls agentbox routes that do not exist

## Context

Diagram ES-02.11 (`docs/diagrams/estate/02-agent-events-agentbox-to-visionclaw.md`) marks this a
`COVERAGE GAP`. `src/services/management_api_client.rs` implements `create_brief` (line 433, POSTs
`{base}/v1/briefs`, line 442), `execute_brief` (line 489, POSTs `{base}/v1/briefs/{id}/execute`,
line 497), and `create_debrief` (line 539, POSTs `{base}/v1/briefs/{id}/debrief`, line 545). A grep
of `agentbox/management-api/` finds no `/v1/briefs` route registration anywhere — the only "briefs"
hit in agentbox is a passing mention in a doc comment (`middleware/linked-data/surfaces/s01-pods.js:10`).
The client is not dead: `src/services/briefing_service.rs` calls all three
(`submit_brief` lines 37, 55; `request_debrief` line 93). Every call to `BriefingService` 404s at
runtime — a live caller, a missing server, spanning three leads' files (beyond bounded scope here).

## Decision

Resolve by **implementing the three routes in agentbox** (direction i), not by removing the client
(direction ii). Reason: `briefing_service.rs` exists specifically to orchestrate a brief → execute →
debrief workflow through the Management API — it is a designed feature with a complete client-side
contract (typed request/response structs, error handling, retry-free single-shot calls), not
speculative or orphaned code; the missing piece is purely server-side. Removing a fully-specified,
well-typed client to erase a gap the server side should have filled is the wrong direction — the
route surface is small (3 endpoints) and the contract is already fully derived below.

Cross-lead routing: **ab-identity-governance** implements the three routes under
`agentbox/management-api/routes/` (that tree is their file ownership per PHASE2 policy). **vc-knowledge**
owns `src/services/briefing_service.rs` and should add integration tests once the routes exist.
**estate** (this ADR's owner) owns `src/services/management_api_client.rs` and makes no code change
here — the client's request/response shapes below are the target contract, not a proposal to change.

## Consequences

- Until the agentbox routes exist, `BriefingService::submit_brief` / `request_debrief` must not be
  wired into any handler that user traffic reaches — any such wiring would be a silent 404 in
  production. (Grep at this verified_commit found no handler invoking `BriefingService`; if one is
  added before the routes land, that is a regression against this ADR.)
- ab-identity-governance's implementation effort is bounded to the acceptance test below — no
  design decisions remain open on the wire contract, it is derived verbatim from the client.
- Follow-on: once routes land, vc-knowledge adds a `#[cfg(test)]` integration test in
  `briefing_service.rs` exercising the full brief → execute → debrief cycle against them, and this
  ADR's `decision_status` moves to `accepted` / `implementation_status` to `complete`.

## Acceptance test (exact contract, derived from `management_api_client.rs` request/response code)

**1. `POST /v1/briefs`** (`management_api_client.rs:433-486`)
- Request JSON: `{"content": string, "roles": string[], "user_context": {"user_id": string,
  "pubkey": string, "display_name": string, "session_id": string, "is_power_user": bool},
  "version"?: string, "brief_type"?: string, "slug"?: string}` (camelCase on the wire per
  `#[serde(rename_all = "camelCase")]` on `BriefResponse`, so response fields are camelCase; request
  fields as literally named in the `serde_json::json!` body above — i.e. snake_case, unconverted).
- Auth: `Authorization: Bearer <api_key>`.
- Success: `201` or `200`, body `{"briefId": string, "briefPath": string, "beadId"?: string}`
  (struct `BriefResponse` at `management_api_client.rs:135-139`, camelCase).
- Failure: any other status → body text becomes the error message; client wraps as
  `ManagementApiError::ApiError`.

**2. `POST /v1/briefs/:brief_id/execute`** (`management_api_client.rs:489-536`)
- Request JSON: `{"brief_path": string, "roles": string[], "user_context": {...same shape as
  above}, "epic_bead_id"?: string}`.
- Success: `202` or `200`, body `{"briefId": string, "roleTasks": RoleTask[]}` (struct
  `ExecuteBriefResponse` at `management_api_client.rs:141-146`, camelCase; `RoleTask` is
  `crate::types::user_context::RoleTask` — implementer must match its serde field names exactly).

**3. `POST /v1/briefs/:brief_id/debrief`** (`management_api_client.rs:539-584`)
- Request JSON: `{"role_responses": [{"role": string, "responsePath": string, "taskId": string,
  "status": "completed"|"pending"}], "user_context": {...same shape as above}}` — note
  `role_responses` itself is NOT camelCased by the client (it is a literal key in the
  `serde_json::json!` body), but its inner fields `responsePath`/`taskId` already are, as written.
- Success: `201` or `200`, body `{"debriefPath": string}` (struct `DebriefResponse` at
  `management_api_client.rs:148-151`, camelCase).

**Whoever implements**: a route is done only when a real VisionClaw `ManagementApiClient` call
(not a curl approximation) round-trips successfully through all three endpoints in sequence using
one brief id end to end, and `cargo test -p visionclaw-server briefing` (once vc-knowledge adds
that test) passes against the running agentbox management-api.

## Verification

Proposed ADR — no implementation to verify. Evidence gathered at `verified_commit`:
- `grep -n "create_brief\|execute_brief\|create_debrief\|BriefResponse\|DebriefResponse\|/v1/briefs"
  src/services/management_api_client.rs` — confirms line numbers cited above.
- `grep -n "create_brief\|execute_brief\|create_debrief" src/services/briefing_service.rs` —
  confirms live caller at lines 37, 55, 93.
- `grep -rn "briefs" agentbox/management-api/` — zero route-registration hits; only doc-comment
  mention at `agentbox/management-api/middleware/linked-data/surfaces/s01-pods.js:10`.
- Working-tree caveat: verification ran on the uncommitted working tree above `verified_commit`;
  re-verify the grep results at the landing commit.

## Verification — 2026-09-05

Implemented. The three routes exist in agentbox as `management-api/routes/briefing.js`, registered in
`management-api/server.js:1181`; the agentbox half is recorded as **ADR-2072** (BASELINE-container).
`activation_status: staged` — management-api is baked into the agentbox image, so the routes serve
traffic only after the next `./agentbox.sh rebuild`; they are complete and tested in the tree now.

**Working-tree base:** `verified_commit` b0bc275f6501aae7751b85a72ce15fe1e730e7e8 (VisionClaw).
The agentbox half sits above e070514d808b218574403377fb75e0e1a0a256b3. Both implementations are
uncommitted at the time of writing, so re-verify at the landing commits. `verified_paths` is `[]`.

Commands and results:

- `cargo test --lib briefing` (VisionClaw root) → **4 passed**, 0 failed, 1333 filtered out. The new
  `#[cfg(test)] mod tests` in `src/services/briefing_service.rs` drives a real `ManagementApiClient`
  — the "not a curl approximation" rule above — against a bare-tokio mock origin replying with
  exactly the JSON `routes/briefing.js` emits, so no Cargo dependency was added. Cases:
  `submit_brief_creates_then_executes_against_the_agentbox_contract` (one brief id end to end
  through create + execute, asserting the snake_case request bodies, the Bearer header, and that
  `epic_bead_id` is carried forward), `request_debrief_posts_role_responses_and_returns_the_debrief_path`
  (the `role_responses` snake_case key with camelCase `responsePath`/`taskId` inner fields, and the
  `bead_id`-derived `completed`/`pending` status), plus two failure paths —
  `submit_brief_surfaces_a_server_error_instead_of_a_silent_success` (the 404 this ADR was written
  about) and `submit_brief_fails_when_execute_rejects_after_a_successful_create` (a 503 from the
  agentbox action plane must fail the whole submit, not return an empty `role_tasks`).
- `cargo fmt --all --check` → clean (exit 0).
- `node scripts/diagram-index-gen.js docs/diagrams --check` → `parsed 71 topic files, 841 mermaid
  diagrams`, no error. ES-02.11's `PROPOSED ADR-2085: CONFIRMED absent` note is now
  `RESOLVED ADR-2085/2072 (2026-09-05): …`, and its `BRF` participant, which read
  "route not found under agentbox/management-api/routes in this pass", now cites
  `agentbox/management-api/routes/briefing.js:1`.
- agentbox side, from `agentbox/management-api`:
  `node node_modules/.bin/jest ../tests/sovereign/briefing-route.test.js` → **10 passed**;
  `node node_modules/.bin/jest` → **80 suites / 1289 passed**, 3 skipped, 34 todo, 0 failed, against
  a 79-suite / 1279-passing baseline measured with the new file excluded — the 10 added tests are
  the entire delta and nothing regressed.

Contract detail worth recording, because it is the part that fails silently: `RoleTask` carries no
`#[serde(rename_all)]` while its enclosing `ExecuteBriefResponse` does, so `roleTasks` is camelCase
but its **elements are snake_case** (`task_id`, `bead_id`, `response_path`). Fastify strips response
properties absent from a route's schema, so a server that camelCased those members would hand the
client an empty field rather than an error. Both test suites assert the exact key sets.

The Consequences constraint above is discharged rather than merely satisfied: `BriefingService` may
now be wired into user-facing handlers, because the routes it calls exist. Two follow-ons stand —
no route reads or lists a brief (the client needs neither, so none was invented), and no retention
policy governs the brief artefacts accumulating under the pods slot at `/briefs/`.
