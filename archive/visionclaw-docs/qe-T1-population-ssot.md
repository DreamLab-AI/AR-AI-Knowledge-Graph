# QE-T1 — Node Population Has No Single Source of Truth

> **✅ SUPERSEDED — FIX LANDED 2026-06-03.** This is a frozen pre-fix reproduction
> report, retained for historical context. Population now has a single source of
> truth: `metadata["type"]`, read via `Node::population_type()` / `Node::population()`
> (`crates/visionclaw-domain/src/models/node.rs:271,283`) and consumed uniformly by
> `graph_state_actor.rs:242,288` and the GPU path. **Note:** the fix chose the
> OPPOSITE field from this report's §6 fix-spec — `metadata["type"]` is authoritative,
> NOT `node_type`. `ontology_enrichment_service` no longer rewrites `node_type`, so
> the two fields can no longer desync. Authoritative current model:
> `02-population-handoff.md §7` and `00-anomaly-register.md`. The §2/§4 truth tables
> below describe pre-fix divergent behaviour that no longer occurs.

**Anomaly**: T1 (cartography audit #1). **Status**: RESOLVED (was: REPRODUCED, defect confirmed).
**Author**: agentic-QE investigator. **Date**: 2026-06-03.
**Scope**: reproduction evidence + failing regression test + fix spec. **No production code changed *by this report*.**

---

## 1. Summary

A node's "population" — which of the three dual-graph discs (Knowledge / Ontology /
Agent) it belongs to — is decided independently by four subsystems that read
**different, desynchronisable fields**. Two of those fields (`metadata["type"]` and
`node_type`, the latter serialised to the client as top-level `type`) are written by
different code paths and are **not kept in sync** when a page is elevated to an
ontology class. There is no authoritative field and no invariant enforcing agreement.

**Net effect for an enriched node**: the GPU projects it onto the **Knowledge** disc
(reads `metadata["type"]="linked_page"`) while the client classifies it as
**Ontology** (reads top-level `type="ontology_node"`). Same node, contradictory
placement and styling.

---

## 2. Live Evidence (read-only GET, recomputed)

Source: `GET http://visionclaw_container:4000/api/graph/data`
(`Authorization: Bearer dev-session-token`). Read-only; no writes, no analytics
endpoints triggered. Analysed in Python.

```
total_nodes        10676
metadata["type"]   present on 10676/10676
top-level type     present on 10676/10676   (serde rename of node_type)

DIVERGENT          2505  (23.5% of all nodes)
```

Divergence by raw value pair (`metadata["type"]` → top-level `type`):

| metadata["type"] | top-level `type` (= node_type) | count | population skew |
|---|---|---|---|
| `linked_page` | `ontology_node` | **1871** | Knowledge → Ontology |
| `linked_page` | `owl_class`     | **628**  | Knowledge → Ontology |
| `ontology_node` | `page`        | **5**    | Ontology → Knowledge (reverse) |
| `page` | `ontology_node`        | **1**    | Knowledge → Ontology (reverse) |

By population pair: `knowledge → ontology` = 2500, `ontology → knowledge` = 5.

The audit's figure of 2551/23.9% (1871 + 628 = 2499) matches within live-data drift.
The **6 reverse-skew nodes** are extra evidence: the desync is **bidirectional**,
which is only possible if neither field is canonical — confirming there is genuinely
no SSOT, not merely a one-directional write bug.

---

## 3. Writer / Reader Map (exact file:line)

### Writers of the two fields

| Field | file:line | What it writes | Updates the OTHER field? |
|---|---|---|---|
| `metadata["type"]="page"` | `src/services/parsers/knowledge_graph_parser.rs:109` | authored page stub | — |
| `node_type` (page **or** `ontology_node`) | `src/services/parsers/knowledge_graph_parser.rs:148-152` | sets `node_type="ontology_node"` when `owl_class_iri` present, else `"page"` | **NO** — `metadata["type"]` stays `"page"` from :109 |
| `metadata["type"]="linked_page"` | `src/services/parsers/knowledge_graph_parser.rs:258` | wikilink stub | sets node_type too (:279) — coherent at creation |
| `node_type="linked_page"` | `src/services/parsers/knowledge_graph_parser.rs:279` | wikilink stub | coherent with :258 at creation |
| `node_type="ontology_node"` | `src/services/ontology_enrichment_service.rs:240` | enrichment elevation | **NO** — `metadata["type"]` left as `"linked_page"`/`"page"` |

