---
id: VC-08
title: Observability, liveness canaries, health composition and the dev/production build loop
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
  - docs/SECURITY-profiles.md
adrs: [ADR-2008, ADR-2026, ADR-2037, ADR-2038, ADR-2049]
sources:
  - src/main.rs
  - src/telemetry/mod.rs
  - src/telemetry/agent_telemetry.rs
  - src/handlers/metrics_handler.rs
  - src/handlers/consolidated_health_handler.rs
  - src/handlers/liveness_harness_handler.rs
  - src/handlers/client_log_handler.rs
  - src/services/liveness_harness.rs
  - src/services/canary_nostr_tap.rs
  - src/services/kpi_compute.rs
  - src/adapters/sqlite_canary_repository.rs
  - src/app_state.rs
  - scripts/rust-backend-wrapper.sh
  - scripts/lib/build-inputs.sh
  - supervisord.dev.conf
  - supervisord.production.conf
  - Dockerfile.unified
  - Dockerfile.production
  - src/config/security_profile.rs
  - crates/visionclaw-gpu/build.rs
  - src/middleware/rbac_gate.rs
  - src/utils/advanced_logging.rs
verified_commit: bed6b617d
---

## VC-08.1 Health and readiness — what each probe actually asserts
```mermaid
sequenceDiagram
    autonumber
    participant K as probe caller
    participant L as liveness_probe<br/>src/handlers/consolidated_health_handler.rs
    participant R as readiness_probe<br/>src/handlers/consolidated_health_handler.rs
    participant AS as AppState::get_degraded_reason<br/>src/app_state.rs
    participant U as unified_health_check<br/>src/handlers/consolidated_health_handler.rs:477

    K->>L: GET /healthz
    L-->>K: 200 {"status":"alive"} — unconditional, no state read
    Note over L: liveness answers even while every subsystem is down — it proves only that the process runs
    K->>R: GET /readyz
    R->>AS: get_degraded_reason()
    alt Some(reason)
        AS-->>R: degraded
        R-->>K: 503 {"status":"not_ready","reason":reason}
        Note over R,AS: doc comment — 503 if critical subsystems (e.g. the Oxigraph store) are unavailable
    else None
        AS-->>R: healthy
        R-->>K: 200 {"status":"ready"}
    end
    K->>U: GET /api/health
    U-->>K: composed health JSON with a `status` field
    Note over U: this is the endpoint the KG watchdog self-polls — see VC-08.2
    Note over K,U: registration — root /healthz and /readyz at src/main.rs:1026-1027 (outside /api, so no<br/>RbacGate and no PublicDemoGuard) — a second /api/healthz and /api/readyz pair for back-compat<br/>at src/handlers/consolidated_health_handler.rs:488-489. See VC-01.7
```

## VC-08.2 KG watchdog — the self-poll that drives kg_backend_up
```mermaid
sequenceDiagram
    autonumber
    participant M as main<br/>src/main.rs:1176-1194
    participant W as run_kg_watchdog<br/>src/services/liveness_harness.rs:444
    participant P as probe_once
    participant V as health_verdict<br/>src/services/liveness_harness.rs:494-514
    participant H as LivenessHarness::record_kg_state<br/>src/services/liveness_harness.rs:422

    M->>W: tokio::spawn(run_kg_watchdog(harness, self_url, period))
    Note over M,W: VISIONCLAW_SELF_URL default http 127.0.0.1 port (src/main.rs:1181)<br/>VISIONCLAW_KG_WATCHDOG_SECS default 30 (src/main.rs:1183)
    loop every period (default 30s)
        W->>P: GET {self_url}/api/health
        Note over P: this server IS the KG backend — the watchdog polls itself
        P->>V: health_verdict(is_success, json)
        alt transport failed (is_success false)
            V-->>W: DOWN
            Note over V: a non-2xx or transport error is backend loss so an outage cannot latch the gauge UP
        else 2xx with status == "unhealthy"
            V-->>W: DOWN
        else 2xx with status present and not "unhealthy"
            V-->>W: UP
        else 2xx, valid JSON, no status field
            V-->>W: UP
            Note over V: a terse healthy response counts as UP (:500-503)
        end
        W->>H: record_kg_state(up)
        alt state CHANGED since the last poll
            H->>H: log "kg_backend_up: X -> Y (watchdog self-poll /api/health)" (:429)
            H->>H: observe(CANARY_KG, evidence) (:433)
            Note over H: CANARY-VC-RESA-KG (src/services/liveness_harness.rs:32)<br/>fires on observed CHANGE, not on every tick (:421)
        else unchanged
            H-->>W: no canary fired
        end
    end
    Note over H: the gauge is tri-state — KG_UNKNOWN 0, KG_UP 1, KG_DOWN 2 (:263-265)<br/>kg_backend_up() returns None until the first poll (:408-410)
```

