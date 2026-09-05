---
id: AB-01
title: Nix flake composition and apply-class gates
area: agentbox
governing:
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2003, ADR-2006, ADR-2029, ADR-2039]
sources:
  - agentbox/flake.nix
  - agentbox/agentbox.toml
  - agentbox/lib/gpu-wrap.nix
  - agentbox/lib/npm-cli.nix
  - agentbox/schema/agentbox.toml.schema.json
  - agentbox/scripts/agentbox-config-validate.js
  - agentbox/management-api/lib/system-manifest.js
  - agentbox/agentbox.sh
  - agentbox/flake.lock
verified_commit: b00c28a0d
---

## AB-01.1 agentbox.toml gates to flake.nix conditionals to package set and supervisord text

```mermaid
flowchart TB
    TOML["agentbox.toml<br/>flake.nix:102 builtins.fromTOML"] --> CFG["agentboxConfig"]
    CFG --> DESK["desktopCfg = agentboxConfig.desktop or {}<br/>flake.nix:106"]
    CFG --> MEDIA["mediaCfg = skillsCfg.media or {}<br/>flake.nix:113"]
    CFG --> VAULT["vaultCfg = agentboxConfig.vault or {}<br/>flake.nix:574"]

    DESK -->|"desktopCfg.enabled or false"| DESKOPT["lib.optionals<br/>flake.nix:1486 desktopPackages"]
    MEDIA -->|"mediaCfg.comfyui_builtin or false"| COMFYOPT["lib.optionals<br/>flake.nix:1059 comfyuiPackages"]
    MEDIA -->|"mediaCfg.ffmpeg or false"| FFOPT["lib.optionals<br/>flake.nix:1067 wrapGpuBin ffmpeg"]
    VAULT -->|"vaultCfg.tui == rune"| RUNEOPT["runeActive<br/>flake.nix:576"]

    DESKOPT --> ALLPKG["allPackages closure"]
    COMFYOPT --> ALLPKG
    FFOPT --> ALLPKG
    RUNEOPT -->|"lib.optionals runeActive"| RUNEPKG["runePackages<br/>flake.nix:577-578"]
    RUNEPKG --> ALLPKG

    DESK -->|"lib.optionalString (desktopCfg.enabled or false)"| DESKSUP["desktopBlocks text<br/>flake.nix:1913<br/>spliced flake.nix:2109"]
    MEDIA -->|"lib.optionalString (mediaCfg.comfyui_builtin or false)"| COMFYSUP["program:comfyui-builtin block<br/>flake.nix:2194-2198"]

    DESKSUP --> SUPTEXT["AUTO-GENERATED supervisord.conf text<br/>flake.nix:2866 header comment"]
    COMFYSUP --> SUPTEXT
    ALLPKG --> MKIMAGE["mkImage layers<br/>flake.nix:3459"]

    ALLPKG -.->|"see AB-01.7"| WRAPTGT["wrapped GPU targets"]

    note1["INVARIANT ADR-2003 - every gate touches package set AND supervisor text AND<br/>a system-manifest.js catalogue entry, management-api/lib/system-manifest.js line 39"]
    SUPTEXT --- note1
```

## AB-01.2 apply_class taxonomy - live, boot, rebuild

```mermaid
stateDiagram-v2
    [*] --> live
    [*] --> boot
    [*] --> rebuild

    state live {
        [*] --> LiveRead
        LiveRead: Read at operation time
        LiveRead --> LiveEffect
        LiveEffect: flipping the key affects the running box, no restart
    }
    state boot {
        [*] --> BootRead
        BootRead: Read once at container boot
        BootRead --> BootEffect
        BootEffect: takes effect on next restart, entrypoint reconciles every boot
    }
    state rebuild {
        [*] --> RebuildRead
        RebuildRead: Changes the Nix image composition
        RebuildRead --> RebuildEffect
        RebuildEffect: needs agentbox.sh rebuild, gates package set AND supervisor block
    }

    note right of live
        APPLY_CLASSES const system-manifest.js line 27
        entries browsercontainer 158, gui-tools-service 161, voice-console 164, memory-hygiene 178-180
    end note
    note right of boot
        entries management-api 42, tmux-autostart 45, setup 48, vault root/pages/format 217, memory_learning 176
        stateOf treats mode string off OR none as off, system-manifest.js line 265
    end note
    note right of rebuild
        entries code-server 51, jupyter-lab 54, xvnc 57, comfyui-builtin 60, vault-tui 220
        DOES NOT reconcile on restart, needs full Nix re-evaluation via agentbox.sh rebuild
    end note

    live --> [*]: does NOT reconcile at boot, only live reads
    boot --> [*]: does NOT re-evaluate Nix composition
    rebuild --> [*]: does NOT take effect on a plain restart
```

