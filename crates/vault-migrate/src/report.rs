//! Machine-readable evidence artefact (governing doc V6, ADR-2042 decision 4).
//!
//! The converter never guesses. Anything it could not faithfully express in
//! the Obsidian dialect is preserved byte-for-byte in the output and listed
//! here, per file, so a human can decide what to do about it.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileCount {
    pub file: String,
    pub count: usize,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Rules {
    pub public_true: usize,
    pub aliases: usize,
    pub namespace_moved: usize,
    pub journals_renamed: usize,
    pub embeds: usize,
    pub tasks: usize,
    pub multiword_tags: usize,
    pub asset_paths: usize,
    pub collapsed_dropped: usize,
    pub id_dropped: usize,
    pub title_echo_removed: usize,
    pub public_promoted_from_body: usize,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Leftovers {
    pub block_refs: Vec<FileCount>,
    pub body_properties: Vec<FileCount>,
    pub scheduled_deadline: Vec<FileCount>,
    pub whiteboards: Vec<String>,
}

/// One destination path that more than one source page maps onto (ADR-2042).
///
/// A Logseq graph can hold `Ns___Title.md` (legacy namespace encoding) *and*
/// `Ns/Title.md` (folder layout) at once; both decode to the same vault path.
/// The converter never silently keeps one body and drops the other — it either
/// refuses the run or applies the declared resolution, and records the fact
/// here either way.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Collision {
    /// The vault-relative destination two or more sources wanted.
    pub destination: String,
    /// Every source that mapped onto it, sorted.
    pub sources: Vec<String>,
    /// What the run did about it: `"rejected"` or the resolved destinations.
    pub resolution: String,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Report {
    pub source: String,
    pub output: String,
    pub mode: String,
    pub pages_total: usize,
    pub pages_converted: usize,
    pub pages_already_obsidian: usize,
    pub rules: Rules,
    pub leftovers: Leftovers,
    /// Destination collisions found while planning (ADR-2042).
    pub collisions: Vec<Collision>,
    /// What this run wrote outside the vault tree. ADR-2042 defines `--dry-run`
    /// and `--check` as producing **no vault output**; an explicitly requested
    /// `--report <PATH>` is the one permitted side effect, and it is recorded
    /// here so the artefact states its own provenance.
    pub report_side_effects: Vec<String>,
    pub errors: Vec<String>,
}

impl Report {
    pub fn to_json(&self) -> String {
        // Trailing newline so the file is a well-formed text file.
        format!("{}\n", serde_json::to_string_pretty(self).unwrap())
    }

    /// The human summary written to stderr.
    pub fn summary(&self) -> String {
        let r = &self.rules;
        let l = &self.leftovers;
        let sum = |v: &Vec<FileCount>| v.iter().map(|f| f.count).sum::<usize>();
        format!(
            "vault-migrate [{mode}]\n  \
             source            {source}\n  \
             output            {output}\n  \
             pages             {total} total, {conv} converted, {already} already Obsidian\n  \
             rules             public:true {pt}, aliases {al}, namespace moved {nm}, journals renamed {jr}\n  \
                               embeds {em}, tasks {tk}, multi-word tags {mt}, asset paths {ap}\n  \
                               collapsed dropped {cd}, id:: dropped {idd}\n  \
             leftovers         block refs {br_f} files / {br_n} refs\n  \
                               body properties {bp_f} files / {bp_n} lines\n  \
                               SCHEDULED/DEADLINE {sd_f} files / {sd_n} lines\n  \
                               whiteboards {wb} files\n  \
             errors            {err}",
            mode = self.mode,
            source = self.source,
            output = self.output,
            total = self.pages_total,
            conv = self.pages_converted,
            already = self.pages_already_obsidian,
            pt = r.public_true,
            al = r.aliases,
            nm = r.namespace_moved,
            jr = r.journals_renamed,
            em = r.embeds,
            tk = r.tasks,
            mt = r.multiword_tags,
            ap = r.asset_paths,
            cd = r.collapsed_dropped,
            idd = r.id_dropped,
            br_f = l.block_refs.len(),
            br_n = sum(&l.block_refs),
            bp_f = l.body_properties.len(),
            bp_n = sum(&l.body_properties),
            sd_f = l.scheduled_deadline.len(),
            sd_n = sum(&l.scheduled_deadline),
            wb = l.whiteboards.len(),
            err = self.errors.len(),
        )
    }
}
