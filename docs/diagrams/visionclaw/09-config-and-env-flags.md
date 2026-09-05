---
id: VC-09
title: Configuration loading, boot-time profile assertion and the environment-flag register
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
  - docs/SECURITY-profiles.md
adrs: [ADR-2012, ADR-2026, ADR-2037, ADR-2038, ADR-2039, ADR-2041, ADR-2043, ADR-2046]
sources:
  - src/main.rs
  - src/app_state.rs
  - src/config/mod.rs
  - src/config/security_profile.rs
  - src/config/path_accessible_impls.rs
  - src/config/feature_access.rs
  - src/services/role_store.rs
  - src/services/github/config.rs
  - src/middleware/rbac_gate.rs
  - src/middleware/public_demo.rs
  - src/handlers/socket_flow_handler/position_updates.rs
  - src/utils/auth.rs
  - docs/adr/ADR-2041-graph-settings-key-knowledge.md
  - client/src/types/generated/settings.ts
  - crates/visionclaw-domain/src/config/visualisation.rs
  - data/settings.yaml
  - src/actors/agent_monitor_actor.rs
  - src/actors/decision_elevation_actor.rs
  - src/actors/elevation_actor.rs
  - src/actors/optimized_settings_actor.rs
  - src/actors/presence_actor.rs
  - src/actors/task_orchestrator_actor.rs
  - src/actors/voice_interface_actor.rs
  - src/agent_events/ingest.rs
  - src/bin/generate_types.rs
  - src/config/dev_config.rs
  - src/config/path_access.rs
  - src/handlers/bots_handler.rs
  - src/handlers/enrichment_proposals_handler.rs
  - src/handlers/fastwebsockets_handler.rs
  - src/handlers/image_gen_handler.rs
  - src/handlers/liveness_harness_handler.rs
  - src/handlers/mcp_relay_handler.rs
  - src/handlers/nostr_handler.rs
  - src/handlers/pay_handler.rs
  - src/handlers/settings_handler/physics.rs
  - src/handlers/socket_flow_handler/http_handler.rs
  - src/handlers/solid_proxy_handler.rs
  - src/services/bots_client.rs
  - src/services/canary_nostr_tap.rs
  - src/services/github_pr_service.rs
  - src/services/github_sync_service.rs
  - src/services/liveness_harness.rs
  - src/services/local_file_sync_service.rs
  - src/services/multi_mcp_agent_discovery.rs
  - src/services/nostr_bead_publisher.rs
  - src/services/nostr_bridge.rs
  - src/services/nostr_service.rs
  - src/services/ontology_class_index.rs
  - src/services/proposal_spine.rs
  - src/services/ragflow_service.rs
  - src/services/speech_service.rs
  - src/services/voice_intent_client.rs
  - src/utils/advanced_logging.rs
  - src/utils/gpu_diagnostics.rs
  - src/utils/unified_gpu_compute/execution.rs
verified_commit: bed6b617d
---

## VC-09.1 Config load precedence — main() boot order
```mermaid
sequenceDiagram
    autonumber
    participant M as main<br/>src/main.rs:172
    participant V as validate_required_env_vars<br/>src/main.rs:60
    participant H as enforce_release_env_hygiene<br/>src/main.rs:118 real / :169 stub
    participant T as telemetry logger<br/>src/main.rs:266
    participant S as AppFullSettings::new<br/>src/main.rs:289
    participant P as assert_effective_profile_or_exit<br/>src/config/security_profile.rs:528
    participant B as HttpServer::new/bind<br/>src/main.rs:893

    Note over M,B: INVARIANT ordering — every refusal runs BEFORE the listener binds
    M->>V: validate_required_env_vars()
    V->>V: required = ["SYSTEM_NETWORK_PORT"] (src/main.rs:62)
    V->>V: recommended = ["MANAGEMENT_API_KEY", "JWT_SECRET", "CORS_ALLOWED_ORIGINS"] (src/main.rs:69) — warn only
    V->>V: is_production = env APP_ENV == "production" (src/main.rs:83, exact match)
    alt missing required AND is_production
        V-->>M: Err("Missing required env vars: ...") — hard fail
    else missing required AND NOT production
        V-->>M: Ok — log::warn per var, continue on defaults
    end
    Note over V: DOC-DRIFT-GUARD src/main.rs:75-81 — is_production is NOT a security toggle<br/>the old case-sensitive APP_ENV security guard was removed as a T2 anti-pattern<br/>(APP_ENV=Production defeated it). Security now lives at the binary level.
    M->>H: enforce_release_env_hygiene() (called src/main.rs:195)
    Note over H: ADR-2026 / ADR-2037 — see VC-09.3
    M->>T: init logger, dir from TELEMETRY_LOG_DIR (src/main.rs:266)
    M->>S: AppFullSettings::new()
    S->>S: file path from SETTINGS_FILE_PATH, default "/app/settings.yaml" (src/main.rs:293)
    Note over S: YAML is snake_case on the wire in, camelCase on the JSON way out<br/>(serde alias, asserted at src/main.rs:296-315)
    alt load fails
        S-->>M: Err — boot aborts
    end
    M->>M: DATA_DIR (src/main.rs:353, src/app_state.rs:448)
    M->>M: BIND_ADDRESS (src/main.rs:797), SYSTEM_NETWORK_PORT (src/main.rs:798)
    M->>P: assert_effective_profile_or_exit(EnvSnapshot::from_process(), BuildIdentity::current(), today) (src/main.rs:873)
    Note over P: ADR-2038 — see VC-09.4. Pure fn over a snapshot plus the UTC date.
    P-->>M: EffectiveProfile — logged as summary + observed_flags (src/main.rs:879-883)
    M->>B: HttpServer::new(...).bind(&bind_address).workers(4)
```

