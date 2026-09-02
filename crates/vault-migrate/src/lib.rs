//! `vault-migrate` — the sole Logseq -> Obsidian converter (ADR-2042).
//!
//! Deterministic, offline, idempotent, and non-destructive: the default mode
//! writes to a fresh output directory and never touches the source graph.

pub mod body;
pub mod convert;
pub mod frontmatter;
pub mod obsidian;
pub mod paths;
pub mod report;

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use report::{FileCount, Report};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Options {
    pub graph: PathBuf,
    pub out: Option<PathBuf>,
    pub in_place: bool,
    pub dry_run: bool,
    pub check: bool,
    pub report: Option<PathBuf>,
    pub keep_logseq_config: bool,
    pub jobs: Option<usize>,
    pub quiet: bool,
    pub force: bool,
    pub allow_dirty: bool,
}

#[derive(Debug)]
pub struct RunOutcome {
    pub report: Report,
    /// Number of files that would change. Only meaningful under `--check`.
    pub drift: usize,
    pub drift_examples: Vec<String>,
}

#[derive(Debug)]
enum Action {
    /// A converted markdown file, or a generated config file.
    Write { rel: PathBuf, content: String },
    /// Anything the converter does not interpret: copied byte-for-byte.
    Copy { src: PathBuf, rel: PathBuf },
}

