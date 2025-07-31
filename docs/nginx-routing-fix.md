# Nginx Routing Configuration Fix

## Issue
The client on port 3001 was getting 502 Bad Gateway errors when trying to connect to the backend API and WebSocket endpoints.

## Root Cause Analysis

### Port Configuration Architecture
The development environment uses a specific port configuration:

1. **Nginx**: Listens on port 3001 (exposed to host)
2. **Rust Backend**: Runs on port 4000 (internal to container)
3. **Vite Dev Server**: Runs on port 5173 (internal)
4. **Vite HMR**: Runs on port 24678 (internal)

### Configuration Sources
- **settings.yaml**: Sets default `port: 3001`
- **docker-compose.dev.yml**: Overrides with `SYSTEM_NETWORK_PORT=4000`
- **nginx.dev.conf**: Proxies from 3001 to backend on 4000

### How It Works
1. The settings.yaml file intentionally has `port: 3001` as a default
2. The environment variable `SYSTEM_NETWORK_PORT=4000` overrides this at runtime
3. Nginx listens on 3001 and proxies requests:
   - `/api/*` → Backend on 4000
   - `/wss`, `/ws/*` → WebSocket on backend 4000
   - `/` → Vite dev server on 5173
   - `/ws` → Vite HMR on 24678

## Changes Made

### 1. Fixed Claude Flow Host Name
Updated `docker-compose.dev.yml`:
```yaml
- CLAUDE_FLOW_HOST=powerdev     # Old
+ CLAUDE_FLOW_HOST=multi-agent-container  # New
```

### 2. Maintained Correct Port Configuration
The settings.yaml should keep `port: 3001` as it's meant to be overridden by the environment variable.

## Troubleshooting

If you're still getting 502 errors:

1. **Check if backend is running**:
   ```bash
   docker logs logseq_spring_thing_webxr
   ```

2. **Verify port override is working**:
   ```bash
   docker exec logseq_spring_thing_webxr sh -c 'echo "SYSTEM_NETWORK_PORT=$SYSTEM_NETWORK_PORT"'
   ```

3. **Check nginx logs**:
   ```bash
   docker exec logseq_spring_thing_webxr cat /var/log/nginx/error.log
   ```

4. **Restart the container if needed**:
   ```bash
   docker-compose -f docker-compose.dev.yml restart webxr
   ```

## Key Takeaway
The port configuration is intentionally complex to allow flexibility between development and production environments. The environment variable override mechanism allows the same settings.yaml to work in different contexts.