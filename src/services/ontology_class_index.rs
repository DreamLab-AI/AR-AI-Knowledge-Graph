//! ADR-114 — RuVector seed leg for the ontology Class-Summary index.
//!
//! The Oxigraph half of ADR-114 (durable summary text as annotation triples in
//! `urn:ngm:graph:ontology:summary`) already ships in
//! [`crate::handlers::ontology_derived_handler`]. This module builds the **seed
//! leg**: per-`owl:Class` summaries condensed from the synced corpus and written
//! into RuVector namespace `ontology-classes` via the claude-flow `memory_store`
//! MCP tool (xinference `bge-small-en-v1.5`, 384-dim, HNSW) — **never raw SQL**,
//! which would bypass the embedding pipeline (agentbox CLAUDE.md mandate;
//! ADR-114 §3).
//!
//! Three pieces, matching the landing-plan row-8 deliverables:
//!   1. a **condensation job** ([`refresh_class_index`]) turning each
//!      [`OwlClass`] into a ~100–150-token retrieval-optimised [`ClassSummary`]
//!      and storing it through a [`ClassIndexStore`];
//!   2. a **[`ClassSummaryIndexRefreshed`] trigger** fired from GitHubSync
//!      ([`maybe_refresh_after_sync`]) when the ontology corpus changed;
//!   3. an **ADR-119 liveness canary** ([`liveness_canary`]) that
//!      `memory_search`-es the namespace and reports.
//!
//! **Config-gated, default-OFF.** Nothing here touches the network unless
//! `ONTOLOGY_CLASS_INDEX_ENABLED=1`. The whole path is fail-open: a store error
//! is logged and counted, never propagated into the sync path.

use async_trait::async_trait;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};

use crate::utils::mcp_tcp_client::McpTcpClient;
use visionclaw_domain::ports::owl_types::OwlClass;

/// Default RuVector namespace for the seed leg (ADR-114 §3).
pub const DEFAULT_NAMESPACE: &str = "ontology-classes";

/// Default canary query for the ADR-119 liveness self-test.
pub const DEFAULT_CANARY_QUERY: &str = "ontology class knowledge graph concept";

/// Target upper bound on a condensed summary, in characters. ADR-113 aims for
/// ~100–150 tokens; at ~4 chars/token that is ~600 chars. We clamp on a word
/// boundary so the embedding sees clean text.
const SUMMARY_CHAR_BUDGET: usize = 600;

/// Runtime configuration for the seed leg. Read from the environment;
/// **disabled by default** so a vanilla build never reaches out to RuVector.
#[derive(Debug, Clone)]
pub struct ClassIndexConfig {
    /// Master gate. `ONTOLOGY_CLASS_INDEX_ENABLED=1|true` to arm.
    pub enabled: bool,
    /// RuVector namespace to write into.
    pub namespace: String,
    /// claude-flow MCP host (shares `MCP_HOST` with the rest of the estate).
    pub mcp_host: String,
    /// claude-flow MCP TCP port (shares `MCP_TCP_PORT`).
    pub mcp_port: u16,
    /// Canary query for the liveness self-test.
    pub canary_query: String,
}

impl Default for ClassIndexConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            namespace: DEFAULT_NAMESPACE.to_string(),
            mcp_host: "agentic-workstation".to_string(),
            mcp_port: 9500,
            canary_query: DEFAULT_CANARY_QUERY.to_string(),
        }
    }
}

