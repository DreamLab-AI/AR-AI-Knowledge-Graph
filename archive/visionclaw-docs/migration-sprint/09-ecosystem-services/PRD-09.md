# PRD-09 — Ecosystem Services & Launch

Status      : Proposed
Date        : 2026-05-16
Owner       : Infrastructure / DevEx
Risk class  : low (infra, no application semantics)
Related     : ADR-09 (this section), ADR-10 (External Integrations),
              ADR-11 (Persistence — removes Neo4j container)

## Capability

Provide a single, environment-aware launch surface for VisionClaw that can:

1. Bring the VisionClaw container up in `dev` or `prod` mode with the right
   build flavour, network attachment, and source-mount semantics.
2. Bring **ecosystem services** (Kokoro TTS, Whisper WebUI, Xinference) up
   alongside VisionClaw, on a shared Docker network, as **independent
   containers** managed by reference — not vendored into the VisionClaw
   image.
3. Detect and adapt to the host environment: GPU presence, CUDA compute
   capability, Docker-in-Docker (DinD) bind-mount paths, and CachyOS-style
   `/opt/cuda` layout.
4. Avoid re-downloading 2 GB of CUDA + pacman state on every build by
   distinguishing source changes from image-level changes.
5. Avoid stale container state by surgically busting only the cargo target
   cache (not registry / git caches) when source changes.

## Why this matters

The freeze regression that triggered the migration sprint cost days because
the build / launch pipeline was unpredictable: source edits silently failed
to land in the running container (DinD bind-mount issue), full rebuilds
took 15+ minutes for what should have been a 2-minute incremental, and the
ecosystem services (TTS, STT, embeddings) drifted between `visionclaw_network`
and `visionclaw_network` without a single source of truth. A reliable
launch surface is a prerequisite for fast iteration on every other section
in this sprint.

## Scope (in)

- `scripts/launch.sh` — single entry-point script. Commands: `up`, `down`,
  `build`, `rebuild`, `restart`, `logs`, `shell`, `status`, `clean`.
- Ecosystem subcommands: `ecosystem`, `ecosystem-down`, `ecosystem-status`,
  and the `--with-ecosystem` flag for combined startup.
- `scripts/rust-backend-wrapper.sh` — dev-mode supervisord-managed entry
  for the Rust backend, with content-hash-based incremental rebuild.
- `Dockerfile.unified` — multi-stage CachyOS-based build (base → rust-deps
  → rust-builder → node-deps → node-builder → development | production).
- `docker-compose.unified.yml` — the canonical compose file referenced by
  `launch.sh`. All other compose files are deprecated for new work.
- Docker network: a single, configurable external network shared between
  VisionClaw, agentbox (Section 10), and ecosystem services.
- `supervisord.dev.conf` and `supervisord.production.conf` — process
  supervision inside the VisionClaw container.
- Tmux tab assignments documenting where commands are *issued from*
  (host build shell vs. agent container) — load-bearing for the DinD
  bind-mount constraint.

## Scope (out)

- agentbox internals — see Section 10. Agentbox is integrated *with* via
  the same network; its container lifecycle is its own concern.
- Application code or behaviour — every other section in this sprint.
- `multi-agent-docker/skills/` — separate workstream.
- Cloudflared tunnel internals — surfaced by `launch.sh` but specified
  in Section 6 (Auth & Security).

## User stories

### US-09-01 — Fast iteration on Rust source

> As a developer editing `src/`, when I run `./scripts/launch.sh up dev` after
> changing one `.rs` file, the backend rebuilds in ~2 minutes (not ~15) and
> the new binary runs in the existing container.

Acceptance:
- No image rebuild triggered (source is volume-mounted in dev).
- `rust-backend-wrapper.sh` detects the source change via content hash,
  surgically clears only the `webxr` crate's incremental artefacts, and
  re-runs `cargo build --release --features gpu`.
- Cargo registry / git caches are preserved across runs.

### US-09-02 — Clean rebuild when Dockerfile changes

> When `Dockerfile.unified`, `Cargo.lock`, `client/package.json`, or any
> entrypoint script changes, `up dev` automatically rebuilds the image
> without me having to remember `rebuild`.

Acceptance:
- `needs_image_rebuild()` compares mtimes of a known set of image-affecting
  files against the existing image's creation time and triggers a build
  if any are newer.
- The set of image-affecting files is explicit and documented in the
  script.

### US-09-03 — Nuclear rebuild path

> When everything is wedged (incremental cache corruption, stale PTX,
> mystery linker error), I have one command that wipes all cargo volumes
> and rebuilds from scratch.

Acceptance:
- `./scripts/launch.sh rebuild dev` runs with `--no-cache`, busts
  `CACHE_BUST`, and removes `visionclaw-cargo-target-cache`,
  `visionclaw-cargo-cache`, and `visionclaw-cargo-git-cache`.
- Takes ~15 minutes worst case; this is the documented upper bound.

### US-09-04 — Ecosystem services as one command

> When I want TTS + STT + Xinference embeddings alongside VisionClaw, one
> command brings them all up on the same network with predictable
> hostnames.

Acceptance:
- `./scripts/launch.sh ecosystem` starts Kokoro TTS (`kokoro-tts:8880`),
  Whisper WebUI (`whisper-webui-backend:8000`), and Xinference
  (`xinference:9997`) on the configured ecosystem network.
