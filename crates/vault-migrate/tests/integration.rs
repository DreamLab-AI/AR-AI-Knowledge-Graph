//! End-to-end conversion against the mini fixture graph.
//!
//! `fixtures/logseq-mini` exercises every V2/V3 rule; `fixtures/expected-vault`
//! is the byte-exact golden output. EXP-V05 (idempotence) is asserted three
//! ways: golden equality, a second independent run, and `--check` drift.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use vault_migrate::{run, CollisionPolicy, Options};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn base(graph: PathBuf, out: Option<PathBuf>) -> Options {
    Options {
        graph,
        out,
        in_place: false,
        dry_run: false,
        check: false,
        report: None,
        keep_logseq_config: false,
        jobs: Some(2),
        quiet: true,
        force: false,
        allow_dirty: false,
        on_collision: CollisionPolicy::Fail,
    }
}

/// Every regular file under `root`, keyed by relative path.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut m = BTreeMap::new();
    for e in walkdir::WalkDir::new(root).sort_by_file_name() {
        let e = e.unwrap();
        if !e.file_type().is_file() {
            continue;
        }
        let rel = e
            .path()
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        m.insert(rel, std::fs::read(e.path()).unwrap());
    }
    m
}

fn assert_trees_equal(a: &Path, b: &Path) {
    let (ta, tb) = (tree(a), tree(b));
    let ka: Vec<&String> = ta.keys().collect();
    let kb: Vec<&String> = tb.keys().collect();
    assert_eq!(
        ka,
        kb,
        "file sets differ\n  {}\n  {}",
        a.display(),
        b.display()
    );
    for (k, va) in &ta {
        let vb = &tb[k];
        if va != vb {
            panic!(
                "{k} differs\n--- {} ---\n{}\n--- {} ---\n{}",
                a.display(),
                String::from_utf8_lossy(va),
                b.display(),
                String::from_utf8_lossy(vb)
            );
        }
    }
}

#[test]
fn converts_the_mini_graph_to_the_expected_vault() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("vault");
    let o = base(fixture("logseq-mini"), Some(out.clone()));
    let r = run(&o).unwrap();
    assert!(r.report.errors.is_empty(), "errors: {:?}", r.report.errors);
    assert_trees_equal(&out, &fixture("expected-vault"));
}

#[test]
fn report_counts_match_the_fixture_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let o = base(fixture("logseq-mini"), Some(tmp.path().join("v")));
    let rep = run(&o).unwrap().report;

    assert_eq!(rep.pages_total, 10);
    assert_eq!(rep.pages_already_obsidian, 1);
    assert_eq!(rep.pages_converted, 9);

    let r = &rep.rules;
    assert_eq!(r.public_true, 9, "one fixture page is public:: false");
    assert_eq!(r.aliases, 2);
    assert_eq!(r.namespace_moved, 2, "Ns___Child and TCP%2FIP");
    assert_eq!(r.journals_renamed, 1);
    assert_eq!(
        r.embeds, 1,
        "only the page embed; the block embed is a leftover"
    );
    assert_eq!(
        r.tasks, 6,
        "5 in Tasks.md + 1 in the journal; none in the fence"
    );
    assert_eq!(r.multiword_tags, 2, "the fenced #[[not a tag]] is excluded");
    assert_eq!(
        r.asset_paths, 3,
        "2 in the body + 1 in a property value; the fenced one is excluded"
    );
    assert_eq!(r.collapsed_dropped, 2, "one leading, one body-level");
    assert_eq!(r.id_dropped, 1);
    assert_eq!(
        r.title_echo_removed, 0,
        "Already Converted.md's title is confirmed by its H1 and kept"
    );

    let l = &rep.leftovers;
    assert_eq!(l.block_refs.len(), 2);
    assert_eq!(l.block_refs.iter().map(|f| f.count).sum::<usize>(), 2);
    assert_eq!(l.body_properties.len(), 1);
    assert_eq!(l.body_properties[0].count, 2);
    assert_eq!(l.scheduled_deadline.len(), 1);
    assert_eq!(l.scheduled_deadline[0].count, 2);
    assert_eq!(
        l.whiteboards,
        vec!["whiteboards/board.whiteboard".to_string()]
    );
}

#[test]
fn two_independent_runs_are_byte_identical() {
    let t1 = tempfile::tempdir().unwrap();
    let t2 = tempfile::tempdir().unwrap();
    let a = t1.path().join("v");
    let b = t2.path().join("v");
    run(&base(fixture("logseq-mini"), Some(a.clone()))).unwrap();
    run(&base(fixture("logseq-mini"), Some(b.clone()))).unwrap();
    assert_trees_equal(&a, &b);
}

