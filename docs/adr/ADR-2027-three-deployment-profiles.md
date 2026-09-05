---
id: ADR-2027
title: Three named deployment profiles, with the shipped compose booting demo-open
date: 2026-08-31
decision_status: accepted
implementation_status: partial
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
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

### Re-verification 2026-09-05 (ADR-2087)

Verification ran on the **uncommitted working tree** above SHA
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; `verified_paths` is emptied because
the tree is uncommitted and the staleness gate cannot bind paths to a commit that
does not yet contain them. Both must be restored at the landing commit.

Every claim above re-confirmed against the current tree, with line numbers
refreshed where they had drifted:

- `public_reads_enabled()` at `src/middleware/rbac_gate.rs:122`, ending
  `.unwrap_or(false)` at `:128` — fail-closed, with the rationale comment at
  `:116-121`.
- Owner-less boot refusal at `src/main.rs:732-752` (`RBAC_ALLOW_OWNERLESS_ENV`
  read at `:732`; `FATAL` / `PermissionDenied` at `:746-752`).
- `parse_visibility_flag` at
  `src/handlers/socket_flow_handler/position_updates.rs:34`, defaulting ON, env
  const at `:26`.
- `RBAC_DEFAULT_ROLE_ENV` at `src/services/role_store.rs:41`, `parse_default_role`
  at `:195`, failing closed to `viewer` at `:204`.
- The demo-open inversions are now at `docker-compose.unified.yml:93-94`
  (they were cited as `:78-86` in prose elsewhere), with
  `RBAC_DEFAULT_ROLE:-editor` at `:101` and `PUBKEY_VISIBILITY_FILTER:-1` at
  `:107`.
- At the time of that grep there was **no** boot-time profile selector:
  `grep -rn 'demo-open\|single-tenant\|multi-user-locked' src/ --include=*.rs`
  returned nothing.

### Superseded within the same day — the selector landed

The statement immediately above is **no longer true of the working tree** and is
retained to date the transition honestly. `src/config/security_profile.rs`
(1038 lines) implements the selector, and `src/main.rs:868-878` calls
`assert_effective_profile_or_exit` at `:873` — before `HttpServer::new` (`:893`)
and `.bind()` (`:1146`), logging a boot receipt at `:879-883`. Drawn in
VC-09.4 / VC-09.5 / VC-09.6.

**Provenance and commit state matter here, so state them plainly.** The file is
**untracked** (`git status` → `??`, mtime 2026-09-05 13:10) and the call site is
**not in HEAD** (`git show HEAD:src/main.rs | grep -c assert_effective_profile_or_exit`
→ `0`). It is an uncommitted working-tree addition whose author is not
established — it is *not* the product of this remediation pass, and my earlier
grep returning nothing was correct at the time it ran.

Consequently **ADR-2038 should stay `proposed` until the implementation is
committed.** Recording a `verified_commit` now would point at
`b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`, which contains neither the file nor
the call site, so anyone re-verifying from a clean checkout would reproduce the
original "no selector exists" result and conclude the record was wrong. That is
the staleness failure the gate exists to catch, inverted. Either commit the
implementation and move the record with the landing SHA, or hold it `proposed`
with a note that an uncommitted implementation exists at those paths.

**Two corrections to this ADR's own Decision text follow from the
implementation:**

1. **"The four composable flags" is wrong — there are six.**
   `PROFILE_FLAGS` (`src/config/security_profile.rs:68`) is `[&str; 6]`:
   `RBAC_PUBLIC_READS`, `RBAC_ALLOW_OWNERLESS`, `RBAC_OWNER_PUBKEY`,
   `RBAC_DEFAULT_ROLE`, `PUBKEY_VISIBILITY_FILTER`, `RBAC_GATE_MODE`. This ADR's
   original four omitted `RBAC_OWNER_PUBKEY` and `RBAC_GATE_MODE`, both of which
   the profile table has always carried as rows. The doc comment at `:67` still
   reads "The four composable security flags ADR-2027 names" — it is faithfully
   echoing this ADR's stale count, so the root correction belongs here, not
   there.
2. **`implementation_status` and `activation_status` are no longer accurate at
   `partial`/`staged`** for the machine-selection facet. The queen should move
   this record and ADR-2038 together once the landing commit exists.

**What remains genuinely open** (reported by vc-core, not re-verified by me):
the ADR-2003 illegal pair `RBAC_PUBLIC_READS=1` + `PUBKEY_VISIBILITY_FILTER=0`
is caught only *indirectly* — it matches no named profile, so it raises
`ProfileDrift` when a profile is **declared** but merely classifies as `Unnamed`
when none is. There is no rule keyed on that pair, and an `Unnamed` classification
is not by itself fatal, so a production deployment declaring no profile and
landing on an unnamed combination still binds. ADR-2038's "a production selector
defaults to multi-user-locked" is therefore **not** implemented as a default —
there is no implicit profile.

## Closeout extension — 2026-09-04

CP-01/04/08. Owner remains jjohare with authentication/release maintainers. Partial/staged remains appropriate. Current compose explicitly defaults public reads and ownerless boot on, the unassigned role to Editor and visibility filtering on. These are source defaults, not an observed running profile. Four named settings do not capture every authority-changing mechanism: report mode, build features, full bypass, public prefixes and legacy power-user fallback need explicit profile treatment.

**Acceptance condition:** Define the effective profile across feature set, all bypass/report controls, role fallbacks, peer/proxy handling and public route exceptions. Test missing versus zero-valued variables, report-mode construction/date rollover/restart, ownerless/error paths and production artefact promotion. Bind configuration and binary identity to the same pre-listener acceptance receipt. Reopen on any security-relevant setting, build feature, route exception or fallback. See the [profile review](../../../VisionFlow/docs/estate-review/role-authority.md#profile-claims-and-effective-policy) and [source receipt](../../../VisionFlow/docs/estate-review/evidence/security-profile-snapshot.json). The prior nine helper cases retain matching source hashes; no new live profile or network test ran.

## Acceptance progress — 2026-09-05

**Implemented.** The consequence this record recorded — "profiles are documented
prose, **not** machine-selected; nothing at boot asserts the running env matches
a named profile, so drift is possible until a selector lands" — now has its
selector. `src/config/security_profile.rs` encodes the three ratified profiles
and their exact flag sets from `docs/SECURITY-profiles.md`:

* `VISIONCLAW_SECURITY_PROFILE` declares the intended profile. Every flag that
  does not match is reported as `ProfileDrift`, and drift is fatal in a
  production artefact.
* When undeclared, the observed flags are **classified** against the three
  profiles and the result appears in the boot receipt; an unrecognised
  combination is reported as `<unnamed>`, which this record calls unsupported.
* `RBAC_GATE_MODE` unset satisfies `enforce` (the code default), and a
  whitespace-only flag value is treated as unset, matching compose's
  `${VAR:-default}` interpolation.

**Tests.** `cargo test --lib --no-default-features security_profile` — 29
passed, 0 failed, including a test that constructs each ratified profile from
its own declaration and asserts it classifies as itself with no findings, and
one asserting that **all five** drifted flags in a mis-declared
`multi-user-locked` environment are reported, not just the first.

**Receipts.** `docs/estate-closeout/2026-09-05/adr-2012-2038-security-profile.txt`.

**Remains open.** The shipped `docker-compose.unified.yml` does not yet set
`VISIONCLAW_SECURITY_PROFILE`, so the deployed stack is classified rather than
asserted. Declaring it is a compose change with a deployment consequence and was
left for the owner.
