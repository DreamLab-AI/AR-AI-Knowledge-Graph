# Claude Flow WebSocket Architecture

## Overview
The Claude Flow integration has been updated to use WebSocket transport instead of stdio for improved reliability and real-time communication.

## Architecture Components

### 1. PowerDev Container
- **Role**: Hosts the MCP WebSocket relay
- **Port**: 3000
- **Service**: `mcp-ws-relay.js`
- **Location**: `/workspace/ext/src/mcp-ws-relay.js`

### 2. WebSocket Relay
The relay acts as a bridge between WebSocket clients and the Claude Flow MCP stdio process:
```
Rust Client <--WebSocket--> mcp-ws-relay.js <--stdio--> claude-flow@alpha MCP
```

### 3. Rust Backend (logseq_spring_thing_webxr)
- **ClaudeFlowActor**: Manages WebSocket connection to PowerDev
- **Connection**: `ws://powerdev:3000/ws`
- **Features**: 
  - Automatic reconnection with exponential backoff
  - Graceful degradation to mock data when disconnected
  - Health monitoring

## Key Configuration

### Environment Variables
```bash
# In Rust container (.env or docker-compose.yml)
CLAUDE_FLOW_HOST=powerdev  # Docker service name
CLAUDE_FLOW_PORT=3000       # WebSocket relay port
```

### Docker Networking
- Both containers must be on the same Docker network
- PowerDev exposes port 3000 internally
- No host port mapping required for container-to-container communication

## Startup Sequence

1. **PowerDev starts** with MCP WebSocket relay
2. **Rust container starts** and attempts connection
3. **If connection fails**, Rust enters degraded mode with mock data
4. **Automatic reconnection** attempts with exponential backoff

## Error Handling

### Connection Failures
- No panic on connection failure
- Provides mock agents for visualization
- Continues attempting reconnection
- Clear error messages in API responses

### API Error Responses
```json
{
  "success": false,
  "error": "Failed to connect to Claude Flow",
  "code": "CONNECTION_REFUSED",
  "details": {
    "suggestion": "Ensure the MCP WebSocket relay is running on powerdev:3000"
  }
}
```

## Monitoring & Diagnostics

### Health Check Endpoint
```bash
GET /api/v1/bots/health
```

### Check WebSocket Relay Status
```bash
# From PowerDev container
netstat -tulpn | grep 3000
ps aux | grep mcp-ws-relay

# View logs
tail -f /var/log/mcp-ws-relay.log
```

### Check Rust Logs
```bash
# From host
./scripts/check-rust-logs.sh -f
```

## Troubleshooting

### MCP Not Starting Automatically
1. Check docker-compose.yml has the correct command
2. Verify `/workspace/ext/src/mcp-ws-relay.js` exists
3. Check Node.js is installed in PowerDev

### Connection Refused Errors
1. Verify MCP relay is running: `docker exec powerdev netstat -tulpn | grep 3000`
2. Check Docker network connectivity
3. Restart both containers in correct order

### Manual Recovery
```bash
# From host system
docker exec powerdev pkill -f mcp-ws-relay
docker exec -d powerdev bash -c "cd /workspace/ext/src && node mcp-ws-relay.js > /var/log/mcp-ws-relay.log 2>&1"
docker restart logseq_spring_thing_webxr
```

## Future Enhancements

### Real-time Updates (Phase 3)
- Implement WebSocket event streaming from MCP
- Push agent status updates to Rust backend
- Eliminate polling in favor of event-driven updates

### Production Hardening
- Implement authentication/authorization
- Add SSL/TLS for WebSocket connections
- Rate limiting and connection management
- Metrics and monitoring integration