impl ClassIndexConfig {
    /// Build from the environment. All fields default-safe; `enabled` is OFF
    /// unless explicitly set truthy.
    pub fn from_env() -> Self {
        let enabled = std::env::var("ONTOLOGY_CLASS_INDEX_ENABLED")
            .map(|v| {
                let v = v.trim().to_ascii_lowercase();
                v == "1" || v == "true" || v == "yes" || v == "on"
            })
            .unwrap_or(false);

        let namespace = std::env::var("ONTOLOGY_CLASS_INDEX_NAMESPACE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_NAMESPACE.to_string());

        let mcp_host =
            std::env::var("MCP_HOST").unwrap_or_else(|_| "agentic-workstation".to_string());
        let mcp_port = std::env::var("MCP_TCP_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(9500);

        let canary_query = std::env::var("ONTOLOGY_CLASS_INDEX_CANARY_QUERY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_CANARY_QUERY.to_string());

        Self {
            enabled,
            namespace,
            mcp_host,
            mcp_port,
            canary_query,
        }
    }
}

/// A per-class condensed record: the RuVector key, the human/retrieval summary
/// text (the embedded payload), and the source IRI for provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassSummary {
    /// Stable RuVector key derived from the class IRI.
    pub key: String,
    /// Source class IRI (kept for provenance / round-trip to Oxigraph).
    pub iri: String,
    /// Retrieval-optimised summary text — this is what gets embedded.
    pub summary: String,
}

/// The `ClassSummaryIndexRefreshed{changed_count}` event (ADR-114 §6, §4
/// drift-mitigation). Fired from GitHubSync after an ontology-touching sync so
/// staleness is observable rather than silent (the anti-PRD-018 posture ADR-119
/// generalises).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassSummaryIndexRefreshed {
    /// Number of class summaries (re-)written into RuVector this refresh.
    pub changed_count: usize,
    /// Total classes considered (denominator for the changed fraction).
    pub classes_seen: usize,
    /// Number of store writes that failed (fail-open: logged, not fatal).
    pub write_errors: usize,
    /// RuVector namespace written.
    pub namespace: String,
}

impl ClassSummaryIndexRefreshed {
    /// Emit the event to the log in a grep-stable form. Kept deliberately simple
    /// (a structured log line) so it needs no bus wiring; a future consumer can
    /// subscribe by tailing `[class-index]`.
    pub fn emit(&self) {
        info!(
            "[class-index] ClassSummaryIndexRefreshed{{changed_count={}, classes_seen={}, write_errors={}, namespace={}}}",
            self.changed_count, self.classes_seen, self.write_errors, self.namespace
        );
    }
}

/// Verdict of the ADR-119 liveness canary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanaryReport {
    /// True iff the namespace was searchable AND returned ≥1 hit.
    pub ok: bool,
    /// Number of hits the canary query returned.
    pub hits: usize,
    /// The query used.
    pub query: String,
    /// Namespace searched.
    pub namespace: String,
    /// Populated when the search itself errored (transport/tool failure).
    pub error: Option<String>,
}

/// Storage seam for the seed leg. The production impl ([`RuVectorMemoryStore`])
/// talks to claude-flow over TCP; tests use an in-memory fake. This keeps the
/// condensation/trigger logic unit-testable with no network.
#[async_trait]
pub trait ClassIndexStore: Send + Sync {
    /// Store one summary. `Err(String)` on any failure (fail-open at the caller).
    async fn store_summary(&self, namespace: &str, key: &str, value: &str) -> Result<(), String>;

    /// Semantic-search the namespace, returning the hit count.
    async fn search(&self, namespace: &str, query: &str, limit: usize) -> Result<usize, String>;
}

/// Production [`ClassIndexStore`] backed by claude-flow `memory_store` /
/// `memory_search` MCP tools → RuVector HNSW. Never issues raw SQL.
pub struct RuVectorMemoryStore {
    client: McpTcpClient,
}

impl RuVectorMemoryStore {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            client: McpTcpClient::new(host, port),
        }
    }

    pub fn from_config(cfg: &ClassIndexConfig) -> Self {
        Self::new(cfg.mcp_host.clone(), cfg.mcp_port)
    }
}

