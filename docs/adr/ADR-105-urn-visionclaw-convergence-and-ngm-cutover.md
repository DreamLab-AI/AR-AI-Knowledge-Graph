# ADR-105 — `urn:visionclaw` Convergence and `urn:ngm` Cutover

| Field | Value |
|-------|-------|
| Status | Accepted (2026-06-11) |
| Supersedes | the implicit "legacy scheme, no rip-out yet" prose in `src/uri/mod.rs` header and ADR-063 §1 |
| Relates to | ADR-100 (ontology IRIs; keeps `urn:ngm:graph:*`), ADR-063 (URN-traced operations), ADR-077 P3 (canonical mint module) |
| Affected repos | `VisionClaw` (+ the `agentbox` BC20 boundary it federates with) |
| Authoritative code | `src/uri/mod.rs` (the converged minter) |

## Context

VisionClaw `main` carries **two** durable-identifier namespaces simultaneously:

1. **The converged `urn:visionclaw:<kind>` grammar** — a complete, typed,
   fail-closed minter shipped at **`src/uri/mod.rs`**. It mints 7 URN kinds
   (`concept`, `kg`, `bead`, `execution`, `group`, `room`, `avatar`) plus
   `did:nostr:<hex-pubkey>` for sovereign identity, with content-addressing
   (`sha256-12-<12 hex>`) and 64-char BIP-340 x-only hex pubkey scopes. It is the
   VisionClaw-side counterpart of the agentbox `urn:agentbox:*` grammar, and it is
   already wired into the live provenance/ingest paths
   (`src/handlers/enrichment_proposals_handler.rs`, `src/agent_events/provenance.rs`,
   `src/agent_events/ingest.rs`).

2. **The legacy `urn:ngm:*` scheme** — the pre-convergence persistence identifiers
   still live in ~20 sites across 5 source files. These are *not* incidental: they
   are the storage-IRI surface bound 1:1 to the Oxigraph named graphs
   (`urn:ngm:graph:knowledge`, `urn:ngm:graph:agent`, `urn:ngm:graph:ontology:*`)
   that **ADR-100 deliberately keeps unchanged**, plus the node/edge/domain IRIs
   that round-trip through SPARQL `FILTER (STRSTARTS(...))` clauses and the
   `iri_to_node_id` parser.

The `src/uri/mod.rs` header has admitted since it shipped that "VisionClaw `main`
still carries the legacy `urn:ngm:*` scheme, which is left intact here to coexist
(no rip-out yet)." The 2026-06-11 ADR-gap inventory flagged this as a **critical
unratified decision**: the convergence had no ADR, no cutover plan, and no statement
of what migrates versus what stays. This ADR ratifies the convergence and records the
cutover boundary.

> **Stale-citation correction.** ADR-063 §1 cites the minter as
> `src/uri/{mint.rs, parse.rs, kinds.rs}`. That layout never landed — the converged
> minter is a single module at **`src/uri/mod.rs`** (mint constructors, `parse`,
> `Kind`, BC20 ingest, and now `parse_dual` all colocated). All references should
> point at `src/uri/mod.rs`.

## Decision

### D1 — New durable identifiers mint as `urn:visionclaw` via `src/uri/mod.rs`

Every **new** durable identifier minted on the VisionClaw side is produced through
the typed constructors in `src/uri/mod.rs` (`concept`, `kg`, `bead`, `execution`,
`group_members`, `room`, `avatar`, `did_nostr`). Ad-hoc `format!("urn:ngm:…")` /
template-literal construction of **new** durable IDs is prohibited, mirroring the
`uris.js` mandate on the agentbox side. New mints are converged-only; no new
identifier is ever produced under the `urn:ngm:` prefix.

### D2 — Parsers dual-read BOTH namespaces (persisted `urn:ngm` keeps resolving)

The resolve/lookup path accepts **both** the converged grammar and legacy
`urn:ngm:*`. This is implemented as `src/uri/mod.rs::parse_dual`, which:

* delegates to the strict `parse` for `did:nostr:*` / `urn:visionclaw:*`, and
* returns `ParsedUri::LegacyNgm { sub }` for any `urn:ngm:<sub>` identifier, carried
  opaquely so it round-trips its string form without being re-minted under the
  converged grammar.

The strict `parse` is **unchanged** — it still rejects `urn:ngm:*`, because the mint
path must stay converged-only. `parse_dual` is the entry point for surfaces that
must resolve historically-persisted IDs. This guarantees that every already-stored
`urn:ngm:node:*`, `urn:ngm:edge:*`, and `urn:ngm:domain:*` identifier keeps
resolving after the cutover with zero data movement.

### D3 — `urn:ngm:graph:*` named graphs stay (per ADR-100)

The Oxigraph named-graph IRIs (`urn:ngm:graph:knowledge`, `urn:ngm:graph:agent`,
`urn:ngm:graph:ontology:inferred`, …) are **not** changed. ADR-100 scopes ontology
IRIs and explicitly leaves the `urn:ngm:graph:*` named graphs in place; this ADR
reaffirms that. They are persistence-layer dataset coordinates, not federation-facing
durable identifiers, and renaming them would require rewriting the entire stored
quad set with no external benefit.

### D4 — BC20 is the cross-namespace anti-corruption boundary

The `urn:agentbox:*` ↔ `urn:visionclaw:*` translation at the federation boundary is
governed by the **BC20 anti-corruption layer**. The executable specification is
agentbox `management-api/lib/bc20-provenance-bridge.js` (its `toVisionclaw` map); the
VisionClaw-side counterpart is `src/uri/mod.rs::cross_from_agentbox`. The closed kind
map is authoritative on both sides:

