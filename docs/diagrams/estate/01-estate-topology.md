---
id: ES-01
title: Estate topology — substrates, network fabric, service ports, compose networks
area: estate
governing:
  - docs/BASELINE-architecture.md
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2023, ADR-2013, ADR-2027, ADR-2025, ADR-2009, ADR-2012, ADR-2062]
sources:
  - .gitmodules
  - Cargo.toml
  - docker-compose.unified.yml
  - docker-compose.cloudflared.yml
  - agentbox/docker-compose.yml
  - agentbox/docker-compose.override.yml
  - agentbox/docker-compose.browsercontainer.yml
  - agentbox/docker-compose.gui-tools.yml
  - agentbox/docker-compose.solid-pods.yml
  - agentbox/docker-compose.voice.yml
  - agentbox/docker-compose.xr-runtime.yml
  - agentbox/docker-compose.hp.yml
  - agentbox/docker-compose.android.yml
  - agentbox/docker-compose.openmed.yml
  - nginx.conf
  - nginx.dev.conf
  - nginx.production.conf
  - supervisord.dev.conf
  - supervisord.production.conf
  - agentbox/agentbox.toml
  - agentbox/CLAUDE.md
  - agentbox/skills/email-search/SKILL.md
  - agentbox/docs/developer/hp-peer-node.md
  - agentbox/docs/developer/ecosystem.md
  - agentbox/docs/developer/native-pod-mesh.md
  - loom/README.md
  - agentbox/docs/adr/ADR-2023-loom-facade.md
  - agentbox/docs/adr/ADR-2013-loopback-publish-except-9096.md
  - docs/adr/ADR-2027-three-deployment-profiles.md
  - docs/adr/ADR-2025-cross-from-agentbox-closed-map.md
  - agentbox/lib/solid-pod-rs.nix
verified_commit: 7a20db228
---
## ES-01.1 Substrate map — six repositories, five-participant identity mesh
```mermaid
flowchart TB
    subgraph ONDISK["On disk in this checkout"]
        VC["VisionClaw (this repo)<br/>root: Cargo.toml, docker-compose.unified.yml"]
        AB["agentbox<br/>git submodule at agentbox/<br/>.gitmodules: url=github.com/DreamLab-AI/agentbox.git"]
    end
    VC -->|"embeds as submodule<br/>path=agentbox"| AB
    AB -.->|"pinned commit 89301ec7<br/>branch archive/feature-high-perf-networking-829<br/>git submodule status: +89301ec7 (dirty vs index)"| ABPIN["submodule pin"]

    subgraph CRATE["Embedded as a Cargo dependency, not a checkout"]
        SPR["solid-pod-rs 0.4.0-alpha.15<br/>crates.io pin, Cargo.toml:219<br/>feature solid-pod-embed (ADR-032 M3)"]
    end
    VC -->|"Cargo dep: fs-backend, nip98-schnorr,<br/>did-nostr, quota, rate-limit"| SPR
    AB -->|"lib/solid-pod-rs.nix pin<br/>supervised [program:solid-pod] :8484<br/>agentbox/docs/developer/native-pod-mesh.md"| SPR

    subgraph EXTERNAL1["EXTERNAL: not checked out at repo root"]
        VF["EXTERNAL: VisionFlow<br/>Umbrella coordination canon<br/>ecosystem.md: pure canon, does NOT sign the relay"]
        NRF["EXTERNAL: nostr-rust-forum<br/>Forum kit — peer on relay mesh<br/>ecosystem.md: receives IS-Envelope, renders ACSP panels"]
        DAW["EXTERNAL: dreamlab-ai-website<br/>Branded deployment<br/>ecosystem.md: downstream consumer of forum kit"]
    end
    VC -->|"peer on relay mesh<br/>renders embodied agent loop (GPU/XR graph)"| NRF
    AB -->|"hosts code-as-harness<br/>NOT double-counted in identity mesh"| NRF
    NRF -.->|"operator overlay"| DAW
    VF -.->|"documentation/positioning only<br/>agentbox/docs/developer/ecosystem.md table"| VC
    VF -.-> AB

    NOTE1["Note: 6 repos total, 5 did:nostr identity-mesh<br/>signing participants (VisionFlow signs nothing)<br/>source: agentbox/docs/developer/ecosystem.md"]
    NOTE1 -.-> VF
```

