# RELOCATED

The voice operator console (console + Caddyfile + compose override) moved into
the agentbox repo, where the code it wires actually lives (tab0-bridge, the AoE
plane, the NIP-98 proxy). This `voice-stack/` tree is untracked in the
VisionFlow host repo; nothing here is authoritative any more.

New home (agentbox, tracked):

    agentbox/voice/console/Caddyfile        →  operator origin routing (:8444/:8443)
    agentbox/voice/console/site/            →  the console page (index.html/app.js/styles.css)
    agentbox/voice/unmute-override.yml      →  Kyutai Unmute stack override (was voice-stack/unmute-override.yml)
    agentbox/docker-compose.voice.yml       →  the caddy console sidecar service
    agentbox.sh voice <up|down|logs|health|status|certs|rebuild|shell>

What stays here: the Kyutai `unmute/` clone (26 GB, its own .git) remains the
external build context, referenced by `agentbox/voice/unmute-override.yml` via
`VOICE_UNMUTE_DIR` (default `../voice-stack/unmute` relative to the agentbox
checkout). Documented follow-up (ADR-044): pin it to a build-arg commit /
published image the way browsercontainer pins Chrome.

TLS certs are no longer committed — `agentbox.sh voice up` generates a
self-signed pair into `agentbox/voice/console/certs/` (gitignored).

See `agentbox/voice/README.md`.