## VC-08.3 LivenessHarness — the canary registry and observation surface
```mermaid
sequenceDiagram
    autonumber
    participant B as boot
    participant LH as LivenessHarness<br/>src/services/liveness_harness.rs:293
    participant DB as SqliteCanaryRepository<br/>src/adapters/sqlite_canary_repository.rs
    participant API as /api/canary routes<br/>src/handlers/liveness_harness_handler.rs:212

    B->>LH: LivenessHarness::new(repo) (:300)
    par seeding by priority band
        B->>LH: seed_p0_canaries() (:329) — P0_CANARIES (:46)
    and
        B->>LH: seed_p1_canaries() (:355) — P1_CANARIES (:161)
    and
        B->>LH: seed_p2_canaries() (:385) — P2_CANARIES (:222)
    end
    LH->>DB: register(CanaryRegistration)
    Note over LH,DB: each entry is a 4-tuple (id, description, kind, priority) — kind "standing" for the wire probes
    API->>LH: POST /api/canary/register (:213) → register (:308)
    API->>LH: POST /api/canary/observe/{canary_id} (:214) → observe
    API->>LH: GET /api/canary/status (:215)
    LH->>DB: append fire-log row
    Note over LH: FRESHNESS_WINDOW_MS = 30 * 24 * 60 * 60 * 1000 (:41) — a 30-day staleness horizon
    Note over LH: current_sha() (:278) stamps each observation, from VISIONCLAW_GIT_SHA (:279)
    Note over API: VISIONCLAW_AGENT_KEY is read by the handler at src/handlers/liveness_harness_handler.rs:36<br/>env register see VC-09.10
```

## VC-08.4 Canary identifier register
```mermaid
flowchart TB
    P0["P0_CANARIES — src/services/liveness_harness.rs:46"]
    P0 --> P0A["CANARY-VC-COM14-DID — selected node addressable by a verified did:nostr<br/>Schnorr challenge at selection, kind standing"]
    P0 --> P0B["CANARY_REC2_CASE = CANARY-VC-REC2-CASE (:37)<br/>broker new_case then case_decided on the multiplexed graph socket"]
    P0 --> P0C["CANARY_KG = CANARY-VC-RESA-KG (:32) — driven by the watchdog, see VC-08.2"]
    P1["P1_CANARIES — src/services/liveness_harness.rs:161"]
    P1 --> P1A["CANARY_COM15_PTT :89 · CANARY_D2_STEER :95 · CANARY_D8_OBS :100<br/>CANARY_D1_BEAM :106 · CANARY_REC3_CTC :112 · CANARY_REC4_KPI :117"]
    P2["P2_CANARIES — src/services/liveness_harness.rs:222"]
    P2 --> P2A["CANARY_RESD_COUNT :122 · CANARY_V3_REPAIR :129 · CANARY_REC10_LOOP :136<br/>CANARY_REC11_TRACE :143 · CANARY_M4_RAY :149 · CANARY_COM18_INTERV :155"]
    G["kg_backend_up gauge states — KG_UNKNOWN 0, KG_UP 1, KG_DOWN 2 (:263-265)"]
    P0C --- G
    N["each constant maps to a subsystem canary in the estate review — the wire crossings<br/>are recorded via observe() on the standing wire rather than at seed time (:354, :381)"]
    P1 --- N
```

