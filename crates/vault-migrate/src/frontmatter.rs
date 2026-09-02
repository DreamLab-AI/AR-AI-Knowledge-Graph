//! Leading property block -> Obsidian Properties (governing doc V2).
//!
//! Only the *leading* block converts. A `key:: value` line after the first
//! blank line, heading, or non-property line is body content: preserved
//! verbatim and reported (V3, and the fail-closed reading of EXP-V03).

use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// `-? key:: value` — the optional leading `- ` is Logseq's outliner bullet.
pub fn prop_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\s*(?:-\s*)?([A-Za-z0-9_.:-]+)::\s*(.*)$").unwrap())
}

fn iso_date_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2})?(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)?$",
        )
        .unwrap()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    Scalar(String),
    List(Vec<String>),
}

/// What happened while mapping a leading block, for the report.
#[derive(Debug, Default, Clone)]
pub struct FmStats {
    pub public_true: bool,
    pub has_aliases: bool,
    pub collapsed_dropped: usize,
    pub id_dropped: usize,
}

#[derive(Debug, Default, Clone)]
pub struct Frontmatter {
    pub map: BTreeMap<String, Value>,
}

impl Frontmatter {
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Insert only when the key is absent — the merge rule "existing wins".
    fn insert_default(&mut self, k: &str, v: Value) {
        self.map.entry(k.to_string()).or_insert(v);
    }
}

fn split_commas(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Strip the tag sigils Logseq allows in a `tags::` value: `#foo`, `[[foo]]`,
/// `#[[foo bar]]`.
fn clean_tag(raw: &str) -> String {
    let mut s = raw.trim();
    s = s.strip_prefix('#').unwrap_or(s);
    s = s.strip_prefix("[[").unwrap_or(s);
    s = s.strip_suffix("]]").unwrap_or(s);
    s.trim().to_string()
}

/// Parse the contiguous leading property block.
///
/// Returns the ordered `(key, raw_value)` pairs and the number of lines
/// consumed. Stops at the first blank line, heading, or non-property line.
pub fn parse_leading_block(lines: &[&str]) -> (Vec<(String, String)>, usize) {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            break;
        }
        if l.trim_start().starts_with('#') {
            break;
        }
        match prop_re().captures(l) {
            Some(c) => {
                out.push((c[1].to_string(), c[2].trim().to_string()));
                i += 1;
            }
            None => break,
        }
    }
    (out, i)
}

/// Apply the V2 key mapping to an ordered leading block.
pub fn map_properties(pairs: &[(String, String)]) -> (Frontmatter, FmStats) {
    let mut fm = Frontmatter::default();
    let mut st = FmStats::default();
    let mut public_explicit = false;

    for (key, raw) in pairs {
        match key.as_str() {
            "public" => {
                let v = raw.trim().eq_ignore_ascii_case("true");
                fm.map.insert("public".into(), Value::Bool(v));
                public_explicit = true;
                st.public_true = v;
            }
            "public-access" => {
                if !public_explicit {
                    let v = raw.trim().eq_ignore_ascii_case("true");
                    fm.map.insert("public".into(), Value::Bool(v));
                    st.public_true = v;
                }
            }
            "alias" | "aliases" => {
                let items = split_commas(raw);
                if !items.is_empty() {
                    st.has_aliases = true;
                    fm.map.insert("aliases".into(), Value::List(items));
                }
            }
            "tags" => {
                let items: Vec<String> = split_commas(raw)
                    .iter()
                    .map(|s| clean_tag(s))
                    .filter(|s| !s.is_empty())
                    .collect();
                if !items.is_empty() {
                    fm.map.insert("tags".into(), Value::List(items));
                }
            }
            "title" => {
                fm.map.insert("title".into(), Value::Scalar(raw.clone()));
            }
            "owl:class" | "owl-class" => {
                fm.map.insert("owl-class".into(), Value::Scalar(raw.clone()));
            }
            "source-domain" => {
                fm.map
                    .insert("source-domain".into(), Value::Scalar(raw.clone()));
            }
            "elevatedFrom" => {
                fm.map
                    .insert("elevatedFrom".into(), Value::Scalar(raw.clone()));
            }
            // Outliner fold state: no Obsidian meaning, dropped silently (V3).
            "collapsed" => st.collapsed_dropped += 1,
            // Logseq block anchor. Dropped, but reported: the `((uuid))` refs
            // that would have targeted it are dangling in this corpus.
            "id" => st.id_dropped += 1,
            other => {
                fm.map
                    .insert(other.to_string(), Value::Scalar(raw.clone()));
            }
        }
    }
    (fm, st)
}

