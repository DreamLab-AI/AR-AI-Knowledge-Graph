//! Decision elevation — the inverse corpus path (ADR-050).
//!
//! The symmetric image of the class-elevation loop ([`crate::actors::elevation_actor`]):
//! a governed [`DecisionRecord`](crate::services::decision_service::DecisionInput)
//! is born in `urn:ngm:graph:ontology:assert` at runtime through the governed
//! decision write door, but that graph is periodically CLEAR+INSERT-rebuilt from
//! the corpus on a `force_full` sync ([`crate::services::github_sync_service`]).
//! A runtime decision is born-in-the-graph / absent-from-source, so the rebuild
//! would erase it. This module routes a **significant** decision *into the
//! corpus* — drafted as a page, gated through the broker, PR'd to `jjohare/logseq`
//! on approve — so the same sync→rebuild path that re-derives every class
//! re-derives the decision too.
//!
//! This module is the PURE half (no actor, no I/O): the significance predicate,
//! the page drafter/parser (a byte-faithful inverse pair), the `dl:` quad
//! re-materialiser, and the fire-and-forget [`DecisionElevationSink`] seam the
//! governed write door calls. The actor half — the broker case open, the
//! approve→PR commit and GOV-2 poll — lives in
//! [`crate::actors::decision_elevation_actor`] (which implements the sink),
//! mirroring the class-elevation `ElevationActor` shape (reuse, not reinvent).
//!
//! ## What lands where (ADR-049 boundary, preserved)
//!
//! The corpus page carries the decision SUMMARY plus a `dl:DecisionRecord`
//! json-ld block (type memberships + direct causal edges) and a *provenance
//! summary* (the `did:nostr` attribution + `generatedAtTime`). It does NOT carry
//! the signed envelope — the authoritative signed PROV-O attribution stays in the
//! `:provenance` graph. Re-materialisation ([`decision_page_quads`]) therefore
//! emits ONLY the asserted `dl:` quads via [`build_decision_quads`], never
//! attribution — exactly the asserted-graph projection the runtime write door
//! produces.

use log::warn;
use oxigraph::model::Quad;
use serde_json::{json, Value};

use crate::services::decision_service::{build_decision_quads, DecisionInput, DL_NS, PROV_NS};

/// Corpus namespace for elevated decision pages. Mirrors the class-elevation
/// convention (`mainKnowledgeGraph/pages/…`) with a dedicated `decisions/`
/// sub-namespace so a `force_full` read-half can list them cheaply.
pub const DECISIONS_DIR: &str = "mainKnowledgeGraph/pages/decisions";

/// Full `dl:DecisionRecord` class IRI (`DL_NS` + `DecisionRecord`). Accepted in
/// either compact (`dl:DecisionRecord`) or expanded form when recognising a page.
fn dl_decision_record_iri() -> String {
    format!("{DL_NS}DecisionRecord")
}

// ---------------------------------------------------------------------------
// Significance predicate (ADR-050 DECIDED: broker-gated, significant-only)
// ---------------------------------------------------------------------------

/// The elevation-significance predicate. A decision elevates to the public
/// corpus IFF it is *significant*; routine/edgeless decisions stay runtime-only
/// so the public corpus stays high-signal and the broker volume stays bounded.
///
/// Significant ⇔ **any** of the three signals already present at decision time:
///   1. it governed a graph mutation — carries a `proposal_urn`
///      (`urn:agentbox:activity:…`), i.e. the decision was *about* a change;
///   2. it carries any causal edge — a non-empty `dl:caused` / `dl:precedentFor`
///      / `dl:influenced` set (it shaped the decision graph);
///   3. it was ACSP-approved (`acsp_approved`) — a governance verdict was recorded.
///
/// `dl:consideredInput` / `dl:governedBy` are NOT significance signals on their
/// own (a decision can weigh inputs / cite a policy without being consequential);
/// an edgeless, proposal-less, un-approved decision is routine.
pub fn is_significant(input: &DecisionInput, acsp_approved: bool) -> bool {
    input.proposal_urn.is_some()
        || !input.caused.is_empty()
        || !input.precedent_for.is_empty()
        || !input.influenced.is_empty()
        || acsp_approved
}