## ES-01.2 Network / compute fabric — machinelearn, HP-Desktop rail, dead trap
```mermaid
flowchart LR
    subgraph ML["machinelearn (LAN .132)"]
        MLHOST["machinelearn host<br/>agentbox/skills/email-search/SKILL.md:79"]
    end
    subgraph HP["HP-Desktop (downstream, no LAN IP)"]
        HPHOST["HP-Desktop<br/>john@10.10.10.1<br/>agentbox/docs/developer/hp-peer-node.md:3"]
        LOOMFACADE["Loom façade :8084<br/>~/githubs/loom docker container<br/>agentbox/docs/adr/ADR-2023-loom-facade.md"]
        LOOMMODEL["loom-model container :8085<br/>Qwen3.8-27B (cutover 2026-08-14)<br/>agentbox/skills/email-search/SKILL.md:94"]
    end
    MLHOST -->|"25G rail 10.10.10.0/30<br/>hp-nat.service DNAT :8084<br/>agentbox/docs/developer/hp-peer-node.md:3"| HPHOST
    HPHOST --> LOOMFACADE
    LOOMFACADE -->|":8085 HTTP delegates to model"| LOOMMODEL
    MLHOST -->|"embeddings :9997<br/>bge-small-en-v1.5<br/>agentbox/skills/email-search/SKILL.md:109"| XINF["xinference :9997"]

    DEAD[".48 DEAD trap<br/>old 192.168.2.48 model host<br/>agentbox/docs/adr/ADR-2023-loom-facade.md:23<br/>agentbox/skills/email-search/SKILL.md:97,210"]
    DEAD -.->|"never target — black-holes reasoning,<br/>GET /health still 200s"| LOOMFACADE

    MESHNODE["agentbox-hp<br/>2nd full agentbox, own did:nostr<br/>docker compose -f docker-compose.yml<br/>-f docker-compose.hp.yml up -d"]
    HPHOST --- MESHNODE
    MLHOST -->|":9096 NIP-98 door, signed by ml key -> 200<br/>unsigned -> 401 (hp-peer-node.md probe table)"| MESHNODE
    MESHNODE -->|":7777 embedded relay, allowlisted signer -> OK true<br/>non-allowlisted -> rejected pubkey"| MLHOST

    NOTEENV["Note (environment fact, not repo-sourced,<br/>cited per workspace CLAUDE.md Compute and LLM endpoints):<br/>machinelearn LAN is .132 on Mellanox p1 (ens1f1np1) to Sodola TE5;<br/>eno1 is 1G DHCP fallback .160 metric 700;<br/>HP rail is ens1f0np0 to enp65s0f0np0, MTU 9000, never-default;<br/>MSS clamp for the 9000 to 1500 step-down;<br/>Sodola VLANs 20/40/50/100; 192.168.2.0/24 trusted server segment"]
    NOTEENV -.-> MLHOST
```

