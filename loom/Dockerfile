# Ontology Loom sidecar — Deployment B (VisionClaw ADR-135 D1).
# The lightweight façade+lifecycle facet as a container; distillation is DELEGATED to
# DISTILL_BACKEND_URL (a co-located model, VisionClaw, or a remote reasoner e.g. HP :8084).
# No GPU in the sidecar — the model is a URL behind the stable façade.
# Corpus is NOT baked: the entrypoint mirrors the published generation into /app/data
# (fail-open to whatever the mounted volume already holds).
FROM python:3.12-slim

RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates bash \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY app/ /app/
RUN chmod +x /app/entrypoint.sh /app/mirror.sh || true

ENV ONTOLOGY_INDEX=/app/data/scaffold-index.json \
    ONTOLOGY_PROSE_INDEX=/app/data/prose-index.json \
    ONTOLOGY_BUDGET=1500 \
    LOOM_FACADE_PORT=8080 \
    LOOM_MIRROR_ON_START=1 \
    ONTOLOGY_SITE=https://narrativegoldmine.com

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=25s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/health || exit 1

ENTRYPOINT ["/app/entrypoint.sh"]