## AB-01.3 ./agentbox.sh rebuild - down, build --variant runtime, up --build, cleanup

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator
    participant REB as cmd_rebuild<br/>agentbox.sh:1041
    participant DOWN as cmd_down<br/>agentbox.sh:848
    participant BUILD as cmd_build<br/>agentbox.sh:881
    participant NIX as nix build<br/>agentbox.sh:897
    participant UP as cmd_up<br/>agentbox.sh:705
    participant DOCKER as docker compose
    participant CLEAN as post-deploy-cleanup.sh

    OP->>REB: ./agentbox.sh rebuild [--no-cleanup]
    REB->>DOWN: cmd_down (agentbox.sh:1054)
    DOWN->>DOCKER: docker compose down (agentbox.sh:873)
    DOCKER-->>DOWN: stack stopped
    REB->>BUILD: cmd_build --variant runtime (agentbox.sh:1057)
    BUILD->>BUILD: validate variant in runtime|desktop|full<br/>agentbox.sh:891-893
    alt unknown variant
        BUILD-->>OP: exit 1 Unknown variant
    end
    BUILD->>NIX: nix build .#runtime (agentbox.sh:897)
    NIX-->>BUILD: result symlink resolved (agentbox.sh:899-900)
    REB->>UP: cmd_up --build (agentbox.sh:1060)
    UP->>UP: mutually exclusive check --build vs --registry<br/>agentbox.sh:720-723
    UP->>NIX: nix build .#runtime (agentbox.sh:728)
    UP->>DOCKER: nix run .#runtime.copyToDockerDaemon<br/>agentbox.sh:732
    UP->>UP: unset AGENTBOX_IMAGE_REF (agentbox.sh:734)
    UP->>UP: resolve image hash + manifest checksum<br/>agentbox.sh:757-766
    opt visionclaw_network absent
        UP->>DOCKER: docker network create visionclaw_network<br/>agentbox.sh:773
    end
    opt orphaned ruvector-postgres container
        UP->>DOCKER: docker rm -f ruvector-postgres<br/>agentbox.sh:788
    end
    UP->>DOCKER: docker compose up -d (agentbox.sh:799)
    loop poll every 2s up to 120s
        UP->>DOCKER: curl READY_URL (agentbox.sh:827)
    end
    alt readiness times out
        UP-->>OP: exit 1 Readiness check timed out<br/>agentbox.sh:835
    end
    UP-->>REB: Stack is up and ready (agentbox.sh:841)
    alt skip_cleanup == 0
        REB->>CLEAN: bash scripts/post-deploy-cleanup.sh<br/>agentbox.sh:1064
        CLEAN->>CLEAN: 1/5 prune old agentbox images, keep CURRENT_ID
        CLEAN->>CLEAN: 2/5 docker system prune -f
        CLEAN->>CLEAN: 3/5 nix store gc
        CLEAN->>CLEAN: 4/5 clean tmp build files
        CLEAN->>CLEAN: 5/5 reap stale cargo target dirs, AGENTBOX_REAP_CARGO
    else --no-cleanup
        Note over REB,CLEAN: cleanup skipped, skip_cleanup=1, agentbox.sh:1051,1061
    end
