# dreamlab-ai-website — Operational Runbook

| Field | Value |
|-------|-------|
| Substrate | dreamlab-ai-website |
| Repo | github.com/DreamLab-AI/dreamlab-ai-website |
| Runtime | GitHub Pages (React SPA + Trunk WASM) + Cloudflare Workers (APIs) |
| Domain | dreamlab-ai.com (CNAME → GitHub Pages) |
| Status | Kit-consumer deployment active; `forum-config/` pins nostr-rust-forum rc11 |
| Verified (2026-05-09) | Main site 200 (9.4KB), /community/ 200 (5.1KB), WASM binary 3.4MB loads OK |

### Live URL Map

| Path | Served by | Content |
|------|-----------|---------|
| `/` | GitHub Pages | React SPA (marketing, workshops, research) |
| `/community/` | GitHub Pages + Trunk WASM | Leptos forum client (nostr-bbs-forum-client) |
| `wss://dreamlab-nostr-relay.*.workers.dev` | CF Workers | Nostr relay (NIP-01, 16 NIPs) |
| `https://dreamlab-auth-api.*.workers.dev` | CF Workers | NIP-98 auth + WebAuthn |
| `https://dreamlab-pod-api.*.workers.dev` | CF Workers | Solid Pod bridge |
| `https://dreamlab-search-api.*.workers.dev` | CF Workers | Vector search (all-MiniLM-L6-v2) |
| `https://dreamlab-link-preview.*.workers.dev` | CF Workers | OpenGraph link preview |

### Runtime Env Injection

The deploy workflow (`.github/workflows/deploy.yml`) injects `window.__ENV__` into the community/index.html with all CF Workers URLs. The `.env` file is for local development only and does NOT affect production.

### Deprecated GCR References

The `.env` file previously pointed to Google Cloud Run services which are now dead (503/500). Updated 2026-05-09 to point to CF Workers URLs for local development consistency.

## Architecture

DreamLab's branded deployment of the nostr-bbs-rs forum kit:
- React marketing site (Vite build, GitHub Pages)
- Leptos WASM forum client at /community/ (Trunk build, GitHub Pages)
- 5 CF Workers (relay, auth, pod, search, link-preview) — all verified healthy
- `forum-config/` package provides operator config (dreamlab.toml per ADR-085)
- `forum-config/deploy/` contains per-worker wrangler manifests

## Deployment

```bash
# forum-config/ provides branded config; nostr-rust-forum kit crates provide runtime.
cargo check --manifest-path forum-config/Cargo.toml
npx wrangler deploy --config forum-config/deploy/<worker>.wrangler.toml --env production
```

## Health Checks

| Endpoint | Expected |
|----------|----------|
| GET / | 200 (landing page) |
| GET /community/health | 200 `{"status":"ok"}` |
| GET /.well-known/solid | 200 JSON-LD |

## Common Failure Modes

### Kit Version Mismatch
- **Symptom**: Build failure or runtime panic after kit update
- **Cause**: `forum-config/` references kit API that changed
- **Fix**: Check kit changelog, update `forum-config/` adapters. Run `cargo check --target wasm32-unknown-unknown`

### Cloudflare Pages Deploy Failure
- **Symptom**: Static content shows old version
- **Cause**: Build script error or cache issue
- **Fix**: Check CF Pages dashboard build logs; trigger manual redeploy

### Community Forum 500 Errors
- **Symptom**: Forum pages return 500
- **Cause**: Same as forum-runbook.md issues (D1, KV, DO)
- **Fix**: Follow forum-runbook.md troubleshooting

## Backup / Restore

Same as forum-runbook.md (inherits Cloudflare D1/KV/DO infrastructure).

Additional:
- **Static content**: Git-tracked, no separate backup needed
- **forum-config/**: Git-tracked configuration package
- **DNS/routing**: Cloudflare dashboard; document in separate infra runbook

## RTO / RPO Targets

| Component | RTO | RPO | Notes |
|-----------|-----|-----|-------|
| Static site | < 1 min | N/A | CF Pages CDN, auto-redeploy |
| Forum workers | < 1 min | N/A | Same as forum-runbook.md |
| D1 databases | < 15 min | < 24h | CF automatic backup |
| Custom config | N/A | N/A | Git-tracked |

## Kit Consumer State

The website has transitioned from the historical `community-forum-rs/` monolith to the kit consumer model:

1. `forum-config/` pins `nostr-rust-forum` kit crates and carries DreamLab branding/config.
2. `forum-config/deploy/` carries the per-worker Wrangler manifests and Cloudflare resource IDs.
3. Historical `community-forum-rs` references in older migration docs are retained for audit context, not as the current source layout.

## CI Pipeline

8-job pipeline validates:
- Rust build (wasm32 target)
- Clippy lints
- Format check
- Unit tests
- Integration tests
- Security audit
- Forum config validation
- Deploy preview (on PR)

## Monitoring

- Cloudflare Analytics dashboard
- `npx wrangler tail` per worker
- CI pipeline status badges in README

## Escalation

1. Check Cloudflare dashboard for platform issues
2. Follow forum-runbook.md for worker-level issues
3. Check `forum-config/` compatibility with kit version
4. File GitHub issue with logs and version info
