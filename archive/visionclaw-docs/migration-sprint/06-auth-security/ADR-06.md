# ADR-06 — Auth & Security

Status      : Proposed
Date        : 2026-05-16
Supersedes  : (none — first formal ADR on this surface)
Related     : ADR-02 D8 (WebSocket auth), ADR-05 (Settings consumes auth),
              ADR-10 (agentbox bridge), ADR-11 (audit log → SQLite)

## Context

The auth surface at `main@HEAD` carries three structural defects:

1. **Runtime-gated bypass.** `SETTINGS_AUTH_BYPASS=true` is honoured by
   the production binary (`src/settings/auth_extractor.rs:44`). It is
   refused only when `APP_ENV=production` or `RUST_ENV=production` is
   also set — one misconfigured docker-compose away from a public
   admin path.
2. **Opt-in per-route auth.** `RequireAuth` wraps 5–6 route scopes. The
   remaining 22+ handler modules expose endpoints with no explicit auth
   declaration; one has the wrap commented out as "temporarily disabled
   for testing".
3. **CSP that refuses little.** `connect-src wss: ws:` permits any
   WebSocket destination; `style-src 'unsafe-inline'` defeats XSS
   mitigations; `client/dist/index.html` has the meta-tag commented out.

The sprint is the right moment to fix this surface because the auth
code is touched by enough of the other refactors that a clean baseline
saves re-doing the work later under deadline.

## Decision

### D1. Compile-time dev-auth gate, no runtime bypass

The Rust auth extractor's dev-bypass code is moved behind a Cargo
feature:

```rust
// src/settings/auth_extractor.rs
#[cfg(any(debug_assertions, feature = "dev-auth"))]
fn try_dev_bypass(req: &HttpRequest) -> Option<AuthenticatedUser> {
    // ... existing bypass logic, simplified — no env-var checks ...
    if req.headers().get("Authorization")?.to_str().ok()? == "Bearer dev-session-token" {
        Some(AuthenticatedUser { pubkey: "dev-user".into(), is_power_user: true })
    } else {
        None
    }
}

#[cfg(not(any(debug_assertions, feature = "dev-auth")))]
fn try_dev_bypass(_req: &HttpRequest) -> Option<AuthenticatedUser> {
    None
}
```

The `SETTINGS_AUTH_BYPASS`, `DOCKER_ENV`, `NODE_ENV` environment
variables are no longer read anywhere in auth code. The production
release build does not contain the bypass branch — the compiler emits
no code for it. There is no env-var path to re-enable it.

The `dev-auth` feature is declared in `Cargo.toml`:

```toml
[features]
default = []
dev-auth = []
```

`scripts/launch.sh up dev` sets `--features dev-auth` on the cargo
invocation. `scripts/launch.sh up prod` (or the production Dockerfile)
does not.

### D2. `--allow-skip-auth` CLI flag, compile-gated

Per ADR-02 D8, the WebSocket upgrade path accepts unauthenticated
connections when the server is launched with `--allow-skip-auth`. The
flag is parsed by `src/main.rs`:

```rust
let allow_skip_auth = std::env::args().any(|a| a == "--allow-skip-auth");

#[cfg(not(any(debug_assertions, feature = "dev-auth")))]
if allow_skip_auth {
    eprintln!("--allow-skip-auth is not available in release builds");
    std::process::exit(1);
}
```

The release binary literally cannot run with `--allow-skip-auth`. This
mirrors D1's discipline: misconfiguration cannot defeat it.

### D3. Three-state route declaration, default-deny

All HTTP routes declare their auth posture explicitly via a wrapper
chosen from exactly three:

- `RequireAuth::authenticated()` — any signed Nostr request.
- `RequireAuth::power_user()` / `::admin()` — elevated.
- `Public` — explicitly unauthenticated, justified in this ADR.

There is no default path. Routes that omit the declaration are caught
by a `compile_routes_have_auth!` macro that wraps the `App::new()`
builder and refuses to compile if any route is unwrapped. (Concrete
implementation: a build-script lint that scans `src/handlers/**/*.rs`
for `.route(` and `.service(` calls inside a `pub fn config*` and
checks for a sibling `.wrap(RequireAuth` or `.wrap(Public::marker())`.
A non-blocking warning at first; promoted to hard error in Phase 7.)

### D4. Endpoint audit table

Every handler classified. Each row is one route scope, not one route.