#[async_trait]
impl ClassIndexStore for RuVectorMemoryStore {
    async fn store_summary(&self, namespace: &str, key: &str, value: &str) -> Result<(), String> {
        self.client
            .memory_store(namespace, key, value)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn search(&self, namespace: &str, query: &str, limit: usize) -> Result<usize, String> {
        let result = self
            .client
            .memory_search(namespace, query, limit)
            .await
            .map_err(|e| e.to_string())?;
        Ok(extract_hit_count(&result))
    }
}

/// Best-effort extraction of a hit count from a `memory_search` tool result.
/// Handles the common shapes claude-flow returns: `{results:[...]}`,
/// `{matches:[...]}`, `{count:N}`, or a bare array.
fn extract_hit_count(v: &serde_json::Value) -> usize {
    if let Some(n) = v.get("count").and_then(|c| c.as_u64()) {
        return n as usize;
    }
    for field in ["results", "matches", "entries", "hits", "data"] {
        if let Some(arr) = v.get(field).and_then(|a| a.as_array()) {
            return arr.len();
        }
    }
    if let Some(arr) = v.as_array() {
        return arr.len();
    }
    0
}

/// Derive a stable, filesystem/namespace-safe RuVector key from a class IRI.
/// Non-alphanumeric runs collapse to a single `-`; leading/trailing `-` trimmed.
/// Deterministic so a re-condense of the same class overwrites its prior entry
/// (incremental refresh, ADR-114 §4) rather than duplicating it.
pub fn key_for_iri(iri: &str) -> String {
    let mut out = String::with_capacity(iri.len() + 6);
    let mut prev_dash = false;
    for ch in iri.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    let body = if trimmed.is_empty() {
        "class".to_string()
    } else {
        trimmed
    };
    format!("class-{body}")
}

/// Condense one [`OwlClass`] into a retrieval-optimised [`ClassSummary`].
///
/// Deterministic (no LLM): assembles the human-facing label, description, class
/// type/domain, and the strongest structural relationships into one compact
/// block, then clamps to [`SUMMARY_CHAR_BUDGET`] on a word boundary. This is the
/// deterministic tier of ADR-113 §2.1 (the optional cheap-LLM condense is the
/// agentbox-side stage; the seed leg is fed by whichever tier ran).
pub fn condense_class(class: &OwlClass) -> ClassSummary {
    let label = class
        .preferred_term
        .as_deref()
        .or(class.label.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| last_iri_segment(&class.iri));

    let mut parts: Vec<String> = Vec::new();
    parts.push(label.to_string());

    if let Some(desc) = class
        .description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(desc.to_string());
    }

    if let Some(ct) = class
        .class_type
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("Type: {ct}."));
    }

    if let Some(domain) = class
        .belongs_to_domain
        .as_deref()
        .or(class.source_domain.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("Domain: {domain}."));
    }

    let parents = human_list(&class.parent_classes);
    if !parents.is_empty() {
        parts.push(format!("Subclass of {parents}."));
    }

    let requires = human_list(&class.requires);
    if !requires.is_empty() {
        parts.push(format!("Requires {requires}."));
    }

    let enables = human_list(&class.enables);
    if !enables.is_empty() {
        parts.push(format!("Enables {enables}."));
    }

    let relates = human_list(&class.relates_to);
    if !relates.is_empty() {
        parts.push(format!("Relates to {relates}."));
    }

    let summary = clamp_words(&parts.join(" "), SUMMARY_CHAR_BUDGET);

    ClassSummary {
        key: key_for_iri(&class.iri),
        iri: class.iri.clone(),
        summary,
    }
}

/// Human-readable label for a related IRI list: the last path/hash segment of
/// each, comma-joined, deduped, capped at 6 to keep the summary tight.
fn human_list(iris: &[String]) -> String {
    let mut seen = std::collections::HashSet::new();
    let labels: Vec<String> = iris
        .iter()
        .map(|i| last_iri_segment(i).to_string())
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .take(6)
        .collect();
    labels.join(", ")
}

/// Last meaningful segment of an IRI (after the final `#`, `/`, or `:`).
fn last_iri_segment(iri: &str) -> &str {
    iri.rsplit(['#', '/', ':'])
        .find(|s| !s.is_empty())
        .unwrap_or(iri)
}

/// Clamp `s` to at most `budget` chars, cutting on the last whitespace before
/// the limit so we never split a word. Appends a single `…` when truncated.
fn clamp_words(s: &str, budget: usize) -> String {
    let s = s.trim();
    if s.len() <= budget {
        return s.to_string();
    }
    // `budget` may land mid-character in multi-byte UTF-8 (OWL descriptions
    // routinely contain non-ASCII); back off to the nearest char boundary at or
    // below it before slicing, so this can never panic.
    let mut limit = budget;
    while limit > 0 && !s.is_char_boundary(limit) {
        limit -= 1;
    }
    let slice = &s[..limit];
    let cut = slice.rfind(char::is_whitespace).unwrap_or(limit);
    let mut out = s[..cut].trim_end().to_string();
    out.push('…');
    out
}