## VC-08.5 Canary Nostr tap — the relay-side observation path
```mermaid
sequenceDiagram
    autonumber
    participant M as main<br/>src/main.rs:1204-1213
    participant T as CanaryNostrTap::from_env<br/>src/services/canary_nostr_tap.rs:245
    participant R as Nostr relay
    participant E as TapEvent::from_value<br/>src/services/canary_nostr_tap.rs:88
    participant D as map_event_to_observation<br/>src/services/canary_nostr_tap.rs:155
    participant LH as LivenessHarness.observe

    alt CANARY_TAP_RELAY_URL is set (:246)
        M->>T: from_env(harness)
        T-->>M: Some(tap)
        M->>T: tokio::spawn(tap.run()) (:273)
    else unset
        M->>M: log "canary Nostr tap not started"
    end
    loop reconnect with exponential backoff
        T->>R: subscribe, SUBSCRIPTION_ID = "canary-tap" (:64)
        Note over T,R: INITIAL_BACKOFF 1s (:62) to MAX_BACKOFF 60s (:63)
        R-->>T: event, kind LIVENESS_CANARY_KIND = 1 (:58), tag "liveness-canary" (:60)
        T->>E: TapEvent::from_value(obj, sig_verified)
        alt not a well-formed tap event
            E-->>T: None — dropped
        end
        T->>D: map_event_to_observation(ev, allowed_pubkeys)
        Note over D: allowed set from CANARY_TAP_ALLOWED_PUBKEYS (:256)
        alt pubkey not allowed or signature unverified
            D-->>T: rejected TapDecision
        else accepted
            D-->>T: observation
            T->>LH: observe — the SAME path the HTTP route uses
        end
    end
    Note over M,LH: RES-a / WP-11 AC3 / ADR-130 D3 — this lets Nostr-only repositories (nostr-rust-forum,<br/>solid-pod-rs) fire canaries they cannot POST over HTTP. Detached task, fail-open.
```

## VC-08.6 Metrics endpoint and telemetry logger
```mermaid
sequenceDiagram
    autonumber
    participant CL as scraper
    participant MH as get_metrics<br/>src/handlers/metrics_handler.rs:34
    participant PS as ProcessStartTime<br/>src/handlers/metrics_handler.rs:13
    participant EB as collect_event_bus_metrics<br/>src/handlers/metrics_handler.rs:63
    participant AS as AppState

    CL->>MH: GET /api/metrics (route registered src/handlers/metrics_handler.rs:92)
    MH->>PS: read ProcessStartTime(Instant) — injected at src/main.rs:987
    MH->>EB: collect_event_bus_metrics(&app_state)
    EB->>AS: read bus counters
    EB-->>MH: EventBusMetrics (:24)
    MH-->>CL: MetricsResponse (:16)
    Note over MH: the endpoint sits inside /api so RbacGate applies — see VC-03.6
```

## VC-08.7 Agent telemetry logger — structured event capture
```mermaid
classDiagram
    class AgentTelemetryLogger {
      <<src/telemetry/agent_telemetry.rs:20>>
      +new(log_dir, buffer_size) :28
      +set_correlation_context(key, id) :39
      +get_correlation_context(key) :45
      +log_event(TelemetryEvent) :53
      +log_agent_spawn(...) :103
      +log_position_update(...) :154
      +log_gpu_execution(...) :190
      +log_mcp_message(...) :217
      +log_graph_state_change(...) :243
      +flush() :323
    }
    class CorrelationId {
      +from_client_session(...) :74
    }
    class GlobalAccess {
      <<free functions>>
      +init_telemetry_logger(log_dir, buffer_size) :335
      +get_telemetry_logger() Option :349
    }
    AgentTelemetryLogger ..> CorrelationId
    GlobalAccess ..> AgentTelemetryLogger
```

## VC-08.8 Client-log ingest
```mermaid
sequenceDiagram
    autonumber
    participant B as browser
    participant RG as RbacGate<br/>src/middleware/rbac_gate.rs
    participant H as handle_client_logs<br/>src/handlers/client_log_handler.rs
    participant T as telemetry sink

    B->>RG: POST /api/client-logs
    Note over RG: ALLOWLISTED — client-logs bypasses the RBAC requirement.<br/>Registered FIRST inside /api at src/main.rs:1057 to avoid scope conflicts. See VC-03.6
    RG->>H: forward
    H->>T: append client-side log records
    H-->>B: ack
    Note over B,T: LOG_DIR src/utils/advanced_logging.rs:570 · DEBUG_ENABLED :643<br/>TELEMETRY_LOG_DIR src/main.rs:266 — env register see VC-09.8
```