The desync is introduced at `knowledge_graph_parser.rs:152` and
`ontology_enrichment_service.rs:240`: both elevate `node_type` to an ontology class
**without touching `metadata["type"]`**.

### Readers — which field each subsystem trusts

| Subsystem | file:line | Field read | Notes |
|---|---|---|---|
| GPU disc projection | `src/actors/gpu/force_compute_actor.rs:607` | `metadata["type"]` first, else `node_type` | copy 1 of classifier |
| GraphStateActor `classify_node` | `src/actors/graph_state_actor.rs:239` | `metadata["type"]` first, else `node_type` | copy 2 of classifier (id-sets) |
| GraphStateActor `reclassify_all_nodes` | `src/actors/graph_state_actor.rs:284` | `metadata["type"]` first, else `node_type` | copy 3 of classifier (id-sets) |
| Server filter gate (`include_linked_pages`) | `src/actors/client_filter.rs:43` (also :24) | `node_type` only | reads elevated value |
| Client visual mode | `client/src/features/graph/hooks/useGraphVisualState.ts:146` | top-level `node.type` (= `node_type`) | reads elevated value |
| Client colour | `client/src/features/graph/components/GemNodes.tsx:462,473` | `node.metadata?.type` | reads non-elevated value |
| Client filter gate | `client/src/features/graph/hooks/useGraphFiltering.ts:97` | top-level `node.type` (= `node_type`) first | reads elevated value |

Three identical Rust copies of the "metadata first, else node_type" classifier
(force_compute:607, graph_state:239, graph_state:284) means even the
metadata-trusting path is duplicated and must be kept in lockstep by hand.

---

## 4. Truth Table for a Divergent Node

Subject: the dominant live shape — `metadata["type"]="linked_page"`,
`node_type` / top-level `type` = `"ontology_node"`, `owl_class_iri=None` (1871 nodes).

| Subsystem | Field read | Value seen | Decision |
|---|---|---|---|
| GPU disc projection (force_compute:607) | `metadata["type"]` | `linked_page` | **Knowledge disc** (−Z) |
| Client visual mode (useGraphVisualState:146) | top-level `type` | `ontology_node` | **ontology** mode |
| Client colour (GemNodes:462,473) | `metadata["type"]` | `linked_page` | ontology branch but `isClass=false` → **ontology *instance* hue**; type-scheme path → **page/linked_page palette** |
| Server filter gate (client_filter:43) | `node_type` | `ontology_node` | **NOT gated** by `include_linked_pages` (no longer "linked_page") → always visible |
| Client filter gate (useGraphFiltering:97) | top-level `type` | `ontology_node` | **NOT gated** → always visible |

**Incoherence**: one node is simultaneously placed on the Knowledge disc, coloured as
an ontology instance, classified Ontology for visual mode, and treated as a
fully-authored page (never gated as a stub) by both filters. No two subsystems agree
on what it is.

For the 628 `linked_page → owl_class` nodes: identical except GemNodes colour reads
`metadata.type != "owl_class"` → `isClass=false` → instance hue, while the visual mode
still resolves Ontology. The reverse-skew nodes (`ontology_node → page`, etc.) flip
GPU→Ontology / client→Knowledge — proof the failure is symmetric.

---

## 5. Failing Regression Test

**File**: `tests/qe_T1_population_ssot_repro_test.rs` (new; integration test in the
`visionclaw-server` test target; depends only on `visionclaw_server::models::node::Node`
— no GPU, no actor system, no network, so it compiles and runs without a full
backend/GPU build).

