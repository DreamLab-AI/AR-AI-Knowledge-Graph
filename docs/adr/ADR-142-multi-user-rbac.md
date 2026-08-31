# ADR-142: Multi-User RBAC bound to NIP-98 pubkeys

## Status

Accepted

## Date

2026-08-31

**Relates:** ADR-040 (enterprise SSO / role hierarchy — the parked OIDC half),
ADR-131 §3 (AUTH-001 doc-drift correction — established that the four-tier
`enterprise_auth.rs` was *branch-only*, never on `main`), ADR-120 (did:nostr
agent identity — pubkey-as-DID), ADR-11 §D5 (SQLite settings owner-layer — the
per-user settings substrate reused here), `src/utils/auth.rs`
(`AccessLevel`/`verify_access` — the pre-existing lattice this extends),
`sprint-3/jss-cut-scaffold` @ `6520d6f2e` `src/middleware/enterprise_auth.rs`
(the ported design reference). Resolves TODO-unified **C-2**, **T-3**, and the
**T-4** multi-user-DIDs held surface; closes **AUTH-001** by implementation.

## Context

`main` shipped NIP-98 Schnorr auth (`src/utils/nip98.rs`), a coarse
`AccessLevel` lattice with `verify_access` (`src/utils/auth.rs`), a `RequireAuth`
middleware, and a binary `POWER_USER_PUBKEYS` allowlist. Three gaps blocked
genuine multi-user operation (the AUTH-001 banner and the CQRS gap analysis):

1. **No persisted per-user roles.** Every authenticated user was one of exactly
   two things — power user (→ `Admin`) or not (→ `Authenticated`). There was no
   way to grant a *specific* pubkey Editor/Viewer/Admin and have it stick.