## VC-09.2 src/config module surface — shims into visionclaw-domain, orphans removed
```mermaid
flowchart LR
    subgraph DECL["declared in src/config/mod.rs:1-5 and :57"]
        D1["dev_config<br/>src/config/dev_config.rs"]
        D2["feature_access<br/>src/config/feature_access.rs"]
        D3["path_access<br/>src/config/path_access.rs"]
        D4["security_profile<br/>src/config/security_profile.rs"]
        D5["path_accessible_impls (private)<br/>src/config/path_accessible_impls.rs"]
        D6["inline pub mod physics<br/>src/config/mod.rs:57"]
    end
    subgraph REEXPORT["re-export shims — src/config/mod.rs:21-56"]
        R1["graph_type<br/>normalise_graph_type, knowledge_graph_value"]
        R2["validation<br/>validate_hex_color, validate_port, ..."]
        R3["visualisation<br/>GraphsSettings, NodeSettings, ..."]
        R4["system<br/>SystemSettings, NetworkSettings, ..."]
        R5["xr<br/>XRSettings, MovementAxes"]
        R6["services<br/>AuthSettings, RagFlowSettings, ..."]
        R7["AppFullSettings, DeveloperConfig<br/>FeatureFlags, UserPreferences"]
    end
    DOM["crates/visionclaw-domain/src/config/*"]
    subgraph ORPHAN["REMOVED ADR-2046 — were on disk but never declared as modules"]
        O1["src/config/field_mappings.rs"]
        O2["src/config/physics.rs"]
        O3["src/config/services.rs"]
        O4["src/config/system.rs"]
        O5["src/config/validation.rs"]
        O6["src/config/xr.rs"]
    end
    DECL --> DOM
    REEXPORT --> DOM
    D6 -->|"shadows"| O2
    ORPHAN -.->|"were unreachable — no mod declaration"| DECL
    N1["RESOLVED ADR-2046 — src/config/mod.rs declares only the five modules above plus the<br/>inline physics module, so the six files in ORPHAN were unreachable dead copies of types<br/>canonical in visionclaw-domain. ADR-2041 Context said the same. All six are now deleted.<br/>Note the inline pub mod physics at :57 SHADOWED physics.rs — that is why deleting the<br/>file changed nothing. Evidence src/config/mod.rs:1-5 and :57"]
    ORPHAN --- N1
```

## VC-09.3 enforce_release_env_hygiene — argv and env refusal (ADR-2026, ADR-2037)
```mermaid
sequenceDiagram
    autonumber
    participant M as main<br/>src/main.rs:195
    participant H as enforce_release_env_hygiene<br/>src/main.rs:118
    participant OS as process env + argv

    Note over H: cfg not(any(debug_assertions, feature="dev-auth")) — dev builds get the no-op stub at src/main.rs:169
    M->>H: enforce_release_env_hygiene()
    H->>OS: std::env::args()
    alt argv contains "--allow-skip-auth" (src/main.rs:120)
        H-->>OS: eprintln FATAL then std::process::exit(1)
        Note over H: ADR-2026 argv refusal — exit code 1
    end
    H->>OS: read SUSPECT_ENVS (src/main.rs:130-134)
    Note over H,OS: SUSPECT_ENVS = SETTINGS_AUTH_BYPASS, ALLOW_INSECURE_DEFAULTS, VISIONCLAW_DEV_MODE
    loop for each SUSPECT_ENV
        alt var is present (any value)
            H->>H: offending.push(var)
        end
    end
    H->>OS: NODE_ENV (src/main.rs:141, eq_ignore_ascii_case "development")
    H->>OS: DOCKER_ENV is_ok (src/main.rs:144)
    alt NODE_ENV=development AND DOCKER_ENV set
        H->>H: offending.push("NODE_ENV=development+DOCKER_ENV")
    end
    alt offending non-empty
        H-->>OS: eprintln FATAL per var then std::process::exit(2) (src/main.rs:156)
        Note over H: presence is the signal, not truthiness — a release binary has no bypass<br/>codepath at all, so the var can only mean a dev config was promoted
    else clean
        H-->>M: return
    end
```