## ES-01.3 Service and port map — every published surface in the estate
```mermaid
flowchart TB
    subgraph vcstack["VisionClaw stack — docker-compose.unified.yml"]
        VCD["visionclaw_container<br/>profiles development, dev<br/>compose:46,165-167"]
        VCP["visionclaw_prod_container<br/>profiles production, prod<br/>compose:170,239-241"]
        LOOMB["loom-sidecar<br/>profile loom, compose:288,350-351"]
        CFT["cloudflared-tunnel<br/>profiles production, prod<br/>compose:244,263-265"]
    end
    subgraph abstack["agentbox stack — agentbox/docker-compose.yml"]
        ABC["agentbox container<br/>agentbox/docker-compose.yml:30"]
        RPG["ruvector-postgres<br/>agentbox/docker-compose.yml:10"]
    end
    subgraph sidecars["agentbox sidecar overlays"]
        BC["browsercontainer"]
        GT["gui-tools-service"]
        VCON["voice-console"]
        XRR["xr-runtime"]
        AND["agentbox-android"]
        OM["openmed"]
    end

    VCD -->|"3001 nginx<br/>compose:147"| EXT1["host"]
    VCD -->|"4000 Rust backend<br/>compose:148"| EXT1
    VCP -->|"3001 only<br/>compose:215"| EXT1
    LOOMB -->|"host 8090 to container 8080<br/>compose:335"| EXT1
    ABC -->|"9096 LAN — the ONLY 0.0.0.0 publish<br/>agentbox/docker-compose.yml:54"| EXT1
    ABC -->|"127.0.0.1 9090 9700 9091 8484 8888 5901 8080<br/>agentbox/docker-compose.yml:55-61"| LOOPBACK["loopback only"]
    RPG -->|"5432 internal"| ABC
    BC -->|"0.0.0.0 5903 VNC / 8931 MCP SSE<br/>host 9222 to container 9223 CDP"| EXT1
    GT -->|"0.0.0.0 5905 / 9876 / 9877"| EXT1
    VCON -->|"0.0.0.0 8443 / 8444"| EXT1
    XRR -->|"0.0.0.0 5904"| EXT1
    AND -->|"127.0.0.1 5555"| LOOPBACK
    OM -->|"127.0.0.1 9093"| LOOPBACK

    INV["INVARIANT ADR-2013 — every compose publish binds 127.0.0.1<br/>unless it is on the SANCTIONED list. In the main agentbox<br/>compose only :9096 is a LAN door. see ES-10.8"]
    D1["NOT A DEFECT — browsercontainer maps host 9222 to container 9223<br/>by design (docker-compose.browsercontainer.yml:51-53, CDP proxy<br/>host:9222 to socat:9223 to Chrome:9222). The ADR-2013 sanctioned<br/>entry names 9222 (host side) and agentbox/CLAUDE.md names 9223<br/>(container side) — both correct, easy to misread as a conflict."]
    D2["EXTERNAL — Loom :8084 façade and loom-model :8085 run on<br/>HP-Desktop, NOT in either compose file. xinference :9997 and<br/>email-mcp-gateway :8765 are likewise separate services on<br/>visionclaw_network. see ES-01.2 and ES-06.1"]

    LOOPBACK --> INV
    BC --> D1
    LOOMB --> D2
```

## ES-01.4 Docker networks and volumes — one external bridge joins both stacks
```mermaid
flowchart TB
    subgraph net["visionclaw_network — external bridge, declared in BOTH stacks"]
        N1["docker-compose.unified.yml:363-366<br/>external true, name ${EXTERNAL_NETWORK:-visionclaw_network}"]
        N2["agentbox/docker-compose.override.yml:181-183<br/>alias visionclaw, external true"]
    end
    subgraph vcvol["VisionClaw volumes — docker-compose.unified.yml:370-390"]
        V1["loom-data — mirrored corpus generation"]
        V2["visionclaw-data / visionclaw-logs"]
        V3["npm-cache / cargo-cache / cargo-git-cache / cargo-target-cache"]
    end
    subgraph abvol["agentbox volumes — agentbox/docker-compose.yml:167-194"]
        W1["ruvector-pg-data / ruvector-data"]
        W2["solid-data / sovereign-identities / agentbox-secrets"]
        W3["code-harness-data / agentbox-events / consultations-data"]
        W4["hf-cache / codeserver-config / telemetry-data"]
        W5["nostr-relay-data / tailscale-state"]
    end
    subgraph shared["Cross-container shared volumes"]
        S1["gui-tools-exchange — declared by the override AND by<br/>browsercontainer and gui-tools overlays. This is how the<br/>browser sidecar reads files this container writes."]
        S2["mad-workspace — EXTERNAL alias to<br/>multi-agent-docker_workspace, from the deprecated MAD stack"]
    end

    N1 --- N2
    net --> vcvol
    net --> abvol
    net --> shared

    D1["DIVERGENCE — mad-workspace is a legacy external volume created<br/>by the DEPRECATED multi-agent-docker stack and reused so<br/>agentbox sees the full project tree. Migration path is<br/>agentbox.sh migrate-workspace, after which the override should<br/>reference agentbox-workspace instead (Q43)."]
    INV["INVARIANT — the cargo-target-cache and cargo-*-cache volumes are<br/>why the DEV container can compile Rust on startup rather than<br/>in the image build. see ES-09"]

    S2 --> D1
    V3 --> INV
```