| Handler module                              | Posture          | Rationale |
|---------------------------------------------|------------------|-----------|
| `api_handler::settings`                     | RequireAuth      | Existing. Settings writes. |
| `api_handler::graph`                        | RequireAuth      | Existing. Graph writes. |
| `api_handler::ontology`                     | RequireAuth      | Existing. Ontology writes. |
| `api_handler::analytics`                    | RequireAuth      | Restored from "temporarily disabled" comment. |
| `graph_export_handler`                      | RequireAuth      | Existing. Exports user data. |
| `admin_sync_handler`                        | RequireAuth::power_user | Existing. Admin op. |
| `workspace_handler`                         | RequireAuth      | Existing. |
| `bots_handler`                              | RequireAuth::power_user | Bot spawn / control. Was missing. |
| `bots_visualization_handler`                | OptionalAuth     | Read-only telemetry view. Public OK in dev. |
| `clustering_handler`                        | RequireAuth      | Computes on the graph; expensive. |
| `constraints_handler`                       | RequireAuth      | Mutates physics constraints. |
| `image_gen_handler`                         | RequireAuth      | Bills external API. Pubkey-attributed. |
| `inference_handler`                         | RequireAuth      | Bills LLM. Pubkey-attributed. |
| `briefing_handler`                          | RequireAuth      | Publishes Nostr events on behalf of user. |
| `natural_language_query_handler`            | RequireAuth      | Bills LLM. |
| `pages_handler`                             | OptionalAuth     | Reads Logseq pages; public read in dev. |
| `physics_handler`                           | RequireAuth      | Mutates simulation parameters. |
| `layout_handler`                            | RequireAuth      | Switches active layout engine globally. |
| `semantic_handler`                          | RequireAuth      | Compute-intensive. |
| `semantic_pathfinding_handler`              | OptionalAuth     | Read-only path query; reasonable to allow anon read. |
| `schema_handler`                            | OptionalAuth     | Read-only schema; public read. |
| `validation_handler`                        | RequireAuth      | Mutates validation rules. |
| `mcp_relay_handler`                         | RequireAuth::power_user | Proxies to MCP servers; tool-execution surface. |
| `multi_mcp_websocket_handler`               | RequireAuth::power_user | Same. |
| `quic_transport_handler`                    | RequireAuth      | Transport-layer; defence in depth. |
| `ontology_agent_handler`                    | RequireAuth      | Agent operations on ontology. |
| `memory_flash_handler`                      | RequireAuth      | Writes to memory store. |
| `client_log_handler`                        | OptionalAuth     | Accepts client logs; rate-limited; pubkey-tagged when present. |
| `metrics_handler`                           | Public           | Prometheus-style metrics. No PII. Scrape endpoint. |
| `consolidated_health_handler`               | Public           | `/health` for orchestrator probes. No data. |
| `nostr_handler`                             | Public (for /challenge), RequireAuth (for /session) | Auth bootstrap. |
| `ragflow_handler`                           | RequireAuth      | Bills external RAG service. |
| `speech_socket_handler`                     | RequireAuth      | Mic stream; PII-bearing. |
| `socket_flow_handler` (binary positions)    | Per ADR-02 D8    | WebSocket auth via query token. |
| `fastwebsockets_handler`                    | Per ADR-02 D8    | Same. |
| `solid_proxy_handler`                       | Removed          | Solid-pod proxy. No production user. Dead code. |
| `settings_handler.rs.temp`                  | Removed          | Stale file. |
| `settings_validation_fix.rs`                | Reviewed         | Helper; folded into validation_handler. |

The audit explicitly *removes* `solid_proxy_handler` and the `.temp`
file. Dead code with auth surface is worse than dead code without —
delete it.

### D5. CSP, end-to-end

Single CSP, delivered three ways for defence in depth:

1. **`client/index.html` meta-tag** (build output):
   ```
   default-src 'self';
   script-src  'self' 'wasm-unsafe-eval';
   connect-src 'self' wss://visionclaw.example.com https://dl.polyhaven.org https://*.polyhaven.org;
   style-src   'self' 'nonce-{{NONCE}}';
   img-src     'self' data: blob: https://dl.polyhaven.org;
   worker-src  'self' blob:;
   font-src    'self' https://fonts.gstatic.com data:;
   frame-ancestors 'none';
   base-uri    'self';
   form-action 'self';
   ```

2. **Actix middleware** sets the `Content-Security-Policy` HTTP header
   on every HTML response with the same value. The nonce is per-response,
   generated in the middleware, and injected into the rendered HTML.

