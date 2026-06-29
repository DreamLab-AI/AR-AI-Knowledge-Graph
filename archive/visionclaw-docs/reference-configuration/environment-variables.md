---
title: Environment Variables Reference
description: Complete reference for all VisionClaw environment variables
category: reference
difficulty-level: intermediate
tags:
  - configuration
  - environment
  - deployment
updated-date: 2026-04-10
---

# Environment Variables Reference

Complete reference for all VisionClaw environment variables.

---

## Core System Variables

### Application Settings

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ENVIRONMENT` | string | `development` | Environment mode: `development`, `staging`, `production` |
| `APP_ENV` | string | `development` | Application environment. Setting to `production` enables security hardening: blocks `ALLOW_INSECURE_DEFAULTS`, blocks `SETTINGS_AUTH_BYPASS`, enforces required secrets |
| `DEBUG_MODE` | boolean | `false` | Enable debug logging |
| `RUST_LOG` | string | `info` | Rust log level: `trace`, `debug`, `info`, `warn`, `error` |
| `HOST_PORT` | integer | `3001` | HTTP server port |
| `API_BASE_URL` | string | `http://localhost:9090` | API base URL |

**Example**:
```bash
ENVIRONMENT=production
DEBUG_MODE=false
RUST_LOG=warn
HOST_PORT=3030
```

### Network Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `MCP_TCP_PORT` | integer | `9500` | MCP protocol TCP port |
| `WS_PORT` | integer | `9090` | WebSocket server port |
| `METRICS_PORT` | integer | `9090` | Prometheus metrics port |
| `MAX_CONCURRENT_REQUESTS` | integer | `5000` | Maximum concurrent HTTP requests |
| `WS_CONNECTION_LIMIT` | integer | `1000` | Maximum WebSocket connections |

---

## Authentication & Security

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `JWT_SECRET` | string | *required* | JWT signing secret (256-bit recommended) |
| `AUTH_PROVIDER` | string | `nostr` | Authentication provider: `jwt`, `nostr`, `oauth` |
| `AUTH_REQUIRED` | boolean | `true` | Require authentication for API access |
| `SESSION_TIMEOUT` | integer | `86400` | Session timeout in seconds (24 hours) |
| `API_KEYS_ENABLED` | boolean | `true` | Enable API key authentication |
| `SETTINGS_AUTH_BYPASS` | boolean | `false` | Bypass authentication for settings endpoints (development only). **Blocked in production mode** (`APP_ENV=production`); removed from docker-compose.yml |
| `ALLOW_INSECURE_DEFAULTS` | boolean | `false` | Allow insecure default secrets (development only). **Blocked when `APP_ENV=production`** — server refuses to start if any secret retains its insecure default value |

**Example**:
```bash
JWT_SECRET=$(openssl rand -hex 32)
AUTH_PROVIDER=nostr
AUTH_REQUIRED=true
SESSION_TIMEOUT=86400
```

---

## Database Configuration

### PostgreSQL

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `POSTGRES_HOST` | string | `postgres` | PostgreSQL host |
| `POSTGRES_PORT` | integer | `5432` | PostgreSQL port |
| `POSTGRES_DB` | string | `visionclaw` | Database name |
| `POSTGRES_USER` | string | `visionclaw` | Database user |
| `POSTGRES_PASSWORD` | string | *required* | Database password |
| `POSTGRES_MAX_CONNECTIONS` | integer | `100` | Maximum connection pool size |
| `POSTGRES_SSL_MODE` | string | `prefer` | SSL mode: `disable`, `prefer`, `require` |

### Graph Store (Oxigraph — embedded)

The graph store is an embedded Oxigraph RDF triple store (ADR-11). It runs in-process
with no separate container, no network endpoint, and no password — the former `NEO4J_*`
variables (`NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD`, `NEO4J_ENABLED`) are removed and
no longer read by any code. The dataset is persisted under the data directory.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `DATA_DIR` | string | `./data` | Root data directory (Docker images set this to `/app/data`). The Oxigraph RocksDB dataset is stored at `${DATA_DIR}/oxigraph/`. Reset the graph store by stopping the server and `rm -rf ${DATA_DIR}/oxigraph` |

### Redis Cache

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `REDIS_HOST` | string | `redis` | Redis host |
| `REDIS_PORT` | integer | `6379` | Redis port |
| `REDIS_PASSWORD` | string | `""` | Redis password (empty = no auth) |
| `REDIS_MAX_MEMORY` | string | `512mb` | Maximum Redis memory |
| `REDIS_MAX_CONNECTIONS` | integer | `100` | Maximum connection pool size |

---

## Resource Limits

### Memory Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `MEMORY_LIMIT` | string | `16g` | Container memory limit |
| `SHARED_MEMORY_SIZE` | string | `2g` | Shared memory size |
| `POSTGRES_SHARED_BUFFERS` | string | `1GB` | PostgreSQL shared buffers |
| `HEAP_SIZE` | string | `8g` | JVM heap size (if applicable) |

