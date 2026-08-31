# ADR-127 — Semantic Trust Layer: SHACL shapes in Oxigraph, PROV-O as reified RDF, relay-mediated SPARQL federation

**Status:** Accepted (WS-0/WS-1/WS-2/WS-5 implemented; WS-3/WS-4 deferred to Phase 2)
**Date:** 2026-06-21
**Decision-type:** Architecture (keystone)
**Relates:** PRD-022 (parent), `docs/ddd-semantic-trust-layer-context.md` (bounded context), ADR-011 (auth enforcement / SERVICE block, `ontology_handler.rs:50-70`), ADR-075 (IS-Envelope), ADR-099 (Whelk reasoner), ADR-106 (SPARQL patch), ADR-112 (retrieval spine), ADR-124 (git-mark/block-trail build-out), PRD-020 (ontology augmentation), PRD-018 (ontosphere rigour — the silent-dead-wiring precedent), PRD-010 (mesh federation), agentbox ADR-005 (pluggable adapters), agentbox ADR-008 (privacy filter), agentbox ADR-012 (JSON-LD encoder), agentbox S05 (`s05-provenance.js`)

> Keystone ADR for the PRD-022 family. Sibling decisions (ADR-128 SHACL shape catalogue, ADR-129 provenance graph schema, ADR-130 federation event kinds) are summarised in the **Decision register** below and split into individual ADRs during implementation.

---

## 1. Context

The TrustGraph thesis identifies four pillars for trusted agent memory: formal ontology, SHACL validation, W3C provenance, and SPARQL federation. VisionFlow ships three natively (OWL 2 EL + Oxigraph SPARQL + cryptographic provenance deeper than RDF-only). Three specific gaps remain:

**Gap 1 — SHACL is lite, not W3C.** SHACL-lite exists as inline Rust shape checks (`shacl_lite.rs`, `shacl_gate.rs`) with advisory non-blocking gating. Per-type required-field rules are embedded in code, not external `.ttl` shape files. The `sh:` namespace is registered in `vocab_registry.rs` but no shapes are loaded into Oxigraph. The gate produces a `ShaclGateReport { violations, shapes_checked }` but never blocks. Full W3C SHACL was deferred to Phase 3 / Epic G (PRD-005).

**Gap 2 — PROV-O is URN-based, not reified.** Activity URNs are minted as `urn:visionclaw:execution:<sha256-12>` (`uris.js`, `provenance.rs`) and PROV-O is encoded in the S5 linked-data surface (`s05-provenance.js`), but provenance is not stored as RDF triples in Oxigraph. The `bc20.crossOutbound()` receipt crossing exists but is invoked only from the sovereign test tier, not production. An agent cannot SPARQL-query "who asserted this class, when, with what evidence" because the provenance lives in JSON fields (`owner_did`, `action_urn`), not in quads.

**Gap 3 — Federation is event-driven, not query-driven.** Each VisionClaw instance serves SPARQL. Cross-org federation works via Nostr relay mesh (event kinds 31400–31405, `did:nostr` identity, shared upper ontology). But `SERVICE` is blocked (`ontology_handler.rs:50-70`, ADR-011 S1) for SSRF prevention, and no query-driven federation mechanism exists. An agent on instance A cannot issue a semantic query that returns results from instance B.

PRD-018 is the governing cautionary precedent: ontology forces were compiled and "wired" but silently inert for months. Every path in this ADR must ship with a liveness proof.

---

## 2. Decision

### D1 — SHACL: W3C shapes in a dedicated named graph, dual-mode gate

**D1.1** Author W3C SHACL Core shapes as `.ttl` files in `crates/visionclaw-ontology/shapes/`. Five initial shapes target the core domain types: `OntologyClassShape`, `InferredAxiomShape`, `BridgeRecordShape`, `KnowledgeNodeShape`, `AgentNodeShape`.

**D1.2** Load shapes into a new Oxigraph named graph `urn:ngm:graph:shapes` via the existing SPARQL migration framework (`sparql_migrations.rs`). Migration is idempotent (content-addressed axiom IRIs, `urn:ngm:axiom:<sha256-12>`). Shapes are loaded at startup and on ontology sync.