/// The condensation job (ADR-114 deliverable 1). Condenses every class and
/// writes it through `store`, returning the event. **Fail-open**: a per-class
/// store error is counted in `write_errors`, never aborts the run — a partial
/// refresh still improves the index monotonically (ADR-113 §2.1).
pub async fn refresh_class_index<S: ClassIndexStore + ?Sized>(
    store: &S,
    namespace: &str,
    classes: &[OwlClass],
) -> ClassSummaryIndexRefreshed {
    let mut changed_count = 0usize;
    let mut write_errors = 0usize;

    for class in classes {
        let summary = condense_class(class);
        match store
            .store_summary(namespace, &summary.key, &summary.summary)
            .await
        {
            Ok(()) => {
                changed_count += 1;
                debug!("[class-index] stored {} ({})", summary.key, summary.iri);
            }
            Err(e) => {
                write_errors += 1;
                warn!(
                    "[class-index] store failed for {} ({}): {}",
                    summary.key, summary.iri, e
                );
            }
        }
    }

    ClassSummaryIndexRefreshed {
        changed_count,
        classes_seen: classes.len(),
        write_errors,
        namespace: namespace.to_string(),
    }
}

/// The ADR-119 liveness canary (deliverable 3). `memory_search`-es the namespace
/// and reports whether it is live and populated. Fail-open: a transport error
/// yields `ok:false` with the error recorded, never panics.
pub async fn liveness_canary<S: ClassIndexStore + ?Sized>(
    store: &S,
    namespace: &str,
    query: &str,
) -> CanaryReport {
    match store.search(namespace, query, 5).await {
        Ok(hits) => {
            let ok = hits > 0;
            if ok {
                info!(
                    "[class-index] canary OK: ns='{}' query='{}' hits={}",
                    namespace, query, hits
                );
            } else {
                warn!(
                    "[class-index] canary EMPTY: ns='{}' query='{}' returned 0 hits (index may be stale/unbuilt)",
                    namespace, query
                );
            }
            CanaryReport {
                ok,
                hits,
                query: query.to_string(),
                namespace: namespace.to_string(),
                error: None,
            }
        }
        Err(e) => {
            warn!(
                "[class-index] canary FAILED: ns='{}' query='{}': {}",
                namespace, query, e
            );
            CanaryReport {
                ok: false,
                hits: 0,
                query: query.to_string(),
                namespace: namespace.to_string(),
                error: Some(e),
            }
        }
    }
}

