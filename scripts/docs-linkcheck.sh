#!/usr/bin/env bash
# docs-linkcheck.sh — grep-based relative-link verifier for the docs/ corpus.
#
# Scans every Markdown file under docs/ for inline links [text](target) and
# reference-style definitions [id]: target, then checks that each *relative*
# link resolves to a file or directory on disk.
#
# Skipped by design: absolute URLs (http/https/mailto/ftp/data), scheme URIs
# (urn:/did:), in-page anchors (#...). Absolute filesystem paths (/...) are
# reported as ABSOLUTE (docs should use relative links). A target with a
# trailing #anchor is checked against the file portion only.
#
# Exit status: 0 if every relative link resolves, 1 otherwise.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOCS="$ROOT/docs"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

checked=0
while IFS= read -r -d '' md; do
  dir="$(dirname "$md")"
  # Strip fenced code blocks (``` … ```): links inside them are illustrative
  # examples, not real links, and must not be checked.
  body="$(awk '/^[[:space:]]*```/{f=!f; next} !f' "$md")"
  while IFS= read -r target; do
    [ -z "$target" ] && continue
    target="${target#<}"; target="${target%>}"; target="${target%% *}"
    case "$target" in
      http://*|https://*|mailto:*|ftp://*|data:*|urn:*|did:*|\#*) continue ;;
      /*) echo "ABSOLUTE  ${md#$ROOT/} -> $target" >> "$tmp"; continue ;;
    esac
    path="${target%%#*}"
    [ -z "$path" ] && continue
    checked=$((checked+1))
    if [ ! -e "$dir/$path" ]; then
      echo "BROKEN    ${md#$ROOT/} -> $target" >> "$tmp"
    fi
  done < <(printf '%s\n' "$body" | grep -oE '\]\([^)]+\)|^\[[^]]+\]:[[:space:]]+[^[:space:]]+' 2>/dev/null \
            | sed -E 's/^\]\(//; s/\)$//; s/^\[[^]]+\]:[[:space:]]+//')
done < <(find "$DOCS" -name '*.md' -type f -print0)

if [ -s "$tmp" ]; then
  sort "$tmp"
  n=$(wc -l < "$tmp")
  echo "----"
  echo "docs-linkcheck: scanned $checked relative link target(s); $n unresolved."
  exit 1
fi
echo "----"
echo "docs-linkcheck: scanned $checked relative link target(s); all resolve. OK"
exit 0
