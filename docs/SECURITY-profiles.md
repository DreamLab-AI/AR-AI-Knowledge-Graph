---
title: Security Profiles & Flag Matrix
doc_id: VC-SECURITY
version: 0.1.0
status: draft-for-ratification
verified_commit: 73540faa0
sources:
  - src/middleware/rbac_gate.rs
  - src/services/role_store.rs
  - src/models/rbac.rs
  - src/handlers/socket_flow_handler/position_updates.rs
  - crates/visionclaw-domain/src/utils/visibility_filter.rs
  - src/utils/nip98.rs
  - src/handlers/socket_flow_handler/http_handler.rs
  - src/main.rs
  - docker-compose.unified.yml
  - .env
date: 2026-08-31
---

# Security Profiles & Flag Matrix

## Purpose

Single source of truth for every security-relevant runtime flag, the three
validated deployment profiles built from them, and the illegal combinations the
operator must never ship. Ground-truth order: live code > audit facts > legacy ADR prose.

## Current State

### Flag inventory (from code)

Every flag below is read directly from process env. The **code default** is what
the binary does with the var *absent*; the **compose default** is what
`docker-compose.unified.yml` injects. Where they differ, the compose value is the
shipped posture — that gap is called out in Known divergences.

| Flag | Purpose | Code default | Source (file:line) |
|------|---------|--------------|--------------------|
| `RBAC_PUBLIC_READS` | Allow anonymous safe-method (`GET`/`HEAD`/`OPTIONS`) reads across `/api` | **OFF** (fail-closed: absence requires auth) | `src/middleware/rbac_gate.rs:121-128` |
| `RBAC_ALLOW_OWNERLESS` | Permit boot with no Owner assigned in the role store | **OFF** (fail-closed: refuses to start, exit on no Owner) | `src/main.rs:717-739`, `src/services/role_store.rs:33` |
| `PUBKEY_VISIBILITY_FILTER` | ADR-060 drop-set: strip private nodes not owned by the session pubkey from the wire frame | **ON** (secure-by-default; only explicit `0`/`false`/`off`/`no` disables) | `src/handlers/socket_flow_handler/position_updates.rs:34-43` |
| `RBAC_GATE_MODE` | `report` downgrades `/api` denials to logs instead of 401/403 | **enforce** (`report` refuses to activate in release without dated `RBAC_REPORT_MODE_ACK`) | `src/middleware/rbac_gate.rs:80-113` |
| `SETTINGS_AUTH_BYPASS` | Legacy dev auth bypass | Codepath `#[cfg]`-stripped in release; **presence hard-fails boot** (exit 2) | `src/main.rs:117-155` |
| `VISIONCLAW_DEV_MODE` | **ADR-2039** LAN-local full auth bypass: in dev/`dev-auth` builds, `=1`/`true` grants dev-admin (`dev-mode-local-admin`) to **every** request — no NIP-98/token/peer check — across REST (`verify_access`), settings extractor, and WS handshake. Peer-agnostic by design (Docker SNAT hides origin). | **OFF** (dev builds); codepath `#[cfg]`-stripped in release + **presence hard-fails boot** (exit 2). Loud boot banner when armed. | `src/utils/auth.rs` (`dev_full_bypass_active`), `src/main.rs:120-133` |
| `ALLOW_INSECURE_DEFAULTS` / `--allow-skip-auth` | Dev auth relaxations | Same release refusal as above | `src/main.rs:120-133` |
| `APP_ENV` | Production signal; drives strict env validation & CORS lockdown | unset ⇒ non-production (permissive) | `src/main.rs:85`, `src/main.rs:887` |
| `RBAC_OWNER_PUBKEY` | Bootstrap the Owner role at startup | unset ⇒ no owner bootstrapped | `src/main.rs:709`, `src/services/role_store.rs` (`bootstrap_owner_from_env`) |
| `POWER_USER_PUBKEYS` | Legacy pubkeys mapped to `Admin` when unassigned | unset ⇒ no power users | `src/services/role_store.rs:194-203` |
| `VISIONCLAW_DEV_TOKEN` | Dev session token (`.env:197`) | dev builds only | `.env:197` |

`POD_DEFAULT_PRIVATE` and a distinct `NIP98_OPTIONAL_AUTH` env var are **not
present in this repo's code** at this commit — Pod-privacy defaults and NIP-98
optionality are governed structurally, not by these named vars (see divergences).

### Default-role & RBAC lattice

- Effective role precedence: explicit assignment → `Admin` if power-user → else
  the configured unassigned-signer default: **`RBAC_DEFAULT_ROLE`**
  (`editor`|`viewer`, parsed fail-closed in `src/services/role_store.rs`
  `parse_default_role`), defaulting to **`Editor`** for pre-RBAC compatibility.
  Under `viewer`, an *unassigned but authenticated* pubkey reads only until an
  Admin grants a role. Unrecognised values (including `admin`/`owner`) fail
  closed to `viewer` — the env var can narrow access, never widen it.
