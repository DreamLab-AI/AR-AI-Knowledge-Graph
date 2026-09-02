//! Vault page metadata — the single parsing entry point for the authored corpus.
//!
//! Governing document: `docs/VAULT-corpus-format.md` §V1–V5. Decision record:
//! ADR-2040 (supersedes ADR-2014).
//!
//! The authored corpus is an [Obsidian](https://obsidian.md) vault: plain
//! markdown whose metadata carrier is a YAML frontmatter block. This module
//! owns *all* knowledge of that carrier — and of the bounded Logseq
//! `key:: value` tolerance that survives until ADR-2040's `review_trigger`.
//! Every reader in the "Readers and writers" table of the governing document
//! calls [`parse`]; none of them re-implements a line scan.
//!
//! # The inclusion gate (§V4)
//!
//! A page is ingested as a knowledge-graph node iff **either** its frontmatter
//! carries `public: true`, **or** it carries a non-empty `owl-class` (formal
//! data ingests unconditionally). Absence of both means private — the gate is
//! fail-closed, and anchors on parsed metadata, never on the file path. See
//! [`PageMeta::is_kg_included`].
//!
//! # Bounded legacy tolerance (§V4, ADR-2040 D3)
//!
//! Logseq property lines are accepted **only** in the *leading property block*:
//! the run of contiguous `key:: value` lines (optionally `- ` prefixed) at the
//! very top of the file, terminated by the first blank line, heading, code
//! fence, or non-property line. A `public:: true` anywhere else — mid-body, or
//! quoted inside a code fence — no longer counts. This is a deliberate
//! narrowing of the pre-ADR-2040 `FileService::is_public_file`, which matched
//! the property anywhere in the file and therefore leaked private pages that
//! merely *quoted* the marker.

use std::collections::BTreeMap;

/// Which metadata carrier supplied a page's properties.
///
/// Readers log this so the operator can watch the Logseq tail shrink as the
/// corpus converts; when no page reports [`PageFormat::LogseqLegacy`] the
/// tolerance in [`parse`] can be removed (ADR-2040 `review_trigger`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PageFormat {
    /// A YAML frontmatter block delimited by `---` lines opened the file (§V2).
    Obsidian,
    /// No frontmatter, but a leading Logseq `key:: value` property block
    /// carried at least one recognised key.
    LogseqLegacy,
    /// Neither carrier is present — the page has no authored metadata.
    #[default]
    None,
}

/// Parsed page metadata: the union of the frontmatter keys in §V2 that the
/// system acts on, plus every other leading-block key preserved verbatim.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageMeta {
    /// `public` — the publish half of the inclusion gate. A real YAML boolean
    /// in frontmatter; the string `"true"` does **not** count (§V2).
    pub public: bool,
    /// `owl-class` — a formal class IRI (e.g. `mv:Foo`). Non-empty means the
    /// page bypasses the publish gate entirely.
    pub owl_class: Option<String>,
    /// `source-domain` — the domain prefix (ai/bc/mv/rb/tc/ngm).
    pub source_domain: Option<String>,
    /// `aliases` (Obsidian) / `alias::` (Logseq). Empty when absent.
    pub aliases: Vec<String>,
    /// `title` — display title when it differs from the filename.
    pub title: Option<String>,
    /// `elevatedFrom` — the provenance bridge to a working-graph page. Stored
    /// as the bare page name: the `[[…]]` brackets and any `|alias` part are
    /// stripped, so `"[[Working Page|shown]]"` yields `Working Page`.
    pub elevated_from: Option<String>,
    /// `tags` (Obsidian and Logseq). Empty when absent.
    pub tags: Vec<String>,
    /// Every other key in the carrier, preserved verbatim (§V2 "any other
    /// `key`"). Ordered so callers and tests see a stable iteration order.
    pub extra: BTreeMap<String, String>,
    /// Which carrier matched.
    pub format: PageFormat,
}

impl PageMeta {
    /// The §V4 inclusion gate: `public: true` **or** a non-empty `owl-class`.
    ///
    /// Fail-closed — a page with neither is private.
    pub fn is_kg_included(&self) -> bool {
        self.public || self.owl_class.is_some()
    }