// ---------------------------------------------------------------------------
// The elevation payload + the fire-and-forget sink
// ---------------------------------------------------------------------------

/// Everything the elevation path needs about a just-recorded decision. Built by
/// the governed write door and handed to the [`DecisionElevationSink`]; enough to
/// draft the corpus page and open the broker case without re-reading the store.
#[derive(Debug, Clone)]
pub struct ElevatedDecision {
    /// The minted `urn:agentbox:decision:<pubkey>:sha256-12-<hex>` — the page `@id`.
    pub decision_urn: String,
    /// The direct decision claims (summary/rationale + the 5 dl: edge sets).
    pub input: DecisionInput,
    /// `did:nostr:<hex>` of the deciding principal — the provenance-summary
    /// attribution on the page (NOT the signed envelope).
    pub agent_did: String,
    /// RFC-3339 activity timestamp for the provenance summary.
    pub generated_at: String,
    /// Whether an ACSP governance verdict approved the act (a significance signal).
    pub acsp_approved: bool,
}

/// The fire-and-forget seam the governed decision write door
/// ([`crate::services::decision_service::DecisionService`]) calls after a
/// SIGNIFICANT decision commits. Implemented by the actor adapter in
/// [`crate::actors::decision_elevation_actor`].
///
/// CONTRACT (ADR-050 fail-open): `elevate` MUST NOT block and MUST NOT be able to
/// fail the governed decision write. It returns `Result` only to report a
/// *synchronous enqueue* failure (e.g. the actor mailbox is gone), which the
/// caller logs and ignores — a broker/PR outage or missing token never blocks or
/// rolls back the decision itself.
pub trait DecisionElevationSink: Send + Sync {
    fn elevate(&self, decision: ElevatedDecision) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// Slug / page drafting (mirror of draft_class_page)
// ---------------------------------------------------------------------------

/// The content-address tail (`sha256-12-<12hex>` → `<12hex>`) of a decision URN,
/// used to make the corpus slug collision-proof and the page path deterministic.
fn urn_hash_tail(decision_urn: &str) -> String {
    decision_urn
        .rsplit("sha256-12-")
        .next()
        .map(|s| s.chars().take(12).collect::<String>())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Slugify to the corpus convention (mirrors the elevation slugifier).
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = true;
    for c in s.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Deterministic corpus slug for a decision: `<summary-slug>-<12hex>`. The URN
/// hash tail guarantees uniqueness even when two decisions share a summary.
pub fn decision_slug(decision_urn: &str, summary: &str) -> String {
    let base = slugify(summary);
    let base = if base.is_empty() {
        "decision".to_string()
    } else {
        base
    };
    // Bound the summary component so paths stay sane.
    let base: String = base.chars().take(64).collect();
    let base = base.trim_end_matches('-');
    format!("{base}-{}", urn_hash_tail(decision_urn))
}

/// Turn a list of target URNs into a json-ld object-list (`[{"@id": …}, …]`),
/// or a bare `{"@id": …}` for a single target, matching the corpus json-ld
/// convention the parser round-trips.
fn id_list(targets: &[String]) -> Value {
    let objs: Vec<Value> = targets.iter().map(|t| json!({ "@id": t })).collect();
    Value::Array(objs)
}

/// Draft the canonical corpus page for a significant decision — the inverse of
/// [`crate::actors::elevation_actor::draft_class_page`], for `dl:DecisionRecord`
/// instances. Emits `pages/decisions/<slug>.md` carrying a `dl:DecisionRecord`
/// json-ld block:
///   * `@id` = the decision URN; `@type` = `[prov:Activity, dl:DecisionRecord]`;
///   * `dl:summary` / `dl:rationale` (human-readable, corpus-only — never asserted);
///   * the direct `dl:caused` / `dl:precedentFor` / `dl:influenced` /
///     `dl:consideredInput` / `dl:governedBy` edges;
///   * a PROVENANCE SUMMARY (`prov:wasAssociatedWith <did:nostr>` +
///     `prov:generatedAtTime`) — NOT the signed envelope (ADR-049: the
///     authoritative signed attribution stays in the `:provenance` graph).
///
/// Returns `(file_path, markdown)`.
pub fn draft_decision_page(dec: &ElevatedDecision) -> (String, String) {
    let slug = decision_slug(&dec.decision_urn, &dec.input.summary);
    let file_path = format!("{DECISIONS_DIR}/{slug}.md");

    let jsonld = json!({
        "@context": {
            "dl": DL_NS,
            "prov": PROV_NS,
            "xsd": "http://www.w3.org/2001/XMLSchema#",
        },
        "@id": dec.decision_urn,
        "@type": ["prov:Activity", "dl:DecisionRecord"],
        "dl:summary": dec.input.summary,
        "dl:rationale": dec.input.rationale,
        "dl:caused": id_list(&dec.input.caused),
        "dl:precedentFor": id_list(&dec.input.precedent_for),
        "dl:influenced": id_list(&dec.input.influenced),
        "dl:consideredInput": id_list(&dec.input.considered_inputs),
        "dl:governedBy": id_list(&dec.input.governed_by),
        // Provenance SUMMARY only — the signed envelope stays in :provenance.
        "prov:wasAssociatedWith": { "@id": dec.agent_did },
        "prov:generatedAtTime": { "@value": dec.generated_at, "@type": "xsd:dateTime" },
    });

    let block = serde_json::to_string_pretty(&jsonld).unwrap_or_else(|_| "{}".to_string());

    let heading = if dec.input.summary.trim().is_empty() {
        "Decision".to_string()
    } else {
        dec.input.summary.trim().to_string()
    };

    let content = format!(
        "# {heading}\n\n\
         > Elevated decision record (ADR-050). Re-derived into \
         `urn:ngm:graph:ontology:assert` on the next corpus sync. Signed \
         attribution lives in the provenance graph; this page carries the summary.\n\n\
         ```json-ld\n{block}\n```\n"
    );

    (file_path, content)
}

// ---------------------------------------------------------------------------
// Page parsing / re-materialisation (the read-half recogniser)
// ---------------------------------------------------------------------------

/// A decision recognised from a corpus page — the parser's node typing of a
/// `dl:DecisionRecord` json-ld block back into the pieces the assert graph needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDecision {
    pub decision_urn: String,
    /// The direct claims recovered from the page (summary/rationale + edge sets).
    /// `proposal_urn` is intentionally `None`: it is a provenance concern and is
    /// not an asserted edge, so it is not carried on the corpus page.
    pub input: DecisionInput,
    /// The provenance-summary attribution, if the page carried one.
    pub agent_did: Option<String>,
}

/// Pull IRIs out of a json-ld relation value: a bare IRI string, a `{"@id": …}`
/// object, or an array of either. Mirrors the elevation parser's `jsonld_iri_list`.
fn jsonld_iri_list(value: &Value) -> Vec<String> {
    fn one(v: &Value, out: &mut Vec<String>) {
        if let Some(s) = v.as_str() {
            if !s.trim().is_empty() {
                out.push(s.to_string());
            }
        } else if let Some(s) = v.get("@id").and_then(|x| x.as_str()) {
            if !s.trim().is_empty() {
                out.push(s.to_string());
            }
        }
    }
    let mut out = Vec::new();
    match value {
        Value::Array(arr) => arr.iter().for_each(|v| one(v, &mut out)),
        v => one(v, &mut out),
    }
    out
}

/// Does the json-ld `@type` value name `dl:DecisionRecord` (compact or expanded)?
fn is_decision_record_type(types: &Value) -> bool {
    let full = dl_decision_record_iri();
    let matches_one = |s: &str| s == "dl:DecisionRecord" || s == full;
    match types {
        Value::String(s) => matches_one(s),
        Value::Array(arr) => arr
            .iter()
            .any(|v| v.as_str().map(matches_one).unwrap_or(false)),
        _ => false,
    }
}

/// Recognise a `dl:DecisionRecord` corpus page and recover its
/// [`ParsedDecision`] — the byte-faithful inverse of [`draft_decision_page`]
/// (via "the parser's node typing": the `@type` recognition). Returns `None` for
/// a page with no json-ld block, an unparseable block, a missing `@id`, or a
/// block that is not a decision record (so class pages are left to the
/// class-rebuild path untouched).
pub fn parse_decision_page(markdown: &str) -> Option<ParsedDecision> {
    let block = markdown
        .split("```json-ld")
        .nth(1)
        .and_then(|s| s.split("```").next())?;
    let value: Value = serde_json::from_str(block.trim()).ok()?;

    // Node typing gate: only decision-record instances (ADR-050 read-half).
    if !is_decision_record_type(value.get("@type")?) {
        return None;
    }
    let decision_urn = value.get("@id").and_then(|v| v.as_str())?.to_string();

    let edges = |key: &str| value.get(key).map(jsonld_iri_list).unwrap_or_default();

    let input = DecisionInput {
        summary: value
            .get("dl:summary")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        rationale: value
            .get("dl:rationale")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        proposal_urn: None,
        caused: edges("dl:caused"),
        precedent_for: edges("dl:precedentFor"),
        influenced: edges("dl:influenced"),
        considered_inputs: edges("dl:consideredInput"),
        governed_by: edges("dl:governedBy"),
    };

    let agent_did = value
        .get("prov:wasAssociatedWith")
        .and_then(|v| v.get("@id").and_then(|x| x.as_str()).or_else(|| v.as_str()))
        .map(str::to_string);

    Some(ParsedDecision {
        decision_urn,
        input,
        agent_did,
    })
}

/// Re-materialise a decision corpus page into its `urn:ngm:graph:ontology:assert`
/// quads — the type memberships + direct `dl:` edges, and NOTHING ELSE (no
/// attribution: that stays in `:provenance`, ADR-049). Reuses the runtime write
/// door's [`build_decision_quads`] so the rebuilt projection is byte-identical to
/// the one the governed door produces. Returns an empty vec for a non-decision
/// page (so the caller can fold it over every synced page cheaply).
pub fn decision_page_quads(markdown: &str) -> Vec<Quad> {
    match parse_decision_page(markdown) {
        Some(parsed) => build_decision_quads(&parsed.decision_urn, &parsed.input),
        None => {
            // Not a decision page — nothing to re-materialise. (Logged at trace
            // by callers that expect one; silent here so bulk folds stay quiet.)
            Vec::new()
        }
    }
}

/// Best-effort helper for callers that fold over many pages: parse + build,
/// logging (not failing) a page that types as a decision record but yields no
/// quads. Kept separate from [`decision_page_quads`] so the silent bulk path and
/// the diagnostic path are both available.
pub fn decision_page_quads_logged(markdown: &str, source: &str) -> Vec<Quad> {
    let quads = decision_page_quads(markdown);
    if quads.is_empty() && parse_decision_page(markdown).is_some() {
        warn!("[DecisionElevation] decision page '{source}' parsed but produced no assert quads");
    }
    quads
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::decision_service::{
        p_caused, p_governed_by, p_influenced, p_precedent_for, PROV_ACTIVITY, RDF_TYPE,
    };

    const PK: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn sample(input: DecisionInput) -> ElevatedDecision {
        let urn = format!("urn:agentbox:decision:{PK}:sha256-12-9ec3d090ff23");
        ElevatedDecision {
            decision_urn: urn,
            input,
            agent_did: format!("did:nostr:{PK}"),
            generated_at: "2026-08-08T00:00:00Z".to_string(),
            acsp_approved: false,
        }
    }

    // --- (b) significance predicate ---------------------------------------

    #[test]
    fn routine_edgeless_decision_is_not_significant() {
        // No proposal, no causal edges, not ACSP-approved → routine → runtime-only.
        let routine = DecisionInput {
            summary: "note a routine observation".into(),
            rationale: "nothing structural".into(),
            proposal_urn: None,
            considered_inputs: vec!["urn:agentbox:activity:src".into()], // input alone ≠ significant
            governed_by: vec!["urn:agentbox:decision:policy".into()], // cited policy alone ≠ significant
            ..Default::default()
        };
        assert!(!is_significant(&routine, false));
    }

    #[test]
    fn causal_or_mutation_or_approved_decision_is_significant() {
        // 1. carries a causal edge.
        let causal = DecisionInput {
            summary: "merge duplicate concepts".into(),
            caused: vec!["urn:agentbox:decision:AA:sha256-12-def".into()],
            ..Default::default()
        };
        assert!(is_significant(&causal, false));

        // 2. governed a graph mutation (has a proposal_urn).
        let mutation = DecisionInput {
            summary: "adopt the delta-scoped gate".into(),
            proposal_urn: Some("urn:agentbox:activity:abc".into()),
            ..Default::default()
        };
        assert!(is_significant(&mutation, false));

        // 3. ACSP-approved.
        let approved = DecisionInput {
            summary: "a plain decision".into(),
            ..Default::default()
        };
        assert!(!is_significant(&approved, false));
        assert!(is_significant(&approved, true));

        // precedentFor / influenced also qualify.
        let precedent = DecisionInput {
            precedent_for: vec!["urn:agentbox:decision:AA:sha256-12-ghi".into()],
            ..Default::default()
        };
        assert!(is_significant(&precedent, false));
        let influenced = DecisionInput {
            influenced: vec!["urn:agentbox:decision:AA:sha256-12-jkl".into()],
            ..Default::default()
        };
        assert!(is_significant(&influenced, false));
    }

    // --- (a) draft round-trips to a parseable page with dl: @type + edges --

    #[test]
    fn draft_decision_page_path_is_namespaced_and_deterministic() {
        let dec = sample(DecisionInput {
            summary: "Merge Duplicate Concepts".into(),
            rationale: "resolves DUPLICATE_CONCEPT".into(),
            ..Default::default()
        });
        let (path, _) = draft_decision_page(&dec);
        assert!(path.starts_with("mainKnowledgeGraph/pages/decisions/"));
        assert!(path.ends_with(".md"));
        // Deterministic: same decision → same path.
        let (path2, _) = draft_decision_page(&dec);
        assert_eq!(path, path2);
        // Slug carries the URN hash tail for collision-safety.
        assert!(
            path.contains("9ec3d090ff23"),
            "path lacks URN hash tail: {path}"
        );
    }

    #[test]
    fn draft_round_trips_type_edges_and_urn() {
        let input = DecisionInput {
            summary: "merge duplicate concepts".into(),
            rationale: "resolves DUPLICATE_CONCEPT".into(),
            proposal_urn: Some("urn:agentbox:activity:abc".into()),
            caused: vec!["urn:agentbox:decision:AA:sha256-12-def".into()],
            precedent_for: vec!["urn:agentbox:decision:AA:sha256-12-ghi".into()],
            influenced: vec!["urn:agentbox:decision:AA:sha256-12-jkl".into()],
            considered_inputs: vec!["urn:agentbox:activity:in".into()],
            governed_by: vec!["urn:agentbox:decision:policy".into()],
        };
        let dec = sample(input.clone());
        let (_, markdown) = draft_decision_page(&dec);

        // The block parses and is typed as a dl:DecisionRecord.
        let parsed = parse_decision_page(&markdown).expect("decision page parses");
        assert_eq!(parsed.decision_urn, dec.decision_urn);
        assert_eq!(parsed.agent_did.as_deref(), Some(dec.agent_did.as_str()));

        // Every direct edge set round-trips (proposal_urn is provenance-only).
        assert_eq!(parsed.input.caused, input.caused);
        assert_eq!(parsed.input.precedent_for, input.precedent_for);
        assert_eq!(parsed.input.influenced, input.influenced);
        assert_eq!(parsed.input.considered_inputs, input.considered_inputs);
        assert_eq!(parsed.input.governed_by, input.governed_by);
        assert_eq!(parsed.input.summary, input.summary);
        assert_eq!(parsed.input.rationale, input.rationale);
        assert_eq!(
            parsed.input.proposal_urn, None,
            "proposal_urn is not on the corpus page"
        );

        // Raw json-ld carries the exact dl: @type (both memberships).
        assert!(markdown.contains("\"dl:DecisionRecord\""));
        assert!(markdown.contains("\"prov:Activity\""));
        assert!(markdown.contains("\"dl:caused\""));
    }

    #[test]
    fn decision_page_quads_match_the_runtime_write_door_projection() {
        let input = DecisionInput {
            summary: "s".into(),
            rationale: "r".into(),
            proposal_urn: Some("urn:agentbox:activity:p".into()),
            caused: vec!["urn:agentbox:decision:AA:sha256-12-def".into()],
            precedent_for: vec![],
            influenced: vec!["urn:agentbox:decision:AA:sha256-12-jkl".into()],
            considered_inputs: vec![],
            governed_by: vec!["urn:agentbox:decision:policy".into()],
        };
        let dec = sample(input.clone());
        let (_, markdown) = draft_decision_page(&dec);

        let quads = decision_page_quads(&markdown);
        // Two type quads + caused(1) + influenced(1) + governedBy(1) = 5.
        assert_eq!(quads.len(), 5, "re-materialised quad count");

        let preds: Vec<String> = quads
            .iter()
            .map(|q| q.predicate.as_str().to_string())
            .collect();
        assert_eq!(preds.iter().filter(|p| *p == RDF_TYPE).count(), 2);
        assert_eq!(preds.iter().filter(|p| **p == p_caused()).count(), 1);
        assert_eq!(preds.iter().filter(|p| **p == p_influenced()).count(), 1);
        assert_eq!(preds.iter().filter(|p| **p == p_governed_by()).count(), 1);
        assert_eq!(preds.iter().filter(|p| **p == p_precedent_for()).count(), 0);

        // NO attribution leaks into the asserted projection (ADR-049 boundary).
        for q in &quads {
            let p = q.predicate.as_str();
            assert!(!p.contains("wasAssociatedWith"));
            assert!(!p.contains("generatedAtTime"));
        }
        // Type memberships are exactly prov:Activity + dl:DecisionRecord.
        let types: Vec<String> = quads
            .iter()
            .filter(|q| q.predicate.as_str() == RDF_TYPE)
            .map(|q| q.object.to_string())
            .collect();
        assert!(types.iter().any(|o| o.contains(PROV_ACTIVITY)));
        assert!(types.iter().any(|o| o.contains("DecisionRecord")));
    }

    #[test]
    fn non_decision_page_is_ignored_by_the_read_half() {
        // A class page (the class-elevation output) must NOT be re-materialised here.
        let class_page = "# Camera\n```json-ld\n{\n  \"@id\": \"urn:ngm:class:camera\",\n  \"@type\": \"Class\"\n}\n```\n";
        assert!(parse_decision_page(class_page).is_none());
        assert!(decision_page_quads(class_page).is_empty());

        // A page with no json-ld block at all.
        assert!(parse_decision_page("# Just prose\nno block here").is_none());
    }

    #[test]
    fn parse_tolerates_bare_iri_and_object_edge_forms() {
        // Hand-authored / older pages may use bare-string edges; accept both.
        let md = format!(
            "# D\n```json-ld\n{{\n  \"@id\": \"urn:agentbox:decision:{PK}:sha256-12-abc\",\n  \"@type\": [\"prov:Activity\", \"dl:DecisionRecord\"],\n  \"dl:caused\": \"urn:agentbox:decision:bare\",\n  \"dl:precedentFor\": [{{\"@id\": \"urn:agentbox:decision:obj\"}}]\n}}\n```\n"
        );
        let parsed = parse_decision_page(&md).expect("parses");
        assert_eq!(parsed.input.caused, vec!["urn:agentbox:decision:bare"]);
        assert_eq!(
            parsed.input.precedent_for,
            vec!["urn:agentbox:decision:obj"]
        );
    }
}