### CPU Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `CPU_LIMIT` | float | `8.0` | Maximum CPU cores |
| `CPU_RESERVATION` | float | `4.0` | Reserved CPU cores |
| `WORKER_THREADS` | integer | `8` | Worker thread count |
| `MAX_AGENTS` | integer | `20` | Maximum concurrent agents |
| `MAX_CONCURRENT_TASKS` | integer | `20` | Maximum concurrent tasks for TaskOrchestratorActor. Controls how many tasks can execute simultaneously across all agents |

### Audit Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `AUDIT_LOG_PATH` | string | `/app/logs/audit.log` | File path for the persistent audit log. Records authentication events, authorization decisions, and administrative actions |

---

## GPU Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ENABLE_GPU` | boolean | `false` | Enable GPU acceleration |
| `NVIDIA_VISIBLE_DEVICES` | string | `0` | GPU device IDs (comma-separated) |
| `CUDA_ARCH` | integer | `89` | CUDA architecture (86=RTX 30xx, 89=RTX 40xx) |
| `GPU_MEMORY_LIMIT` | string | `8g` | GPU memory limit |
| `NVIDIA_DRIVER_CAPABILITIES` | string | `compute,utility` | Driver capabilities |

**Example**:
```bash
ENABLE_GPU=true
NVIDIA_VISIBLE_DEVICES=0,1
CUDA_ARCH=89
GPU_MEMORY_LIMIT=16g
```

---

## Feature Flags

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ENABLE_XR` | boolean | `false` | Enable XR/VR features |
| `ENABLE_VOICE` | boolean | `false` | Enable voice interaction |
| `ENABLE_NEURAL_ENHANCEMENT` | boolean | `false` | Enable neural acceleration |
| `ENABLE_WASM_ACCELERATION` | boolean | `false` | Enable WASM acceleration |
| `ENABLE_GITHUB_SYNC` | boolean | `true` | Enable GitHub synchronization |
| `ENABLE_METRICS` | boolean | `true` | Enable Prometheus metrics |
| `ENABLE_SOLID` | boolean | `false` | Enable Solid/LDP integration |

---

## AI Service Configuration

### OpenAI

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `OPENAI_API_KEY` | string | `""` | OpenAI API key |
| `DEFAULT_LLM_MODEL` | string | `gpt-4o` | Default model |
| `LLM_TEMPERATURE` | float | `0.7` | Model temperature |
| `LLM_MAX_TOKENS` | integer | `4096` | Maximum tokens per request |
| `LLM_TIMEOUT` | integer | `30` | Request timeout in seconds |

### Anthropic Claude

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ANTHROPIC_API_KEY` | string | `""` | Anthropic API key |
| `CLAUDE_MODEL` | string | `claude-3-5-sonnet-20241022` | Claude model version |
| `CLAUDE_MAX_TOKENS` | integer | `4096` | Maximum tokens |

### Perplexity

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `PERPLEXITY_API_KEY` | string | `""` | Perplexity API key |
| `PERPLEXITY_MODEL` | string | `llama-3.1-sonar-small-128k-online` | Model version |

---

## Solid Integration (JSS Sidecar)

### JSS Server Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `JSS_ENABLED` | boolean | `false` | Enable Solid/JSS integration |
| `JSS_HOST` | string | `jss` | JSS container hostname |
| `JSS_PORT` | integer | `3000` | JSS HTTP port |
| `JSS_BASE_URL` | string | `http://localhost:3000` | Public JSS base URL |
| `JSS_ROOT_PATH` | string | `/data/pods` | Root path for pod storage |
| `JSS_LOG_LEVEL` | string | `info` | JSS log level |

### Solid Authentication

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SOLID_OIDC_ISSUER` | string | `""` | OIDC issuer URL for Solid auth |
| `SOLID_ALLOW_NOSTR_AUTH` | boolean | `true` | Allow NIP-98 authentication |
| `SOLID_DPOP_ENABLED` | boolean | `true` | Enable DPoP token binding |
| `SOLID_SESSION_TIMEOUT` | integer | `3600` | Session timeout in seconds |

---

## Logging & Monitoring

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `LOG_LEVEL` | string | `info` | Logging level |
| `LOG_FORMAT` | string | `json` | Log format: `json`, `plain` |
| `LOG_FILE` | string | `/app/logs/visionclaw.log` | Log file path |
| `LOG_ROTATION` | string | `daily` | Rotation policy |
| `LOG_MAX_SIZE` | string | `100MB` | Maximum log file size |
| `TRACK_PERFORMANCE` | boolean | `true` | Enable performance tracking |

---

## Related Documentation

- [Docker Compose Options](./docker-compose-options.md)
- [Configuration Reference](./README.md)
- [Deployment Guide](../../how-to/deployment-guide.md)