    /// Serialise to the YAML body of a §V2 frontmatter block — the `---`
    /// delimiters are added by [`render_page`], which is what writers call.
    ///
    /// Key order is deterministic (the §V2 table order, then `extra` in its
    /// sorted order) so re-writing an unchanged page is a byte-level no-op.
    /// Values are serialised by `serde_yaml`, so `mv:Foo` and `[[Page]]` are
    /// quoted correctly rather than being re-read as YAML structure.
    pub fn to_frontmatter_yaml(&self) -> String {
        use serde_yaml::Value;

        let mut map = serde_yaml::Mapping::new();
        let key = |k: &str| Value::String(k.to_string());

        map.insert(key("public"), Value::Bool(self.public));
        if let Some(ref owl_class) = self.owl_class {
            map.insert(key("owl-class"), Value::String(owl_class.clone()));
        }
        if let Some(ref source_domain) = self.source_domain {
            map.insert(key("source-domain"), Value::String(source_domain.clone()));
        }
        if let Some(ref title) = self.title {
            map.insert(key("title"), Value::String(title.clone()));
        }
        if !self.aliases.is_empty() {
            map.insert(key("aliases"), string_sequence(&self.aliases));
        }
        if !self.tags.is_empty() {
            map.insert(key("tags"), string_sequence(&self.tags));
        }
        if let Some(ref elevated_from) = self.elevated_from {
            // §V2: wikilinks inside property values are quoted strings.
            map.insert(
                key("elevatedFrom"),
                Value::String(format!("[[{}]]", elevated_from)),
            );
        }
        for (extra_key, extra_value) in &self.extra {
            map.insert(key(extra_key), Value::String(extra_value.clone()));
        }

        serde_yaml::to_string(&Value::Mapping(map)).unwrap_or_default()
    }
}

fn string_sequence(items: &[String]) -> serde_yaml::Value {
    serde_yaml::Value::Sequence(
        items
            .iter()
            .map(|item| serde_yaml::Value::String(item.clone()))
            .collect(),
    )
}

/// Parse a page's metadata from its full markdown content.
///
/// Frontmatter wins when present: it must start at the **very first bytes** of
/// the file (`---\n` … `\n---\n`), as Obsidian requires. Otherwise the leading
/// Logseq property block is scanned under the bounded tolerance described in
/// the module docs. A page with neither carrier yields
/// `PageMeta { format: PageFormat::None, .. }` — every field at its default,
/// which the gate reads as private.
pub fn parse(content: &str) -> PageMeta {
    split(content).0
}

/// Parse a page and return its metadata alongside the body that follows the
/// metadata carrier.
///
/// The body is everything after the closing `---` of a frontmatter block, or
/// after the last line of a leading Logseq property block. Writers use this to
/// edit a page's metadata without disturbing its prose and JSON-LD fences —
/// and to convert a legacy page's property block to frontmatter on write
/// (§V5).
pub fn split(content: &str) -> (PageMeta, &str) {
    if let Some((yaml, body)) = split_frontmatter(content) {
        if let Some(meta) = parse_frontmatter(yaml) {
            return (meta, body);
        }
        // Delimiters present but the block is not a YAML mapping. Fall through
        // to the legacy scan, which sees `---` as its first line and therefore
        // finds no property block — the page stays private (fail-closed).
    }
    parse_leading_property_block(content)
}

