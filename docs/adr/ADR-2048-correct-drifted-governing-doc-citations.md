---
id: ADR-2048
title: Correct the drifted file:line citations in the boot, identity and posture governing docs
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: the next governing-doc verified_commit refresh, or a citation audit failing in CI
repo: visionclaw
domain: BASELINE-architecture
lineage: Phase 1 diagram work re-derived every cited line; Phase 2 removals moved more.
---

# ADR-2048 — Correct the drifted file:line citations in the boot, identity and posture governing docs

## Context

The governing docs cite `file:line` as their compliance surface, and both carry a
`verified_commit` of `73540faa0` while the working tree has moved well past it.
Phase 1 re-derived every cited location for the diagram tree and Phase 2's
removals shifted more. Several citations pointed at the wrong line, and three
pointed at a *different construct entirely* — a comment or an unrelated setting —
which is worse than a stale number, because a reader who follows the reference
finds something plausible and concludes the doc is right.

Two of these were found by other leads reviewing my edits (vc-clients on the
compose and role-default citations, vc-knowledge on the NIP-98 section), which is
the mechanism working as intended.

## Decision

Every `file:line` in the sections this record lists is re-verified by opening the
line, not by arithmetic on a diff. Where a citation named a construct rather than
a number — a comment line, a doc-comment, the wrong env var — the text is
corrected to name the construct that actually implements the behaviour, so the
citation survives the next code move better than a bare line number would.

Citations are corrected in place rather than annotated as drifted: the doc's job
is to be right now, and the history lives in this record.

## Consequences

- `docs/BASELINE-architecture.md` and `docs/IDENTITY-authority-chain.md` are
  accurate at this working tree. Both still carry `verified_commit: 73540faa0` in
  frontmatter; refreshing that is the owner's call at the landing commit, and is
  this record's review trigger.
- Where a behaviour is implemented by a named function, the doc now cites the
  function and its line rather than a line alone (e.g.
  `UserRole::default_authenticated()`), which degrades more gracefully.
- No code changed.

## Verification

**`docs/BASELINE-architecture.md`, "Trust boundaries" / shipped posture** — three
compose citations were wrong, two of them pointing at unrelated lines:

| claim | was | is | evidence |
|---|---|---|---|
| `RBAC_PUBLIC_READS=1` | `docker-compose.unified.yml:78` | `:93` | `:78` is a comment — "SCOPE (codex review, ADR-2039): this line lives in the DEV service only" |
| `RBAC_ALLOW_OWNERLESS=1` | `:79` | `:94` | |
| `PUBKEY_VISIBILITY_FILTER=1` | `:86` | `:107` | |
| unassigned pubkey ⇒ Editor | `role_store.rs:191` | `src/models/rbac.rs:70` (`UserRole::default_authenticated()`), reached from `parse_default_role` at `role_store.rs:197-198` | `role_store.rs:191` is a doc-comment line |

The compose and role-default corrections were routed from vc-clients and verified
independently before applying (`sed -n '78p;93p;94p' docker-compose.unified.yml`,
`sed -n '66,72p' src/models/rbac.rs`).

**`docs/IDENTITY-authority-chain.md`, "Request signing: NIP-98"** — the whole
section had drifted by roughly 60 lines. Every reference re-verified against
`src/utils/nip98.rs`: `validate_nip98_token` `:270`→`:330`; kind check
`:288`→`:348` (plus the `HTTP_AUTH_KIND` const at `:20`); `TOKEN_MAX_AGE_SECONDS`
`:168`→`:169` with past/future arms `:302`/`:307`→`:362`/`:367`; tag match
`:328`-`:349`→`:376`-`:389`; `urls_match` `:463`→`:524`; payload hash →
`compute_payload_hash` `:132`, applied `:413`; signature verify `:365`→`:426`;
replay claim `:374`→`:435`; `claim_event_id` `:199`→`:234`; `REPLAY_CACHE`
`:187`→`:215` (plus `replay_cache()` at `:217`). The constant the doc called
`REPLAY_CACHE_TTL_SECONDS` at `:177` does not exist under that name — it is
`REPLAY_CACHE_TTL` at `:178`, compared at `:255`. Routed from vc-knowledge as
ADR-2070; the single line they flagged turned out to be the whole section.

**`docs/IDENTITY-authority-chain.md`, "Session tokens"** — the claim "expiry is
enforced against `last_seen + token_expiry` (`:212`, `:482`)" was false for
`get_session`, which enforced no expiry at all. Rewritten under ADR-2044 to
describe the shared `session_is_fresh` rule, with `get_session` at `:574`,
`validate_session` at `:478` and the helper at `:597`.

**`docs/BASELINE-architecture.md`, "Crate layout"** — the census was stale
independently of any line drift: "the root plus **nine** members" is now eleven
(`vault-migrate` and `visionclaw-integration-tests` were added), the
`graph-cognition-extract` orphan described as "present on disk" has been removed,
and `src/actors/*.rs` is 23 files, not 25, after ADR-2045. The
`crates/visionclaw-actors/src` figure of 11 is the recursive count (4 at the top
level plus 7 under `messages/`) and is unchanged — recorded explicitly because
ADR-2005 quotes the top-level 4 and BASELINE quotes the recursive 11, which reads
as a contradiction until the bases are stated.

**Verification ran on the uncommitted working tree above
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4` and must be re-run at the landing
commit, which sets `verified_paths`.**