## VC-08.9 ADR-2008 dev restart loop — the timestamp-gated wrapper
```mermaid
sequenceDiagram
    autonumber
    participant SV as supervisord<br/>supervisord.dev.conf:19-32
    participant WR as rust-backend-wrapper.sh<br/>scripts/rust-backend-wrapper.sh
    participant NV as nvidia-smi
    participant BI as build-inputs.sh<br/>scripts/lib/build-inputs.sh
    participant CG as cargo
    participant BIN as visionclaw-server

    Note over SV: program rust-backend — command /app/scripts/rust-backend-wrapper.sh<br/>autostart, autorestart, startretries 3, startsecs 10, stopwaitsecs 30
    Note over SV: env RUST_LOG, RUST_BACKTRACE=1, SYSTEM_NETWORK_PORT=4000, MCP_TCP_PORT=9500,<br/>MCP_TRANSPORT=tcp, CLAUDE_FLOW_HOST=multi-agent-container, DOCKER_ENV=1
    SV->>WR: exec
    WR->>NV: nvidia-smi --query-gpu=compute_cap
    alt detected
        NV-->>WR: e.g. 89
        WR->>WR: export CUDA_ARCH=89 — ALWAYS prefer runtime detection over .env
        opt .env CUDA_ARCH differs
            WR->>WR: warn "WARNING: .env CUDA_ARCH=X != GPU sm_Y. Overriding."
        end
    else nvidia-smi failed
        WR->>WR: export CUDA_ARCH=${CUDA_ARCH:-75} with a warning
    end
    WR->>WR: BUILD_FEATURES default "gpu,ontology,dev-auth"
    Note over WR: the feature set is part of the binary's identity — a change must rebuild<br/>even when no file changed, so it feeds the stamp signature
    alt SKIP_RUST_REBUILD != true
        WR->>BI: needs_rebuild(RUST_BINARY, /app, BUILD_STAMP, BUILD_FEATURES)
        BI-->>WR: 0 build / 1 skip, plus a one-line reason
        alt skip
            WR->>WR: log "Skipping cargo: binary is up to date"
        else build
            WR->>CG: cargo build --release --features "$BUILD_FEATURES"
            alt success
                CG-->>WR: ok
                WR->>BI: write_build_stamp(BUILD_STAMP, BUILD_FEATURES)
            else failure
                WR->>WR: rm -f BUILD_STAMP — a failed build leaves a stamp describing an environment no binary exists for
                WR->>CG: cargo clean then retry cargo build --release --features
                alt retry fails
                    WR-->>SV: log FATAL then exit 1
                end
            end
        end
    else SKIP_RUST_REBUILD=true
        WR->>WR: RUST_BINARY=$APP_ROOT/visionclaw-server
    end
    alt binary missing
        WR-->>SV: ERROR then exit 1
    end
    WR->>BIN: exec ${RUST_BINARY}
    Note over SV,BIN: sibling programs — nginx (supervisord.dev.conf:8) and vite-dev (:34)<br/>vite env NODE_ENV=development, VITE_DEV_SERVER_PORT=5173, VITE_API_PORT=4000,<br/>VITE_HMR_PORT=24678, VITE_DEV_MODE_AUTH=true
```

## VC-08.10 ADR-2008 build-input inventory — what counts as a build input
```mermaid
flowchart TB
    NR["needs_rebuild(binary, root, stamp, features)<br/>scripts/lib/build-inputs.sh:116 — rebuild is the SAFE DEFAULT"]
    NR --> C1{"binary file exists?"}
    C1 -->|no| B1["BUILD — 'no binary at PATH'"]
    C1 -->|yes| C2["latest_build_input_mtime(root) :74"]
    C2 --> C3{"latest == 0?"}
    C3 -->|yes| B2["BUILD — 'no build inputs found (refusing to trust the binary)'"]
    C3 -->|no| C4{"bin_mtime <= latest?"}
    C4 -->|yes| B3["BUILD — 'build input newer than binary'"]
    C4 -->|no| C5{"stamp file exists?"}
    C5 -->|no| B4["BUILD — 'no build stamp (build environment unverifiable)'"]
    C5 -->|yes| C6{"stamp == build_env_signature(features)?"}
    C6 -->|no| B5["BUILD — 'build environment changed (was X, now Y)'"]
    C6 -->|yes| SK["SKIP — 'binary is up to date, environment unchanged'"]
    GLOB["BUILD_INPUT_NAME_GLOBS :42<br/>*.rs (incl. every crate build.rs) · *.cu *.cuh *.ptx<br/>Cargo.toml (root AND crate manifests) · Cargo.lock<br/>rust-toolchain · rust-toolchain.toml · config.toml"]
    PRUNE["BUILD_INPUT_PRUNE_DIRS :33 — target node_modules .git .venv dist"]
    ENVV["BUILD_INPUT_ENV_VARS :46 — CUDA_ARCH CUDA_PATH DOCKER_ENV CARGO_BUILD_FEATURES<br/>declared rerun-if-env-changed by build.rs and crates/visionclaw-gpu/build.rs"]
    C2 --- GLOB
    C2 --- PRUNE
    C6 --- ENVV
    SIG["build_env_signature :88 renders an UNSET variable explicitly as &lt;unset&gt;<br/>so unset and empty stay distinguishable — this is what makes a GPU swap<br/>(CUDA_ARCH 75 to 89) or a feature-set edit visible with no file touched"]
    ENVV --- SIG
    HOLES["the two holes this file closed (:9-13) — the original heuristic globbed only the ROOT<br/>Cargo.toml/Cargo.lock/build.rs so a CRATE manifest edit left a stale binary running,<br/>and globbed *.cu under /app/src only so a crate CUDA kernel edit was missed"]
    GLOB --- HOLES
    DIV["DIVERGENCE (BASELINE 2026-09-04 development acceptance, l.281) — ADR-2008 is PARTIAL.<br/>Normal development startup uses this timestamp-gated wrapper, with demonstrated<br/>misses for crate CUDA and manifest edits."]
    NR --- DIV
```