/// EXP-V05: converting the converter's own output changes nothing.
#[test]
fn converting_the_vault_again_is_a_no_op() {
    let t1 = tempfile::tempdir().unwrap();
    let t2 = tempfile::tempdir().unwrap();
    let first = t1.path().join("v");
    let second = t2.path().join("v");
    run(&base(fixture("logseq-mini"), Some(first.clone()))).unwrap();
    run(&base(first.clone(), Some(second.clone()))).unwrap();
    assert_trees_equal(&first, &second);
}

#[test]
fn check_on_the_converted_vault_reports_no_drift() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    run(&base(fixture("logseq-mini"), Some(out.clone()))).unwrap();

    let mut o = base(out.clone(), None);
    o.check = true;
    let r = run(&o).unwrap();
    assert_eq!(r.drift, 0, "unexpected drift: {:?}", r.drift_examples);
}

#[test]
fn check_detects_drift_when_a_page_regresses() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    run(&base(fixture("logseq-mini"), Some(out.clone()))).unwrap();

    // Reintroduce a legacy construct.
    let p = out.join("pages/Tasks.md");
    let s = std::fs::read_to_string(&p)
        .unwrap()
        .replace("- [ ] write the converter", "- TODO write the converter");
    std::fs::write(&p, s).unwrap();

    let mut o = base(out, None);
    o.check = true;
    let r = run(&o).unwrap();
    assert_eq!(r.drift, 1, "examples: {:?}", r.drift_examples);
    assert!(r.drift_examples[0].contains("Tasks.md"));
}

#[test]
fn check_detects_a_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    run(&base(fixture("logseq-mini"), Some(out.clone()))).unwrap();
    std::fs::remove_file(out.join(".obsidian/app.json")).unwrap();

    let mut o = base(out, None);
    o.check = true;
    assert_eq!(run(&o).unwrap().drift, 1);
}

#[test]
fn logseq_config_is_skipped_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    run(&base(fixture("logseq-mini"), Some(out.clone()))).unwrap();
    assert!(!out.join("logseq").exists());
    assert!(!out.join(".logseq-archive").exists());
}

#[test]
fn logseq_config_is_archived_on_request() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    let mut o = base(fixture("logseq-mini"), Some(out.clone()));
    o.keep_logseq_config = true;
    run(&o).unwrap();
    assert!(out.join(".logseq-archive/config.edn").is_file());
    assert!(
        !out.join("logseq").exists(),
        "never at the live config path"
    );
}

#[test]
fn dry_run_writes_nothing_but_still_reports() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    let mut o = base(fixture("logseq-mini"), Some(out.clone()));
    o.dry_run = true;
    let r = run(&o).unwrap();
    assert_eq!(r.report.pages_total, 10);
    assert!(
        !out.exists(),
        "dry run must not create the output directory"
    );
}

#[test]
fn refuses_a_non_empty_output_directory_without_force() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("existing.md"), "keep me").unwrap();

    let o = base(fixture("logseq-mini"), Some(out.clone()));
    assert!(run(&o).is_err());

    let mut forced = base(fixture("logseq-mini"), Some(out.clone()));
    forced.force = true;
    run(&forced).unwrap();
    assert_eq!(
        std::fs::read_to_string(out.join("existing.md")).unwrap(),
        "keep me"
    );
}

#[test]
fn an_empty_output_directory_is_accepted() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    std::fs::create_dir_all(&out).unwrap();
    run(&base(fixture("logseq-mini"), Some(out.clone()))).unwrap();
    assert!(out.join("pages/Simple Page.md").is_file());
}

#[test]
fn out_and_in_place_are_mutually_exclusive() {
    let tmp = tempfile::tempdir().unwrap();
    let mut o = base(fixture("logseq-mini"), Some(tmp.path().join("v")));
    o.in_place = true;
    assert!(run(&o).is_err());
}

#[test]
fn the_source_graph_is_never_modified() {
    let src = fixture("logseq-mini");
    let before = tree(&src);
    let tmp = tempfile::tempdir().unwrap();
    run(&base(src.clone(), Some(tmp.path().join("v")))).unwrap();
    assert_eq!(
        before,
        tree(&src),
        "output-dir mode must not touch the source"
    );
}

