---
id: AB-06
title: Compose overlays, sidecar topology and the loopback-publish invariant
area: agentbox
governing:
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2013, ADR-2003, ADR-2040]
sources:
  - agentbox/docker-compose.yml
  - agentbox/docker-compose.override.yml
  - agentbox/docker-compose.browsercontainer.yml
  - agentbox/docker-compose.gui-tools.yml
  - agentbox/docker-compose.voice.yml
  - agentbox/docker-compose.xr-runtime.yml
  - agentbox/docker-compose.openmed.yml
  - agentbox/docker-compose.solid-pods.yml
  - agentbox/docker-compose.android.yml
  - agentbox/docker-compose.hp.yml
  - agentbox/scripts/ci/check-ports-loopback.mjs
  - agentbox/scripts/ci/check-ports-loopback.sh
  - agentbox/browsercontainer/server.js
  - agentbox/agentbox.sh
  - agentbox/flake.nix
  - agentbox/.github/workflows/invariants.yml
  - agentbox/scripts/ci/check-seccomp.sh
  - agentbox/voice/unmute-override.yml
  - agentbox/agentbox.toml
  - agentbox/mcp/servers/ruvector-mcp.cjs
  - agentbox/scripts/ci/check-db-password.sh
  - agentbox/scripts/ci/check-manifest-catalogue.js
  - agentbox/scripts/ci/check-nnp.sh
  - agentbox/scripts/ci/check-no-npx-latest.sh
  - agentbox/scripts/ci/check-secret-not-in-env.sh
  - agentbox/scripts/ci/check-single-metrics.js
verified_commit: 7a20db228
---

## AB-06.1 Compose overlay topology on visionclaw_network
```mermaid
flowchart TB
    subgraph BASE["docker-compose.yml — AUTO-GENERATED from agentbox.toml via flake.nix, do not edit by hand (:1-2)"]
        PG["ruvector-postgres<br/>image pinned by digest :11<br/>db ruvector, healthcheck pg_isready :20-25"]
        AB["agentbox<br/>image AGENTBOX_IMAGE_REF :31<br/>depends_on ruvector-postgres service_healthy :35-37<br/>healthcheck curl localhost:9090/ready :39-44"]
    end
    PG -->|"service_healthy gate"| AB
    AB ---|"9096:9096 LAN — the ONE identity-gated door"| LAN(("LAN"))
    AB ---|"127.0.0.1:9090 management-api"| LO(("host loopback"))
    AB ---|"127.0.0.1:9700"| LO
    AB ---|"127.0.0.1:9091 metrics"| LO
    AB ---|"127.0.0.1:8484 solid-pod"| LO
    AB ---|"127.0.0.1:8888 jupyter"| LO
    AB ---|"127.0.0.1:5901 vnc"| LO
    AB ---|"127.0.0.1:8080 code-server"| LO
    subgraph OVR["docker-compose.override.yml — operator layer, auto-loaded when present"]
        OV1["agentbox service overrides :9<br/>env_file :13, environment :21-71<br/>volumes :72-149, deploy/GPU :150-164<br/>group_add 965 docker socket gid :177-178"]
    end
    OVR -.->|"-f base -f override (agentbox.sh:564-567)"| BASE
    subgraph SIDE["sidecar overlays — own lifecycle, joined via visionclaw_network"]
        BC["browsercontainer<br/>5903 VNC, 8931 MCP SSE, 9222 to 9223 CDP"]
        GT["gui-tools-service<br/>5905 VNC, 9876 BlenderMCP, 9877 QGIS MCP"]
        VC["voice-console<br/>8443, 8444 Caddy origin"]
        XR["xr-runtime<br/>5904 VNC"]
        OM["openmed<br/>127.0.0.1:9093"]
        AND["android redroid<br/>127.0.0.1:5555 adb — profile android"]
        CF["cloudflared-pod<br/>tunnel, no published port"]
    end
    AB --- NET(("visionclaw_network"))
    BC --- NET
    GT --- NET
    VC --- NET
    XR --- NET
    OM --- NET
    AND --- NET
    CF --- NET
    HP["docker-compose.hp.yml — host overlay<br/>agentbox env_file/volumes/GPU reservations :6-37"] -.-> BASE
    Note1["DOC-DRIFT: docker-compose.yml:52 still says --auth none. The generator<br/>(flake.nix:2557-2559) is already correct - the committed artefact is stale<br/>and regenerates."]
    BASE -.-> Note1
```

