#!/usr/bin/env bash
# d1-beam-check.sh — D1 embodiment-join liveness instrument (PRD-023 WP-2).
#
# D1's register framing (a missing connect(), resolveAgentPosition hardcoded
# false) was FALSIFIED — the wire already exists in current code. What remains
# unproven is whether live agent activity ever reaches the client data path. This
# script supplies that live proof, to be run against a RUNNING stack in the
# sprint-end live session.
#
# It observes the exact endpoint the desktop client polls
# (AgentNodesLayer.tsx:444 → GET /api/bots/agents) and, when that roster carries
# non-empty live agent data, POSTs the observation to
#   POST /api/canary/observe/CANARY-VC-D1-BEAM
# on the LivenessHarness (RES-a / P0). It is NOT a synthetic prober: it fires ONLY
# on a genuinely non-empty roster produced by a live agent. If no agent is active
# it times out and does NOT fire (DDD invariant 5 — a canary fires only on
# observed live traffic).
#
# Usage:
#   scripts/canary/d1-beam-check.sh [BASE_URL]
#
# Environment:
#   VISIONCLAW_BASE_URL   Base URL of the running stack (default http://localhost:4000)
#   VISIONCLAW_TOKEN      Bearer token, if the roster route requires auth (optional)
#   D1_MAX_ATTEMPTS       Poll attempts before timing out (default 30)
#   D1_INTERVAL_SECS      Seconds between polls (default 2)
#   D1_CANARY_ID          Canary id to observe (default CANARY-VC-D1-BEAM)
#
# Exit codes:
#   0  a non-empty roster was observed and the canary was fired
#   2  timed out with no live agent data (canary correctly NOT fired)
#   1  a hard error (stack unreachable, malformed response, observe rejected)

set -euo pipefail

BASE_URL="${1:-${VISIONCLAW_BASE_URL:-http://localhost:4000}}"
BASE_URL="${BASE_URL%/}"
TOKEN="${VISIONCLAW_TOKEN:-}"
MAX_ATTEMPTS="${D1_MAX_ATTEMPTS:-30}"
INTERVAL_SECS="${D1_INTERVAL_SECS:-2}"
CANARY_ID="${D1_CANARY_ID:-CANARY-VC-D1-BEAM}"

ROSTER_URL="${BASE_URL}/api/bots/agents"
OBSERVE_URL="${BASE_URL}/api/canary/observe/${CANARY_ID}"

log() { printf '[d1-beam-check] %s\n' "$*" >&2; }

auth_args=()
if [[ -n "${TOKEN}" ]]; then
  auth_args=(-H "Authorization: Bearer ${TOKEN}")
fi

# Extract the agent-roster count from the /api/bots/agents body. The route wraps
# its payload in the StandardResponse envelope, so the roster lives at
# `.data.count` (with `.data.agents[]` the roster itself). Tolerate both the
# wrapped and a bare shape. Prefer jq; fall back to python3.
roster_count() {
  local body="$1"
  if command -v jq >/dev/null 2>&1; then
    # `cnt` yields null (not 0) for a missing/non-array field so `//` falls
    # through — a bare `0` would short-circuit `//` (jq treats 0 as truthy).
    jq -r '
      def cnt(x): if (x | type) == "array" then (x | length) else null end;
      ( .data.count // .count // cnt(.data.agents) // cnt(.agents) // 0 )
      | tostring
    ' <<<"${body}" 2>/dev/null || echo "0"
  elif command -v python3 >/dev/null 2>&1; then
    python3 - "$body" <<'PY' 2>/dev/null || echo "0"
import json, sys
try:
    b = json.loads(sys.argv[1])
except Exception:
    print(0); raise SystemExit
d = b.get("data", b) if isinstance(b, dict) else {}
if not isinstance(d, dict):
    d = {}
count = d.get("count")
if count is None:
    agents = d.get("agents")
    count = len(agents) if isinstance(agents, list) else 0
print(int(count) if isinstance(count, int) else 0)
PY
  else
    log "neither jq nor python3 available for JSON parsing"
    echo "0"
  fi
}

fire_canary() {
  local count="$1"
  local evidence
  evidence="live agent roster observed: ${count} agent(s) via GET /api/bots/agents \
(the AgentNodesLayer client data path) — live agent activity reaching the client"
  local payload
  payload=$(printf '{"evidence":"%s"}' "${evidence//\"/\\\"}")

  log "firing ${CANARY_ID} → ${OBSERVE_URL}"
  local resp
  resp=$(curl -fsS -X POST "${OBSERVE_URL}" \
    -H "Content-Type: application/json" \
    "${auth_args[@]}" \
    -d "${payload}") || {
      log "observe POST failed (is the LivenessHarness up? is ${CANARY_ID} registered?)"
      return 1
    }
  log "observe response: ${resp}"
  return 0
}

log "polling ${ROSTER_URL} (max ${MAX_ATTEMPTS} × ${INTERVAL_SECS}s) for a non-empty roster"

attempt=0
while (( attempt < MAX_ATTEMPTS )); do
  attempt=$((attempt + 1))

  if ! body=$(curl -fsS "${ROSTER_URL}" "${auth_args[@]}" 2>/dev/null); then
    log "attempt ${attempt}/${MAX_ATTEMPTS}: roster unreachable at ${ROSTER_URL}"
    sleep "${INTERVAL_SECS}"
    continue
  fi

  count=$(roster_count "${body}")
  if ! [[ "${count}" =~ ^[0-9]+$ ]]; then
    count=0
  fi

  if (( count > 0 )); then
    log "attempt ${attempt}/${MAX_ATTEMPTS}: roster carries ${count} live agent(s) — observing"
    if fire_canary "${count}"; then
      log "D1 beam check PASSED — ${CANARY_ID} fired on ${count} live agent(s)"
      exit 0
    else
      exit 1
    fi
  fi

  log "attempt ${attempt}/${MAX_ATTEMPTS}: roster empty (no live agent yet)"
  sleep "${INTERVAL_SECS}"
done

log "TIMED OUT after ${MAX_ATTEMPTS} attempts — no live agent data reached the client data path"
log "${CANARY_ID} correctly NOT fired (no synthetic stand-in for live traffic)"
exit 2