```

## AB-01.4 Static config validation - schema then semantic rules

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator or CI
    participant WRAP as agentbox-config-validate.sh
    participant JS as agentbox-config-validate.js
    participant AJV as Ajv 2020 compiler<br/>agentbox-config-validate.js:126
    participant SCHEMA as agentbox.toml.schema.json
    participant SEM as semantic rule functions

    OP->>WRAP: ./scripts/agentbox-config-validate.sh agentbox.toml
    WRAP->>WRAP: probe node_modules/@iarna/toml, ajv<br/>agentbox-config-validate.sh:27-29
    opt node_modules missing
        WRAP->>WRAP: npm ci bootstrap or exit 2 if AGENTBOX_VALIDATOR_NO_BOOTSTRAP=1
    end
    WRAP->>JS: exec node agentbox-config-validate.js agentbox.toml
    JS->>JS: TOML.parse(raw)<br/>agentbox-config-validate.js:106
    alt TOML parse error
        JS-->>OP: emit E000, exit 1<br/>agentbox-config-validate.js:110-111
    end
    JS->>SCHEMA: JSON.parse schema file<br/>agentbox-config-validate.js:117
    JS->>AJV: ajv.compile(schema)<br/>agentbox-config-validate.js:126
    JS->>AJV: validate(manifest)<br/>agentbox-config-validate.js:127
    alt schemaValid == false
        AJV-->>JS: additionalProperty violations
        JS->>JS: push E016 UnknownManifestKey per error<br/>agentbox-config-validate.js:137-143
    end
    JS->>SEM: run E0xx/W0xx rule families<br/>adapters, providers, nostr relay, privacy filter, linked-data
    SEM-->>JS: errors[] and warnings[] arrays populated
    JS->>JS: emit every warning to stderr<br/>agentbox-config-validate.js:1494-1496
    alt errors.length == 0
        JS-->>OP: stdout "agentbox manifest valid" + advisory count, exit 0<br/>agentbox-config-validate.js:1499-1502
    else errors present
        JS->>JS: emit every error to stderr
        JS-->>OP: exit 1<br/>agentbox-config-validate.js:1507
    end

    Note over JS,SEM: DIVERGENCE - static-schema stage is advisory for W0xx dead-policy warnings,<br/>only E016 schema-additionalProperties violations and other E-code semantic rules hard-fail<br/>see agentbox-config-validate.js line 4 comment and lines 1499-1507 exit logic
```

## AB-01.5 npm-cli.nix pinned exact-semver closure - ruvector always in package set

```mermaid
sequenceDiagram
    autonumber
    participant FLAKE as flake.nix eval<br/>flake.nix:189
    participant MK as makeNpmCli<br/>lib/npm-cli.nix:120
    participant FETCH as pkgs.fetchurl stage 1<br/>lib/npm-cli.nix:184
    participant FOD as packageWithDeps FOD stage 2<br/>lib/npm-cli.nix:202
    participant WRAP as wrapper derivation stage 3
    participant ALWAYS as npmCliAlwaysPackages<br/>flake.nix:414

    FLAKE->>MK: mkNpmCli pkgName=ruvector version=0.3.0<br/>flake.nix:194-199
    MK->>FETCH: registryUrl ruvector 0.3.0<br/>lib/npm-cli.nix:102-115
    FETCH->>FETCH: sha256 = SRI hash of the .tgz<br/>lib/npm-cli.nix:186-187
    alt sha256 is lib.fakeHash placeholder
        FETCH-->>MK: eval-time hint, realisation-time hash mismatch<br/>lib/npm-cli.nix:161-173
    end
    MK->>FOD: npm install --production --ignore-scripts --legacy-peer-deps<br/>lib/npm-cli.nix:264
    FOD->>FOD: outputHash = nodeModulesHash, network allowed inside sandbox<br/>lib/npm-cli.nix header Stage 2 rationale lines 29-37
    FOD-->>MK: $out/lib/ruvector with populated node_modules
    MK->>WRAP: thin mkDerivation, no network, writes $out/bin/ruvector wrapper<br/>lib/npm-cli.nix Stage 3 rationale lines 39-41
    WRAP-->>FLAKE: ruvectorPkg derivation
    FLAKE->>ALWAYS: npmCliAlwaysPackages = [ ruvectorPkg wranglerPkg ]<br/>flake.nix:414
    Note over FLAKE,ALWAYS: comment at flake.nix:193 says pin is ruvector-0.2.25,<br/>but the version field at flake.nix:196 is 0.3.0
    Note over FLAKE,ALWAYS: RESOLVED ADR-2039: BASELINE-container.md:43 now<br/>states 0.3.0. The nix-prefetch-url comment at flake.nix:193<br/>still names ruvector-0.2.25.tgz - stale code comment, left<br/>as-is deliberately, not a doc claim
```