#[test]
fn assets_and_whiteboards_are_copied_verbatim() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    run(&base(fixture("logseq-mini"), Some(out.clone()))).unwrap();
    for rel in [
        "assets/diagram.png",
        "assets/bundle.zip",
        "whiteboards/board.whiteboard",
        "README.md",
    ] {
        assert_eq!(
            std::fs::read(fixture("logseq-mini").join(rel)).unwrap(),
            std::fs::read(out.join(rel)).unwrap(),
            "{rel} must be byte-identical"
        );
    }
}

#[test]
fn in_place_completes_the_namespace_rename() {
    let tmp = tempfile::tempdir().unwrap();
    let graph = tmp.path().join("g");
    copy_dir(&fixture("logseq-mini"), &graph);

    let mut o = base(graph.clone(), None);
    o.in_place = true;
    o.allow_dirty = true;
    let r = run(&o).unwrap();
    assert!(r.report.errors.is_empty(), "{:?}", r.report.errors);

    assert!(
        graph.join("pages/Ns/Child.md").is_file(),
        "new path written"
    );
    assert!(
        !graph.join("pages/Ns___Child.md").exists(),
        "legacy name removed"
    );
    assert!(graph.join("journals/2026-09-02.md").is_file());
    assert!(!graph.join("journals/2026_09_02.md").exists());

    // And it settles: a --check pass over the result is clean.
    let mut c = base(graph, None);
    c.check = true;
    assert_eq!(run(&c).unwrap().drift, 0);
}

fn copy_dir(from: &Path, to: &Path) {
    for e in walkdir::WalkDir::new(from) {
        let e = e.unwrap();
        let rel = e.path().strip_prefix(from).unwrap();
        let dest = to.join(rel);
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(e.path(), &dest).unwrap();
        }
    }
}

// ---------------------------------------------------------------------------
// ADR-2042 acceptance — destination collisions, dry-run side effects,
// interrupted writes
// ---------------------------------------------------------------------------

/// A disposable graph directory, removed when the guard drops.
struct Graph(PathBuf);

impl Graph {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("adr2042-{tag}-{nanos}"));
        std::fs::create_dir_all(dir.join("pages")).expect("mkdir pages");
        std::fs::create_dir_all(dir.join("journals")).expect("mkdir journals");
        Graph(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Write a page at a graph-relative path.
    fn page(&self, rel: &str, body: &str) -> &Self {
        let p = self.0.join("pages").join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(p, body).expect("write page");
        self
    }

    fn journal(&self, rel: &str, body: &str) -> &Self {
        std::fs::write(self.0.join("journals").join(rel), body).expect("write journal");
        self
    }
}

impl Drop for Graph {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn out_dir(graph: &Graph) -> PathBuf {
    graph.path().with_extension("out")
}

/// A graph carrying the same page in both layouts: the legacy namespace
/// encoding and the folder layout. Both map to `pages/Ns/Title.md`.
fn colliding_graph(tag: &str) -> Graph {
    let g = Graph::new(tag);
    g.page("Ns___Title.md", "- legacy namespace body\n");
    g.page("Ns/Title.md", "- folder layout body\n");
    g
}

/// The reproduced defect: the CLI accepted colliding paths and kept only one
/// output body while preserving both sources. It must now refuse.
#[test]
fn colliding_destinations_are_rejected_before_any_write() {
    let g = colliding_graph("collide-fail");
    let out = out_dir(&g);
    let err = run(&base(g.path().to_path_buf(), Some(out.clone())))
        .expect_err("a destination collision must refuse the run");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("destination collision"),
        "expected a collision failure, got: {msg}"
    );
    assert!(msg.contains("pages/Ns/Title.md"), "message: {msg}");
    assert!(msg.contains("Ns___Title.md"), "message: {msg}");
    assert!(msg.contains("Ns/Title.md"), "message: {msg}");
    assert!(
        msg.contains("--on-collision suffix"),
        "the failure must state the resolution: {msg}"
    );

    // Nothing was written: the refusal happens on the plan.
    assert!(
        !out.exists() || tree(&out).is_empty(),
        "a refused run must not write a vault"
    );
    let _ = std::fs::remove_dir_all(&out);
}