- Any error in role lookup **fails closed to `Viewer`**, never up
  (`src/services/role_store.rs:204-208`).
- `/api` gate policy (`rbac_gate.rs:133-169`): public allowlist
  (`/api/auth/*`, `/api/client-logs`, health probes); `/api/admin/*` needs `Admin`
  for every method; safe reads public iff `RBAC_PUBLIC_READS`; mutations need
  `WriteSettings` under `/api/settings`, else `WriteGraph` (denies Viewer writes).

### NIP-98 replay resistance (landing 2026-08-31)

Two-layer scheme (`src/utils/nip98.rs`): (1) a 60s freshness window
(`TOKEN_MAX_AGE_SECONDS`), and (2) a **single-use replay cache** — after
validation the event id is atomically claimed under a mutex
(`claim_event_id`) with a TTL of `2 × 60s`, closing the
previously-open replay-within-window gap. Concurrent presentations of the same id
cannot both win (single lock acquisition). The cache keys on a **monotonic
`Instant`** so a backward clock step cannot extend entry lifetimes, and enforces
a hard `REPLAY_CACHE_MAX_ENTRIES` ceiling (100 000): once full, new auth is
rejected fail-closed with `ReplayCacheFull` (mapped to 503) rather than evicting
a live id — evict-oldest would let a signature flooder purge a genuine id and
re-enable replay.

**Process-local scope:** the cache lives in one process's memory. Replay
protection does **not** span replicas; horizontal scaling requires shared
storage (e.g. Redis) or sticky routing so every presentation of a token lands on
the same process.

### Pubkey visibility filter (landing 2026-08-31)

Default flipped **ON** in the handler gate (`position_updates.rs:34-43`). The
pure drop-set logic (`visibility_filter.rs`) is fail-closed: a missing session
pubkey drops every private node, yielding a public-only graph. Public nodes are
unaffected, so default-on is behaviour-neutral for all-public deployments.

### Named validated profiles

Each profile is an **exact** flag set. Anything not listed takes its code default.

| Flag | `demo-open` | `single-tenant` | `multi-user-locked` |
|------|-------------|-----------------|---------------------|
| `RBAC_PUBLIC_READS` | `1` | `0` | `0` |
| `RBAC_ALLOW_OWNERLESS` | `1` | `1` | `0` |
| `RBAC_OWNER_PUBKEY` | unset | set (64-hex) | set (64-hex) |
| `RBAC_DEFAULT_ROLE` | `editor` | `editor` | `viewer` |
| `PUBKEY_VISIBILITY_FILTER` | `1` | `1` | `1` |
| `RBAC_GATE_MODE` | enforce | enforce | enforce |
| `APP_ENV` | `production` | `production` | `production` |
| `SETTINGS_AUTH_BYPASS` etc. | unset | unset | unset |

- **`demo-open`** — public read-only kiosk. Anonymous reads on, no owner required,
  visibility filter still on so only `public::true` nodes reach the wire. Writes
  still require an Editor+ session. This is the closest profile to today's shipped
  compose posture.
- **`single-tenant`** — one operator, private graph. Reads require auth; an Owner
  is bootstrapped; ownerless boot tolerated for the legacy `POWER_USER_PUBKEYS`
  path. The unassigned-⇒-Editor default is acceptable because the operator
  controls who can authenticate.
- **`multi-user-locked`** — hardened multi-tenant. Owner mandatory
  (`RBAC_ALLOW_OWNERLESS=0` ⇒ hard-fail without an Owner), no anonymous reads,
  visibility filter on, and `RBAC_DEFAULT_ROLE=viewer` so an unknown-but-valid
  NIP-98 signer is read-only until an Admin grants a role (least-privilege
  admission; closes the former open item where any valid signer was an Editor).

### Illegal combinations

