// Build-time PTX acceptance policy (ADR-2030) — pure, dependency-free logic
// shared by `build.rs` and the library's test suite.
//
// # Why this file is `include!`d by the build script
//
// A build script cannot depend on the crate it builds, so build-time logic is
// normally untestable and drifts from whatever the library believes. The
// closeout for ADR-2030 had to *extract* the PTX phase into a separate probe to
// test it at all, which proves the point: nothing guaranteed the probe still
// matched the build script.
//
// This module is therefore compiled twice from one source of truth —
// `include!`d into `build.rs`, and declared as `pub mod ptx_policy` in the
// library so the tests below run against the exact code the build executes. It
// uses only `std`, so the include is free of dependency concerns.
//
// # What the closeout found, and what this fixes
//
// | Finding | Policy here |
// |---|---|
// | `nvcc` absent panicked *before* the fallback was consulted | [`NvccOutcome::LaunchFailed`] is distinct from [`NvccOutcome::CompilerFailed`], and both reach the fallback |
// | A successful compiler writing `NOT PTX` passed the non-empty gate | [`validate_ptx`] checks directives and required symbols, not just length |
// | The `.version` rewrite spliced a fixed 12-byte window, so `9.10` became `9.00` | [`rewrite_ptx_version`] parses the version token and rewrites by span |
// | A downgrade warning was emitted even when nothing changed | [`VersionRewrite`] reports `Unchanged` distinctly from `Rewritten` |
// | Nothing recorded which module was selected, or its content identity | [`PtxArtefact`] records source, original and rewritten digests |
//
// Digests here are **not** cryptographic and are never used as a security
// boundary: they are FNV-1a content tags for matching a built artefact to the
// source it came from in a build manifest.

use std::fmt;

/// The highest PTX ISA version the target driver is assumed to accept. CUDA 13.x
/// toolkits emit `.version 9.x`; drivers in the field may only accept 9.0.
pub const TARGET_PTX_ISA: PtxVersion = PtxVersion { major: 9, minor: 0 };

/// A parsed PTX `.version` token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PtxVersion {
    pub major: u32,
    pub minor: u32,
}

impl fmt::Display for PtxVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // PTX writes a single-digit minor as `9.0`, not `9.00`.
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// How the compiler invocation ended (ADR-2030).
///
/// The closeout's first finding is the distinction this enum exists to make: a
/// **missing executable** and a **compiler that ran and rejected the source** are
/// different failures, and the old code panicked on the former before it ever
/// looked for a fallback. A missing toolchain is exactly the case the fallback
/// PTX was shipped for, so panicking there defeated its whole purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NvccOutcome {
    /// The process could not be started at all — nvcc not installed, not on
    /// `PATH`, or not executable. The fallback must be consulted.
    LaunchFailed { reason: String },
    /// nvcc ran and exited non-zero: the source or the host compiler is at
    /// fault. The fallback must also be consulted, but this is a *different*
    /// diagnosis and must be reported as such.
    CompilerFailed { code: Option<i32> },
    /// nvcc exited zero. Output still has to be validated — a zero exit is not
    /// evidence that the file contains usable PTX.
    Succeeded,
}

impl NvccOutcome {
    /// Classify an invocation. `spawn_error` is `Some` when the process could not
    /// be started; otherwise `success`/`code` describe how it exited.
    pub fn classify(spawn_error: Option<String>, success: bool, code: Option<i32>) -> Self {
        match spawn_error {
            Some(reason) => NvccOutcome::LaunchFailed { reason },
            None if success => NvccOutcome::Succeeded,
            None => NvccOutcome::CompilerFailed { code },
        }
    }

    /// Whether this outcome means the pre-compiled fallback should be used.
    /// Both failure modes fall back; only the *reporting* differs.
    pub fn needs_fallback(&self) -> bool {
        !matches!(self, NvccOutcome::Succeeded)
    }

    /// A short diagnosis for the build log, naming which failure occurred.
    pub fn diagnosis(&self) -> String {
        match self {
            NvccOutcome::LaunchFailed { reason } => {
                format!("nvcc could not be launched ({reason}) — CUDA toolkit missing or not on PATH")
            }
            NvccOutcome::CompilerFailed { code } => format!(
                "nvcc ran and failed (exit {}) — source or host-compiler error",
                code.map(|c| c.to_string()).unwrap_or_else(|| "signal".into())
            ),
            NvccOutcome::Succeeded => "nvcc succeeded".to_string(),
        }
    }
}