/// The explicit resolution keeps every page body, deterministically named.
#[test]
fn suffix_policy_preserves_every_colliding_body() {
    let g = colliding_graph("collide-suffix");
    let out = out_dir(&g);
    let mut opts = base(g.path().to_path_buf(), Some(out.clone()));
    opts.on_collision = CollisionPolicy::Suffix;

    let outcome = run(&opts).expect("suffix policy resolves the collision");
    let written = tree(&out);

    assert!(
        written.contains_key("pages/Ns/Title.md"),
        "the first source keeps the natural path: {:?}",
        written.keys().collect::<Vec<_>>()
    );
    assert!(
        written.contains_key("pages/Ns/Title (2).md"),
        "the second source is suffixed: {:?}",
        written.keys().collect::<Vec<_>>()
    );

    // Both bodies survive — nothing was silently discarded.
    let all: String = written
        .iter()
        .filter(|(k, _)| k.starts_with("pages/Ns/"))
        .map(|(_, v)| String::from_utf8_lossy(v).into_owned())
        .collect();
    assert!(all.contains("legacy namespace body"), "bodies: {all}");
    assert!(all.contains("folder layout body"), "bodies: {all}");

    // The report names the collision and the resolution it applied.
    assert_eq!(outcome.report.collisions.len(), 1);
    let c = &outcome.report.collisions[0];
    assert_eq!(c.destination, "pages/Ns/Title.md");
    assert_eq!(c.sources.len(), 2);
    assert!(c.resolution.contains("suffixed"), "{}", c.resolution);
    assert!(c.resolution.contains("Title (2).md"), "{}", c.resolution);

    let _ = std::fs::remove_dir_all(&out);
}

/// Suffixing is deterministic: a second run produces the identical vault.
#[test]
fn suffix_resolution_is_deterministic_across_runs() {
    let g = colliding_graph("collide-determinism");
    let first = g.path().with_extension("out1");
    let second = g.path().with_extension("out2");

    for out in [&first, &second] {
        let mut opts = base(g.path().to_path_buf(), Some(out.clone()));
        opts.on_collision = CollisionPolicy::Suffix;
        run(&opts).expect("run");
    }
    assert_eq!(
        tree(&first),
        tree(&second),
        "suffixing must be deterministic"
    );
    let _ = std::fs::remove_dir_all(&first);
    let _ = std::fs::remove_dir_all(&second);
}

/// Three-way collisions get ` (2)` and ` (3)`, not two files fighting over one
/// suffix.
#[test]
fn three_way_collision_gets_distinct_suffixes() {
    let g = Graph::new("collide-three");
    g.page("Ns___Title.md", "- one\n");
    g.page("Ns%2FTitle.md", "- two\n");
    g.page("Ns/Title.md", "- three\n");
    let out = out_dir(&g);
    let mut opts = base(g.path().to_path_buf(), Some(out.clone()));
    opts.on_collision = CollisionPolicy::Suffix;

    run(&opts).expect("run");
    let written = tree(&out);
    for name in [
        "pages/Ns/Title.md",
        "pages/Ns/Title (2).md",
        "pages/Ns/Title (3).md",
    ] {
        assert!(
            written.contains_key(name),
            "missing {name}: {:?}",
            written.keys().collect::<Vec<_>>()
        );
    }
    let _ = std::fs::remove_dir_all(&out);
}

/// Mixed namespace/folder layouts that do NOT collide convert normally — the
/// check must not fire on ordinary graphs.
#[test]
fn mixed_layouts_without_collisions_convert_normally() {
    let g = Graph::new("mixed-ok");
    g.page("Ns___Alpha.md", "- alpha\n");
    g.page("Ns/Beta.md", "- beta\n");
    g.page("Gamma.md", "- gamma\n");
    let out = out_dir(&g);

    let outcome = run(&base(g.path().to_path_buf(), Some(out.clone()))).expect("run");
    assert!(outcome.report.collisions.is_empty());
    let written = tree(&out);
    for name in ["pages/Ns/Alpha.md", "pages/Ns/Beta.md", "pages/Gamma.md"] {
        assert!(written.contains_key(name), "missing {name}");
    }
    let _ = std::fs::remove_dir_all(&out);
}

/// Journals collide too: `2026_01_02.md` and `2026-01-02.md` both become
/// `journals/2026-01-02.md`.
#[test]
fn colliding_journals_are_rejected() {
    let g = Graph::new("collide-journal");
    g.journal("2026_01_02.md", "- logseq journal\n");
    g.journal("2026-01-02.md", "- converted journal\n");
    let out = out_dir(&g);

    let err = run(&base(g.path().to_path_buf(), Some(out.clone())))
        .expect_err("colliding journals must refuse the run");
    let msg = format!("{err:#}");
    assert!(msg.contains("journals/2026-01-02.md"), "message: {msg}");
    let _ = std::fs::remove_dir_all(&out);
}

/// Collisions are detected in `--check` too, so CI catches them without a run.
#[test]
fn check_mode_detects_collisions() {
    let g = colliding_graph("collide-check");
    let mut opts = base(g.path().to_path_buf(), None);
    opts.check = true;
    let err = run(&opts).expect_err("--check must surface the collision");
    assert!(format!("{err:#}").contains("destination collision"));
}

