---
title: ComfyUI Service Integration - Automatic Startup
description: ComfyUI is now configured to start automatically as a supervised service on port 8188 when the container launches.
category: how-to
tags:
  - api
  - api
  - endpoints
  - http
  - docker
related-docs:
  - architecture/overview.md
  - architecture/overview.md
  - ASCII_DEPRECATION_COMPLETE.md
updated-date: 2025-12-18
difficulty-level: intermediate
status: deprecated
dependencies:
  - Docker installation
---

> **DEPRECATED**: This document describes the legacy `multi-agent-docker` ComfyUI integration. For the current Nix-based agentbox deployment, see [agentbox ComfyUI docs](../../../agentbox/docs/user/comfyui.md). The paths below reference `multi-agent-docker/` which is on deprecation track per [ADR-058](../../archive/adr/ADR-058-mad-to-agentbox-migration.md).

# ComfyUI Service Integration - Automatic Startup

## Overview

ComfyUI is now configured to start automatically as a supervised service on port 8188 when the container launches.

## Changes Made

### 1. supervisord.unified.conf

**Location:** `/home/devuser/workspace/project/multi-agent-docker/unified-config/supervisord.unified.conf`

**Added service (priority 520):**

```ini
[program:comfyui]
command=/home/devuser/ComfyUI/venv/bin/python /home/devuser/ComfyUI/main.py --listen 0.0.0.0 --port 8188
directory=/home/devuser/ComfyUI
user=devuser
environment=HOME="/home/devuser",DISPLAY=":1",CUDA_VISIBLE_DEVICES="0",LD_LIBRARY_PATH="/opt/cuda/lib64:/usr/local/cuda/lib64"
autostart=true
autorestart=true
priority=520
stdout_logfile=/var/log/comfyui.log
stderr_logfile=/var/log/comfyui.error.log
startsecs=10
stopwaitsecs=30
stopasgroup=true
killasgroup=true
```

**Configuration Details:**
- **Port**: 8188 (default ComfyUI port)
- **User**: devuser (same as other services)
- **GPU**: Uses GPU 0 via CUDA_VISIBLE_DEVICES
- **Auto-restart**: Yes - will restart if it crashes
- **Startup delay**: 10 seconds before considered "running"
- **Shutdown grace**: 30 seconds to gracefully shut down

### 2. entrypoint-unified.sh

**Location:** `/home/devuser/workspace/project/multi-agent-docker/unified-config/entrypoint-unified.sh`

**Updated service list (line 711):**
```bash
echo "  ✓ ComfyUI server (port 8188)"
```

### 3. build-unified.sh

**Location:** `/home/devuser/workspace/project/multi-agent-docker/build-unified.sh`

**Added verification (lines 115-136):**
```bash
echo "Testing ComfyUI service..."
if docker exec agentic-workstation /opt/venv/bin/supervisorctl status comfyui | grep -q RUNNING; then
    echo "✅ ComfyUI service running (port 8188)"

    # Test if ComfyUI is responding
    if docker exec agentic-workstation curl -s http://localhost:8188/system_stats >/dev/null 2>&1; then
        echo "✅ ComfyUI API responding"
        # Show device info (GPU or CPU)
    fi
fi
```

## Service Management

### Check Service Status

```bash
# Inside container
sudo supervisorctl status comfyui

# Outside container
docker exec agentic-workstation /opt/venv/bin/supervisorctl status comfyui
```

**Expected output:**
```
comfyui                          RUNNING   pid 1234, uptime 0:05:23
```

### View Logs

```bash
# Real-time logs
sudo supervisorctl tail -f comfyui

# Outside container
docker exec agentic-workstation /opt/venv/bin/supervisorctl tail -f comfyui

# Log files
tail -f /var/log/comfyui.log
tail -f /var/log/comfyui.error.log
```

### Restart Service

```bash
# Restart ComfyUI
sudo supervisorctl restart comfyui

# Stop ComfyUI
sudo supervisorctl stop comfyui

# Start ComfyUI
sudo supervisorctl start comfyui
```

### Access ComfyUI

**From inside container:**
```bash
curl http://localhost:8188/system_stats
```

**From host machine:**
```bash
# If port 8188 is exposed in docker-compose.yml
curl http://localhost:8188/system_stats
```

**Web UI:**
- URL: http://localhost:8188
- No authentication required

## Integration with Management API

The Management API (port 9090) already has ComfyUI endpoints built in:

- `POST /v1/comfyui/workflow` - Submit workflow
- `GET /v1/comfyui/workflow/:id` - Get status
- `DELETE /v1/comfyui/workflow/:id` - Cancel workflow
- `GET /v1/comfyui/models` - List models
- `GET /v1/comfyui/outputs` - List outputs
- `WS /v1/comfyui/stream` - Real-time updates

**To connect Management API to ComfyUI:**

Edit `/opt/management-api/utils/comfyui-manager.js` and update the `_processQueue` method to use `http://localhost:8188` instead of simulation.

See `/home/devuser/workspace/project/docs/comfyui-management-api-integration-summary.md` for full details.

## Startup Sequence

Services start in priority order:

1. **Priority 50**: SSH server
2. **Priority 90-100**: Xvfb, x11vnc (display)
3. **Priority 200-210**: Openbox, tint2 (desktop)
4. **Priority 300**: Management API
5. **Priority 400**: code-server
6. **Priority 500**: Claude Z.AI
7. **Priority 510-517**: MCP servers
8. **Priority 520**: **ComfyUI** ← NEW
9. **Priority 600**: Gemini-flow
10. **Priority 900**: tmux autostart

