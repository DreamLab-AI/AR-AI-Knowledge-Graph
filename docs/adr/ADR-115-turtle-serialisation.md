# ADR-115 — Terse Turtle over SPARQL-Results JSON for ontology augmentation

**Status:** Accepted — retroactive record 2026-07-22. Split out of the ADR-112
Decision register (§5, row ADR-115) to document code that already ships:
`serialiseTurtle()` in `agentbox/mcp/servers/lib/ontology-retrieval.js:72-109`.
**Date:** 2026-06-14 (decided under ADR-112) · recorded 2026-07-22
**Decision-type:** Architecture
**Relates:** ADR-112 (keystone), ADR-116 (token budgets — the consumer of the
serialised width), PRD-020

---

## 1. Context

The retrieval brain returns a subgraph (seed classes + optional expand triples)
to a model context on every augmented call. SPARQL-Results JSON is the ergonomic
default a raw SPARQL endpoint emits, but it is verbose: per-binding `{"type","value"}`
objects, repeated variable names, and repeated full IRIs blow the token budget
that ADR-116 has to enforce. The keystone (ADR-112 §2.1) named "terse Turtle
serialisation" as one of the retrieval brain's owned responsibilities; this is
the split-out record of the concrete format decision.

## 2. Decision

Serialise the augmentation payload as **terse, prefix-once Turtle**, not
SPARQL-Results JSON. `serialiseTurtle(seeds, expandTriples)` in
`ontology-retrieval.js`:
- emits the shared `VC_PREFIXES` block **once** at the head (opt-out via
  `includePrefixes`), then never repeats a prefix;
- renders each seed as `<full-iri> a owl:Class ; rdfs:label "…" ; vc:sourceDomain
  "…" ; vc:maturity "…" ; vc:relations "…" .` — full IRIs in angle brackets so
  any scheme (`vc:#…`, `urn:ngm:…`, `urn:visionclaw:…`) is valid Turtle without a
  prefix-mismatch;
- caps `vc:relations` to the first 5 to bound width;
- attaches the Class Summary as a trailing `# …` comment line (human/model
  readable, zero triple-parse cost);
- appends expand triples in already-compact `s p o .` form.

Measured **2–9× token reduction** versus SPARQL-Results JSON for the same
subgraph (ADR-112 §5 register). The one-line PUSH breadcrumb (`breadcrumb()`,
`ontology-retrieval.js:104-115`) is a further compression of the same data — a
pointer (`[ONTOLOGY] seed: vc:… → expand via ontology_ask`), not a payload.

## 3. Consequences

**Positive** — the serialised width that ADR-116's budget governor has to clamp
is 2–9× smaller before clamping, so more of the actual subgraph survives inside a
fixed tier budget. Turtle is directly model-legible and needs no client-side
result-set reshaping.

**Negative** — Turtle is a lossy projection of the full SPARQL result (types,
datatypes, language tags collapse to string labels); acceptable because the
augmentation path binds structure (IRIs, `subClassOf`/`enables`/`requires`,
domain, maturity) and fetches full prose only on explicit `full:true` (ADR-116).

**Neutral** — the serialiser is pure/synchronous and shared in-process by every
channel, consistent with the ADR-112 §2.1 "one brain, imported not served" form.