## VC-09.4 Boot-time effective-profile assertion (ADR-2038) — the pure evaluator
```mermaid
sequenceDiagram
    autonumber
    participant M as main<br/>src/main.rs:868-878
    participant E as EnvSnapshot::from_process<br/>src/config/security_profile.rs:169
    participant BI as BuildIdentity::current<br/>src/config/security_profile.rs:227
    participant A as assert_effective_profile_or_exit<br/>src/config/security_profile.rs:528
    participant V as evaluate_effective_profile<br/>src/config/security_profile.rs:422

    Note over M,V: ADR-2038 closes ADR-2012 / ADR-2026 / ADR-2027 / ADR-2037<br/>runs BEFORE HttpServer::bind at src/main.rs:893
    M->>E: from_process() — vars + argv snapshot taken once
    M->>BI: current() — debug_assertions, dev_auth
    M->>M: today = chrono::Utc::now().format("%Y-%m-%d") (src/main.rs:872)
    M->>A: assert_effective_profile_or_exit(env, build, today)
    A->>V: evaluate_effective_profile(env, build, today)
    Note over V: pure — reads no process env and no clock of its own
    V->>V: 1. FORBIDDEN_DEV_VARS presence scan (src/config/security_profile.rs:60-66)
    Note over V: SETTINGS_AUTH_BYPASS, ALLOW_INSECURE_DEFAULTS, VISIONCLAW_DEV_MODE, DEV_AUTH_LOOPBACK<br/>presence not truthiness — SETTINGS_AUTH_BYPASS=0 is still a finding
    V->>V: 2. NODE_ENV=development + DOCKER_ENV → DevelopmentNodeEnvInContainer
    V->>V: 3. argv --allow-skip-auth → AllowSkipAuthArgv
    V->>V: 4. build carries dev-auth → DevAuthFeatureInArtefact (ADR-2037)
    V->>V: 5. report_mode_requested(env) (src/config/security_profile.rs:396)
    alt public_reads_enabled_in(env) AND NOT visibility_filter_enabled_in(env)
        V->>V: 5b. findings.push(FullDisclosureFlagPair) — ADR-2043
        Note over V: RESOLVED ADR-2043 — the ADR-2003 illegal pair, rejected unconditionally.<br/>public_reads_enabled_in mirrors rbac_gate::public_reads_enabled (only 1/true enable)<br/>visibility_filter_enabled_in mirrors position_updates::parse_visibility_flag (default ON,<br/>only 0/false/off/no disable). Duplicated against the snapshot, not called live, because<br/>the gate reads the process env while the assertion reads the boot snapshot.
    end
    V->>V: 6. classify observed PROFILE_FLAGS vs declared VISIONCLAW_SECURITY_PROFILE
    V-->>A: EffectiveProfile { build, declared, classified, observed_flags, findings }
    alt findings empty
        A-->>M: log::info "security profile OK" then return
    else production artefact (build.is_production_artefact())
        A-->>M: eprintln FATAL per finding then "refusing to bind a listener" then std::process::exit(2)
        Note over A: ADR-2038 — a mis-promoted image never accepts a single request
    else development build
        A-->>M: log::warn per finding then continue
        Note over A: may_bind_listener() src/config/security_profile.rs:358 — dev always may
    end
```

## VC-09.5 ProfileFinding and EffectiveProfile — the rejection vocabulary
```mermaid
classDiagram
    class EffectiveProfile {
      +BuildIdentity build
      +Option~DeploymentProfile~ declared
      +Option~DeploymentProfile~ classified
      +BTreeMap~String_OptionString~ observed_flags
      +Vec~ProfileFinding~ findings
      +may_bind_listener() bool
      +summary() String
    }
    class BuildIdentity {
      +bool debug_assertions
      +bool dev_auth
      +label() str
      +is_production_artefact() bool
    }
    class EnvSnapshot {
      -BTreeMap~String_String~ vars
      -Vec~String~ argv
      +from_process() EnvSnapshot
      +from_pairs() EnvSnapshot
      +with_argv() EnvSnapshot
      +get(name) Option~str~
      +is_present(name) bool
    }
    class ProfileFinding {
      <<enum>>
      ForbiddenDevVariable
      DevelopmentNodeEnvInContainer
      AllowSkipAuthArgv
      DevAuthFeatureInArtefact
      ReportModeRequested
      ProfileDrift
      UnknownDeclaredProfile
      FullDisclosureFlagPair
    }
    class ForbiddenDevVariable {
      +String name
      +String value
    }
    class ReportModeRequested {
      +bool acknowledged
      +Option~String~ ack_value
      +String today
    }
    class ProfileDrift {
      +DeploymentProfile profile
      +String flag
      +String expected
      +Option~String~ observed
    }
    class UnknownDeclaredProfile {
      +String declared
    }
    class FullDisclosureFlagPair {
      +String public_reads
      +String visibility_filter
    }
    class DeploymentProfile {
      <<enum>>
      DemoOpen
      SingleTenant
      MultiUserLocked
      +as_str() str
      +parse(s) Option
      +expected_flags() BTreeMap
    }
    class FlagExpectation {
      <<enum>>
      Unset
      AnyNonEmpty
      Exactly
    }
    EffectiveProfile *-- BuildIdentity
    EffectiveProfile *-- ProfileFinding
    EffectiveProfile ..> DeploymentProfile
    ProfileFinding <|-- ForbiddenDevVariable
    ProfileFinding <|-- ReportModeRequested
    ProfileFinding <|-- ProfileDrift
    ProfileFinding <|-- UnknownDeclaredProfile
    ProfileFinding <|-- FullDisclosureFlagPair
    DeploymentProfile ..> FlagExpectation
    EnvSnapshot ..> EffectiveProfile
```

