# Recommendations for Fixing Logseq-Powerdev Connection

## Summary of Issues Found

1. **MCP Relay is Working**: The MCP WebSocket relay is running correctly in powerdev on port 3000
2. **Network Connectivity is Good**: Both containers can reach each other on the docker_ragflow network
3. **Rust Backend Issue**: The Rust backend in logseq is failing to connect, likely due to WebSocket protocol handling

## Immediate Fix

### 1. Update the Rust Backend Configuration

The issue appears to be in how the Rust backend connects to the MCP service. The current code uses WebSocket transport but might have issues with the connection handshake.

### 2. Restart the Logseq Container

From the host machine (NOT from inside powerdev), run:

```bash
# Rebuild and restart the logseq container
cd /path/to/logseq/project
docker-compose -f docker-compose.dev.yml down
docker-compose -f docker-compose.dev.yml up --build -d
```

### 3. Monitor the Connection

After restart, check the logs:

```bash
# Check Rust backend logs
docker exec logseq_spring_thing_webxr tail -f /app/logs/rust.log

# Check MCP relay logs in powerdev
docker exec powerdev tail -f /tmp/mcp-http-wrapper.log
```

## Long-term Improvements

### 1. Add Retry Logic to Rust Backend

The `claude_flow_actor.rs` should implement exponential backoff retry logic for connection failures.

### 2. Implement Health Check Endpoints

Add proper health check endpoints in the Rust backend that verify:
- MCP connection status
- WebSocket connectivity
- Agent polling functionality

### 3. Update Nginx Configuration

The nginx configuration should properly proxy WebSocket connections to the MCP relay:

```nginx
# Add to nginx.dev.conf
location /ws/mcp-relay {
    proxy_pass http://backend;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection $connection_upgrade;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_read_timeout 600m;
    proxy_buffering off;
}
```

### 4. Environment Variable Updates

Ensure these environment variables are set correctly in `docker-compose.dev.yml`:

```yaml
environment:
  - CLAUDE_FLOW_HOST=powerdev
  - CLAUDE_FLOW_PORT=3000
  - MCP_RELAY_FALLBACK_TO_MOCK=true
```

## Current Working MCP Service

The MCP service is now available at:
- Health Check: `http://powerdev:3000/api/health`
- WebSocket: `ws://powerdev:3000/ws`

The service provides full Claude Flow MCP functionality including:
- Swarm initialization
- Agent spawning
- Task orchestration
- Memory management
- Neural pattern training

## Testing the Connection

From inside the logseq container after restart:

```bash
# Test health endpoint
curl http://powerdev:3000/api/health

# Test WebSocket with wscat
wscat -c ws://powerdev:3000/ws
```

## Next Steps

1. Restart the logseq container with the build flag to ensure all changes are applied
2. Monitor the logs for successful connection
3. The Rust backend should connect and start polling for agent updates
4. The frontend visualization should display agent activity

The MCP relay is running stably in powerdev and ready to accept connections from the logseq container.