## AB-06.2 ADR-2013 — the loopback-publish invariant and its CI gate
```mermaid
flowchart TD
    CI[".github/workflows/invariants.yml"] --> W["scripts/ci/check-ports-loopback.sh<br/>stable entry point, resolves the gate<br/>relative to itself and FAILS LOUDLY if missing (:9-13)"]
    W --> G["scripts/ci/check-ports-loopback.mjs<br/>real YAML reader"]
    G --> WALK["walk the WHOLE tree of every docker-compose*.yml<br/>at the repo root glob, not only services/*/ports (:594)"]
    WALK --> NORM["normalise every spelling to one tuple —<br/>long-form host_ip/published/target and<br/>short 0.0.0.0:8080:80 judge identically (:31-32)"]
    NORM --> J{"host_ip is 127.0.0.1?<br/>LOOPBACK const :89"}
    J -->|yes| PASS["pass"]
    J -->|no| S{"on the SANCTIONED list?<br/>check-ports-loopback.mjs:93-104 matched on<br/>file + host_ip + published + target + protocol (:611)"}
    S -->|yes| PASS
    S -->|no| FAIL["violation — publishes X, not loopback and not sanctioned (:638)"]
    S --> L1["docker-compose.yml 9096 to 9096 host_ip null — :77"]
    S --> L2["docker-compose.voice.yml 8443, 8444 on 0.0.0.0 — :78-79"]
    S --> L3["docker-compose.browsercontainer.yml 5903, 8931, 9222 to 9223 on 0.0.0.0 — :80-82"]
    S --> L4["docker-compose.gui-tools.yml 5905, 9876, 9877 on 0.0.0.0 — :83-85"]
    S --> L5["docker-compose.xr-runtime.yml 5904 on 0.0.0.0 — :86"]
    G --> ALSO["also fails on env interpolation in a port value,<br/>and treats an unsanctioned IPv6 [::] bind as a public door (:36 and :533)"]
    G -.-> LIM["DIVERGENCE — the gate's own stated limit (:44-47): it does NOT resolve<br/>overlay order or --env-file interpolation, so a pass proves the compose files<br/>DECLARE no unsanctioned door, not that no unsanctioned door is OPEN"]
    NORM -.-> HIST["ADR-2013 replaced an awk line-walker after the estate review reproduced a<br/>structural bypass — a public port written as a nested service-flow or JSON-flow<br/>mapping passed a gate that only armed on a line beginning with ports: (:4-7 of the .sh)"]
```

## AB-06.3 The gui-tools-exchange volume — asymmetric mount pair
```mermaid
flowchart LR
    V[("named volume<br/>gui-tools-exchange")] -->|"mounted at /home/devuser/gui-tools<br/>docker-compose.override.yml:134"| AB["agentbox container"]
    V -->|"mounted at /home/devuser/exchange<br/>docker-compose.browsercontainer.yml:61"| BC["browsercontainer"]
    V -->|"mounted at /home/devuser/exchange<br/>docker-compose.gui-tools.yml:55"| GT["gui-tools-service"]
    AB -->|"write file to ~/gui-tools/x.svg"| V
    V -->|"read as file:///home/devuser/exchange/x.svg"| BC
    BC -->|"screenshot or render result back into the volume"| V
    V -->|"agent reads ~/gui-tools/result"| AB
    AB -.-> NOTE["INVARIANT — the SAME volume has DIFFERENT mount paths per container.<br/>An agent writing ~/gui-tools/foo.svg must address it as<br/>file:///home/devuser/exchange/foo.svg from the browser sidecar"]
    V -.-> DECL["declared in all three overlays —<br/>override.yml:199, browsercontainer.yml:69, gui-tools.yml:66"]
```

## AB-06.4 browsercontainer — HTTP surface and MCP transport
```mermaid
sequenceDiagram
    autonumber
    participant A as agent in agentbox
    participant S as browsercontainer/server.js<br/>request router :202-280
    participant CH as headless Chrome
    participant V as gui-tools-exchange volume

    alt OPTIONS preflight (server.js:202)
        A->>S: OPTIONS any path
        S-->>A: CORS headers
    end
    alt GET /health (server.js:208)
        A->>S: GET :8931/health
        S-->>A: status ok, transport sse, sessions N, chrome true, cdp 127.0.0.1:9222
    end
    alt POST /render-mermaid (server.js:238)
        A->>V: write mermaid source
        A->>S: POST :8931/render-mermaid
        S->>CH: render
        CH-->>S: SVG or PNG
        S->>V: write result
        S-->>A: rendered artefact
    end
    alt GET /sse (server.js:257)
        A->>S: GET :8931/sse — MCP SSE stream opens
        S-->>A: event stream (registered as the browser-gpu MCP server)
        A->>S: POST /messages (server.js:280) — JSON-RPC tool calls
        S->>CH: drive via CDP
        CH-->>S: result
        S-->>A: tool result
    end
    Note over S,CH: raw CDP is also reachable — published 9222 on the host mapping to container 9223 (docker-compose.browsercontainer.yml:53, SANCTIONED at check-ports-loopback.mjs:99)
    Note over S: VNC :5903 for eyes-on debugging (browsercontainer.yml:48)
    Note over A,S: GPU reservation and NVIDIA device request in the deploy block (browsercontainer.yml:33-45)
    Note over A,V: extra_hosts host.docker.internal maps to host-gateway (browsercontainer.yml:57-58)
```