## AB-01.6 gpu-wrap.nix wrapGpuBins - LD_LIBRARY_PATH suffix and vendor ICDs

```mermaid
sequenceDiagram
    autonumber
    participant FLAKE as flake.nix eval<br/>flake.nix:166-169
    participant WRAP as gpuWrap.wrapGpuBins<br/>lib/gpu-wrap.nix:76
    participant JOIN as pkgs.symlinkJoin<br/>lib/gpu-wrap.nix:77
    participant MAKEW as makeWrapper wrapProgram<br/>lib/gpu-wrap.nix:87
    participant BIN as wrapped binary at runtime

    FLAKE->>FLAKE: gpuActive = agentbox.toml gpu.backend == local-cuda<br/>flake.nix:166
    alt gpu.backend == none
        FLAKE->>FLAKE: wrapGpuBin pkg bins = pkg, unwrapped passthrough<br/>flake.nix:167-168
        Note over FLAKE: alt gpu.backend=none - wrapping is inert without injected driver libs, gpu-wrap.nix comment lines 21-22
    else gpu.backend == local-cuda
        FLAKE->>WRAP: wrapGpuBins pkg=pkgs.blender bins=[blender]<br/>flake.nix:1082
        WRAP->>JOIN: paths=[pkg], nativeBuildInputs=[makeWrapper]<br/>lib/gpu-wrap.nix:77-80
        JOIN->>MAKEW: for each bin, wrapProgram target gpuEnvArgs<br/>lib/gpu-wrap.nix:81-89
        MAKEW->>MAKEW: --suffix LD_LIBRARY_PATH : /usr/lib:/usr/lib/x86_64-linux-gnu:/run/opengl-driver/lib<br/>lib/gpu-wrap.nix:46-51,56
        MAKEW->>MAKEW: --set-default __GLX_VENDOR_LIBRARY_NAME nvidia<br/>lib/gpu-wrap.nix:57
        MAKEW->>MAKEW: --set-default __EGL_VENDOR_LIBRARY_FILENAMES /usr/share/glvnd/egl_vendor.d/10_nvidia.json<br/>lib/gpu-wrap.nix:58-59
        MAKEW->>MAKEW: --set-default VK_ICD_FILENAMES /run/opengl-driver/share/vulkan/icd.d/nvidia_icd.x86_64.json<br/>lib/gpu-wrap.nix:60-65
        JOIN-->>WRAP: symlinkJoin derivation, meta description appended C-9<br/>lib/gpu-wrap.nix:93-96
        WRAP-->>FLAKE: gpu-wrapped drop-in replacement for pkg
        FLAKE->>BIN: dlopen libcuda.so.1 resolves via appended LD_LIBRARY_PATH
        BIN-->>FLAKE: CUDA devices enumerated, verified RTX A6000 + 2x RTX 6000 Ada 2026-08-31
    end

    Note over MAKEW: INVARIANT - --suffix never --prefix, so Nix's own libstdc++/libc stays authoritative,<br/>ADR-2006 and gpu-wrap.nix comment lines 23-26
    Note over BIN: DIVERGENCE - GPU wrapper is CUDA-only by design, no Nix-binary Vulkan/GLX presentation path,<br/>interactive 3D depends on the FHS gui-tools sidecar, ADR-2006 Context and Consequences
    Note over MAKEW: DIVERGENCE - BASELINE GPU scope and evidence qualification 2026-09-04 -<br/>current wrappers include GLX/EGL/Vulkan defaults alongside CUDA library-path config,<br/>so ADR-2006's graphics review trigger has been reached, agentbox/docs/BASELINE-container.md line 206-208
```

## AB-01.7 Wrapped-target list - ffmpeg, qgis, blender, 3DGS

