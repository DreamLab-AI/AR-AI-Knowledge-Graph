//! One page in, one page out.

use crate::body::{self, BodyCounts, BodyLeftovers};
use crate::frontmatter::{self, Value};
use crate::paths::inferred_title;

#[derive(Debug, Default, Clone)]
pub struct PageStats {
    pub public_true: bool,
    pub has_aliases: bool,
    pub already_obsidian: bool,
    pub collapsed_dropped: usize,
    pub id_dropped: usize,
    /// `title:` values that merely echoed the page identity (or its leaf),
    /// removed as converter artefacts.
    pub title_echo_removed: usize,
    /// Asset targets rewritten inside frontmatter values (V1), counted into
    /// the same `asset_paths` rule as the body ones.
    pub frontmatter_asset_paths: usize,
    pub counts: BodyCounts,
    pub leftovers: BodyLeftovers,
}

#[derive(Debug, Clone)]
pub struct PageResult {
    pub content: String,
    pub stats: PageStats,
}

/// Convert one page.
///
/// `page_name` is the Logseq page identity for this file — the vault-relative
/// path under `pages/` without `.md`, with `/` as the namespace separator. It
/// is used only to decide whether an explicit `title:` is needed (V1: Obsidian
/// infers the title from the basename, which loses the namespace).
pub fn convert_page(text: &str, page_name: &str) -> PageResult {
    convert_inner(text, Some(page_name))
}

/// Convert a journal. Journals carry no page identity, so no `title:` is
/// synthesised; otherwise the pipeline is identical.
pub fn convert_journal(text: &str) -> PageResult {
    convert_inner(text, None)
}

