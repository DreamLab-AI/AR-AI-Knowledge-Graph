# ADR-106: SPARQL PATCH for Ontology Mutations

> **Renumber note (2026-06-11):** previously filed as `ADR-028-sparql-patch-ontology`,
> which collided with `ADR-028-ext-optional-auth`. Renumbered to ADR-106 to enforce
> the one-number-one-decision uniqueness convention (see ADR-105 §Numbering and the
> ADR index in `README.md`). `ADR-028` now refers solely to optional-auth.

## Status
Accepted

## Context
JssOntologyService.ts provides read-only ontology access (JSON-LD fetch, Turtle content negotiation, WebSocket subscriptions). Ontology edits require full-document PUT, which is:

1. **Race-prone**: Two concurrent editors overwrite each other (last-write-wins).
2. **Bandwidth-heavy**: Sending the entire ontology document to change one triple.
3. **Non-atomic**: No way to express conditional updates (e.g., "only if this triple still exists").

The JSS sidecar already supports HTTP PATCH with two content types defined by the Solid Protocol:

- `application/sparql-update` -- SPARQL 1.1 Update for granular triple mutations (INSERT DATA, DELETE DATA, DELETE/INSERT WHERE).
- `text/n3` -- N3 Patch with `solid:where` clauses for optimistic concurrency control.

## Decision
Add PATCH-based mutation methods to JssOntologyService:

### Low-level PATCH methods
- `patchOntology(sparqlUpdate: string)` -- sends raw SPARQL Update via PATCH with `Content-Type: application/sparql-update`.
- `patchOntologyN3(n3Patch: string)` -- sends raw N3 Patch via PATCH with `Content-Type: text/n3` for conditional updates using `solid:where`.

### High-level triple helpers
- `addOntologyTriple(subject, predicate, object)` -- generates `INSERT DATA { s p o . }`.
- `removeOntologyTriple(subject, predicate, object)` -- generates `DELETE DATA { s p o . }`.
- `updateOntologyTriple(subject, predicate, oldValue, newValue)` -- generates atomic `DELETE { ... } INSERT { ... } WHERE { ... }`.

All methods reuse the existing `fetchWithAuth` flow (NIP-98 Nostr signing for authenticated mutations).

### RdfTerm type
A typed discriminated union (`iri | literal | prefixed`) prevents injection bugs by ensuring terms are serialized correctly with angle brackets, quotes, or bare prefixed names.

### Cache invalidation
Successful PATCH responses (2xx) invalidate the JSON-LD and Turtle caches, ensuring subsequent reads reflect mutations. WebSocket notifications from JSS provide the same invalidation path for other connected clients.

### Why SPARQL Update over N3 Patch as the default?
- SPARQL Update is more widely supported across Solid server implementations.
- N3 Patch adds optimistic concurrency (`solid:where`) but is only needed when conflict detection matters.
- Both are exposed; callers choose based on their concurrency requirements.

### Why not a full CRUD abstraction?
- The ontology schema is OWL -- class hierarchies, properties, restrictions. A generic CRUD layer would hide the RDF semantics that callers need to reason about.
- Triple-level helpers (add/remove/update) match the granularity of ontology edits without imposing a domain model.

## Consequences
- **Positive**: Granular mutations without full-document replacement.
- **Positive**: Optimistic concurrency via N3 Patch `solid:where` prevents silent overwrites.
- **Positive**: Bandwidth reduction -- a single triple change sends ~100 bytes instead of the full ontology.
- **Positive**: Atomic conditional updates via SPARQL DELETE/INSERT WHERE.
- **Negative**: Callers must construct valid SPARQL or N3 for low-level methods. Mitigated by the high-level triple helpers.
- **Negative**: Requires JSS sidecar with PATCH support (already available in current deployment).

## Related Decisions

- ADR-048: Dual-tier identity model — relies on SPARQL PATCH for ontology mutations triggered by approved migrations
