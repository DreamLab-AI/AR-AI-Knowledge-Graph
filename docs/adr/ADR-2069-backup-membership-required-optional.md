---
id: ADR-2069
title: Ratify the required/optional backup membership contract and refuse incomplete backup sets
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a new per-domain SQLite database is added under DATA_DIR, or Oxigraph gains a point-in-time backup mechanism
repo: visionclaw
---

# ADR-2069 — Ratify the required/optional backup membership contract and refuse incomplete backup sets

## Context
- `docs/DATA-authority-erasure.md` (Known divergences, "Backup coverage is partial") states that
  "missing databases are skipped if at least one backs up".
- The code does not do this. `scripts/backup-sqlite.sh:74-76` splits membership into
  `REQUIRED_DBS` (`settings.sqlite3 enrichment.sqlite3 kpi.sqlite3`) and `OPTIONAL_DBS`
  (`liveness.sqlite3`), with the comment "ADR-2017 membership: required members fail the run when absent".
- On a missing required member the script deletes the partial destination and `die`s, publishing no
  manifest (`:242-245`); a missing optional member is logged and the run continues (`:229`).
- Diagram VC-22.7 and VC-22.11 (Phase 1) exposed the contradiction; VC-22.11 carried it as DOC-DRIFT.
- The code behaviour is the fail-closed one and matches ADR-2017's write-master backup posture; the
  doc sentence describes a weaker, fail-open contract that was never implemented.
- Oxigraph still has no point-in-time backup: that half of the divergence bullet remains true.

## Decision
The backup set is a **membership contract, not a best-effort sweep**. `scripts/backup-sqlite.sh`
declares `REQUIRED_DBS` and `OPTIONAL_DBS`. A missing *required* database aborts the run, removes the
partial destination directory, and writes no `MANIFEST.txt` — an incomplete backup set is never
published, because a manifest that omits a write-master silently understates what was captured. A
missing *optional* database is logged and does not fail the run. Both lists stay overridable by
environment for ad-hoc runs. `docs/DATA-authority-erasure.md` is corrected to describe this contract.

This ADR ratifies existing behaviour: it is a DOC-CORRECT, not a code change.

## Consequences
- The documented erasure/backup posture now matches the shipped script, so an operator reading the
  governing doc can predict whether a run will publish.
- Adding a new per-domain SQLite database is a decision point: it must be placed in `REQUIRED_DBS` or
  `OPTIONAL_DBS` explicitly, and the `review_trigger` above forces that choice.
- **Unchanged and still open:** Oxigraph has no point-in-time backup, there is no cross-store
  consistent restore, and no RPO/RTO is declared. That remains a live divergence bullet in
  `docs/DATA-authority-erasure.md` and is out of scope here.

## Verification
Ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; must be re-run
at the landing commit.

```
$ sed -n '73,78p' scripts/backup-sqlite.sh
KEEP="${KEEP:-14}"                             # rotation depth
# ADR-2017 membership: required members fail the run when absent.
REQUIRED_DBS="${REQUIRED_DBS:-settings.sqlite3 enrichment.sqlite3 kpi.sqlite3}"
OPTIONAL_DBS="${OPTIONAL_DBS:-liveness.sqlite3}"

$ sed -n '240,248p' scripts/backup-sqlite.sh
if [[ -n "$missing_required" ]]; then
  rm -rf "$DEST"
  die "required database(s) missing from the source: $missing_required \
(declared REQUIRED_DBS=[$REQUIRED_DBS]) — refusing to publish an incomplete backup set"
fi
[[ -n "$missing_optional" ]] && log "optional database(s) absent, continuing: $missing_optional"
```

No code was changed. `docs/DATA-authority-erasure.md` was edited in the same change to replace the
incorrect sentence and to record this ADR under `## Remediation — 2026-09-05`.
