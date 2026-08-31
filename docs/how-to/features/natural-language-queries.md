---
title: Building Graph Queries
description: Build and run graph queries visually with the triple-pattern query builder — mark nodes as variables, run the pattern against the graph, and read the bindings back.
category: how-to
tags:
  - query
  - graph
  - visual-query-builder
  - sparql
updated-date: 2026-08-31
difficulty-level: intermediate
---

# Building Graph Queries

## What this covers

VisionClaw's graph store is [Oxigraph/SPARQL](../../adr/ADR-132-neo4j-removal-oxigraph-adoption.md),
not Neo4j — the old "type a sentence, get Cypher back" flow was removed by
ADR-132. Two query surfaces exist today:

1. **The visual query builder** (flagship, XR + desktop). You *mark* nodes on
   the live graph as variables, the visible edges between them become a
   **triple pattern**, and the pattern runs server-side against
   `POST /api/graph/query/pattern`. This is the recommended way to query and
   what the rest of this guide is about.
2. **The legacy `/api/nl-query/*` routes**, which still exist but only
   *translate* natural language into a query string using an LLM — they do not
   execute anything. See [The legacy nl-query routes](#the-legacy-nl-query-routes)
   at the end.

For the raw request/response schema, always defer to
[REST API Reference §POST /api/graph/query/pattern](../../reference/rest-api.md#post-apigraphquerypattern).

---

## The triple-pattern model

A query is a set of **triples**, each `{src, edgeType, tgt}`:

- `src` and `tgt` are either a concrete node id (a `u32`) or a variable of the
  form `"?v1"`, `"?v2"`, …
- `edgeType` is a concrete predicate string, or `"any"` for a wildcard edge.

The server evaluates the pattern over the graph and returns every combination
of node ids that satisfies **all** triples simultaneously — that combination is
a *binding*. Limits (verified in `src/handlers/api_handler/graph/mod.rs`,
`query_pattern` at `mod.rs:1451`): **max 16 triples, max 8 variables**, and a
`limit` on returned bindings (default 24).

---

## Building a pattern in XR

The query builder is the "Query" tab of the wand HUD
(`xr-client/scripts/hud.gd`, tab order at `hud.gd:155`), backed by the pure
state object `xr-client/scripts/query_builder.gd`.

1. **Mark nodes as variables.** Point the wand at a node and press the **Menu
   button (or A/X)** to open its node menu, then choose **Mark as ?vN**. The
   node recolours to a per-variable highlight (palette cycles through 8
   colours) and gains a `?vN` proximity badge. `query_builder.gd::mark()`
   assigns names `?v1`, `?v2`, … in the order you mark them; names are never
   reused within a query even if you unmark an earlier one.
2. **Let the edges form the pattern.** Every *visible edge between two marked
   nodes* becomes a triple. With **Edges: concrete** (the default) the edge
   contributes its real predicate; with **Edges: any** every pattern edge is
   the `"any"` wildcard. Toggle this from the radial menu.
3. **Watch the live count.** The Query tab shows a running summary of the
   pattern and a **live match count** (`hud.gd:374`, `_build_query_page`). This
   is a `countOnly` preview call to `/api/graph/query/pattern` on every pattern
   change, so you can tell whether a pattern is too broad before you run it.
4. **Run it.** Press **Execute** — the HUD emits `query_execute_pressed`, the
   pattern runs, and the view jumps to the results.
5. **Clear it.** **Clear** unmarks every variable and empties the pattern.

To mark a node as a variable you must first open its node menu (Menu / A / X);
see [Immersive Controls](immersive-controls.md) for the full wand cheat-sheet.

---

## Building a pattern on desktop

The desktop client mirrors the same two-step model over the same endpoints
(`client/src/api/graphExpandApi.ts`). You mark nodes as variables from the node
context menu, the client previews the count, and Execute posts the pattern. The
wire shape is identical to XR because both talk to
`POST /api/graph/query/pattern`.

---

## The wire, directly

If you want to script queries yourself, post the pattern:

```bash
curl -s -X POST http://localhost:4000/api/graph/query/pattern \
  -H 'Content-Type: application/json' \
  -d '{
        "triples": [
          { "src": "?person", "edgeType": "authored",   "tgt": "?doc" },
          { "src": "?doc",    "edgeType": "references",  "tgt": 42 }
        ],
        "limit": 24,
        "countOnly": false
      }' | jq .
```

Response:

```json
{
  "vars": ["?person", "?doc"],
  "bindingCount": 3,
  "truncated": false,
  "bindings": [
    { "?person": 17, "?doc": 88 },
    { "?person": 23, "?doc": 88 },
    { "?person": 17, "?doc": 91 }
  ]
}
```

Reading the result:

- `vars` — the variable names in the pattern, in a stable order.
- `bindingCount` — how many rows are returned (bounded by `limit`).
- `truncated` — `true` when more bindings exist than `limit` allowed; raise
  `limit` or tighten the pattern.
- `bindings` — one object per solution, mapping each `?vN` to a concrete node
  id. Coerce ids with `String()` before comparing — node ids are `u32`.

Set `"countOnly": true` to get just `bindingCount` (the preview the builder
uses) without materialising the rows.

---

## The legacy nl-query routes

The `/api/nl-query/*` routes remain mounted
(`src/handlers/natural_language_query_handler.rs`, wired in `src/main.rs`) but
they are a **translation aid only** — they call the LLM
(`src/services/natural_language_query_service.rs`) to *generate a query string*
and never touch the graph store. Note that although the response field is still
named `cypherQuery` for backwards compatibility, the service's system prompt now
asks the model to emit **SPARQL for Oxigraph**, not Cypher. Treat any output as a
draft to inspect, not something that ran:

- `POST /api/nl-query/translate` — natural language → query string(s).
- `POST /api/nl-query/explain` — explain a query string in plain language.
- `POST /api/nl-query/validate` — syntactic sanity check only.
- `GET  /api/nl-query/examples` — curated example queries.

Prefer the visual query builder for anything you actually want to run against
the graph.

---

## Related documentation

- [REST API Reference](../../reference/rest-api.md) — full schema for
  `/api/graph/query/pattern`, `/relations`, `/expand`, and `/fold`.
- [REST API Usage Guide](../rest-api-usage.md) — worked curl flows.
- [Immersive Controls](immersive-controls.md) — wand and desktop bindings for
  marking variables and running queries.
- [Intelligent Pathfinding Guide](intelligent-pathfinding.md) — GPU shortest
  paths, a complement to pattern queries.
- [ADR-132 — Neo4j removal, Oxigraph adoption](../../adr/ADR-132-neo4j-removal-oxigraph-adoption.md).
</content>
</invoke>
