//! Filename / page-identity mapping (governing doc V1, V3 row "Wikilinks").
//!
//! Logseq stores a namespaced page `Ns/Title` on disk as `Ns___Title.md`, or
//! (older exports / URL-safe writers) `Ns%2FTitle.md`. Obsidian has no
//! namespace concept: the hierarchy *is* the filesystem, so both encodings
//! decode to a real folder boundary.

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// `YYYY_MM_DD` (Logseq) or `YYYY-MM-DD` (already-converted vault).
fn journal_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(\d{4})[_-](\d{2})[_-](\d{2})$").unwrap())
}

/// Decode the Logseq namespace encodings into `/`.
///
/// Both `___` and `%2F`/`%2f` are replaced everywhere, so a multi-level page
/// (`a___b___c`) decodes in one pass to `a/b/c`. The result is then sanitised:
/// empty, `.` and `..` components are dropped so a crafted page name can never
/// escape the output root.
pub fn decode_page_name(stem: &str) -> String {
    let decoded = stem
        .replace("___", "/")
        .replace("%2F", "/")
        .replace("%2f", "/");
    sanitise_rel(&decoded)
}

/// Drop path components that would escape the vault root, and collapse
/// repeated separators. Never returns a leading or trailing `/`.
pub fn sanitise_rel(s: &str) -> String {
    let parts: Vec<&str> = s
        .split('/')
        .map(str::trim)
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
        .collect();
    parts.join("/")
}

/// The page name Obsidian would infer from a vault-relative path: the basename
/// without `.md`.
pub fn inferred_title(page_name: &str) -> &str {
    match page_name.rsplit_once('/') {
        Some((_, leaf)) => leaf,
        None => page_name,
    }
}

/// Map a `pages/`-relative source path to its `pages/`-relative vault path and
/// the Logseq page name it carries.
///
/// Already-foldered input (`Ns/Title.md`, i.e. this converter's own output) maps
/// to itself, which is what makes the whole thing idempotent.
pub fn map_page(rel: &Path) -> Option<(PathBuf, String)> {
    if rel.extension().and_then(|e| e.to_str()) != Some("md") {
        return None;
    }
    let stem = rel.file_stem()?.to_str()?;
    // Directory components already present in the source are namespace levels.
    let mut prefix: Vec<String> = Vec::new();
    if let Some(parent) = rel.parent() {
        for c in parent.components() {
            if let std::path::Component::Normal(os) = c {
                prefix.push(os.to_string_lossy().into_owned());
            }
        }
    }
    let decoded = decode_page_name(stem);
    if decoded.is_empty() {
        return None;
    }
    let page_name = if prefix.is_empty() {
        decoded
    } else {
        format!("{}/{}", prefix.join("/"), decoded)
    };
    let out = PathBuf::from(format!("{page_name}.md"));
    Some((out, page_name))
}

/// Map a `journals/`-relative source path to its vault path.
///
/// `2026_09_02.md` -> `2026-09-02.md`; an already-hyphenated name maps to
/// itself. Anything that is not a date keeps its name (preserve, never guess).
pub fn map_journal(rel: &Path) -> Option<(PathBuf, bool)> {
    if rel.extension().and_then(|e| e.to_str()) != Some("md") {
        return None;
    }
    let stem = rel.file_stem()?.to_str()?;
    match journal_re().captures(stem) {
        Some(c) => {
            let renamed = format!("{}-{}-{}", &c[1], &c[2], &c[3]);
            let changed = renamed != stem;
            Some((PathBuf::from(format!("{renamed}.md")), changed))
        }
        None => Some((PathBuf::from(format!("{stem}.md")), false)),
    }
}

/// Rewrite a wikilink target that still carries a legacy namespace encoding.
pub fn decode_link_target(target: &str) -> String {
    if target.contains("___") || target.contains("%2F") || target.contains("%2f") {
        target
            .replace("___", "/")
            .replace("%2F", "/")
            .replace("%2f", "/")
    } else {
        target.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_triple_underscore() {
        assert_eq!(decode_page_name("Ns___Title"), "Ns/Title");
    }

    #[test]
    fn decodes_multiple_separators_recursively() {
        assert_eq!(decode_page_name("a___b___c"), "a/b/c");
        assert_eq!(decode_page_name("a%2Fb%2Fc"), "a/b/c");
        assert_eq!(decode_page_name("a___b%2Fc"), "a/b/c");
    }

    #[test]
    fn decodes_percent_encoded_real_corpus_names() {
        assert_eq!(decode_page_name("TCP%2FIP"), "TCP/IP");
        assert_eq!(
            decode_page_name("ISO%2FIEC 9075 SQL Standard"),
            "ISO/IEC 9075 SQL Standard"
        );
    }

    #[test]
    fn refuses_to_escape_the_root() {
        assert_eq!(decode_page_name("..___..___etc___passwd"), "etc/passwd");
        assert_eq!(decode_page_name("___leading"), "leading");
        assert_eq!(decode_page_name("trailing___"), "trailing");
    }

    #[test]
    fn plain_page_is_untouched() {
        assert_eq!(decode_page_name("Content Addressing"), "Content Addressing");
    }

    #[test]
    fn maps_namespace_page_to_folder() {
        let (p, name) = map_page(Path::new("Ns___Title.md")).unwrap();
        assert_eq!(p, PathBuf::from("Ns/Title.md"));
        assert_eq!(name, "Ns/Title");
    }

    #[test]
    fn already_foldered_page_maps_to_itself() {
        let (p, name) = map_page(Path::new("Ns/Title.md")).unwrap();
        assert_eq!(p, PathBuf::from("Ns/Title.md"));
        assert_eq!(name, "Ns/Title");
    }

    #[test]
    fn journal_underscores_become_hyphens() {
        let (p, changed) = map_journal(Path::new("2026_09_02.md")).unwrap();
        assert_eq!(p, PathBuf::from("2026-09-02.md"));
        assert!(changed);
    }

    #[test]
    fn journal_already_hyphenated_is_stable() {
        let (p, changed) = map_journal(Path::new("2026-09-02.md")).unwrap();
        assert_eq!(p, PathBuf::from("2026-09-02.md"));
        assert!(!changed);
    }

    #[test]
    fn non_date_journal_is_preserved() {
        let (p, changed) = map_journal(Path::new("notes.md")).unwrap();
        assert_eq!(p, PathBuf::from("notes.md"));
        assert!(!changed);
    }

    #[test]
    fn inferred_title_is_the_leaf() {
        assert_eq!(inferred_title("Ns/Title"), "Title");
        assert_eq!(inferred_title("Title"), "Title");
    }
}
