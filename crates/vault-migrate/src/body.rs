//! Body dialect rewrites (governing doc V3).
//!
//! Every rule runs through one fence-aware line walker: content inside a
//! backtick or tilde fence is copied byte-for-byte. That is load-bearing, not
//! cosmetic — the corpus documents Logseq syntax *inside* code fences
//! (`{{embed ((block-uuid))}}`, `- TODO ...`), and rewriting those would
//! corrupt prose that is deliberately showing the legacy form.

use crate::paths::decode_link_target;
use regex::{Captures, Regex};
use std::sync::OnceLock;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}

re!(fence_re, r"^(\s*)(```+|~~~+)(.*)$");
re!(embed_page_re, r"\{\{embed\s+\[\[([^\]]+)\]\]\s*\}\}");
re!(task_re, r"^(\s*[-*+]\s+)(TODO|DOING|NOW|LATER|DONE)\s+(.*)$");
re!(multiword_tag_re, r"#\[\[([^\]\n]+)\]\]");
re!(asset_re, r"\]\((?:\.\./)+assets/");
re!(
    block_ref_re,
    r"\(\([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\)\)"
);
re!(collapsed_re, r"^\s*(?:-\s*)?collapsed::\s*true\s*$");
re!(sched_re, r"^\s*(?:-\s*)?(SCHEDULED|DEADLINE):");
re!(legacy_link_re, r"\[\[([^\]\n]+)\]\]");
re!(ws_run_re, r"\s+");

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BodyCounts {
    pub embeds: usize,
    pub tasks: usize,
    pub multiword_tags: usize,
    pub asset_paths: usize,
    pub collapsed_dropped: usize,
    pub namespace_links: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BodyLeftovers {
    pub block_refs: usize,
    pub body_properties: usize,
    pub scheduled_deadline: usize,
}

#[derive(Debug, Default, Clone)]
pub struct BodyOutcome {
    pub lines: Vec<String>,
    pub counts: BodyCounts,
    pub leftovers: BodyLeftovers,
}

/// Tracks ``` / ~~~ fences across a document.
///
/// A fence opens on a marker with an info string (or none) and closes only on a
/// marker of the same character, at least as long, with no info string — the
/// CommonMark rule. Indentation is ignored, because the corpus nests fences
/// under outliner bullets.
#[derive(Default)]
struct FenceState {
    open: Option<(char, usize)>,
}

impl FenceState {
    fn inside(&self) -> bool {
        self.open.is_some()
    }

    /// Feed a line; returns true when the line is itself a fence marker.
    fn feed(&mut self, line: &str) -> bool {
        let Some(c) = fence_re().captures(line) else {
            return false;
        };
        let marker = &c[2];
        let ch = marker.chars().next().unwrap();
        let len = marker.len();
        let info = c[3].trim();
        match self.open {
            None => {
                self.open = Some((ch, len));
                true
            }
            Some((open_ch, open_len)) => {
                if ch == open_ch && len >= open_len && info.is_empty() {
                    self.open = None;
                }
                true
            }
        }
    }
}

/// True when the body (outside code fences) carries a Logseq `public:: true`
/// line — as a bare line or an outliner bullet at any depth. Used by the
/// converter's one-time promotion rule: the pre-vault reader accepted the
/// marker anywhere, so a page whose only marker sits under an H1 must not be
/// silently privatised by the leading-block-only gate (V4).
pub fn body_declares_public(body: &[&str]) -> bool {
    let mut fence = FenceState::default();
    for line in body {
        if fence.feed(line) || fence.inside() {
            continue;
        }
        let t = line.trim_start();
        let t = t.strip_prefix('-').map(str::trim_start).unwrap_or(t);
        if let Some(rest) = t.strip_prefix("public::") {
            if rest.trim().eq_ignore_ascii_case("true") {
                return true;
            }
        }
    }
    false
}

/// Convert `#[[multi word]]` to `#multi-word`: case is preserved, runs of
/// whitespace collapse to a single hyphen (Obsidian tags cannot contain spaces).
fn tagify(inner: &str) -> String {
    ws_run_re().replace_all(inner.trim(), "-").into_owned()
}

/// Rewrite `](../assets/...` to `](assets/...` in an arbitrary string, and
/// report how many targets moved.
///
/// Used for the body *and* for frontmatter property values: the corpus stores
/// `file:: [name](../assets/x.pdf)` in the leading property block, and a
/// note-relative `../assets/` breaks as soon as the page moves into a
/// namespace folder (`pages/Ns/Title.md` would resolve it to `pages/assets/`).
/// V1 requires vault-root-relative asset links, so the rule follows the link
/// target rather than the section it happens to sit in.
pub fn rewrite_asset_paths(s: &str) -> (String, usize) {
    let n = asset_re().find_iter(s).count();
    if n == 0 {
        return (s.to_string(), 0);
    }
    (asset_re().replace_all(s, "](assets/").into_owned(), n)
}

pub fn rewrite(body: &[&str]) -> BodyOutcome {
    let mut out = BodyOutcome::default();
    let mut fence = FenceState::default();

    for raw in body {
        // A fence marker line itself is never rewritten.
        if fence.feed(raw) {
            out.lines.push((*raw).to_string());
            continue;
        }
        if fence.inside() {
            out.lines.push((*raw).to_string());
            continue;
        }

        // `collapsed:: true` is outliner fold state anywhere it appears (V3).
        if collapsed_re().is_match(raw) {
            out.counts.collapsed_dropped += 1;
            continue;
        }

        let mut line = (*raw).to_string();

        // --- leftovers: detected on the ORIGINAL line, never rewritten ---
        let brefs = block_ref_re().find_iter(&line).count();
        out.leftovers.block_refs += brefs;
        if sched_re().is_match(&line) {
            out.leftovers.scheduled_deadline += 1;
        }
        if crate::frontmatter::prop_re().is_match(&line) {
            out.leftovers.body_properties += 1;
        }

        // --- rewrites ---
        let n = embed_page_re().find_iter(&line).count();
        if n > 0 {
            out.counts.embeds += n;
            line = embed_page_re()
                .replace_all(&line, "![[$1]]")
                .into_owned();
        }

        if let Some(c) = task_re().captures(&line) {
            let box_ = if &c[2] == "DONE" { "[x]" } else { "[ ]" };
            // Normalise the bullet to `- ` so output is stable.
            let indent: String = c[1].chars().take_while(|ch| ch.is_whitespace()).collect();
            line = format!("{indent}- {box_} {}", &c[3]);
            out.counts.tasks += 1;
        }

        let n = multiword_tag_re().find_iter(&line).count();
        if n > 0 {
            out.counts.multiword_tags += n;
            line = multiword_tag_re()
                .replace_all(&line, |c: &Captures| format!("#{}", tagify(&c[1])))
                .into_owned();
        }

        let n = asset_re().find_iter(&line).count();
        if n > 0 {
            out.counts.asset_paths += n;
            line = asset_re().replace_all(&line, "](assets/").into_owned();
        }

        // `[[Ns___Title]]` -> `[[Ns/Title]]`. Plain `[[Ns/Title]]` is already
        // correct and passes through untouched.
        if line.contains("___") || line.contains("%2F") || line.contains("%2f") {
            let mut changed = 0usize;
            let replaced = legacy_link_re().replace_all(&line, |c: &Captures| {
                let (target, alias) = match c[1].split_once('|') {
                    Some((t, a)) => (t, Some(a)),
                    None => (&c[1], None),
                };
                let decoded = decode_link_target(target);
                if decoded != target {
                    changed += 1;
                }
                match alias {
                    Some(a) => format!("[[{decoded}|{a}]]"),
                    None => format!("[[{decoded}]]"),
                }
            });
            if changed > 0 {
                out.counts.namespace_links += changed;
                line = replaced.into_owned();
            }
        }

        out.lines.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &str) -> BodyOutcome {
        let lines: Vec<&str> = s.split('\n').collect();
        rewrite(&lines)
    }

    fn text(s: &str) -> String {
        run(s).lines.join("\n")
    }

    #[test]
    fn page_embed_becomes_obsidian_embed() {
        assert_eq!(text("- {{embed [[Some Page]]}}"), "- ![[Some Page]]");
        assert_eq!(run("{{embed [[A]]}}").counts.embeds, 1);
    }

    #[test]
    fn block_embed_is_left_literal_and_reported() {
        let o = run("- {{embed ((661d5f74-f334-4872-ba92-51244c2fb490))}}");
        assert_eq!(o.lines[0], "- {{embed ((661d5f74-f334-4872-ba92-51244c2fb490))}}");
        assert_eq!(o.counts.embeds, 0);
        assert_eq!(o.leftovers.block_refs, 1);
    }

    #[test]
    fn task_markers_become_checkboxes() {
        assert_eq!(text("- TODO write it"), "- [ ] write it");
        assert_eq!(text("- DOING write it"), "- [ ] write it");
        assert_eq!(text("- NOW write it"), "- [ ] write it");
        assert_eq!(text("- LATER write it"), "- [ ] write it");
        assert_eq!(text("- DONE write it"), "- [x] write it");
    }

    #[test]
    fn task_markers_keep_indentation() {
        assert_eq!(text("\t\t- TODO nested"), "\t\t- [ ] nested");
        assert_eq!(text("    - DONE nested"), "    - [ ] nested".replace("[ ]", "[x]"));
    }

    #[test]
    fn task_marker_mid_sentence_is_not_a_task() {
        assert_eq!(text("We should TODO this later"), "We should TODO this later");
        assert_eq!(text("- a TODO in prose"), "- a TODO in prose");
    }

    #[test]
    fn multiword_tag_collapses_whitespace_and_keeps_case() {
        assert_eq!(text("#[[Virtual Reality]]"), "#Virtual-Reality");
        assert_eq!(text("#[[Active Research Projects Registry]]"), "#Active-Research-Projects-Registry");
    }

    #[test]
    fn single_word_bracket_tag_loses_the_brackets() {
        assert_eq!(text("#[[Anthropic]]"), "#Anthropic");
    }

    #[test]
    fn asset_paths_become_vault_root_relative() {
        assert_eq!(
            text("![image.png](../assets/image_1717159684964_0.png)"),
            "![image.png](assets/image_1717159684964_0.png)"
        );
        assert_eq!(
            text("[zip](../../assets/a.zip)"),
            "[zip](assets/a.zip)"
        );
    }

    #[test]
    fn plain_wikilink_is_untouched() {
        assert_eq!(text("see [[Ns/Title]] and [[Other|alias]]"), "see [[Ns/Title]] and [[Other|alias]]");
    }

    #[test]
    fn legacy_namespace_wikilink_is_decoded() {
        let o = run("see [[Ns___Title]]");
        assert_eq!(o.lines[0], "see [[Ns/Title]]");
        assert_eq!(o.counts.namespace_links, 1);
        assert_eq!(text("[[Ns___Title|shown]]"), "[[Ns/Title|shown]]");
    }

    #[test]
    fn collapsed_lines_are_dropped_anywhere() {
        let o = run("- a\n  collapsed:: true\n- b");
        assert_eq!(o.lines, vec!["- a", "- b"]);
        assert_eq!(o.counts.collapsed_dropped, 1);
    }

    #[test]
    fn body_properties_are_preserved_and_reported() {
        let o = run("- enables:: [[Quadratic Funding]]");
        assert_eq!(o.lines[0], "- enables:: [[Quadratic Funding]]");
        assert_eq!(o.leftovers.body_properties, 1);
    }

    #[test]
    fn scheduled_and_deadline_are_preserved_and_reported() {
        let o = run("SCHEDULED: <2026-09-02 Wed>\nDEADLINE: <2026-09-09 Wed>");
        assert_eq!(o.lines.len(), 2);
        assert_eq!(o.leftovers.scheduled_deadline, 2);
    }

    // --- the fence walker: the rule everything else depends on ---

    #[test]
    fn backtick_fence_content_is_untouched() {
        let src = "```\npublic:: true\n- TODO not a task\n{{embed [[X]]}}\n#[[multi word]]\n```";
        let o = run(src);
        assert_eq!(o.lines.join("\n"), src);
        assert_eq!(o.counts.tasks, 0);
        assert_eq!(o.counts.embeds, 0);
        assert_eq!(o.counts.multiword_tags, 0);
        assert_eq!(o.leftovers.body_properties, 0);
    }

    #[test]
    fn tilde_fence_content_is_untouched() {
        let src = "~~~\n- TODO not a task\n~~~";
        assert_eq!(text(src), src);
    }

    #[test]
    fn fence_with_info_string_is_untouched() {
        let src = "```json-ld\n{\"a\": 1}\n- TODO no\n```";
        assert_eq!(text(src), src);
    }

    #[test]
    fn indented_fence_is_honoured() {
        // Real corpus shape: a fence nested under outliner bullets.
        let src = "\t\t\t  ```\n\t\t\t- ![x](../assets/y.png)\n\t\t  ```";
        let o = run(src);
        assert_eq!(o.lines.join("\n"), src);
        assert_eq!(o.counts.asset_paths, 0);
    }

    #[test]
    fn rewrites_resume_after_the_fence_closes() {
        let o = run("```\n- TODO inside\n```\n- TODO outside");
        assert_eq!(o.counts.tasks, 1);
        assert_eq!(o.lines[3], "- [ ] outside");
    }

    #[test]
    fn tilde_does_not_close_a_backtick_fence() {
        let o = run("```\n~~~\n- TODO still inside\n```\n- TODO outside");
        assert_eq!(o.counts.tasks, 1);
    }

    #[test]
    fn a_shorter_marker_does_not_close_a_longer_fence() {
        let o = run("````\n```\n- TODO still inside\n````\n- TODO outside");
        assert_eq!(o.counts.tasks, 1);
    }

    #[test]
    fn an_unclosed_fence_swallows_the_rest_conservatively() {
        let o = run("```\n- TODO never rewritten\n- more");
        assert_eq!(o.counts.tasks, 0);
        assert_eq!(o.lines.len(), 3);
    }

    #[test]
    fn asset_paths_rewrite_outside_the_body_too() {
        let (out, n) = rewrite_asset_paths("[doc](../assets/a.pdf) and [b](../../assets/b.pdf)");
        assert_eq!(out, "[doc](assets/a.pdf) and [b](assets/b.pdf)");
        assert_eq!(n, 2);
        assert_eq!(rewrite_asset_paths("nothing here").1, 0);
        // idempotent
        assert_eq!(rewrite_asset_paths(&out).0, out);
    }

    #[test]
    fn rewrite_is_idempotent() {
        let src = "- TODO a\n{{embed [[B]]}}\n#[[multi word]]\n![i](../assets/i.png)\n[[Ns___T]]";
        let once = text(src);
        let twice = text(&once);
        assert_eq!(once, twice);
    }
}