## AB-06.5 gui-tools-service — the FHS GPU presentation sidecar
```mermaid
sequenceDiagram
    autonumber
    participant OP as operator
    participant SH as cmd_gui_tools<br/>agentbox.sh:1778
    participant DC as docker compose<br/>GUI_TOOLS_COMPOSE_ARGS agentbox.sh:572-573
    participant GT as gui-tools-service
    participant HC as /opt/gui-tools/healthcheck.sh

    OP->>SH: ./agentbox.sh gui-tools up
    SH->>DC: docker compose --project-name agentbox -f docker-compose.gui-tools.yml up -d --build (agentbox.sh:1784)
    DC->>GT: start with DISPLAY=:2, NVIDIA_DRIVER_CAPABILITIES compute,utility,graphics (gui-tools.yml:18-22)
    Note over GT: __GLX_VENDOR_LIBRARY_NAME=nvidia (gui-tools.yml:25) — the presentation path the Nix wrappers cannot provide, see AB-01.6
    GT->>GT: BlenderMCP binds 0.0.0.0:9876, QGIS MCP binds 0.0.0.0:9877 (gui-tools.yml:26-29)
    loop poll until deadline now plus 120 s, sleep 3 (agentbox.sh:1786-1790)
        SH->>HC: docker exec gui-tools-service bash /opt/gui-tools/healthcheck.sh
        alt healthy
            HC-->>SH: exit 0 — break
        else not yet
            HC-->>SH: non-zero
        end
    end
    alt deadline passed with ready 0
        SH-->>OP: Health check timed out then exit 1 (agentbox.sh:1791-1794)
    else
        SH-->>OP: BlenderMCP gui-tools-service:9876, QGIS gui-tools-service:9877, VNC vnc://localhost:5905 (agentbox.sh:1796-1798)
    end
    Note over SH,DC: sibling subcommands down :1795, logs :1799, status :1800 all reuse GUI_TOOLS_COMPOSE_ARGS
    Note over GT: everything runs under vglrun — interactive GL/Vulkan goes here, NOT through the wrapped Nix bins (BASELINE GPU wrappers limitation)
```

## AB-06.6 Per-sidecar compose argument sets and lifecycle entry points
```mermaid
flowchart LR
    SD["SCRIPT_DIR"] --> A1["COMPOSE_FILE docker-compose.yml — agentbox.sh:562"]
    SD --> A2["OVERRIDE_FILE docker-compose.override.yml — agentbox.sh:561"]
    A1 --> CA{"override file present?<br/>agentbox.sh:564"}
    A2 --> CA
    CA -->|yes| CA1["COMPOSE_ARGS = --project-name agentbox -f base -f override — :564"]
    CA -->|no| CA2["COMPOSE_ARGS = --project-name agentbox -f base — :566"]
    SD --> S1["SIDECAR_FILE browsercontainer — :562<br/>SIDECAR_COMPOSE_ARGS :568<br/>cmd_browsercontainer agentbox.sh:1304"]
    SD --> S2["XR_RUNTIME_FILE :569<br/>XR_RUNTIME_COMPOSE_ARGS :570<br/>cmd_xr_runtime agentbox.sh:1417"]
    SD --> S3["GUI_TOOLS_FILE :571<br/>GUI_TOOLS_COMPOSE_ARGS :572<br/>cmd_gui_tools agentbox.sh:1778"]
    SD --> S4["OPENMED_FILE :573<br/>OPENMED_COMPOSE_ARGS :574<br/>cmd_openmed agentbox.sh:1843"]
    SD --> S5["VOICE_FILE :580 plus voice/unmute-override.yml :581<br/>VOICE_COMPOSE_ARGS --project-name agentbox-voice :1917<br/>cmd_voice agentbox.sh:1929"]
    SD --> S6["ANDROID_FILE :594<br/>ANDROID_COMPOSE_ARGS adds --profile android :595<br/>cmd_android agentbox.sh:1199"]
    S5 --> VH["VOICE_HOST_ROOT default /mnt/mldata/githubs/AR-AI-Knowledge-Graph — :589<br/>compose bind SOURCES resolve on the HOST docker daemon,<br/>so they must be host paths (:584-588)"]
    S6 --> AG["EXPERIMENTAL and GATED OFF — additionally requires<br/>AGENTBOX_ENABLE_ANDROID=1 (:591-593)"]
    CA1 --> MGMT["MGMT_PORT 9090 — agentbox.sh:601"]
    S5 -.-> VNOTE["voice-console uses its OWN project name agentbox-voice,<br/>so it is a separate compose project from every other sidecar"]
```

