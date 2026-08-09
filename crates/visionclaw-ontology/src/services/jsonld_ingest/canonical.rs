//! Canonical entity extraction from JSON-LD-bearing markdown.
//!
//! Reuses the existing `extract_jsonld_blocks` + `expand_block` pipeline,
//! then distills the expanded nodes into a single [`CanonicalEntity`] keyed
//! by `vc:slug`.
//!
//! One file → one canonical entity. The entity rolls up:
//! - identity (`vc:slug`) from the `@type: Page` block,
//! - title from the `title` or `rdfs:label` field,
//! - kind from whether `@type: Class` / `NamedIndividual` blocks are present,
//! - outbound wikilinks from `vc:outboundWikilinks`.
//!
//! See ADR-090 Phase B for the architectural rationale (replacing the
//! filename-hash + body-regex + side-channel JSON-LD paths with a single
//! JSON-LD-first pass).

use visionclaw_domain::models::canonical_entity::{CanonicalEntity, EntityKind, OutboundLink};

use super::errors::Result;
use super::expander::{expand_block, expand_iri_or_keep, slugify, ExpandedNode, ExpandedValue};
use super::extractor::extract_jsonld_blocks;

/// Predicates we look for inside an expanded node, expressed in short form.
/// `node.fields` predicates are always stored expanded (`expand_iri_or_keep`),
/// so [`pred_matches`] expands these candidates through the same function —
/// the expander is the single source of truth and namespace-version drift
/// cannot desync the two lists.
const VC_SLUG_PREDICATES: &[&str] = &["vc:slug"];
const VC_PUBLIC_PREDICATES: &[&str] = &["vc:public"];
const VC_OUTBOUND_PREDICATES: &[&str] = &["vc:outboundWikilinks"];
const VC_LABEL_PREDICATES: &[&str] = &["vc:label"];
const TITLE_PREDICATES: &[&str] = &["title", "rdfs:label", "label"];

/// Match a node-field predicate (stored expanded) against short-form
/// candidates, expanding each through the expander so the two never diverge.
fn pred_matches(pred: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|c| pred == *c || pred == expand_iri_or_keep(c))
}

/// Type-token detection. JSON-LD `@type` values are expanded the same way as
/// predicates; we match on the short forms produced by the source documents
/// (most blocks in the corpus use unprefixed `Page` / `Class` / `NamedIndividual`
/// because the host context defines the default vocabulary).
fn has_type(types: &[String], tokens: &[&str]) -> bool {
    types.iter().any(|t| {
        tokens.iter().any(|tok| {
            t == tok
                || t.ends_with(&format!(":{tok}"))
                || t.ends_with(&format!("/{tok}"))
                || t.ends_with(&format!("#{tok}"))
        })
    })
}

/// Find the first literal/iri value for the given predicates.
fn find_first_literal(node: &ExpandedNode, preds: &[&str]) -> Option<String> {
    for (pred, val) in &node.fields {
        if !pred_matches(pred, preds) {
            continue;
        }
        if let Some(s) = literal_or_iri(val) {
            return Some(s);
        }
    }
    None
}

fn literal_or_iri(v: &ExpandedValue) -> Option<String> {
    match v {
        ExpandedValue::Literal { value, .. } => Some(value.clone()),
        ExpandedValue::Iri(s) => Some(s.clone()),
        ExpandedValue::Multi(items) => items.iter().find_map(literal_or_iri),
        _ => None,
    }
}

fn truthy_literal(v: &ExpandedValue) -> bool {
    match v {
        ExpandedValue::Literal { value, .. } => {
            matches!(value.to_lowercase().as_str(), "true" | "1" | "yes")
        }
        ExpandedValue::Iri(_) => true,
        ExpandedValue::Multi(items) => items.iter().any(truthy_literal),
        _ => false,
    }
}

/// Collect outbound wikilinks from a Page node. Each link is a nested object
/// `{@id: "...", vc:label: "..."}`.
fn collect_outbound_links(node: &ExpandedNode) -> Vec<OutboundLink> {
    let mut out = Vec::new();
    for (pred, val) in &node.fields {
        if !pred_matches(pred, VC_OUTBOUND_PREDICATES) {
            continue;
        }
        push_link_value(val, &mut out);
    }
    out
}

