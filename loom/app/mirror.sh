#!/usr/bin/env bash
# Mirror the published ontology artifacts from narrativegoldmine.com into ./data/
# Idempotent + timestamp-aware (--time-cond): only downloads when the remote is newer.
# Safe to run any time; safe to cron (suggested: hourly).
set -euo pipefail

SITE="${ONTOLOGY_SITE:-https://narrativegoldmine.com}"
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/data"
mkdir -p "$DIR"

fetch() {
  local path="$1" out="$DIR/$2"
  local args=(-fsSL --retry 3 --connect-timeout 10 -o "$out.tmp" "$SITE/$path")
  if [[ -f "$out" ]]; then args+=(--time-cond "$out"); fi
  if curl "${args[@]}"; then
    if [[ -s "$out.tmp" ]]; then mv "$out.tmp" "$out"; echo "updated  $2 ($(du -h "$out" | cut -f1))";
    else rm -f "$out.tmp"; echo "current  $2"; fi
  else
    rm -f "$out.tmp"; echo "FAILED   $2 (kept previous copy if any)" >&2
  fi
}

fetch data/scaffold-index.json    scaffold-index.json
fetch data/prose-index.json       prose-index.json
fetch data/ontology.ttl           ontology.ttl
fetch data/ontology-inferred.ttl  ontology-inferred.ttl

echo "---"
if command -v python3 >/dev/null && [[ -f "$DIR/scaffold-index.json" ]]; then
  python3 - "$DIR/scaffold-index.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
print(f"scaffold-index: version={d.get('version')} classes={d.get('counts',{}).get('classes')} generated={d.get('generated')}")
PY
fi
