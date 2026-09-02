//! `vault-migrate` — convert a Logseq graph into an Obsidian vault.
//!
//! Exit codes: 0 success (leftovers are not a failure), 1 I/O or argument
//! failure, 2 `--check` found drift.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use vault_migrate::{run, Options};

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
The default mode writes to a new directory and never touches the source graph."
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
    };

    let outcome = run(&opts)?;
    let json = outcome.report.to_json();

    if a.dry_run && a.report.is_none() {
        print!("{json}");
    } else if !a.check || a.report.is_some() {
        let target = a.out.clone().unwrap_or_else(|| a.graph.clone());
        let path = a
            .report
            .clone()
            .unwrap_or_else(|| target.join("vault-migrate-report.json"));
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&path, &json)?;
        if !a.quiet {
            eprintln!("report written to {}", path.display());
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
