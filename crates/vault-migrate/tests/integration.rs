//! End-to-end conversion against the mini fixture graph.
//!
//! `fixtures/logseq-mini` exercises every V2/V3 rule; `fixtures/expected-vault`
//! is the byte-exact golden output. EXP-V05 (idempotence) is asserted three
//! ways: golden equality, a second independent run, and `--check` drift.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use vault_migrate::{run, Options};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
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
        let rel = e.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        m.insert(rel, std::fs::read(e.path()).unwrap());
    }
    m
}

fn assert_trees_equal(a: &Path, b: &Path) {
    let (ta, tb) = (tree(a), tree(b));
    let ka: Vec<&String> = ta.keys().collect();
    let kb: Vec<&String> = tb.keys().collect();
    assert_eq!(ka, kb, "file sets differ\n  {}\n  {}", a.display(), b.display());
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
    assert_eq!(r.embeds, 1, "only the page embed; the block embed is a leftover");
    assert_eq!(r.tasks, 6, "5 in Tasks.md + 1 in the journal; none in the fence");
    assert_eq!(r.multiword_tags, 2, "the fenced #[[not a tag]] is excluded");
    assert_eq!(r.asset_paths, 3, "2 in the body + 1 in a property value; the fenced one is excluded");
    assert_eq!(r.collapsed_dropped, 2, "one leading, one body-level");
    assert_eq!(r.id_dropped, 1);

    let l = &rep.leftovers;
    assert_eq!(l.block_refs.len(), 2);
    assert_eq!(l.block_refs.iter().map(|f| f.count).sum::<usize>(), 2);
    assert_eq!(l.body_properties.len(), 1);
    assert_eq!(l.body_properties[0].count, 2);
    assert_eq!(l.scheduled_deadline.len(), 1);
    assert_eq!(l.scheduled_deadline[0].count, 2);
    assert_eq!(l.whiteboards, vec!["whiteboards/board.whiteboard".to_string()]);
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
    let s = std::fs::read_to_string(&p).unwrap().replace("- [ ] write the converter", "- TODO write the converter");
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
    assert!(!out.join("logseq").exists(), "never at the live config path");
}

#[test]
fn dry_run_writes_nothing_but_still_reports() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    let mut o = base(fixture("logseq-mini"), Some(out.clone()));
    o.dry_run = true;
    let r = run(&o).unwrap();
    assert_eq!(r.report.pages_total, 10);
    assert!(!out.exists(), "dry run must not create the output directory");
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
    assert_eq!(std::fs::read_to_string(out.join("existing.md")).unwrap(), "keep me");
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
    assert_eq!(before, tree(&src), "output-dir mode must not touch the source");
}

#[test]
fn assets_and_whiteboards_are_copied_verbatim() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("v");
    run(&base(fixture("logseq-mini"), Some(out.clone()))).unwrap();
    for rel in ["assets/diagram.png", "assets/bundle.zip", "whiteboards/board.whiteboard", "README.md"] {
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

    assert!(graph.join("pages/Ns/Child.md").is_file(), "new path written");
    assert!(!graph.join("pages/Ns___Child.md").exists(), "legacy name removed");
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
