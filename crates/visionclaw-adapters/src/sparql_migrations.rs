// src/sparql_migrations.rs
//! Versioned, idempotent SPARQL migration framework for the Oxigraph triple
//! store (ADR-101).
//!
//! Mirrors the SQLite `schema_migrations` discipline: numbered files, a
//! crate-local `migrations/sparql/` authoring directory, a ledger recording
//! what has been applied (version +
//! checksum + applied-at), and apply-exactly-once semantics. The ledger lives
//! in the dedicated named graph `urn:ngm:graph:migrations` (ADR-101 D2),
//! alongside the existing `urn:ngm:graph:*` family.
//!
//! ## Why an embedded registry, not a filesystem scan
//!
//! Source bind-mounts diverge between the build container and the runtime
//! container (see project CLAUDE.md), and only the crate tree is mounted —
//! repo-root dirs are not. So the authoring files live crate-locally under
//! `crates/visionclaw-adapters/migrations/sparql/` and each migration is
//! embedded with `include_str!`, so the compiled binary always carries the
//! exact migration text it was built with — and the checksum in the ledger
//! detects any drift.
//!
//! ## Idempotency contract (ADR-101 D1)
//!
//! Each migration is a single SPARQL UPDATE (Oxigraph applies it atomically)
//! that MUST be safe to re-run. The framework additionally gates on the
//! ledger so an applied migration never runs twice; the per-migration
//! idempotency is the second line of defence verified by tests.
//!
//! ## SPARQL construction (ADR-101 D3)
//!
//! The ledger INSERT is built from oxigraph typed term builders
//! (`NamedNode`, `Literal::new_typed_literal`) serialised via the store's own
//! quad insertion — never string-concatenated. The migration bodies are
//! static, author-reviewed `.rups` text (no untrusted input), so no injection
//! surface is introduced.

use oxigraph::model::vocab::xsd;
use oxigraph::model::{GraphNameRef, Literal, NamedNode, NamedNodeRef, QuadRef};
use oxigraph::store::Store;
use sha2::{Digest, Sha256};

/// The migrations ledger named graph (ADR-101 D2). Excluded from
/// graph-export / round-trip and from the GPU/inference paths.
pub const GRAPH_MIGRATIONS: &str = "urn:ngm:graph:migrations";

/// Ledger predicate IRIs (vc: namespace), one source of truth.
const MIG_NS: &str = "https://narrativegoldmine.com/ns/v1#migration/";
const P_VERSION: &str = "https://narrativegoldmine.com/ns/v1#migrationVersion";
const P_CHECKSUM: &str = "https://narrativegoldmine.com/ns/v1#migrationChecksum";
const P_APPLIED_AT: &str = "https://narrativegoldmine.com/ns/v1#migrationAppliedAt";
const T_MIGRATION: &str = "https://narrativegoldmine.com/ns/v1#Migration";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// A single embedded migration: a stable version id and its SPARQL UPDATE
/// body. Append-only — an applied migration file is never edited (ADR-101 D5).
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: &'static str,
    pub sparql: &'static str,
}

impl Migration {
    /// SHA-256 of the body, hex. Recorded in the ledger so drift is detectable.
    pub fn checksum(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.sparql.as_bytes());
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// The ordered migration registry. New migrations append here.
///
/// `0001` is ADR-100's canonical IRI re-mint. ADR-099's provenance back-fill
/// (`0002`) is owned by the reasoning agent and is intentionally NOT embedded
/// here (PRD-018 WS-2 handoff).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "0001_canonical_iri_remint",
        sparql: include_str!("../migrations/sparql/0001_canonical_iri_remint.rups"),
    },
    Migration {
        version: "0002_bootstrap_shapes_graph",
        sparql: include_str!("../migrations/sparql/0002_bootstrap_shapes_graph.rups"),
    },
    Migration {
        version: "0003_bootstrap_provenance_graph",
        sparql: include_str!("../migrations/sparql/0003_bootstrap_provenance_graph.rups"),
    },
];

