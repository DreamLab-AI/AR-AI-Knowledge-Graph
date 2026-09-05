---
id: ADR-2012
title: Development token and RBAC report mode have distinct activation gates
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: live
supersedes: []                   # legacy ADR-011 dev-bypass clause distilled — not in this tree; see lineage
superseded_by: []
verified_commit: f326a3b1172df4fea8183e6a4344d3f55c575013
verified_paths: [src/utils/auth.rs, src/middleware/rbac_gate.rs]
owner: jjohare
review_trigger: any change to the dev-auth feature gate, DEV_AUTH_LOOPBACK handling, or the report-mode ack check
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: Distils legacy ADR-011's dev-bypass exception clause; a Codex HIGH finding fenced the literal token behind three compiled-out gates.
---

# ADR-2012 — Development token and RBAC report mode have distinct activation gates

## Context

Local development needs an auth shortcut, but ADR-011's dev-bypass clause was dangerous: a Codex
HIGH finding showed `Bearer dev-session-token` with an arbitrary `X-Nostr-Pubkey` previously
satisfied the whole lattice — including Admin — on any request. Separately, a stray
`RBAC_GATE_MODE=report` env var could silently downgrade all auth denials to logs in production.
Both are foot-guns that must be structurally unreachable in a release build.

## Decision

The `Bearer dev-session-token` bypass (with arbitrary `X-Nostr-Pubkey`) fires only under **all
three** of: a debug or dev-auth build, runtime `DEV_AUTH_LOOPBACK=1`, and a loopback peer
address. RBAC report-mode refuses to disable enforcement unless it is acknowledged — a debug
build, or `RBAC_REPORT_MODE_ACK` equal to today's UTC date — otherwise it logs the refusal and
stays in `Enforce`. The token is absent only when neither debug assertions nor dev-auth is enabled;
report mode can be deliberately activated in a non-debug build with its dated ack.

## Consequences

- Developers must set `DEV_AUTH_LOOPBACK=1` and hit the server from loopback for the shortcut;
  a remote peer or unset flag is rejected with a warning.
- Report-mode checks acknowledgement at construction; it does not automatically
  expire in an already-running middleware instance at UTC midnight.
- The bypass code still ships in release binaries but is fenced (feature + runtime + peer), so it
  is dead unless all gates are opened — complementary to the SECURITY release-env boot-abort (ADR-2026).
- Absorbs the SECURITY-profiles report-mode dated-ack facet — cross-ref SECURITY ADR-2027.

## Verification

Re-checked at `e0f8cd896`: `src/utils/auth.rs:82` `dev_bypass_permitted_for_addr` requires
`DEV_AUTH_LOOPBACK=1` (`:83`) and a loopback peer; the acceptance site `:130-144` gates on it and
rejects with a warning otherwise. `src/middleware/rbac_gate.rs:80` `GateMode::from_env` returns
`Report` only when `report_acknowledged()` (`:105-110`: debug build or `RBAC_REPORT_MODE_ACK` =
today's UTC date) holds, else logs the refusal and returns `Enforce` (`:96-101`).
Governing doc: `docs/IDENTITY-authority-chain.md`.

## Closeout extension — 2026-09-04

CP-01/04/08. Owner remains jjohare with authentication/release maintainers. Implementation is partial against the original release-unreachability and midnight-expiry claims. The literal token is gated by debug-or-dev-auth compilation, explicit runtime opt-in and the observed peer being loopback. Report mode is different: a non-debug build may enable it with a current-date acknowledgement and retain it for that middleware instance. The full dev-mode bypass in ADR-2039 is also separate and peer-agnostic.

**Acceptance condition:** Define the effective profile across feature set, all bypass/report controls, role fallbacks, peer/proxy handling and public route exceptions. Test missing versus zero-valued variables, report-mode construction/date rollover/restart, ownerless/error paths and production artefact promotion. Bind configuration and binary identity to the same pre-listener acceptance receipt. Reopen on any security-relevant setting, build feature, route exception or fallback. See the [profile review](../../../VisionFlow/docs/estate-review/role-authority.md#profile-claims-and-effective-policy) and [source receipt](../../../VisionFlow/docs/estate-review/evidence/security-profile-snapshot.json). The prior nine helper cases retain matching source hashes; no new live profile or network test ran.

## Acceptance progress — 2026-09-05

**Implemented.** New module `src/config/security_profile.rs`, called from
`src/main.rs` immediately before `HttpServer::new`/`bind`.

The reproduced gap — the literal dev token is triple-gated, but **report mode is
not**: a non-debug build can disable RBAC enforcement with a current-date
acknowledgement and retain it for that middleware instance — is closed at boot.
`evaluate_effective_profile(env, build, today)` is a pure function over an
injected environment snapshot, the build identity and the UTC date, so the whole
acceptance matrix runs as unit tests with no global state.

Findings, each fatal in a production artefact (no `debug_assertions`, no
`dev-auth`) and reported-only in a development build:
`ForbiddenDevVariable` (presence, not truthiness — `SETTINGS_AUTH_BYPASS=0` is a
rejection, and `DEV_AUTH_LOOPBACK` is now in the set, which ADR-2026's
`SUSPECT_ENVS` did not cover), `DevelopmentNodeEnvInContainer`,
`AllowSkipAuthArgv`, `DevAuthFeatureInArtefact`, `ReportModeRequested`
(acknowledged or not) and `ProfileDrift` / `UnknownDeclaredProfile`.

**Tests.** `cargo test --lib --no-default-features security_profile` — 29
passed, 0 failed. The missing-versus-zero-valued matrix is explicit: `VAR=0`,
`VAR=""` and `VAR=1` all reject, a missing variable does not. Report mode is
covered at construction, across the date rollover (yesterday's acknowledgement
no longer acknowledges; a future date does not pre-authorise), and on restart
(the same environment evaluated on two dates gives two answers).

**Receipts.** `docs/estate-closeout/2026-09-05/adr-2012-2038-security-profile.txt`.

**Remains open.** Binary identity is asserted from `cfg!` flags, not from an
image digest — binding configuration and artefact identity to one pre-listener
receipt still needs a build-side step. Peer/proxy handling and the full REST +
WebSocket paths are unexercised. No live profile or network test ran.
