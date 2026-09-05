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
    /// ADR-2042: what to do when two source pages map onto the same vault path.
    pub on_collision: CollisionPolicy,
}

/// How a destination collision is handled (ADR-2042).
///
/// A collision is never resolved implicitly. The planner detects every one
/// **before any file is written**, and the policy decides between refusing the
/// run and applying a declared, deterministic renaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollisionPolicy {
    /// Refuse the run, naming every colliding destination and its sources.
    /// The default: silently keeping one body and discarding the other is data
    /// loss, and the operator is the only one who can say which page wins.
    #[default]
    Fail,
    /// Keep the first source (in sorted order) at the natural destination and
    /// give each subsequent one a ` (2)`, ` (3)` … suffix before the extension.
    /// Deterministic, so a re-run produces the same vault.
    Suffix,
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
    /// A converted markdown file, or a generated config file. `source` is the
    /// page it came from, or `None` for generated starter config — it is what
    /// a collision report names.
    Write {
        rel: PathBuf,
        content: String,
        source: Option<PathBuf>,
    },
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

    fn rel_mut(&mut self) -> &mut PathBuf {
        match self {
            Action::Write { rel, .. } => rel,
            Action::Copy { rel, .. } => rel,
        }
    }

    /// The source this action carries, for collision reporting.
    fn source(&self) -> Option<&Path> {
        match self {
            Action::Write { source, .. } => source.as_deref(),
            Action::Copy { src, .. } => Some(src.as_path()),
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
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(j.max(1))
            .build_global();
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
                        actions.push(Action::Copy {
                            src: abs,
                            rel: Path::new("pages").join(&rel),
                        });
                        continue;
                    }
                    match paths::map_page(&rel) {
                        Some((out_rel, page_name)) => {
                            page_files.push((abs, Path::new("pages").join(out_rel), page_name))
                        }
                        None => actions.push(Action::Copy {
                            src: abs,
                            rel: Path::new("pages").join(&rel),
                        }),
                    }
                }
            }
            "journals" if is_dir => {
                for rel in collect_tree(path, &mut rep.errors) {
                    let abs = path.join(&rel);
                    if is_hidden(&rel) {
                        actions.push(Action::Copy {
                            src: abs,
                            rel: Path::new("journals").join(&rel),
                        });
                        continue;
                    }
                    match paths::map_journal(&rel) {
                        Some((out_rel, renamed)) => {
                            journal_files.push((abs, Path::new("journals").join(out_rel), renamed))
                        }
                        None => actions.push(Action::Copy {
                            src: abs,
                            rel: Path::new("journals").join(&rel),
                        }),
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
                    actions.push(Action::Copy {
                        src: path.join(&rel),
                        rel: r,
                    });
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
                    actions.push(Action::Copy {
                        src: path.clone(),
                        rel: PathBuf::from(name),
                    });
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
                let moved = Path::new("pages")
                    .join(src_rel.strip_prefix("pages").unwrap_or(src_rel))
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
                actions.push(Action::Write {
                    rel,
                    content: r.content,
                    source: Some(abs.clone()),
                });
            }
        }
    }
    for ((abs, _, renamed), res) in journal_files.iter().zip(journal_out) {
        match res {
            Err(e) => rep.errors.push(e),
            Ok((rel, r, _)) => {
                if *renamed {
                    rep.rules.journals_renamed += 1;
                }
                tally(&mut rep, &r.stats);
                collect_leftovers(&rel, &r.stats, &mut block_refs, &mut body_props, &mut sched);
                actions.push(Action::Write {
                    rel,
                    content: r.content,
                    source: Some(abs.clone()),
                });
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
        actions.push(Action::Write {
            rel,
            content,
            source: None,
        });
    }

    actions.sort_by(|a, b| a.rel().cmp(b.rel()));

    // --- ADR-2042: destination collisions, resolved before any write ------
    //
    // A graph can hold both `Ns___Title.md` and `Ns/Title.md`; both decode to
    // `pages/Ns/Title.md`. Left alone, the last action to be applied wins and
    // one page body is silently lost while both sources survive — precisely
    // the reproduced defect. Detect every collision here, on the plan, and
    // either refuse or apply the declared resolution.
    rep.collisions = resolve_collisions(&mut actions, opts.on_collision, source);
    if opts.on_collision == CollisionPolicy::Fail && !rep.collisions.is_empty() {
        bail!("{}", collision_failure_message(&rep.collisions));
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

    Ok(RunOutcome {
        report: rep,
        drift,
        drift_examples,
    })
}

// ---------------------------------------------------------------------------
// ADR-2042 — destination collisions
// ---------------------------------------------------------------------------

/// Find every destination two or more actions map onto and apply `policy`.
///
/// Returns one [`report::Collision`] per colliding destination, sorted, and —
/// under [`CollisionPolicy::Suffix`] — rewrites the losing actions' destinations
/// in place. Under [`CollisionPolicy::Fail`] the actions are left untouched and
/// the caller aborts, so nothing is written.
///
/// `source_root` is stripped from reported source paths so the report is
/// relocatable.
fn resolve_collisions(
    actions: &mut [Action],
    policy: CollisionPolicy,
    source_root: &Path,
) -> Vec<report::Collision> {
    use std::collections::BTreeMap;

    // Group action indices by destination. Actions are already sorted by
    // destination, and within a destination we take them in the order they were
    // planned, so the resolution is deterministic across runs.
    let mut by_dest: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (idx, a) in actions.iter().enumerate() {
        by_dest.entry(a.rel().to_path_buf()).or_default().push(idx);
    }

    let describe = |p: Option<&Path>| -> String {
        match p {
            Some(p) => p
                .strip_prefix(source_root)
                .unwrap_or(p)
                .display()
                .to_string(),
            None => "<generated starter config>".to_string(),
        }
    };

    // Destinations already claimed, so a suffixed name cannot collide in turn.
    let claimed: BTreeSet<PathBuf> = by_dest.keys().cloned().collect();
    let mut extra_claimed: BTreeSet<PathBuf> = BTreeSet::new();

    let mut collisions = Vec::new();
    for (dest, idxs) in by_dest {
        if idxs.len() < 2 {
            continue;
        }
        let mut sources: Vec<String> = idxs
            .iter()
            .map(|i| describe(actions[*i].source()))
            .collect();
        sources.sort();

        let resolution = match policy {
            CollisionPolicy::Fail => "rejected".to_string(),
            CollisionPolicy::Suffix => {
                let mut resolved = vec![dest.display().to_string()];
                // The first action keeps the natural destination.
                for i in idxs.iter().skip(1) {
                    let new_rel = next_free_suffixed(&dest, &claimed, &extra_claimed);
                    extra_claimed.insert(new_rel.clone());
                    resolved.push(new_rel.display().to_string());
                    *actions[*i].rel_mut() = new_rel;
                }
                format!("suffixed -> {}", resolved.join(", "))
            }
        };

        collisions.push(report::Collision {
            destination: dest.display().to_string(),
            sources,
            resolution,
        });
    }
    collisions.sort();
    collisions
}

/// `pages/Ns/Title.md` -> `pages/Ns/Title (2).md`, then ` (3)`, … skipping any
/// name another action already claims.
fn next_free_suffixed(
    dest: &Path,
    claimed: &BTreeSet<PathBuf>,
    extra: &BTreeSet<PathBuf>,
) -> PathBuf {
    let parent = dest.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = dest
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = dest.extension().map(|e| e.to_string_lossy().into_owned());

    for n in 2..=10_000u32 {
        let name = match &ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let candidate = parent.join(name);
        if !claimed.contains(&candidate) && !extra.contains(&candidate) {
            return candidate;
        }
    }
    // Unreachable in practice: 10k same-named pages. Fall back to a name that
    // is certainly free rather than looping forever or overwriting.
    let name = match &ext {
        Some(e) => format!("{stem} (collision-{}).{e}", extra.len()),
        None => format!("{stem} (collision-{})", extra.len()),
    };
    parent.join(name)
}

/// The operator-facing message for a refused run.
fn collision_failure_message(collisions: &[report::Collision]) -> String {
    let mut out = format!(
        "{} destination collision(s): more than one source page maps onto the same \
vault path. Refusing to write, because keeping one body and discarding the other \
would lose a page.\n",
        collisions.len()
    );
    for c in collisions {
        out.push_str(&format!(
            "  {} <- {}\n",
            c.destination,
            c.sources.join(", ")
        ));
    }
    out.push_str(
        "Resolve them in the source graph (rename or merge the pages), or re-run with \
--on-collision suffix to have the converter disambiguate deterministically.",
    );
    out
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
            let non_empty = fs::read_dir(out)
                .map(|mut d| d.next().is_some())
                .unwrap_or(false);
            if non_empty && !o.force {
                bail!(
                    "{} exists and is not empty; pass --force to write into it",
                    out.display()
                );
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
    for e in walkdir::WalkDir::new(root)
        .follow_links(true)
        .sort_by_file_name()
    {
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
        block_refs.push(FileCount {
            file: f.clone(),
            count: s.leftovers.block_refs,
        });
    }
    if s.leftovers.body_properties > 0 {
        body_props.push(FileCount {
            file: f.clone(),
            count: s.leftovers.body_properties,
        });
    }
    if s.leftovers.scheduled_deadline > 0 {
        sched.push(FileCount {
            file: f,
            count: s.leftovers.scheduled_deadline,
        });
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

/// Write `bytes` to `dest` via a sibling temporary file and an atomic rename.
///
/// The temporary lives beside the destination so the rename never crosses a
/// filesystem boundary. It is removed on any failure, so a failed run leaves
/// neither a partial destination nor a stray temporary.
fn write_atomically(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = temp_sibling(dest);
    match fs::write(&tmp, bytes).and_then(|()| fs::rename(&tmp, dest)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Copy `src` to `dest` via a sibling temporary file and an atomic rename.
fn copy_atomically(src: &Path, dest: &Path) -> std::io::Result<()> {
    let tmp = temp_sibling(dest);
    match fs::copy(src, &tmp).and_then(|_| fs::rename(&tmp, dest)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// A per-process, per-destination temporary name beside `dest`. Including the
/// pid keeps two concurrent converters from fighting over the same staging file.
fn temp_sibling(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".to_string());
    let parent = dest.parent().map(Path::to_path_buf).unwrap_or_default();
    parent.join(format!(".{name}.vault-migrate-{}.tmp", std::process::id()))
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn apply(a: &Action, dest: &Path, errors: &mut Vec<String>) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    match a {
        Action::Write { content, .. } => {
            // Skip the write when the bytes already match, so re-runs do not
            // churn mtimes across 8.6k files.
            if fs::read(dest)
                .map(|b| b == content.as_bytes())
                .unwrap_or(false)
            {
                return Ok(());
            }
            // ADR-2042: write atomically. An interrupted run (SIGKILL, full
            // disk, container stop) must never leave a truncated page behind
            // that a later --check would read as valid content; the temporary
            // file absorbs the partial write and the rename is atomic within
            // the destination filesystem.
            write_atomically(dest, content.as_bytes())
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
            // Copies are staged the same way, so a half-copied asset never
            // appears at its final path.
            if let Err(e) = copy_atomically(src, dest) {
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