// ---------------------------------------------------------------------------
// YAML emission
// ---------------------------------------------------------------------------

const LEAD_SPECIAL: &[char] = &[
    '-', '?', ':', '!', '&', '*', '>', '|', '%', '@', '`', '{', '}', '[', ']', ',', '"', '\'', '#',
];

/// Quote when the value would otherwise be invalid, retyped, or ambiguous YAML.
/// ISO dates and datetimes stay bare so Obsidian types them as Date.
pub fn needs_quote(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if iso_date_re().is_match(s) {
        return false;
    }
    if s.contains("[[") || s.contains(':') || s.contains('#') {
        return true;
    }
    if s.starts_with(LEAD_SPECIAL) {
        return true;
    }
    if s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace) {
        return true;
    }
    false
}

pub fn yaml_scalar(s: &str) -> String {
    if needs_quote(s) {
        let esc = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{esc}\"")
    } else {
        s.to_string()
    }
}

/// Fixed key order: the V2 table's own order, then everything else
/// alphabetically so the output is byte-stable across runs.
const KEY_ORDER: &[&str] = &[
    "public",
    "title",
    "aliases",
    "tags",
    "owl-class",
    "source-domain",
    "elevatedFrom",
];

pub fn emit(fm: &Frontmatter) -> String {
    if fm.is_empty() {
        return String::new();
    }
    let mut out = String::from("---\n");
    let mut seen: Vec<&str> = Vec::new();

    for k in KEY_ORDER {
        if let Some(v) = fm.map.get(*k) {
            emit_entry(&mut out, k, v);
            seen.push(k);
        }
    }
    // BTreeMap iteration is already alphabetical.
    for (k, v) in &fm.map {
        if seen.contains(&k.as_str()) {
            continue;
        }
        emit_entry(&mut out, k, v);
    }
    out.push_str("---\n");
    out
}

fn emit_entry(out: &mut String, key: &str, value: &Value) {
    match value {
        Value::Bool(b) => out.push_str(&format!("{key}: {b}\n")),
        Value::Scalar(s) => {
            // elevatedFrom is always a link -> always quoted (V2 rules).
            let rendered = if key == "elevatedFrom" && !needs_quote(s) {
                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                yaml_scalar(s)
            };
            out.push_str(&format!("{key}: {rendered}\n"));
        }
        Value::List(items) => {
            out.push_str(&format!("{key}:\n"));
            for it in items {
                out.push_str(&format!("  - {}\n", yaml_scalar(it)));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reading back an already-converted page
// ---------------------------------------------------------------------------

/// Parse a YAML frontmatter block that this converter (or Obsidian) wrote.
///
/// Deliberately a strict subset — `key: scalar`, `key:` + `  - item` lists, and
/// inline `key: [a, b]` — which is exactly what `emit` produces and what
/// Obsidian's Properties UI round-trips. Returns the map and the index of the
/// first body line. `None` when the file does not open with a closed `---`
/// block.
pub fn parse_existing(lines: &[&str]) -> Option<(Frontmatter, usize)> {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return None;
    }
    let close = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| l.trim_end() == "---")
        .map(|(i, _)| i)?;

    let mut fm = Frontmatter::default();
    let mut i = 1;
    while i < close {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        let Some((k, rest)) = line.split_once(':') else {
            i += 1;
            continue;
        };
        let key = k.trim().to_string();
        let rest = rest.trim();
        if rest.is_empty() {
            // block list
            let mut items = Vec::new();
            let mut j = i + 1;
            while j < close {
                let l = lines[j];
                let t = l.trim();
                if let Some(item) = t.strip_prefix("- ") {
                    items.push(unquote(item.trim()));
                    j += 1;
                } else {
                    break;
                }
            }
            if items.is_empty() {
                fm.map.insert(key, Value::Scalar(String::new()));
            } else {
                fm.map.insert(key, Value::List(items));
            }
            i = j;
            continue;
        }
        if rest == "true" || rest == "false" {
            fm.map.insert(key, Value::Bool(rest == "true"));
        } else if rest.starts_with('[') && rest.ends_with(']') {
            let inner = &rest[1..rest.len() - 1];
            let items: Vec<String> = inner
                .split(',')
                .map(|s| unquote(s.trim()))
                .filter(|s| !s.is_empty())
                .collect();
            fm.map.insert(key, Value::List(items));
        } else {
            fm.map.insert(key, Value::Scalar(unquote(rest)));
        }
        i += 1;
    }
    Some((fm, close + 1))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\")
    } else if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        s[1..s.len() - 1].replace("''", "'")
    } else {
        s.to_string()
    }
}

