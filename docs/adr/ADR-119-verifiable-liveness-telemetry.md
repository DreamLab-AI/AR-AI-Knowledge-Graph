# ADR-119 — Fail-open with verifiable per-channel liveness telemetry (anti-PRD-018)

**Status:** Accepted — retroactive record 2026-07-22. Split out of the ADR-112
Decision register (§5, row ADR-119). The liveness sink — flagged unwired in the
doc-drift audit (§2 C2, the exact "wired ≠ working" trap it was built to avoid) —
**shipped 2026-07-22** as `agentbox/mcp/servers/lib/ontology-telemetry.js`
(real default file+memory sink, JSONL, boot canary), consumed by
`ontology-retrieval.js` (`getTelemetrySnapshot` `:235`) and surfaced through
`ontology-bridge.js` `ontology_health` as `_agentbox_ontology_ask_telemetry`
(`:244`). 20/20 node tests pass.
**Date:** 2026-06-14 (decided under ADR-112) · shipped + recorded 2026-07-22
**Decision-type:** Operations
**Relates:** ADR-112 (keystone §2.5, §7), PRD-018 (the silent-dead-wiring
precedent), ADR-117/118 (the WS-0 sweep this proves is live), PRD-020

---

## 1. Context

ADR-112 §2.5 required liveness to be proven by a **per-channel matrix + startup
canary**, "not by wiring", because PRD-018 is the cautionary precedent: ontology
forces were compiled but silently inert for months because nobody proved the
consumer invoked them. The doc-drift audit (§2 C2) found the telemetry itself had
fallen into that trap: the retrieval brain recorded `fail_open` / `ask` /
`cache_hit` events into a **default no-op sink** `{ record(){} }`, so the records
vanished and `fail_open_count` / the liveness matrix were **unobservable** unless
a dependency was injected. No `ontology-telemetry` module, no startup canary.

## 2. Decision

Ship a **real default sink** (`ontology-telemetry.js`) so the brain is observable
out of the box, and keep the whole path **fail-open** so telemetry can never
break retrieval.

- **`createTelemetrySink()`** makes events observable three ways:
  1. in-memory counters — `fail_open` (aliased `fail_open_count` in `snapshot()`,
     `:151`), per-stage `fail_open_seed`/`fail_open_expand`, `canary_ok/fail`,
     `ask`, `cache_hit`, `write_errors`;
  2. an append-only **JSONL** audit trail (`{ts, event, detail, counters}`) under
     `AGENTBOX_POD_ROOT/telemetry/ontology-retrieval.jsonl` (override via
     `AGENTBOX_ONTOLOGY_TELEMETRY_PATH`);
  3. a **startup canary** that writes one liveness record and reads it back.
- **Cause-split** fail-open: `record({event:'fail_open', stage, cause})` with
  `classifyCause(err)`, so `fail_open_count{cause=auth}` is distinguishable
  (ADR-112 §7 item 6 steady-state assertion).
- The retrieval brain records at the real seam points:
  `ontology-retrieval.js:166` (cache_hit), `:175` (fail_open seed), `:205`
  (fail_open expand), `:227` (ask). The default is now this sink, not `{record(){}}`.
- **Fail-open contract** (ADR-112 / PRD-020): every disk op is wrapped; a dead or
  read-only data dir degrades to in-memory counters + a **loud warning**
  (`:127`) and never throws into the retrieval path — retrieval continues even if
  telemetry cannot persist.
- **Observability path:** `ontology-bridge.js` boot logs the sink target +
  `fail_open_count` (`:230`); `ontology_health` returns the full snapshot under
  `_agentbox_ontology_ask_telemetry` (`:244`), so the liveness matrix and
  `fail_open_count` are readable without shell access to the JSONL.

## 3. Consequences

**Positive** — the anti-PRD-018 guarantee is real: a green boot now proves an
*authed* read canary fired and was read back, and `fail_open_count` is a live
observable, not a value that silently defaulted to zero. Reconciles ADR-112 §7
items 2 and 6.

**Negative** — a per-turn JSONL append is a small write cost; bounded by the
fail-open wrapper (a slow/full disk degrades to memory-only, never blocks).

**Neutral** — the JSONL is append-only and unrotated at ship; rotation is an
operator concern, not a retrieval-path one.

## 4. Verification
20/20 node tests pass. `getTelemetrySnapshot()` exposes `fail_open_count` and the
canary verdict; a gap-close evidence file with a Falsification block asserts
`fail_open_count` is observable end-to-end (audit §3, ADR-119 telemetry row).