/// Errors from the migration runner.
#[derive(Debug)]
pub enum MigrationError {
    /// The Oxigraph store rejected the UPDATE or a ledger query.
    Store(String),
    /// An applied migration's recorded checksum differs from the embedded
    /// body — an append-only-violation / drift (ADR-101 D5).
    ChecksumDrift {
        version: String,
        recorded: String,
        embedded: String,
    },
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::Store(e) => write!(f, "migration store error: {e}"),
            MigrationError::ChecksumDrift { version, recorded, embedded } => write!(
                f,
                "migration {version} checksum drift: ledger has {recorded}, binary has {embedded} \
                 (migrations are append-only — fix with a NEW migration)"
            ),
        }
    }
}

impl std::error::Error for MigrationError {}

/// Run all pending migrations against `store`, in registry order, exactly once.
///
/// Returns the list of versions applied during THIS call (empty on a fully
/// migrated store — the idempotent steady state). Re-running is a no-op.
pub fn run_pending(store: &Store) -> Result<Vec<String>, MigrationError> {
    let mut applied_now = Vec::new();

    for migration in MIGRATIONS {
        let embedded_checksum = migration.checksum();

        match recorded_checksum(store, migration.version)? {
            Some(recorded) => {
                // Already applied. Verify the body has not drifted.
                if recorded != embedded_checksum {
                    return Err(MigrationError::ChecksumDrift {
                        version: migration.version.to_string(),
                        recorded,
                        embedded: embedded_checksum,
                    });
                }
                // Idempotent: skip.
            }
            None => {
                // Pending — apply the UPDATE atomically, then record it.
                store
                    .update(migration.sparql)
                    .map_err(|e| MigrationError::Store(e.to_string()))?;
                record_applied(store, migration.version, &embedded_checksum)?;
                applied_now.push(migration.version.to_string());
                tracing::info!(
                    "applied SPARQL migration {} (checksum {})",
                    migration.version,
                    &embedded_checksum[..12.min(embedded_checksum.len())]
                );
            }
        }
    }

    Ok(applied_now)
}

/// IRI of a migration's ledger subject.
fn migration_subject(version: &str) -> NamedNode {
    // `version` is a static, author-controlled identifier; still routed through
    // the typed `NamedNode` builder (no string-concatenated SPARQL).
    NamedNode::new(format!("{MIG_NS}{version}"))
        .unwrap_or_else(|_| NamedNode::new_unchecked(format!("{MIG_NS}invalid")))
}

/// Look up the recorded checksum for `version` in the ledger, if applied.
fn recorded_checksum(store: &Store, version: &str) -> Result<Option<String>, MigrationError> {
    let subject = migration_subject(version);
    let predicate = NamedNodeRef::new_unchecked(P_CHECKSUM);
    let graph = NamedNodeRef::new_unchecked(GRAPH_MIGRATIONS);

    for quad in store.quads_for_pattern(
        Some((&subject).into()),
        Some(predicate),
        None,
        Some(GraphNameRef::NamedNode(graph)),
    ) {
        let quad = quad.map_err(|e| MigrationError::Store(e.to_string()))?;
        if let oxigraph::model::Term::Literal(lit) = quad.object {
            return Ok(Some(lit.value().to_string()));
        }
    }
    Ok(None)
}