/// Render a complete vault page: a §V2 frontmatter block followed by `body`.
///
/// This is the **only** sanctioned way to write an authored page (§V5,
/// Invariant 1). Emitting `key:: value` lines from any writer is a violation,
/// and hand-rolling the YAML risks mis-quoting values such as `mv:Foo` or
/// `[[Page]]`, which YAML would otherwise read as a mapping or a flow
/// sequence.
///
/// `render_page` round-trips: `parse(&render_page(&m, body)) == m`.
pub fn render_page(meta: &PageMeta, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 256);
    out.push_str("---\n");
    out.push_str(&meta.to_frontmatter_yaml());
    out.push_str("---\n");

    let body = body.trim_start_matches('\n');
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// Scan **every** `key:: value` line in a page, not just the leading block.
///
/// This is *not* the gate and must never be used as one — it exists solely so
/// that ontology-enrichment readers can keep harvesting the deeply-indented
/// `- ### OntologyBlock` property lists that pre-ADR-2040 writers emitted
/// (`term-id::`, `quality-score::`, `is-subclass-of::`, …). Those blocks sit
/// under a heading, so [`parse`] correctly refuses to see them, and narrowing
/// the enrichment path too would silently drop metadata from ~8.6k existing
/// pages. Pairs are returned in document order, with repeats preserved:
/// `is-subclass-of::` legitimately appears several times on one page, so a map
/// would silently drop all but the last parent.
///
/// Retire this alongside the [`PageFormat::LogseqLegacy`] tolerance.
pub fn legacy_properties_anywhere(content: &str) -> Vec<(String, String)> {
    content
        .lines()
        .filter_map(split_property_line)
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Derive a page's vault identity from a vault-relative path (§V1).
///
/// Strips the `.md` extension and a leading `pages/` segment, then decodes the
/// legacy namespace encodings `___` and `%2F` to `/`. The result is the page
/// name in the `[[Ns/Title]]` form the corpus already links with.
///
/// Identity is stable across the conversion (governing doc Invariant 4):
/// slugification collapses any run of non-alphanumerics to a single `-`, so
/// `A___B Testing`, `A%2FB Testing` and `A/B Testing` all slugify identically.
pub fn page_name_from_path(rel: &str) -> String {
    let trimmed = rel.trim().trim_start_matches('/');
    let without_ext = trimmed.strip_suffix(".md").unwrap_or(trimmed);
    let without_pages = without_ext.strip_prefix("pages/").unwrap_or(without_ext);
    without_pages
        .replace("___", "/")
        .replace("%2F", "/")
        .replace("%2f", "/")
}

// ---------------------------------------------------------------------------
// Frontmatter (§V2)
// ---------------------------------------------------------------------------

/// Return the YAML text of a leading frontmatter block, if the content opens
/// with one. The opening `---` must be the first bytes of the file; the block
/// ends at the first subsequent line that is exactly `---`.
fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content
        .strip_prefix("---\r\n")
        .or_else(|| content.strip_prefix("---\n"))?;

    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']).trim_end() == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

fn parse_frontmatter(yaml: &str) -> Option<PageMeta> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    let mapping = value.as_mapping()?;

    let mut meta = PageMeta {
        format: PageFormat::Obsidian,
        ..PageMeta::default()
    };

    for (key, value) in mapping {
        let Some(key) = key.as_str() else { continue };
        match key {
            // A real YAML boolean, never the string "true" (§V2).
            "public" => meta.public = value.as_bool().unwrap_or(false),
            "owl-class" | "owl:class" => meta.owl_class = yaml_non_empty_string(value),
            "source-domain" => meta.source_domain = yaml_non_empty_string(value),
            "title" => meta.title = yaml_non_empty_string(value),
            "elevatedFrom" | "elevated-from" => {
                meta.elevated_from = yaml_non_empty_string(value).as_deref().map(strip_wikilink)
            }
            "aliases" | "alias" => meta.aliases = yaml_string_list(value),
            "tags" => meta.tags = yaml_string_list(value),
            other => {
                if let Some(rendered) = yaml_render(value) {
                    meta.extra.insert(other.to_string(), rendered);
                }
            }
        }
    }

    Some(meta)
}

/// A YAML scalar as a trimmed string, or `None` when absent/empty/non-scalar.
fn yaml_non_empty_string(value: &serde_yaml::Value) -> Option<String> {
    let rendered = yaml_scalar(value)?;
    let trimmed = rendered.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// A YAML list, or a comma-separated scalar, as a list of trimmed strings.
fn yaml_string_list(value: &serde_yaml::Value) -> Vec<String> {
    match value {
        serde_yaml::Value::Sequence(items) => items
            .iter()
            .filter_map(yaml_non_empty_string)
            .map(|s| strip_wikilink(&s))
            .filter(|s| !s.is_empty())
            .collect(),
        other => yaml_non_empty_string(other)
            .map(|s| split_comma_list(&s))
            .unwrap_or_default(),
    }
}

/// Render any YAML value for the verbatim `extra` map: scalars as themselves,
/// sequences comma-joined, mappings skipped (nothing in §V2 nests).
fn yaml_render(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::Sequence(items) => {
            let joined: Vec<String> = items.iter().filter_map(yaml_scalar).collect();
            (!joined.is_empty()).then(|| joined.join(", "))
        }
        other => yaml_scalar(other),
    }
}

