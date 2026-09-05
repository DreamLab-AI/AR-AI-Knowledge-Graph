---
id: ADR-2064
title: The ontology loader walks the real authored corpus, not a hardcoded sample
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a change to the vault path authority in docs/VAULT-corpus-format.md Invariant 3, or to the OntologyBlock body dialect the parser reads
repo: visionclaw
domain: VAULT-corpus-format
---

# ADR-2064 — The ontology loader walks the real authored corpus, not a hardcoded sample

## Context
- `src/bin/load_ontology.rs` built five `OwlClass` literals (`mv:Person` … `mv:Technology`) and wrote
  them to Oxigraph. Its module doc claimed to load OWL from the corpus; it never opened a corpus file
  and never called `owl_extractor_service` or `ontology_pipeline_service` (Phase 1 diagram VC-20.1).
- Running it against a real store therefore seeded a five-class fiction, not the authored ontology.
- `src/services/owl_extractor_service.rs` — the service that pulls OWL Functional Syntax blocks out of
  markdown — had **no `mod` declaration in `src/services/mod.rs`**, so it had never been compiled.
- Wiring it up surfaced that it referenced `AnnotatedOntology` and
  `horned_functional::io::reader::read`, an API that cannot resolve here: `horned-functional` 0.4.0
  binds **horned-owl 0.11.0** while the crate pins **horned-owl 1.2.0** directly (`Cargo.lock` carries
  both), and the two ontology models are incompatible.
- It also carried two latent bugs that had never been compiled: a moved-value use in `parse_owl_blocks`
  and a `flat_map`/`sum` type error in `count_axioms`.

## Decision
`load_ontology` walks the **real authored corpus**. The root resolves the way the sibling bins resolve
theirs — an optional CLI path argument overrides everything, else `VAULT_ROOT`'s `pages/` subdirectory
(the vault path authority, `docs/VAULT-corpus-format.md` Invariant 3), else the container default
`/app/data/pages` that `LocalFileSyncService` also uses. `DATA_DIR` resolves the Oxigraph store exactly
as in `sync_github`/`sync_local`. Every `.md` file under the root is parsed with the real
`OntologyParser`, and files carrying an `### OntologyBlock` section are persisted through
`OntologyRepository::save_ontology` — the same write path `LocalFileSyncService` uses. `OwlExtractorService`
then runs over the freshly persisted classes. The bin reports per-stage counts and **exits non-zero**
when the corpus path is absent or nothing was extracted, so a silent no-op cannot look like a success.

No ontology content is hardcoded in the binary.

`parse_with_horned_owl` and `build_complete_ontology` are **removed** rather than repaired: they had
zero callers tree-wide, and they could not compile against the pinned dependency graph. Functional-syntax
parsing, if wanted later, belongs on horned-owl 1.2.0's own `io::ofn` reader — not on a second, older
ontology model pulled in transitively.

## Consequences
- Running the loader now changes real data: it writes the authored ontology into Oxigraph. That is the
  point, but it makes the bin destructive-by-intent in a way the sample version was not, hence the
  explicit non-zero exits and the per-stage counts.
- `owl_extractor_service` is compiled for the first time, so its two latent bugs are fixed and it is
  now covered by `cargo check`.
- The horned-owl version conflict is documented at the removal site but **not resolved**: the tree still
  resolves both 0.11.0 and 1.2.0. Any future functional-syntax work must first decide whether to drop
  `horned-functional` or to move the crate to a horned-functional release built against horned-owl 1.x.
- Diagram VC-20.1 no longer describes a sample loader.

## Verification
Ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; must be re-run at
the landing commit.

```
$ cargo check -p visionclaw-server --bin load_ontology
    Finished `dev` profile [optimized + debuginfo] target(s) in 53.34s
# (exit 0; the only notes are the pre-existing quick-xml future-incompat warnings)

$ cargo check -p visionclaw-server --lib
# exit 0, and zero warnings attributed to src/bin/load_ontology.rs or
# src/services/owl_extractor_service.rs

$ grep -n "OwlClass {" src/bin/load_ontology.rs
# (no matches — the five hardcoded class literals are gone)

$ grep -n "VAULT_ROOT\|DATA_DIR\|std::env::args" src/bin/load_ontology.rs
49:    if let Ok(vault_root) = std::env::var("VAULT_ROOT") {
102:    let cli_override = std::env::args().nth(1);
116:    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());

$ grep -n "owl_extractor_service" src/services/mod.rs
22:pub mod owl_extractor_service;      # newly declared — the module now compiles

$ grep -rn "build_complete_ontology\|parse_with_horned_owl" src/ crates/ --include=*.rs
# (no matches outside the removal comment — both had zero callers)
```

Not verified: no run against the real 2026-09-02 corpus was performed in this environment, so the
extracted class/axiom counts are unmeasured. The bin's non-zero exit on an empty extraction is the
guard against that going unnoticed.