/// Where a module's PTX ultimately came from, recorded per build (ADR-2030).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtxProvenance {
    /// Freshly compiled by nvcc in this build.
    Compiled,
    /// Copied from a pre-compiled file because nvcc could not be launched.
    FallbackAfterLaunchFailure,
    /// Copied from a pre-compiled file because nvcc ran and failed.
    FallbackAfterCompilerFailure,
}

impl PtxProvenance {
    /// The provenance implied by a failing outcome.
    pub fn for_fallback(outcome: &NvccOutcome) -> Option<Self> {
        match outcome {
            NvccOutcome::LaunchFailed { .. } => Some(PtxProvenance::FallbackAfterLaunchFailure),
            NvccOutcome::CompilerFailed { .. } => Some(PtxProvenance::FallbackAfterCompilerFailure),
            NvccOutcome::Succeeded => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PtxProvenance::Compiled => "compiled",
            PtxProvenance::FallbackAfterLaunchFailure => "fallback-launch-failure",
            PtxProvenance::FallbackAfterCompilerFailure => "fallback-compiler-failure",
        }
    }
}

/// Why a candidate PTX file is unacceptable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtxDefect {
    /// Zero bytes — the old code caught this one.
    Empty,
    /// No `.version` directive: not PTX at all.
    MissingVersion,
    /// No `.target` directive: no declared architecture.
    MissingTarget,
    /// No `.entry` directive: no kernels, so nothing is launchable.
    NoEntryPoints,
    /// A kernel the runtime will look up by name is absent. This is the check
    /// that catches a compiler which succeeded on a *different* or truncated
    /// source: the file is syntactically PTX but not the PTX we need.
    MissingSymbol { name: String },
    /// The `.version` token is present but not parseable as `major.minor`.
    MalformedVersion { token: String },
}

impl fmt::Display for PtxDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PtxDefect::Empty => write!(f, "PTX file is empty"),
            PtxDefect::MissingVersion => write!(f, "no .version directive — not PTX"),
            PtxDefect::MissingTarget => write!(f, "no .target directive"),
            PtxDefect::NoEntryPoints => write!(f, "no .entry directive — no launchable kernels"),
            PtxDefect::MissingSymbol { name } => write!(f, "required kernel `{name}` is absent"),
            PtxDefect::MalformedVersion { token } => {
                write!(f, "unparseable .version token `{token}`")
            }
        }
    }
}

/// Locate the `.version` directive and return its numeric token plus the byte
/// span that token occupies.
///
/// Returned span covers **only the number**, so a rewrite replaces exactly the
/// version characters however many there are. The old code assumed the token was
/// always three characters (`9.0`) and spliced a fixed 12-byte window from the
/// start of `.version`, which silently turned `9.10` into `9.00` — a *lower*
/// version than either the original or the target.
pub fn find_version_token(ptx: &str) -> Option<(&str, std::ops::Range<usize>)> {
    let directive = ptx.find(".version")?;
    let after = directive + ".version".len();
    let bytes = ptx.as_bytes();
    let mut start = after;
    while start < bytes.len() && (bytes[start] == b' ' || bytes[start] == b'\t') {
        start += 1;
    }
    let mut end = start;
    while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
        end += 1;
    }
    if end == start {
        return None;
    }
    Some((&ptx[start..end], start..end))
}

/// Parse a `major.minor` PTX version token.
pub fn parse_version_token(token: &str) -> Option<PtxVersion> {
    let (major, minor) = token.split_once('.')?;
    if major.is_empty() || minor.is_empty() {
        return None;
    }
    Some(PtxVersion {
        major: major.parse().ok()?,
        minor: minor.parse().ok()?,
    })
}

/// Result of applying the ISA downgrade policy to a PTX file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionRewrite {
    /// Already at or below the target: content untouched. Reported distinctly so
    /// the build log cannot claim a downgrade that did not happen — the closeout
    /// observed nine "downgrade" warnings on content that never changed.
    Unchanged { version: PtxVersion },
    /// Rewritten down to the target.
    Rewritten {
        from: PtxVersion,
        to: PtxVersion,
        text: String,
    },
    /// No `.version` directive, or one that will not parse.
    Defective(PtxDefect),
}