**D1.3** Upgrade `ShaclGate` (`shacl_gate.rs`) from advisory-only to dual-mode:

- **Enforcing** (write paths — `propose`, `load`, `ingest`): shape violations produce a `ShaclViolationReport` domain event and **reject the payload**. The governed write path (`propose → Whelk → PR`) gains SHACL as a pre-Whelk gate: data must be shape-valid before it is consistency-checked.
- **Advisory** (read paths — `discover`, `read`, `sparql`): shape violations increment `shacl_violations_total{shape, severity}` and proceed. This catches ontology drift without blocking consumers.

**D1.4** Validation engine: SPARQL-ASK-based. Each shape compiles to a set of SPARQL ASK queries at startup. A violation is a query that returns `true` (positive match = constraint violation found). This avoids a full SHACL processor dependency. If `rudof` (the Rust SHACL ecosystem, crates `shacl-ast` + `shacl-validation`) reaches a stable 1.0 release, it replaces the ASK-based engine for richer constraint support (qualified cardinality, SPARQL-based constraints).

**D1.5** The existing SHACL-lite inline checks in `shacl_lite.rs` become the **fallback** if the shapes graph is unavailable (e.g., Oxigraph startup race). They are not removed — they provide defense-in-depth.

### D2 — PROV-O: reified triples in a provenance named graph

**D2.1** Create a new Oxigraph named graph `urn:ngm:graph:provenance`. This graph is **append-only**: the SPARQL migration framework and the handler-level validator both reject `DELETE`, `DROP`, `CLEAR`, and `WITH` operations targeting this graph.

**D2.2** Implement a `ProvenanceEmitter` (Rust, in `crates/visionclaw-adapters/`) that receives activity records from three sources:

- `provenance.rs` — ingest provenance classification (agent → VisionClaw boundary)
- `broker_inbox_handler.rs` — PROV-O URN minting for governed proposals
- `enrichment_proposals_handler.rs` — activity URN content-addressing for enrichment

Each activity record is reified as a set of quads using the W3C PROV-O vocabulary:

```turtle
<urn:visionclaw:execution:{sha256-12}> a prov:Activity ;
    prov:wasAssociatedWith <did:nostr:{hex-pubkey}> ;
    prov:startedAtTime "{iso-datetime}"^^xsd:dateTime ;
    prov:used <{source-iri}> ;
    prov:generated <{output-urn}> ;
    prov:wasInformedBy <{prior-decision-urn}> ;  # causal chain
    vc:action "{verb}" ;
    vc:derivation "{asserted|inferred|proposed}" .
```

**D2.3** Wire `bc20.crossOutbound()` into the production receipt path. When a receipt is minted in agentbox (`receipt-minter.js`), the BC20 bridge translates `urn:agentbox:activity:…` → `urn:visionclaw:execution:…` and the `ProvenanceEmitter` stores the reified triples. This closes the test-only gap.

**D2.4** Wire the agentbox S5 PROV-O surface (`s05-provenance.js`) into the production adapter dispatch middleware chain. S5 currently encodes PROV-O records but is opt-in and not invoked from hot paths. After this change, every durable write through an adapter slot (except `orchestrator`) emits a PROV-O activity record that flows to the `ProvenanceEmitter` via the BC20 bridge.

**D2.5** The `:provenance` graph does not participate in Whelk reasoning and is not covered by ontology CUDA forces. It is a side-graph for auditability, not a reasoning input.

### D3 — Federation: relay-mediated SPARQL queries over did:nostr mesh *(Deferred to Phase 2)*

> **Deferral note (2026-06-21):** WS-3 (relay-mediated SPARQL federation) and the `ontology_federate` MCP tool are deferred to Phase 2. The design below is retained for implementation reference. D1 (SHACL) and D2 (PROV-O) ship in Phase 1 and provide standalone value without federation.

**D3.1** `SERVICE` remains blocked. This decision does not revisit ADR-011 S1. The SSRF vector is real: a malicious SPARQL query with `SERVICE <http://internal-service/...>` could exfiltrate data from the container network. The security boundary is permanent.

**D3.2** Define two new Nostr event kinds extending the ACSP range:

- **Kind 31406 (`SemanticQueryRequest`)**: Signed by the requesting instance's `did:nostr`. NIP-44 v2 encrypted to the target peer(s). Payload is an IS-Envelope (ADR-075) with `kind: "semantic_query"`. Body:
  ```json
  {
    "query": "SELECT ?c ?label WHERE { ?c a owl:Class ; rdfs:label ?label }",
    "result_format": "application/sparql-results+json",
    "ttl": 300,
    "budget": { "max_rows": 1000, "max_bytes": 65536 }
  }
  ```
  The query is validated by the same `validate_read_only_sparql()` function used locally. Mutations, SERVICE, and uncapped queries are rejected before relay transmission.

- **Kind 31407 (`SemanticQueryResult`)**: Signed by the responding instance. Payload:
  ```json
  {
    "request_id": "<kind-31406 event id>",
    "source_did": "did:nostr:<responder-hex-pubkey>",
    "results": { /* SPARQL JSON results */ },
    "provenance": {
      "graph_version": "<sha256 of :assert graph>",
      "timestamp": "2026-06-21T14:30:00Z"
    }
  }
  ```

**D3.3** Peer authorization: Each VisionClaw instance maintains a `federation.authorized_peers` list (hex pubkeys). Only queries from listed peers are executed. The list is configurable via environment (`FEDERATION_AUTHORIZED_PEERS`) or the agentbox TOML manifest (`[federation.peers]`). Default: empty (no federation).

**D3.4** Result merge: The requesting instance collects results from all responding peers (with a configurable timeout, default 30s). Results are merged by IRI deduplication — if two instances report the same class IRI, the local instance's version wins; conflicting properties from peers are retained with a `vc:source_did` annotation. The shared upper ontology (`urn:ngm:graph:ontology:assert`) serves as the semantic alignment substrate.

**D3.5** IS-Envelope extension: `semantic_query` is added as an 8th kind to the IS-Envelope contract (ADR-075). The body shape follows D3.2. The envelope travels gift-wrapped (NIP-59) over the relay mesh — the same transport used by existing ACSP events. No new transport.

---

## 3. Decision register (sibling ADRs)

| ADR | Title | Scope | Key decision |
|---|---|---|---|
| **ADR-128** | SHACL shape catalogue | VisionClaw | Five `.ttl` shape files; constraint types used; shape-to-type mapping |
| **ADR-129** | Provenance graph schema | VisionClaw + agentbox | `:provenance` graph triple patterns; append-only enforcement; archival policy |
| **ADR-130** | Federation event kinds 31406/31407 | Nostr relay mesh | IS-Envelope `semantic_query` body shape; peer auth model; merge strategy |

---

## 4. Consequences

### Positive

- **Closes the trust trinity.** After implementation, VisionFlow satisfies all four TrustGraph pillars (ontology, SHACL, PROV-O, federation) while retaining its differentiators (cryptographic provenance, GPU ontology reasoning, identity-authenticated federation).
- **SHACL shapes are auditable.** Shape files in `.ttl` are version-controlled, diffable, and human-readable. An external auditor can inspect exactly what constraints the system enforces.
- **Provenance becomes SPARQL-queryable.** An agent can ask "who asserted class X, when, with what evidence" and get a formal answer — not a JSON field, but a graph traversal.
- **Federation is secure by construction.** No arbitrary outbound SPARQL (SERVICE stays blocked). Peer authorization is explicit (allowlist, not implicit). Queries are signed and encrypted end-to-end.
- **Maximum reuse.** No new services, no new identity primitives, no new transport. Every component extends existing infrastructure: Oxigraph named graphs, SPARQL migrations, the Nostr relay mesh, the adapter middleware chain, the IS-Envelope contract.

### Negative

- **SPARQL-ASK validation has limited expressiveness.** Qualified cardinality constraints, SPARQL-based constraints, and SHACL-AF features are not supported by the ASK-based engine. This is acceptable for core domain shapes; complex shapes wait for `rudof` or a future SHACL processor.
- **Append-only `:provenance` grows without bound.** Requires archival policy (WS-2, 90-day TTL default). Tombstones retain addressability.
- **Federation adds relay latency.** Seconds-scale, not milliseconds. Not suitable for interactive queries. Documented as a constraint; async delivery via MCP streaming mitigates.
- **IS-Envelope gains an 8th kind.** All existing consumers must handle unknown kinds gracefully (they already do per ADR-075 D1).

