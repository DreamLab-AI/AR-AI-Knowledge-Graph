# Ontology binding — env tokens to paste into `agentbox/.env`

> I can't edit `agentbox/.env` (permission-blocked secrets file). Paste the lines
> below into it, then **restart agentbox** so the env reloads. `AGENTBOX_PUBKEY`
> is already correct (it's in `POWER_USER_PUBKEYS`); only the token is wrong.

## 1. REQUIRED — fix the placeholder token (currently causes SPARQL expand → 403)

Replace the existing line `VISIONCLAW_DEV_TOKEN = this_is_the_dev_token` with:

```
VISIONCLAW_DEV_TOKEN=dev-session-token
```

(Verified live: `dev-session-token` → HTTP 200 on `POST /api/ontology/sparql`; the placeholder → 403. This is the dev power_user token; swap for the real secret in prod.)

## 2. Endpoint (default is already correct — include only if not present)

```
VISIONCLAW_API_URL=http://visionclaw-server:4000
```

## 3. Enable the channels (both default OFF per ADR-122 / resolved choices)

```
# PUSH breadcrumb on every turn — set to 1 AFTER WS-2 builds the cache (else it no-ops)
ONTOLOGY_INJECT=1

# Consultant seam (PULL-A) on by default for all 5 consultants
# (leave unset to keep per-call: pass ontology_context:true on a consult instead)
CONSULT_ONTOLOGY_AUGMENT=1
```

## 4. Optional tuning (defaults shown — only add to override)

```
CONSULT_ONTOLOGY_MAX_TOKENS=1500
ONTOLOGY_PUSH_MIN_RELEVANCE=0.3
ONTOLOGY_PUSH_CACHE=/home/devuser/.claude-flow/data/ontology-classes-cache.json
ONTOLOGY_TIMEOUT_MS=10000
```

---

**After pasting + restart:** the deployed `ontology-bridge` self-test should log
`self-test OK: authed SELECT returned N row(s)`, and `ontology_ask` expand mode
will work. (Also requires the `a458ddf9` lib fix to be baked — i.e. one more rebuild.)
