//! ADR-117 WS-0 regression guard: server-side SPARQL result clamp.
//!
//! The `/ontology/query` (CQRS) read path returned `Vec<HashMap>` straight from
//! Oxigraph's `run_select` with no LIMIT / row / byte bound, so an authed caller
//! could materialise an unbounded SELECT against the store (the sibling
//! `/ontology/sparql` path was already fenced inside `sparql_select_json`).
//!
//! These tests exercise the pure clamp/cap helpers the handler now applies:
//!   * a no-LIMIT SELECT gets a default LIMIT injected,
//!   * an oversize LIMIT is rewritten down to the hard cap,
//!   * an oversize result set is truncated with an explicit `truncated` flag
//!     rather than being silently cut.
//!
//! Idiom mirrors the inline clamp assertion in crates/visionclaw-adapters and
//! the `tests/rec1_route_guard.rs` regression canaries.

use std::collections::HashMap;
use visionclaw_server::handlers::ontology_handler::{cap_result_rows, clamp_sparql_limit};

const ROW_CAP: usize = 10_000;

#[test]
fn no_limit_query_gets_default_injected() {
    let out = clamp_sparql_limit("SELECT ?s ?p ?o WHERE { ?s ?p ?o }");
    assert!(
        out.to_uppercase().contains(&format!("LIMIT {ROW_CAP}")),
        "a SELECT without LIMIT must have the default injected, got: {out}"
    );
}

#[test]
fn oversize_limit_is_reduced_to_cap() {
    let out = clamp_sparql_limit("SELECT ?s WHERE { ?s ?p ?o } LIMIT 5000000");
    assert!(
        out.contains(&format!("LIMIT {ROW_CAP}")),
        "an oversize LIMIT must be clamped to the cap, got: {out}"
    );
    assert!(
        !out.contains("5000000"),
        "the original oversize LIMIT value must be gone, got: {out}"
    );
}

#[test]
fn within_cap_limit_is_preserved() {
    let q = "SELECT ?s WHERE { ?s ?p ?o } LIMIT 50";
    assert_eq!(
        clamp_sparql_limit(q),
        q,
        "a LIMIT within the cap must pass through unchanged"
    );
}

#[test]
fn ask_query_is_not_rewritten() {
    let q = "ASK { ?s ?p ?o }";
    assert_eq!(
        clamp_sparql_limit(q),
        q,
        "ASK has no result set to bound and must be left untouched"
    );
}

#[test]
fn byte_cap_truncation_is_flagged() {
    // ~2.1 KiB per row, 5_000 rows ≈ 10.5 MiB > the 8 MiB byte cap, while the
    // row count (5_000) stays under the 10_000 row cap — so this isolates the
    // byte fence specifically.
    let big = "x".repeat(2100);
    let rows: Vec<HashMap<String, String>> = (0..5_000)
        .map(|i| {
            let mut m = HashMap::new();
            m.insert("v".to_string(), format!("{i}-{big}"));
            m
        })
        .collect();
    assert!(rows.len() < ROW_CAP, "fixture must stay under the row cap");

    let (capped, truncated) = cap_result_rows(rows);
    assert!(
        truncated,
        "an over-byte-cap result set must set the truncated flag"
    );
    assert!(
        capped.len() < 5_000,
        "rows must actually be dropped by the byte cap, kept {}",
        capped.len()
    );
}

#[test]
fn row_count_cap_is_flagged() {
    // Tiny rows so cumulative bytes stay well under the byte cap; only the
    // row-count fence should fire here.
    let rows: Vec<HashMap<String, String>> = (0..(ROW_CAP + 5))
        .map(|i| {
            let mut m = HashMap::new();
            m.insert("v".to_string(), i.to_string());
            m
        })
        .collect();

    let (capped, truncated) = cap_result_rows(rows);
    assert!(
        truncated,
        "more than the row cap must set the truncated flag"
    );
    assert_eq!(
        capped.len(),
        ROW_CAP,
        "rows must be truncated down to exactly the row cap"
    );
}

#[test]
fn under_cap_result_is_not_flagged() {
    let rows: Vec<HashMap<String, String>> = (0..100)
        .map(|i| {
            let mut m = HashMap::new();
            m.insert("v".to_string(), i.to_string());
            m
        })
        .collect();

    let (capped, truncated) = cap_result_rows(rows);
    assert!(
        !truncated,
        "a small result set must not be flagged truncated"
    );
    assert_eq!(
        capped.len(),
        100,
        "a small result set must pass through intact"
    );
}
