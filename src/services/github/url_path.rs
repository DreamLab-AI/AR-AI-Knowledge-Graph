//! URL path construction for the GitHub APIs.
//!
//! Repository paths are **literal filenames**, not pre-encoded URL fragments,
//! and the corpus contains names with a literal percent sign — `51% Attack.md`,
//! `Presentation%3A Conclusion.md`, `chatgpt__2024-08-20 09%3A48%3A00.md`.
//! Interpolating those into a URL unescaped fails in two different ways:
//!
//! * `51% Attack.md` becomes the invalid escape `%%20` → **400 Bad Request**;
//! * `%3A` is read as a valid escape and decoded to `:`, so a perfectly
//!   well-formed request is made for a file that does not exist → **404**.
//!
//! Both dropped the page from the graph. Every URL built from a repository
//! path therefore goes through the builders here, which percent-encode each
//! segment exactly once.
//!
//! The inverse mistake is just as damaging: `GitHubClient::get_full_path` used
//! to `urlencoding::decode` the path first, corrupting a literal `%3A` into a
//! colon before the URL was even assembled. Paths enter these builders raw.

/// Percent-encode one URL path segment per RFC 3986 §3.3.
///
/// Keeps every character that is legal unescaped in a path segment (the
/// unreserved set plus the sub-delims and `:@`) and escapes the rest as the
/// percent-encoded UTF-8 octets — including `%` itself, which becomes `%25`.
/// Leaving the legal characters alone keeps the change to previously working
/// URLs at zero: the URL parser already produced exactly this output for every
/// name that did not contain a percent sign.
pub(crate) fn encode_path_segment(segment: &str) -> String {
    // Legal unescaped in a path segment alongside ALPHA / DIGIT: the
    // unreserved marks plus sub-delims plus `:` and `@`.
    const SAFE: &[u8] = b"-._~!$&'()*+,;=:@";
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut out = String::with_capacity(segment.len());
    for &byte in segment.as_bytes() {
        if byte.is_ascii_alphanumeric() || SAFE.contains(&byte) {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0F) as usize] as char);
        }
    }
    out
}

/// Percent-encode a repository path (or a branch ref) for a URL, preserving
/// the `/` separators between segments.
///
/// Used for branch refs too: a branch legitimately contains `/`
/// (`feature/thing`) and must survive, while a space or percent in it must be
/// escaped like any other segment content.
pub(crate) fn encode_repo_path(path: &str) -> String {
    path.split('/')
        .map(encode_path_segment)
        .collect::<Vec<_>>()
        .join("/")
}

/// `https://raw.githubusercontent.com/<owner>/<repo>/<branch>/<path>` with the
/// branch and path percent-encoded.
pub(crate) fn raw_download_url(owner: &str, repo: &str, branch: &str, path: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}",
        owner,
        repo,
        encode_repo_path(branch),
        encode_repo_path(path)
    )
}

