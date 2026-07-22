# ADR-113 — Offline ontology condensation mesh + staleness-driven scheduler

**Status:** Accepted — retroactive record 2026-07-22. Split out of the ADR-112
Decision register (§5, row ADR-113) to document code that already ships. The
condensation pipeline lands in `agentbox/mcp/servers/lib/{ontology-index-build,ontology-condense}.js`
+ `agentbox/scripts/ontology-condense-refresh.sh`; the missing execution surface
(the scheduler) shipped 2026-07-22 as `agentbox/scripts/ontology-condense-scheduler.mjs`
(audit §2 C7 close).
**Date:** 2026-06-14 (decided under ADR-112) · recorded 2026-07-22
**Decision-type:** Agent Orchestration
**Relates:** ADR-112 (keystone), ADR-114 (memory substrate), ADR-116 (token budgets), PRD-020 WS-2

---

## 1. Context

ADR-112 §2.3 chose to compress the ~40M-word logseq corpus / ~14.7k `owl:Class`
records into ~100–150-token Class Summaries **offline**, never on a request path.
The design was a Sonnet-lead / Haiku-worker mesh. Two facts forced a retroactive
record separate from the keystone:

1. The shipped implementation **generalised the fixed Haiku mesh into a pluggable
   cheap-**_local_**-LLM step** (DiffusionGemma / vLLM / LM Studio / Ollama), so the
   as-built decision differs from the ADR-112 prose and needs its own governing note.
2. The pipeline **had no trigger**. `ontology-index-build.js` and
   `ontology-condense.js` existed and `ontology-condense-refresh.sh` orchestrated
   them, but nothing re-ran the refresh when GitHubSync/elevation rewrote the
   corpus. ADR-112 §2.3's "triggered incrementally on GitHubSync/elevation" was
   unwired — the PUSH Class-Summary cache and the `ns:ontology-classes` condensed
   store could silently go stale (audit §2 C7, the exact PRD-018 "wired ≠ working"
   trap).

## 2. Decision

### 2.1 Deterministic parse, then optional cheap-LLM condense, then cache fold
Three deterministic stages, orchestrated by `ontology-condense-refresh.sh`:
1. **index-build** — `ontology-index-build.js` parses the v2 `@type:Class`
   JSON-LD block out of every page into a compact class record. Fast, complete,
   no LLM.
2. **condense** — `ontology-condense.js` asks a cheap **operator-supplied local
   LLM** for one retrieval-optimised sentence + a synonym list per class, emitting
   an `ONTOLOGY_ALIASES` map + condensed text. Config comes from
   `[skills.ontology.condense]` in `agentbox.toml` (`ONTOLOGY_CONDENSE_ENABLED/
   _ENDPOINT/_MODEL/_STYLE/_N_BLOCKS/_CONCURRENCY`). Vanilla agentbox ships this
   **OFF**. Fail-soft per class: a class that errors keeps its deterministic terms
   so a partial run still improves the cache monotonically.
3. **index-build (re-run)** — folds the aliases into the PUSH Class-Summary
   cache that feeds the per-turn `[ONTOLOGY]` breadcrumb.

The condensed-text JSON (stage 2) is the payload stored into RuVector
`ns:ontology-classes` for semantic recall via the embedding pipeline.

### 2.2 DiffusionGemma serialisation constraint
The DiffusionGemma server holds a single model context and **serialises**
requests — `ONTOLOGY_CONDENSE_CONCURRENCY` MUST be 1, never fan out. Its
"thinking" can leak into `message.content` behind a `<|channel>thought` marker;
`stripThinking()` removes it. This constraint is why the shipped mesh is a serial
local-LLM pass, not the parallel Haiku fan-out of the ADR-112 sketch.

### 2.3 Staleness-driven scheduler (the missing trigger — C7)
`ontology-condense-scheduler.mjs` is the execution surface ADR-112 §2.3 assumed.
It is a **thin** wrapper — it does not reimplement condensation; it decides
*when* to invoke `ontology-condense-refresh.sh`. It follows the house pattern of
`scripts/ruvector-aggregate-sweep.mjs`:
- **Self-gating, default off** — runs iff both `ONTOLOGY_CONDENSE_ENABLED` and
  `ONTOLOGY_CONDENSE_SCHEDULE` are set (baked from `[skills.ontology.condense]`).
- **Staleness gate** — a tick rebuilds only when the newest logseq page mtime is
  later than the last condense output, the output is missing, or it is older than
  `ONTOLOGY_CONDENSE_SCHEDULE_MAX_AGE_HOURS` (default 24). A fresh index writes
  nothing: no LLM load, no cache churn. This is the GitHubSync/elevation trigger,
  realised as mtime staleness rather than an event hook.
- **Jittered** ±20% cadence (`ONTOLOGY_CONDENSE_SCHEDULE_INTERVAL_MINS`, default 60).
- **Locked + idempotent** — the refresh script is itself `flock`-serialised
  (mkdir fallback) and SKIPs (never fails) if another refresh holds the lock, so
  the scheduler, the entrypoint, and a manual operator run cannot overlap or race
  the shared `CLASSES/ALIASES/CONDENSED` outputs.
- **Fail-open** — an unreachable model / missing corpus / non-zero refresh exit
  is logged and retried next tick; nothing throws out of the loop.

## 3. Consequences

**Positive** — the "triggered incrementally on sync" claim is now honest; the
class index self-heals against corpus drift; the local-LLM generalisation removes
the hard Haiku/cloud dependency for operators running their own inference.

**Negative / costs** — the condense pass is a long serialised local-LLM run;
it is gated OFF by default and the scheduler is a no-op until an operator opts in.
**Activation requires the next image rebuild** to stage the supervisord program
+ flake env (the live-container detached self-loop is the interim path documented
in the scheduler header §SCHEDULING ARTEFACTS).

**Neutral** — default-off at ship; the capability is installed before delivered,
consistent with ADR-112 §4.

## 4. Status of the code (as-built, 2026-07-22)
- `ontology-index-build.js` — shipped (deterministic parser).
- `ontology-condense.js` — shipped (pluggable local-LLM condense, off by default).
- `ontology-condense-refresh.sh` — shipped, `flock`/mkdir-locked (C7).
- `ontology-condense-scheduler.mjs` — shipped 2026-07-22 (C7); staleness-driven,
  jittered, self-gating, fail-open. Supervisord/flake staging lands on next rebuild.
