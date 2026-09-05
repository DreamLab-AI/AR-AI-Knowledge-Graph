---
id: ADR-2087
title: Correct the deployment-profile documentation to match the code defaults
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any change to a profile flag's code default, to the RBAC lattice env surface, or to the WS upgrade auth path
repo: visionclaw
domain: SECURITY-profiles
lineage: ADR-2027 (three named profiles), ADR-2026 (fail-closed posture), ADR-2010 (editor default), ADR-2003 (visibility-filter illegal combination), ADR-2038 (boot assertion, routed to vc-core)
---

# ADR-2087 — Correct the deployment-profile documentation to match the code defaults

## Context

Phase 1 diagrams ES-10.1, ES-10.3, ES-10.4 and ES-10.5 were drawn from code and
exposed four documentation defects around the security-profile surface:

1. `docs/SECURITY-profiles.md` "Known divergences" asserts *"No flag exists to
   select the default role — code change required"*. The flag exists:
   `RBAC_DEFAULT_ROLE_ENV` (`src/services/role_store.rs:41`), parsed by
   `parse_default_role` (`:195`) which fails closed to `viewer` on an
   unrecognised value (`:204`). The same document's profile table already lists
   `RBAC_DEFAULT_ROLE` per profile, so the file contradicts itself.
2. The same file cites the `?token=` WS query path at `http_handler.rs:342-354`.
   The query-parameter extraction is at `src/handlers/socket_flow_handler/http_handler.rs:139-152`
   (`.find(|(k, _)| k == "token")` at `:148`); the file is 376 lines, so the cited
   range is the wrong region.
3. `docs/DATA-authority-erasure.md` describes `RBAC_PUBLIC_READS` as
   *"`rbac_gate.rs:119-122`, default on"*. The code default is fail-closed —
   `public_reads_enabled()` ends `.unwrap_or(false)` (`src/middleware/rbac_gate.rs:128`).
   Only `docker-compose.unified.yml:93` makes it on. That file is owned by
   vc-knowledge and the correction is routed, not made here.
4. ADR-2003's illegal combination (`RBAC_PUBLIC_READS=1` with
   `PUBKEY_VISIBILITY_FILTER=0`) is the one row in the "Illegal combinations"
   table with no machine enforcement; ADR-2038 would supply it but is
   `proposed / none / inactive` with empty `verified_paths`.

## Decision

Documentation states the **code** default and the **compose** default as two
distinct facts, and never conflates them. Where a shipped compose value inverts a
fail-closed code default, both are named with their own `file:line`: the code
default cites the reading function, the deployed value cites the compose line.
Every flag named in the profile table must exist in code at a cited path; a
"Known divergences" bullet claiming a flag is absent is retired the moment the
flag lands. Citations in this document are line-exact and re-verified whenever the
cited file changes.

The compose defaults themselves are **not** changed: `RBAC_PUBLIC_READS=1` and
`RBAC_ALLOW_OWNERLESS=1` in `docker-compose.unified.yml:93-94` are ADR-2027's
deliberate demo-open posture, and remain.

Machine enforcement of the profile matrix stays ADR-2038's decision and is routed
to the owner of `src/main.rs` (vc-core); until it lands, the illegal-combination
row is documented as operator-discipline-only rather than implied to be enforced.

## Consequences

- The `RBAC_DEFAULT_ROLE` divergence bullet is retired; least-privilege admission
  for `multi-user-locked` is a configuration choice, not an outstanding code change.
- A reader can no longer conclude from `docs/SECURITY-profiles.md` that the code
  opens reads by default — the fail-closed default and the demo-open override are
  stated separately.
- The cross-document contradiction with `docs/DATA-authority-erasure.md` is
  closed only when vc-knowledge applies the routed correction; until then the two
  documents disagree and `docs/SECURITY-profiles.md` is the correct one.
- No behavioural change: this ADR changes documentation only.

### Update, same day — ADR-2038 landed

An implementation of ADR-2038 exists in the working tree, so the "machine
enforcement is routed and pending" framing above is superseded — but it is
**uncommitted and not the product of this pass**: `src/config/security_profile.rs`
is untracked (mtime 13:10) and the call site is absent from HEAD. It was not
written by vc-core, who surfaced it and corrected my initial mis-attribution.
`src/config/security_profile.rs` (1038 lines)
resolves the profile and `src/main.rs:873` calls
`assert_effective_profile_or_exit` before `HttpServer::new` (`:893`) and
`.bind()` (`:1146`), exiting 2 on any finding in a production artefact. ES-10.7
and ES-10.2 are redrawn accordingly.

Two consequences for **this** ADR's subject matter:

- The profile table in `docs/SECURITY-profiles.md` is now executable rather than
  advisory, which raises the cost of a citation error in it from "misleading" to
  "contradicts a boot gate". The line-exactness rule this ADR sets is therefore
  load-bearing, not housekeeping.
- The ADR-2003 illegal pair is still **not** enforced as a pair — it is caught
  only when a profile is explicitly declared, and an undeclared deployment
  landing on an unnamed combination still binds. The "Illegal combinations"
  table's `Not machine-enforced` cell for that row therefore stays accurate and
  must not be softened. Keying a rule on the pair is offered by vc-core and is a
  small addition to `evaluate_effective_profile` (`security_profile.rs:422`).

## Verification

Ran on the uncommitted working tree above SHA
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; `verified_paths` is empty because the
tree is uncommitted, and verification must be re-run at the landing commit.

- `grep -rn 'RBAC_DEFAULT_ROLE\|parse_default_role' src/ --include=*.rs` →
  `src/services/role_store.rs:41` (`RBAC_DEFAULT_ROLE_ENV`), `:186`, `:195`
  (`fn parse_default_role`), `:204` (fail-closed branch), `:213`. The flag exists;
  the divergence bullet claiming otherwise is stale.
- `grep -n 'token' src/handlers/socket_flow_handler/http_handler.rs` →
  `:135-136` (the SECURITY comment), `:139` (extraction begins), `:148`
  (`.find(|(k, _)| k == "token")`), `:152` (`match token.as_deref()`).
  `wc -l` on the same file → 376, confirming the previously cited `:342-354` was
  the wrong region.
- `sed -n '110,140p' src/middleware/rbac_gate.rs` → `fn public_reads_enabled()` at
  `:122` ending `.unwrap_or(false)` at `:128`, with the doc comment at `:116-121`
  stating that the absence of a security flag must never widen access.
- `grep -n 'RBAC_\|PUBKEY_VISIBILITY\|VISIONCLAW_DEV' docker-compose.unified.yml` →
  `:93` `RBAC_PUBLIC_READS: "${RBAC_PUBLIC_READS:-1}"`, `:94`
  `RBAC_ALLOW_OWNERLESS: "${RBAC_ALLOW_OWNERLESS:-1}"`, `:101`
  `RBAC_DEFAULT_ROLE: "${RBAC_DEFAULT_ROLE:-editor}"`, `:107`
  `PUBKEY_VISIBILITY_FILTER: "${PUBKEY_VISIBILITY_FILTER:-1}"`, `:85`
  `VISIONCLAW_DEV_MODE: "${VISIONCLAW_DEV_MODE:-0}"`.
- `node scripts/adr-index-gen.js docs/adr --check` → exits 0.
