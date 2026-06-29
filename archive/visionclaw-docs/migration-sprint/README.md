# Migration Sprint — radical-rollback → main HEAD

Author: anthropic@xrsystems.uk
Branch baseline: `41979d33e3cb5219cefb889d51bd36613704bc3b` (Sun 12 Apr 2026, "feat: ForceAtlas2 LinLog kernel, 5 layout engines, all P1 fixes")
Branch target  : `main@HEAD` (15 May 2026, post-freeze-fix series)
Delta          : 371 commits, ~2,565 files, ~401k insertions / 446k deletions across the whole tree

## Why this sprint exists

The codebase between 12 April and 15 May accumulated a freeze regression that
proved very expensive to debug live: nine rounds of Codex consultation, several
days of incremental fixes (broadcast rate, Comlink zero-copy, single-flight
frame, Zustand narrowing, MAX_EDGES, polling gate), and a tab that still freezes
on the 4,519-node knowledge graph after all of them landed.

Rather than continue debugging forward from a contaminated baseline, this sprint
treats `41979d33e` as a known-clean *intent* reference point and rebuilds the
feature set on top of it deliberately, with every capability classified, named,
and migrated through PRD + ADR + (where appropriate) DDD documentation.

The sprint is doc-driven, not run-driven. We do not attempt to compile or run
the rollback baseline. The sole input from the existing code is its *expressed
intent* — what feature it was trying to deliver — extracted from commits, file
contents, and prior architectural records. The sole output is a coherent
migration plan that a future implementation phase can execute against.

## Ground rules

1. **Intent over implementation.** The existing code on `main` is reference,
   not specification. Every feature we adopt must be justified by capability,
   not by code-already-existing. Code that exists but does not make logical
   sense gets flagged in the ADR, not waved through.

2. **No code wave-through.** If a commit on `main` introduces complexity that
   we cannot explain from first principles in the ADR, we do not bring it
   forward. We either replace it with a simpler design or defer it.

3. **Hexagonal preserved.** The ports/adapters architecture present in
   `src/ports/` and `src/adapters/` is the spine. Repositories, services,
   and actors continue to talk to traits, not concrete adapters. The
   persistence migration (Section 11) is enabled by this discipline.

4. **Three artefact types:**
   - **PRD** (capability spec) — mandatory per section. What this section
     delivers, why it matters, acceptance criteria, non-goals.
   - **ADR** (architectural decision) — mandatory per section. The choice
     adopted, options considered, rationale, rejected alternatives, risks.
   - **DDD** (domain model) — only for genuine bounded contexts: physics,
     ontology, bots/telemetry, persistence. Skipped where a section is
     primarily integration or infrastructure.

5. **Bugs flagged.** Sections include a "Bugs and Smells at the Reset Point"
   sub-section where the rollback baseline already has known-defective code.
   Migration must not preserve these bugs.

6. **agentbox out of scope as code.** The agentbox submodule exists on `main`
   at commit `372f6636` and is preserved there. On `radical-rollback` agentbox
   is gitignored — its working tree is external on this branch by design.
   We integrate *with* agentbox; we do not import its code.

7. **docs/ cherry-pick later.** The `docs/` tree changed substantially between
   baseline and main. This sprint generates new authoritative documentation
   under `docs/migration-sprint/`. Pre-existing docs on `main` are reference
   material to be cherry-picked selectively after the sprint lands.

## Section taxonomy

The 371-commit delta is decomposed into 12 sections. Each is a tractable unit
of work with its own PRD/ADR/(DDD) trio.

| #  | Section                              | DDD? | Risk class            |
|----|--------------------------------------|------|-----------------------|
| 01 | GPU Physics & Force Engines          | yes  | high (correctness)    |
| 02 | Binary Protocol & Broadcast          | no   | high (perf + protocol)|
| 03 | Client State & Workers               | no   | high (regression)     |
| 04 | Rendering Layer                      | no   | medium                |
| 05 | Settings & Control Panel             | no   | low                   |
| 06 | Auth & Security                      | no   | high (production)     |
| 07 | Bots & Agent Telemetry               | yes  | medium                |
| 08 | Ontology & Graph Data                | yes  | high (data model)     |
| 09 | Ecosystem Services & Launch          | no   | low (infra)           |
| 10 | External Integrations (agentbox/forum)| no   | low (integration spec)|
| 11 | Persistence Strategy Migration       | yes  | high (data store)     |
| 12 | XR Client (Godot + gdext)            | no   | medium                |