## GPU Configuration

ComfyUI is configured with:
- `CUDA_VISIBLE_DEVICES="0"` - Uses first GPU
- `LD_LIBRARY_PATH="/opt/cuda/lib64:/usr/local/cuda/lib64"` - CUDA libraries
- `DISPLAY=":1"` - X11 display for GUI apps

On startup, ComfyUI will detect GPU and show:
```
Device: cuda:0 NVIDIA RTX A6000
```

If GPU is not available, it will fall back to CPU:
```
Device: cpu
```

## Expected Startup Logs

**Success (GPU mode):**
```
Checkpoint files will always be loaded safely.
Total VRAM 49140 MB, total RAM 385573 MB
pytorch version: 2.6.0+cu124
Set vram state to: NORMAL_VRAM
Device: cuda:0 NVIDIA RTX A6000
...
To see the GUI go to: http://0.0.0.0:8188
```

**Fallback (CPU mode):**
```
Checkpoint files will always be loaded safely.
Total VRAM 385573 MB, total RAM 385573 MB
pytorch version: 2.6.0+cu124
Set vram state to: DISABLED
Device: cpu
...
To see the GUI go to: http://0.0.0.0:8188
```

## Troubleshooting

### Service won't start

**Check logs:**
```bash
sudo supervisorctl tail comfyui
```

**Common issues:**
- Python environment not activated → Check command path
- Missing dependencies → Check Dockerfile Phase 14
- Port 8188 in use → Check for other processes
- GPU not accessible → Check CUDA environment variables

### Service crashes immediately

**Check error log:**
```bash
tail -50 /var/log/comfyui.error.log
```

**Common causes:**
- Missing FLUX model → Will download on first run
- CUDA library mismatch → Check LD_LIBRARY_PATH
- Permissions issue → Should run as devuser

### API not responding

**Check if service is running:**
```bash
supervisorctl status comfyui
```

**Check if port is listening:**
```bash
netstat -tlnp | grep 8188
# or
ss -tlnp | grep 8188
```

**Test direct connection:**
```bash
curl -v http://localhost:8188/
```

### GPU not detected

**Check CUDA environment:**
```bash
echo $CUDA_VISIBLE_DEVICES
echo $LD_LIBRARY_PATH
nvidia-smi
```

**Test PyTorch CUDA:**
```bash
cd /home/devuser/ComfyUI
source venv/bin/activate
python -c "import torch; print(torch.cuda.is_available())"
```

## Testing After Container Rebuild

After rebuilding the container:

```bash
# 1. Check service started
docker exec agentic-workstation /opt/venv/bin/supervisorctl status comfyui

# 2. Check API responding
docker exec agentic-workstation curl -s http://localhost:8188/system_stats

# 3. Check GPU detected
docker exec agentic-workstation curl -s http://localhost:8188/system_stats | \
  docker exec -i agentic-workstation python3 -c "import sys,json; print(json.load(sys.stdin)['devices'][0])"

# 4. Generate test image (if FLUX model installed)
# Submit workflow via Management API or ComfyUI web interface
```

## Performance Notes

**Startup time:**
- First start: 15-30 seconds (loading models)
- Subsequent starts: 10-15 seconds

**Memory usage:**
- Base: ~2-3 GB RAM
- With FLUX loaded: ~8-12 GB VRAM (GPU mode)
- With FLUX loaded: ~20-30 GB RAM (CPU mode)

**Generation speed:**
- GPU (RTX A6000): 3-15 seconds per image
- CPU: 2-5 minutes per image

## Port Exposure

To access ComfyUI from outside the container, add to `docker-compose.unified.yml`:

```yaml
ports:
  - "2222:22"
  - "5901:5901"
  - "8080:8080"
  - "9090:9090"
  - "8188:8188"  # ← Add this line
```

Then rebuild and access at http://localhost:8188

## Security Considerations

**Current configuration:**
- No authentication on ComfyUI (default)
- Listens on 0.0.0.0 (all interfaces)
- Auto-restart enabled

**For production:**
- Consider adding authentication
- Use reverse proxy with SSL
- Restrict to localhost if accessed via Management API only

## Files Modified

1. `/multi-agent-docker/unified-config/supervisord.unified.conf` - Added service definition
2. `/multi-agent-docker/unified-config/entrypoint-unified.sh` - Updated service list
3. `/multi-agent-docker/build-unified.sh` - Added verification checks

## Next Steps

1. **Rebuild container** to apply changes:
   ```bash
   cd /path/to/multi-agent-docker
   docker compose -f docker-compose.unified.yml down
   docker build -f Dockerfile.unified -t agentic-workstation:latest .
   docker compose -f docker-compose.unified.yml up -d
   ```

2. **Verify service running**:
   ```bash
   docker exec agentic-workstation /opt/venv/bin/supervisorctl status comfyui
   ```

3. **Test API**:
   ```bash
   curl http://localhost:8188/system_stats
   ```

4. **Optional: Connect Management API** by updating `/opt/management-api/utils/comfyui-manager.js`

---

## Related Documentation

- [ComfyUI SAM3D Setup](../comfyui-sam3d-setup.md)
- [Development Guide](../development.md)
- [Pipeline Operator Runbook](../operations/pipeline-operator-runbook.md)

## Status

✅ **All changes implemented and ready for container rebuild**

ComfyUI will now:
- Start automatically on container launch
- Restart automatically if it crashes
- Run with GPU acceleration (if available)
- Be accessible on port 8188
- Log to `/var/log/comfyui.log`
- Be manageable via supervisorctl
