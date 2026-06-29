# PRD-06 — Auth & Security

Status   : Proposed
Date     : 2026-05-16
Owner    : anthropic@xrsystems.uk
Related  : ADR-06 (this section), ADR-02 D8 (WebSocket auth), ADR-05 (Settings),
           ADR-09 (Ecosystem), ADR-10 (External integrations / agentbox bridge),
           ADR-11 (Persistence — settings audit log lives in SQLite)

## Capability

VisionClaw is a single-deployment knowledge-graph visualiser that ships into
two operating modes:

- **Production** — facing real users on a domain, TLS-terminated, exposed to
  the open internet via a reverse proxy.
- **Development** — running on a developer laptop or inside the container
  fleet behind no proxy, with browser automation (`?skipAuth=true`) and
  test fixtures depending on a reliable bypass.

The capability this section delivers is a *single* auth posture that is
correct in both modes without flag-tuning at deploy time, has no path by
which production traffic can land on a code branch that trusts an
environment variable instead of a Nostr signature, and has a coherent CSP
that covers the actual subresources the application loads (WASM, Comlink
workers, font CDNs, polyhaven assets) without widening to `*` or
`'unsafe-eval'`.

The capability also delivers an end-to-end audit trail: every settings
mutation, graph mutation, ontology edit, and admin operation is logged with
pubkey + timestamp + request-id, queryable from a single SQLite table.

## Why this matters

Three sources of risk converged on the same hour of debugging during the
freeze investigation:

1. **`SETTINGS_AUTH_BYPASS=true` is honoured by the Rust auth extractor**
   (`src/settings/auth_extractor.rs:44`). The code has defensive checks
   (rejects bypass when `APP_ENV=production` or `RUST_ENV=production`),
   *but* the guard is a runtime check against a string environment variable.
   A misconfigured docker-compose file, a missing env var, or a typo
   (`APP_ENV=Production`) defeats it. The bypass produces a synthetic
   "dev-user" pubkey with `is_power_user: true`, granting full admin to any
   caller. This is one missing env-var away from a public-facing admin
   bypass on every endpoint that uses the `AuthenticatedUser` extractor.

2. **Endpoint auth is opt-in, applied per scope, easy to miss.**
   `RequireAuth` is wrapped on roughly five route scopes
   (`graph_export_handler`, `admin_sync_handler`, `api_handler::settings`,
   `api_handler::graph`, `api_handler::ontology`, `workspace_handler`).
   It is NOT wrapped on `bots_handler`, `clustering_handler`,
   `metrics_handler`, `consolidated_health_handler`, `physics_handler`,
   `inference_handler`, `image_gen_handler`, `ragflow_handler`,
   `briefing_handler`, `natural_language_query_handler`, `pages_handler`,
   `client_log_handler`, `schema_handler`, `semantic_handler`,
   `semantic_pathfinding_handler`, `layout_handler`, `validation_handler`,
   `constraints_handler`, `mcp_relay_handler`, `quic_transport_handler`,
   `ontology_agent_handler`, `memory_flash_handler`, plus
   `api_handler::analytics` which has `RequireAuth` *commented out* with
   the marker `// auth temporarily disabled for testing` at
   `src/handlers/api_handler/analytics/mod.rs:113`.

   That is 22+ handler modules without an enforced auth surface, and one
   with an explicitly disabled wrapper that should be presumed never to
   come back on without intervention.

3. **CSP is plausible but not principled.** The CSP in `client/index.html`
   (line 14) is:

   ```
   default-src 'self';
   script-src  'self' 'wasm-unsafe-eval';
   connect-src 'self' wss: ws: https://dl.polyhaven.org https://*.polyhaven.org;
   style-src   'self' 'unsafe-inline';
   img-src     'self' data: blob: https://dl.polyhaven.org;
   worker-src  'self' blob:;
   font-src    'self' data:;
   ```

   It permits `wss:` and `ws:` to *any* host (no scheme-restriction to
   `'self'`), permits `'unsafe-inline'` in style-src (defeats most CSP
   protection against injected style attacks), permits arbitrary `data:`
   in img-src and font-src, and the `client/dist/index.html` has the same
   meta-tag commented out — meaning the production build may not be
   enforcing the policy at all depending on which `index.html` is shipped.

These three risks compound. An attacker who can land arbitrary script in
the page (via inline-style-equivalent injection, via an unauthenticated
endpoint that returns user-controlled HTML, via an XSS in a markdown
preview) can issue same-origin requests to `bots_handler`, `physics_handler`,
`inference_handler`, et al, with no authentication required. If the
deployment additionally has `SETTINGS_AUTH_BYPASS=true` set (the default in
the dev compose) and the production guard fails to fire (because
`APP_ENV` is misspelled), the attacker also writes settings.