## Cross-cutting decisions adopted up-front

These are settled in this README rather than re-litigated in every section:

- **Persistence**: Neo4j replaced by Oxigraph (Rust-embedded SPARQL 1.1) for
  ontology + graph data, and SQLite for settings. Justification in ADR-11.
- **Reasoning**: `whelk-rs` retained for OWL EL reasoning. Inference results
  materialised back as RDF triples in Oxigraph.
- **Binary protocol**: V3 full-sync, no delta. Justification in ADR-02.
- **Broadcast cadence**: Server-pushed cadence governed by GPU settlement, not
  fixed FPS. Justification in ADR-02.
- **Auth posture**: Nostr-only, no `SETTINGS_AUTH_BYPASS` in production. The
  `?skipAuth=true` query parameter remains for dev-mode browser automation
  *only* and must be inert in production builds.
- **Client memory model**: SharedArrayBuffer where available; Comlink with
  explicit `transfer()` zero-copy fallback. Single-flight frame processing.
- **agentbox**: External integration via documented contracts; no code import.
- **docs/**: Cherry-picked selectively after sprint completion.

## Reading order

1. This README.
2. Section ADRs in numeric order (`01-gpu-physics/ADR-01.md` first). The PRD
   is companion reading; the DDD where present is reference detail.
3. The cross-cutting `ADR-PERSISTENCE.md` lives inside `11-persistence-migration/`
   and is referenced from `08-ontology-graph-data/` and `05-settings/`.

## Implementation phasing (post-sprint)

Once these documents are approved, implementation phases proceed in this order:

1. **Persistence ports** (Section 11) — write new Oxigraph + SQLite adapters
   behind existing trait surfaces. No app code changes. New adapters proven
   in tests before any switch-over.
2. **Ontology + KG data model** (Section 8) — adopt the OntologyClass rename,
   migration queries (replaced with SPARQL Update equivalents).
2.5. **Auth compile-gates** (Section 6, D1+D2 only) — land ADR-06 D1
   (`dev-auth` Cargo feature) and D2 (compile-time gating of
   `--allow-skip-auth` and any bypass surface). Section 6's remaining
   decisions D3-D11 ship in Phase 7. This split is mandated by ADR-06
   §Phasing: ADR-02 D8 depends on the `--allow-skip-auth` flag existing
   in the codebase as of Phase 3.
3. **Binary protocol + broadcast** (Section 2) — adopt V3 full-sync semantics
   and settlement-gated cadence at the server. Removes the freeze surface.
4. **Client state + workers** (Section 3) — Comlink zero-copy, narrow selectors,
   single-flight. Removes the second freeze surface.
5. **GPU physics** (Section 1) — bring forward layout engines and force kernels.
6. **Rendering** (Section 4) — node geometries, edges, labels, Environment.
7. **Settings (D3-D11 of Section 6 only — D1+D2 landed in Phase 2.5), Bots,
   Ecosystem, External, XR** in parallel.

## Glossary of overloaded terms

The sprint documents use several terms with multiple meanings depending
on section. At first use within any document, write the qualified form;
subsequent in-document uses may shorten if context is unambiguous.

| Term | Disambiguated forms |
|------|---------------------|
| snapshot | **position-frame snapshot** (ADR-02 D4 — V3 binary frame from `GraphStateActor::current_snapshot()`); **telemetry snapshot** (ADR-10 D1, DDD-07 — `SwarmSnapshot` JSON on agent reconnect); **persistence snapshot** (ADR-11 D4/D10 — operator backup); **position-triple snapshot** (DDD-11 — per-60 s `:hasX/Y/Z` Oxigraph write). |

## Future docs

When authoring new PRDs or ADRs in this tree, follow the canonical
heading shape established by PRD-12 (`F1..Fn` for functional
requirements, `A1..An` for acceptance criteria) and ADR-01..ADR-11
(`D1..Dn` for decisions, plus `Options considered`, `Risks`, `Rejected
from main`, `Bugs and smells at the reset point`). Templates land at
`docs/migration-sprint/_templates/PRD-template.md` and `ADR-template.md`.
Existing documents are not retrofitted — the residual inconsistency is
not load-bearing.

## Out of scope

- Agentbox internals (Section 10 specifies integration contracts only).
- `docs/` tree on main (cherry-pick later).
- `presentation/` (slide-deck markdown).
- `multi-agent-docker/skills/` (skill orchestration is a separate workstream).
- Compile / run of either baseline or migrated code. Build verification is the
  first task *after* this sprint lands.
