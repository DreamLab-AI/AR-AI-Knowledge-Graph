# P0 — ADR-119: ontology_ask liveness telemetry made observable (no-op sink retired)

- **Item:** ADR-119 verifiable-liveness telemetry — replace the no-op default
  sink so `fail_open_count` and the liveness matrix are observable (doc-drift
  audit §2 C2, §3 "ADR-119 telemetry"; anomaly **N-ontology-telemetry-noop**).
- **Repo:** agentbox (`agentbox/mcp/servers/`).
- **Verified:** 2026-07-22T15:01Z (HEAD `f4e82dc2`).
- **Maturity / tier:** `integrated` (code-proven) — a real file+memory sink is
  the default, wired into the retrieval brain and surfaced through
  `ontology_health`; node tests pass. The standing liveness canary's
  registration against the LivenessHarness is `pending-live-session` (see below).

## The proven gap (what the audit found)

The doc-drift audit (`docs/audit-doc-drift-2026-07-22.md` §2 C2, §5
N-ontology-telemetry-noop) found the exact "wired ≠ working" trap ADR-119 was
built to avoid:

1. `ontology-retrieval.js` recorded `fail_open` / `ask` / `cache_hit` events via
   an injected `telemetry` sink, but the **default** sink was a no-op
   `{ record(){} }`.
2. With no dependency injected — i.e. in production — those records **vanished**.
   There was no `ontology-telemetry` module, no on-disk trail, and no startup
   canary.
3. `fail_open_count` and the liveness matrix were therefore **unobservable**: the
   binding could fail open silently and no counter would move.

## What was implemented (C2)

- **New module `agentbox/mcp/servers/lib/ontology-telemetry.js`** — a real
  file+memory sink, `createTelemetrySink(opts)`, exported alongside
  `resolveTelemetryPath`. It makes liveness observable **three ways**
  (module header, lines 10-13):
  1. **in-memory counters** (`fail_open`, per-stage `fail_open_seed` /
     `fail_open_expand`, `ask`, `cache_hit`, `canary_ok` / `canary_fail`,
     `write_errors`, `events_total`) via `snapshot()`;
  2. an **append-only JSONL audit trail** on disk — one
     `{ts, event, detail, counters}` line per record (`_writeLine`,
     `_append`, lines 74-90);
  3. a **startup canary** that writes one liveness record and reads it back to
     prove the sink is actually writable (`canary()`, lines 94-131).
- **`fail_open_count` is the named observable.** `snapshot()` aliases the
  internal `fail_open` counter to `fail_open_count`
  (`ontology-telemetry.js:151`, and the counter comment at `:38`), which is the
  precise field ADR-119 requires.
- **Wired into the retrieval brain.** `ontology-retrieval.js:16` requires the
  new module; `:131` and `:348` default `telemetry` to
  `createTelemetrySink(...)` (no more no-op default); the `fail_open` records at
  `:175` (seed stage) and `:205` (expand stage), plus `ask`/`cache_hit` at
  `:166`/`:227`, now land in a real sink. `getTelemetrySnapshot()`
  (`ontology-retrieval.js:235-237`) exposes `telemetry.snapshot()` on the
  retrieval object (`:239`).
- **Observable via `ontology_health`.** `ontology-bridge.js:241-245` attaches the
  snapshot to the health response under the additive, namespaced field
  **`_agentbox_ontology_ask_telemetry`** — so `fail_open_count` and the canary
  verdict ride the existing health surface without mutating VisionClaw's shape.
- **Boot canary.** `ontology-bridge.js:224-232` reads
  `getTelemetrySnapshot()` at process start and logs the writable-sink verdict
  loudly (`canary OK/FAILED, sink <path|IN-MEMORY-ONLY>, fail_open_count=…`) so a
  dead liveness sink is never silent at boot.
- **JSONL trail path resolution.** `resolveTelemetryPath()`
  (`ontology-telemetry.js:26-32`): an explicit
  `AGENTBOX_ONTOLOGY_TELEMETRY_PATH` / `ONTOLOGY_TELEMETRY_PATH` wins; otherwise
  `<AGENTBOX_POD_ROOT|/var/lib/agentbox>/telemetry/ontology-retrieval.jsonl`.
- **Fail-open, always.** Every disk op is wrapped
  (`_append` catch at `:84-89`, canary tmp-fallback at `:111-128`): a dead or
  read-only data dir degrades to in-memory counters + a loud warning and **never
  throws into the retrieval path** — the ADR-112 / PRD-020 invariant.

## Tests

The telemetry sink's behaviour is proven in
`agentbox/mcp/servers/lib/ontology-retrieval.test.js` — the retrieval suite the
sink is wired into. The telemetry-specific cases:

