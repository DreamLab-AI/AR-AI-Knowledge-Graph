#!/usr/bin/env bash
# ADR-2003 — inventory of every output path that emits node/edge/label data,
# and whether it applies the owner-pubkey visibility filter.
#
# Usage: bash docs/estate-closeout/2026-09-05/visibility-output-paths.sh
# Emits JSON on stdout. Run from the repository root.
set -euo pipefail

filtered() { grep -qE 'compute_private_opaque_ids|pubkey_visibility_filter_enabled|filtered_node_ids|is_visible' "$1" && echo true || echo false; }

printf '{\n  "adr": "ADR-2003",\n  "generated_utc": "%s",\n  "paths": [\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
first=1
for f in \
  src/handlers/socket_flow_handler/types.rs \
  src/handlers/socket_flow_handler/position_updates.rs \
  src/handlers/api_handler/graph/mod.rs \
  src/handlers/graph_export_handler.rs \
  src/handlers/graph_state_handler.rs \
  src/handlers/bots_visualization_handler.rs \
  src/handlers/api_handler/analytics/clustering_handlers.rs \
  src/handlers/api_handler/analytics/community.rs
do
  [ -f "$f" ] || continue
  [ $first -eq 1 ] || printf ',\n'
  first=0
  printf '    {"path": "%s", "visibility_filtered": %s}' "$f" "$(filtered "$f")"
done
printf '\n  ]\n}\n'