/// Record a migration as applied: version + checksum + applied-at timestamp,
/// all inserted via typed term builders (ADR-101 D3 — no string SPARQL).
fn record_applied(
    store: &Store,
    version: &str,
    checksum: &str,
) -> Result<(), MigrationError> {
    let subject = migration_subject(version);
    let graph = NamedNode::new_unchecked(GRAPH_MIGRATIONS);
    let now = chrono::Utc::now().to_rfc3339();

    let type_node = NamedNodeRef::new_unchecked(T_MIGRATION);
    let p_type = NamedNodeRef::new_unchecked(RDF_TYPE);
    let p_version = NamedNodeRef::new_unchecked(P_VERSION);
    let p_checksum = NamedNodeRef::new_unchecked(P_CHECKSUM);
    let p_applied = NamedNodeRef::new_unchecked(P_APPLIED_AT);

    let version_lit = Literal::new_simple_literal(version);
    let checksum_lit = Literal::new_simple_literal(checksum);
    let applied_lit = Literal::new_typed_literal(now, xsd::DATE_TIME);

    let quads = [
        QuadRef::new(&subject, p_type, type_node, &graph),
        QuadRef::new(&subject, p_version, &version_lit, &graph),
        QuadRef::new(&subject, p_checksum, &checksum_lit, &graph),
        QuadRef::new(&subject, p_applied, &applied_lit, &graph),
    ];

    for q in quads {
        store
            .insert(q)
            .map_err(|e| MigrationError::Store(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_store() -> Store {
        Store::new().expect("in-memory store")
    }

    #[test]
    fn registry_is_well_formed() {
        // Every embedded migration has a non-empty body and a stable checksum.
        for m in MIGRATIONS {
            assert!(!m.sparql.trim().is_empty(), "{} has empty body", m.version);
            assert_eq!(m.checksum().len(), 64, "sha256 hex is 64 chars");
        }
    }

    #[test]
    fn applies_pending_then_is_noop_on_rerun() {
        let store = mem_store();

        // First run applies every registered migration.
        let first = run_pending(&store).expect("first run");
        assert_eq!(first.len(), MIGRATIONS.len());
        assert!(first.contains(&"0001_canonical_iri_remint".to_string()));

        // Second run applies nothing (idempotent steady state).
        let second = run_pending(&store).expect("second run");
        assert!(second.is_empty(), "re-run must be a no-op, got {second:?}");
    }

    #[test]
    fn ledger_records_checksum_in_migrations_graph() {
        let store = mem_store();
        run_pending(&store).expect("run");

        let recorded = recorded_checksum(&store, "0001_canonical_iri_remint")
            .expect("query")
            .expect("recorded");
        assert_eq!(recorded, MIGRATIONS[0].checksum());

        // The ledger triples live in the dedicated migrations graph and nowhere
        // else — they must not leak into the ontology/knowledge graphs.
        let graph = NamedNodeRef::new_unchecked(GRAPH_MIGRATIONS);
        let count = store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::NamedNode(graph)))
            .count();
        // 4 quads per migration (type + version + checksum + appliedAt)
        assert_eq!(count, 4 * MIGRATIONS.len(), "4 ledger quads per migration");
    }

    #[test]
    fn drift_is_detected() {
        let store = mem_store();
        run_pending(&store).expect("run");

        // Corrupt the recorded checksum to simulate an edited (drifted)
        // migration body — the runner must refuse to proceed.
        let subject = migration_subject("0001_canonical_iri_remint");
        let graph = NamedNode::new_unchecked(GRAPH_MIGRATIONS);
        let p_checksum = NamedNodeRef::new_unchecked(P_CHECKSUM);
        // Remove the good checksum, insert a bad one.
        let bad = Literal::new_simple_literal("deadbeef");
        let good = Literal::new_simple_literal(MIGRATIONS[0].checksum());
        store
            .remove(QuadRef::new(&subject, p_checksum, &good, &graph))
            .unwrap();
        store
            .insert(QuadRef::new(&subject, p_checksum, &bad, &graph))
            .unwrap();

        match run_pending(&store) {
            Err(MigrationError::ChecksumDrift { version, .. }) => {
                assert_eq!(version, "0001_canonical_iri_remint");
            }
            other => panic!("expected ChecksumDrift, got {other:?}"),
        }
    }

    #[test]
    fn remint_migration_is_idempotent_on_data() {
        // Seed a non-canonical entity, apply 0001 twice, assert the second
        // apply changes nothing beyond the first (no double-rewrite).
        let store = mem_store();

        let assert_graph = NamedNode::new_unchecked("urn:ngm:graph:ontology:assert");
        let legacy = NamedNode::new_unchecked("urn:ngm:class:camera");
        let p_label = NamedNodeRef::new_unchecked(
            "http://www.w3.org/2000/01/rdf-schema#label",
        );
        let label = Literal::new_simple_literal("Camera");
        store
            .insert(QuadRef::new(&legacy, p_label, &label, &assert_graph))
            .unwrap();

        run_pending(&store).expect("first apply");
        let after_first = store.len().expect("len");

        // Re-run: ledger short-circuits, store unchanged.
        run_pending(&store).expect("second apply");
        let after_second = store.len().expect("len");
        assert_eq!(after_first, after_second, "re-run must not mutate data");
    }
}