/// ADR-2042 dry-run side effects: no vault output, and the report is the one
/// permitted artefact.
#[test]
fn dry_run_produces_no_vault_output_and_reports_its_side_effects() {
    let g = Graph::new("dry-run-side-effects");
    g.page("Alpha.md", "- alpha\n");
    let out = out_dir(&g);
    let mut opts = base(g.path().to_path_buf(), Some(out.clone()));
    opts.dry_run = true;

    let outcome = run(&opts).expect("dry run");
    assert!(
        !out.exists() || tree(&out).is_empty(),
        "--dry-run must write no vault output"
    );
    assert_eq!(outcome.report.mode, "dry-run");
    // The library run itself has no side effects; the CLI records the report
    // write, which is the only one the mode permits.
    assert!(outcome.report.report_side_effects.is_empty());
    let _ = std::fs::remove_dir_all(&out);
}

/// The source graph is never touched by a refused or dry run.
#[test]
fn a_refused_run_leaves_the_source_graph_intact() {
    let g = colliding_graph("collide-source-intact");
    let before = tree(g.path());
    let out = out_dir(&g);
    let _ = run(&base(g.path().to_path_buf(), Some(out.clone())));
    assert_eq!(before, tree(g.path()), "the source graph must be untouched");
    let _ = std::fs::remove_dir_all(&out);
}

/// ADR-2042 interrupted writes: a truncated destination left by a killed run
/// is replaced with the complete body on the next run, and no staging file is
/// left behind.
#[test]
fn a_truncated_destination_is_repaired_by_the_next_run() {
    let g = Graph::new("interrupted");
    g.page("Alpha.md", "- alpha body that is long enough to truncate\n");
    let out = out_dir(&g);

    // First run produces the real output.
    run(&base(g.path().to_path_buf(), Some(out.clone()))).expect("first run");
    let complete = std::fs::read(out.join("pages/Alpha.md")).expect("read output");
    assert!(!complete.is_empty());

    // Simulate an interrupted write: half the bytes on disk.
    std::fs::write(out.join("pages/Alpha.md"), &complete[..complete.len() / 2]).expect("truncate");

    let mut opts = base(g.path().to_path_buf(), Some(out.clone()));
    opts.force = true;
    run(&opts).expect("second run");

    assert_eq!(
        std::fs::read(out.join("pages/Alpha.md")).expect("read"),
        complete,
        "the truncated page must be restored in full"
    );
    assert!(
        !tree(&out)
            .keys()
            .any(|k| k.contains("vault-migrate-") && k.ends_with(".tmp")),
        "no staging file may survive a completed run"
    );
    let _ = std::fs::remove_dir_all(&out);
}

/// `--check` sees a truncated destination as drift rather than silently
/// accepting it.
#[test]
fn check_reports_a_truncated_destination_as_drift() {
    let g = Graph::new("interrupted-check");
    g.page("Alpha.md", "- alpha body long enough to truncate\n");
    let out = out_dir(&g);
    run(&base(g.path().to_path_buf(), Some(out.clone()))).expect("first run");

    let complete = std::fs::read(out.join("pages/Alpha.md")).expect("read");
    std::fs::write(out.join("pages/Alpha.md"), &complete[..complete.len() / 2]).expect("truncate");

    let mut opts = base(g.path().to_path_buf(), Some(out.clone()));
    opts.check = true;
    let outcome = run(&opts).expect("check run");
    assert!(outcome.drift > 0, "a truncated page must register as drift");
    let _ = std::fs::remove_dir_all(&out);
}

/// Case-only differences are distinct destinations on a case-sensitive
/// filesystem and must not be reported as a collision there.
#[test]
fn case_differing_page_names_are_distinct_destinations() {
    let g = Graph::new("case");
    g.page("Alpha.md", "- lower\n");
    g.page("ALPHA.md", "- upper\n");
    let out = out_dir(&g);

    match run(&base(g.path().to_path_buf(), Some(out.clone()))) {
        Ok(outcome) => {
            // Case-sensitive filesystem: two destinations, no collision.
            assert!(outcome.report.collisions.is_empty());
            assert_eq!(
                tree(&out)
                    .keys()
                    .filter(|k| k.starts_with("pages/"))
                    .count(),
                2
            );
        }
        Err(e) => {
            // Case-insensitive filesystem: the fixture could not even be
            // created distinctly, so a collision report is the correct answer.
            assert!(format!("{e:#}").contains("collision"));
        }
    }
    let _ = std::fs::remove_dir_all(&out);
}
