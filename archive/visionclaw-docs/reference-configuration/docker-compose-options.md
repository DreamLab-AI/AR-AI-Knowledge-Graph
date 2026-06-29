---
title: Docker Compose Options Reference
description: Complete reference for VisionClaw Docker Compose configuration options
category: reference
difficulty-level: intermediate
tags:
  - configuration
  - docker
  - deployment
updated-date: 2025-01-29
---

# Docker Compose Options Reference

Complete reference for VisionClaw Docker Compose configuration.

---

## Configuration Files

| File | Purpose |
|------|---------|
| `docker-compose.yml` | Main container configuration |
| `docker-compose.solid.yml` | Solid sidecar configuration |
| `docker-compose.override.yml` | Local development overrides |
| `docker-compose.prod.yml` | Production configuration |

---

## Service Configuration

### VisionClaw Container

```yaml
services:
  visionclaw:
    image: visionclaw:latest
    ports:
      - "${HOST_PORT:-3001}:3001"
      - "${WS_PORT:-9090}:9090"
      - "${MCP_TCP_PORT:-9500}:9500"
    environment:
      - ENVIRONMENT=${ENVIRONMENT:-development}
      - JWT_SECRET=${JWT_SECRET}
      - POSTGRES_HOST=postgres
      - REDIS_HOST=redis
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs
    depends_on:
      - postgres
      - redis
    deploy:
      resources:
        limits:
          cpus: "${CPU_LIMIT:-8.0}"
          memory: "${MEMORY_LIMIT:-16g}"
        reservations:
          cpus: "${CPU_RESERVATION:-4.0}"
          memory: "8g"
```

### PostgreSQL

```yaml
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: ${POSTGRES_DB:-visionclaw}
      POSTGRES_USER: ${POSTGRES_USER:-visionclaw}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"
    deploy:
      resources:
        limits:
          memory: "4g"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-visionclaw}"]
      interval: 10s
      timeout: 5s
      retries: 5
```

### Redis

```yaml
  redis:
    image: redis:7-alpine
    command: >
      redis-server
      --maxmemory ${REDIS_MAX_MEMORY:-512mb}
      --maxmemory-policy allkeys-lru
    volumes:
      - redis_data:/data
    ports:
      - "6379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5
```

---

## GPU Configuration

### NVIDIA GPU Support

```yaml
services:
  visionclaw:
    # ... other config ...
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: ${GPU_COUNT:-1}
              capabilities: [gpu, compute, utility]
    environment:
      - ENABLE_GPU=true
      - NVIDIA_VISIBLE_DEVICES=${NVIDIA_VISIBLE_DEVICES:-all}
      - CUDA_ARCH=${CUDA_ARCH:-89}
```

### AMD GPU Support (ROCm)

```yaml
services:
  visionclaw:
    devices:
      - /dev/kfd
      - /dev/dri
    group_add:
      - video
    environment:
      - ENABLE_GPU=true
      - GPU_BACKEND=rocm
```

---

## Solid Sidecar (docker-compose.solid.yml)

```yaml
services:
  jss:
    image: ghcr.io/visionclaw/jss:latest
    ports:
      - "${JSS_PORT:-3000}:3000"
      - "${SOLID_WS_PORT:-3001}:3001"
    environment:
      - JSS_BASE_URL=${JSS_BASE_URL:-http://localhost:3000}
      - JSS_ROOT_PATH=/data/pods
      - JSS_LOG_LEVEL=${JSS_LOG_LEVEL:-info}
    volumes:
      - jss_pods:/data/pods
    depends_on:
      - visionclaw
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/.well-known/solid"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  jss_pods:
```

---

## Volume Configuration

### Named Volumes

```yaml
volumes:
  postgres_data:
    driver: local
  redis_data:
    driver: local
  visionclaw_data:
    driver: local
  jss_pods:
    driver: local
```

### Bind Mounts (Development)

```yaml
services:
  visionclaw:
    volumes:
      - ./data:/app/data:rw
      - ./logs:/app/logs:rw
      - ./config:/app/config:ro
```

---

## Network Configuration

### Custom Network

```yaml
networks:
  visionclaw_network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.28.0.0/16

services:
  visionclaw:
    networks:
      visionclaw_network:
        ipv4_address: 172.28.0.10
```

### External Network

```yaml
networks:
  external_network:
    external: true
    name: my-existing-network
```

---

## Health Checks

### Application Health Check

```yaml
services:
  visionclaw:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9090/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s
```

### Dependency Health Checks

```yaml
services:
  visionclaw:
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
```

---

## Resource Limits

### CPU and Memory

```yaml
services:
  visionclaw:
    deploy:
      resources:
        limits:
          cpus: "8.0"
          memory: "16g"
        reservations:
          cpus: "4.0"
          memory: "8g"
```

### Shared Memory (for GPU)

```yaml
services:
  visionclaw:
    shm_size: "2g"
```

---

## Logging Configuration

### JSON Logging

```yaml
services:
  visionclaw:
    logging:
      driver: "json-file"
      options:
        max-size: "100m"
        max-file: "5"
```

### Syslog

```yaml
services:
  visionclaw:
    logging:
      driver: "syslog"
      options:
        syslog-address: "udp://localhost:514"
        tag: "visionclaw"
```

---

## Deployment Profiles

### Small Deployment (< 10K nodes)

```bash
MEMORY_LIMIT=8g CPU_LIMIT=4.0 ENABLE_GPU=false docker-compose up
```

### Medium Deployment (10K-100K nodes)

```bash
MEMORY_LIMIT=16g CPU_LIMIT=8.0 ENABLE_GPU=true docker-compose up
```

### Large Deployment (100K+ nodes)

```bash
MEMORY_LIMIT=32g CPU_LIMIT=16.0 ENABLE_GPU=true GPU_COUNT=2 docker-compose up
```

---

## Related Documentation

- [Environment Variables](./environment-variables.md)
- [Deployment Guide](../../how-to/deployment-guide.md)
- [Configuration Reference](./README.md)
