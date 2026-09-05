//! ADR-2008 acceptance: dev-image rebuild decision covers every build input.
//!
//! The estate-review probe (`dev-build-input-probe.json`) reproduced three
//! decision fixtures against the old inline heuristic in
//! `scripts/rust-backend-wrapper.sh`:
//!
//! | edit                                       | old heuristic | required |
//! |--------------------------------------------|---------------|----------|
//! | crate Rust source (`crates/*/src/*.rs`)    | rebuild ✓     | rebuild  |
//! | crate CUDA kernel (`crates/*/**/*.cu`)     | **skipped ✗** | rebuild  |
//! | crate manifest (`crates/*/Cargo.toml`)     | **skipped ✗** | rebuild  |
//!
//! These tests drive the extracted decision (`scripts/lib/build-inputs.sh`)
//! against a synthetic fixture tree shaped like the real repository, so each
//! row above is an executable assertion. They also cover the build inputs no
//! timestamp can see: the cargo feature set and the `rerun-if-env-changed`
//! variables both build scripts declare (notably `CUDA_ARCH`, which the
//! wrapper itself recomputes from `nvidia-smi` on every start).

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<crate> has a repository root")
        .to_path_buf()
}

fn library() -> PathBuf {
    repo_root().join("scripts/lib/build-inputs.sh")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("adr2008-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        Scratch(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, body).expect("write fixture file");
}

/// Set a file's mtime to a fixed epoch second, so tests never race the clock.
fn set_mtime(path: &Path, epoch_secs: i64) {
    let out = Command::new("touch")
        .arg("-d")
        .arg(format!("@{epoch_secs}"))
        .arg(path)
        .output()
        .expect("run touch");
    assert!(
        out.status.success(),
        "touch failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Base epoch for the fixtures — an arbitrary fixed point, far from "now".
const T0: i64 = 1_700_000_000;

/// Build a fixture tree with the same shape as the repository: root manifests
/// and build script, a root `src/` with Rust and CUDA, and a `crates/` tree
/// with its own manifest, Rust sources, build script and CUDA kernels. Every
/// input is stamped at T0; the binary at T0 + 100, so nothing needs rebuilding.
fn fixture_tree(root: &Path) -> PathBuf {
    let files = [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "src/main.rs",
        "src/utils/kernel.cu",
        "src/utils/ptx/kernel.ptx",
        "crates/visionclaw-gpu/Cargo.toml",
        "crates/visionclaw-gpu/build.rs",
        "crates/visionclaw-gpu/src/lib.rs",
        "crates/visionclaw-gpu/src/cuda_sources/forces.cu",
        "crates/visionclaw-gpu/src/cuda_sources/forces.cuh",
        "crates/visionclaw-domain/Cargo.toml",
        "crates/visionclaw-domain/src/lib.rs",
        // Noise that must never influence the decision.
        "target/release/build/stale.rs",
        "node_modules/pkg/index.rs",
        "README.md",
        "docs/notes.md",
    ];
    for f in files {
        let p = root.join(f);
        write(&p, "// fixture\n");
        set_mtime(&p, T0);
    }

    let binary = root.join("target/release/visionclaw-server");
    write(&binary, "binary");
    set_mtime(&binary, T0 + 100);
    binary
}

/// Run one shell expression with the library sourced. Returns (stdout, ok).
fn run_shell(expr: &str) -> (String, bool) {
    let script = format!(". '{}'\n{expr}\n", library().display());
    let out = Command::new("bash")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run bash");
    (
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        out.status.success(),
    )
}

/// Evaluate `needs_rebuild` for a fixture tree. Returns (reason, rebuild?).
fn decide(root: &Path, binary: &Path, stamp: &Path, env: &[(&str, &str)]) -> (String, bool) {
    let exports: String = env
        .iter()
        .map(|(k, v)| format!("export {k}='{v}'\n"))
        .collect();
    let expr = format!(
        "{exports}if needs_rebuild '{}' '{}' '{}' 'gpu,ontology,dev-auth'; then exit 0; else exit 7; fi",
        binary.display(),
        root.display(),
        stamp.display()
    );
    let script = format!(". '{}'\n{expr}\n", library().display());
    let out = Command::new("bash")
        .arg("-c")
        .arg(script)
        .output()
        .expect("run bash");
    let reason = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let rebuild = match out.status.code() {
        Some(0) => true,
        Some(7) => false,
        other => panic!("unexpected exit {other:?}: {reason}"),
    };
    (reason, rebuild)
}

/// Write a stamp matching the environment the decision will be asked about, so
/// the environment half of the check passes and only timestamps are in play.
fn write_matching_stamp(stamp: &Path, env: &[(&str, &str)]) {
    let exports: String = env
        .iter()
        .map(|(k, v)| format!("export {k}='{v}'\n"))
        .collect();
    let (_, ok) = run_shell(&format!(
        "{exports}write_build_stamp '{}' 'gpu,ontology,dev-auth'",
        stamp.display()
    ));
    assert!(ok, "write_build_stamp failed");
}

/// The baseline environment used by every timestamp test.
const ENV: &[(&str, &str)] = &[
    ("CUDA_ARCH", "89"),
    ("CUDA_PATH", "/usr/local/cuda"),
    ("DOCKER_ENV", "1"),
    ("CARGO_BUILD_FEATURES", ""),
];

/// A tree with nothing newer than the binary skips cargo.
#[test]
fn untouched_tree_skips_the_build() {
    let scratch = Scratch::new("untouched");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(!rebuild, "expected a skip, got rebuild: {reason}");
    assert!(reason.contains("up to date"), "reason: {reason}");
}

/// Fixture row 1 — a crate Rust edit rebuilds (the old heuristic got this right).
#[test]
fn crate_rust_edit_rebuilds() {
    let scratch = Scratch::new("crate-rs");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    set_mtime(
        &scratch.path().join("crates/visionclaw-domain/src/lib.rs"),
        T0 + 200,
    );
    let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(rebuild, "a crate Rust edit must rebuild: {reason}");
}

/// Fixture row 2 — the reproduced miss. A crate CUDA kernel edit must rebuild;
/// the old heuristic globbed `*.cu` under `/app/src` only.
#[test]
fn crate_cuda_edit_rebuilds() {
    let scratch = Scratch::new("crate-cu");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    set_mtime(
        &scratch
            .path()
            .join("crates/visionclaw-gpu/src/cuda_sources/forces.cu"),
        T0 + 200,
    );
    let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(rebuild, "a crate CUDA edit must rebuild: {reason}");
}

/// A crate CUDA *header* is compiled into the kernel and is equally a build
/// input.
#[test]
fn crate_cuda_header_edit_rebuilds() {
    let scratch = Scratch::new("crate-cuh");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    set_mtime(
        &scratch
            .path()
            .join("crates/visionclaw-gpu/src/cuda_sources/forces.cuh"),
        T0 + 200,
    );
    let (_, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(rebuild, "a crate CUDA header edit must rebuild");
}

/// Fixture row 3 — the second reproduced miss. A crate manifest edit changes
/// features and dependencies; the old heuristic only stat'd the ROOT manifest.
#[test]
fn crate_manifest_edit_rebuilds() {
    let scratch = Scratch::new("crate-toml");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    set_mtime(
        &scratch.path().join("crates/visionclaw-gpu/Cargo.toml"),
        T0 + 200,
    );
    let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(rebuild, "a crate manifest edit must rebuild: {reason}");
}

/// A crate build script is a build input in its own right.
#[test]
fn crate_build_script_edit_rebuilds() {
    let scratch = Scratch::new("crate-buildrs");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    set_mtime(
        &scratch.path().join("crates/visionclaw-gpu/build.rs"),
        T0 + 200,
    );
    let (_, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(rebuild, "a crate build script edit must rebuild");
}

/// The root inputs the old heuristic did cover stay covered.
#[test]
fn root_inputs_still_rebuild() {
    for file in ["Cargo.toml", "Cargo.lock", "build.rs", "src/main.rs"] {
        let scratch = Scratch::new("root");
        let binary = fixture_tree(scratch.path());
        let stamp = scratch.path().join("target/.stamp");
        write_matching_stamp(&stamp, ENV);

        set_mtime(&scratch.path().join(file), T0 + 200);
        let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
        assert!(rebuild, "editing {file} must rebuild: {reason}");
    }
}

/// A pre-compiled PTX artefact is linked into the binary and is a build input.
#[test]
fn ptx_edit_rebuilds() {
    let scratch = Scratch::new("ptx");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    set_mtime(&scratch.path().join("src/utils/ptx/kernel.ptx"), T0 + 200);
    let (_, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(rebuild, "a PTX edit must rebuild");
}

/// Build outputs and vendored dependencies are not build inputs: touching them
/// must not force a pointless 12-minute rebuild on every restart.
#[test]
fn build_outputs_and_vendored_trees_are_not_inputs() {
    for file in ["target/release/build/stale.rs", "node_modules/pkg/index.rs"] {
        let scratch = Scratch::new("noise");
        let binary = fixture_tree(scratch.path());
        let stamp = scratch.path().join("target/.stamp");
        write_matching_stamp(&stamp, ENV);

        set_mtime(&scratch.path().join(file), T0 + 200);
        let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
        assert!(!rebuild, "{file} must not be a build input: {reason}");
    }
}

/// Documentation is not a build input either.
#[test]
fn documentation_is_not_a_build_input() {
    let scratch = Scratch::new("docs");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    set_mtime(&scratch.path().join("docs/notes.md"), T0 + 200);
    let (_, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(!rebuild, "a docs edit must not rebuild");
}

/// A GPU swap changes CUDA_ARCH, which both build scripts declare as
/// `rerun-if-env-changed`. No file changes, so no timestamp can see it — the
/// stamp signature must.
#[test]
fn cuda_arch_change_rebuilds_without_any_file_edit() {
    let scratch = Scratch::new("cuda-arch");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    let moved: Vec<(&str, &str)> = ENV
        .iter()
        .map(|(k, v)| {
            if *k == "CUDA_ARCH" {
                (*k, "75")
            } else {
                (*k, *v)
            }
        })
        .collect();
    let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, &moved);
    assert!(rebuild, "a CUDA_ARCH change must rebuild: {reason}");
    assert!(
        reason.contains("build environment changed"),
        "reason: {reason}"
    );
}

/// An unset variable and an empty one are different build environments.
#[test]
fn unset_and_empty_variables_are_distinguishable() {
    let (with_empty, ok1) = run_shell("export DOCKER_ENV=''; build_env_signature x");
    let (unset, ok2) = run_shell("unset DOCKER_ENV; build_env_signature x");
    assert!(ok1 && ok2);
    assert!(with_empty.contains("DOCKER_ENV=;"), "{with_empty}");
    assert!(unset.contains("DOCKER_ENV=<unset>"), "{unset}");
    assert_ne!(with_empty, unset);
}

/// A missing stamp means the environment cannot be verified, so we rebuild
/// rather than run a binary of unknown provenance.
#[test]
fn missing_stamp_rebuilds() {
    let scratch = Scratch::new("no-stamp");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.absent-stamp");

    let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(rebuild, "a missing stamp must rebuild: {reason}");
    assert!(reason.contains("no build stamp"), "reason: {reason}");
}

/// A missing binary always rebuilds.
#[test]
fn missing_binary_rebuilds() {
    let scratch = Scratch::new("no-binary");
    fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    let absent = scratch.path().join("target/release/absent");
    let (reason, rebuild) = decide(scratch.path(), &absent, &stamp, ENV);
    assert!(rebuild, "a missing binary must rebuild");
    assert!(reason.contains("no binary at"), "reason: {reason}");
}

/// An empty source tree cannot be reasoned about: rebuild rather than trust a
/// binary whose inputs we cannot see (a broken bind-mount looks exactly like
/// this, and the old heuristic silently ran the stale binary).
#[test]
fn empty_source_tree_rebuilds() {
    let scratch = Scratch::new("empty");
    let root = scratch.path().join("empty-root");
    std::fs::create_dir_all(&root).expect("mkdir");
    let binary = scratch.path().join("bin");
    write(&binary, "binary");
    set_mtime(&binary, T0 + 100);
    let stamp = scratch.path().join(".stamp");
    write_matching_stamp(&stamp, ENV);

    let (reason, rebuild) = decide(&root, &binary, &stamp, ENV);
    assert!(rebuild, "an empty source tree must rebuild: {reason}");
    assert!(reason.contains("no build inputs found"), "reason: {reason}");
}

/// A build input with exactly the binary's mtime rebuilds: same-second edits
/// are indistinguishable from later ones at whole-second resolution, so the
/// comparison is `<=`, not `<`.
#[test]
fn same_second_edit_rebuilds() {
    let scratch = Scratch::new("same-second");
    let binary = fixture_tree(scratch.path());
    let stamp = scratch.path().join("target/.stamp");
    write_matching_stamp(&stamp, ENV);

    set_mtime(&scratch.path().join("src/main.rs"), T0 + 100);
    let (reason, rebuild) = decide(scratch.path(), &binary, &stamp, ENV);
    assert!(
        rebuild,
        "an edit in the binary's own second must rebuild: {reason}"
    );
}

/// The real repository tree is covered: the inventory sees crate manifests and
/// crate CUDA that the old heuristic missed.
#[test]
fn real_repository_inventory_covers_crate_manifests_and_cuda() {
    let root = repo_root();
    let (out, ok) = run_shell(&format!(
        "list_build_inputs '{}'",
        root.join("crates").display()
    ));
    assert!(ok, "list_build_inputs failed");
    assert!(
        out.lines().any(|l| l.ends_with("/Cargo.toml")),
        "crate manifests must be inventoried"
    );
    assert!(
        out.lines().any(|l| l.ends_with(".cu")),
        "crate CUDA kernels must be inventoried"
    );
    assert!(
        out.lines().any(|l| l.ends_with("/build.rs")),
        "crate build scripts must be inventoried"
    );
    assert!(
        !out.lines().any(|l| l.contains("/target/")),
        "build outputs must be pruned"
    );
}

/// The wrapper must actually consume the shared inventory rather than carrying
/// its own copy of the globs.
#[test]
fn wrapper_sources_the_shared_inventory() {
    let wrapper = std::fs::read_to_string(repo_root().join("scripts/rust-backend-wrapper.sh"))
        .expect("read wrapper");
    assert!(
        wrapper.contains("lib/build-inputs.sh"),
        "the wrapper must source the shared build-input inventory"
    );
    assert!(
        wrapper.contains("needs_rebuild"),
        "the wrapper must use the shared decision"
    );
    assert!(
        !wrapper.contains("LATEST_CUDA"),
        "the old inline heuristic must be gone"
    );
}