impl Action {
    fn rel(&self) -> &Path {
        match self {
            Action::Write { rel, .. } => rel,
            Action::Copy { rel, .. } => rel,
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(opts: &Options) -> Result<RunOutcome> {
    validate(opts)?;
    if let Some(j) = opts.jobs {
        // Best-effort: a second call in the same process is a no-op.
        let _ = rayon::ThreadPoolBuilder::new().num_threads(j.max(1)).build_global();
    }

    let source = &opts.graph;
    let target = target_dir(opts);

    let mut rep = Report {
        source: source.display().to_string(),
        output: target.display().to_string(),
        mode: mode_name(opts).to_string(),
        ..Default::default()
    };

    let mut actions: Vec<Action> = Vec::new();
    let mut block_refs: Vec<FileCount> = Vec::new();
    let mut body_props: Vec<FileCount> = Vec::new();
    let mut sched: Vec<FileCount> = Vec::new();

    // --- top-level classification -----------------------------------------
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for e in fs::read_dir(source).with_context(|| format!("reading {}", source.display()))? {
        let e = e?;
        entries.push((e.file_name().to_string_lossy().into_owned(), e.path()));
    }
    entries.sort();

    let mut page_files: Vec<(PathBuf, PathBuf, String)> = Vec::new(); // (abs, rel_out, page_name)
    let mut journal_files: Vec<(PathBuf, PathBuf, bool)> = Vec::new(); // (abs, rel_out, renamed)

    for (name, path) in &entries {
        let is_dir = fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false);
        match name.as_str() {
            "pages" if is_dir => {
                for rel in collect_tree(path, &mut rep.errors) {
                    let abs = path.join(&rel);
                    if is_hidden(&rel) {
                        actions.push(Action::Copy { src: abs, rel: Path::new("pages").join(&rel) });
                        continue;
                    }
                    match paths::map_page(&rel) {
                        Some((out_rel, page_name)) => {
                            page_files.push((abs, Path::new("pages").join(out_rel), page_name))
                        }
                        None => actions
                            .push(Action::Copy { src: abs, rel: Path::new("pages").join(&rel) }),
                    }
                }
            }
            "journals" if is_dir => {
                for rel in collect_tree(path, &mut rep.errors) {
                    let abs = path.join(&rel);
                    if is_hidden(&rel) {
                        actions
                            .push(Action::Copy { src: abs, rel: Path::new("journals").join(&rel) });
                        continue;
                    }
                    match paths::map_journal(&rel) {
                        Some((out_rel, renamed)) => journal_files.push((
                            abs,
                            Path::new("journals").join(out_rel),
                            renamed,
                        )),
                        None => actions
                            .push(Action::Copy { src: abs, rel: Path::new("journals").join(&rel) }),
                    }
                }
            }
            // Logseq's own config: skipped, or archived out of the way so it
            // never looks like live vault config.
            "logseq" if is_dir => {
                if opts.keep_logseq_config {
                    for rel in collect_tree(path, &mut rep.errors) {
                        actions.push(Action::Copy {
                            src: path.join(&rel),
                            rel: Path::new(".logseq-archive").join(&rel),
                        });
                    }
                }
            }
            "whiteboards" if is_dir => {
                for rel in collect_tree(path, &mut rep.errors) {
                    let r = Path::new("whiteboards").join(&rel);
                    rep.leftovers.whiteboards.push(r.display().to_string());
                    actions.push(Action::Copy { src: path.join(&rel), rel: r });
                }
            }
            // Managed by this converter; an existing one is never overwritten.
            ".obsidian" => {}
            _ => {
                if is_dir {
                    for rel in collect_tree(path, &mut rep.errors) {
                        actions.push(Action::Copy {
                            src: path.join(&rel),
                            rel: Path::new(name).join(&rel),
                        });
                    }
                } else {
                    actions.push(Action::Copy { src: path.clone(), rel: PathBuf::from(name) });
                }
            }
        }
    }

    page_files.sort();
    journal_files.sort();

    // --- convert (parallel, order-preserving) ------------------------------
    let page_out: Vec<Result<(PathBuf, convert::PageResult, bool), String>> = page_files
        .par_iter()
        .map(|(abs, rel_out, page_name)| match read_text(abs) {
            Ok(text) => {
                let r = convert::convert_page(&text, page_name);
                let changed = r.content != text;
                Ok((rel_out.clone(), r, changed))
            }
            Err(e) => Err(format!("{}: {e}", abs.display())),
        })
        .collect();

    let journal_out: Vec<Result<(PathBuf, convert::PageResult, bool), String>> = journal_files
        .par_iter()
        .map(|(abs, rel_out, _)| match read_text(abs) {
            Ok(text) => {
                let r = convert::convert_journal(&text);
                let changed = r.content != text;
                Ok((rel_out.clone(), r, changed))
            }
            Err(e) => Err(format!("{}: {e}", abs.display())),
        })
        .collect();

    // --- accumulate --------------------------------------------------------
    for ((abs, rel_out, _), res) in page_files.iter().zip(page_out) {
        match res {
            Err(e) => rep.errors.push(e),
            Ok((rel, r, changed)) => {
                rep.pages_total += 1;
                let src_rel = abs.strip_prefix(source).unwrap_or(abs);
                let moved = Path::new("pages").join(src_rel.strip_prefix("pages").unwrap_or(src_rel))
                    != *rel_out;
                if r.stats.already_obsidian {
                    rep.pages_already_obsidian += 1;
                }
                if changed || moved {
                    rep.pages_converted += 1;
                }
                if moved {
                    rep.rules.namespace_moved += 1;
                }
                tally(&mut rep, &r.stats);
                collect_leftovers(&rel, &r.stats, &mut block_refs, &mut body_props, &mut sched);
                actions.push(Action::Write { rel, content: r.content });
            }
        }
    }
    for ((_, _, renamed), res) in journal_files.iter().zip(journal_out) {
        match res {
            Err(e) => rep.errors.push(e),
            Ok((rel, r, _)) => {
                if *renamed {
                    rep.rules.journals_renamed += 1;
                }
                tally(&mut rep, &r.stats);
                collect_leftovers(&rel, &r.stats, &mut block_refs, &mut body_props, &mut sched);
                actions.push(Action::Write { rel, content: r.content });
            }
        }
    }

    // --- starter config: only where nothing already claims the path --------
    let claimed: BTreeSet<PathBuf> = actions.iter().map(|a| a.rel().to_path_buf()).collect();
    for (rel, content) in obsidian::config_files() {
        let rel = PathBuf::from(rel);
        if claimed.contains(&rel) || target.join(&rel).exists() {
            continue;
        }
        actions.push(Action::Write { rel, content });
    }

    actions.sort_by(|a, b| a.rel().cmp(b.rel()));

    block_refs.sort();
    body_props.sort();
    sched.sort();
    rep.leftovers.whiteboards.sort();
    rep.leftovers.block_refs = block_refs;
    rep.leftovers.body_properties = body_props;
    rep.leftovers.scheduled_deadline = sched;

    // --- execute or compare ------------------------------------------------
    let mut drift = 0usize;
    let mut drift_examples = Vec::new();

    for a in &actions {
        let dest = target.join(a.rel());
        if opts.check {
            if let Some(why) = differs(a, &dest)? {
                drift += 1;
                if drift_examples.len() < 20 {
                    drift_examples.push(format!("{}: {why}", a.rel().display()));
                }
            }
            continue;
        }
        if opts.dry_run {
            continue;
        }
        apply(a, &dest, &mut rep.errors)?;
    }

    // In-place mode leaves the pre-move original behind unless we rename it.
    if !opts.check && !opts.dry_run && opts.in_place {
        rename_moved_originals(source, &page_files, &journal_files, &mut rep.errors);
    }

    Ok(RunOutcome { report: rep, drift, drift_examples })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn mode_name(o: &Options) -> &'static str {
    if o.check {
        "check"
    } else if o.dry_run {
        "dry-run"
    } else if o.in_place {
        "in-place"
    } else {
        "out"
    }
}

fn target_dir(o: &Options) -> PathBuf {
    match &o.out {
        Some(p) => p.clone(),
        None => o.graph.clone(),
    }
}

fn validate(o: &Options) -> Result<()> {
    if !o.graph.is_dir() {
        bail!("{} is not a directory", o.graph.display());
    }
    if o.in_place && o.out.is_some() {
        bail!("--in-place and --out are mutually exclusive");
    }
    if !o.in_place && !o.check && o.out.is_none() {
        bail!("one of --out <VAULT_DIR>, --in-place or --check is required");
    }
    if o.in_place && !o.allow_dirty && !o.dry_run && !o.check && git_is_dirty(&o.graph) {
        bail!(
            "{} has a dirty git working tree; commit, stash, or pass --allow-dirty",
            o.graph.display()
        );
    }
    if let Some(out) = &o.out {
        if !o.check && !o.dry_run && out.exists() {
            let non_empty = fs::read_dir(out).map(|mut d| d.next().is_some()).unwrap_or(false);
            if non_empty && !o.force {
                bail!("{} exists and is not empty; pass --force to write into it", out.display());
            }
        }
        if out.starts_with(&o.graph) && o.graph.starts_with(out) {
            bail!("--out must not be the source graph; use --in-place");
        }
    }
    Ok(())
}

fn git_is_dirty(dir: &Path) -> bool {
    match std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["status", "--porcelain"])
        .output()
    {
        Ok(o) if o.status.success() => !o.stdout.is_empty(),
        // Not a git repo (or no git): nothing to protect.
        _ => false,
    }
}

/// Relative paths of every regular file under `root`, sorted, symlinks followed.
///
/// Following symlinks is required by the real corpus: `mainKnowledgeGraph/assets`
/// is a symlink to a sibling graph's asset store.
fn collect_tree(root: &Path, errors: &mut Vec<String>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in walkdir::WalkDir::new(root).follow_links(true).sort_by_file_name() {
        match e {
            Ok(e) if e.file_type().is_file() => {
                if let Ok(rel) = e.path().strip_prefix(root) {
                    out.push(rel.to_path_buf());
                }
            }
            Ok(_) => {}
            Err(err) => errors.push(format!("walk {}: {err}", root.display())),
        }
    }
    out.sort();
    out
}

fn is_hidden(rel: &Path) -> bool {
    rel.components().any(|c| {
        matches!(c, std::path::Component::Normal(os)
            if os.to_string_lossy().starts_with('.'))
    })
}

fn read_text(p: &Path) -> Result<String, String> {
    match fs::read(p) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) => Err(e.to_string()),
    }
}