## VC-09.6 DeploymentProfile expected-flag matrix (ADR-2027 as coded)
```mermaid
flowchart TB
    subgraph DO["demo-open — src/config/security_profile.rs:122"]
        DO1["RBAC_PUBLIC_READS = Exactly 1"]
        DO2["RBAC_ALLOW_OWNERLESS = Exactly 1"]
        DO3["RBAC_OWNER_PUBKEY = Unset"]
        DO4["RBAC_DEFAULT_ROLE = Exactly editor"]
    end
    subgraph ST["single-tenant — src/config/security_profile.rs:123"]
        ST1["RBAC_PUBLIC_READS = Exactly 0"]
        ST2["RBAC_ALLOW_OWNERLESS = Exactly 1"]
        ST3["RBAC_OWNER_PUBKEY = AnyNonEmpty"]
        ST4["RBAC_DEFAULT_ROLE = Exactly editor"]
    end
    subgraph ML["multi-user-locked — src/config/security_profile.rs:124-126"]
        ML1["RBAC_PUBLIC_READS = Exactly 0"]
        ML2["RBAC_ALLOW_OWNERLESS = Exactly 0"]
        ML3["RBAC_OWNER_PUBKEY = AnyNonEmpty"]
        ML4["RBAC_DEFAULT_ROLE = Exactly viewer"]
    end
    COMMON["required by ALL three — src/config/security_profile.rs:133-134<br/>PUBKEY_VISIBILITY_FILTER = Exactly 1<br/>RBAC_GATE_MODE = Exactly enforce (unset also satisfies enforce, the code default)"]
    SEL["VISIONCLAW_SECURITY_PROFILE<br/>src/config/security_profile.rs:55<br/>parse tolerates case and _ vs - (:107)"]
    SEL --> DO
    SEL --> ST
    SEL --> ML
    DO --> COMMON
    ST --> COMMON
    ML --> COMMON
    DRIFT["declared but a flag mismatches → ProfileFinding::ProfileDrift<br/>declared unparseable → UnknownDeclaredProfile<br/>undeclared and matching none → classified None, reported Unnamed (ADR-2027 unsupported)<br/>RESOLVED ADR-2043 — RBAC_PUBLIC_READS=1 with PUBKEY_VISIBILITY_FILTER=0 is now<br/>rejected on its own terms, unconditionally, declared profile or not. Previously it<br/>raised ProfileDrift only when a profile WAS declared, so the careless deployment<br/>was the one that got through. PROPOSED — the wider Unnamed-is-not-fatal gap stays<br/>open: ADR-2038 says undeclared production defaults to multi-user-locked, code binds."]
    COMMON --> DRIFT
    NOTE["RESOLVED ADR-2043 — the comment above PROFILE_FLAGS said four composable flags<br/>over a [&str; 6] array. Traced with the estate lead to a stale count in ADR-2027,<br/>corrected at the root there and in the code comment. The array was always six."]
    COMMON --- NOTE
```

## VC-09.7 RBAC report mode — the dated acknowledgement (ADR-2012)
```mermaid
sequenceDiagram
    autonumber
    participant C as caller<br/>src/config/security_profile.rs:422
    participant R as report_mode_requested<br/>src/config/security_profile.rs:396
    participant K as report_mode_acknowledged<br/>src/config/security_profile.rs:410
    participant G as RbacGate::from_env<br/>src/middleware/rbac_gate.rs:80-113

    Note over C,G: single implementation shared with the gate so the two cannot drift
    C->>R: report_mode_requested(env)
    R->>R: RBAC_GATE_MODE trimmed, eq_ignore_ascii_case "report"
    alt not requested
        R-->>C: false — mode is enforce (the code default)
    else requested
        R-->>C: true
        C->>K: report_mode_acknowledged(env, build, today)
        alt build.debug_assertions
            K-->>C: true — a debug build acknowledges implicitly
        else RBAC_REPORT_MODE_ACK trimmed == today exactly
            K-->>C: true
            Note over K: the acknowledgement expires at the UTC date rollover
        else
            K-->>C: false
        end
        C->>C: findings.push(ReportModeRequested { acknowledged, ack_value, today })
        Note over C: ADR-2012 gap closed — a dated ack makes report mode AUDITABLE,<br/>never ACCEPTABLE in production. Fatal either way in a production artefact.
    end
    Note over G: the gate reads the same pair at construction — see VC-03 for the request-time behaviour
```