| Combination | Why it is illegal | Enforcement |
|-------------|-------------------|-------------|
| `SETTINGS_AUTH_BYPASS` / `VISIONCLAW_DEV_MODE` / `ALLOW_INSECURE_DEFAULTS` set in a release build | Promotes dev auth relaxation to production | **Hard-fail at boot**, `src/main.rs:117-155` |
| `--allow-skip-auth` argv in release | Same | Hard-fail, `src/main.rs:120-126` |
| `NODE_ENV=development` + `DOCKER_ENV` both set | Dev config in a container image | Hard-fail, `src/main.rs:141-147` |
| `RBAC_GATE_MODE=report` in release without `RBAC_REPORT_MODE_ACK=<today UTC>` | Would silently disable `/api` auth | Refuses to activate, falls back to enforce, `rbac_gate.rs:88-102` |
| `RBAC_ALLOW_OWNERLESS=0` with no `RBAC_OWNER_PUBKEY` and no prior Owner | Permanent lockout / unmanageable RBAC | Refuses to start (`PermissionDenied`), `src/main.rs:729-739` |
| `RBAC_PUBLIC_READS=1` with `PUBKEY_VISIBILITY_FILTER=0` | Anonymous reads *and* private nodes on the wire = full data disclosure | **Not machine-enforced** — operator must never combine; flagged here |
| `APP_ENV` unset in a public deployment | Skips strict env validation and CORS lockdown (`main.rs:85,887`) | Not enforced; profiles pin `APP_ENV=production` |

## Known divergences & open items

- **Shipped compose posture is open-by-default, and inverts two code defaults.**
  `docker-compose.unified.yml:78-86` sets `RBAC_PUBLIC_READS=1` and
  `RBAC_ALLOW_OWNERLESS=1` — both are code-default **OFF/fail-closed**
  (`rbac_gate.rs:127`, `main.rs:719`). The image therefore boots owner-less with
  anonymous reads unless an operator overrides the `.env`. `PUBKEY_VISIBILITY_FILTER=1`
  in compose matches the new code default. Net shipped posture ≈ `demo-open`. This
  is a deliberate compatibility trade-off (matches legacy ADR-142 open-by-default)
  and needs a named, ratified security profile before any multi-tenant deployment.
- **Unassigned authenticated pubkey ⇒ Editor** (`rbac.rs:68`,
  `role_store.rs:194-203`). Write-capable by default; contradicts least-privilege
  for `multi-user-locked`. No flag exists to select the default role — code change
  required.
- **`?token=` accepted on `/wss`** (`http_handler.rs:342-354`) — contradicts legacy
  ADR-011 (header-only). Session token in the query string is a log-hygiene /
  referrer-leak risk (medium). Header path exists; query path should be removed or
  gated.
- **`SETTINGS_AUTH_BYPASS=false` and `APP_ENV=development` sit in `.env`**
  (`.env:36,39`). Harmless in a dev build; in a release image the mere presence of
  `SETTINGS_AUTH_BYPASS` hard-fails boot — the `.env` must be scrubbed for release.
- **SOPS never executed** (legacy ADR-109, Accepted 2026-05-09): `.env` is
  plaintext today, no SOPS artifacts in-tree. Secrets management is an open item.
- **agentbox AoE :9095 token auth** — staged for the next image rebuild (was
  `--auth none` on loopback with tokenless direct routes). Not yet in the running image.
- **Key custody / rotation frozen** (legacy ADR-081) and **delegated admin frozen**
  (legacy ADR-094), both 2026-07-03. NIP-26 delegation not wired — the Nostr bridge
  re-signs under the bridge key (fail-closed NIP-26 deferred to unbuilt Phase 5).
- **`POD_DEFAULT_PRIVATE` / `NIP98_OPTIONAL_AUTH` absent** as named env vars — the
  brief's expected flags do not exist at this commit; Pod default-privacy and NIP-98
  optionality are structural, not flag-driven. Recorded so a future reader does not
  hunt for a non-existent switch.

## Invariants (must not silently change)

1. Absence of any security flag must **widen nothing** — every flag fails closed
   except `PUBKEY_VISIBILITY_FILTER`, which fails *safe* (filter on) with the same effect.
2. `default_authenticated()` and the `Editor` mapping are load-bearing: changing
   the default role silently re-grants or revokes write access estate-wide.
3. Release builds must retain the boot-time refusal of dev-auth env/argv
   (`main.rs:117-155`) and the report-mode acknowledgement gate.
4. NIP-98 must keep both layers — window **and** single-use cache. Removing the
   cache re-opens replay-within-60s. The cache is **process-local** by design:
   replay protection does not span replicas, so any multi-replica deployment must
   add shared-state (Redis) or sticky routing before it can rely on this
   invariant. The hard capacity ceiling must fail closed (reject, `ReplayCacheFull`
   → 503), never evict a live id.
5. `RBAC_PUBLIC_READS=1` and `PUBKEY_VISIBILITY_FILTER=0` must never coexist in a
   deployed profile (full-disclosure combination).

## Change process

Any new security-relevant env var, or any change to a default in this matrix,
requires: (1) updating this table with the new file:line; (2) confirming the
fail-closed invariant; (3) adding it to the illegal-combinations table if it can
interact badly; (4) bumping `version` and re-recording `verified_commit`. Compose
default changes require an accompanying named-profile update. Legacy ADR prose is
evidence, not authority — cite it, do not defer to it.