It replicates the **exact** production match arms (force_compute:609-621,
graph_state:241-256/:290-305) in `population_of()`, then models the two reader
strategies: `gpu_population()` (metadata-first, force_compute:607) and
`client_population()` (node_type / top-level type, useGraphVisualState:146 +
client_filter:43).

**Tests and the assertions they make:**

1. `repro_t1_gpu_and_client_must_agree_on_population_elevated_linked_page`
   — `assert_eq!(gpu_population(node), client_population(node))` for the 1871-node
   shape. **Fails now**: `Knowledge` (metadata=`linked_page`) ≠ `Ontology`
   (node_type=`ontology_node`).
2. `repro_t1_gpu_and_client_must_agree_on_population_elevated_owl_class`
   — same assertion for the 628-node `owl_class` tier. **Fails now**:
   `Knowledge` ≠ `Ontology`.
3. `repro_t1_metadata_type_and_node_type_must_imply_same_population`
   — structural: for each of the 4 live divergent shapes, asserts
   `population_of(metadata_type) == population_of(node_type)`. **Fails now**: all 4
   shapes diverge.
4. `control_coherent_node_agrees_and_passes` — a node with both fields = `page`
   classifies identically and lands on Knowledge. **Passes now and after the fix** —
   guards against a degenerate "make everything one population" fix.

**Why it fails today**: the test encodes the invariant *population(metadata["type"]) ==
population(node_type)*. On current `main`, ontology enrichment elevates `node_type`
without updating `metadata["type"]`, so the two fields imply different populations for
23.5% of nodes. The assertion is therefore violated for every elevated node. Verified
the pass/fail logic deterministically (mirror computation): tests 1–3 FAIL, control
and test 4 PASS.

> Marked clearly in the module header as a REPRODUCTION TEST that is EXPECTED TO FAIL
> until the SSOT fix lands. It must not be "fixed" by editing the test.

A client-side unit test is **not** added: the Rust test already pins the cross-subsystem
contract at the data-model level (the field divergence is upstream of the client, in
the wire payload), which is the most direct and build-light place to fix the
invariant. The client readers are documented in the truth table as downstream
consequences.

---

## 6. SSOT Fix Specification (for the fix agent — DO NOT implement here)

Make **`node_type`** the single authoritative population field (it already carries the
elevated/canonical class; the client's top-level `type` is its serde alias; it is the
only field every elevation path writes). The fix must guarantee:

1. **Single classifier**: collapse the three duplicated classifier copies
   (force_compute_actor.rs:607, graph_state_actor.rs:239, :284) into one shared helper
   (e.g. on `visionclaw_domain::models::node::Node`, returning the population), and have
   all three call sites — plus the GPU path — read **only `node_type`** (with
   `owl_class_iri` as the documented secondary signal). Drop the `metadata["type"]`-first
   precedence everywhere.
2. **Writer coherence**: any path that sets `node_type`
   (knowledge_graph_parser.rs:148-152, ontology_enrichment_service.rs:240) is now the
   sole population writer; `metadata["type"]` becomes non-authoritative provenance/origin
   metadata (ABox/TBox origin), explicitly documented as **not** a classification input.
   Alternatively, if `metadata["type"]` must remain authoritative for origin, the
   enrichment writers must update **both** fields atomically — but the single-field
   approach is preferred to eliminate the desync class entirely.
3. **Reader alignment**: `GemNodes.tsx:462,473` (client colour) must switch from
   `node.metadata?.type` to the top-level `node.type` so colour, visual mode, and both
   filter gates all read `node_type`. `useGraphVisualState.ts:146`, `client_filter.rs:43`,
   `useGraphFiltering.ts:97` already read it — verify no remaining `metadata["type"]`
   reads for classification/population/colour.
4. **Invariant holds**: after the fix, every node satisfies
   `population(authoritative field) ==` the value used by every subsystem, so
   `tests/qe_T1_population_ssot_repro_test.rs` passes (because `gpu_population` and
   `client_population` both resolve from `node_type`), and the control test still passes.

When the fix lands, this test flips from red to green with no edit to the test itself —
that flip is the acceptance signal for anomaly T1.
