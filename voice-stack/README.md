# voice-stack — local voice meta-controller (Track A)

Fully local voice loop for the agentbox tmux plane:

```
laptop browser (tailnet, self-signed TLS)
  ├─ https://<host>:8444  voice-console — custom page: Unmute UI (iframe :8443)
  │                        + live tab-0 feed (WS) + type-into-tab-0 box
  └─ https://<host>:8443  Unmute origin (caddy → frontend:3000, /api → backend:80)
        ├─ Kyutai STT 1B  (semantic VAD, streaming)   ┐ RTX A6000 (CUDA dev 0)
        ├─ Kyutai TTS 1.6B (streaming)                ┘
        └─ "LLM" = tab0-bridge  http://agentbox:8971  (OpenAI-compatible)

tab0-bridge (inside agentbox container, tmux window "bridge", port 8971)
  ├─ brain: headless `claude -p` (Claude subscription OAuth, Haiku)
  ├─ tools: tmux send-keys (window 0 only) / capture-pane / list-windows
  └─ feed:  Stop + UserPromptSubmit hooks → turn summaries → WS /feed
```

The kokoros container serving the visionclaw visualiser is untouched.

## Deploy / operate (from INSIDE agentbox)

The docker CLI here drives the HOST daemon via the socket. Build contexts
stream client-side (safe); every runtime bind mount in `unmute-override.yml`
is an absolute HOST path under `/mnt/mldata/githubs/AR-AI-Knowledge-Graph`
(≡ container `~/workspace/project`) — keep it that way.

```bash
export DOCKER_CONFIG=~/workspace/.docker   # container $HOME is read-only
cd ~/workspace/project/voice-stack/unmute  # git clone of kyutai-labs/unmute (gitignored)
docker compose -f docker-compose.yml -f ../unmute-override.yml \
  up -d --build traefik frontend backend stt tts voice-console
```

Notes:
- First STT/TTS start downloads models into `unmute/volumes/hf-cache/`.
  Export `HUGGING_FACE_HUB_TOKEN` before `up` if downloads 401.
- Caddyfile changes need `docker restart unmute-voice-console-1`
  (bind-mounted config; compose won't recreate on content change).
- traefik v3.3.1 needs `DOCKER_API_VERSION=1.44` against this host daemon
  (set in the override). Our TLS origins bypass traefik entirely.
- TLS: self-signed pair in `console/certs/` (generated via
  `docker run --rm -v <host path>:/out alpine/openssl req -x509 ...`).

## Use (laptop, over tailnet)

1. Visit `https://<host>:8443`, accept the self-signed cert once.
2. Open `https://<host>:8444`, accept its cert. Left: voice (grant mic).
   Right: live tab-0 mirror with click-to-expand summaries; the input box
   types straight into tab 0.

Voice grammar the meta-controller understands: "tell tab zero to …" /
"ask the main agent …" (relays a written prompt into window 0), "what's
tab zero doing?" (summarised status), "what's the Codex tab doing?"
(capture-pane + summary), or plain questions (answered directly).

## tab0-bridge (inside agentbox)

- Code: `~/workspace/tab0-bridge/` (`server.mjs`, `turn-sink.cjs`,
  `start.sh`); runs in tmux window `agentbox:bridge`.
  Restart: `tmux kill-window -t agentbox:bridge;`
  `tmux new-window -d -t agentbox -n bridge '~/workspace/tab0-bridge/start.sh 2>&1 | tee -a ~/workspace/tab0-bridge/bridge.log'`
- Endpoints: `/v1/chat/completions` (streaming, what Unmute calls),
  `/hook/turn`, `/turns`, `/feed` (WS), `/tab0/send`, `/tabs[/n]`, `/health`.
- Hooks: `~/.claude/settings.json` Stop + UserPromptSubmit →
  `turn-sink.cjs`; forwards only sessions with cwd under
  `~/workspace/project` (filters the bridge's own headless sessions).
- Gotchas baked in: container's `ANTHROPIC_API_KEY` is set-but-empty and
  must be unset for child `claude` processes; `--allowedTools` is variadic
  so the prompt goes via stdin; tmux default shell is fish — wrap bashisms
  in `sh -c`.
- Sovereign upgrade path: replace `claudeTurn()` with any OpenAI-compatible
  local server (e.g. vLLM on the A6000) — the rest of the stack is agnostic.
