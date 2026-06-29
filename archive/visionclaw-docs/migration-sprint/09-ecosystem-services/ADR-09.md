# ADR-09 — Ecosystem Services & Launch

Status      : Proposed
Date        : 2026-05-16
Supersedes  : ad-hoc `launch.sh` patches accumulated 12 Apr → 15 May
Related     : PRD-09 (this section), ADR-10 (External Integrations),
              ADR-11 (Persistence — Neo4j container removed)

## Context

Between baseline `41979d33e` and `main@HEAD`, the launch surface grew
from "one script that runs docker compose" to a 1140-line bash script
covering DinD path detection, GPU detection, content-hash incremental
rebuild, multi-stage CachyOS image construction, supervisord lifecycle,
agentbox co-orchestration, ecosystem service management, network
migration, and cargo cache hygiene. Each commit fixed a real problem
(`b19df2d37`, `dc6b52e59`, `0e3d7431e`, `3126674fe`, `4e0ffaddf`,
`a0ad18d4f`, `eafd22002`, `73e2d8209`, `28c3521bb`, `d2f77703c`) but
the invariants are implicit.

This ADR documents those invariants and resolves the one inconsistency
the rollback exposes: the network migration from `visionclaw_network` to
`visionclaw_network` (commit `d2f77703c`) is **incomplete** on `main`.
Every `docker-compose*.yml` and parts of `launch.sh` /
`scripts/fix_kokoro_network.sh` still contain `visionclaw_network`
literals. This ADR finishes the migration.

## Decision

### D1. One launch script, one compose file

`scripts/launch.sh` is the sole supported entry-point. It dispatches
against `docker-compose.unified.yml`. The other compose files
(`docker-compose.dev.yml`, `docker-compose.production.yml`,
`docker-compose.unified-with-neo4j.yml`, `docker-compose.voice.yml`,
`docker-compose.yml`) are deprecated and either deleted in the
post-sprint cleanup or marked `# DEPRECATED, see docker-compose.unified.yml`.
`docker-compose.vircadia.yml` is retained as an opt-in overlay only
(per Section 12).

Justification: every "which compose file should I use?" question we
answered during the freeze regression cost real time. There is one
compose file. Profiles (`dev`, `prod`) and `BUILD_TARGET` select
between dev and prod within it.

### D2. Single configurable ecosystem network

```
ECOSYSTEM_NETWORK="${EXTERNAL_NETWORK:-visionclaw_network}"
```

This name appears in exactly two places:

1. `scripts/launch.sh` as the `ECOSYSTEM_NETWORK` variable.
2. `docker-compose.unified.yml` `networks:` section as the external
   network reference.

All `visionclaw_network` literals are removed from:

- `scripts/launch.sh` (lines 628–632, 746–749 currently still
  reference it directly — those must be changed to use
  `${EXTERNAL_NETWORK:-visionclaw_network}`).
- `docker-compose.unified.yml` (current ref:
  `name: ${EXTERNAL_NETWORK:-visionclaw_network}` → change default).
- `scripts/fix_kokoro_network.sh` — either deleted (Kokoro reconnect
  is now in `launch.sh`) or updated to use the env var.
- All other compose files: deleted under D1.

The default is `visionclaw_network`. The legacy name remains
overridable for an interop window (one release cycle) but is not the
default.

Anti-pattern rejected: hardcoding `visionclaw_network` anywhere. CI gate
greps the tree for it and fails the build.

### D3. Multi-stage Dockerfile.unified with promoted CUDA_ARCH

The Dockerfile has seven explicit stages plus two target stages:

1. `base` — CachyOS-v3 + pacman keyring + system deps + CUDA via
   pacman (lands at `/opt/cuda`) + Rust + Node 20.
2. `rust-deps` — copy `Cargo.toml`, `Cargo.lock`, `build.rs`,
   `whelk-rs`, and `src/utils/*.cu` only; create dummy `[[bin]]`
   stubs; `cargo fetch && cargo build --release --features gpu`.
   This layer caches the 300+ dep compile across source edits.
3. `rust-builder` — copy real `src/`; `touch` all `.rs` files to
   defeat the COPY-mtime-preservation gotcha; `cargo build` again.