## Acceptance criteria

A1. **No production auth bypass.** The `SETTINGS_AUTH_BYPASS` codepath is
removed from `src/settings/auth_extractor.rs` and replaced with a *single*
compile-time gate: dev-bypass code only compiles under `#[cfg(debug_assertions)]`
or when `cargo` is invoked with the `--features dev-auth` flag. Release
builds physically do not contain the bypass branch. Misconfiguration
cannot re-enable it.

A2. **Single auth source-of-truth.** All HTTP handlers route through one
of three middleware wrappings: `RequireAuth::*`, `OptionalAuth`, or
explicit `Public`. Every route, without exception, declares which of
these applies. A handler that omits the declaration fails to compile.

A3. **Endpoint audit closed.** Each of the 22+ handlers listed above is
classified into one of: `needs-auth` (gets `RequireAuth`), `intentionally-public`
(gets `Public` marker + ADR justification), or `should-be-removed`
(dead endpoint, deleted in migration). Classification table in ADR-06.

A4. **WebSocket auth aligned.** WebSocket upgrade requires the same Nostr
token as REST in production. Dev-mode bypass uses the same single
compile-time flag as A1 — `--allow-skip-auth` is the runtime CLI flag,
gated by `cfg(any(debug_assertions, feature = "dev-auth"))`. There is no
runtime env-var path. Release binaries refuse the flag with `exit(1)`.

A5. **CSP tightened end-to-end.** The CSP delivered to the browser:
- Restricts `connect-src` to `'self'` plus an explicit allowlist of
  external origins (`https://dl.polyhaven.org`, `https://fonts.googleapis.com`,
  `https://fonts.gstatic.com`). No bare `ws:` / `wss:`.
- Removes `'unsafe-inline'` from `style-src`. Inline styles are replaced
  with nonce-based or refactored to external stylesheets.
- Keeps `'wasm-unsafe-eval'` in `script-src` (required by Rust→WASM).
- `worker-src 'self' blob:` retained (required by Comlink).
- `frame-ancestors 'none'` added (prevents clickjacking).
- CSP is delivered both via meta-tag (defence in depth) *and* by an
  Actix middleware setting the `Content-Security-Policy` response
  header on every HTML response. The commented-out meta-tag in
  `client/dist/index.html` is restored.

A6. **Audit log present.** Every state-mutating request that passes
`RequireAuth` writes a row to a `audit_log` table in SQLite with columns:
`id`, `ts`, `pubkey`, `method`, `path`, `status_code`, `request_id`,
`payload_hash`. Queryable via `/api/admin/audit?since=...&pubkey=...`
(behind `RequireAuth::admin()`). Retention: 90 days, then archived.

A7. **Docker socket scoped or removed.** The Docker socket mounted into
the container (per `project_config_gap_analysis.md`) is removed unless a
documented capability requires it. If it must remain, it is scoped via a
Docker socket proxy (`tecnativa/docker-socket-proxy` or equivalent) that
exposes only the capabilities used (typically `GET /containers/*`), with
no `POST` access. Defence in depth, not perimeter.

A8. **TLS termination documented.** Production deployment terminates TLS
at the reverse proxy. The Actix server binds only to `127.0.0.1` (or
container-internal IP), never directly to the internet. `X-Forwarded-Proto`
and `X-Forwarded-Host` are trusted only when the request originates from
the proxy's known IP range; an explicit ADR-section documents the trust
boundary.

A9. **`unwrap()` audit complete for handlers.** All `unwrap()` calls in
`src/handlers/` and `src/middleware/` (currently 139 across the broader
crate per memory note) are reviewed. Each is either:
- Justified as panic-impossible (with `// SAFETY:` comment), or
- Replaced with `?` returning a structured error, or
- Replaced with `.expect("compile-time invariant")` with comment.
Panic in a handler returns 500 to the caller (Actix catches it) but is
treated as a DoS-surface bug to be eliminated, not relied on.

