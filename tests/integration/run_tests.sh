#!/usr/bin/env bash
# Integration test runner.
#
# The suites that used to live here as pytest files are now the Rust crate
# `crates/visionclaw-integration-tests` (HTTP/WS/TCP probes + NVML GPU checks).
# `cargo test` is the runner, so this script only resolves endpoints, reports
# what is reachable, and hands over.
#
#   ./run_tests.sh            # all probes
#   ./run_tests.sh tcp        # one suite: tcp | security | polling | gpu
#   ./run_tests.sh slow       # + the #[ignore]d probes (30s idle hold)
#   ./run_tests.sh check      # endpoint reachability only, run nothing
#
# Every probe SKIPS CLEANLY (and passes) when VISIONCLAW_URL is unset or the
# server is unreachable, so this script is safe to run anywhere.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CRATE=visionclaw-integration-tests

: "${VISIONCLAW_URL:=http://localhost:9501}"
: "${VISIONCLAW_WS_URL:=ws://localhost:3002}"
: "${VISIONCLAW_TCP_ADDR:=localhost:9500}"
export VISIONCLAW_URL VISIONCLAW_WS_URL VISIONCLAW_TCP_ADDR

RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; YELLOW=$'\033[1;33m'; BLUE=$'\033[0;34m'; NC=$'\033[0m'
info()  { echo "${GREEN}[INFO]${NC} $*"; }
warn()  { echo "${YELLOW}[WARN]${NC} $*"; }
error() { echo "${RED}[ERROR]${NC} $*" >&2; }

check_services() {
    info "Checking endpoint availability..."

    local tcp_host="${VISIONCLAW_TCP_ADDR%:*}" tcp_port="${VISIONCLAW_TCP_ADDR##*:}"
    if nc -z "$tcp_host" "$tcp_port" 2>/dev/null; then
        info "TCP JSON-RPC ($VISIONCLAW_TCP_ADDR) - available"
    else
        warn "TCP JSON-RPC ($VISIONCLAW_TCP_ADDR) - unreachable, those probes will skip"
    fi

    local ws_hostport="${VISIONCLAW_WS_URL#*://}"
    if nc -z "${ws_hostport%:*}" "${ws_hostport##*:}" 2>/dev/null; then
        info "WebSocket bridge ($VISIONCLAW_WS_URL) - available"
    else
        warn "WebSocket bridge ($VISIONCLAW_WS_URL) - unreachable, those probes will skip"
    fi

    if curl -fsS "$VISIONCLAW_URL/health" >/dev/null 2>&1; then
        info "Health endpoint ($VISIONCLAW_URL/health) - available"
    else
        warn "Health endpoint ($VISIONCLAW_URL/health) - unreachable, ALL HTTP probes will skip"
    fi

    if command -v nvidia-smi >/dev/null 2>&1 && nvidia-smi -L >/dev/null 2>&1; then
        info "GPU - visible to NVML"
    else
        warn "GPU - not visible, those probes will skip"
    fi
}

run() {
    local label="$1"; shift
    echo "${BLUE}Running: $label${NC}"
    echo "----------------------------------------"
    ( cd "$PROJECT_ROOT" && cargo test -p "$CRATE" "$@" )
}

main() {
    local command="${1:-all}"
    case "$command" in
        check)    check_services ;;
        tcp)      run "TCP persistence"   --test tcp_persistence ;;
        security) run "Security probes"   --test security_probes ;;
        polling)  run "Client polling"    --test polling_probes ;;
        gpu)      run "GPU stability"     --test gpu_stability ;;
        slow)     check_services; echo; run "All probes (incl. slow)" -- --include-ignored ;;
        all)      check_services; echo; run "All probes" ;;
        help|-h|--help)
            sed -n '2,16p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            ;;
        *)
            error "Unknown command: $command"
            echo "Use '$0 help' for usage information."
            exit 1
            ;;
    esac
}

main "$@"
