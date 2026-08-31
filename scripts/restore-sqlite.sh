#!/usr/bin/env bash
#
# restore-sqlite.sh — restore a VisionClaw SQLite database from a backup taken
# by backup-sqlite.sh.
#
# Restoring OVERWRITES live data. This is destructive and irreversible, so it
# refuses to touch a live target unless you pass --yes. By default it restores
# to a temp path so you can inspect first (a restore *drill*), which is also
# what the test in the runbook exercises.
#
# Usage:
#   # Dry restore to a temp file you can inspect (safe, no --yes needed):
#   scripts/restore-sqlite.sh --backup data/backups/<STAMP> --db kpi.sqlite3 \
#       --to /tmp/kpi.restored.sqlite3
#
#   # Restore INTO the live container DB (requires --yes):
#   scripts/restore-sqlite.sh --backup data/backups/<STAMP> --db kpi.sqlite3 \
#       --into-container --yes
#
#   # Restore to a host path (requires --yes when target already exists):
#   scripts/restore-sqlite.sh --backup data/backups/<STAMP> --db kpi.sqlite3 \
#       --to ./data/kpi.sqlite3 --yes
#
set -euo pipefail

CONTAINER="${CONTAINER:-visionclaw_container}"
CONTAINER_DATA_DIR="${CONTAINER_DATA_DIR:-/app/data}"

BACKUP_DIR=""; DB=""; TO=""; INTO_CONTAINER=0; YES=0

log() { printf '%s [restore-sqlite] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

usage() { grep '^#' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-0}"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --backup) BACKUP_DIR="$2"; shift 2 ;;
    --db)     DB="$2"; shift 2 ;;
    --to)     TO="$2"; shift 2 ;;
    --into-container) INTO_CONTAINER=1; shift ;;
    --yes)    YES=1; shift ;;
    -h|--help) usage 0 ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
done

[[ -n "$BACKUP_DIR" ]] || die "--backup <dir> is required"
[[ -n "$DB" ]] || die "--db <name.sqlite3> is required"
SRC="${BACKUP_DIR%/}/$DB"
[[ -f "$SRC" ]] || die "backup not found: $SRC"

# Verify the backup itself is intact before we rely on it.
command -v sqlite3 >/dev/null 2>&1 || die "sqlite3 required on PATH"
chk="$(sqlite3 "$SRC" 'PRAGMA integrity_check;' 2>&1)"
[[ "$chk" == "ok" ]] || die "backup failed integrity_check ($SRC): $chk"
log "backup verified ok: $SRC"

if [[ "$INTO_CONTAINER" == 1 ]]; then
  TARGET="${CONTAINER_DATA_DIR%/}/$DB"
  log "TARGET: live database $CONTAINER:$TARGET"
  [[ "$YES" == 1 ]] || die "refusing to overwrite live container DB without --yes"
  # .restore pulls the backup file into the live DB via the online API, which
  # correctly resets the WAL. We copy the backup in, then restore from it.
  tmp="/tmp/restore-${DB}.$$"
  docker cp "$SRC" "$CONTAINER:$tmp" || die "docker cp into container failed"
  docker exec "$CONTAINER" sqlite3 "$TARGET" ".restore '$tmp'" \
    || die "sqlite3 .restore failed in container"
  docker exec "$CONTAINER" rm -f "$tmp" || true
  post="$(docker exec "$CONTAINER" sqlite3 "$TARGET" 'PRAGMA integrity_check;' 2>&1)"
  [[ "$post" == "ok" ]] || die "post-restore integrity_check failed: $post"
  log "DONE — restored $DB into $CONTAINER:$TARGET (integrity=ok)"
  exit 0
fi

# Host / temp restore.
[[ -n "$TO" ]] || die "either --into-container or --to <path> is required"
if [[ -e "$TO" && "$YES" != 1 ]]; then
  die "target exists ($TO) — pass --yes to overwrite"
fi
mkdir -p "$(dirname "$TO")"
# .restore rather than cp so WAL state is handled cleanly on the target.
rm -f "$TO" "$TO-wal" "$TO-shm"
sqlite3 "$TO" ".restore '$SRC'" || die "sqlite3 .restore failed"
post="$(sqlite3 "$TO" 'PRAGMA integrity_check;' 2>&1)"
[[ "$post" == "ok" ]] || die "post-restore integrity_check failed: $post"
log "DONE — restored $DB to $TO (integrity=ok)"