/// The lexical form of a YAML scalar. Non-scalars yield `None`.
fn yaml_scalar(value: &serde_yaml::Value) -> Option<String> {
    match value {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Legacy leading property block (§V4, bounded tolerance)
// ---------------------------------------------------------------------------

fn parse_leading_property_block(content: &str) -> (PageMeta, &str) {
    let mut meta = PageMeta::default();
    let mut matched_known_key = false;
    let mut body_offset = 0usize;

    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();

        // The block is contiguous and leading: a blank line, a heading, or a
        // code fence terminates it, and so does any line that is not a
        // `key:: value` property.
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("```") {
            break;
        }
        let Some((key, value)) = split_property_line(line) else {
            break;
        };
        body_offset += line.len();

        match key {
            "public" | "public-access" => {
                if value.eq_ignore_ascii_case("true") {
                    meta.public = true;
                }
                matched_known_key = true;
            }
            "owl:class" => {
                meta.owl_class = non_empty(value);
                matched_known_key = true;
            }
            "source-domain" => {
                meta.source_domain = non_empty(value);
                matched_known_key = true;
            }
            "title" => {
                meta.title = non_empty(value);
                matched_known_key = true;
            }
            "elevatedFrom" => {
                meta.elevated_from = non_empty(value).as_deref().map(strip_wikilink);
                matched_known_key = true;
            }
            "alias" | "aliases" => {
                meta.aliases = split_comma_list(value);
                matched_known_key = true;
            }
            "tags" => {
                meta.tags = split_comma_list(value);
                matched_known_key = true;
            }
            other => {
                meta.extra.insert(other.to_string(), value.to_string());
            }
        }
    }

    meta.format = if matched_known_key {
        PageFormat::LogseqLegacy
    } else {
        PageFormat::None
    };
    (meta, &content[body_offset..])
}

/// Split a Logseq property line into its trimmed key and value.
///
/// Accepts an optional `- ` outliner prefix and any leading indentation. The
/// key must be non-empty and must not itself contain a `:` — that keeps a
/// prose line such as `see also:: nothing` out of the block. The one dotted
/// key §V2 defines, `owl:class`, is unaffected: its single colons never form
/// the `::` separator, so the first `::` still lands after the whole key.
fn split_property_line(line: &str) -> Option<(&str, &str)> {
    let body = line.trim().trim_start_matches("- ").trim_start();
    let (key, value) = body.split_once("::")?;
    let key = key.trim();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    Some((key, value.trim()))
}

// ---------------------------------------------------------------------------
// Shared value helpers
// ---------------------------------------------------------------------------

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Strip the `[[…]]` wrapper and any `|alias` suffix from a wikilink value,
/// leaving the bare page name. Values that are not wikilinks pass through.
fn strip_wikilink(value: &str) -> String {
    let inner = value
        .trim()
        .strip_prefix("[[")
        .and_then(|rest| rest.split("]]").next())
        .unwrap_or_else(|| value.trim());
    inner
        .split('|')
        .next()
        .unwrap_or(inner)
        .trim()
        .trim_start_matches('#')
        .to_string()
}

/// Split a comma-separated scalar into trimmed, wikilink-stripped entries.
fn split_comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(strip_wikilink)
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- EXP-V01: the publish half of the gate ------------------------------

    #[test]
    fn exp_v01_frontmatter_public_true_is_included() {
        let meta = parse("---\npublic: true\n---\n\n# Page\n\nBody.\n");
        assert!(meta.public);
        assert!(meta.is_kg_included());
        assert_eq!(meta.format, PageFormat::Obsidian);
    }

    #[test]
    fn exp_v01_frontmatter_public_false_is_excluded() {
        let meta = parse("---\npublic: false\n---\n\n# Page\n");
        assert!(!meta.public);
        assert!(!meta.is_kg_included());
        assert_eq!(meta.format, PageFormat::Obsidian);
    }

    #[test]
    fn exp_v01_no_frontmatter_is_excluded() {
        let meta = parse("# Page\n\nJust prose, no metadata anywhere.\n");
        assert!(!meta.is_kg_included());
        assert_eq!(meta.format, PageFormat::None);
    }

    // -- EXP-V02: owl-class bypasses the publish gate ------------------------

    #[test]
    fn exp_v02_owl_class_without_public_is_included() {
        let meta = parse("---\nowl-class: mv:Foo\n---\n\n# Foo\n");
        assert_eq!(meta.owl_class.as_deref(), Some("mv:Foo"));
        assert!(!meta.public);
        assert!(meta.is_kg_included());
    }

    #[test]
    fn owl_class_with_public_false_is_still_included() {
        let meta = parse("---\npublic: false\nowl-class: mv:Foo\n---\n\n# Foo\n");
        assert!(!meta.public);
        assert_eq!(meta.owl_class.as_deref(), Some("mv:Foo"));
        assert!(meta.is_kg_included());
    }

    #[test]
    fn empty_owl_class_does_not_open_the_gate() {
        let meta = parse("---\nowl-class: \"\"\n---\n\n# Foo\n");
        assert_eq!(meta.owl_class, None);
        assert!(!meta.is_kg_included());
    }

    // -- EXP-V03: bounded legacy tolerance -----------------------------------

    #[test]
    fn exp_v03_legacy_leading_public_is_included() {
        let meta = parse("public:: true\nsource-domain:: mv\n\n# Page\n\nBody.\n");
        assert!(meta.public);
        assert!(meta.is_kg_included());
        assert_eq!(meta.source_domain.as_deref(), Some("mv"));
        assert_eq!(meta.format, PageFormat::LogseqLegacy);
    }

    #[test]
    fn exp_v03_legacy_public_after_a_heading_is_excluded() {
        let meta = parse("# Page\n\npublic:: true\n\nBody.\n");
        assert!(!meta.public);
        assert!(!meta.is_kg_included());
        assert_eq!(meta.format, PageFormat::None);
    }

    #[test]
    fn exp_v03_legacy_public_inside_a_code_fence_after_a_heading_is_excluded() {
        let meta = parse("# Example\n\nTo publish a page, write:\n\n```\npublic:: true\n```\n");
        assert!(!meta.public);
        assert!(!meta.is_kg_included());
        assert_eq!(meta.format, PageFormat::None);
    }

    #[test]
    fn legacy_public_mid_body_is_excluded() {
        // The pre-ADR-2040 `is_public_file` matched this anywhere in the file
        // and leaked the page. The narrowing is deliberate (ADR-2040 D3).
        let meta = parse("Some prose first.\n\npublic:: true\n");
        assert!(!meta.is_kg_included());
    }

    #[test]
    fn legacy_public_access_alias_counts_as_public() {
        let meta = parse("public-access:: true\n\n# Page\n");
        assert!(meta.public);
        assert!(meta.is_kg_included());
        assert_eq!(meta.format, PageFormat::LogseqLegacy);
    }

    #[test]
    fn legacy_bullet_prefixed_properties_are_accepted() {
        let meta = parse("- public:: true\n- owl:class:: mv:Foo\n\n# Page\n");
        assert!(meta.public);
        assert_eq!(meta.owl_class.as_deref(), Some("mv:Foo"));
        assert_eq!(meta.format, PageFormat::LogseqLegacy);
    }

    #[test]
    fn legacy_block_stops_at_the_first_non_property_line() {
        let meta = parse("public:: true\nprose interrupts here\nowl:class:: mv:Foo\n");
        assert!(meta.public);
        assert_eq!(meta.owl_class, None, "the block ended before this line");
    }

    // -- `public` must be a real YAML boolean (§V2) --------------------------

    #[test]
    fn frontmatter_public_as_a_string_does_not_count() {
        let meta = parse("---\npublic: \"true\"\n---\n\n# Page\n");
        assert!(!meta.public, "the string \"true\" is not a YAML boolean");
        assert!(!meta.is_kg_included());
        assert_eq!(meta.format, PageFormat::Obsidian);
        assert_eq!(meta.extra.get("public"), None);
    }

    // -- elevatedFrom --------------------------------------------------------

    #[test]
    fn frontmatter_elevated_from_strips_brackets_and_alias() {
        let meta = parse("---\npublic: true\nelevatedFrom: \"[[Working Page|alias]]\"\n---\n");
        assert_eq!(meta.elevated_from.as_deref(), Some("Working Page"));
    }

    #[test]
    fn frontmatter_elevated_from_without_an_alias() {
        let meta = parse("---\nelevatedFrom: \"[[Working Page]]\"\n---\n");
        assert_eq!(meta.elevated_from.as_deref(), Some("Working Page"));
    }

    #[test]
    fn legacy_elevated_from_strips_brackets_and_alias() {
        let meta = parse("public:: true\nelevatedFrom:: [[Working Page|alias]]\n");
        assert_eq!(meta.elevated_from.as_deref(), Some("Working Page"));
    }

    // -- aliases / tags: list or comma-separated scalar ----------------------

    #[test]
    fn frontmatter_aliases_and_tags_accept_yaml_lists() {
        let meta = parse("---\npublic: true\naliases:\n  - One\n  - Two\ntags:\n  - alpha\n  - beta\n---\n");
        assert_eq!(meta.aliases, vec!["One", "Two"]);
        assert_eq!(meta.tags, vec!["alpha", "beta"]);
    }

    #[test]
    fn frontmatter_aliases_and_tags_accept_comma_separated_scalars() {
        let meta = parse("---\npublic: true\naliases: One, Two\ntags: alpha, beta\n---\n");
        assert_eq!(meta.aliases, vec!["One", "Two"]);
        assert_eq!(meta.tags, vec!["alpha", "beta"]);
    }

    #[test]
    fn legacy_alias_and_tags_are_comma_separated_and_unwrap_wikilinks() {
        let meta = parse("public:: true\nalias:: [[One]], [[Two]]\ntags:: #alpha, beta\n");
        assert_eq!(meta.aliases, vec!["One", "Two"]);
        assert_eq!(meta.tags, vec!["alpha", "beta"]);
    }

    // -- extra ---------------------------------------------------------------

    #[test]
    fn unrecognised_keys_are_preserved_verbatim_in_extra() {
        let meta = parse("public:: true\nterm-id:: mv-0042\nmaturity:: draft\n");
        assert_eq!(meta.extra.get("term-id").map(String::as_str), Some("mv-0042"));
        assert_eq!(meta.extra.get("maturity").map(String::as_str), Some("draft"));
    }

    #[test]
    fn frontmatter_unrecognised_keys_are_preserved_in_extra() {
        let meta = parse("---\npublic: true\nquality-score: 0.6\nmaturity: draft\n---\n");
        assert_eq!(meta.extra.get("maturity").map(String::as_str), Some("draft"));
        assert_eq!(
            meta.extra.get("quality-score").map(String::as_str),
            Some("0.6")
        );
    }

    // -- frontmatter must be the very first bytes ----------------------------

    #[test]
    fn frontmatter_not_at_the_start_of_the_file_is_not_frontmatter() {
        let meta = parse("\n---\npublic: true\n---\n\n# Page\n");
        assert!(!meta.public, "a blank first line disqualifies the block");
        assert_eq!(meta.format, PageFormat::None);
    }

    #[test]
    fn unterminated_frontmatter_is_not_frontmatter() {
        let meta = parse("---\npublic: true\n\n# Page with no closing delimiter\n");
        assert!(!meta.public);
        assert!(!meta.is_kg_included());
    }

    #[test]
    fn malformed_frontmatter_yaml_fails_closed() {
        let meta = parse("---\n\tpublic: [unclosed\n---\n\n# Page\n");
        assert!(!meta.is_kg_included());
    }

    #[test]
    fn crlf_frontmatter_is_accepted() {
        let meta = parse("---\r\npublic: true\r\n---\r\n\r\n# Page\r\n");
        assert!(meta.public);
        assert_eq!(meta.format, PageFormat::Obsidian);
    }

    // -- legacy_properties_anywhere (enrichment only, never the gate) --------

    #[test]
    fn legacy_properties_anywhere_sees_indented_ontology_blocks() {
        let page = "- Foo\n  - ### OntologyBlock\n    - term-id:: mv-0001\n    - owl:class:: mv:Foo\n";
        let props = legacy_properties_anywhere(page);
        assert_eq!(
            props,
            vec![
                ("term-id".to_string(), "mv-0001".to_string()),
                ("owl:class".to_string(), "mv:Foo".to_string()),
            ]
        );
        // …while the gate correctly refuses to see them.
        assert!(!parse(page).is_kg_included());
    }

    #[test]
    fn legacy_properties_anywhere_preserves_repeated_keys() {
        let page = "- Foo\n  - ### OntologyBlock\n    - is-subclass-of:: [[A]]\n    - is-subclass-of:: [[B]]\n";
        let props = legacy_properties_anywhere(page);
        let values: Vec<&str> = props
            .iter()
            .filter(|(k, _)| k == "is-subclass-of")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(values, vec!["[[A]]", "[[B]]"]);
    }

    // -- the writer half (§V5, Invariant 1) ----------------------------------

    #[test]
    fn render_page_round_trips_through_parse() {
        let meta = PageMeta {
            public: true,
            owl_class: Some("mv:Foo".to_string()),
            source_domain: Some("mv".to_string()),
            aliases: vec!["One".to_string(), "Two".to_string()],
            title: Some("Foo: A Study".to_string()),
            elevated_from: Some("Working Page".to_string()),
            tags: vec!["alpha".to_string()],
            extra: [
                ("maturity".to_string(), "draft".to_string()),
                ("quality-score".to_string(), "0.6".to_string()),
            ]
            .into_iter()
            .collect(),
            format: PageFormat::Obsidian,
        };
        let page = render_page(&meta, "# Foo\n\nBody prose.\n");
        assert_eq!(parse(&page), meta);
    }

    #[test]
    fn render_page_quotes_values_yaml_would_otherwise_reinterpret() {
        let meta = PageMeta {
            public: true,
            owl_class: Some("mv:Foo".to_string()),
            elevated_from: Some("Working Page".to_string()),
            ..PageMeta::default()
        };
        let page = render_page(&meta, "# Foo\n");
        let reparsed = parse(&page);
        assert_eq!(reparsed.owl_class.as_deref(), Some("mv:Foo"));
        assert_eq!(reparsed.elevated_from.as_deref(), Some("Working Page"));
        assert!(reparsed.is_kg_included());
    }

    #[test]
    fn render_page_is_idempotent() {
        let meta = PageMeta {
            public: true,
            title: Some("Foo".to_string()),
            ..PageMeta::default()
        };
        let once = render_page(&meta, "# Foo\n");
        let (reparsed, body) = split(&once);
        assert_eq!(render_page(&reparsed, body), once);
    }

    #[test]
    fn split_returns_the_body_after_frontmatter() {
        let (meta, body) = split("---\npublic: true\n---\n\n# Page\n\nBody.\n");
        assert!(meta.public);
        assert_eq!(body, "\n# Page\n\nBody.\n");
    }

    #[test]
    fn split_returns_the_body_after_a_legacy_property_block() {
        let (meta, body) = split("public:: true\nowl:class:: mv:Foo\n\n# Page\n\nBody.\n");
        assert!(meta.public);
        assert_eq!(body, "\n# Page\n\nBody.\n");
    }

    #[test]
    fn split_returns_the_whole_page_when_there_is_no_carrier() {
        let (meta, body) = split("# Page\n\nBody.\n");
        assert_eq!(meta.format, PageFormat::None);
        assert_eq!(body, "# Page\n\nBody.\n");
    }

    #[test]
    fn a_legacy_page_converts_to_frontmatter_on_write() {
        // §V5: a writer that must touch a legacy page converts its leading
        // property block. The gate verdict must survive the conversion.
        let legacy = "public:: true\nowl:class:: mv:Foo\nalias:: [[Bar]]\n\n# Foo\n\nProse.\n";
        let (meta, body) = split(legacy);
        let converted = render_page(&meta, body);
        assert!(converted.starts_with("---\n"));
        assert!(!converted.contains(":: "), "no `key:: value` line survives");
        let reparsed = parse(&converted);
        assert_eq!(reparsed.format, PageFormat::Obsidian);
        assert!(reparsed.is_kg_included());
        assert_eq!(reparsed.owl_class.as_deref(), Some("mv:Foo"));
        assert_eq!(reparsed.aliases, vec!["Bar"]);
        assert!(converted.ends_with("# Foo\n\nProse.\n"));
    }

    // -- page_name_from_path (§V1) -------------------------------------------

    #[test]
    fn page_name_decodes_triple_underscore_namespaces() {
        assert_eq!(page_name_from_path("A___B Testing.md"), "A/B Testing");
    }

    #[test]
    fn page_name_strips_a_leading_pages_segment() {
        assert_eq!(
            page_name_from_path("pages/ETSI_Domain_Governance___Economy.md"),
            "ETSI_Domain_Governance/Economy"
        );
    }

    #[test]
    fn page_name_decodes_percent_encoded_slashes() {
        assert_eq!(page_name_from_path("A%2FB.md"), "A/B");
        assert_eq!(page_name_from_path("A%2fB.md"), "A/B");
    }

    #[test]
    fn page_name_passes_through_folder_namespaces() {
        assert_eq!(page_name_from_path("Ns/Title.md"), "Ns/Title");
        assert_eq!(page_name_from_path("pages/Ns/Title.md"), "Ns/Title");
    }

    #[test]
    fn page_name_without_an_extension_is_unchanged() {
        assert_eq!(page_name_from_path("Plain Page"), "Plain Page");
    }
}