## VC-09.8 Env register — boot, process identity and storage
```mermaid
flowchart TB
    subgraph REQ["required — hard-fail in production"]
        E1["SYSTEM_NETWORK_PORT<br/>required list src/main.rs:62 · read :798"]
    end
    subgraph RECO["recommended — warn only, src/main.rs:69"]
        E2["MANAGEMENT_API_KEY<br/>src/app_state.rs:82 · src/main.rs:689"] --> E3["JWT_SECRET<br/>src/app_state.rs:116"] --> E4["CORS_ALLOWED_ORIGINS<br/>src/main.rs:902"]
    end
    subgraph IDENT["process identity — gates the release refusal"]
        E5["APP_ENV<br/>unset = non-production<br/>src/main.rs:85 and :928"] --> E6["NODE_ENV<br/>src/main.rs:141"] --> E7["DOCKER_ENV<br/>src/main.rs:144"] --> E8["VISIONCLAW_GIT_SHA<br/>src/services/liveness_harness.rs:279"]
    end
    subgraph PATHS["paths, stores and logging"]
        E9["DATA_DIR<br/>src/main.rs:353 · src/app_state.rs:448"] --> E10["SETTINGS_FILE_PATH<br/>default /app/settings.yaml<br/>src/main.rs:293"] --> E11["EVENT_STORE_PATH<br/>src/app_state.rs:892"] --> E12["LOG_DIR<br/>src/utils/advanced_logging.rs:570"] --> E13["TELEMETRY_LOG_DIR<br/>src/main.rs:266"] --> E14["DEBUG_ENABLED<br/>src/utils/advanced_logging.rs:643"]
    end
    subgraph BIND["listener"]
        E15["BIND_ADDRESS<br/>src/main.rs:797"] --> E16["ALLOWED_WS_ORIGINS<br/>src/handlers/fastwebsockets_handler.rs:185"]
    end
    REQ --> RECO --> IDENT --> PATHS --> BIND
    N["branch effects — APP_ENV=production makes a missing required var fatal<br/>src/main.rs:83-92. NODE_ENV=development plus DOCKER_ENV is a<br/>release-build boot refusal, src/main.rs:141-148"]
    IDENT --- N
```


## VC-09.9 Env register — security and authorisation
```mermaid
flowchart TB
    subgraph RBAC["RBAC lattice — request-time behaviour see VC-03"]
        S1["RBAC_PUBLIC_READS<br/>default OFF, fail-closed unwrap_or(false)<br/>src/middleware/rbac_gate.rs:121-128, read :123"] --> S2["RBAC_ALLOW_OWNERLESS<br/>const src/services/role_store.rs:33<br/>read src/main.rs:732 — absence refuses boot"] --> S3["RBAC_OWNER_PUBKEY<br/>const src/services/role_store.rs:27, read :642"] --> S4["RBAC_DEFAULT_ROLE<br/>const src/services/role_store.rs:41, read :218<br/>default Editor, fail-closed to viewer"] --> S5["RBAC_GATE_MODE<br/>default enforce<br/>src/middleware/rbac_gate.rs:80-113"] --> S6["RBAC_REPORT_MODE_ACK<br/>must equal today exactly<br/>src/config/security_profile.rs:410"] --> S7["POWER_USER_PUBKEYS<br/>src/services/nostr_service.rs:125<br/>maps to Admin when unassigned"]
    end
    subgraph BYPASS["dev bypass — presence refused in release"]
        S8["VISIONCLAW_DEV_MODE<br/>src/utils/auth.rs:100 dev_full_bypass_active"] --> S9["DEV_AUTH_LOOPBACK<br/>src/utils/auth.rs:123"] --> S10["SETTINGS_AUTH_BYPASS<br/>presence only — src/main.rs:130-134"] --> S11["ALLOW_INSECURE_DEFAULTS<br/>src/main.rs:906 · src/agent_events/ingest.rs:55<br/>src/handlers/socket_flow_handler/http_handler.rs:21"]
    end
    subgraph POSTURE["posture selectors"]
        S12["VISIONCLAW_SECURITY_PROFILE<br/>src/config/security_profile.rs:55"] --> S13["PUBLIC_DEMO<br/>const src/middleware/public_demo.rs:24, read :28"] --> S14["PUBKEY_VISIBILITY_FILTER<br/>default ON, read ONCE and cached<br/>position_updates.rs:26 :34-43 :50-58"]
    end
    subgraph SESS["session realm"]
        S15["AUTH_TOKEN_EXPIRY<br/>src/handlers/nostr_handler.rs:134 :234<br/>src/services/nostr_service.rs:131"] --> S16["REDIS_URL<br/>src/services/nostr_service.rs:140<br/>src/actors/optimized_settings_actor.rs:146"]
    end
    RBAC --> BYPASS --> POSTURE --> SESS
    N2["FORBIDDEN_DEV_VARS src/config/security_profile.rs:60-66 = the four in BYPASS<br/>PROFILE_FLAGS src/config/security_profile.rs:68-76 = S1 S2 S3 S4 S14 S5"]
    POSTURE --- N2
    N3["a release build refuses to boot on the mere PRESENCE of a BYPASS var — exit(2)<br/>see VC-09.3 and VC-09.4"]
    BYPASS --- N3
```


