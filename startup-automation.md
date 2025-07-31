# Automating PowerDev MCP Service Startup

## Current Situation
The MCP WebSocket relay needs to be manually started in the powerdev container. This should be automated.

## Solution Options

### Option 1: Dockerfile CMD/ENTRYPOINT
Modify the powerdev Dockerfile to include the startup script:

```dockerfile
# In powerdev Dockerfile
CMD ["/bin/bash", "/workspace/start-claude-flow-mcp.sh"]
```

### Option 2: Docker Compose Command Override
Add command override in docker-compose.yml:

```yaml
services:
  powerdev:
    # ... existing config ...
    command: /bin/bash /workspace/start-claude-flow-mcp.sh
```

### Option 3: Supervisor Process Manager
Use supervisor to manage multiple processes:

```dockerfile
# Install supervisor
RUN apt-get update && apt-get install -y supervisor

# Add supervisor config
COPY supervisord.conf /etc/supervisor/conf.d/supervisord.conf

# Start supervisor
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/supervisord.conf"]
```

With supervisord.conf:
```ini
[supervisord]
nodaemon=true

[program:mcp-ws-relay]
command=/bin/bash /workspace/start-claude-flow-mcp.sh
autostart=true
autorestart=true
stderr_logfile=/var/log/mcp-ws-relay.err.log
stdout_logfile=/var/log/mcp-ws-relay.out.log

[program:keep-alive]
command=python /app/keep_alive.py
autostart=true
autorestart=true
```

### Option 4: systemd Service (if using systemd)
Create a systemd service file:

```ini
[Unit]
Description=MCP WebSocket Relay
After=network.target

[Service]
Type=simple
WorkingDirectory=/workspace
ExecStart=/bin/bash /workspace/start-claude-flow-mcp.sh
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

## Recommended Approach
**Option 2** (Docker Compose command override) is the simplest and most maintainable solution. It requires minimal changes and keeps the startup logic visible in the docker-compose.yml file.