/// Downgrade a PTX file's declared ISA to `target` when it declares a higher one.
///
/// Rewrites by parsed span, so a two-digit minor is handled correctly. Note what
/// this does and does not mean: changing the declared ISA makes the driver accept
/// the module for JIT, but it does **not** prove every instruction in the body is
/// supported by that ISA. A downgrade is a compatibility *attempt*, and only a
/// real load on the target driver settles it.
pub fn rewrite_ptx_version(ptx: &str, target: PtxVersion) -> VersionRewrite {
    let Some((token, span)) = find_version_token(ptx) else {
        return VersionRewrite::Defective(PtxDefect::MissingVersion);
    };
    let Some(found) = parse_version_token(token) else {
        return VersionRewrite::Defective(PtxDefect::MalformedVersion {
            token: token.to_string(),
        });
    };
    if found <= target {
        return VersionRewrite::Unchanged { version: found };
    }
    let mut text = String::with_capacity(ptx.len());
    text.push_str(&ptx[..span.start]);
    text.push_str(&target.to_string());
    text.push_str(&ptx[span.end..]);
    VersionRewrite::Rewritten {
        from: found,
        to: target,
        text,
    }
}

/// Validate a candidate PTX file's structure and required kernel symbols.
///
/// A non-empty file is not a valid one: the closeout showed a fake compiler
/// writing the literal text `NOT PTX` passing the old length-only gate. Required
/// symbols are the strongest cheap check available at build time — they catch a
/// compiler that succeeded against stale or partial source.
pub fn validate_ptx(ptx: &str, required_symbols: &[&str]) -> Result<(), PtxDefect> {
    if ptx.trim().is_empty() {
        return Err(PtxDefect::Empty);
    }
    let Some((token, _)) = find_version_token(ptx) else {
        return Err(PtxDefect::MissingVersion);
    };
    if parse_version_token(token).is_none() {
        return Err(PtxDefect::MalformedVersion {
            token: token.to_string(),
        });
    }
    if !ptx.contains(".target") {
        return Err(PtxDefect::MissingTarget);
    }
    if !ptx.contains(".entry") {
        return Err(PtxDefect::NoEntryPoints);
    }
    for name in required_symbols {
        if !ptx.contains(name) {
            return Err(PtxDefect::MissingSymbol {
                name: (*name).to_string(),
            });
        }
    }
    Ok(())
}

/// A non-cryptographic FNV-1a content tag.
///
/// Used only to bind a built artefact to the bytes it came from in the build
/// manifest — never as a security or integrity boundary, which is why a real
/// hash function is deliberately not pulled into the build script for it.
pub fn content_tag(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// The record a build emits for one PTX module (ADR-2030).
///
/// Recording provenance and both content tags is what makes "which module is
/// actually loaded?" answerable after the fact. File recency — the runtime
/// loader's current tie-breaker — proves nothing about source revision or ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtxArtefact {
    /// Module name, e.g. `visionclaw_unified`.
    pub module: String,
    /// Source path compiled, or the fallback path copied.
    pub source: String,
    /// How this artefact was obtained.
    pub provenance: PtxProvenance,
    /// Declared ISA after any rewrite.
    pub isa: PtxVersion,
    /// Content tag of the bytes as produced/copied, before any rewrite.
    pub original_tag: u64,
    /// Content tag after the ISA rewrite. Equal to `original_tag` when unchanged.
    pub rewritten_tag: u64,
}

impl PtxArtefact {
    /// Whether the ISA rewrite actually altered the bytes.
    pub fn was_rewritten(&self) -> bool {
        self.original_tag != self.rewritten_tag
    }

    /// One manifest line, stable enough to diff between builds.
    pub fn manifest_line(&self) -> String {
        format!(
            "{} source={} provenance={} isa={} original={:016x} rewritten={:016x}",
            self.module,
            self.source,
            self.provenance.as_str(),
            self.isa,
            self.original_tag,
            self.rewritten_tag
        )
    }
}

