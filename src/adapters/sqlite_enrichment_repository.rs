// src/adapters/sqlite_enrichment_repository.rs
//! SQLite EnrichmentProposal Repository Adapter (WS-9).
//!
//! Durable lifecycle store for broker enrichment proposals + their governance
//! decisions, backing both the WS-9 decide path and the WS-12 broker inbox.
//!
//! ## Persistence choice
//!
//! `tokio-rusqlite`, mirroring [`crate::adapters::sqlite_settings_repository`]
//! verbatim. SQLite is the project's established embedded durable store
//! (ADR-11 §D1 single-binary), already in the deps (`Cargo.toml` rusqlite
//! bundled + tokio-rusqlite). Oxigraph was the alternative but is wrong for a
//! *mutable lifecycle* store (decide transitions, status filters, offset/limit
//! pagination); Oxigraph holds RDF/KG facts, and the KG *consequence* of an
//! approval is a separate fenced write to `:summary`/`:observed` performed via
//! [`crate::adapters::oxigraph_ontology_repository`]. Postgres rejected: the
//! project's own SQLite is canonical.
//!
//! Lives in its OWN file (`data/enrichment.sqlite3`), not `settings.sqlite3`,
//! so the single-writer constraint is isolated and migrations are independent.
//! Because it is a separate file, [`CREATE_SCHEMA`] self-bootstraps its own
//! `schema_migrations` table (settings.sqlite3's does not exist here).
//!
//! ## Single-writer note
//!
//! Like the settings adapter, SQLite is single-writer. `tokio-rusqlite`
//! serialises every `call` onto its own worker thread, so concurrent
//! `decide()` calls are safe. [`record_decision`](SqliteEnrichmentRepository::record_decision)
//! is one transaction (INSERT decision + UPDATE proposal) so the lifecycle
//! stays atomic — never split across two `call` closures.

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio_rusqlite::Connection;

/// Embedded canonical schema. Self-bootstrapping (single-binary deployments,
/// ADR-11 §D1). The on-disk file at `migrations/sqlite/0003_enrichment_proposals.sql`
/// is the human-authoring source; changes there must be mirrored here.
///
/// Unlike the settings DB, this file has no pre-existing `schema_migrations`
/// table — so it is created first.
pub const CREATE_SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous  = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA temp_store   = MEMORY;

