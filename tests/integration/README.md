# Integration Test Suite

Network-level probes against a **running** VisionClaw server: HTTP, WebSocket,
the line-delimited JSON-RPC TCP port, and the GPU.

The suites live in the Rust crate
[`crates/visionclaw-integration-tests`](../../crates/visionclaw-integration-tests).
They were `pytest` files in this directory until 2026-09-03; the port is recorded
in `docs/python-legacy-audit-2026-09-02.md` item #12. Nothing here links against
`visionclaw-server`, so the suite compiles in seconds.

The `.rs` files that remain in *this* directory are a different thing: modules of
the server crate's own in-process integration tests, not network probes.

## Running them

```bash
./run_tests.sh            # all probes
./run_tests.sh tcp        # one suite: tcp | security | polling | gpu
./run_tests.sh slow       # + the #[ignore]d probes (a 30s idle hold)
./run_tests.sh check      # endpoint reachability only, run nothing
```

Or straight through cargo, from the repo root:

```bash
VISIONCLAW_URL=http://localhost:9501 cargo test -p visionclaw-integration-tests
cargo test -p visionclaw-integration-tests --test security_probes
cargo test -p visionclaw-integration-tests -- --include-ignored
```

## The liveness gate

Every probe skips cleanly — passing, with a `SKIP:` line — when `VISIONCLAW_URL`
is unset, or is set but unreachable. `cargo test` at the workspace root therefore
never fails for want of a running server, and the suite is safe in CI.

| Variable | Default | Purpose |
|---|---|---|
| `VISIONCLAW_URL` | *(unset ⇒ skip all)* | HTTP base, e.g. `http://localhost:9501` |
| `VISIONCLAW_WS_URL` | `ws://<host>:3002` | WebSocket bridge |
| `VISIONCLAW_TCP_ADDR` | `<host>:9500` | line-delimited JSON-RPC port |

`run_tests.sh` defaults all three to the local ports, so it always attempts a
real run; plain `cargo test` skips unless you set `VISIONCLAW_URL` yourself.

## Suites

### `tcp_persistence` — 7 probes
Connection establishment (JSON-RPC `initialize`), persistence across ten pings,
reconnection after a clean close, five concurrent clients, a 1 MiB payload, a
slow operation inside the request timeout, and — behind `--ignored` — a 30-second
idle hold.

### `security_probes` — 13 probes
SQL / XSS / command-injection and path-traversal corpora, rate-limit
enforcement, authentication on protected endpoints, five auth-bypass attempts
(including an `alg: none` JWT), input validation, security headers, oversized
payload refusal, connection-flood throttling, secret exposure on debug
endpoints, and a CORS wildcard check.

### `polling_probes` — 8 probes
WebSocket ping bursts, reconnection, ten concurrent clients, delivery across
repeated drops, an unauthenticated `admin_command`, HTTP `/poll`, `/long-poll`
latency, and poll rate limiting.

### `gpu_stability` — 4 probes
CUDA device presence, name and memory via **NVML** (`nvml-wrapper`), stability of
memory reporting across repeated reads, five concurrent NVML readers, and the GPU
status field on `/health`. These skip when NVML cannot initialise, so CPU-only
hosts are fine.

The Python original reached CUDA through `docker exec mcp-gui-tools python -c
"import torch; torch.cuda.is_available()"`. NVML asks the driver directly, in
process — no container, no torch, no Python.

## Prerequisites

- TCP JSON-RPC on port 9500, WebSocket bridge on 3002, health endpoint on 9501
  (whichever are absent simply skip).
- A visible NVIDIA GPU for the `gpu_stability` suite.
- A Rust toolchain. There is no `requirements.txt` any more.

## Reports

`cargo test` is the runner and libtest is the reporter; the Python
`test_runner.py`, which existed to produce a markdown summary, has no successor
and needs none. For machine-readable output use cargo's own JSON:

```bash
cargo test -p visionclaw-integration-tests -- -Z unstable-options --format json
```

## Adding a probe

1. Put it in the suite file it belongs to, or add a new `tests/*.rs`.
2. Open with `let h = require_server!();` so it inherits the liveness gate.
3. Assert with a message that names the endpoint and what was expected.
4. Mark anything over a few seconds `#[ignore]` with a reason string.
5. Keep every file under 500 lines and update the counts above.

## CI

```yaml
- name: Run integration probes
  run: cargo test -p visionclaw-integration-tests
  env:
    VISIONCLAW_URL: http://localhost:9501
```

Without `VISIONCLAW_URL` the step still passes, reporting skips — useful as a
compile check on runners with no server.
