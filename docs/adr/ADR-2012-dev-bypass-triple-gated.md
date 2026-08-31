---
id: ADR-2012
title: The dev-session-token bypass is triple-gated and RBAC report-mode needs a dated ack; both unreachable in release
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []                   # legacy ADR-011 dev-bypass clause distilled — not in this tree; see lineage
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: any change to the dev-auth feature gate, DEV_AUTH_LOOPBACK handling, or the report-mode ack check
repo: visionclaw
domain: IDENTITY-authority-chain
lineage: Distils legacy ADR-011's dev-bypass exception clause; a Codex HIGH finding fenced the literal token behind three compiled-out gates.
---

# ADR-2012 — The dev-session-token bypass is triple-gated and RBAC report-mode needs a dated ack; both unreachable in release

## Context

Local development needs an auth shortcut, but ADR-011's dev-bypass clause was dangerous: a Codex
HIGH finding showed `Bearer dev-session-token` with an arbitrary `X-Nostr-Pubkey` previously
satisfied the whole lattice — including Admin — on any request. Separately, a stray
`RBAC_GATE_MODE=report` env var could silently downgrade all auth denials to logs in production.
Both are foot-guns that must be structurally unreachable in a release build.

## Decision

The `Bearer dev-session-token` bypass (with arbitrary `X-Nostr-Pubkey`) fires only under **all
three** of: the compile-time dev-auth feature, runtime `DEV_AUTH_LOOPBACK=1`, and a loopback peer
address. RBAC report-mode refuses to disable enforcement unless it is acknowledged — a debug
build, or `RBAC_REPORT_MODE_ACK` equal to today's UTC date — otherwise it logs the refusal and
stays in `Enforce`. This forecloses a release binary ever honouring the literal token or silently
running in report-mode from a leaked env var.

## Consequences

- Developers must set `DEV_AUTH_LOOPBACK=1` and hit the server from loopback for the shortcut;
  a remote peer or unset flag is rejected with a warning.
- Report-mode is a same-day, deliberate act (dated ack); an ack goes stale at UTC midnight,
  preventing long-lived silent bypass.
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