```mermaid
flowchart TB
    GPUACT["gpuActive = gpu.backend == local-cuda<br/>flake.nix:166"]

    subgraph MEDIA["mediaPackages - flake.nix:1063-1070"]
        FF["wrapGpuBin ffmpeg bins ffmpeg ffprobe ffplay<br/>flake.nix:1068<br/>gate: mediaCfg.ffmpeg or false"]
    end

    subgraph SPATIAL["spatialPackages - flake.nix:1072-1088"]
        QGIS["wrapGpuBin pkgs.qgis bin qgis<br/>flake.nix:1075<br/>gate: spatialCfg.qgis or false"]
        BLENDER["wrapGpuBin pkgs.blender bin blender<br/>flake.nix:1082<br/>gate: spatialCfg.blender or false"]
        GAUSS["gauss3dPackages via lib/3dgs-stack.nix<br/>flake.nix:427-428<br/>gate: spatialCfg.gaussian_splatting or false"]
        WRAPALL["map wrapGpuAll gauss3dPackages<br/>flake.nix:1088"]
        GAUSS --> WRAPALL
    end

    GPUACT --> FF
    GPUACT --> QGIS
    GPUACT --> BLENDER
    GPUACT --> WRAPALL

    FF --> MEDIAPKG["mediaPackages closure"]
    QGIS --> SPATIALPKG["spatialPackages closure"]
    BLENDER --> SPATIALPKG
    WRAPALL --> SPATIALPKG

    MEDIAPKG --> ALLPKG["allPackages / mkImage layers<br/>flake.nix:3459-3466"]
    SPATIALPKG --> ALLPKG

    NOTE1["INVARIANT - wrapGpuBin names exact bins, wrapGpuAll wraps every<br/>executable under out/bin for upstream-versioned bin sets like colmap/lichtfeld,<br/>flake.nix comment lines 1083-1087"]
    WRAPALL --- NOTE1
```

## AB-01.8 agentbox.toml schema shape - top-level sections

```mermaid
classDiagram
    class AgentboxToml {
        +GpuSection gpu
        +VaultSection vault
        +SkillsSection skills
        +AdaptersSection adapters
        +CoreSection core
        +FederationSection federation
    }
    class GpuSection {
        +string backend
    }
    class VaultSection {
        +string root
        +string pages
        +string format
        +string tui
        +string working
        +string transcripts
    }
    class SkillsSection {
        +BrowserSkills browser
        +MediaSkills media
        +SpatialSkills spatial_and_3d
        +DataScienceSkills data_science
    }
    class AdaptersSection {
        +string beads
        +string pods
        +string memory
        +string events
        +string orchestrator
    }
    class CoreSection {
        +string orchestration
        +string vector_db
    }
    class FederationSection {
        +string mode
        +string external_url
    }

    AgentboxToml "1" --> "1" GpuSection
    AgentboxToml "1" --> "1" VaultSection
    AgentboxToml "1" --> "1" SkillsSection
    AgentboxToml "1" --> "1" AdaptersSection
    AgentboxToml "1" --> "1" CoreSection
    AgentboxToml "1" --> "1" FederationSection

    note for GpuSection "backend enum: none, ollama-rocm, ollama-cuda, local-cuda - schema.json line 108-113"
    note for VaultSection "required: root - schema.json line 222-224. format enum obsidian, logseq-legacy - schema.json line 244-246. tui enum rune, none, default none - schema.json line 264-269, ADR-2029"
    note for AdaptersSection "each value resolves to local-star, external, or off per slot - agentbox/CLAUDE.md adapter contract"
```

## AB-01.9 flake.lock inputs to flake outputs