fn push_link_value(val: &ExpandedValue, out: &mut Vec<OutboundLink>) {
    match val {
        ExpandedValue::Multi(items) => {
            for item in items {
                push_link_value(item, out);
            }
        }
        ExpandedValue::Nested(node) => {
            if let Some(iri) = &node.id {
                let label = find_first_literal(node, VC_LABEL_PREDICATES).unwrap_or_default();
                let slug = slug_from_iri(iri);
                out.push(OutboundLink {
                    target_slug: slug,
                    target_label: if label.is_empty() { iri.clone() } else { label },
                    target_iri: iri.clone(),
                });
            }
        }
        ExpandedValue::Iri(iri) => {
            out.push(OutboundLink {
                target_slug: slug_from_iri(iri),
                target_label: iri.clone(),
                target_iri: iri.clone(),
            });
        }
        _ => {}
    }
}

/// Extract a slug from an IRI by taking the segment after the last `:` or `/`
/// and slugifying it. Matches the upstream pipeline's convention.
pub fn slug_from_iri(iri: &str) -> String {
    let after_colon = iri.rsplit_once(':').map(|(_, r)| r).unwrap_or(iri);
    let after_slash = after_colon
        .rsplit_once('/')
        .map(|(_, r)| r)
        .unwrap_or(after_colon);
    slugify(after_slash)
}

