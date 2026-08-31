---
id: ADR-2027
title: Three named deployment profiles, with the shipped compose booting demo-open
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: eac01130366a25d758e2421ce6718b7854ab9174
verified_paths: [src/middleware/rbac_gate.rs, src/main.rs, src/services/role_store.rs, src/handlers/socket_flow_handler/position_updates.rs, docker-compose.unified.yml]
owner: jjohare
review_trigger: adding a fourth profile, machine-selecting a profile at boot, or changing a compose security default
repo: visionclaw
domain: SECURITY-profiles
lineage: legacy ADR-142 (RBAC lattice + flag surface), ADR-011 (auth-enforcement exceptions the profiles honour)
---

# ADR-2027 — Three named deployment profiles, with the shipped compose booting demo-open

## Context

The fail-closed flag surface (ADR-2026) is composable, so the space of legal
security postures is large. Left unnamed, each deployment invents its own flag
soup and no one can say whether a given combination is supported. The estate
needs a small, ratified set of postures. The shipped `docker-compose.unified.yml`
must also preserve the legacy single-operator experience (anonymous graph reads,
owner-less boot) without reintroducing an open code default. ADR-142 defined the
RBAC lattice and flags; ADR-011 defined the auth exceptions each profile honours.

## Decision

Security posture is exactly three ratified profiles — **demo-open**,
**single-tenant**, **multi-user-locked** — each an exact flag set, with any flag
left unlisted taking its fail-closed code default (ADR-2026). The four composable
flags are `RBAC_PUBLIC_READS` (`rbac_gate.rs`), `RBAC_ALLOW_OWNERLESS`
(`main.rs`), `RBAC_DEFAULT_ROLE` (`role_store.rs`, added 2026-08-31 — the
multi-user-locked profile sets `viewer` so unknown signers cannot write), and
`PUBKEY_VISIBILITY_FILTER` (`position_updates.rs`).
`docker-compose.unified.yml` realises demo-open by inverting two fail-closed code
defaults (`RBAC_PUBLIC_READS:-1`, `RBAC_ALLOW_OWNERLESS:-1`). Any combination
outside the three profiles is unsupported. This forecloses ad-hoc flag mixes
being treated as legitimate deployments.

## Consequences

- A reviewer can classify any deployment by matching its env to one of three
  named rows in `docs/SECURITY-profiles.md`; a mismatch is a defect, not a
  variant.
- The shipped compose is deliberately the *least* locked profile (demo-open);
  operators moving to production must switch profiles, not merely tweak a flag.
- Profiles are documented prose, **not** machine-selected — nothing at boot
  asserts the running env matches a named profile, so drift is possible until a
  selector lands (hence `implementation_status: partial`, `activation: staged`).
- The editor-default facet is owned by ADR-2010 and the report-mode dated-ack by
  ADR-2012 (both cross-ref), not re-decided here.

## Verification

Re-checked at `e0f8cd896`: the three composable flags read fail-closed at
`src/middleware/rbac_gate.rs:121-128`, `src/main.rs:717-739`, and
`src/handlers/socket_flow_handler/position_updates.rs:34-42`
(`parse_visibility_flag` defaults ON). `docker-compose.unified.yml` inverts two of
them for demo-open: `RBAC_PUBLIC_READS: "${RBAC_PUBLIC_READS:-1}"` and
`RBAC_ALLOW_OWNERLESS: "${RBAC_ALLOW_OWNERLESS:-1}"` (against code defaults of
`false`), with `PUBKEY_VISIBILITY_FILTER:-1` matching the code default. No
boot-time profile selector exists (grep) — profiles are documentation, confirming
the partial/staged statuses.