## VC-08.11 Image builds and the dev-auth exclusion (ADR-2037)
```mermaid
sequenceDiagram
    autonumber
    participant DU as Dockerfile.unified
    participant DP as Dockerfile.production
    participant SD as supervisord.dev.conf
    participant SP as supervisord.production.conf
    participant WR as rust-backend-wrapper.sh
    participant RT as running binary

    rect rgb(232,240,232)
    Note over DU,SD: development image
    DU->>DU: cargo build --release --features gpu (Dockerfile.unified:185 and :208)
    DU->>SD: COPY supervisord.dev.conf ./supervisord.dev.conf (Dockerfile.unified:303)
    SD->>WR: program rust-backend runs the wrapper at container start
    WR->>RT: cargo build --release --features "gpu,ontology,dev-auth" then exec
    Note over WR,RT: dev-auth is added at CONTAINER START by the wrapper's BUILD_FEATURES default,<br/>not at image-build time. The image layer itself carries no dev-auth binary.
    end
    rect rgb(244,236,236)
    Note over DP,SP: production image
    DP->>DP: cargo build --release (Dockerfile.production:153) — NO --features, so no dev-auth
    DP->>SP: COPY supervisord.production.conf (Dockerfile.unified:403)
    DP->>RT: the shipped binary is a production artefact
    Note over RT: ADR-2037 — with dev-auth absent, every bypass codepath is #[cfg]-stripped.<br/>enforce_release_env_hygiene becomes the real impl (src/main.rs:118) rather than the stub (:169)
    end
    RT->>RT: enforce_release_env_hygiene() at src/main.rs:195 — see VC-09.3
    RT->>RT: assert_effective_profile_or_exit() at src/main.rs:873 — see VC-09.4
    Note over RT: ADR-2038 — BuildIdentity::current() reports dev_auth true for a dev-auth artefact, which is<br/>itself the finding DevAuthFeatureInArtefact (src/config/security_profile.rs:271). A dev-auth<br/>binary promoted to production refuses to bind at all.
    Note over DU,RT: RESOLVED ADR-2049 — the warm-up stage used to run cargo build --release || true twice,<br/>which shell precedence made unfailable, so a broken lockfile or an uncompilable dependency<br/>produced a green layer. It now gates on cargo fetch --locked (must succeed) and tolerates<br/>only the crate compile, which legitimately fails against the stub build.rs.
```

## VC-08.12 Degraded-state and boot-receipt observability
```mermaid
sequenceDiagram
    autonumber
    participant B as boot
    participant P as assert_effective_profile_or_exit<br/>src/config/security_profile.rs:528
    participant L as log
    participant R as readiness_probe<br/>src/handlers/consolidated_health_handler.rs
    participant O as operator

    B->>P: evaluate the effective security profile
    P->>L: info "security profile OK — build=X declared=Y classified=Z findings=N"
    Note over P,L: EffectiveProfile::summary() src/config/security_profile.rs:363<br/>main logs it with observed_flags at src/main.rs:879-883 — the boot receipt
    alt production artefact with findings
        P->>O: eprintln FATAL per finding then "refusing to bind a listener (ADR-2038)" then exit(2)
        Note over P,O: the remediation line names the three options — remove the offending variables,<br/>rebuild without --features dev-auth, or set VISIONCLAW_SECURITY_PROFILE to what this really is
    else development build with findings
        P->>L: warn per finding then "N finding(s) but this is a development build — continuing"
    end
    B->>B: serve
    O->>R: GET /readyz
    alt AppState reports a degraded reason
        R-->>O: 503 not_ready with the reason string
    else
        R-->>O: 200 ready
    end
    Note over B,O: the boot receipt is one-shot at start — the degraded reason is the only continuous<br/>readiness signal. There is no endpoint that re-reports the effective profile after boot.
```