A10. **Nostr signing model documented.** The NIP-98 signing flow used by
`authInterceptor.ts` is documented end-to-end: how the client constructs
the canonical request URL (`new URL(url, window.location.origin).href`,
line 39), the headers it injects (`Authorization: Nostr <token>`,
`X-Nostr-Pubkey`, `X-Request-ID`), how the server verifies (canonical
URL reconstruction from `X-Forwarded-*`, signature check), and the dev
fallback (`Bearer dev-session-token` — also gated by A1's compile flag).

## Non-goals

- **OAuth / OIDC support.** Nostr-only. Multiple identity providers add
  configuration surface and integration tests for negligible benefit in
  this user model.
- **Per-user rate limiting on read endpoints.** Existing IP-based rate
  limiter is sufficient. Power-user identity-based limits are deferred.
- **Field-level encryption of settings at rest.** SQLite database file
  permissions and TLS-at-rest disk encryption (deployment concern) are
  the layers we rely on.
- **mTLS between internal services.** All internal services run on the
  same Docker network. mTLS would be theatre.
- **Multi-tenant isolation.** Single-tenant deployment. If multi-tenancy
  is added later, the audit log's `pubkey` column becomes the tenant
  discriminator and policies attach to it.

## Out of scope (lives elsewhere)

- agentbox forum identity bridging — Section 10 (ADR-10).
- Settings UI auth flow (login modal, session lifecycle in the React app) —
  Section 5 (ADR-05) consumes the auth state exposed here.
- WebSocket upgrade auth specifics — ADR-02 D8 is normative; this section
  references but does not duplicate.

## User-facing flows

### Flow 1: First-time user, production

1. User opens `https://visionclaw.example.com/`.
2. React app loads. `nostrAuth.isAuthenticated()` returns `false`.
3. User clicks "Sign in with Nostr" → NIP-07 extension prompt
   (Alby, nos2x, Podkey).
4. Client signs a challenge from `POST /api/auth/nostr/challenge`.
5. Server validates signature, issues a short-lived (1h) session JWT
   bound to the pubkey.
6. Client stores `pubkey` + `session_token` in memory only (not
   localStorage — XSS protection). On reload, user re-signs.
7. Subsequent API calls use NIP-98 per-request signing via
   `authInterceptor.ts`. Each request is signed with the URL + method +
   body hash.

### Flow 2: Developer, local laptop

1. Developer runs `cargo run --features dev-auth` (or
   `./scripts/launch.sh up dev` which wraps it).
2. Developer opens `http://localhost:8080/?skipAuth=true`.
3. React app reads the query param, sets `nostrAuth.isDevMode() = true`,
   and emits `Authorization: Bearer dev-session-token` + `X-Nostr-Pubkey: dev-user`.
4. Server's `AuthenticatedUser` extractor — compiled with `dev-auth` —
   accepts the dev token, synthesises `is_power_user: true`.
5. Audit log records `pubkey = 'dev-user'` so dev actions are still
   traceable.

### Flow 3: Browser automation (Playwright, agent-browser)

Identical to Flow 2. Automation binaries always run against
`--features dev-auth` builds. Production CI never builds the automation
target against a release binary.

### Flow 4: Misconfigured production (the threat model)

Production binary is `cargo build --release` without `--features dev-auth`.
The dev-bypass code is *physically absent* from the binary. Any
combination of `SETTINGS_AUTH_BYPASS=true`, `DOCKER_ENV=1`,
`NODE_ENV=development`, `?skipAuth=true`, or `Authorization: Bearer
dev-session-token` is rejected at the type level — the codepath does not
exist to be reached.

## Bugs and smells brought forward from main

- `src/handlers/api_handler/analytics/mod.rs:113` —
  `// .wrap(RequireAuth::authenticated()) // auth temporarily disabled
  for testing`. Restored under the per-endpoint audit (A3).
- `src/settings/auth_extractor.rs:44–62` — runtime bypass replaced by
  compile-time gate (A1).
- `client/dist/index.html` line 16 — CSP meta-tag commented out.
  Re-enabled, kept in sync with `client/index.html` (A5).
- `client/index.html` line 14 — overly broad `connect-src wss: ws:`,
  `'unsafe-inline'` in style-src. Tightened (A5).
- Docker socket bind-mount in `docker-compose.yml` — scoped or removed
  (A7).
- 139 `unwrap()` calls — reviewed (A9). Handler-layer panics that
  could be triggered by malformed input become 4xx returns instead.

## Success metric

A `cargo build --release && docker run` of the production image, with
*every plausible misconfiguration of the auth env vars*, refuses to
serve any state-mutating request without a valid NIP-98 signature.
Tested via a CI job that brings up the release image, sets each of
`SETTINGS_AUTH_BYPASS=true`, `DOCKER_ENV=1`, `NODE_ENV=development`,
`APP_ENV=development`, then issues a `POST /api/settings` with no auth
header and expects `401`. Job fails the build if any combination returns
2xx.