2. **No sweep-wide enforcement.** 15+ mutating endpoints were reachable
   unauthenticated because each handler opted into its own guard (or didn't).
3. **The reference RBAC was unmergeable.** `enterprise_auth.rs` on
   `sprint-3/jss-cut-scaffold` predates months of handler churn *and* keys off a
   spoofable `X-Enterprise-Role` header with a workflow taxonomy
   (Admin/Broker/Auditor/Contributor) unsuited to a Nostr-identity system.

## Decision

Port the *intent* of the reference four-tier RBAC into `main`, but bound to the
cryptographically-verified NIP-98 pubkey (the user's DID) rather than a header,
and layered onto the existing `AccessLevel` machinery instead of replacing it.

### 1. Role model (`src/models/rbac.rs`)

A persisted `UserRole` lattice: `Owner (4) > Admin (3) > Editor (2) > Viewer
(1)`. It maps onto the legacy `AccessLevel` so every existing guard keeps
working:

| UserRole | → AccessLevel     | Effective capability                    |
|----------|-------------------|-----------------------------------------|
| Owner    | `Admin`           | everything + grant/revoke Admin & Owner |
| Admin    | `Admin`           | user/settings management                |
| Editor   | `Authenticated`   | read + graph/content mutation           |
| Viewer   | `ReadOnly`        | read-only                               |

Assignment is lattice-constrained (`UserRole::can_assign`): only an Owner may
grant Owner or Admin; an Admin may grant only Editor/Viewer; nobody can escalate
a user past a role they could not themselves assign.

**Default = Editor, not Viewer.** `main`'s pre-RBAC `verify_access` mapped every
authenticated NIP-98 user to `Authenticated` (read + write-graph). Defaulting an
unassigned user to `Editor` preserves that behaviour exactly, so switching RBAC
on does not silently revoke write access. `Viewer` is an explicit, admin-applied
downgrade.

### 2. Persistence (`src/services/role_store.rs`)

A dedicated `user_roles(pubkey PRIMARY KEY, role, assigned_by, updated_at)` table
sharing the settings SQLite `Connection`. Deliberately *not* the user-writable
settings owner-layer — a user must never be able to write their own role through
a settings-write endpoint. A process-global handle (`global_role_store`) lets
`verify_access` resolve roles without threading a parameter through every call
site; it is installed once at startup and is non-fatal on failure (auth degrades
to the legacy power-user mapping rather than taking the server down).

Owner bootstrap: `RBAC_OWNER_PUBKEY` grants Owner on startup if that pubkey has
no explicit row (idempotent). The legacy `POWER_USER_PUBKEYS` allowlist still
resolves to `Admin` for pubkeys without an explicit assignment, easing
migration.

### 3. Central enforcement (`src/middleware/rbac_gate.rs`)

Rather than edit 40 handler `configure` fns (high conflict risk with concurrent
editors, and drift-prone), a single `RbacGate` Transform wraps the whole `/api`
scope. Policy mirrors the codebase's own `RequireAuth::mutations_only()` idiom —
**open reads, gated writes** — so the many public GETs the client depends on keep
working while the real hole (unauthenticated mutations) is closed:

- **Public allowlist** (any method): `/api/auth/*` (self-authenticating — you
  cannot require a session to create one), `/api/client-logs`, health probes.
- **`/api/admin/*`** → `Admin` for every method (sensitive to read and write).
- **Safe methods** elsewhere → public (per-user handlers already use
  `OptionalAuth`).
- **Mutations** elsewhere → `WriteSettings` under `/api/settings`, otherwise
  `Authenticated` (Editor+).

`RBAC_GATE_MODE` selects `enforce` (default; denials return 401/403) or `report`
(denials logged, request proceeds) as an operator escape hatch during rollout.

### 4. Admin surface (`src/handlers/admin_rbac_handler.rs`)

NIP-98-authenticated management under `/api/admin/rbac`:

| Method | Path                                  | Min role | Purpose                    |
|--------|---------------------------------------|----------|----------------------------|
| GET    | `/api/admin/rbac/whoami`              | any auth | caller's own resolved role |
| GET    | `/api/admin/rbac/users`               | Admin    | list explicit assignments  |
| PUT    | `/api/admin/rbac/users/{pubkey}/role` | Admin    | assign a role              |
| DELETE | `/api/admin/rbac/users/{pubkey}/role` | Admin    | revert to default role     |

### 5. Per-user settings

Already provided by ADR-11 §D5's settings owner-layer (`owner_pubkey` column;
global layer is `''`, per-user rows key by pubkey; `GET/PUT /api/user/filter`
and the per-user `/api/settings/all` layer already round-trip through
`CURRENT_OWNER_PUBKEY`). Multi-user settings isolation therefore needs no new
storage — it is inherited. Actor-authoritative fields (e.g. `layout_mode`) stay
global as before.

## Consequences

**Positive**
- Roles bind to a verified DID, not a spoofable header — strictly stronger than
  the reference design.
- The unauthenticated-mutation gap closes in one reviewed file, not 40.
- Zero regression for existing authenticated users (Editor default) and existing
  power users (still Admin).
- New taxonomy layers onto `AccessLevel`, so `RequireAuth`/`verify_access` and
  their tests are untouched.

**Negative / follow-up**
- The gate authenticates per mutating request (NIP-98 verification cost) — the
  same cost the per-handler guards already paid, now uniform.
- Internal automation that mutates `/api/*` must present a valid identity; the
  `report` mode exists precisely to surface any such caller before enforcing.
- OIDC/SAML SSO and SCIM (ADR-040 Phase 1/2) remain parked — this ADR delivers
  the Nostr-native RBAC half, not enterprise IdP federation.
- Role changes take effect on the next request (no session-role caching); a
  revoked Admin is denied immediately, which is the desired security property.

## Security hardening (adversarial review)

An adversarial review (Codex) surfaced six issues, all fixed before acceptance:

1. **Dev-token bypass (HIGH).** `Bearer dev-session-token` satisfied the whole
   lattice with an arbitrary `X-Nostr-Pubkey` in any debug build. Now triple-
   gated: compile-time (`debug_assertions`/`dev-auth`) **and** explicit runtime
   opt-in `DEV_AUTH_LOOPBACK=1` **and** a loopback peer address. A remote caller
   on a debug deployment can no longer reach it (`dev_bypass_permitted`).
2. **Owner lockout (MED).** With `RBAC_OWNER_PUBKEY` unset and an empty store, no
   one could ever mint an Owner/Admin. Startup now **fails closed** when no Owner
   exists, unless `RBAC_ALLOW_OWNERLESS=1` (documented escape for legacy single-
   user deployments where the `POWER_USER_PUBKEYS`→Admin fallback is intended).
3. **Report-mode bypass (MED).** `RBAC_GATE_MODE=report` is one typo from
   disabling auth. It now refuses to activate unless the build has
   `debug_assertions` **or** `RBAC_REPORT_MODE_ACK` equals today's UTC date, and
   it logs at `error` level at startup and on every request it waves through.
4. **Public-read default (MED).** Anonymous reads are now an *explicit, visible*
   switch (`RBAC_PUBLIC_READS`, default on for the single-operator deployment)
   rather than an invisible structural default; with it off, reads require
   authentication. WebSocket upgrades live at top-level `/ws*` (outside `/api`),
   so they are unaffected either way.
5. **Prefix matching (LOW).** Path matching is segment-aware (whole `/`-segments)
   so `/api/administrator` does not inherit `/api/admin` policy and
   `/api/health-x` does not inherit the public allowlist.
6. **Schema hardening (LOW).** `user_roles` carries `CHECK` constraints (role in
   the canonical set; pubkey 64-hex); `set` validates the pubkey; a present-but-
   unparseable stored role is a hard `RoleStoreError::InvalidRole`, and
   `effective_role` **fails closed to Viewer** on any error — never up to
   Admin/power-user.

A related correctness fix fell out of (1)/(6): the default mutation requirement
is `AccessLevel::WriteGraph`, not `Authenticated`. `has_permission` treats a
required `Authenticated` as "any authenticated user" — a Viewer would have
passed. `WriteGraph` is satisfied by Editor/Admin but not by a Viewer(→ReadOnly),
so Viewers are correctly denied writes.

### Round-2 review (estate-wide + fail-closed)

A second review pass required the fixes be applied **estate-wide** and the
defaults be **fail-closed**:

1. **Dev-token estate-wide.** The loopback gate now covers *every* place the
   literal `dev-session-token` is compared: `verify_access`, the REST
   `auth_extractor` (both Bearer paths), and the WS `filter_auth` handshake — all
   route through the one `dev_bypass_permitted[_for_addr]` helper (the WS path
   captures eligibility from the handshake peer address into `dev_bypass_ok`).
2. **Store-init fatal.** A failed `RoleStore::new` now aborts startup (was
   fail-open: it continued without the store, silently reverting to the legacy
   power-user mapping).
3. **Public-reads fail-closed.** `RBAC_PUBLIC_READS` **absent = reads require
   auth**. The single-operator deployment opts back into anonymous reads
   *visibly* by setting `RBAC_PUBLIC_READS=1` in `docker-compose.unified.yml`
   (see *Operational configuration*). Absence of a security flag never widens
   access.
4. **Last-Owner + transactional.** Admin role changes go through
   `RoleStore::assign_checked`/`revoke_checked`, which perform the current-role
   read, the `can_assign` checks, the last-Owner count, and the write in **one
   SQLite transaction** (no TOCTOU). Demoting/removing the sole Owner is refused
   (`409`).
5. **Canonicalisation.** One `canonicalise_pubkey` (trim + lowercase) is used by
   get/set/remove/bootstrap AND the resolve path, so a mixed-case pubkey always
   hits the same row.
6. **Legacy-table migration.** `RoleStore::new` detects a pre-constraint
   `user_roles` table (its `sqlite_master.sql` lacks `CHECK`) and rebuilds it
   with constraints inside a transaction, validating existing rows (invalid ones
   are dropped → the user reverts to default; count logged).

### Operational configuration

RBAC reads three env vars; all defaults are fail-closed. The repo's
`docker-compose.unified.yml` sets the single-operator values explicitly:

| Var | Default | Deploy value | Meaning |
|-----|---------|--------------|---------|
| `RBAC_PUBLIC_READS` | off (auth required) | `1` | anonymous `/api` GET reads |
| `RBAC_ALLOW_OWNERLESS` | off (startup fails) | `1` | permit boot with no Owner (POWER_USER→Admin only) |
| `RBAC_OWNER_PUBKEY` | unset | unset | 64-hex pubkey bootstrapped to Owner |
| `RBAC_GATE_MODE` | `enforce` | unset | `report` needs debug build or `RBAC_REPORT_MODE_ACK=<today UTC>` |
| `DEV_AUTH_LOOPBACK` | off | unset | dev-token bypass (dev builds, loopback only) |

To move off the owner-less fallback: set `RBAC_OWNER_PUBKEY` to the operator's
pubkey, boot once (bootstraps Owner), then set `RBAC_ALLOW_OWNERLESS=0`.

## Testing

- `src/models/rbac.rs` — lattice ordering, `satisfies`, `AccessLevel` mapping,
  parse round-trip/aliases, assignment-escalation rules, serde wire form.
- `src/services/role_store.rs` — default/power-user/explicit resolution
  precedence, upsert, remove, list (in-memory SQLite).
- `src/middleware/rbac_gate.rs` — `required_level` policy table (public reads,
  gated writes, admin surface, settings writes).
- `tests/adr142_rbac_gate.rs` — end-to-end route-guard: public read passes,
  unauthenticated mutation rejected, auth endpoint allowlisted, admin surface
  rejected unauthenticated.