3. **`client/dist/index.html`** — the commented-out meta-tag is restored
   and kept in lockstep with `client/index.html` via a build-script check.

Notes:
- `wss:` and `ws:` bare schemes removed. WebSocket endpoints are
  enumerated in §D12. All endpoints are same-origin
  (`wss://<host>/wss`, `wss://<host>/ws/...`, `wss://<host>/api/<section>/ws`)
  and fall under `'self'`. External `wss:` is not used.
- `'unsafe-inline'` removed from style-src. Replaced with nonce-based
  styles. The build emits a unique nonce per response and injects it
  into the HTML and the CSP header simultaneously.
- `'unsafe-eval'` is *not* added. `'wasm-unsafe-eval'` is sufficient
  for the Rust→WASM modules (scene-effects, ontology).
- `frame-ancestors 'none'` blocks clickjacking. There is no legitimate
  iframe embedding use case for VisionClaw.
- The commit `7c5a4abd4` (referenced in the task) adding `blob:` to
  `connect-src` for Comlink/WASM is *rejected*. Comlink uses
  `worker-src`, not `connect-src`. WASM modules use `script-src
  'wasm-unsafe-eval'`. The `blob:` in connect-src does not address any
  real Comlink/WASM need and widens the policy unnecessarily. If a
  Comlink message is observably blocked at runtime under the tightened
  policy, the fix is to route through same-origin URLs, not to widen
  CSP.

### D6. Audit log

A new SQLite table (per ADR-11) is created:

```sql
CREATE TABLE audit_log (
    id            INTEGER PRIMARY KEY,
    ts            INTEGER NOT NULL,  -- unix ms
    pubkey        TEXT    NOT NULL,
    method        TEXT    NOT NULL,
    path          TEXT    NOT NULL,
    status_code   INTEGER NOT NULL,
    request_id    TEXT    NOT NULL,
    payload_hash  TEXT,              -- sha256 of request body, nullable
    duration_ms   INTEGER NOT NULL,
    client_ip     TEXT                -- from X-Forwarded-For, last hop
);

CREATE INDEX idx_audit_ts     ON audit_log(ts);
CREATE INDEX idx_audit_pubkey ON audit_log(pubkey, ts);
```

A new middleware `AuditLog` is wrapped after `RequireAuth` on every
state-mutating route (POST, PUT, PATCH, DELETE). Read endpoints are
not logged — too high-volume, not the security question we're trying
to answer. The middleware writes asynchronously via a bounded channel
(`tokio::sync::mpsc`, capacity 1024). If the channel is full, the
event is dropped and a counter increments. The audit log is
best-effort observable, not transactional with the request.

Retention: 90 days online, then move to `audit_log_archive_yyyymm`
tables. Archive tables are read-only; never written to after creation.

Read endpoint: `GET /api/admin/audit?since=&pubkey=&path=` wrapped with
`RequireAuth::admin()`.

### D7. Docker socket: socket-proxy or remove

Current docker-compose mounts `/var/run/docker.sock` into containers
that enumerate sibling containers — root-equivalent capability on the
host. Two acceptable resolutions: remove the mount entirely if the
only consumer is a feature we drop (verify in Section 9), or replace
with `tecnativa/docker-socket-proxy` (`CONTAINERS=1 POST=0 DELETE=0`)
exposed on a sidecar. Section 9 owns the final decision; this ADR
records the constraint that a bare socket mount is rejected from the
release image.

### D8. Trusted proxy boundary

The auth extractor reads `X-Forwarded-Proto` / `X-Forwarded-Host` for
NIP-98 URL reconstruction (auth_extractor.rs:94–103). Actix is
configured to trust forwarded headers *only* from the configured CIDR
list (env `VISIONCLAW_TRUSTED_PROXIES`, default `127.0.0.1/32`).
Requests from any other source have forwarded headers stripped.
Without this, an attacker can spoof `X-Forwarded-Proto: https` to
manipulate the signed URL. The env var is configuration, not a
security toggle — misconfiguring it tightens, not loosens.

### D9. `unwrap()` audit, security-relevant subset

The 139 `unwrap()` calls flagged in `project_config_gap_analysis.md`
are addressed in three layers:

- **Section 1 (ADR-01 D4)** addresses GPU-actor `unwrap()` via supervisor
  restart.
- **Section 8** addresses ontology-actor `unwrap()` via the same
  restart pattern.