## VC-09.10 Env register — Nostr, ACSP and canary taps
```mermaid
flowchart TB
    subgraph KEYS["signing keys"]
        K1["VISIONCLAW_NOSTR_PRIVKEY<br/>src/services/nostr_bridge.rs:37<br/>src/services/nostr_bead_publisher.rs:40<br/>src/app_state.rs:1366<br/>src/actors/elevation_actor.rs:182<br/>src/actors/decision_elevation_actor.rs:128"] --> K2["ACSP_PANEL_NOSTR_PRIVKEY<br/>src/app_state.rs:1365<br/>src/actors/elevation_actor.rs:181<br/>src/actors/decision_elevation_actor.rs:127<br/>src/services/voice_intent_client.rs:152"] --> K3["VISIONCLAW_AGENT_KEY<br/>src/handlers/enrichment_proposals_handler.rs:132<br/>src/handlers/image_gen_handler.rs:46<br/>src/handlers/liveness_harness_handler.rs:36"]
    end
    subgraph RELAYS["relays and taps"]
        R1["FORUM_RELAY_URL<br/>src/services/nostr_bridge.rs:40 · src/app_state.rs:1364<br/>src/actors/elevation_actor.rs:180<br/>src/actors/decision_elevation_actor.rs:126"] --> R2["NOSTR_RELAY_URL<br/>src/services/nostr_bridge.rs:44<br/>src/services/nostr_bead_publisher.rs:44"] --> R3["CANARY_TAP_RELAY_URL<br/>unset = tap not started<br/>src/services/canary_nostr_tap.rs:246"] --> R4["CANARY_TAP_ALLOWED_PUBKEYS<br/>src/services/canary_nostr_tap.rs:256"]
    end
    subgraph GATES["actor and envelope gates"]
        G1["ELEVATION_ACTOR_ENABLED<br/>src/actors/elevation_actor.rs:173"] --> G2["DECISION_ELEVATION_ENABLED<br/>src/actors/decision_elevation_actor.rs:119"] --> G3["ONTOLOGY_REQUIRE_SIGNED_ENVELOPE<br/>src/services/proposal_spine.rs:411"]
    end
    KEYS --> RELAYS --> GATES
    N3["DIVERGENCE — NIP-26 delegation is not wired. nostr_bridge.rs re-signs under<br/>the bridge key (src/services/nostr_bridge.rs:29 field, :65 Keys::new,<br/>:169 sign_with_keys) — service-signing, not delegation. See VC-03."]
    KEYS --- N3
```


