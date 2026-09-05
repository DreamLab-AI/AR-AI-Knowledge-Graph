---
id: ADR-2074
title: One helper computes the browser client's auth headers; no transport carries its own copy
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a new HTTP transport added to the browser client, or a change to the NIP-98 kind-27235 event shape, either of which must go through computeAuthHeaders rather than beside it
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: de-duplicates the signing sites exposed by VC-30.8 and VC-33.2; the signed event shape and its server-side replay cache are unchanged (ADR-2002), as are the two auth realms (ADR-2009)
---

# ADR-2074 — One helper computes the browser client's auth headers

## Context

Four HTTP transports in the browser client each carried their own copy of the same auth-header
branch — `nostrAuth.isDevMode()` → `Bearer dev-session-token` plus `X-Nostr-Pubkey`, otherwise
NIP-98 `signRequest` → `Authorization: Nostr <b64>`:

- `services/api/authInterceptor.ts:41` (`UnifiedApiClient`), the canonical one;
- `api/settings/endpoints.ts:37`, a **separate global** `axios.interceptors.request.use`;
- `features/ontology/services/jss/contextLoader.ts:22` `fetchWithAuth`, over `fetch`/`Headers`;
- `services/solidPod/ldpClient.ts:91` `fetchWithAuth`, a second `fetch`/`Headers` copy.

All four were correctly gated — an earlier reading that two were ungated was wrong and is
corrected in VC-33.3. The defect was duplication: a change to the event shape, the dev-token
literal or the release-mode 401 handling propagated to one transport and silently not the
others. Exposed by diagrams VC-30.8 and VC-33.2.

## Decision

`computeAuthHeaders(fullUrl, method, body?) => Promise<Record<string,string>>`, exported from
`client/src/services/api/authInterceptor.ts`, is the single place the browser client decides
what auth headers a request carries. It returns `{}` when unauthenticated, the dev-token pair
when `isDevMode()`, and the NIP-98 header otherwise. Every HTTP transport calls it and none
reimplements the branch.

Each caller keeps its own transport concerns, because those genuinely differ: `authInterceptor`
keeps `X-Request-ID` generation and its release-mode 401 warning; the axios interceptor keeps
its `new URL(config.url, config.baseURL ?? origin)` derivation and `config.data` stringification;
the two `fetch` wrappers keep their absolute-URL construction, their `Headers` copy and their
warn-and-proceed behaviour on signing failure. `ldpClient` keeps its stale-session warning.

WebSocket `authenticate` frames are **out of scope and stay separate** — `analyticsApi.ts:450`,
`VoiceWebSocketService.ts:144` (ADR-2075) and `websocket/connectionManager.ts:379` send a JSON
frame after the upgrade, not an HTTP header, because browsers cannot set WebSocket headers.

## Consequences

- The literal `Bearer dev-session-token` now appears in exactly one code path
  (`authInterceptor.ts:65`), plus its own unit test and the WS frames named above.
- A change to the kind-27235 event, the dev gate or the header names is a one-file change.
- The helper adds a guard the `fetch` copies lacked: it returns `{}` rather than signing when
  authenticated with no `pubkey`. Strictly safer, and unreachable in practice.
- Bytes on the wire are unchanged. This is a pure de-duplication: no call site, endpoint, URL
  or request shape was altered.
- Follow-on: `connectionManager.ts:379` and `analyticsApi.ts:450` still each build their own WS
  auth frame. Consolidating the *frame* builders is a separate, smaller decision.

## Verification

Verification ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`
and must be re-run at the landing commit.

- `cd client && ./node_modules/.bin/tsc --noEmit` → `TSC_EXIT=0`, no output.
- `cd client && npm test` (`vitest run`) → `Test Files 69 passed (69)`, `Tests 773 passed (773)` —
  identical to the pre-change baseline, including
  `client/src/services/api/__tests__/authInterceptor.test.ts` which asserts the dev header.
- `grep -rn "Bearer dev-session-token" client/src/ --include=*.ts --include=*.tsx | grep -v __tests__`
  → three hits, all in `authInterceptor.ts` (`:33` and `:127` are the release-mode 401 warning
  and its check, `:65` is the single construction site). None in `endpoints.ts`,
  `contextLoader.ts` or `ldpClient.ts`.