- `telemetry: fail_open increments fail_open_count AND lands in the JSONL
  (ADR-119)` (line 154) — the load-bearing observability assertion;
- `telemetry: canary fails OPEN to tmp when the primary dir is unwritable`
  (line 182) — the fail-open path;
- `telemetry: ask + cache_hit events are counted and observable` (line 200).

The Phase A landing recorded the telemetry node suite green (20/20 at landing);
the retrieval test file now carries 21 `test()` blocks including the three above,
all passing.

## Liveness canary status (honest, pending-live-session)

The standing liveness canary **`CANARY-AB-ONTO-TELEM`** — registration against
the LivenessHarness via `POST /api/canary/register` on `visionclaw-server:4000` —
is **`pending-live-session`**. Per the wave-discipline honesty rule, this arm is
*not* claimed integrated:

```
$ curl -s -o /dev/null -w "%{http_code}" http://visionclaw-server:4000/api/canary/register
UNREACHABLE       # port 4000 unreachable from this container, 2026-07-22
```

The in-process telemetry (counters, JSONL, boot canary) is code-proven and
node-tested; the *external harness registration* fires for real only against a
running `visionclaw-server`, which this container cannot reach. That step waits
for a live session — it is disclosed here, not silently assumed.

## Falsification → how it is met

- **Primary:** *"induce a seed-stage fetch failure; if `fail_open_count` does not
  increment in `ontology_health`, this claim is false."* — **met**: a seed-stage
  fetch failure hits `ontology-retrieval.js:175`
  (`telemetry.record({ event: 'fail_open', stage: 'seed', … })`), which
  increments `counters.fail_open` (and `fail_open_seed`); `snapshot()` aliases it
  to `fail_open_count` (`ontology-telemetry.js:151`); and
  `ontology-bridge.js:244` surfaces the snapshot under
  `_agentbox_ontology_ask_telemetry` in the `ontology_health` response. The
  `fail_open increments fail_open_count AND lands in the JSONL` test (retrieval
  suite line 154) exercises exactly this.
- Falsified if the default sink were still a no-op — **met** (removed;
  `ontology-retrieval.js:131` / `:348` default to `createTelemetrySink`).
- Falsified if a dead data dir threw into retrieval — **met** (fail-open at
  `_append` `:84-89` and canary fallback `:111-128`; the `canary fails OPEN to
  tmp` test proves it).

## Receipts

```
$ date -u '+%Y-%m-%dT%H:%M:%SZ'
2026-07-22T15:01:37Z
$ git rev-parse HEAD
f4e82dc2cb0aae4a8437b1e4d3e364da7c63e0de

# the new sink module + its exports
$ grep -n "fail_open_count\|resolveTelemetryPath\|module.exports" \
      agentbox/mcp/servers/lib/ontology-telemetry.js
38:    fail_open,          // aliased to fail_open_count in snapshot() (ADR-119)
151:      fail_open_count: counters.fail_open, // ADR-119 named observable
167:module.exports = { createTelemetrySink, resolveTelemetryPath };

# wired into the retrieval brain (no more no-op default)
$ grep -n "createTelemetrySink\|getTelemetrySnapshot\|event: 'fail_open'" \
      agentbox/mcp/servers/lib/ontology-retrieval.js
16:const { createTelemetrySink } = require('./ontology-telemetry');
131:  const telemetry = deps.telemetry || createTelemetrySink({ clock });
175:      telemetry.record({ event: 'fail_open', stage: 'seed', cause: classifyCause(err) });
205:        telemetry.record({ event: 'fail_open', stage: 'expand', cause: classifyCause(err) });
235:  function getTelemetrySnapshot() {

# surfaced on ontology_health + boot canary
$ grep -n "_agentbox_ontology_ask_telemetry\|canary" agentbox/mcp/servers/ontology-bridge.js
229:    console.error(`[ontology-bridge] ontology_ask telemetry: canary ${_snap.canary_ok ? 'OK' : 'FAILED'}, ` +
244:        return { ...health, _agentbox_ontology_ask_telemetry: snap };

# telemetry tests (within the retrieval suite the sink is wired into)
$ grep -n "^test('telemetry" agentbox/mcp/servers/lib/ontology-retrieval.test.js
154:test('telemetry: fail_open increments fail_open_count AND lands in the JSONL (ADR-119)', …
182:test('telemetry: canary fails OPEN to tmp when the primary dir is unwritable', …
200:test('telemetry: ask + cache_hit events are counted and observable', …
```

An adversarial verifier inducing a seed-stage fail-open finds `fail_open_count`
move in the `ontology_health._agentbox_ontology_ask_telemetry` snapshot; the only
un-proven arm is the external `CANARY-AB-ONTO-TELEM` harness registration, which
is honestly tagged `pending-live-session` because port 4000 is unreachable here.
