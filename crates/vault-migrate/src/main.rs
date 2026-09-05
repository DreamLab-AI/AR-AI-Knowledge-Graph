//! `vault-migrate` — convert a Logseq graph into an Obsidian vault.
//!
//! Exit codes: 0 success (leftovers are not a failure), 1 I/O or argument
//! failure, 2 `--check` found drift.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use vault_migrate::{run, CollisionPolicy, Options};

#[derive(Parser, Debug)]
#[command(
    name = "vault-migrate",
    version,
    about = "Convert a Logseq graph into an Obsidian vault (ADR-2042)",
    long_about = "Convert a Logseq graph into an Obsidian vault.\n\n\
Deterministic, offline and idempotent. The leading property block becomes YAML \
frontmatter, `Ns___Title.md` becomes `Ns/Title.md`, journals are renamed to \
`YYYY-MM-DD.md`, and the body dialect is rewritten to Obsidian's. Anything with \
no faithful equivalent (block refs, body-level properties) is preserved \
byte-for-byte and listed in the JSON report.\n\n\
The default mode writes to a new directory and never touches the source graph.\n\n\
--dry-run and --check produce NO vault output. The single permitted side effect \
in those modes is the JSON report, and only when --report <PATH> asks for it; \
without --report, --dry-run prints the report to stdout instead. The report \
records this in its report_side_effects field."
)]
struct Args {
    /// The Logseq graph directory (contains pages/, journals/, assets/).
    #[arg(value_name = "GRAPH_DIR")]
    graph: PathBuf,

    /// Write the vault here. The source graph is left untouched.
    #[arg(long, value_name = "VAULT_DIR")]
    out: Option<PathBuf>,

    /// Convert the graph directory itself. Refuses a dirty git tree.
    #[arg(long, conflicts_with = "out")]
    in_place: bool,

    /// Compute everything and write nothing; print the report to stdout.
    #[arg(long)]
    dry_run: bool,

    /// Exit 2 if any file would change. The CI hook for a converted vault.
    #[arg(long, conflicts_with = "dry_run")]
    check: bool,

    /// Report path. Default <VAULT_DIR>/vault-migrate-report.json.
    #[arg(long, value_name = "PATH")]
    report: Option<PathBuf>,

    /// Copy logseq/ to .logseq-archive/ instead of skipping it.
    #[arg(long)]
    keep_logseq_config: bool,

    /// Worker threads for page conversion. Default: one per core.
    #[arg(long, value_name = "N")]
    jobs: Option<usize>,

    /// Write into a non-empty output directory.
    #[arg(long)]
    force: bool,

    /// Allow --in-place on a dirty git working tree.
    #[arg(long)]
    allow_dirty: bool,

    /// Suppress the human summary on stderr.
    #[arg(long, short)]
    quiet: bool,

    /// What to do when two source pages map onto the same vault path
    /// (`Ns___Title.md` and `Ns/Title.md` both become `Ns/Title.md`).
    /// `fail` (default) refuses the run and names every collision; `suffix`
    /// keeps the first source at the natural path and gives the rest a
    /// ` (2)`, ` (3)` … suffix.
    #[arg(long, value_name = "MODE", default_value = "fail",
          value_parser = ["fail", "suffix"])]
    on_collision: String,
}

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("vault-migrate: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn real_main() -> Result<ExitCode> {
    let a = Args::parse();
    let opts = Options {
        graph: a.graph.clone(),
        out: a.out.clone(),
        in_place: a.in_place,
        dry_run: a.dry_run,
        check: a.check,
        report: a.report.clone(),
        keep_logseq_config: a.keep_logseq_config,
        jobs: a.jobs,
        quiet: a.quiet,
        force: a.force,
        allow_dirty: a.allow_dirty,
        on_collision: match a.on_collision.as_str() {
            "suffix" => CollisionPolicy::Suffix,
            _ => CollisionPolicy::Fail,
        },
    };

    let mut outcome = run(&opts)?;

    // ADR-2042 — report side effects, stated explicitly.
    //
    // `--dry-run` and `--check` write NO vault output; `run()` above returns
    // before touching the target tree. The one side effect either mode may have
    // is the JSON report, and only because `--report <PATH>` asked for it. That
    // is narrower than "writes nothing", so the artefact records what it did.
    let report_target = if a.dry_run && a.report.is_none() {
        None
    } else if !a.check || a.report.is_some() {
        let target = a.out.clone().unwrap_or_else(|| a.graph.clone());
        Some(
            a.report
                .clone()
                .unwrap_or_else(|| target.join("vault-migrate-report.json")),
        )
    } else {
        None
    };
    outcome.report.report_side_effects = match &report_target {
        Some(path) => vec![format!("wrote report to {}", path.display())],
        None => vec!["report printed to stdout; no file written".to_string()],
    };

    let json = outcome.report.to_json();
    match &report_target {
        None => print!("{json}"),
        Some(path) => {
            if let Some(p) = path.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::write(path, &json)?;
            if !a.quiet {
                eprintln!("report written to {}", path.display());
            }
        }
    }

    if !a.quiet {
        eprintln!("{}", outcome.report.summary());
    }

    if a.check && outcome.drift > 0 {
        if !a.quiet {
            eprintln!("\ndrift: {} file(s) would change", outcome.drift);
            for d in &outcome.drift_examples {
                eprintln!("  {d}");
            }
        }
        return Ok(ExitCode::from(2));
    }
    Ok(ExitCode::SUCCESS)
}