## ES-01.5 Compose overlay composition — which file adds what
```mermaid
flowchart LR
    BASE["agentbox/docker-compose.yml<br/>agentbox + ruvector-postgres"]
    OV["docker-compose.override.yml<br/>auto-applied — joins visionclaw_network,<br/>group_add 965 for the docker socket,<br/>mounts mad-workspace + gui-tools-exchange"]
    HP["docker-compose.hp.yml<br/>2nd full agentbox on HP-Desktop,<br/>own did:nostr — see ES-01.2"]
    BCF["docker-compose.browsercontainer.yml<br/>GPU Chrome sidecar"]
    GTF["docker-compose.gui-tools.yml<br/>FHS GUI sidecar"]
    VF["docker-compose.voice.yml<br/>voice-console Caddy origin"]
    XF["docker-compose.xr-runtime.yml<br/>XR runtime + gdext build volumes"]
    SPF["docker-compose.solid-pods.yml<br/>cloudflared-pod tunnel"]
    ANF["docker-compose.android.yml"]
    OMF["docker-compose.openmed.yml"]

    BASE --> OV
    OV --> BCF
    OV --> GTF
    OV --> VF
    OV --> XF
    OV --> SPF
    OV --> ANF
    OV --> OMF
    BASE --> HP

    N1["Every overlay declares the same external network under the<br/>local alias visionclaw, so all sidecars share one bridge."]
    N2["group_add 965 is the docker socket gid — the container drives<br/>docker WITHOUT sudo, which no-new-privileges blocks."]
    D1["RESOLVED ADR-2013 — the voice overlay publishes :8443/:8444 on<br/>0.0.0.0 while the main compose publishes only :9096. Across all<br/>overlays there are TEN sanctioned publishes, each cited on the<br/>SANCTIONED list and CI-enforced — decided exposures, not a<br/>breach. see ES-10.8"]
    D2["TRAP — a build launched from INSIDE this container resolves bind<br/>paths against the HOST filesystem and silently bakes stale code.<br/>Build only from the host shell. see ES-09"]

    OV --> N1
    OV --> N2
    VF --> D1
    BASE --> D2
```

## ES-01.6 Repositories on disk — what is a checkout, what is a stub, what is external
```mermaid
flowchart TB
    subgraph real["Checked out WITH content at the repo root"]
        R1["JavaScriptSolidServer/ — 25 entries<br/>legacy JS Solid server. see ES-08"]
        R2["nntp-stack/ — 8 entries"]
        R3["voice-stack/ — 5 entries"]
        R4["loom/ — README.md + app/<br/>deployment notes only, NO implementation.<br/>The Rust loom-facade lives in the separate loom repo."]
        R5["vircadia-world/ — server/ only"]
    end
    subgraph stub["Uninitialised submodules — an EMPTY same-named subdir only"]
        S1["Kokoros/Kokoros — 0 entries"]
        S2["Whisper-WebUI/Whisper-WebUI — 0 entries"]
        S3["xinference/xinference — 0 entries"]
    end
    subgraph ext["EXTERNAL — not on disk in any form"]
        E1["EXTERNAL: nostr-rust-forum"]
        E2["EXTERNAL: dreamlab-ai-website"]
        E3["EXTERNAL: VisionFlow canon"]
        E4["EXTERNAL: solid-pod-rs — consumed as a crates.io pin<br/>and as an agentbox Nix pin, never as a checkout"]
    end

    WARN["INVARIANT for this diagram tree — a claim about an EXTERNAL<br/>repo may only assert what THIS repo's code or docs state.<br/>Nothing about their internals is asserted here."]
    D1["DIVERGENCE — the three stub directories look like checkouts to<br/>a naive ls but contain nothing. Any doc describing their<br/>contents from this repo is describing something not present."]
    D2["DIVERGENCE — loom/README.md records that a second Python<br/>implementation (app/{loom_facade,ontology_proxy,<br/>ontology_scaffold,loom_graph}.py, 1,727 lines) was DELETED<br/>2026-09-03 as a dead twin of the Rust facade. see ES-06.6"]

    stub --> D1
    R4 --> D2
    ext --> WARN
```