/// `https://api.github.com/repos/<owner>/<repo>/contents/<path>` with the path
/// percent-encoded, plus `?ref=<branch>` when a ref is given.
pub(crate) fn contents_url(owner: &str, repo: &str, path: &str, branch: Option<&str>) -> String {
    let base = format!(
        "https://api.github.com/repos/{}/{}/contents/{}",
        owner,
        repo,
        encode_repo_path(path)
    );
    match branch {
        Some(branch) => format!("{}?ref={}", base, encode_repo_path(branch)),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three pages the sync dropped, plus a plain name and a name with a
    /// space. Every builder is checked against the same five.
    const NAMES: [&str; 5] = [
        "51% Attack.md",
        "chatgpt__2024-08-20 09%3A48%3A00.md",
        "Presentation%3A Conclusion.md",
        "Bitcoin.md",
        "Agentic AI.md",
    ];

    // -- segment / path encoding ---------------------------------------------

    #[test]
    fn a_literal_percent_becomes_percent_25_exactly_once() {
        assert_eq!(encode_path_segment("51% Attack.md"), "51%25%20Attack.md");
        assert_eq!(
            encode_path_segment("chatgpt__2024-08-20 09%3A48%3A00.md"),
            "chatgpt__2024-08-20%2009%253A48%253A00.md"
        );
        assert_eq!(
            encode_path_segment("Presentation%3A Conclusion.md"),
            "Presentation%253A%20Conclusion.md"
        );
    }

    #[test]
    fn ordinary_names_are_unchanged_or_only_space_encoded() {
        assert_eq!(encode_path_segment("Bitcoin.md"), "Bitcoin.md");
        assert_eq!(encode_path_segment("Agentic AI.md"), "Agentic%20AI.md");
    }

    #[test]
    fn separators_survive_but_a_slash_inside_a_segment_does_not() {
        assert_eq!(encode_repo_path("a/b/c.md"), "a/b/c.md");
        assert_eq!(encode_path_segment("a/b"), "a%2Fb");
    }

    // -- Contents API builder ------------------------------------------------

    #[test]
    fn the_contents_url_addresses_every_name_correctly() {
        for name in NAMES {
            let url = contents_url(
                "owner",
                "repo",
                &format!("mainKnowledgeGraph/pages/{name}"),
                Some("main"),
            );
            let parsed = reqwest::Url::parse(&url).expect("valid URL");
            let decoded = urlencoding::decode(parsed.path()).expect("utf-8");

            assert!(decoded.ends_with(name), "{name} → {decoded}");
            assert!(!parsed.path().contains("%%"), "{name} → bogus escape");
            assert!(parsed.path().starts_with("/repos/owner/repo/contents/"));
            assert_eq!(parsed.query(), Some("ref=main"));
        }
    }

    #[test]
    fn the_contents_url_keeps_the_literal_percent_escaped() {
        let url = contents_url("o", "r", "pages/51% Attack.md", Some("main"));
        assert_eq!(
            url,
            "https://api.github.com/repos/o/r/contents/pages/51%25%20Attack.md?ref=main"
        );
    }

    #[test]
    fn the_contents_url_omits_the_ref_when_none_is_given() {
        // `pr.rs::update_file` carries the branch in the request body instead.
        let url = contents_url("o", "r", "pages/51% Attack.md", None);
        assert_eq!(
            url,
            "https://api.github.com/repos/o/r/contents/pages/51%25%20Attack.md"
        );
    }

    // -- raw download builder ------------------------------------------------

    #[test]
    fn the_raw_download_url_addresses_every_name_correctly() {
        for name in NAMES {
            let url = raw_download_url("owner", "repo", "main", &format!("pages/{name}"));
            let parsed = reqwest::Url::parse(&url).expect("valid URL");
            let decoded = urlencoding::decode(parsed.path()).expect("utf-8");

            assert!(decoded.ends_with(name), "{name} → {decoded}");
            assert!(!parsed.path().contains("%%"), "{name} → bogus escape");
        }
    }

    // -- branch / ref encoding -----------------------------------------------

    #[test]
    fn a_branch_containing_a_space_is_encoded_in_both_builders() {
        let raw = raw_download_url("o", "r", "feature/my branch", "pages/Bitcoin.md");
        assert_eq!(
            raw,
            "https://raw.githubusercontent.com/o/r/feature/my%20branch/pages/Bitcoin.md"
        );
        assert!(reqwest::Url::parse(&raw).is_ok());

        let contents = contents_url("o", "r", "pages/Bitcoin.md", Some("feature/my branch"));
        assert_eq!(
            contents,
            "https://api.github.com/repos/o/r/contents/pages/Bitcoin.md?ref=feature/my%20branch"
        );
        assert!(reqwest::Url::parse(&contents).is_ok());
    }

    #[test]
    fn a_branch_slash_survives_but_a_percent_in_it_is_escaped() {
        assert_eq!(encode_repo_path("feature/100%-done"), "feature/100%25-done");
    }

    // -- the defect this module exists to prevent ----------------------------

    #[test]
    fn the_unencoded_url_produced_an_invalid_escape() {
        let broken =
            reqwest::Url::parse("https://raw.githubusercontent.com/o/r/main/pages/51% Attack.md")
                .expect("parses, but wrongly");
        assert!(
            broken.path().contains("%%"),
            "got {} — expected the bogus escape behind the 400",
            broken.path()
        );
    }

    #[test]
    fn the_unencoded_url_silently_decoded_a_literal_escape() {
        let wrong = reqwest::Url::parse(
            "https://api.github.com/repos/o/r/contents/pages/Presentation%3A Conclusion.md",
        )
        .expect("parses");
        let decoded = urlencoding::decode(wrong.path()).expect("utf-8");
        assert!(
            decoded.contains("Presentation: Conclusion.md"),
            "the server saw a colon, not the real filename: {decoded}"
        );
    }
}