### Neutral

- SHACL-lite remains as a fallback. Two validation paths coexist (shapes graph + inline checks). This is defense-in-depth, not duplication — they serve different failure modes.
- The `:provenance` graph is not reasoned over by Whelk. This is intentional: provenance is metadata, not ontological content.

---

## 5. Alternatives considered

### A1 — Full SHACL processor via `rudof` (Rust crate)

The `rudof` project provides `shacl-ast` and `shacl-validation` crates for SHACL processing in Rust. Adopting it would give full W3C SHACL support including SPARQL-based constraints.

**Rejected (for now):** `rudof` is pre-1.0 (as of 2026-06-21). Introducing a pre-stable dependency for a security-critical validation gate violates the project's stability posture. The SPARQL-ASK approach covers SHACL Core constraints for the five initial shapes. `rudof` becomes the preferred path when it stabilizes (tracked as open question PRD-022 §9.3).

### A2 — Unblock SPARQL SERVICE for federation

Re-enable `SERVICE` with an allowlist of trusted peer endpoints.

**Rejected:** Even with an allowlist, `SERVICE` exposes the Oxigraph process to network I/O during query execution. A malicious or slow peer endpoint can hang the query thread. The relay-mediated approach isolates federation I/O from the SPARQL engine: queries are dispatched asynchronously, and results are merged after collection. The SPARQL engine never opens an outbound connection. ADR-011 S1 stands.

### A3 — PROV-O via external triplestore (separate from Oxigraph)

Store provenance triples in a separate triplestore (e.g., Fuseki) to isolate audit data from ontological data.

**Rejected:** Adds an operational dependency (second triplestore process). VisionFlow's Oxigraph already supports named graphs for isolation. The `:provenance` graph is structurally separate from `:assert`/`:inferred` (not reasoned over, append-only, different query patterns). A named graph provides logical isolation without operational cost.

### A4 — GraphQL federation instead of SPARQL federation

Use GraphQL stitching/federation for cross-instance queries.

**Rejected:** GraphQL is not the native query language of the data (RDF). A GraphQL layer would require a translation from RDF to GraphQL schema, losing the formal semantics that are the entire point. SPARQL over RDF preserves the standard; the relay mediation handles the transport concern.

### A5 — SHACL at the IS-Envelope boundary (Nostr relay)

Validate IS-Envelopes against SHACL shapes at the relay, replacing JSON Schema.

**Rejected:** IS-Envelope is a message envelope, not domain data. JSON Schema (ADR-075) is the correct validation for message structure. SHACL validates the *ontological content* within the message body, not the message itself. The two concerns are different layers.

---

## 6. Verification

Each decision has a concrete liveness proof:

| Decision | Verification | File:line (target) |
|---|---|---|
| D1.1 (shapes exist) | `ls crates/visionclaw-ontology/shapes/*.ttl` returns 5 files | new directory |
| D1.2 (shapes loaded) | `SPARQL SELECT (COUNT(*) as ?n) FROM <urn:ngm:graph:shapes>` returns >0 | `oxigraph_ontology_repository.rs` |
| D1.3 (enforcing mode) | Write a class missing `rdfs:label` → 400 + `ShaclViolationReport` | `shacl_gate.rs` |
| D1.4 (ASK validation) | Known-bad test data triggers expected violations | integration test |
| D2.1 (provenance graph) | `SPARQL ASK FROM <urn:ngm:graph:provenance> { ?s a prov:Activity }` → true | `oxigraph_ontology_repository.rs` |
| D2.3 (bc20 production) | `bc20.crossOutbound()` invoked from receipt-minter production path | `bc20-provenance-bridge.js` |
| D2.4 (S5 wired) | Adapter dispatch on memory/pods slot emits PROV-O activity record | `s05-provenance.js` |
| D3.2 (kind 31406) | `ontology_federate` on instance A returns results including instance B classes | `ontology-bridge.js` |
| D3.3 (peer auth) | Query from unauthorized peer → ignored (no response) | relay consumer |
| D3.5 (IS-Envelope) | `semantic_query` passes IS-Envelope V1 schema validation | `is-envelope-v1.schema.json` |