4. `wasm-builder` (after `rust-deps`, before `node-builder`): runs
   `wasm-pack build --target web --release client/crates/scene-effects/`
   and outputs JS/wasm glue to `client/src/wasm/scene-effects/`. The
   `node-builder` stage `COPY --from=wasm-builder` consumes this path
   before `vite build` runs. The `node-builder`'s output therefore
   contains the compiled WASM as part of the static bundle. PRD-04 F7's
   versioning contract holds via the same `client/dist` artefact.
5. `node-deps` — `npm ci` for client.
6. `node-builder` — `npx vite build` (consumes `wasm-builder` output).
7. `development` — full toolchain, source mounted, runs
   `dev-entrypoint.sh` which starts supervisord.
8. `production` — minimal runtime, copies only the compiled `webxr`
   binary and built frontend `dist`, runs `prod-entrypoint.sh`,
   non-root user `appuser`.

`CUDA_ARCH` is declared as `ARG CUDA_ARCH=75` in `base` then
**promoted to `ENV`** in the same stage so child stages inherit it:

```dockerfile
ARG CUDA_ARCH=75
ENV CUDA_ARCH=${CUDA_ARCH}
```

This is load-bearing. Without the ENV promotion, child stages see
an empty `CUDA_ARCH` and `build.rs` falls back to its own default,
which may not match the operator's intent. The current Dockerfile
already does this correctly (line 38); this ADR codifies that it
must remain so.

### D4. CachyOS CUDA path is `/opt/cuda`

`CUDA_HOME`, `CUDA_PATH`, `CUDA_INCLUDE_PATH`, `CUDA_LIB_PATH`,
`LIBRARY_PATH`, `CPATH`, `CFLAGS`, `LDFLAGS` all point at
`/opt/cuda` and `/opt/cuda/lib64` / `/opt/cuda/include`. A
compatibility symlink `/usr/local/cuda -> /opt/cuda` is created in
the `base` stage so that crates which hardcode `/usr/local/cuda`
(notably `find_cuda_helper`) still resolve.

Justification: CachyOS / Arch installs CUDA via pacman to
`/opt/cuda`. Ubuntu / Debian ships it at `/usr/local/cuda`. The base
image is CachyOS, so `/opt/cuda` is canonical. Hardcoding
`/usr/local/cuda` anywhere new is a defect.

### D5. Ecosystem services managed as independent containers

Three ecosystem services are managed by `launch.sh`:

| Service     | Container             | Image                                                 | GPU      | Port |
|-------------|-----------------------|-------------------------------------------------------|----------|------|
| Kokoro TTS  | `kokoro-tts-container`| `ghcr.io/remsky/kokoro-fastapi-gpu:latest`            | device=2 | 8880 |
| Whisper     | `whisper-webui-backend` | `registry.gitlab.com/aadnk/whisper-webui:latest` (fallback; prefer the project's own compose if present) | device=1 | 8000 / 7860 |
| Xinference  | `xinference`          | `xprobe/xinference:latest`                            | all      | 9997 |

Each follows this lifecycle:

1. `start_<service>()`: if running, no-op (success). If stopped,
   `docker start` *and reconnect to the ecosystem network*
   (D5a). If not present, `docker run` with `--restart unless-stopped`.
2. `stop_ecosystem()`: stops *and removes* each container.
3. `show_ecosystem_status()`: tabulates state and network membership.

**D5a — Stopped-container network reconnect (per `28c3521bb`)**:
`docker start <name>` does **not** restore `--network` from the
original `docker run`. A stopped container that's restarted may
come back orphaned. `start_kokoro` (and equivalent for Whisper and
Xinference) must `docker network connect "$ECOSYSTEM_NETWORK" <name>`
after `docker start` if the container is not already on the
ecosystem network. This pattern is generalised across all three
services in the post-sprint implementation, not just Kokoro.

**D5b — GPU device assignment is configurable, not hardcoded**:
The defaults `device=2` (Kokoro), `device=1` (Whisper), `all`
(Xinference) match the current production layout but are
configurable via `KOKORO_GPU_DEVICE`, `WHISPER_GPU_DEVICE`,
`XINFERENCE_GPU_DEVICE` env vars. Single-GPU hosts default to
`device=0` for all three or omit the flag.

**D5c — Compose-file-first, docker-run-fallback**: where the
upstream service ships a `docker-compose.yaml` (Whisper, Xinference
are typical), `launch.sh` prefers it. Where no compose file exists
or the path is missing, it falls back to `docker run`. This is the
current behaviour in commits `73e2d8209` / `28c3521bb`; codify it.

**D5d — Consumers**: each ecosystem service maps to exactly one
VisionClaw consumer surface. The table below is exhaustive for the
sprint; adding a new ecosystem service requires adding a row here in
the same PR.

| Ecosystem service | VisionClaw consumer | WS endpoint | ADR-06 D4 handler |
|-------------------|---------------------|-------------|-------------------|
| Kokoro TTS (8880) | TTS dispatch in speech actor | `/ws/speech` (egress) | `speech_socket_handler` |
| Whisper STT (8000) | STT dispatch in speech actor | `/ws/speech` (ingress) | `speech_socket_handler` |
| Xinference (9997) | RAG embeddings + completion | n/a (HTTP) | `inference_handler`, `ragflow_handler` |

Out-of-scope for this sprint: any expansion of this consumer list.
Adding a new ecosystem service requires a row here in the same PR.

### D6. PTX ISA version downgrade is part of build.rs

`build.rs` performs the PTX ISA downgrade (CUDA 13.x emits
`.version 9.x`, host drivers may only JIT `.version 9.0`) as a
post-`nvcc` text substitution. Surfaced during the freeze
investigation as `project_ptx_version_fix.md` and historically
treated as a workaround. **This ADR promotes it to a documented
requirement.**

- `build.rs` compiles each `.cu` to PTX via `nvcc -arch=sm_<n>`
  (default `75` under `DOCKER_ENV`, overridable by `CUDA_ARCH`).
- Immediately after each PTX is produced, `build.rs` reads it,
  finds any `.version 9.N` where `N > 0`, and rewrites to
  `.version 9.0` in place.
- Putting this anywhere other than `build.rs` produces a binary
  that builds but does not run on the target driver — the
  single class of bug this fix prevents.
- If the toolchain ever ships an nvcc flag to cap the ISA version
  directly, replace the text substitution with the flag.

Anti-pattern rejected: PTX patching in `rust-backend-wrapper.sh`
or `dev-entrypoint.sh`.

### D7. Rust-backend wrapper: content-hash-based incremental rebuild

`scripts/rust-backend-wrapper.sh` is the supervisord-managed dev-mode
entry for the Rust backend. Responsibilities:

1. Auto-detect runtime GPU compute capability via `nvidia-smi` and
   override any stale `.env` `CUDA_ARCH`.
2. Content-hash every `.rs` and `.cu` file under `/app/src/` and
   compare against the cached hash from the previous successful
   build (`/app/target/.source-hash`). On match, skip rebuild and
   exec the cached binary.
3. On mismatch, surgically remove only the `webxr` crate's
   incremental artefacts under `/app/target/release/`. Do **not**
   `cargo clean` — that wipes the 300-dep compile. Then
   `cargo build --release --features gpu`.
4. If incremental fingerprints are >24h old, treat as corrupt and
   `cargo clean` (recovery path, not hot path).
5. Truncate stale `rust-error.log` on entry.

The content-hash approach is the pattern from `b19df2d37`,
`dc6b52e59`, `0e3d7431e`. The current script implements steps 1, 3
(partial — falls back to time-based stale-fingerprint check), 4, 5.
Step 2 (content hash) is the promotion this ADR mandates.

### D8. Image-rebuild vs source-change detection in launch.sh

`launch.sh` distinguishes two classes of change:

- **Image-affecting changes** — files where a change requires a
  Docker image rebuild. The allow-list is:
  `Dockerfile.unified`, `Dockerfile.production`, `Dockerfile.dev`,
  `Cargo.toml`, `Cargo.lock`, `client/package.json`,
  `client/package-lock.json`, `supervisord.dev.conf`,
  `nginx.dev.conf`, `nginx.production.conf`,
  `scripts/dev-entrypoint.sh`, `scripts/rust-backend-wrapper.sh`,
  `scripts/production-startup.sh`.
- **Source-only changes** — anything else under `src/`, `client/src/`,
  `build.rs`, `.cu` files. These do **not** require an image
  rebuild because source is volume-mounted in dev. They are picked
  up by `rust-backend-wrapper.sh`'s content hash inside the running
  container.

`needs_image_rebuild()` walks the allow-list, comparing mtimes to
the existing image's creation time. `needs_recompile()` walks the
source tree, comparing mtimes to the container start time.

In dev mode, `up` returns the existing container if both checks pass
and the container is healthy. Otherwise it restarts the container
(if source changed) or rebuilds the image (if Dockerfile / deps
changed). In prod mode, the source-only path doesn't apply because
the binary is baked into the image.

### D9. Three cache tiers

| Tier | Volume                              | Wiped by             | Contains |
|------|-------------------------------------|----------------------|----------|
| 1    | `visionclaw-cargo-cache`            | `rebuild` only       | `~/.cargo/registry` (dep source) |
| 2    | `visionclaw-cargo-git-cache`        | `rebuild` only       | `~/.cargo/git` (git deps) |
| 3    | `visionclaw-cargo-target-cache`     | `up` if source changed; `rebuild` always | `/app/target` (build artefacts) |

`clean_cargo_target()` removes only tier 3. `clean_cargo_volumes()`
removes all three. The distinction is critical: tier 1 represents a
2 GB cold-cache cost. Wiping it without cause turns a 2-minute
build into a 15-minute build.

### D10. Tmux tab convention — load-bearing for DinD

The agent container runs Claude Code in tmux tab 0 with the host
Docker socket bind-mounted. It can call `docker` but a bind mount
`${HOST_PROJECT_ROOT:-.}/src:/app/src:ro` declared in
`docker-compose.unified.yml` resolves against the **host** filesystem,
not the agent container. Running `launch.sh` from inside the agent
container produces a VisionClaw image with **baked-in source**
(silent bind-mount failure), not live source — edits appear to have
no effect.

**Operational rule** (already documented in
`/home/devuser/workspace/CLAUDE.md`): builds are sent to the host
shell on **tmux tab 6** via `tmux send-keys -t 6
'./scripts/launch.sh up dev' Enter`. Tab 6 sees the real project
root. Tab 0 never runs `launch.sh` directly.

`launch.sh`'s `detect_dind()` is a best-effort runtime guard that
inspects container mounts to resolve `HOST_PROJECT_ROOT`. The tab-6
convention is the primary defence.

### D11. Supervisord layout

`supervisord.dev.conf` runs three programs:

- `nginx` — reverse proxy in front of Vite + Rust API.
- `rust-backend` — wrapped by `rust-backend-wrapper.sh`
  (per D7); startretries=3, autorestart=true.
- `vite-dev` — `npm run dev` in `/app/client`.

`supervisord.production.conf` runs `nginx` and the pre-built
`webxr` binary; no Vite, no wrapper.

Production runs as non-root `appuser` (uid 1000). Dev runs as root
because pacman/cargo/incremental-rebuild paths assume root in
container builds. The privilege gap between dev and prod is
intentional and called out in Section 6 (Auth & Security).

### D12. Cloudflared tunnel: prod only

`launch.sh up dev` runs `docker compose up -d --scale cloudflared=0`,
explicitly suppressing the tunnel. `launch.sh up prod` brings it up.
This is the policy decision; tunnel internals (auth, route mapping)
are Section 6.

## Options considered

### O1. Bring each launch.sh patch forward individually

Rejected. The script is large enough that re-merging 30+ commits
without a designed structure produces the same emergent complexity
we just lived through.

### O2. Rewrite launch.sh in Rust / Python

Rejected (for this sprint). The script's logic is well-suited to
bash + docker CLI. A Rust rewrite is a non-trivial side-quest with
no payoff for the freeze-fix work. Defer indefinitely.

### O3. One script, explicit invariants, finished migration (this ADR)

Adopted. Codify the existing behaviour, finish the `visionclaw_network`
→ `visionclaw_network` migration, and promote the PTX fix from
workaround to documented requirement.

## Risks

| ID  | Risk                                                              | Mitigation |
|-----|-------------------------------------------------------------------|------------|
| R1  | A `visionclaw_network` literal slips back in via a new compose file   | CI grep gate (`! grep -r visionclaw_network .`); pre-commit hook |
| R2  | `needs_image_rebuild()` allow-list goes stale                     | List is reviewed when Dockerfile changes; covered by ADR-09 D8 |
| R3  | `rust-backend-wrapper.sh` content hash misses a file type         | Hash all `.rs` and `.cu` under `src/`; review list when a new file extension appears in `build.rs` |
| R4  | Ecosystem services drift to different network names per service   | Single `ECOSYSTEM_NETWORK` variable, used by all three start functions |
| R5  | DinD bind-mount confusion (build from inside agent container)     | Tab-6 convention documented in three places (this ADR, workspace CLAUDE.md, agent container README); `detect_dind()` as a runtime guard |
| R6  | PTX ISA downgrade silently misses a new ISA version (e.g. 10.x)   | Regex covers `.version 9.N`; extend when toolchain bumps |

## Rejected from main as buggy / unjustified

- Hardcoded `device=2` for Kokoro in single-GPU dev environments
  (current `start_kokoro` does this) — replace with configurable
  `KOKORO_GPU_DEVICE` per D5b.
- `scripts/fix_kokoro_network.sh` as a standalone script that uses
  the hardcoded legacy `visionclaw_network` name — the reconnect logic
  belongs inside `start_kokoro` (D5a) using the configurable network
  name (D2). The standalone script is deleted.
- `cargo clean` as the default recovery in `rust-backend-wrapper.sh`
  — kept as the >24h-stale-fingerprint recovery path only, not the
  hot path.
- Compose files other than `docker-compose.unified.yml` (and
  `docker-compose.vircadia.yml` as opt-in overlay).

## Bugs and smells at the reset point (41979d33e)

To flag for migration awareness:

- The rollback baseline predates the ecosystem subcommands
  (`73e2d8209`). Bringing this section forward is *additive* —
  no existing ecosystem code is being replaced.
- The rollback baseline uses `visionclaw_network` as the network name
  throughout. This ADR D2 is the new state.
- The rollback baseline's `rust-backend-wrapper.sh` does
  `cargo clean` more aggressively than D7 specifies. Tighten on
  migration.
- `Dockerfile.unified` at baseline does not yet promote
  `CUDA_ARCH` from `ARG` to `ENV` consistently across stages.
  D3 codifies that the current `main` behaviour (which does
  promote it) is the correct one.
- The PTX downgrade is present in `build.rs` on `main` but is
  documented only in a memory note. D6 promotes it to first-class.

## Implementation checklist (post-sprint)

1. Grep the tree for `visionclaw_network`; replace every occurrence
   with `${EXTERNAL_NETWORK:-visionclaw_network}` or delete the
   containing file under D1.
2. Add a CI gate: `! grep -rn visionclaw_network . --exclude-dir=.git`.
3. Delete `scripts/fix_kokoro_network.sh`; verify the in-script
   reconnect (D5a) covers its use case.
4. Add `KOKORO_GPU_DEVICE`, `WHISPER_GPU_DEVICE`,
   `XINFERENCE_GPU_DEVICE` env vars; document defaults in
   `.env.example`.
5. Promote `rust-backend-wrapper.sh` to content-hash incremental
   per D7; ensure the registry / git caches are preserved on
   `up dev`.
6. Verify the `CUDA_ARCH` ARG→ENV promotion in `Dockerfile.unified`
   is consistent across the `base`, `rust-deps`, and `rust-builder`
   stages. (Currently set in `base`, inherited automatically by
   child `FROM base`. Confirmed correct.)
7. Confirm `build.rs` PTX downgrade covers all `.version 9.N`
   where `N > 0`. Currently it does. Codify as a unit test.
8. Add tmux-tab-6 instruction to this section's README, the
   workspace CLAUDE.md (already present), and the agent container
   README (Section 10).
