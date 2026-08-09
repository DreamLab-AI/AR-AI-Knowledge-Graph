// src/services/jsonld_ingest/vocab_resolver.rs
//! `owl:imports` auto-discovery + PURL resolution (ADR-100 D4).
//!
//! When an ingested document declares `owl:imports <iri>`, the pipeline
//! resolves the referenced vocabulary through the well-known-vocabulary
//! registry ([`crate::services::vocab_registry`]) and, on a hit, fetches the
//! ontology document via its PURL — caching the result so a vocabulary is
//! fetched at most once per process.
//!
//! ## Non-fatal by construction (ADR-100 D4)
//!
//! > "PURL resolution adds a network dependency at ingest; mitigated by
//! > caching resolved vocabularies and treating resolution failure as
//! > non-fatal (vocab terms degrade to local mints with a logged warning)."
//!
//! Every resolution returns an [`ImportResolution`] — it never returns `Err`
//! and never panics. A failed fetch, an unknown import, or a network error
//! all degrade to [`ImportResolution::LocalMint`] with a `log::warn!`. Ingest
//! is therefore never blocked on the network.
//!
//! ## Injectable fetcher (tests never hit the network)
//!
//! The HTTP layer is the [`VocabFetcher`] trait. Production uses
//! [`ReqwestFetcher`]; tests inject a [`StubFetcher`] backed by a fixture map
//! so the success and failure paths are exercised deterministically and
//! offline.

use std::collections::HashMap;
use std::sync::Mutex;

use super::expander::{ExpandedDocument, ExpandedValue, OWL_NS};
use crate::services::vocab_registry::{self, VocabularyRegistration};

/// The `owl:imports` predicate, fully expanded (the expander turns
/// `owl:imports` into this absolute IRI).
pub fn owl_imports_iri() -> String {
    format!("{OWL_NS}imports")
}

/// Outcome of resolving one `owl:imports` target. Resolution NEVER errors —
/// a miss degrades to [`ImportResolution::LocalMint`] (ADR-100 D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportResolution {
    /// The import resolved to a registered vocabulary AND its document was
    /// fetched (or served from cache). Carries the registration and the
    /// fetched document body (so a future serialiser / reasoner can load it).
    Resolved {
        registration: VocabularyRegistration,
        /// Document body fetched from the PURL (or cache).
        document: String,
        /// True if this came from the in-process cache rather than a fetch.
        cached: bool,
    },
    /// The import IRI is a known vocabulary but its PURL could not be fetched
    /// (network/HTTP failure). The term still resolves to its *canonical
    /// namespace* via the registry — only the document body is unavailable.
    /// Non-fatal: callers keep the canonical IRI; nothing is blocked.
    NamespaceOnly {
        registration: VocabularyRegistration,
    },
    /// The import is not in the registry at all. The vocab term degrades to a
    /// local mint; a warning has been logged. Non-fatal (ADR-100 D4).
    LocalMint { target: String },
}

/// Injectable document-fetch layer. The single method maps a PURL to its body.
/// `Err` signals a *fetch* failure (network/HTTP), which the resolver treats
/// as non-fatal and downgrades to [`ImportResolution::NamespaceOnly`].
pub trait VocabFetcher: Send + Sync {
    /// Fetch the ontology document at `purl`. Returns the body on success.
    fn fetch(&self, purl: &str) -> Result<String, String>;
}

/// Production fetcher: a blocking reqwest GET with an Accept header covering
/// the common ontology serialisations. Kept deliberately small; the resolver
/// owns retry/cache policy.
pub struct ReqwestFetcher {
    client: reqwest::blocking::Client,
}

impl ReqwestFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .user_agent("visionclaw-ontology/owl-imports")
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
        }
    }
}

impl Default for ReqwestFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl VocabFetcher for ReqwestFetcher {
    fn fetch(&self, purl: &str) -> Result<String, String> {
        let resp = self
            .client
            .get(purl)
            .header(
                "Accept",
                "application/rdf+xml, text/turtle, application/ld+json, application/n-triples, */*",
            )
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        resp.text().map_err(|e| e.to_string())
    }
}

/// A test/offline fetcher backed by a fixture map (PURL → body). Any PURL not
/// in the map fails (exercising the non-fatal degrade path). NEVER touches the
/// network.
pub struct StubFetcher {
    fixtures: HashMap<String, String>,
}

impl StubFetcher {
    pub fn new() -> Self {
        Self {
            fixtures: HashMap::new(),
        }
    }

    /// Register a fixture body for a PURL.
    pub fn with_fixture(mut self, purl: impl Into<String>, body: impl Into<String>) -> Self {
        self.fixtures.insert(purl.into(), body.into());
        self
    }
}

impl Default for StubFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl VocabFetcher for StubFetcher {
    fn fetch(&self, purl: &str) -> Result<String, String> {
        self.fixtures
            .get(purl)
            .cloned()
            .ok_or_else(|| format!("no fixture for {purl}"))
    }
}

/// Resolves `owl:imports` targets through the registry + an injected fetcher,
/// caching fetched documents so each PURL is fetched at most once.
pub struct VocabResolver<F: VocabFetcher> {
    fetcher: F,
    /// PURL → fetched body. Guarded so the resolver is `Sync`.
    cache: Mutex<HashMap<String, String>>,
}