/// Kernels the runtime looks up by name in the unified module. A build whose
/// unified PTX lacks one of these has silently produced something unusable.
pub const REQUIRED_UNIFIED_SYMBOLS: [&str; 2] = ["force_pass_kernel", "integrate_pass_kernel"];

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_ptx(version: &str) -> String {
        format!(
            ".version {version}\n.target sm_75\n.address_size 64\n\
             .visible .entry force_pass_kernel(.param .u32 n) {{ ret; }}\n\
             .visible .entry integrate_pass_kernel(.param .u32 n) {{ ret; }}\n"
        )
    }

    // ── Launch failure is not compiler failure ─────────────────────────────

    #[test]
    fn a_missing_compiler_is_classified_apart_from_a_failing_one() {
        let missing = NvccOutcome::classify(Some("No such file".into()), false, None);
        let failed = NvccOutcome::classify(None, false, Some(1));
        assert!(matches!(missing, NvccOutcome::LaunchFailed { .. }));
        assert!(matches!(failed, NvccOutcome::CompilerFailed { code: Some(1) }));
        assert_ne!(missing, failed);

        // The closeout's first finding: a missing executable used to panic before
        // the fallback was consulted, defeating the fallback's entire purpose.
        assert!(missing.needs_fallback(), "a missing toolkit must fall back");
        assert!(failed.needs_fallback(), "a failing compiler must fall back");
        assert!(missing.diagnosis().contains("PATH"));
        assert!(failed.diagnosis().contains("exit 1"));
    }

    #[test]
    fn success_neither_falls_back_nor_implies_valid_output() {
        let ok = NvccOutcome::classify(None, true, Some(0));
        assert_eq!(ok, NvccOutcome::Succeeded);
        assert!(!ok.needs_fallback());
        assert_eq!(PtxProvenance::for_fallback(&ok), None);
        // A zero exit says nothing about the file; validation is separate.
        assert_eq!(validate_ptx("NOT PTX", &[]), Err(PtxDefect::MissingVersion));
    }

    #[test]
    fn fallback_provenance_records_which_failure_caused_it() {
        assert_eq!(
            PtxProvenance::for_fallback(&NvccOutcome::LaunchFailed { reason: "x".into() }),
            Some(PtxProvenance::FallbackAfterLaunchFailure)
        );
        assert_eq!(
            PtxProvenance::for_fallback(&NvccOutcome::CompilerFailed { code: Some(2) }),
            Some(PtxProvenance::FallbackAfterCompilerFailure)
        );
    }

    // ── Version token parsing and the fixed-width splice bug ───────────────

    #[test]
    fn a_two_digit_minor_is_rewritten_correctly() {
        // The closeout's headline defect: the fixed 12-byte splice turned
        // `.version 9.10` into `.version 9.00` — a version lower than either the
        // original or the target, produced silently.
        let ptx = minimal_ptx("9.10");
        match rewrite_ptx_version(&ptx, TARGET_PTX_ISA) {
            VersionRewrite::Rewritten { from, to, text } => {
                assert_eq!(from, PtxVersion { major: 9, minor: 10 });
                assert_eq!(to, TARGET_PTX_ISA);
                assert!(text.contains(".version 9.0\n"), "got: {text}");
                assert!(!text.contains("9.00"), "the 9.00 splice bug is gone");
                // Nothing after the token was disturbed.
                assert!(text.contains(".target sm_75"));
                assert!(text.contains("force_pass_kernel"));
            }
            other => panic!("expected a rewrite, got {other:?}"),
        }
    }

    #[test]
    fn an_already_compliant_version_is_reported_unchanged_not_downgraded() {
        // The closeout saw nine "downgrade" warnings on content that never
        // changed. Unchanged and Rewritten are now distinct outcomes.
        for v in ["9.0", "8.7", "1.0"] {
            let ptx = minimal_ptx(v);
            match rewrite_ptx_version(&ptx, TARGET_PTX_ISA) {
                VersionRewrite::Unchanged { .. } => {}
                other => panic!("{v} must be unchanged, got {other:?}"),
            }
        }
    }

    #[test]
    fn higher_majors_and_minors_both_downgrade() {
        for (v, major, minor) in [("9.1", 9, 1), ("9.7", 9, 7), ("10.0", 10, 0)] {
            match rewrite_ptx_version(&minimal_ptx(v), TARGET_PTX_ISA) {
                VersionRewrite::Rewritten { from, text, .. } => {
                    assert_eq!(from, PtxVersion { major, minor });
                    assert!(text.contains(".version 9.0\n"));
                }
                other => panic!("{v} must downgrade, got {other:?}"),
            }
        }
    }

    #[test]
    fn version_tokens_parse_by_span_not_by_fixed_width() {
        let (token, span) = find_version_token(".version 9.10\n.target sm_75").unwrap();
        assert_eq!(token, "9.10");
        assert_eq!(span.len(), 4, "span covers the number only");
        // Tab and multi-space separation both parse.
        assert_eq!(find_version_token(".version\t7.5\n").unwrap().0, "7.5");
        assert_eq!(find_version_token(".version   7.5\n").unwrap().0, "7.5");
        assert_eq!(parse_version_token("9.10"), Some(PtxVersion { major: 9, minor: 10 }));
        assert_eq!(parse_version_token("9"), None);
        assert_eq!(parse_version_token("9."), None);
        assert_eq!(parse_version_token("x.y"), None);
    }

    #[test]
    fn a_malformed_version_is_a_defect_not_a_silent_pass() {
        let ptx = ".version ...\n.target sm_75\n.entry k(){ret;}";
        assert!(matches!(
            rewrite_ptx_version(ptx, TARGET_PTX_ISA),
            VersionRewrite::Defective(PtxDefect::MalformedVersion { .. })
        ));
    }

    #[test]
    fn a_file_without_a_version_directive_is_defective() {
        assert!(matches!(
            rewrite_ptx_version("nothing here", TARGET_PTX_ISA),
            VersionRewrite::Defective(PtxDefect::MissingVersion)
        ));
    }

    #[test]
    fn displaying_a_version_never_zero_pads_the_minor() {
        assert_eq!(PtxVersion { major: 9, minor: 0 }.to_string(), "9.0");
        assert_eq!(PtxVersion { major: 9, minor: 10 }.to_string(), "9.10");
    }

    // ── Output validation beyond "non-empty" ───────────────────────────────

    #[test]
    fn a_nonempty_file_that_is_not_ptx_is_rejected() {
        // The exact closeout fixture: a successful fake compiler writing "NOT PTX"
        // passed the old length-only gate.
        assert_eq!(validate_ptx("NOT PTX", &[]), Err(PtxDefect::MissingVersion));
        assert_eq!(validate_ptx("", &[]), Err(PtxDefect::Empty));
        assert_eq!(validate_ptx("   \n\t ", &[]), Err(PtxDefect::Empty));
    }

    #[test]
    fn each_structural_directive_is_required() {
        assert_eq!(
            validate_ptx(".version 9.0\n.entry k(){ret;}", &[]),
            Err(PtxDefect::MissingTarget)
        );
        assert_eq!(
            validate_ptx(".version 9.0\n.target sm_75\n", &[]),
            Err(PtxDefect::NoEntryPoints)
        );
    }

    #[test]
    fn required_kernel_symbols_are_checked_by_name() {
        // Catches a compiler that succeeded against stale or partial source: the
        // file is syntactically PTX but is not the PTX this build needs.
        let ptx = minimal_ptx("9.0");
        assert_eq!(validate_ptx(&ptx, &REQUIRED_UNIFIED_SYMBOLS), Ok(()));

        let partial = ".version 9.0\n.target sm_75\n.visible .entry other_kernel(){ret;}\n";
        assert_eq!(
            validate_ptx(partial, &REQUIRED_UNIFIED_SYMBOLS),
            Err(PtxDefect::MissingSymbol {
                name: "force_pass_kernel".into()
            })
        );
    }

    #[test]
    fn a_valid_module_passes_every_gate() {
        let ptx = minimal_ptx("9.0");
        assert_eq!(validate_ptx(&ptx, &REQUIRED_UNIFIED_SYMBOLS), Ok(()));
        assert!(matches!(
            rewrite_ptx_version(&ptx, TARGET_PTX_ISA),
            VersionRewrite::Unchanged { .. }
        ));
    }

    // ── Artefact identity ──────────────────────────────────────────────────

    #[test]
    fn content_tags_distinguish_a_rewritten_artefact_from_its_original() {
        let original = minimal_ptx("9.10");
        let VersionRewrite::Rewritten { text, .. } =
            rewrite_ptx_version(&original, TARGET_PTX_ISA)
        else {
            panic!("expected a rewrite");
        };
        let artefact = PtxArtefact {
            module: "visionclaw_unified".into(),
            source: "src/cuda_sources/visionclaw_unified.cu".into(),
            provenance: PtxProvenance::Compiled,
            isa: TARGET_PTX_ISA,
            original_tag: content_tag(original.as_bytes()),
            rewritten_tag: content_tag(text.as_bytes()),
        };
        assert!(artefact.was_rewritten(), "the bytes did change");
        assert_ne!(artefact.original_tag, artefact.rewritten_tag);

        let line = artefact.manifest_line();
        assert!(line.contains("visionclaw_unified"));
        assert!(line.contains("provenance=compiled"));
        assert!(line.contains("isa=9.0"));
    }

    #[test]
    fn an_unrewritten_artefact_reports_equal_tags() {
        let ptx = minimal_ptx("9.0");
        let tag = content_tag(ptx.as_bytes());
        let artefact = PtxArtefact {
            module: "pagerank".into(),
            source: "src/ptx/pagerank.ptx".into(),
            provenance: PtxProvenance::FallbackAfterLaunchFailure,
            isa: TARGET_PTX_ISA,
            original_tag: tag,
            rewritten_tag: tag,
        };
        assert!(!artefact.was_rewritten());
        assert!(artefact
            .manifest_line()
            .contains("provenance=fallback-launch-failure"));
    }

    #[test]
    fn content_tags_are_stable_and_sensitive() {
        assert_eq!(content_tag(b"abc"), content_tag(b"abc"));
        assert_ne!(content_tag(b"abc"), content_tag(b"abd"));
        assert_ne!(content_tag(b""), content_tag(b"a"));
    }
}
