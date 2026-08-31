#!/usr/bin/env bash
#
# backup-sqlite.sh — online-safe backups of the VisionClaw SQLite databases.
#
# Why this exists (PRD-014 / TODO-unified C-3):
#   data/{kpi,enrichment,settings,liveness}.sqlite3 had NO backup at all.
#   The LIVE databases are not the stale copies in the repo's data/ dir — they
#   live in the Docker named volume `visionclaw-data`, mounted at /app/data
#   inside `visionclaw_container`. All four run in WAL mode, so a naive `cp`
#   can capture a torn snapshot (committed pages still sitting in the -wal
#   file). This script uses the SQLite ONLINE BACKUP API — `.backup` — which
#   takes a lock-consistent snapshot across the main DB and its WAL.
#
# Two modes, auto-detected:
#   docker  — DBs live in visionclaw_container:/app/data. We run sqlite3 .backup
#             INSIDE the container, then `docker cp` the artefact out to a
#             host-visible backup dir (off the source volume, so a volume loss
#             does not take the backups with it).
#   host    — DBs are plain files in a local directory (DATA_DIR). We run
#             sqlite3 .backup directly. Used on machines where the DBs sit on a
#             host path rather than inside a container.
#
# Rotation: timestamped dir per run; keep the newest KEEP (default 14).
#
# Usage:
#   scripts/backup-sqlite.sh                 # auto-detect, default settings
#   BACKUP_ROOT=/srv/backups scripts/backup-sqlite.sh
#   MODE=host DATA_DIR=./data scripts/backup-sqlite.sh
#   KEEP=30 scripts/backup-sqlite.sh
#
set -euo pipefail

# ----- configuration (env-overridable) --------------------------------------
CONTAINER="${CONTAINER:-visionclaw_container}"
CONTAINER_DATA_DIR="${CONTAINER_DATA_DIR:-/app/data}"
DATA_DIR="${DATA_DIR:-./data}"                 # host-mode source dir
BACKUP_ROOT="${BACKUP_ROOT:-./data/backups}"   # where timestamped dirs land
KEEP="${KEEP:-14}"                             # rotation depth
DBS="${DBS:-kpi.sqlite3 enrichment.sqlite3 settings.sqlite3 liveness.sqlite3}"
MODE="${MODE:-auto}"

log() { printf '%s [backup-sqlite] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

command -v sqlite3 >/dev/null 2>&1 || log "note: host sqlite3 not found (only needed for host mode / verification)"

# ----- mode detection -------------------------------------------------------
detect_mode() {
  if [[ "$MODE" != "auto" ]]; then echo "$MODE"; return; fi
  if command -v docker >/dev/null 2>&1 \
     && docker inspect -f '{{.State.Running}}' "$CONTAINER" >/dev/null 2>&1; then
    echo docker
  else
    echo host
  fi
}
MODE="$(detect_mode)"
log "mode=$MODE keep=$KEEP dbs=[$DBS]"

# ----- destination ----------------------------------------------------------
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
DEST="${BACKUP_ROOT%/}/${STAMP}"
mkdir -p "$DEST"
log "destination: $DEST"

backup_one_docker() {
  local db="$1"
  local src="${CONTAINER_DATA_DIR%/}/$db"
  # Skip DBs that do not exist in the container (some may be unused).
  if ! docker exec "$CONTAINER" test -f "$src"; then
    log "skip $db (not present in $CONTAINER:$src)"
    return 0
  fi
  local tmp="/tmp/backup-${db}.${STAMP}"
  # .backup is the online backup API — safe against concurrent writers.
  docker exec "$CONTAINER" sqlite3 "$src" ".backup '$tmp'" \
    || die "sqlite3 .backup failed for $db in container"
  # Integrity-check the snapshot before we trust it.
  local chk
  chk="$(docker exec "$CONTAINER" sqlite3 "$tmp" 'PRAGMA integrity_check;' 2>&1)"
  [[ "$chk" == "ok" ]] || die "integrity_check failed for $db snapshot: $chk"
  docker cp "$CONTAINER:$tmp" "$DEST/$db" || die "docker cp failed for $db"
  docker exec "$CONTAINER" rm -f "$tmp" || true
  log "backed up $db ($(du -h "$DEST/$db" | cut -f1)) integrity=ok"
}

backup_one_host() {
  local db="$1"
  local src="${DATA_DIR%/}/$db"
  if [[ ! -f "$src" ]]; then
    log "skip $db (not present at $src)"
    return 0
  fi
  command -v sqlite3 >/dev/null 2>&1 || die "host mode needs sqlite3 on PATH"
  sqlite3 "$src" ".backup '$DEST/$db'" || die "sqlite3 .backup failed for $db"
  local chk
  chk="$(sqlite3 "$DEST/$db" 'PRAGMA integrity_check;' 2>&1)"
  [[ "$chk" == "ok" ]] || die "integrity_check failed for $db snapshot: $chk"
  log "backed up $db ($(du -h "$DEST/$db" | cut -f1)) integrity=ok"
}

# ----- run ------------------------------------------------------------------
count=0
for db in $DBS; do
  case "$MODE" in
    docker) backup_one_docker "$db" ;;
    host)   backup_one_host   "$db" ;;
    *)      die "unknown MODE=$MODE" ;;
  esac
  [[ -f "$DEST/$db" ]] && count=$((count + 1))
done
[[ "$count" -gt 0 ]] || die "no databases backed up — check CONTAINER/DATA_DIR"

# Manifest for the restore drill and for auditing what a run captured.
{
  echo "backup_utc=$STAMP"
  echo "mode=$MODE"
  echo "source=$([[ "$MODE" == docker ]] && echo "$CONTAINER:$CONTAINER_DATA_DIR" || echo "$DATA_DIR")"
  echo "databases=$count"
  for db in $DBS; do
    [[ -f "$DEST/$db" ]] && echo "sha256=$(sha256sum "$DEST/$db" | cut -d' ' -f1)  $db"
  done
} > "$DEST/MANIFEST.txt"
log "wrote $DEST/MANIFEST.txt ($count databases)"

# ----- rotation: keep newest KEEP timestamped dirs --------------------------
mapfile -t all < <(find "${BACKUP_ROOT%/}" -mindepth 1 -maxdepth 1 -type d -name '20*' | sort)
if (( ${#all[@]} > KEEP )); then
  prune=$(( ${#all[@]} - KEEP ))
  log "rotation: ${#all[@]} backups present, pruning oldest $prune (keep=$KEEP)"
  for ((i = 0; i < prune; i++)); do
    log "  removing ${all[$i]}"
    rm -rf "${all[$i]}"
  done
else
  log "rotation: ${#all[@]} backups present, under keep=$KEEP, nothing to prune"
fi

log "DONE — $count databases in $DEST"
echo "$DEST"