/// Parse a markdown file's JSON-LD blocks and distill them into a single
/// canonical entity.
///
/// Returns `Ok(None)` when the file has no JSON-LD blocks at all (skip).
/// Returns `Ok(Some(entity))` for any file that has a `@type: Page` block
/// (with `vc:slug`). Files that have only a `Class` block without a `Page`
/// block are still ingested — the Class block's `@id` provides identity.
///
/// Validation failures (missing `@context`, malformed JSON) propagate as
/// errors — these are corpus integrity issues, not silent skips.
pub fn parse_canonical_entity(
    markdown: &str,
    source_path: &str,
) -> Result<Option<CanonicalEntity>> {
    let blocks = extract_jsonld_blocks(markdown);
    if blocks.is_empty() {
        return Ok(None);
    }

    // Expand every block and collect the nodes.
    let mut all_nodes: Vec<ExpandedNode> = Vec::new();
    for block in &blocks {
        // Some non-vocabulary blocks (e.g. an `Axiom` fragment) may fail
        // validation but still hold useful structural data; we use the
        // expander (which is permissive) and skip validation here — the
        // separate `ingest_page` call handles strict validation for quad
        // persistence.
        let doc = expand_block(source_path, block.index, &block.body)?;
        all_nodes.extend(doc.nodes);
    }

    // Locate the Page block (carries identity) and any Class/Individual block.
    let page_node = all_nodes.iter().find(|n| has_type(&n.types, &["Page"]));
    let class_node = all_nodes.iter().find(|n| {
        has_type(&n.types, &["Class", "owl:Class"])
            || has_type(&n.types, &["NamedIndividual", "owl:NamedIndividual"])
    });

    // Identity rules:
    //   - Prefer the Page block's `vc:slug` (the upstream-authoritative key).
    //   - Fall back to the Class block's @id local-name if no Page block.
    //   - If neither yields a slug, skip the file (corpus error logged elsewhere).
    let (slug, page_iri) = match page_node {
        Some(n) => {
            let slug = find_first_literal(n, VC_SLUG_PREDICATES)
                .or_else(|| n.id.as_deref().map(slug_from_iri));
            let iri = n.id.clone().unwrap_or_default();
            (slug, iri)
        }
        None => (
            class_node.and_then(|n| n.id.as_deref().map(slug_from_iri)),
            String::new(),
        ),
    };

    let Some(slug) = slug.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    let kind = if class_node.is_some() {
        if class_node
            .map(|n| has_type(&n.types, &["NamedIndividual", "owl:NamedIndividual"]))
            .unwrap_or(false)
        {
            EntityKind::OntologyIndividual
        } else {
            EntityKind::OntologyClass
        }
    } else {
        EntityKind::KgPage
    };

    let title = page_node
        .and_then(|n| find_first_literal(n, TITLE_PREDICATES))
        .or_else(|| class_node.and_then(|n| find_first_literal(n, TITLE_PREDICATES)))
        .unwrap_or_else(|| slug.clone());

    let public = page_node
        .map(|n| {
            n.fields
                .iter()
                .any(|(p, v)| pred_matches(p, VC_PUBLIC_PREDICATES) && truthy_literal(v))
        })
        .unwrap_or(true);

    let outbound_links = page_node.map(collect_outbound_links).unwrap_or_default();

    let class_iri = class_node.and_then(|n| n.id.clone());

    Ok(Some(CanonicalEntity {
        slug,
        page_iri,
        class_iri,
        title,
        public,
        kind,
        outbound_links,
        source_path: source_path.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_camera() -> &'static str {
        r#"public:: true

# Camera
```json-ld
{
  "@context": "https://narrativegoldmine.com/context/v1.jsonld",
  "@id": "urn:visionflow:page:abc",
  "@type": "Page",
  "vc:slug": "camera",
  "title": "Camera",
  "vc:public": true,
  "vc:outboundWikilinks": [
    {"@id": "urn:visionflow:linked:image-sensor", "vc:label": "ImageSensor"},
    {"@id": "urn:visionflow:owl:class:sensor", "vc:label": "Sensor"}
  ],
  "prov:wasAttributedTo": {"@id": "did:nostr:test"},
  "prov:generatedAtTime": {"@value": "2026-01-01T00:00:00Z", "@type": "xsd:dateTime"}
}
```

```json-ld
{
  "@context": "https://narrativegoldmine.com/context/v1.jsonld",
  "@id": "urn:ngm:class:camera",
  "@type": "Class",
  "label": "Camera",
  "definition": "An imaging sensor device."
}
```
"#
    }

    #[test]
    fn parses_canonical_entity_from_camera_sample() {
        let e = parse_canonical_entity(sample_camera(), "Camera.md")
            .unwrap()
            .expect("entity");
        assert_eq!(e.slug, "camera");
        assert_eq!(e.kind, EntityKind::OntologyClass);
        assert_eq!(e.title, "Camera");
        assert!(e.public);
        assert_eq!(e.outbound_links.len(), 2);
        assert!(e
            .outbound_links
            .iter()
            .any(|l| l.target_slug == "image-sensor"));
        assert!(e.outbound_links.iter().any(|l| l.target_slug == "sensor"));
        assert_eq!(e.class_iri.as_deref(), Some("urn:ngm:class:camera"));
    }

    #[test]
    fn returns_none_when_no_jsonld_blocks_present() {
        let md = "# Plain markdown\n\nNo JSON-LD here.";
        let out = parse_canonical_entity(md, "plain.md").unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn classifies_kgpage_when_no_class_block() {
        let md = r#"public:: true

# Bitcoin
```json-ld
{
  "@context": "https://narrativegoldmine.com/context/v1.jsonld",
  "@id": "urn:visionflow:page:bitcoin",
  "@type": "Page",
  "vc:slug": "bitcoin",
  "title": "Bitcoin",
  "vc:public": true,
  "prov:wasAttributedTo": {"@id": "did:nostr:test"},
  "prov:generatedAtTime": {"@value": "2026-01-01T00:00:00Z", "@type": "xsd:dateTime"}
}
```
"#;
        let e = parse_canonical_entity(md, "Bitcoin.md").unwrap().unwrap();
        assert_eq!(e.kind, EntityKind::KgPage);
        assert_eq!(e.slug, "bitcoin");
    }

    #[test]
    fn slug_from_iri_handles_urn_and_camel_cased_local_names() {
        assert_eq!(slug_from_iri("urn:ngm:class:camera"), "camera");
        assert_eq!(
            slug_from_iri("urn:visionflow:linked:image-sensor"),
            "image-sensor"
        );
    }
}