CREATE TABLE IF NOT EXISTS schema_migrations (
    id          TEXT PRIMARY KEY,
    applied_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS enrichment_proposals (
    case_id        TEXT    PRIMARY KEY,
    category       TEXT,
    source_iri     TEXT,
    proposal_json  TEXT    NOT NULL,
    status         TEXT    NOT NULL DEFAULT 'pending',
    created_at     INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at     INTEGER NOT NULL DEFAULT (unixepoch())
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS enrichment_status_idx
    ON enrichment_proposals(status, updated_at);

CREATE TABLE IF NOT EXISTS enrichment_decisions (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    case_id              TEXT    NOT NULL,
    outcome              TEXT    NOT NULL,
    attributed           INTEGER NOT NULL,
    broker_pubkey        TEXT,
    reasoning            TEXT,
    writeback_triggered  INTEGER NOT NULL,
    writeback_committed  INTEGER NOT NULL DEFAULT 0,
    activity_urn         TEXT    NOT NULL,
    proposal_urn         TEXT,
    owner_did            TEXT,
    decided_at_ms        INTEGER NOT NULL,
    FOREIGN KEY(case_id) REFERENCES enrichment_proposals(case_id)
);

CREATE INDEX IF NOT EXISTS enrichment_decision_case_idx
    ON enrichment_decisions(case_id, decided_at_ms);

INSERT OR IGNORE INTO schema_migrations (id) VALUES ('0003_enrichment_proposals');
"#;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error)]
pub enum EnrichmentStoreError {
    #[error("db: {0}")]
    Database(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("json: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, EnrichmentStoreError>;

fn map_db_err(e: tokio_rusqlite::Error) -> EnrichmentStoreError {
    EnrichmentStoreError::Database(e.to_string())
}

fn map_json_err<E: std::fmt::Display>(e: E) -> EnrichmentStoreError {
    EnrichmentStoreError::Serialization(e.to_string())
}

/// Map a broker outcome string to the durable proposal status. Approvals (in
/// any spelling the broker uses) → `approved`; rejections → `rejected`;
/// everything else (amend/delegate/precedent/...) → `reviewed`. Mirrors the
/// broker-bridge status enum (pending|claimed|decided is the *coarse* enum the
/// bridge filters on; here we keep the fine-grained decided sub-state).
pub fn status_for_outcome(outcome: &str) -> &'static str {
    let o = outcome.trim().to_ascii_lowercase();
    match o.as_str() {
        "approve" | "approved" | "accept" | "accepted" | "promote" => "approved",
        s if s.starts_with("reject") => "rejected",
        _ => "reviewed",
    }
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// A durable enrichment proposal row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentProposal {
    pub case_id: String,
    pub category: Option<String>,
    pub source_iri: Option<String>,
    pub proposal_json: serde_json::Value,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A durable governance decision row. Mirrors the handler's `RecordedDecision`
/// fields plus the `writeback_committed` truth bit that the in-memory ghost
/// never had (it always claimed `triggered` regardless of whether any write
/// landed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDecision {
    pub case_id: String,
    pub outcome: String,
    pub attributed: bool,
    pub broker_pubkey: Option<String>,
    pub reasoning: Option<String>,
    pub writeback_triggered: bool,
    pub writeback_committed: bool,
    pub activity_urn: String,
    pub proposal_urn: Option<String>,
    pub owner_did: Option<String>,
    pub decided_at_ms: i64,
}

// ---------------------------------------------------------------------------
// Repository
// ---------------------------------------------------------------------------

/// SQLite-backed enrichment proposal + decision store. Holds one
/// `tokio-rusqlite` connection in `Arc` so it can be cheaply cloned into
/// handlers and `AppState`. See module docs for the single-writer posture.
pub struct SqliteEnrichmentRepository {
    conn: Arc<Connection>,
}

impl SqliteEnrichmentRepository {
    /// Open (or create) the enrichment DB at `db_path` and apply [`CREATE_SCHEMA`].
    pub async fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path).await.map_err(map_db_err)?;
        conn.call(|c| {
            c.execute_batch(CREATE_SCHEMA)?;
            Ok(())
        })
        .await
        .map_err(map_db_err)?;
        Ok(Self {
            conn: Arc::new(conn),
        })
    }

    /// Construct over an already-opened connection (tests).
    pub fn from_connection(conn: Arc<Connection>) -> Self {
        Self { conn }
    }

    /// Convenience accessor for tests.
    pub fn connection(&self) -> &Arc<Connection> {
        &self.conn
    }

    /// Insert or replace a proposal row (idempotent upsert keyed on `case_id`).
    /// `created_at` is preserved on replace via COALESCE against the prior row.
    pub async fn create_or_update(&self, p: &EnrichmentProposal) -> Result<()> {
        let case_id = p.case_id.clone();
        let category = p.category.clone();
        let source_iri = p.source_iri.clone();
        let proposal_json = serde_json::to_string(&p.proposal_json).map_err(map_json_err)?;
        let status = p.status.clone();
        self.conn
            .call(move |c| {
                let mut stmt = c.prepare_cached(
                    "INSERT INTO enrichment_proposals
                         (case_id, category, source_iri, proposal_json, status, created_at, updated_at)
                     VALUES
                         (?1, ?2, ?3, ?4, ?5, unixepoch(), unixepoch())
                     ON CONFLICT(case_id) DO UPDATE SET
                         category      = excluded.category,
                         source_iri    = excluded.source_iri,
                         proposal_json = excluded.proposal_json,
                         status        = excluded.status,
                         updated_at    = unixepoch()",
                )?;
                stmt.execute(rusqlite::params![
                    &case_id,
                    &category,
                    &source_iri,
                    &proposal_json,
                    &status,
                ])?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    /// Fetch one proposal by `case_id`.
    pub async fn get(&self, case_id: &str) -> Result<Option<EnrichmentProposal>> {
        let case_id_owned = case_id.to_string();
        let row: Option<(String, Option<String>, Option<String>, String, String, i64, i64)> = self
            .conn
            .call(move |c| {
                let mut stmt = c.prepare_cached(
                    "SELECT case_id, category, source_iri, proposal_json, status, created_at, updated_at
                     FROM enrichment_proposals WHERE case_id = ?1",
                )?;
                let mut rows = stmt.query(rusqlite::params![&case_id_owned])?;
                if let Some(r) = rows.next()? {
                    Ok(Some((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    )))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(map_db_err)?;

        match row {
            Some((case_id, category, source_iri, proposal_json, status, created_at, updated_at)) => {
                let proposal_json: serde_json::Value =
                    serde_json::from_str(&proposal_json).map_err(map_json_err)?;
                Ok(Some(EnrichmentProposal {
                    case_id,
                    category,
                    source_iri,
                    proposal_json,
                    status,
                    created_at,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// List proposals, optionally filtered by status, newest-updated first.
    /// Backs the WS-12 inbox (offset/limit pagination).
    pub async fn list(
        &self,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<EnrichmentProposal>> {
        let status_owned = status.map(|s| s.to_string());
        let rows: Vec<(String, Option<String>, Option<String>, String, String, i64, i64)> = self
            .conn
            .call(move |c| {
                // `?1 IS NULL OR status = ?1` collapses the optional filter into
                // one prepared statement.
                let mut stmt = c.prepare_cached(
                    "SELECT case_id, category, source_iri, proposal_json, status, created_at, updated_at
                     FROM enrichment_proposals
                     WHERE (?1 IS NULL OR status = ?1)
                     ORDER BY updated_at DESC
                     LIMIT ?2 OFFSET ?3",
                )?;
                let mut q = stmt.query(rusqlite::params![&status_owned, limit, offset])?;
                let mut out = Vec::new();
                while let Some(r) = q.next()? {
                    out.push((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ));
                }
                Ok(out)
            })
            .await
            .map_err(map_db_err)?;

        let mut out = Vec::with_capacity(rows.len());
        for (case_id, category, source_iri, proposal_json, status, created_at, updated_at) in rows {
            let proposal_json: serde_json::Value =
                serde_json::from_str(&proposal_json).map_err(map_json_err)?;
            out.push(EnrichmentProposal {
                case_id,
                category,
                source_iri,
                proposal_json,
                status,
                created_at,
                updated_at,
            });
        }
        Ok(out)
    }

    /// Count proposals, optionally filtered by status.
    pub async fn count(&self, status: Option<&str>) -> Result<i64> {
        let status_owned = status.map(|s| s.to_string());
        self.conn
            .call(move |c| {
                let mut stmt = c.prepare_cached(
                    "SELECT COUNT(*) FROM enrichment_proposals
                     WHERE (?1 IS NULL OR status = ?1)",
                )?;
                let mut rows = stmt.query(rusqlite::params![&status_owned])?;
                let n: i64 = if let Some(r) = rows.next()? {
                    r.get(0)?
                } else {
                    0
                };
                Ok(n)
            })
            .await
            .map_err(map_db_err)
    }

    /// Atomically record a decision and transition the parent proposal's status.
    /// ONE transaction — INSERT decision + UPDATE proposal — mirroring the
    /// `upsert_file_sha1s` tx pattern (sqlite_settings_repository.rs:296-310).
    /// Never split into two `call` closures (single-writer atomicity).
    pub async fn record_decision(&self, d: &StoredDecision) -> Result<i64> {
        let case_id = d.case_id.clone();
        let outcome = d.outcome.clone();
        let attributed = d.attributed as i64;
        let broker_pubkey = d.broker_pubkey.clone();
        let reasoning = d.reasoning.clone();
        let writeback_triggered = d.writeback_triggered as i64;
        let writeback_committed = d.writeback_committed as i64;
        let activity_urn = d.activity_urn.clone();
        let proposal_urn = d.proposal_urn.clone();
        let owner_did = d.owner_did.clone();
        let decided_at_ms = d.decided_at_ms;
        let status = status_for_outcome(&outcome).to_string();

        self.conn
            .call(move |c| {
                let tx = c.transaction()?;
                let decision_id: i64;
                {
                    let mut ins = tx.prepare_cached(
                        "INSERT INTO enrichment_decisions
                             (case_id, outcome, attributed, broker_pubkey, reasoning,
                              writeback_triggered, writeback_committed, activity_urn,
                              proposal_urn, owner_did, decided_at_ms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    )?;
                    ins.execute(rusqlite::params![
                        &case_id,
                        &outcome,
                        attributed,
                        &broker_pubkey,
                        &reasoning,
                        writeback_triggered,
                        writeback_committed,
                        &activity_urn,
                        &proposal_urn,
                        &owner_did,
                        decided_at_ms,
                    ])?;
                    decision_id = tx.last_insert_rowid();

                    let mut upd = tx.prepare_cached(
                        "UPDATE enrichment_proposals
                         SET status = ?2, updated_at = unixepoch()
                         WHERE case_id = ?1",
                    )?;
                    upd.execute(rusqlite::params![&case_id, &status])?;
                }
                tx.commit()?;
                Ok(decision_id)
            })
            .await
            .map_err(map_db_err)
    }

    /// Flip `writeback_committed = 1` on the latest decision for a case + an
    /// activity URN, after the Oxigraph derived write returned `Ok`. Scoped by
    /// `activity_urn` so the correct decision row is marked even if a case has
    /// multiple decisions.
    pub async fn mark_writeback_committed(
        &self,
        case_id: &str,
        activity_urn: &str,
    ) -> Result<()> {
        let case_id_owned = case_id.to_string();
        let activity_owned = activity_urn.to_string();
        self.conn
            .call(move |c| {
                let mut stmt = c.prepare_cached(
                    "UPDATE enrichment_decisions
                     SET writeback_committed = 1
                     WHERE case_id = ?1 AND activity_urn = ?2",
                )?;
                stmt.execute(rusqlite::params![&case_id_owned, &activity_owned])?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    /// Decision outcomes recorded at or after `cutoff_ms`, newest first. Backs
    /// the REC-4 KPI compute (ADR-130 D5): the escalation volume is the row
    /// count and the Trust-Variance dispersion is over the `outcome` column, both
    /// windowed on `decided_at_ms`. Returns `(outcome, activity_urn, decided_at_ms)`
    /// so the KPI lineage can trace a value back to each contributing decision.
    pub async fn decisions_since(
        &self,
        cutoff_ms: i64,
    ) -> Result<Vec<(String, String, i64)>> {
        self.conn
            .call(move |c| {
                let mut stmt = c.prepare_cached(
                    "SELECT outcome, activity_urn, decided_at_ms
                     FROM enrichment_decisions
                     WHERE decided_at_ms >= ?1
                     ORDER BY decided_at_ms DESC, id DESC",
                )?;
                let mut q = stmt.query(rusqlite::params![cutoff_ms])?;
                let mut out = Vec::new();
                while let Some(r) = q.next()? {
                    out.push((r.get(0)?, r.get(1)?, r.get(2)?));
                }
                Ok(out)
            })
            .await
            .map_err(map_db_err)
    }

    /// All decisions for a case, newest first. Backs the WS-9 decision-log read
    /// (the broker-bridge `cases/{id}/history` follow-on).
    pub async fn decisions_for(&self, case_id: &str) -> Result<Vec<StoredDecision>> {
        let case_id_owned = case_id.to_string();
        self.conn
            .call(move |c| {
                let mut stmt = c.prepare_cached(
                    "SELECT case_id, outcome, attributed, broker_pubkey, reasoning,
                            writeback_triggered, writeback_committed, activity_urn,
                            proposal_urn, owner_did, decided_at_ms
                     FROM enrichment_decisions
                     WHERE case_id = ?1
                     ORDER BY decided_at_ms DESC, id DESC",
                )?;
                let mut q = stmt.query(rusqlite::params![&case_id_owned])?;
                let mut out = Vec::new();
                while let Some(r) = q.next()? {
                    let attributed: i64 = r.get(2)?;
                    let writeback_triggered: i64 = r.get(5)?;
                    let writeback_committed: i64 = r.get(6)?;
                    out.push(StoredDecision {
                        case_id: r.get(0)?,
                        outcome: r.get(1)?,
                        attributed: attributed != 0,
                        broker_pubkey: r.get(3)?,
                        reasoning: r.get(4)?,
                        writeback_triggered: writeback_triggered != 0,
                        writeback_committed: writeback_committed != 0,
                        activity_urn: r.get(7)?,
                        proposal_urn: r.get(8)?,
                        owner_did: r.get(9)?,
                        decided_at_ms: r.get(10)?,
                    });
                }
                Ok(out)
            })
            .await
            .map_err(map_db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_repo() -> SqliteEnrichmentRepository {
        // tempfile-backed; mirrors sqlite_settings_repository test posture.
        let dir = std::env::temp_dir().join(format!("enrich-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!(
            "enrichment-{}.sqlite3",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        SqliteEnrichmentRepository::open(&path).await.expect("open")
    }

    fn proposal(case_id: &str) -> EnrichmentProposal {
        EnrichmentProposal {
            case_id: case_id.to_string(),
            category: Some("knowledge_enrichment".into()),
            source_iri: Some("urn:ngm:node/foo".into()),
            proposal_json: serde_json::json!({"target_path": "pages/foo.md"}),
            status: "pending".into(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn decision(case_id: &str, committed: bool) -> StoredDecision {
        StoredDecision {
            case_id: case_id.to_string(),
            outcome: "approve".into(),
            attributed: true,
            broker_pubkey: Some("a".repeat(64)),
            reasoning: Some("looks good".into()),
            writeback_triggered: true,
            writeback_committed: committed,
            activity_urn: "urn:visionclaw:execution:sha256-12-abcdef012345".into(),
            proposal_urn: Some("urn:visionclaw:kg:pk:sha256-12-deadbeef0000".into()),
            owner_did: Some("did:nostr:pk".into()),
            decided_at_ms: 1_700_000_000_000,
        }
    }

    #[tokio::test]
    async fn upsert_get_and_list_roundtrip() {
        let repo = temp_repo().await;
        repo.create_or_update(&proposal("case-1")).await.unwrap();
        let got = repo.get("case-1").await.unwrap().expect("present");
        assert_eq!(got.case_id, "case-1");
        assert_eq!(got.status, "pending");
        assert_eq!(repo.count(None).await.unwrap(), 1);
        let listed = repo.list(Some("pending"), 50, 0).await.unwrap();
        assert_eq!(listed.len(), 1);
    }

    #[tokio::test]
    async fn record_decision_transitions_status_atomically() {
        let repo = temp_repo().await;
        repo.create_or_update(&proposal("case-2")).await.unwrap();
        let id = repo.record_decision(&decision("case-2", false)).await.unwrap();
        assert!(id > 0);
        let got = repo.get("case-2").await.unwrap().unwrap();
        assert_eq!(got.status, "approved", "approve outcome ⇒ approved status");
        let decisions = repo.decisions_for("case-2").await.unwrap();
        assert_eq!(decisions.len(), 1);
        assert!(!decisions[0].writeback_committed);
    }

    #[tokio::test]
    async fn mark_writeback_committed_flips_truth_bit() {
        let repo = temp_repo().await;
        repo.create_or_update(&proposal("case-3")).await.unwrap();
        let d = decision("case-3", false);
        repo.record_decision(&d).await.unwrap();
        repo.mark_writeback_committed("case-3", &d.activity_urn)
            .await
            .unwrap();
        let decisions = repo.decisions_for("case-3").await.unwrap();
        assert!(decisions[0].writeback_committed, "committed now true");
    }

    #[tokio::test]
    async fn decisions_since_windows_on_decided_at() {
        let repo = temp_repo().await;
        repo.create_or_update(&proposal("case-w1")).await.unwrap();
        repo.create_or_update(&proposal("case-w2")).await.unwrap();
        // Two decisions inside the window, one before the cutoff.
        let mut d1 = decision("case-w1", true);
        d1.decided_at_ms = 2_000;
        d1.activity_urn = "urn:visionclaw:execution:sha256-12-aaaaaaaaaaaa".into();
        let mut d2 = decision("case-w2", true);
        d2.outcome = "reject".into();
        d2.decided_at_ms = 3_000;
        d2.activity_urn = "urn:visionclaw:execution:sha256-12-bbbbbbbbbbbb".into();
        let mut d0 = decision("case-w1", true);
        d0.decided_at_ms = 500;
        d0.activity_urn = "urn:visionclaw:execution:sha256-12-cccccccccccc".into();
        repo.record_decision(&d0).await.unwrap();
        repo.record_decision(&d1).await.unwrap();
        repo.record_decision(&d2).await.unwrap();

        let windowed = repo.decisions_since(1_000).await.unwrap();
        assert_eq!(windowed.len(), 2, "only decisions at/after cutoff are returned");
        // Newest first.
        assert_eq!(windowed[0].0, "reject");
        assert_eq!(windowed[0].2, 3_000);
        assert!(windowed.iter().all(|(_, _, ts)| *ts >= 1_000));
    }

    #[test]
    fn status_mapping_matches_outcomes() {
        assert_eq!(status_for_outcome("approve"), "approved");
        assert_eq!(status_for_outcome("Accepted"), "approved");
        assert_eq!(status_for_outcome("reject"), "rejected");
        assert_eq!(status_for_outcome("rejected"), "rejected");
        assert_eq!(status_for_outcome("amend"), "reviewed");
    }
}
