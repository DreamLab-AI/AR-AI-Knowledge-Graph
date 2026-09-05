---
id: ADR-2095
title: Mint urn:ngm:class through a typed constructor in the domain crate
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b0bc275f6501aae7751b85a72ce15fe1e730e7e8
verified_paths: []
owner: jjohare
review_trigger: the legacy `urn:ngm:*` scheme is retired in favour of `urn:visionclaw:concept:*`, or a fourth crate needs to mint a class IRI
repo: visionclaw
---

# ADR-2095 — Mint `urn:ngm:class` through a typed constructor in the domain crate

## Context

ADR-2021 routed the legacy `urn:ngm:*` mint sites through typed constructors paired with parsers, and
IDENTIFIER-taxonomy makes that an invariant: every durable identifier is minted through the typed
module, never by `format!`. The class scheme was left out. `uri::ngm` carried `node_iri`/`edge_iri`
but no class constructor, so five sites formatted `urn:ngm:class:<slug>` by hand — two in
`src/actors/elevation_actor.rs` (the ACSP case subject and the drafted corpus page's `@id`) and three
in `crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs` (class minting and two node→IRI
fallbacks in `save_ontology_graph`). Untyped and unpaired with a parser, they were free to drift from
each other and from the recognisers that consume the scheme.

Placement is forced by the dependency graph: `visionclaw-server` depends on `visionclaw-adapters`,
not the reverse, so a constructor defined only in `src/uri/mod.rs` is unreachable from the adapter —
the crate that mints most of these IRIs.

## Decision

The legacy class scheme is defined once, in `crates/visionclaw-domain/src/uri.rs` as
`visionclaw_domain::uri::ngm`: `CLASS_PREFIX`, an infallible `class_iri(slug)`, the paired
`parse_class_iri` (which rejects the empty slug), and `is_class_iri`. The domain crate is the
innermost crate, so both the server and the adapters can reach it.

`src/uri/mod.rs`'s `ngm` module re-exports all four, so `uri::ngm::*` remains the single lookup point
for the legacy scheme alongside `node_iri`/`edge_iri`. Every mint site calls the constructor; no
`format!("urn:ngm:class:…")` remains in `src/` or `crates/`.

`class_iri` is infallible and never rewrites its argument, mirroring `node_iri`: callers slugify
first, and the constructor reproduces the previously emitted string byte-for-byte. Validation lives
in the parser, which is where it can reject without silently changing a persisted identifier.

## Consequences

Persisted class IRIs are unchanged — this is a refactor of the mint path, not a migration. A future
change to the scheme now has one place to make it and a parser that must be changed with it.

The `ngm` module is split across two crates: definition in `visionclaw-domain`, re-export in
`src/uri/mod.rs`. Anything added to `uri::ngm` that a crate upstream of the server needs must be
defined in the domain crate and re-exported the same way.

Recognisers that match the prefix without minting are untouched and remain outside the invariant:
`jsonld_validator/iri.rs`'s scheme table, `ONTOLOGY_ROOT_IRI_NGM`, and the `sparql_migrations`
remint. Consolidating those is follow-on work, not covered here.

The `urn:ngm:property:<slug>` and `urn:ngm:axiom:<sha256-12>` schemes in the same adapter are still
minted by `format!`. This ADR closes the class scheme only; the same treatment for the sibling
schemes is outstanding.

## Verification

At `verified_commit` plus this change, on the working tree:

- `grep -rn 'urn:ngm:class' src crates --include=*.rs` — remaining hits are the typed module, tests,
  doc comments, and prefix recognisers. `grep -rn 'format!("urn:ngm:class' src crates --include=*.rs`
  matches only the literal-preservation test in `crates/visionclaw-domain/src/uri.rs`.
- `cargo check --workspace --all-targets` — exit 0.
- `cargo test -p visionclaw-domain --lib uri::` — 4 passed, including
  `class_iri_reproduces_the_pre_typed_literal`.
- `cargo test -p visionclaw-server --lib uri::` — 43 passed;
  `--lib actors::elevation_actor` — 10 passed, including the pre-existing
  `draft_page_has_canonical_identity_and_draft_maturity`, which asserts the literal
  `"@id": "urn:ngm:class:finality-mechanism"` and is the proof the emitted string did not shift;
  `--lib elevation` — 47 passed.
- `cargo test -p visionclaw-adapters` — 76 passed.
- `cargo fmt --all --check` — exit 0. `cargo clippy -p visionclaw-domain -p visionclaw-adapters
  -p visionclaw-server --all-targets` — exit 0, no warning in a touched file.
