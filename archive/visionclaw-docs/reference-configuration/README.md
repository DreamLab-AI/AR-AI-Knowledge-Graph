---
title: Configuration Reference
description: Complete reference for all VisionClaw configuration options
category: reference
difficulty-level: intermediate
tags:
  - configuration
  - deployment
updated-date: 2025-01-29
---

# Configuration Reference

Complete reference for all VisionClaw configuration options across environment variables, YAML files, and runtime settings.

---

## Configuration Files

| File | Location | Purpose |
|------|----------|---------|
| `.env` | Project root | Environment variables |
| `data/settings.yaml` | Data directory | Application settings |
| `docker-compose.yml` | Project root | Container configuration |
| `docker-compose.solid.yml` | Project root | Solid sidecar configuration |
| `config/database.yaml` | Config directory | Database settings |
| `config/security.yaml` | Config directory | Security policies |
| `config/solid.yaml` | Config directory | Solid/JSS settings |

---

## Configuration Precedence

**Priority Order** (highest to lowest):
1. Runtime API calls (`POST /api/config`)
2. Environment variables (`.env`)
3. YAML configuration files (`data/settings.yaml`)
4. Default values (hardcoded)

---

## Documentation Index

| Topic | File | Description |
|-------|------|-------------|
| **Environment Variables** | [environment-variables.md](./environment-variables.md) | All env vars with types and defaults |
| **Docker Compose** | [docker-compose-options.md](./docker-compose-options.md) | Container configuration options |

---

## Quick Reference

### Essential Environment Variables

```bash
# Core
ENVIRONMENT=production
DEBUG_MODE=false
HOST_PORT=3001

# Authentication (required)
JWT_SECRET=$(openssl rand -hex 32)

# Database (required)
POSTGRES_PASSWORD=$(openssl rand -hex 24)

# Features
ENABLE_GPU=true
ENABLE_METRICS=true
```

### Essential YAML Settings

```yaml
# data/settings.yaml
system:
  agent-management:
    max-concurrent-agents: 50

visualization:
  graphs:
    default:
      physics:
        enabled: true
        enable-gpu: true

auth:
  enabled: true
  provider: nostr
```

---

## Runtime Configuration

### API Configuration Endpoint

Update configuration at runtime:

```http
POST /api/config/update
Content-Type: application/json
Authorization: Bearer {token}

{
  "physics": {
    "enabled": true,
    "gpuAcceleration": true
  },
  "rendering": {
    "quality": "high",
    "shadows": true
  }
}
```

### Dynamic Feature Toggles

Enable/disable features without restart:

```http
POST /api/features/toggle
Content-Type: application/json

{
  "feature": "gpu-acceleration",
  "enabled": true
}
```

---

## Feature Flags

### System Features

| Flag | Default | Description |
|------|---------|-------------|
| `gpu-acceleration` | `false` | GPU-accelerated physics |
| `neural-enhancement` | `false` | Neural network optimization |
| `wasm-acceleration` | `false` | WebAssembly SIMD |
| `ontology-validation` | `true` | OWL ontology validation |
| `github-sync` | `true` | GitHub repository sync |

### Experimental Features

| Flag | Default | Description |
|------|---------|-------------|
| `delta-encoding` | `false` | WebSocket delta encoding (V4) |
| `quantum-resistant-auth` | `false` | Post-quantum cryptography |
| `distributed-reasoning` | `false` | Multi-node reasoning |
| `solid-integration` | `false` | Solid/LDP pod integration |
| `nostr-solid-auth` | `false` | NIP-98 auth for Solid endpoints |

---

## Performance Tuning

### Recommended Configurations

#### Small Deployment (< 10K nodes)

```bash
# .env
MEMORY_LIMIT=8g
CPU_LIMIT=4.0
WORKER_THREADS=4
ENABLE_GPU=false
```

#### Medium Deployment (10K-100K nodes)

```bash
MEMORY_LIMIT=16g
CPU_LIMIT=8.0
WORKER_THREADS=8
ENABLE_GPU=true
GPU_MEMORY_LIMIT=8g
```

#### Large Deployment (100K+ nodes)

```bash
MEMORY_LIMIT=32g
CPU_LIMIT=16.0
WORKER_THREADS=16
ENABLE_GPU=true
GPU_MEMORY_LIMIT=16g
WS_CONNECTION_LIMIT=2000
```

---

## Security Configuration

### Production Security Checklist

```bash
# Strong authentication
AUTH_REQUIRED=true
JWT_SECRET=$(openssl rand -hex 32)

# HTTPS enforcement
HSTS_MAX_AGE=31536000
FORCE_HTTPS=true

# CORS configuration
CORS_ORIGINS="https://yourdomain.com"
CORS_CREDENTIALS=true

# Rate limiting
RATE_LIMIT_ENABLED=true
RATE_LIMIT_MAX=1000
RATE_LIMIT_WINDOW=900

# Database security
POSTGRES_SSL_MODE=require
REDIS_PASSWORD=$(openssl rand -hex 24)

# Audit logging
ENABLE_AUDIT_LOGGING=true
LOG_SENSITIVE_DATA=false
```

---

## Configuration Validation

### Validation Script

```bash
#!/bin/bash
# scripts/validate-config.sh

# Check required variables
required_vars=(
  "JWT_SECRET"
  "POSTGRES_PASSWORD"
)

for var in "${required_vars[@]}"; do
  if [ -z "${!var}" ]; then
    echo "Error: $var is not set"
    exit 1
  fi
done

echo "Configuration valid"
```

---

## Related Documentation

- [API Reference](../rest-api.md)
- [Error Reference](../error-codes.md)
- [Deployment Guide](../../how-to/deployment-guide.md)