- **This ADR (D9)** addresses *handler-layer* `unwrap()`. Each is one
  of:
  - On parsing user-controlled input: replace with `?` returning
    `BadRequest` 400.
  - On reading config that's known-present at startup: replace with
    `.expect("loaded at boot, invariant")` — a panic here is a boot-time
    bug, not a runtime DoS.
  - On atomic ordering / lock acquisition that genuinely cannot fail:
    `// SAFETY: <reason>` comment with retained `.unwrap()`.

A clippy lint (`clippy::unwrap_used` set to `warn` in
`src/handlers/`) enforces ongoing discipline.

### D10. Nostr signing canonicalisation

The NIP-98 URL canonicalisation has ambiguities (trailing slashes,
query parameter ordering, percent-encoding) between the client's
`new URL(url, window.location.origin).href` and the server's
`format!("{}://{}{}", scheme, host, path_and_query)`. The canonical
form for signing: scheme as forwarded; host lowercase with default
port elided; path as-is; query parameter order preserved; fragment
stripped. Client and server share a `canonicalise_url` function
spec (Rust in `src/services/nostr_service.rs`, TypeScript in
`client/src/services/nostrAuthService.ts`); unit tests cross-validate
on a fixed corpus.

### D11. Startup refusal of dev-mode env vars in release

The release binary, in `main.rs` after `dotenv().ok()` and before binding
any socket, refuses to start if any of `SETTINGS_AUTH_BYPASS`,
`VISIONCLAW_DEV_MODE`, `ALLOW_INSECURE_DEFAULTS`, or `NODE_ENV=development`
with `DOCKER_ENV` set are present. Logs each offending var to stderr,
exits with status 2. Wrapped in
`#[cfg(not(any(debug_assertions, feature = "dev-auth")))]` so dev builds
skip it. The release binary cannot *honour* these vars (no code reads
them) but their presence is signal of an ops promotion that brought dev
settings forward.

### D12. WebSocket endpoint enumeration

The WebSocket surface is enumerated below. Every endpoint enforces the
auth model from ADR-02 D8 and rejects anonymous upgrades outside
compile-time-gated dev builds. New endpoints add a row here in the same
PR that introduces them.

| Path | Direction | Auth | Owning section | Protocol version | Purpose |
|------|-----------|------|----------------|------------------|---------|
| `/wss` | bidir | RequireAuth | Section 2 | V3 (`magic=0xV3F0`) | Binary position broadcast (PRD-04 + PRD-12 consumers). Settlement-gated cadence. |
| `/ws/speech` | bidir | RequireAuth | Section 9 | JSON | Mic-in (Whisper STT) + agent TTS (Kokoro). PTT-gated. |
| `/ws/client-messages` | server→client | RequireAuth | Section 3 | JSON | `filter_update_success`, `initialGraphLoad`, `memory_flash`, settings sync. |
| `/ws/mcp-relay` | bidir | RequireAuth::power_user | Section 7 | JSON-RPC | MCP tool-call relay. **REMOVE Phase 7** — re-homed in agentbox. |
| `/ws/xr-presence` | client→server | RequireAuth | Section 12 | `visionclaw-xr-presence` v1 | XR head/hand/gaze pose 30Hz. v1 sink-only; v2 adds relay. |
| `/ws/agent-telemetry` | server→client | RequireAuth | Section 10 | `AgentTelemetryEnvelope` v1 (ADR-10 D1) | Agent state from agentbox. **NEW** — replaces baseline `/api/visualization/agents/ws`. |
| `/ws/enterprise-events` | server→client | RequireAuth | Section 10 | `EnterpriseEventEnvelope` v1 (ADR-10 D5) | Forum events: membership / role / session_revoked. |
| `/api/multi-mcp/ws` | bidir | RequireAuth::power_user | Section 7 | JSON | Multi-MCP discovery. **REMOVE Phase 7** — agentbox. |
| `/api/analytics/ws` | server→client | RequireAuth | Section 1 | JSON | PageRank progress, clustering ticks. |
| `/api/ontology/ws` | bidir | RequireAuth | Section 8 | JSON | Reasoning progress + validation events. |
| `/api/visualization/agents/ws` | server→client | OptionalAuth | Section 7 | JSON | **DEPRECATED Phase 7a** → `410 Gone` with `Link` to `/ws/agent-telemetry`. **REMOVE Phase 7b.** |

Cross-section ownership:
- §D12 owns the URL space and the auth posture per endpoint.
- Each owning section defines the wire format for its endpoint.
- Default backpressure is drop-never-queue (ADR-02 D3); deviations
  documented in the owning ADR.