- Each service is idempotent: re-running `ecosystem` when services are
  already up is a no-op (not a restart).
- A stopped container that is restarted is automatically reconnected to
  the ecosystem network (per fix in `28c3521bb`).

### US-09-05 — Health visibility

> When something doesn't work, I run one command to see who's up, who's
> stopped, who's missing, and what network they're attached to.

Acceptance:
- `./scripts/launch.sh ecosystem-status` prints a three-column table
  (service, status, endpoint) plus the network's container members.
- `./scripts/launch.sh status` does the same for VisionClaw.

### US-09-06 — DinD build safety

> When I'm SSH'd into the agent container, I cannot accidentally bake the
> wrong source into the VisionClaw image.

Acceptance:
- Documented constraint that builds must be sent to the **host tmux tab 6**
  via `tmux send-keys`, not run from inside the agent container. This is
  enforced operationally; the script does not need to detect this, but
  the failure mode (silently baked image instead of bind mount) is
  documented prominently in the README of this section.

### US-09-07 — Portable CUDA

> When the build runs on a machine with a different GPU than the deploy
> target, the resulting binary still runs on the target GPU.

Acceptance:
- `build.rs` defaults to `sm_75` in Docker builds (portable PTX baseline)
  regardless of the build host's GPU.
- `rust-backend-wrapper.sh` at runtime auto-detects the runtime GPU's
  compute capability via `nvidia-smi` and overrides any stale `.env`
  `CUDA_ARCH` value.
- PTX ISA version is downgraded to 9.0 post-compilation in `build.rs`
  to match host driver capability. (See ADR-09 D6.)

## Non-goals

- Replacing Docker Compose with a different orchestrator (k8s, Nomad).
- Building VisionClaw into a single binary that runs the ecosystem
  services in-process. The whole point of "ecosystem" is independent
  lifecycle.
- Hot-swapping the ecosystem network on an already-running stack.
  Network change is a stop / re-up event.

## Acceptance criteria (rolled-up)

1. `./scripts/launch.sh up dev` is the canonical dev-mode start command.
2. `./scripts/launch.sh up prod` is the canonical prod-mode start command.
3. The two paths share `Dockerfile.unified` and `docker-compose.unified.yml`
   but select different stages (`development` vs `production`) via
   `BUILD_TARGET` and different compose profiles.
4. The ecosystem network is named via `${EXTERNAL_NETWORK}` (default
   documented in ADR-09 D2) and is the single network for all ecosystem
   services and VisionClaw.
5. After this sprint, no compose file, script, or .env contains the
   string `visionclaw_network`. The migration commit `d2f77703c` is
   considered the start of this work; this PRD/ADR are the finish.
6. `rust-backend-wrapper.sh` re-uses cargo registry / git caches across
   runs and only wipes the target cache when source has changed.
7. The CUDA path `/opt/cuda` is the single source of truth in build and
   runtime. `/usr/local/cuda` is a compatibility symlink only.
8. PTX ISA downgrade is part of `build.rs`, not a manual post-step or a
   wrapper-script kludge. (Promoted from ADR-PTX in
   `project_ptx_version_fix.md`.)

## Risks

| ID  | Risk                                                        | Mitigation |
|-----|-------------------------------------------------------------|------------|
| R1  | Ecosystem services drift back to `visionclaw_network`           | ADR-09 D2: configurable network name with single default, grep gate in CI |
| R2  | Devs run `launch.sh` from inside the agent container (DinD) | Documented in this PRD + visible warning in agent container README |
| R3  | `needs_image_rebuild()` misses a relevant file              | Explicit allow-list, reviewed when Dockerfile changes |
| R4  | Different GPUs between build and runtime produce non-portable PTX | ADR-09 D6: build defaults to sm_75 baseline + runtime arch detection |
| R5  | Stopped Kokoro container loses network on restart           | Fixed in `28c3521bb`, codified in ADR-09 D5 |
| R6  | `--with-ecosystem` flag accidentally consumed as ENVIRONMENT | Fixed in `28c3521bb` (`--*` glob), codified as a test case |

## Out-of-scope deferrals

- **Health probes for ecosystem services**: a follow-up could add
  `curl`-style health checks to `ecosystem-status` rather than just
  process presence. Deferred to Section 09 phase 2.
- **Auto-restart on host reboot**: ecosystem containers use
  `--restart unless-stopped` so the Docker daemon handles this. No
  systemd unit needed for VisionClaw.
- **Kokoro voice presets**: a property of the integration contract
  (Section 10), not infra.

## Glossary

- **Ecosystem service**: a containerised AI service that VisionClaw
  consumes over the network and that has its own upstream repo
  (Kokoro, Whisper, Xinference). Not vendored.
- **DinD**: Docker-in-Docker. A container with `/var/run/docker.sock`
  bind-mounted, able to run `docker` commands against the host daemon.
- **Bind mount**: a Docker volume that maps a host path into a
  container path. Under DinD, the host path is resolved relative to the
  *host*, not the calling container — which is why `launch.sh` from
  inside the agent container produces an image with baked source rather
  than live source.
- **Surgical cache bust**: clearing only the cargo target cache
  (build artefacts) while keeping the registry cache (dependency
  source) and the git cache (git deps).
- **Portable PTX**: PTX compiled at `sm_75` JIT-compiles to any GPU
  with compute capability ≥ 7.5, at the cost of one-time JIT latency.
