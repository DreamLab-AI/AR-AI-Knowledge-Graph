// src/services/jsonld_ingest/extractor.rs
//! Markdown → ```json-ld code fence extractor.
//!
//! Per ADR-D01 §D1, every semantically meaningful assertion lives inside
//! a fenced `json-ld` code block. The parser MUST scan only fences with
//! the explicit `json-ld` language tag — bare ``` fences, ~~~ fences with
//! other languages, and indented code blocks are documentation and must
//! be skipped.
//!
//! ## Why not pulldown-cmark
//!
//! `pulldown-cmark` is not a transitive dependency of webxr at the current
//! baseline. Adding it pulls in ~12k LoC of CommonMark machinery that we
//! do not exercise (the only markdown structure this module cares about
//! is fenced code blocks). The hand-rolled scanner below is 60 LoC, has
//! zero allocations on miss, and matches the CommonMark §4.5 fenced-code-
//! block production for the subset we need:
//!
//! - Opening fence: line starting with `\`\`\`` (3+ backticks) followed by
//!   optional info string. The info string's FIRST WHITESPACE-DELIMITED
//!   token is the language tag (CommonMark §4.5).
//! - Closing fence: line containing `\`\`\`` (same count of backticks or
//!   more) and no other content.
//! - Tilde fences (`~~~`) are NOT scanned — by D1 we mandate backtick fences
//!   for JSON-LD blocks. Tilde fences would be skipped silently regardless.
//!
//! Indented code blocks (CommonMark §4.4) are intentionally invisible to
//! this scanner per fixture 107's invariant.

/// One JSON-LD block carved out of a markdown source.
#[derive(Debug, Clone)]
pub struct ExtractedBlock {
    /// 0-based index of this block within the file (used for error reporting).
    pub index: usize,
    /// 1-based line number where the OPENING fence appears.
    pub opening_line: usize,
    /// The raw JSON payload between the opening and closing fences. NOT
    /// guaranteed to be valid JSON — that's the next stage.
    pub body: String,
}

/// Extract every ```json-ld fenced block from `markdown`. Returns blocks
/// in document order. Empty result is legitimate (no JSON-LD = no events)
/// and the caller decides whether to flag it.
pub fn extract_jsonld_blocks(markdown: &str) -> Vec<ExtractedBlock> {
    let markdown = markdown.strip_prefix('\u{FEFF}').unwrap_or(markdown);
    let mut blocks = Vec::new();
    let mut idx = 0_usize;

    let lines: Vec<&str> = markdown.split_inclusive('\n').collect();
    let mut i = 0_usize;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = strip_trailing_newline(line);

        if let Some((fence_len, info)) = parse_opening_fence(trimmed) {
            // First whitespace-delimited token of the info string is the
            // language tag. Per fixture 107, only `json-ld` qualifies.
            let lang = info.split_whitespace().next().unwrap_or("");
            if lang == "json-ld" {
                // Scan forward for matching close fence (>= fence_len backticks).
                let opening_line = i + 1;
                let mut body = String::new();
                let mut j = i + 1;
                let mut closed = false;
                while j < lines.len() {
                    let l = strip_trailing_newline(lines[j]);
                    if is_closing_fence(l, fence_len) {
                        closed = true;
                        break;
                    }
                    body.push_str(lines[j]);
                    j += 1;
                }
                if closed {
                    blocks.push(ExtractedBlock {
                        index: idx,
                        opening_line,
                        body,
                    });
                    idx += 1;
                    i = j + 1;
                    continue;
                } else {
                    // Unterminated fence — skip the line and move on. This
                    // mirrors CommonMark's tolerance for unterminated code
                    // blocks (they extend to EOF). We emit one block.
                    blocks.push(ExtractedBlock {
                        index: idx,
                        opening_line,
                        body,
                    });
                    idx += 1;
                    i = lines.len();
                    continue;
                }
            } else {
                // Different language tag — skip past the matching close
                // so we don't accidentally re-enter mid-block.
                let mut j = i + 1;
                while j < lines.len() {
                    let l = strip_trailing_newline(lines[j]);
                    if is_closing_fence(l, fence_len) {
                        break;
                    }
                    j += 1;
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }

    blocks
}

/// Returns `Some((fence_length, info_string))` if `line` is a CommonMark
/// fenced-code opening line (3+ backticks possibly indented up to 3 spaces).
/// Returns `None` otherwise. Tilde fences are deliberately ignored.
fn parse_opening_fence(line: &str) -> Option<(usize, &str)> {
    // CommonMark allows up to 3 spaces of indent on the opening fence.
    let trimmed_start = line.trim_start_matches(|c: char| c == ' ');
    let leading = line.len() - trimmed_start.len();
    if leading > 3 {
        return None;
    }
    let mut bytes = trimmed_start.bytes();
    let mut count = 0_usize;
    loop {
        match bytes.next() {
            Some(b'`') => count += 1,
            _ => break,
        }
    }
    if count < 3 {
        return None;
    }
    let info = &trimmed_start[count..];
    // CommonMark forbids backticks inside the info string of a backtick fence.
    if info.contains('`') {
        return None;
    }
    Some((count, info.trim()))
}

/// `line` is a closing fence if, after optional leading whitespace, it
/// contains a run of >= `min_len` backticks and nothing else.
fn is_closing_fence(line: &str, min_len: usize) -> bool {
    let trimmed_start = line.trim_start_matches(|c: char| c == ' ');
    let leading = line.len() - trimmed_start.len();
    if leading > 3 {
        return false;
    }
    let mut count = 0_usize;
    for c in trimmed_start.chars() {
        if c == '`' {
            count += 1;
        } else {
            break;
        }
    }
    if count < min_len {
        return false;
    }
    // Anything after the run must be whitespace only.
    trimmed_start[count..].chars().all(|c| c.is_whitespace())
}

fn strip_trailing_newline(line: &str) -> &str {
    line.strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_single_jsonld_block() {
        let md = "Some prose.\n\n```json-ld\n{\"@id\":\"x\"}\n```\n\nMore prose.\n";
        let blocks = extract_jsonld_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].body.trim(), r#"{"@id":"x"}"#);
        assert_eq!(blocks[0].opening_line, 3);
    }

    #[test]
    fn skips_bare_fence() {
        let md = "```\n{\"@id\":\"x\"}\n```\n";
        assert!(extract_jsonld_blocks(md).is_empty());
    }

    #[test]
    fn skips_other_language() {
        let md = "```rust\nfn main() {}\n```\n```json-ld\n{\"k\":1}\n```\n";
        let blocks = extract_jsonld_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].body.contains("\"k\":1"));
    }

    #[test]
    fn skips_indented_code() {
        let md = "    {\"@id\":\"x\"}\n";
        assert!(extract_jsonld_blocks(md).is_empty());
    }

    #[test]
    fn handles_multiple_blocks() {
        let md = "```json-ld\n{\"a\":1}\n```\nprose\n```json-ld\n{\"b\":2}\n```\n";
        let blocks = extract_jsonld_blocks(md);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].index, 0);
        assert_eq!(blocks[1].index, 1);
    }

    #[test]
    fn info_string_first_token_only() {
        // CommonMark: first whitespace-delimited token of info is the lang.
        let md = "```json-ld foo bar\n{\"a\":1}\n```\n";
        let blocks = extract_jsonld_blocks(md);
        assert_eq!(blocks.len(), 1);
    }
}
