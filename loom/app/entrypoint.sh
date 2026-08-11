#!/usr/bin/env bash
# Loom sidecar entrypoint (Deployment B).
# 1) Refresh the corpus generation from the published cloud replica (atomic, fail-open to
#    the baked seed — WS-A atomic-mirror discipline). 2) Launch the façade.
set -u
cd /app

if [[ "${LOOM_MIRROR_ON_START:-1}" == "1" ]]; then
  echo "[loom] mirroring corpus generation from ${ONTOLOGY_SITE:-https://narrativegoldmine.com} ..."
  ONTOLOGY_SITE="${ONTOLOGY_SITE:-https://narrativegoldmine.com}" \
    bash /app/mirror.sh || echo "[loom] mirror failed — using baked seed in /app/data"
else
  echo "[loom] LOOM_MIRROR_ON_START=0 — using baked/mounted /app/data as-is"
fi

# Optional: MCP server (stdio) is available in the image at /app/ontology-mcp for MCP
# consumers; the HTTP façade is the primary contact endpoint for this sidecar.
exec python3 /app/loom_facade.py
