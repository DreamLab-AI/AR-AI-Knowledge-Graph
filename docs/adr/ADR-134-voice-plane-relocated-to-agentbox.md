# ADR-134: Voice meta-controller relocated from the VisionFlow tree into the agentbox submodule

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-08-04 |
| Deciders | Dr John O'Hare |
| Scope | Repository structure / cross-repo boundary |
| Related | agentbox ADR-042/043/044 (interaction plane, session identity, voice repoint), PRD-021 |

## Context

The local voice meta-controller ("Track A" voice loop) lived at `voice-stack/` in
this (VisionFlow) working tree: a console web surface (`voice-stack/console/`,
Caddy on `:8444`) plus the Kyutai Unmute fork (`voice-stack/unmute/`) and a compose
override (`voice-stack/unmute-override.yml`). It was **never tracked** in this repo
(`voice-stack/unmute/` is gitignored; `voice-stack/console/` + the override were
untracked working-dir files).

Investigation for the agentbox interaction-plane sprint established that this stack
is **agentbox's**, not VisionFlow's:

- Its LLM is agentbox's tab0-bridge (`http://agentbox:8971`); it is deployed and
  operated from inside the agentbox container.
- No VisionFlow application code (`src/`, `client/`, `xr-client/`) consumes it.
- VisionClaw's own voice (the kokoros visualiser path) is separate and untouched.

The two projects are deeply entwined in the VisionFlow ecosystem, so the code was
sitting in the wrong tree. It is also a significant, emerging surface that should
benefit from the interaction-plane work (session-aware, NIP-98-governed).

## Decision

**The agentbox-owned voice code is relocated into the agentbox submodule** at
`agentbox/voice/` (console + Caddy wiring + `unmute-override.yml` + a
`docker-compose.voice.yml` sidecar + an `agentbox.sh voice` subcommand + a `[voice]`
manifest gate), and re-imagined there as a first-class voice+TUI operator cockpit
(agentbox ADR-044). The relocation and rebuild are recorded in the agentbox
submodule's own history (branch `sprint/interaction-plane`, pending an external
security audit before it merges to agentbox `main`).

Consequences for **this** (VisionFlow) repository:

1. **No files are removed here** — the relocated originals were never tracked, so
   there is nothing to `git rm`. `voice-stack/console/RELOCATED.md` is left as an
   on-disk pointer. `voice-stack/unmute/` **remains** on disk: it is the upstream
   `kyutai-labs/unmute` clone that the agentbox voice sidecar references as an
   external build context (it is not vendored into agentbox), consistent with how
   `browsercontainer/` references its runtime.
2. **The canonical record of the move in this repo is the `agentbox` submodule
   pointer bump**, which is deferred until the agentbox work merges to agentbox
   `main` after the external audit. Bumping the pointer to a branch commit now is
   deliberately avoided.
3. This ADR is the durable note of the relocation for VisionFlow's own history.

## Alternatives considered

- **Leave it in `voice-stack/`.** Rejected: structurally impure (agentbox surface in
  the VisionFlow tree), and it would not benefit from the agentbox interaction-plane
  identity/session work.
- **Vendor the 26 GB Unmute fork into agentbox.** Rejected: it is upstream Kyutai
  code with its own git history; agentbox references it as a build context instead.
- **Bump the agentbox submodule pointer now.** Rejected: the agentbox work is on a
  branch pending audit; the pointer moves on merge to `main`.