## VC-09.11 Env register — GitHub corpus sync
```mermaid
flowchart TB
    subgraph TOK["tokens"]
        T1["PRIVATE_REPO_GITHUB_PAT<br/>const GITHUB_TOKEN_ENV src/services/github/config.rs:23<br/>read :31"] --> T2["legacy token alias<br/>GITHUB_TOKEN_ENV_LEGACY src/services/github/config.rs:33"]
    end
    subgraph LOC["repository coordinates"]
        L1["GITHUB_OWNER<br/>src/services/github/config.rs:88<br/>src/services/github_pr_service.rs:129<br/>src/services/local_file_sync_service.rs:370"] --> L2["GITHUB_REPO<br/>src/services/github/config.rs:91<br/>src/services/github_pr_service.rs:135<br/>src/services/local_file_sync_service.rs:373"] --> L3["GITHUB_BRANCH<br/>src/services/github/config.rs:113<br/>src/services/github_pr_service.rs:141<br/>src/services/local_file_sync_service.rs:374"] --> L4["GITHUB_BASE_PATH<br/>src/services/github/config.rs:99<br/>src/services/github_sync_service.rs:2179<br/>src/services/local_file_sync_service.rs:376"] --> L5["GITHUB_BASE_PATHS<br/>src/services/github/config.rs:98<br/>src/services/github_sync_service.rs:2178"] --> L6["GITHUB_REPO_OWNER :130 · GITHUB_REPO_NAME :136<br/>GITHUB_BASE_BRANCH :142<br/>all in src/services/github_pr_service.rs"]
    end
    subgraph BEHAV["sync behaviour"]
        B1["FORCE_FULL_SYNC — bypasses the SHA1 incremental filter<br/>src/services/github_sync_service.rs:325"] --> B2["FANOUT_NODE_THRESHOLD<br/>src/services/github_sync_service.rs:410 and :671"] --> B3["GITHUB_RATE_LIMIT<br/>src/services/github/config.rs:115"] --> B4["GITHUB_API_VERSION<br/>src/services/github/config.rs:119"]
    end
    TOK --> LOC --> BEHAV
    N4["DIVERGENCE — GITHUB_REPO_OWNER, GITHUB_REPO_NAME and GITHUB_BASE_BRANCH are read<br/>only by github_pr_service.rs and duplicate GITHUB_OWNER, GITHUB_REPO and GITHUB_BRANCH<br/>read elsewhere — two naming grammars for the same coordinates, unreconciled"]
    LOC --- N4
    N5["corpus ingest pipeline see VC-21"]
    BEHAV --- N5
```


## VC-09.12 Env register — MCP, agents and the management plane
```mermaid
flowchart TB
    subgraph MCP["MCP transport"]
        M1["MCP_HOST<br/>src/app_state.rs:1180 · src/services/bots_client.rs:116<br/>src/services/speech_service.rs:1145<br/>src/services/ontology_class_index.rs:88<br/>analytics/anomaly_handlers.rs:101<br/>analytics/clustering_handlers.rs:346"] --> M2["MCP_TCP_PORT<br/>src/app_state.rs:1181 · src/services/bots_client.rs:118<br/>src/services/multi_mcp_agent_discovery.rs:92<br/>src/services/speech_service.rs:1146<br/>src/services/ontology_class_index.rs:89<br/>plus the two analytics handlers above"] --> M3["CLAUDE_FLOW_HOST<br/>src/services/bots_client.rs:115<br/>src/services/multi_mcp_agent_discovery.rs:91"]
    end
    subgraph MGMT["management API"]
        G1["MANAGEMENT_API_HOST<br/>src/main.rs:684 · src/app_state.rs:1262<br/>src/actors/agent_monitor_actor.rs:209"] --> G2["MANAGEMENT_API_PORT<br/>src/main.rs:685 · src/app_state.rs:1264<br/>src/actors/agent_monitor_actor.rs:211"] --> G3["MANAGEMENT_API_KEY<br/>src/main.rs:689 · src/app_state.rs:82<br/>src/actors/agent_monitor_actor.rs:216"]
    end
    subgraph DISC["swarm discovery"]
        D1["DAA_HOST :125 · DAA_PORT :126<br/>src/services/multi_mcp_agent_discovery.rs"] --> D2["RUV_SWARM_HOST :108 · RUV_SWARM_PORT :109<br/>src/services/multi_mcp_agent_discovery.rs"] --> D3["ORCHESTRATOR_WS_URL<br/>src/handlers/mcp_relay_handler.rs:77"]
    end
    subgraph AGENTS["agent behaviour and agentbox bridge"]
        A1["MAX_CONCURRENT_TASKS<br/>src/actors/task_orchestrator_actor.rs:68"] --> A2["MOCK_AGENTS<br/>src/actors/agent_monitor_actor.rs:495"] --> A3["AGENTBOX_MANAGEMENT_URL :147<br/>AGENTBOX_VOICE_INTENT_URL :143<br/>VISIONCLAW_VOICE_ACTOR_LABEL :162<br/>all in src/services/voice_intent_client.rs"]
    end
    MCP --> MGMT --> DISC --> AGENTS
    N["agent integration and the MCP relay see VC-27 — the agentbox side is the agentbox area"]
    AGENTS --- N
```


