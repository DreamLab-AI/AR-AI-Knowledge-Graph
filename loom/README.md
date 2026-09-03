# Ontology Loom — Deployment B (sidecar)

**Deployment B is the Rust façade.** This directory no longer contains an
implementation; it holds the deployment notes for the `loom` service defined in
[`../docker-compose.unified.yml`](../docker-compose.unified.yml) under the
`loom` profile.

The canonical implementation lives in the **loom repo**
(`/home/devuser/workspace/loom`) as the `loom-facade` binary. Until 2026-09-03
this directory carried a second, Python implementation of the same contract —
`app/{loom_facade,ontology_proxy,ontology_scaffold,loom_graph}.py`, 1,727 lines —
serving the same endpoints under the same environment-variable names. It was a
dead twin: identical contract, an entire extra language runtime, and a `pyoxigraph`
dependency where the Rust crate already uses `oxigraph` directly. It has been
deleted. See `docs/python-legacy-audit-2026-09-02.md` item #4.

## What this service is

The consumer-facing **door** on `visionclaw_network`, and a required one: the
email gateway binds `REASONER_BASE_URL=http://loom:8080/v1`. Grounding
(`/loom/scaffold`, `/health`) needs no model; only `/v1/*` delegates to one.

The model is always a URL behind the façade (`DISTILL_BACKEND_URL`) — swapping
Gemma → Muse → Qwen3.8 → next never touches a consumer.

| Endpoint | Purpose | Needs a model? |
|---|---|---|
| `GET  /health` | liveness, corpus generation stamp, backend/graph/index readiness, `injection_policy` | no |
| `GET  /loom/generation` | the corpus generation identity being served | no |
| `POST /loom/scaffold` | budget-clamped ontology grounding for a prompt | no |
| `POST /loom/sparql` | read-only, clamped SPARQL over the reasoned closure | no |
| `POST /loom/search` | label/substring search over the store | no |
| `POST /v1/chat/completions` | scaffold-inject the last user message → delegate | **yes** |
| `GET  /v1/models` | model identity passthrough | **yes** |

## Building the image

The image is **not** built from this repo — there is no Dockerfile here any more.
Build it on the host from the loom checkout. Its Dockerfile takes the workspace
**parent** as its build context, because loom path-depends on the sibling
`../ruvector` crate and a `COPY` cannot escape its context:

```bash
docker build -f loom/deploy/Dockerfile -t loom:rust /home/devuser/workspace
```

Then bring the sidecar up from this repo:

```bash
docker compose -f docker-compose.unified.yml --profile loom up -d loom
curl -fsS http://127.0.0.1:${LOOM_PORT:-8090}/health
```

Override the tag with `LOOM_IMAGE` if you build it under another name.

## Staging the corpus generation

The Python sidecar mirrored the corpus from `ONTOLOGY_SITE` on every start. **The
Rust image has no mirror-on-start step** — `ONTOLOGY_SITE` and
`LOOM_MIRROR_ON_START` are gone from the service, and the generation is staged by
the operator and then served immutably (read-only mount).

Point `LOOM_DATA_SOURCE` at either the named `loom-data` volume (the default) or
a host directory holding a full generation:

```bash
LOOM_DATA_SOURCE=/home/devuser/workspace/loom/data
```

A full generation is `scaffold-index.json` + `prose-index.json` + the TTLs +
`ontology-corpus.rvdb` (with its `.generation.json` sidecar).

> **The empty-floor trap.** With an empty or mis-pointed source the façade still
> starts and `/health` still returns 200, but the log reads *"lexical index NOT
> loaded ... empty floor"*. That is a staging bug, not a dead container — check
> the mount before you check the process.

The `.rvdb` is mounted read-only but *opening* it mutates the redb file even for
reads (the HNSW index is repacked on open), so the image's entrypoint copies it
to a writable `tmpfs` at `/run/loom` first. The tmpfs `uid`/`gid` must stay at
`65532` to match the image's non-root user, or that copy fails `EACCES`.

## Environment contract

Set in the compose service; every one of these is read by `loom-facade`.

| Variable | Default here | Meaning |
|---|---|---|
| `DISTILL_BACKEND_URL` | *(blank)* | the model-swap seam; blank ⇒ retrieval-only, `/v1` returns 503 |
| `XINFERENCE_URL` | `http://xinference:9997/v1` | bge-small-en-v1.5/384 for the semantic fallback's query embed |
| `ONTOLOGY_BUDGET` | `1500` | scaffold token budget |
| `LOOM_FACADE_PORT` | `8080` | in-container listen port (the gateway depends on this) |
| `LOOM_DEPLOY_PROFILE` | `b` | echoed in `/health` |
| `LOOM_SEMANTIC_FALLBACK` | `0` | gated off until the recall bench clears |
| `LOOM_MIN_MAX_TOKENS` | `1536` | reasoning backends truncate to empty below this |
| `LOOM_CONFIDENCE_INJECTION` | `0` | confidence-gate master switch |
| `LOOM_STRONG_MATCH_SCORE` | `8.0` | at/above ⇒ full budget; the `confidence` denominator |
| `LOOM_MIN_INJECT_SCORE` | `2.0` | below ⇒ skip injection entirely |
| `LOOM_MIN_INJECT_FRACTION` | `0.4` | weakest kept match's share of budget |

The four confidence variables are **stated explicitly** rather than inherited, so
that `/health.injection_policy` can be diffed against this file. Their defaults
reproduce the loom repo's Profile B posture — master switch off, thresholds pinned
at the code defaults in `loom-scaffold`'s `tuning.rs`. The 2026-09-02 dream cycle
found a doc/deploy mismatch caused precisely by leaving them unstated; see loom
`ADR-138`. Arm the gate with `LOOM_CONFIDENCE_INJECTION=1` in `.env`.

## Still here

`app/ontology-mcp/` — a standalone stdio MCP server (JavaScript) that shipped
inside the retired Python image. It is not Python, so it was out of scope for the
port, but with the image gone it now has no build or run path from this repo. The
architecture docs (PRD-025, ADR-135) treat `ontology-mcp` as a thin client of the
Loom index rather than part of the sidecar, so it is left in place pending a
separate decision on where it should live.