/// GitHubSync entry point (deliverable 2 — the trigger). Called after a sync
/// that touched the ontology corpus. Reads config; **no-op + returns `None`
/// when disabled** (default). When enabled it condenses `classes` into RuVector,
/// emits [`ClassSummaryIndexRefreshed`], then runs the liveness canary. Fully
/// fail-open — any error is logged, never returned into the sync path.
///
/// `ontology_changed` is the GitHubSync signal (e.g. `ontology_files_processed
/// > 0`); when false the refresh is skipped to avoid needless embedding load.
pub async fn maybe_refresh_after_sync(
    ontology_changed: bool,
    classes: &[OwlClass],
) -> Option<ClassSummaryIndexRefreshed> {
    let cfg = ClassIndexConfig::from_env();
    if !cfg.enabled {
        debug!("[class-index] disabled (ONTOLOGY_CLASS_INDEX_ENABLED unset) — skipping refresh");
        return None;
    }
    if !ontology_changed {
        debug!("[class-index] ontology corpus unchanged this sync — skipping refresh");
        return None;
    }
    if classes.is_empty() {
        debug!("[class-index] no classes to index — skipping refresh");
        return None;
    }

    info!(
        "[class-index] refreshing {} class summaries into RuVector ns='{}' via {}:{}",
        classes.len(),
        cfg.namespace,
        cfg.mcp_host,
        cfg.mcp_port
    );

    let store = RuVectorMemoryStore::from_config(&cfg);
    let event = refresh_class_index(&store, &cfg.namespace, classes).await;
    event.emit();

    // ADR-119 canary immediately after refresh so staleness surfaces at source.
    let _ = liveness_canary(&store, &cfg.namespace, &cfg.canary_query).await;

    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn class(iri: &str) -> OwlClass {
        OwlClass {
            iri: iri.to_string(),
            ..Default::default()
        }
    }

    /// In-memory fake store — records writes, answers searches from what was
    /// written. Lets the condensation/trigger/canary logic run with no network.
    #[derive(Default)]
    struct FakeStore {
        writes: Mutex<Vec<(String, String, String)>>,
        fail_keys: Vec<String>,
        force_search_err: bool,
    }

    #[async_trait]
    impl ClassIndexStore for FakeStore {
        async fn store_summary(
            &self,
            namespace: &str,
            key: &str,
            value: &str,
        ) -> Result<(), String> {
            if self.fail_keys.iter().any(|k| k == key) {
                return Err(format!("injected failure for {key}"));
            }
            self.writes.lock().unwrap().push((
                namespace.to_string(),
                key.to_string(),
                value.to_string(),
            ));
            Ok(())
        }

        async fn search(
            &self,
            namespace: &str,
            _query: &str,
            _limit: usize,
        ) -> Result<usize, String> {
            if self.force_search_err {
                return Err("injected search error".to_string());
            }
            let n = self
                .writes
                .lock()
                .unwrap()
                .iter()
                .filter(|(ns, _, _)| ns == namespace)
                .count();
            Ok(n)
        }
    }

    #[test]
    fn config_defaults_off() {
        let cfg = ClassIndexConfig::default();
        assert!(!cfg.enabled, "must default to disabled");
        assert_eq!(cfg.namespace, "ontology-classes");
        assert_eq!(cfg.mcp_port, 9500);
    }

    #[test]
    fn key_is_stable_and_sanitised() {
        let k = key_for_iri("https://narrativegoldmine.com/ns/v1#KnowledgeGraph");
        assert_eq!(
            k,
            key_for_iri("https://narrativegoldmine.com/ns/v1#KnowledgeGraph"),
            "deterministic"
        );
        assert!(k.starts_with("class-"));
        assert!(
            k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "sanitised: {k}"
        );
        // Empty / punctuation-only IRIs still yield a usable key.
        assert_eq!(key_for_iri("###"), "class-class");
    }

    #[test]
    fn last_segment_extraction() {
        assert_eq!(
            last_iri_segment("https://x.com/ns/v1#KnowledgeGraph"),
            "KnowledgeGraph"
        );
        assert_eq!(last_iri_segment("http://x/a/b/Concept"), "Concept");
        assert_eq!(last_iri_segment("urn:ngm:Thing"), "Thing");
        // Trailing slash tolerated.
        assert_eq!(last_iri_segment("http://x/a/b/"), "b");
    }

    #[test]
    fn condense_prefers_label_and_includes_relationships() {
        let mut c = class("https://ngm/ns/v1#GpuActor");
        c.preferred_term = Some("GPU Actor".to_string());
        c.description = Some("Owns the CUDA context and dispatches kernels.".to_string());
        c.class_type = Some("Component".to_string());
        c.belongs_to_domain = Some("Compute".to_string());
        c.parent_classes = vec!["https://ngm/ns/v1#Actor".to_string()];
        c.requires = vec!["https://ngm/ns/v1#CudaDevice".to_string()];
        c.enables = vec!["https://ngm/ns/v1#PhysicsSim".to_string()];

        let s = condense_class(&c);
        assert_eq!(s.iri, "https://ngm/ns/v1#GpuActor");
        assert_eq!(s.key, "class-https-ngm-ns-v1-gpuactor");
        assert!(s.summary.starts_with("GPU Actor"));
        assert!(s.summary.contains("Owns the CUDA context"));
        assert!(s.summary.contains("Type: Component."));
        assert!(s.summary.contains("Domain: Compute."));
        assert!(s.summary.contains("Subclass of Actor."));
        assert!(s.summary.contains("Requires CudaDevice."));
        assert!(s.summary.contains("Enables PhysicsSim."));
    }

    #[test]
    fn condense_falls_back_to_iri_segment_when_no_label() {
        let s = condense_class(&class("https://ngm/ns/v1#OrphanClass"));
        assert!(s.summary.starts_with("OrphanClass"));
    }

    #[test]
    fn condense_clamps_long_text_on_word_boundary() {
        let mut c = class("https://ngm/ns/v1#Wordy");
        c.preferred_term = Some("Wordy".to_string());
        c.description = Some("lorem ipsum ".repeat(200)); // ~2400 chars
        let s = condense_class(&c);
        assert!(
            s.summary.len() <= SUMMARY_CHAR_BUDGET + 3,
            "clamped ~budget"
        );
        assert!(s.summary.ends_with('…'), "truncation marker present");
        // No mid-word split: the char before the ellipsis is not alphanumeric-cut.
        assert!(!s.summary.contains("loremipsum"));
    }

    #[test]
    fn condense_clamps_multibyte_utf8_without_panic() {
        // Regression: a description whose multi-byte char straddles the byte
        // budget must clamp on a char boundary, not panic on a mid-char slice.
        let mut c = class("https://ngm/ns/v1#Café");
        c.preferred_term = Some("Café".to_string());
        // "café " is 6 bytes / 5 chars; repeating past the budget guarantees a
        // multi-byte 'é' lands on the byte-600 boundary for some repetition.
        c.description = Some("café ".repeat(200));
        let s = condense_class(&c); // must not panic
        assert!(s.summary.ends_with('…'), "truncation marker present");
        assert!(s.summary.is_char_boundary(s.summary.len()));
    }

    #[test]
    fn clamp_words_is_panic_free_across_all_multibyte_offsets() {
        // Exhaustively shift the multibyte char across the budget boundary.
        for pad in 0..8 {
            let text = format!("{}éééééééééé", "a".repeat(pad));
            let _ = clamp_words(&text, 4); // small budget → boundary walk exercised
        }
    }

    #[tokio::test]
    async fn refresh_counts_writes_and_is_failopen() {
        let store = FakeStore {
            fail_keys: vec![key_for_iri("https://ngm/ns/v1#B")],
            ..Default::default()
        };
        let classes = vec![
            class("https://ngm/ns/v1#A"),
            class("https://ngm/ns/v1#B"), // injected failure
            class("https://ngm/ns/v1#C"),
        ];
        let ev = refresh_class_index(&store, "ontology-classes", &classes).await;
        assert_eq!(ev.classes_seen, 3);
        assert_eq!(ev.changed_count, 2, "B failed, A+C written");
        assert_eq!(ev.write_errors, 1);
        assert_eq!(ev.namespace, "ontology-classes");
        assert_eq!(store.writes.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn canary_ok_when_populated() {
        let store = FakeStore::default();
        let classes = vec![class("https://ngm/ns/v1#A")];
        refresh_class_index(&store, "ontology-classes", &classes).await;
        let report = liveness_canary(&store, "ontology-classes", "concept").await;
        assert!(report.ok);
        assert_eq!(report.hits, 1);
        assert!(report.error.is_none());
    }

    #[tokio::test]
    async fn canary_empty_when_unbuilt() {
        let store = FakeStore::default();
        let report = liveness_canary(&store, "ontology-classes", "concept").await;
        assert!(!report.ok, "empty namespace is not live");
        assert_eq!(report.hits, 0);
    }

    #[tokio::test]
    async fn canary_reports_transport_error() {
        let store = FakeStore {
            force_search_err: true,
            ..Default::default()
        };
        let report = liveness_canary(&store, "ontology-classes", "concept").await;
        assert!(!report.ok);
        assert!(report.error.is_some());
    }

    #[test]
    fn hit_count_extraction_handles_shapes() {
        use serde_json::json;
        assert_eq!(extract_hit_count(&json!({"count": 7})), 7);
        assert_eq!(extract_hit_count(&json!({"results": [1, 2, 3]})), 3);
        assert_eq!(extract_hit_count(&json!({"matches": [{}, {}]})), 2);
        assert_eq!(extract_hit_count(&json!([1, 2, 3, 4])), 4);
        assert_eq!(extract_hit_count(&json!({"nothing": true})), 0);
    }

    #[test]
    fn event_serialises() {
        let ev = ClassSummaryIndexRefreshed {
            changed_count: 5,
            classes_seen: 6,
            write_errors: 1,
            namespace: "ontology-classes".to_string(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["changed_count"], 5);
        assert_eq!(v["write_errors"], 1);
    }
}