## AB-06.7 Sidecar surface census with published bindings
```mermaid
flowchart TB
    subgraph LANP["LAN-reachable — every one on the ADR-2013 SANCTIONED list"]
        P1["agentbox 9096:9096 — NIP-98 sovereign ingress<br/>docker-compose.yml:54"]
        P2["voice-console 0.0.0.0:8443 and 0.0.0.0:8444 Caddy origin<br/>docker-compose.voice.yml:39-40"]
        P3["browsercontainer 0.0.0.0:5903 VNC, 0.0.0.0:8931 MCP SSE,<br/>0.0.0.0:9222 to 9223 CDP — browsercontainer.yml:48-53"]
        P4["gui-tools-service 0.0.0.0:5905 VNC, 0.0.0.0:9876 Blender,<br/>0.0.0.0:9877 QGIS — gui-tools.yml:45-49"]
        P5["xr-runtime 0.0.0.0:5904 VNC — xr-runtime.yml:64"]
    end
    subgraph LOOP["host-loopback only"]
        Q1["agentbox 9090 mgmt, 9700, 9091 metrics, 8484 pod,<br/>8888 jupyter, 5901 vnc, 8080 code-server<br/>docker-compose.yml:55-61"]
        Q2["openmed 127.0.0.1:9093 — openmed.yml:28"]
        Q3["android 127.0.0.1:5555 adb — android.yml:40"]
    end
    subgraph NONE["no published port"]
        R1["ruvector-postgres — network-internal only, docker-compose.yml:10-28"]
        R2["cloudflared-pod — outbound tunnel only, solid-pods.yml:25-33"]
    end
    Q3 -.-> AND["android comment: this is an authenticated Google session,<br/>never expose it on 0.0.0.0, prefer docker exec (android.yml:38-39)"]
    Q2 -.-> OM["openmed refuses to serve until the operator sets<br/>OPENMED_LICENSE_ACKNOWLEDGED, _ONNX_RUNTIME_PRESENT and<br/>_GOVERNANCE_ACKNOWLEDGED — all default false (openmed.yml:17-21)"]
    Q1 -.-> CS["DIVERGENCE — the HOST publish for code-server is loopback, but the CONTAINER bind is not:<br/>[program:code-server] runs code-server --bind-addr 0.0.0.0:8080 --auth none, so it is reachable unauthenticated<br/>from any sibling container on visionclaw_network. BASELINE flags this and cites flake.nix:1927, which is stale"]
    Q1 -.-> CSR["RESOLVED ADR-2040 (implementation_status: partial): code-server<br/>([program:code-server]) now runs --auth password, credential minted at boot<br/>into /home/devuser/.local/share/code-server/config.yaml (0600).<br/>jupyter-lab's empty --IdentityProvider.token= ([program:jupyter-lab])<br/>was DELETED in favour of a minted JUPYTER_TOKEN. Listener-side<br/>CI gate is still open work."]
    R1 -.-> PGN["ADR-015 — mandatory memory sidecar, health-gated;<br/>ruvector-mcp.cjs fails closed with no sql.js fallback"]
```

## AB-06.8 Container hardening posture declared in the base compose
```mermaid
flowchart TD
    AB["agentbox service<br/>docker-compose.yml:30"] --> CD["cap_drop :102"]
    AB --> CA2["cap_add :104"]
    AB --> TM["tmpfs :114"]
    AB --> SO["security_opt :140"]
    AB --> VOL["volumes :143"]
    AB --> NET["networks :160"]
    SO --> SCC["seccomp and no-new-privileges declarations<br/>gated by scripts/ci/check-seccomp.sh and check-nnp.sh"]
    VOL --> NV["named volumes declared :164-169 —<br/>ruvector-pg-data, ruvector-data, solid-data"]
    AB --> HC["healthcheck curl -f http://localhost:9090/ready<br/>interval 30s, timeout 10s, retries 5, start_period 60s — :39-44"]
    HC --> RDY["so compose readiness rides the SAME /ready contract<br/>the management API publishes — see AB-02"]
    AB --> DEP["depends_on ruvector-postgres condition service_healthy :35-37"]
    DEP --> ORD["memory sidecar must pass pg_isready before agentbox starts,<br/>which is what lets the memory adapter boot probe expect a live store — see AB-04.6"]
    AB -.-> GEN["INVARIANT — docker-compose.yml is AUTO-GENERATED from agentbox.toml<br/>via flake.nix (:1-2). Editing it by hand is overwritten by nix build .#compose"]
    CD -.-> SIB["sibling CI invariants in scripts/ci/ — check-db-password.sh,<br/>check-secret-not-in-env.sh, check-nnp.sh, check-seccomp.sh,<br/>check-no-npx-latest.sh, check-manifest-catalogue.js, check-single-metrics.js"]
```