fn tally(rep: &mut Report, s: &convert::PageStats) {
    if s.public_true {
        rep.rules.public_true += 1;
    }
    if s.has_aliases {
        rep.rules.aliases += 1;
    }
    rep.rules.embeds += s.counts.embeds;
    rep.rules.tasks += s.counts.tasks;
    rep.rules.multiword_tags += s.counts.multiword_tags;
    rep.rules.asset_paths += s.counts.asset_paths + s.frontmatter_asset_paths;
    rep.rules.collapsed_dropped += s.collapsed_dropped;
    rep.rules.id_dropped += s.id_dropped;
    rep.rules.title_echo_removed += s.title_echo_removed;
    rep.rules.public_promoted_from_body += s.public_promoted_from_body;
}

fn collect_leftovers(
    rel: &Path,
    s: &convert::PageStats,
    block_refs: &mut Vec<FileCount>,
    body_props: &mut Vec<FileCount>,
    sched: &mut Vec<FileCount>,
) {
    let f = rel.display().to_string();
    if s.leftovers.block_refs > 0 {
        block_refs.push(FileCount { file: f.clone(), count: s.leftovers.block_refs });
    }
    if s.leftovers.body_properties > 0 {
        body_props.push(FileCount { file: f.clone(), count: s.leftovers.body_properties });
    }
    if s.leftovers.scheduled_deadline > 0 {
        sched.push(FileCount { file: f, count: s.leftovers.scheduled_deadline });
    }
}