| agentbox kind | VisionClaw target | Notes |
|---|---|---|
| `agent` | `did:nostr:<pubkey>` | identity; structural round-trip |
| `activity` | `urn:visionclaw:execution:<sha256-12>` | unscoped; owner in `owner_did` |
| `thing` | `urn:visionclaw:kg:<pubkey>:<sha256-12>` | owner-scoped, content-addressed |
| `memory` | `urn:visionclaw:concept:<domain>:<slug>` | needs elevation `{domain,slug}`; absent on the hot path → recorded as crossing-without-translation (`None`) |
| _other_ | — | unmapped → `None` (closed-map discipline) |

`did:nostr:*` is already converged on both sides and passes through unchanged. The
BC20 boundary is the **only** cross-namespace importer; nothing else translates
between `urn:agentbox` and `urn:visionclaw`.

### D5 — Bulk rewrite of stored IDs is a deferred Phase-2 (OUT OF SCOPE here)

Rewriting the **already-persisted** `urn:ngm:node:*` / `urn:ngm:edge:*` /
`urn:ngm:domain:*` identifiers in the Oxigraph store into the converged grammar is an
explicit **Phase-2 data migration** and is **out of scope for this sprint**. It
requires: a converged kind for node/edge persistence IRIs (the current minter has no
`node`/`edge`/`domain` kind — `kg` is content-addressed by pubkey, not by the legacy
`u32` node id), a stop-the-world or online migration of the stored quad set, a
rewrite of every SPARQL `FILTER (STRSTARTS(STR(?s), "urn:ngm:edge:"))` and the
`iri_to_node_id` round-trip parser in lockstep, and a re-derivation of node ids under
the converged content-addressing scheme. None of that is attempted now. Dual-read
(D2) is precisely what makes deferring it safe: nothing breaks while the legacy IDs
remain in storage.

## Phase-2 backlog (sites left on `urn:ngm` deliberately)

These sites mint or round-trip `urn:ngm:*` and are **intentionally not converted** in
this sprint because each is structurally bound to the ADR-100-protected named-graph
storage and would desync stored quads from their FILTERs/parser if flipped in
isolation:

| Site | What it is | Why deferred |
|---|---|---|
| `src/adapters/oxigraph_graph_repository.rs::node_iri` (`urn:ngm:node:{id}`) | persistence node IRI mint | round-trips via `iri_to_node_id`; no converged `node` kind exists; tied to stored quads |
| `src/adapters/oxigraph_graph_repository.rs::edge_iri` + `remove_edge` (`urn:ngm:edge:*`) | persistence edge IRI mint | matched by 3 SPARQL `STRSTARTS` FILTERs; converting desyncs stored triples |
| `src/adapters/oxigraph_graph_repository.rs` SPARQL FILTERs (lines ~296/327/347) | `STRSTARTS(STR(?s), "urn:ngm:edge:")` | must move in lockstep with the edge-IRI mint |
| `src/adapters/oxigraph_graph_repository.rs::iri_to_node_id` (`strip_prefix("urn:ngm:node:")`) | node-IRI → `u32` round-trip | the inverse of `node_iri`; Phase-2 with it |
| `src/services/github_sync_service.rs::ensure_domain_roots` (`urn:ngm:domain:{slug}`) | ontology `owl_class_iri` mint | joined against ontology named-graph quads + `enrich_node_from_quads` ngm-prefix match; ADR-100 territory |
| `src/adapters/oxigraph_graph_repository.rs` / `src/handlers/ontology_handler.rs` `urn:ngm:graph:*` | named graphs | **stays permanently** per ADR-100/D3 (not a Phase-2 item — a permanent exclusion) |

## What was executed in this sprint (low-risk half)

* **Added `parse_dual` + `ParsedUri::LegacyNgm`** to `src/uri/mod.rs` (D2): the
  resolve path now accepts both schemes; strict `parse` stays converged-only so the
  mint path is unchanged and the existing ngm-rejection test still passes. New tests
  cover converged round-trip, legacy round-trip, empty-sub rejection, and
  foreign-namespace rejection.
* **Confirmed** the converged minter is already the mint path for the live
  provenance/ingest surfaces (`enrichment_proposals_handler`, `agent_events`).
* **Left** all persistence-IRI mints on `urn:ngm` (the Phase-2 table above), because
  converting any one in isolation desyncs the ADR-100-protected stored quad set from
  its FILTERs and round-trip parser.
* **Corrected** the stale ADR-063 `src/uri/{mint,parse,kinds}.rs` citation to the
  real `src/uri/mod.rs`.

## Numbering

This ADR also anchors the **one-number-one-decision** numbering convention recorded
in `docs/README.md`. ADR-105 is the next free number after the on-disk max (ADR-104).
The same sweep that ratified this convergence renumbered four colliding duplicate-pair
files to ADR-106…ADR-109 (see `docs/README.md` → "Numbering convention").

## Consequences

**Positive**

* The convergence is ratified; `src/uri/mod.rs` is the named authoritative minter.
* Persisted `urn:ngm` IDs keep resolving (dual-read) with zero data movement.
* The BC20 boundary and its closed kind map are recorded as the single
  cross-namespace anti-corruption spec.
* The migration is explicitly deferred and scoped, not silently dropped.

**Negative / residual**

* Two durable namespaces coexist until Phase-2; readers must call `parse_dual`
  (not the strict `parse`) when they may encounter persisted legacy IDs.
* `urn:ngm:graph:*` named graphs remain `ngm`-prefixed permanently (by ADR-100
  design, not by omission).

**Risks**

* A surface that calls strict `parse` where it should call `parse_dual` would fail to
  resolve a legacy ID. Mitigation: `parse_dual` is documented as the resolve-path
  entry point; the strict `parse` carries an explicit "rejects `urn:ngm:*`" contract
  and test.