/// Merge `incoming` under `existing` — existing frontmatter always wins.
pub fn merge_existing_wins(existing: Frontmatter, incoming: Frontmatter) -> Frontmatter {
    let mut out = existing;
    for (k, v) in incoming.map {
        out.insert_default(&k, v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(s: &str) -> Vec<&str> {
        s.split('\n').collect()
    }

    #[test]
    fn leading_block_stops_at_blank_line() {
        let l = split("public:: true\nalias:: A\n\ntitle:: not mine\n");
        let (pairs, n) = parse_leading_block(&l);
        assert_eq!(n, 2);
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn leading_block_stops_at_heading() {
        let l = split("public:: true\n# Heading\ntitle:: not mine\n");
        let (pairs, n) = parse_leading_block(&l);
        assert_eq!(n, 1);
        assert_eq!(pairs[0].0, "public");
    }

    #[test]
    fn leading_block_stops_at_prose() {
        let l = split("public:: true\nSome prose.\ntitle:: not mine\n");
        let (_, n) = parse_leading_block(&l);
        assert_eq!(n, 1);
    }

    #[test]
    fn leading_block_accepts_outliner_bullets() {
        let l = split("- public:: true\n- alias:: A\n");
        let (pairs, n) = parse_leading_block(&l);
        assert_eq!(n, 2);
        assert_eq!(pairs[0], ("public".into(), "true".into()));
    }

    #[test]
    fn public_is_a_real_boolean() {
        let (fm, st) = map_properties(&[("public".into(), "true".into())]);
        assert_eq!(fm.map["public"], Value::Bool(true));
        assert!(st.public_true);
        assert_eq!(emit(&fm), "---\npublic: true\n---\n");
    }

    #[test]
    fn public_access_maps_to_public() {
        let (fm, _) = map_properties(&[("public-access".into(), "true".into())]);
        assert_eq!(fm.map["public"], Value::Bool(true));
    }

    #[test]
    fn explicit_public_beats_public_access() {
        let (fm, _) = map_properties(&[
            ("public".into(), "false".into()),
            ("public-access".into(), "true".into()),
        ]);
        assert_eq!(fm.map["public"], Value::Bool(false));
    }

    #[test]
    fn alias_becomes_aliases_list() {
        let (fm, st) = map_properties(&[("alias".into(), "A, B ,C".into())]);
        assert_eq!(
            fm.map["aliases"],
            Value::List(vec!["A".into(), "B".into(), "C".into()])
        );
        assert!(st.has_aliases);
    }

    #[test]
    fn tags_strip_sigils() {
        let (fm, _) = map_properties(&[("tags".into(), "#work, [[deep focus]], plain".into())]);
        assert_eq!(
            fm.map["tags"],
            Value::List(vec!["work".into(), "deep focus".into(), "plain".into()])
        );
    }

    #[test]
    fn owl_class_key_is_kebabed_and_quoted() {
        let (fm, _) = map_properties(&[("owl:class".into(), "mv:Foo".into())]);
        assert_eq!(emit(&fm), "---\nowl-class: \"mv:Foo\"\n---\n");
    }

    #[test]
    fn elevated_from_is_always_quoted() {
        let (fm, _) = map_properties(&[("elevatedFrom".into(), "[[Working Page]]".into())]);
        assert_eq!(emit(&fm), "---\nelevatedFrom: \"[[Working Page]]\"\n---\n");
    }

    #[test]
    fn collapsed_and_id_are_dropped() {
        let (fm, st) = map_properties(&[
            ("collapsed".into(), "true".into()),
            ("id".into(), "abc".into()),
        ]);
        assert!(fm.is_empty());
        assert_eq!(st.collapsed_dropped, 1);
        assert_eq!(st.id_dropped, 1);
    }

    #[test]
    fn unknown_keys_are_preserved_verbatim() {
        let (fm, _) = map_properties(&[("episode-url".into(), "https://x.test/a".into())]);
        // contains ':' -> quoted so the YAML stays valid
        assert_eq!(
            emit(&fm),
            "---\nepisode-url: \"https://x.test/a\"\n---\n"
        );
    }

    #[test]
    fn iso_dates_stay_unquoted() {
        assert!(!needs_quote("2026-09-02"));
        assert!(!needs_quote("2026-05-29T00:00:00Z"));
        let (fm, _) = map_properties(&[("episode-date".into(), "2026-04-20".into())]);
        assert_eq!(emit(&fm), "---\nepisode-date: 2026-04-20\n---\n");
    }

    #[test]
    fn key_order_is_the_v2_table_then_alphabetical() {
        let (fm, _) = map_properties(&[
            ("zeta".into(), "z".into()),
            ("alpha".into(), "a".into()),
            ("elevatedFrom".into(), "[[W]]".into()),
            ("title".into(), "T".into()),
            ("public".into(), "true".into()),
            ("source-domain".into(), "mv".into()),
            ("tags".into(), "t".into()),
            ("owl:class".into(), "c".into()),
            ("alias".into(), "A".into()),
        ]);
        let out = emit(&fm);
        let keys: Vec<&str> = out
            .lines()
            .filter(|l| !l.starts_with("---") && !l.starts_with("  - "))
            .map(|l| l.split(':').next().unwrap())
            .collect();
        assert_eq!(
            keys,
            vec![
                "public",
                "title",
                "aliases",
                "tags",
                "owl-class",
                "source-domain",
                "elevatedFrom",
                "alpha",
                "zeta"
            ]
        );
    }

    #[test]
    fn round_trips_its_own_output() {
        let (fm, _) = map_properties(&[
            ("public".into(), "true".into()),
            ("alias".into(), "A, B".into()),
            ("owl:class".into(), "mv:Foo".into()),
            ("episode-date".into(), "2026-04-20".into()),
        ]);
        let text = emit(&fm);
        let lines: Vec<&str> = text.split('\n').collect();
        let (parsed, _) = parse_existing(&lines).unwrap();
        assert_eq!(emit(&parsed), text);
    }

    #[test]
    fn existing_frontmatter_wins_on_merge() {
        let mut existing = Frontmatter::default();
        existing.map.insert("title".into(), Value::Scalar("Kept".into()));
        let mut incoming = Frontmatter::default();
        incoming
            .map
            .insert("title".into(), Value::Scalar("Discarded".into()));
        incoming.map.insert("public".into(), Value::Bool(true));
        let merged = merge_existing_wins(existing, incoming);
        assert_eq!(merged.map["title"], Value::Scalar("Kept".into()));
        assert_eq!(merged.map["public"], Value::Bool(true));
    }

    #[test]
    fn unclosed_frontmatter_is_not_frontmatter() {
        let l = split("---\npublic: true\nno close here\n");
        assert!(parse_existing(&l).is_none());
    }
}