impl<F: VocabFetcher> VocabResolver<F> {
    pub fn new(fetcher: F) -> Self {
        Self {
            fetcher,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Auto-discover every `owl:imports` target in a document and resolve each.
    /// Returns one [`ImportResolution`] per discovered import, in document
    /// order. Never errors (ADR-100 D4).
    pub fn resolve_document_imports(&self, doc: &ExpandedDocument) -> Vec<ImportResolution> {
        discover_imports(doc)
            .into_iter()
            .map(|t| self.resolve_one(&t))
            .collect()
    }

    /// Resolve a single import target. Registry miss → `LocalMint` (warn);
    /// registry hit but fetch failure → `NamespaceOnly` (warn); full success
    /// → `Resolved`.
    pub fn resolve_one(&self, target: &str) -> ImportResolution {
        let registration = match vocab_registry::resolve_import_target(target) {
            Some(r) => r.clone(),
            None => {
                log::warn!(
                    "owl:imports <{target}> is not a registered well-known vocabulary — \
                     degrading to local mint (ADR-100 D4, non-fatal)"
                );
                return ImportResolution::LocalMint {
                    target: target.to_string(),
                };
            }
        };

        // Cache hit?
        if let Ok(cache) = self.cache.lock() {
            if let Some(body) = cache.get(registration.purl) {
                return ImportResolution::Resolved {
                    registration,
                    document: body.clone(),
                    cached: true,
                };
            }
        }

        match self.fetcher.fetch(registration.purl) {
            Ok(body) => {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.insert(registration.purl.to_string(), body.clone());
                }
                ImportResolution::Resolved {
                    registration,
                    document: body,
                    cached: false,
                }
            }
            Err(err) => {
                log::warn!(
                    "owl:imports <{}> resolved to vocabulary '{}' but PURL fetch failed ({}) — \
                     keeping canonical namespace, no document loaded (ADR-100 D4, non-fatal)",
                    target,
                    registration.prefix,
                    err
                );
                ImportResolution::NamespaceOnly { registration }
            }
        }
    }
}

/// Auto-discovery: scan every node's fields for `owl:imports` and collect the
/// target IRIs. Handles single-IRI, multi (array), and `{@id}` reference
/// shapes. Pure; no IO.
pub fn discover_imports(doc: &ExpandedDocument) -> Vec<String> {
    let imports_pred = owl_imports_iri();
    let mut out = Vec::new();
    for node in &doc.nodes {
        for (pred, value) in &node.fields {
            if pred == &imports_pred {
                collect_iri_targets(value, &mut out);
            }
        }
    }
    out
}

fn collect_iri_targets(value: &ExpandedValue, out: &mut Vec<String>) {
    match value {
        ExpandedValue::Iri(iri) => out.push(iri.clone()),
        ExpandedValue::Multi(arr) => {
            for v in arr {
                collect_iri_targets(v, out);
            }
        }
        ExpandedValue::Nested(node) => {
            if let Some(iri) = &node.id {
                out.push(iri.clone());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::jsonld_ingest::expander::expand_block;

    const PROV_PURL: &str = "http://www.w3.org/ns/prov-o";

    fn doc_importing(target: &str) -> ExpandedDocument {
        let body = format!(
            r#"{{
              "@context": "https://narrativegoldmine.com/context/v1.jsonld",
              "@id": "urn:visionclaw:owl:class:built-environment",
              "@type": "OntologyClass",
              "owl:imports": {{ "@id": "{target}" }},
              "prov:wasAttributedTo": {{ "@id": "did:nostr:npub1abc" }},
              "prov:generatedAtTime": {{ "@value": "2026-01-01T00:00:00Z", "@type": "xsd:dateTime" }}
            }}"#
        );
        expand_block("imports.md", 0, &body).unwrap()
    }

    #[test]
    fn discovers_owl_imports_target() {
        let doc = doc_importing(PROV_PURL);
        let imports = discover_imports(&doc);
        assert_eq!(imports, vec![PROV_PURL.to_string()]);
    }

    #[test]
    fn success_path_loads_terms_from_stub_fixture() {
        // Stubbed fetcher with a fixture body — no network.
        let fetcher = StubFetcher::new().with_fixture(PROV_PURL, "Ontology(<prov-o> ...)");
        let resolver = VocabResolver::new(fetcher);
        let doc = doc_importing(PROV_PURL);

        let results = resolver.resolve_document_imports(&doc);
        assert_eq!(results.len(), 1);
        match &results[0] {
            ImportResolution::Resolved {
                registration,
                document,
                cached,
            } => {
                assert_eq!(registration.prefix, "prov");
                assert!(document.contains("prov-o"));
                assert!(!cached, "first fetch is not cached");
            }
            other => panic!("expected Resolved, got {other:?}"),
        }

        // Second resolution of the same PURL is served from cache.
        let again = resolver.resolve_one(PROV_PURL);
        match again {
            ImportResolution::Resolved { cached, .. } => assert!(cached, "second hit is cached"),
            other => panic!("expected cached Resolved, got {other:?}"),
        }
    }

    #[test]
    fn failure_path_degrades_to_namespace_only_without_erroring() {
        // Empty stub → fetch fails. Registry still knows the vocab, so we keep
        // the canonical namespace but load no document. NON-FATAL.
        let resolver = VocabResolver::new(StubFetcher::new());
        let doc = doc_importing(PROV_PURL);

        let results = resolver.resolve_document_imports(&doc);
        assert_eq!(results.len(), 1);
        match &results[0] {
            ImportResolution::NamespaceOnly { registration } => {
                assert_eq!(registration.prefix, "prov");
            }
            other => panic!("expected NamespaceOnly, got {other:?}"),
        }
    }

    #[test]
    fn unknown_import_degrades_to_local_mint_without_erroring() {
        let resolver = VocabResolver::new(StubFetcher::new());
        let doc = doc_importing("http://example.com/not-a-known-vocab");

        let results = resolver.resolve_document_imports(&doc);
        assert_eq!(results.len(), 1);
        match &results[0] {
            ImportResolution::LocalMint { target } => {
                assert_eq!(target, "http://example.com/not-a-known-vocab");
            }
            other => panic!("expected LocalMint, got {other:?}"),
        }
    }
}