```mermaid
flowchart LR
    subgraph INPUTS["flake.lock root inputs"]
        NIXPKGS["nixpkgs - nixpkgs_3<br/>github NixOS/nixpkgs pin e7a3ca8092b6<br/>flake.nix:5"]
        FU["flake-utils<br/>github numtide/flake-utils<br/>flake.nix:6"]
        N2C["nix2container<br/>github nlewo/nix2container<br/>flake.nix:7"]
        RO["rust-overlay<br/>github oxalica/rust-overlay<br/>flake.nix:8"]
        AOE["aoe<br/>github DreamLab-AI/agentbox-of-empires pin d615b8c8<br/>flake.nix:18"]
        SKILLS["skills - path:./skills, flake=false<br/>flake.nix:26-27"]
        CODEX["codexPlugin<br/>github openai/codex-plugin-cc pin db52e28f<br/>flake.nix:37-38"]
    end

    NIXPKGS --> EVAL["flake outputs eval, per-system<br/>flake.nix flake-utils.lib.eachDefaultSystem"]
    FU --> EVAL
    N2C --> EVAL
    RO --> EVAL
    AOE --> EVAL
    SKILLS --> EVAL
    CODEX --> EVAL

    EVAL --> PACKAGES["packages - flake.nix:3498<br/>lib.optionalAttrs pkgs.stdenv.isLinux"]
    EVAL --> DEVSHELL["devShells.default<br/>flake.nix:3575"]

    PACKAGES --> RUNTIME["runtime = mkImage tag runtime-system<br/>flake.nix:3499"]
    PACKAGES --> FULL["full = mkImage extraPackages allPackages<br/>flake.nix:3500-3504"]
    PACKAGES --> DESKTOP["desktop = mkImage extraPackages desktopPackages<br/>flake.nix:3505-3509"]
    PACKAGES --> CUDART["cuda-runtime, requires gpu.backend local-cuda<br/>flake.nix:3527-3541"]
    PACKAGES --> GSPLAT["gaussian-splatting = 3DGS stack over cuda-runtime<br/>flake.nix:3550-3567"]
    PACKAGES --> COMPOSE["compose = docker-compose.yml text, cross-platform<br/>flake.nix:3569-3573"]

    RUNTIME --> MKIMG["mkImage - n2c.buildImage 4 layers<br/>flake.nix:3459-3486"]
    FULL --> MKIMG
    DESKTOP --> MKIMG

    MKIMG --> ENTRYPOINT["config = Entrypoint entrypoint/bin/entrypoint<br/>flake.nix:3476"]

    NOTE1["INVARIANT - container-image outputs are Linux-only,<br/>darwin exposes only compose and devShells, flake.nix comment lines 3489-3492"]
    PACKAGES --- NOTE1
```

## AB-01.10 ADR-2029 vault-tui rebuild-class gate - two catalogue entries

```mermaid
flowchart TB
    TOMLVAULT["[vault] in agentbox.toml<br/>agentbox.toml:680-692<br/>tui = rune"] --> VAULTCFG["vaultCfg = agentboxConfig.vault or {}<br/>flake.nix:574"]

    VAULTCFG --> TUIVAL["vaultTui = vaultCfg.tui or none<br/>flake.nix:575"]
    TUIVAL --> RUNEACTIVE["runeActive = vaultTui == rune<br/>flake.nix:576"]
    RUNEACTIVE -->|"true"| RUNEPKGIMPORT["runePkg = import lib/rune.nix<br/>flake.nix:577, lazy import"]
    RUNEACTIVE -->|"lib.optionals runeActive"| RUNEPACKAGES["runePackages<br/>flake.nix:578"]
    RUNEPKGIMPORT --> RUNEPACKAGES
    RUNEPACKAGES --> ALLPKG["allPackages closure - Nix image composition"]

    RUNEBUILD["rune.nix pkgs.rustPlatform.buildRustPackage<br/>fetchFromGitHub aka-rider/rune tag v1.4.0<br/>lib/rune.nix:14-24<br/>cargoBuildFlags -p rune-cli, doCheck=false"]
    RUNEPKGIMPORT --> RUNEBUILD

    subgraph MANIFEST["system-manifest.js CATALOGUE - management-api/lib/system-manifest.js"]
        VAULTENTRY["id vault<br/>gate vault.format, apply_class boot<br/>line 216-218"]
        VAULTTUIENTRY["id vault-tui<br/>gate vault.tui, apply_class rebuild<br/>line 219-221"]
    end

    TOMLVAULT -.->|"vault.format read at boot"| VAULTENTRY
    TOMLVAULT -.->|"vault.tui read at rebuild"| VAULTTUIENTRY

    STATEOF["stateOf function<br/>system-manifest.js:248-267"]
    VAULTTUIENTRY --> STATEOF
    STATEOF --> MODESTRING["mode string off OR none both count as off<br/>system-manifest.js:261-265"]

    NOTE1["INVARIANT - two catalogue entries, not one, because root/pages/format<br/>are boot-class and tui is rebuild-class, one entry would mislead the operator,<br/>system-manifest.js comment lines 210-215"]
    VAULTTUIENTRY --- NOTE1

    NOTE2["DIVERGENCE - vault-tui is REBUILD-class, flipping tui=rune and restarting<br/>the container is NOT sufficient, the Rune binary is absent from the package set<br/>until agentbox.sh rebuild runs, system-manifest.js line 221"]
    STATEOF --- NOTE2
```