/// `Some(reason)` when applying this action would change the target.
///
/// Generated markdown is compared by content. Copies are compared by size:
/// hashing a 961 MB asset store on every `--check` would make the CI hook
/// unusable, and a same-size binary rewrite is not a case this converter
/// can cause.
fn differs(a: &Action, dest: &Path) -> Result<Option<String>> {
    match a {
        Action::Write { content, .. } => match fs::read(dest) {
            Ok(existing) => {
                if existing == content.as_bytes() {
                    Ok(None)
                } else {
                    Ok(Some("content would change".into()))
                }
            }
            Err(_) => Ok(Some("would be created".into())),
        },
        Action::Copy { src, .. } => {
            if same_file(src, dest) {
                return Ok(None);
            }
            let s = fs::metadata(src).map(|m| m.len()).unwrap_or(0);
            match fs::metadata(dest) {
                Ok(d) if d.len() == s => Ok(None),
                Ok(_) => Ok(Some("size would change".into())),
                Err(_) => Ok(Some("would be copied".into())),
            }
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn apply(a: &Action, dest: &Path, errors: &mut Vec<String>) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    match a {
        Action::Write { content, .. } => {
            // Skip the write when the bytes already match, so re-runs do not
            // churn mtimes across 8.6k files.
            if fs::read(dest).map(|b| b == content.as_bytes()).unwrap_or(false) {
                return Ok(());
            }
            fs::write(dest, content)
                .with_context(|| format!("writing {}", dest.display()))?;
        }
        Action::Copy { src, .. } => {
            if same_file(src, dest) {
                return Ok(());
            }
            let src_len = fs::metadata(src).map(|m| m.len()).ok();
            if let (Some(sl), Ok(dm)) = (src_len, fs::metadata(dest)) {
                if dm.len() == sl {
                    return Ok(());
                }
            }
            if let Err(e) = fs::copy(src, dest) {
                errors.push(format!("copy {} -> {}: {e}", src.display(), dest.display()));
            }
        }
    }
    Ok(())
}

/// In `--in-place` mode a renamed page has already been written to its new
/// path. Completing the rename means removing the legacy-named original —
/// and only ever that: the new file must exist and be non-empty first, so a
/// failed write can never cost the owner a page.
fn rename_moved_originals(
    source: &Path,
    pages: &[(PathBuf, PathBuf, String)],
    journals: &[(PathBuf, PathBuf, bool)],
    errors: &mut Vec<String>,
) {
    let mut moves: Vec<(&PathBuf, PathBuf)> = Vec::new();
    for (abs, rel_out, _) in pages {
        let new_path = source.join(rel_out);
        if new_path != *abs {
            moves.push((abs, new_path));
        }
    }
    for (abs, rel_out, _) in journals {
        let new_path = source.join(rel_out);
        if new_path != *abs {
            moves.push((abs, new_path));
        }
    }
    for (old, new_path) in moves {
        let old_len = fs::metadata(old).map(|m| m.len()).unwrap_or(0);
        match fs::metadata(&new_path) {
            // A zero-byte original legitimately renames to a zero-byte page.
            Ok(m) if m.len() > 0 || old_len == 0 => {
                if let Err(e) = fs::remove_file(old) {
                    errors.push(format!("removing superseded {}: {e}", old.display()));
                }
            }
            _ => errors.push(format!(
                "refusing to remove {}: {} was not written",
                old.display(),
                new_path.display()
            )),
        }
    }
}