## VC-09.13 Env register — external services, payment and Solid
```mermaid
flowchart TB
    subgraph EXT["external inference and RAG"]
        X1["RAGFLOW_API_KEY :107 · RAGFLOW_API_BASE_URL :118<br/>RAGFLOW_AGENT_ID :129<br/>all in src/services/ragflow_service.rs"] --> X2["COMFYUI_URL src/handlers/image_gen_handler.rs:31<br/>COMFYUI_SALAD_URL :36"] --> X3["PRIMARY_PROVIDER<br/>src/actors/voice_interface_actor.rs:161<br/>src/handlers/bots_handler.rs:301 :408 :541 :724"]
    end
    subgraph PAY["HTTP 402 payment — feature solid-pod-embed"]
        P1["PAY_ENABLED<br/>src/handlers/pay_handler.rs:95 and :966<br/>routes inert until true"] --> P2["PAY_COST_SATS :98 :967 · PAY_INFERENCE_COST_SATS :106<br/>PAY_IMAGE_GEN_COST_SATS :110 · PAY_ANALYTICS_COST_SATS :114<br/>all in src/handlers/pay_handler.rs"] --> P3["PAY_LEDGER_DIR<br/>src/handlers/pay_handler.rs:102 and :968"]
    end
    subgraph SOLID["Solid pod proxy"]
        O1["SOLID_DATA_ROOT :122 · SOLID_PROXY_SECRET_KEY :125<br/>SOLID_ALLOW_ANONYMOUS :129<br/>all in src/handlers/solid_proxy_handler.rs"] --> O2["SOLID_INTERNAL_URL<br/>src/handlers/image_gen_handler.rs:41"]
    end
    subgraph SELF["self-reference and liveness"]
        F1["VISIONCLAW_SELF_URL<br/>default http 127.0.0.1 port<br/>src/main.rs:1181"] --> F2["VISIONCLAW_KG_WATCHDOG_SECS<br/>default 30 · src/main.rs:1183"] --> F3["VISIONCLAW_INTERNAL_URL<br/>src/actors/voice_interface_actor.rs:159<br/>src/handlers/bots_handler.rs:522"]
    end
    EXT --> PAY --> SOLID --> SELF
    N5["PAY routes are mounted UNCONDITIONALLY under feature solid-pod-embed<br/>at src/main.rs:1017-1027 and stay inert until PAY_ENABLED=true —<br/>.info reports disabled, gated routes 403. See VC-04."]
    PAY --- N5
```


## VC-09.14 Env register — GPU, ontology index and XR presence
```mermaid
flowchart LR
    subgraph GPU["GPU runtime"]
        U1["CUDA_VISIBLE_DEVICES<br/>src/utils/gpu_diagnostics.rs:316"]
        U2["VISIONCLAW_PTX_PATH<br/>src/utils/gpu_diagnostics.rs:86"]
        U3["VISIONCLAW_BLOCK_SIZE<br/>src/utils/unified_gpu_compute/execution.rs:123"]
    end
    subgraph ONT["ontology class index"]
        C1["ONTOLOGY_CLASS_INDEX_ENABLED<br/>src/services/ontology_class_index.rs:75"]
        C2["ONTOLOGY_CLASS_INDEX_NAMESPACE<br/>src/services/ontology_class_index.rs:82"]
        C3["ONTOLOGY_CLASS_INDEX_CANARY_QUERY<br/>src/services/ontology_class_index.rs:94"]
    end
    subgraph XR["XR presence"]
        Y1["PRESENCE_HAND_REACH_M<br/>src/actors/presence_actor.rs:46, :1060"]
    end
    subgraph DYN["dynamic reads — name not a literal"]
        Z1["src/config/feature_access.rs:30<br/>env::var(var_name) — feature-gate name resolved at runtime"]
        Z2["src/main.rs:65 and :72<br/>env::var(var) over the required and recommended lists"]
        Z3["src/utils/gpu_diagnostics.rs:129<br/>env::var(var) over a diagnostic list"]
    end
    GPU --> ONT --> XR
    DYN -.->|"invisible to a literal grep"| GPU
    N6["GPU internals see VC-10. The register above lists only the env surface."]
    GPU --- N6
```

## VC-09.15 ADR-2041 — the knowledge graph-settings key and its one-release logseq alias
```mermaid
sequenceDiagram
    autonumber
    participant Y as data/settings.yaml
    participant D as GraphsSettings<br/>crates/visionclaw-domain/src/config/visualisation.rs
    participant P as path_accessible_impls<br/>src/config/path_accessible_impls.rs:159
    participant G as generate_types<br/>src/bin/generate_types.rs
    participant C as client generated types<br/>client/src/types/generated/settings.ts

    Note over Y,C: ADR-2041 decision_status proposed, implementation_status complete, activation_status staged
    Y->>D: deserialise graphs.knowledge
    alt persisted key is the legacy "logseq"
        Y->>D: serde alias logseq accepted on the way in
        D-->>Y: serialisation always emits "knowledge"
    end
    P->>P: match segment "knowledge" | "logseq" (src/config/path_accessible_impls.rs:160 and :185)
    Note over P: both path segments resolve to the same field — read-only alias for ONE release
    D->>G: settings schema
    G->>C: emit "knowledge" only
    Note over C: client GraphType becomes knowledge | visionclaw<br/>a store migration maps a persisted graphs.logseq object to graphs.knowledge on load
    Note over P,C: DIVERGENCE — review_trigger is the release after ADR-2040's tolerance ends<br/>remove the alias and the client migration shim then. Settings round-trip detail see VC-06.
```