fn convert_inner(text: &str, page_name: Option<&str>) -> PageResult {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut stats = PageStats::default();

    // 1. An already-converted page opens with a closed `---` block.
    let (existing, body_start) = match frontmatter::parse_existing(&lines) {
        Some((fm, idx)) => {
            stats.already_obsidian = true;
            (Some(fm), idx)
        }
        None => (None, 0),
    };

    // 2. A Logseq leading property block — at the top of a raw page, or after
    //    the frontmatter of a half-converted one.
    let rest = &lines[body_start..];
    let (pairs, consumed) = frontmatter::parse_leading_block(rest);
    let (mapped, fm_stats) = frontmatter::map_properties(&pairs);

    stats.collapsed_dropped += fm_stats.collapsed_dropped;
    stats.id_dropped += fm_stats.id_dropped;

    // 3. Merge — existing frontmatter always wins (never silently retyped).
    let mut fm = match existing {
        Some(e) => frontmatter::merge_existing_wins(e, mapped),
        None => mapped,
    };

    // 4. `title` is a DISPLAY value (V2), never the identity. An earlier
    //    converter release wrote the page's identity path into `title:` for
    //    namespace pages; a `title` that merely echoes the identity or its
    //    leaf is a converter artefact and is removed here so a second run
    //    repairs an already-converted vault (`title_echo_removed`). An
    //    author-supplied title that differs from both is never touched.
    //    Exception: when the page's own first H1 confirms the title (e.g.
    //    `A/B Testing`, `TCP/IP` — the slash is part of the real name), it is
    //    a genuine display title and is kept; without it Obsidian would show
    //    only the leaf.
    if let Some(name) = page_name {
        let echoes = matches!(
            fm.map.get("title"),
            Some(Value::Scalar(t)) if t == name || t == inferred_title(name)
        );
        if echoes {
            let h1 = rest[consumed..]
                .iter()
                .find_map(|l| l.strip_prefix("# ").map(|t| t.trim()));
            let confirmed = matches!((fm.map.get("title"), h1), (Some(Value::Scalar(t)), Some(h)) if t == h);
            if !confirmed {
                fm.map.remove("title");
                stats.title_echo_removed += 1;
            }
        }
        // A namespace page whose own H1 is the full slashed name (`# TCP/IP`)
        // needs that H1 as its display title, or Obsidian shows only `IP`.
        // The H1 is the page's authored title, so carrying it is not writing
        // the identity into `title`; flat pages and leaf-titled pages get none.
        if !fm.map.contains_key("title") && name != inferred_title(name) {
            let h1 = rest[consumed..]
                .iter()
                .find_map(|l| l.strip_prefix("# ").map(|t| t.trim()));
            if h1 == Some(name) {
                fm.map.insert("title".into(), Value::Scalar(name.to_string()));
            }
        }
    }

    // 4b. Asset links can also live inside property values (`file::` in the
    //     working graph). Same rule, same counter.
    for v in fm.map.values_mut() {
        match v {
            Value::Scalar(s) => {
                let (rewritten, n) = body::rewrite_asset_paths(s);
                if n > 0 {
                    *s = rewritten;
                    stats.frontmatter_asset_paths += n;
                }
            }
            Value::List(items) => {
                for it in items.iter_mut() {
                    let (rewritten, n) = body::rewrite_asset_paths(it);
                    if n > 0 {
                        *it = rewritten;
                        stats.frontmatter_asset_paths += n;
                    }
                }
            }
            Value::Bool(_) => {}
        }
    }

    stats.public_true = matches!(fm.map.get("public"), Some(Value::Bool(true)));
    stats.has_aliases = matches!(fm.map.get("aliases"), Some(Value::List(v)) if !v.is_empty());

    // 5. Body.
    let body_lines = &rest[consumed..];
    let outcome = body::rewrite(body_lines);
    stats.collapsed_dropped += outcome.counts.collapsed_dropped;
    stats.counts = outcome.counts.clone();
    stats.leftovers = outcome.leftovers.clone();

    // 6. Assemble. When frontmatter is emitted the body is normalised to start
    //    after exactly one blank line, which is what makes a second run
    //    byte-identical regardless of the source's spacing.
    let fm_text = frontmatter::emit(&fm);
    let content = if fm_text.is_empty() {
        outcome.lines.join("\n")
    } else {
        let mut body: &[String] = &outcome.lines;
        while body.first().map(|l| l.trim().is_empty()) == Some(true) {
            body = &body[1..];
        }
        if body.is_empty() {
            fm_text
        } else {
            format!("{fm_text}\n{}", body.join("\n"))
        }
    };

    PageResult { content, stats }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_a_typical_corpus_page() {
        let src = "public:: true\nalias:: ContentAddressing\n\n# Content Addressing\nbody\n";
        let r = convert_page(src, "Content Addressing");
        assert_eq!(
            r.content,
            "---\npublic: true\naliases:\n  - ContentAddressing\n---\n\n# Content Addressing\nbody\n"
        );
        assert!(r.stats.public_true);
        assert!(r.stats.has_aliases);
        assert!(!r.stats.already_obsidian);
    }

    #[test]
    fn namespace_page_gains_no_title() {
        // `title` is display-only (V2); the identity path is never written.
        let src = "public:: true\n\n# T\n";
        let r = convert_page(src, "Ns/Title");
        assert!(r.content.starts_with("---\npublic: true\n---\n"));
        assert!(!r.content.contains("title:"));
        assert_eq!(r.stats.title_echo_removed, 0);
    }

    #[test]
    fn a_title_confirmed_by_the_h1_is_a_real_title_and_is_kept() {
        // The slash is part of the genuine name: `A/B Testing`, `TCP/IP`.
        let src = "---\npublic: true\ntitle: A/B Testing\n---\n\n# A/B Testing\nbody\n";
        let r = convert_page(src, "A/B Testing");
        assert!(r.content.contains("title: A/B Testing"));
        assert_eq!(r.stats.title_echo_removed, 0);
        let src = "public:: true\ntitle:: TCP/IP\n\n# TCP/IP\n";
        let r = convert_page(src, "TCP/IP");
        assert!(r.content.contains("title: TCP/IP"));
    }

    #[test]
    fn a_namespace_page_whose_h1_is_the_slashed_name_gets_it_as_title() {
        let r = convert_page("public:: true\n\n# TCP/IP\n", "TCP/IP");
        assert!(r.content.contains("title: TCP/IP"));
        // leaf-titled namespace page: Obsidian infers `Child` already
        let r = convert_page("public:: true\n\n# Child\n", "Ns/Child");
        assert!(!r.content.contains("\ntitle: "));
        // idempotent: a second run keeps it (H1 confirms) — no echo removal
        let r2 = convert_page(&convert_page("public:: true\n\n# TCP/IP\n", "TCP/IP").content, "TCP/IP");
        assert!(r2.content.contains("title: TCP/IP"));
        assert_eq!(r2.stats.title_echo_removed, 0);
    }

    #[test]
    fn a_title_echoing_the_identity_is_repaired_on_rerun() {
        for echo in ["Ns/Title", "Title"] {
            let src = format!("---\npublic: true\ntitle: {echo}\n---\n\n# T\nbody\n");
            let r = convert_page(&src, "Ns/Title");
            assert!(!r.content.contains("title:"), "{echo}");
            assert_eq!(r.stats.title_echo_removed, 1);
            assert!(r.stats.already_obsidian);
        }
    }

    #[test]
    fn flat_page_gains_no_title() {
        let r = convert_page("public:: true\n\n# T\n", "Title");
        assert!(!r.content.contains("title:"));
    }

    #[test]
    fn an_authored_title_is_never_overridden() {
        let r = convert_page("public:: true\ntitle:: Author's Own\n\n# T\n", "Ns/Title");
        assert!(r.content.contains("title: Author's Own"));
        assert!(!r.content.contains("title: Ns/Title"));
    }

    #[test]
    fn body_property_block_after_a_heading_is_not_frontmatter() {
        // The real podcast-page shape: only `public:: true` is leading.
        let src = "public:: true\n\n# Title\n\ntitle:: Body Level\nsource:: AI Daily Brief\n";
        let r = convert_page(src, "podcast-evidence/x");
        assert!(!r.content.contains("\ntitle: "));
        assert!(r.content.contains("\ntitle:: Body Level\n"));
        assert_eq!(r.stats.leftovers.body_properties, 2);
    }

    #[test]
    fn already_converted_page_is_a_no_op() {
        let src = "---\npublic: true\ntitle: Kept Display Title\n---\n\n# T\nbody\n";
        let r = convert_page(src, "Ns/Title");
        assert_eq!(r.content, src);
        assert!(r.stats.already_obsidian);
    }

    #[test]
    fn existing_frontmatter_wins_over_a_trailing_logseq_block() {
        let src = "---\npublic: false\n---\npublic:: true\n\n# T\n";
        let r = convert_page(src, "T");
        assert!(r.content.contains("public: false"));
    }

    #[test]
    fn page_with_no_properties_keeps_its_body_verbatim() {
        let src = "# 2022_11_07\n- a\n- b\n";
        let r = convert_journal(src);
        assert_eq!(r.content, src);
    }

    #[test]
    fn conversion_is_idempotent_for_every_rule_at_once() {
        let src = "public:: true\nalias:: A\ncollapsed:: true\nid:: 1234\n\n\
                   # T\n- TODO t\n{{embed [[E]]}}\n#[[multi word]]\n\
                   ![i](../assets/i.png)\n[[Ns___L]]\n```\n- TODO fenced\n```\n";
        let once = convert_page(src, "Ns/Title");
        let twice = convert_page(&once.content, "Ns/Title");
        assert_eq!(once.content, twice.content, "second pass must be byte-identical");
        assert_eq!(twice.stats.counts.tasks, 0, "nothing left to rewrite");
        assert!(twice.content.contains("- TODO fenced"), "fence preserved");
    }

    #[test]
    fn dropped_keys_are_counted() {
        let r = convert_page("public:: true\ncollapsed:: true\nid:: x\n\n# T\n", "T");
        assert_eq!(r.stats.collapsed_dropped, 1);
        assert_eq!(r.stats.id_dropped, 1);
        assert!(!r.content.contains("collapsed"));
        assert!(!r.content.contains("id:"));
    }
}