A CI route-drift check (`scripts/ci/check-ws-route-enumeration.sh`)
parses `App::new()` route registrations in `src/main.rs` and asserts
every registered `.route("/ws...")` and `.route("/wss")` appears as a
row in this table. PRs that add a WebSocket endpoint without an
accompanying table row fail CI.

## Options considered

### O1. Keep runtime env-var bypass, add more env-var guards

Rejected. Defence-in-depth on a security toggle is still depth on the
*same* failure mode (env var mistyped, env var dropped). Each
additional guard adds confusion ("which combination is safe?") without
adding security. The compile-time gate is one decision, made at the
right time, by the right person.

### O2. Move auth into a sidecar (oauth2-proxy, Pomerium)

Rejected for this deployment. Single-tenant, single-host,
single-process. The sidecar is the kind of complexity that pays for
itself at 10+ services, not 1.

### O3. Three-state declaration + compile-time gate (this ADR)

Adopted.

## Risks

- **R1**: The compile-time-gated dev auth requires building two
  binaries (debug and release). The launch script must select the
  right one for each environment. Mitigation: the
  `scripts/launch.sh` flag-parsing fix (commit `28c3521bb`) is the
  vehicle; verify in Section 9.
- **R2**: Tightening CSP may break a third-party component
  (Polyhaven asset loader, font CDN). Mitigation: Phase the tightening:
  ship the tightened CSP in *Report-Only* mode for one week, gather
  CSP violation reports via `report-uri`, then enforce.
- **R3**: Audit log write contention under load. Mitigation: bounded
  channel + drop-on-full + counter metric. The audit is best-effort,
  not authoritative — the source of truth for "what was the request"
  is the request itself; the audit log is for retrospective
  investigation.
- **R4**: The build-script lint for D3 ("every route has an auth
  declaration") is a custom check. Mitigation: write it as a `cargo
  test` (a unit test that parses the source files) rather than a
  custom subcommand, so it runs in `cargo test` and CI without setup.
- **R5**: Trusted-proxy CIDR misconfiguration could strip legitimate
  headers in production. Mitigation: log every header-strip event at
  `warn!`, surface a metric, and document the env var prominently.

## Rejected from main as buggy / unjustified

- `src/settings/auth_extractor.rs:44–62` runtime bypass — replaced by
  D1's compile-time gate.
- `src/handlers/api_handler/analytics/mod.rs:113` commented-out
  `RequireAuth` — restored under D4.
- `client/dist/index.html` commented-out CSP meta-tag — restored
  under D5.
- Commit `7c5a4abd4` adding `blob:` to connect-src — rejected (D5
  rationale).
- `src/handlers/solid_proxy_handler.rs` — removed (D4).
- `src/handlers/settings_handler.rs.temp` — removed (D4).

## Bugs and smells at the reset point (41979d33e)

- The baseline already has `SETTINGS_AUTH_BYPASS` honoured at runtime
  (the bypass predates this sprint). Migration must not preserve it.
- Baseline NIP-98 verification may not handle `X-Forwarded-Proto`
  correctly (verify against `nostr_service.rs` during implementation).
  If absent at baseline, D8's trusted-proxy boundary work brings it
  in.
- Baseline CSP is the unedited version from when the React app was
  scaffolded — `connect-src 'self'` only. Migration tightens *and*
  extends (adds explicit Polyhaven, fonts allowlists).
- Baseline has fewer handlers than `main` — the audit table in D4
  applies only to handlers that exist at the migration target. New
  handlers added during migration (e.g. the audit-log read endpoint)
  inherit the same three-state discipline.

## Phasing

This ADR's work falls in the README's Phase 7 ("Settings, Auth, Bots,
Ecosystem, External, XR in parallel") but two items block earlier
work:

- D1 + D2 (compile-time gates) must land before Phase 3 (binary
  protocol) because ADR-02 D8 depends on the `--allow-skip-auth` flag
  existing.
- D5 (CSP) and D6 (audit log) can land in Phase 7 without blocking
  anything else.

Implementation order within Phase 7:
1. D1 + D2 (compile-time gate, CLI flag).
2. D3 + D4 (route audit, three-state declarations).
3. D8 (trusted-proxy boundary).
4. D5 (CSP, Report-Only week, then enforce).
5. D6 (audit log).
6. D7 (docker socket — coordinate with Section 9).
7. D9 (`unwrap()` audit).
8. D10 (Nostr canonicalisation cross-validation).
