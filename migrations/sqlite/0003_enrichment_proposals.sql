-- migrations/sqlite/0003_enrichment_proposals.sql
-- WS-9: Durable EnrichmentProposal lifecycle store.
--
-- Mirrors the schema_migrations discipline in
-- src/adapters/sqlite_settings_repository.rs:77-98. This migration lives in
-- its OWN database file (data/enrichment.sqlite3), NOT settings.sqlite3, so
-- the single-writer posture of the settings store is not shared and WS-9
-- migrations stay independent (lower blast-radius). Because it is a separate
-- file, schema_migrations does not exist here — the adapter's CREATE_SCHEMA
-- self-bootstraps it (see sqlite_enrichment_repository.rs CREATE_SCHEMA).
--
-- The in-memory writeback ghost this replaces conflated two distinct facts:
--   * writeback_triggered  — this outcome SHOULD write back (approve + attributed)
--   * writeback_committed   — the Oxigraph derived write actually landed
-- Both are persisted here so the agentbox broker-bridge can key true closure
-- off `committed`, not the (previously always-lying) `triggered`.

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
