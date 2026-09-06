---
id: VC-06
title: Settings round trip — REST, actors, SQLite adapter and generated client types
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2005, ADR-2011, ADR-2041, ADR-2046, ADR-2047, ADR-2080]
sources:
  - src/settings/api/settings_routes.rs
  - src/settings/auth_extractor.rs
  - src/settings/mod.rs
  - src/settings/models.rs
  - src/actors/optimized_settings_actor.rs
  - src/actors/protected_settings_actor.rs
  - src/adapters/sqlite_settings_repository.rs
  - src/ports/settings_repository.rs
  - src/ports/mod.rs
  - src/handlers/settings_validation_fix.rs
  - src/config/mod.rs
  - src/config/path_accessible_impls.rs
  - src/bin/generate_types.rs
  - src/app_state.rs
  - src/main.rs
  - src/handlers/tests/mod.rs
  - docs/adr/ADR-2041-graph-settings-key-knowledge.md
  - client/src/types/generated/settings.ts
  - src/handlers/nostr_handler.rs
  - src/middleware/rbac_gate.rs
verified_commit: 7a20db228
---

## VC-06.1 Settings route surface and the actor behind it
```mermaid
flowchart TB
    S["web::scope('/settings') + RateLimit::per_minute(60)<br/>src/main.rs:1063-1067"]
    S --> CFG["settings::api::configure_routes<br/>src/settings/api/settings_routes.rs:1712"]
    CFG --> P["GET|PUT physics :1713-1714<br/>POST physics/reset-layout :1715"]
    CFG --> C["GET|PUT constraints :1716-1717"]
    CFG --> R["GET|PUT rendering :1718-1719"]
    CFG --> NF["GET|PUT node-filter :1720-1721"]
    CFG --> QG["GET|PUT quality-gates :1722-1723"]
    CFG --> V["GET|PUT visual :1724-1725"]
    CFG --> AL["GET all :1726"]
    CFG --> PR["POST|GET profiles :1727-1728<br/>GET|DELETE profiles/{id} :1729-1730"]
    CFG --> U["nested scope /user<br/>GET|PUT /filter :1734-1736"]
    ACT["state.settings_addr : Addr of OptimizedSettingsActor<br/>src/app_state.rs:355, started src/app_state.rs:1175"]
    REPO["settings_repo : web::Data of Arc of SqliteSettingsRepository<br/>injected src/main.rs:1008"]
    P --> ACT
    P --> REPO
    N1["INVARIANT — /api/settings writes require the WriteSettings capability at the RbacGate<br/>src/main.rs:1054-1056. Request-time gate behaviour see VC-03.6"]
    S --- N1
    N2["DIVERGENCE — the settings hot-reload watcher is DISABLED, src/app_state.rs:1179-1181<br/>reason recorded in code: it was causing database deadlocks"]
    ACT --- N2
```

## VC-06.2 PUT /api/settings/physics — the flagship round trip, phase 1 read and merge
```mermaid
sequenceDiagram
    autonumber
    participant CL as client
    participant RG as RbacGate<br/>src/middleware/rbac_gate.rs
    participant H as update_physics_settings<br/>src/settings/api/settings_routes.rs:469
    participant AU as AuthenticatedUser<br/>src/settings/auth_extractor.rs
    participant SA as OptimizedSettingsActor<br/>src/actors/optimized_settings_actor.rs:778
    participant VA as validate_physics_settings<br/>src/settings/api/settings_routes.rs

    CL->>RG: PUT /api/settings/physics
    Note over RG: WriteSettings capability required — see VC-03.6
    RG->>H: forward
    H->>AU: extract AuthenticatedUser (auth.pubkey logged at :475-478)
    H->>SA: send(GetSettings)
    Note over H,SA: single GetSettings call — a full snapshot is fetched ONCE to avoid a TOCTOU race<br/>comment src/settings/api/settings_routes.rs:482
    alt Ok(Ok(settings))
        SA-->>H: AppFullSettings
    else Ok(Err(e))
        SA-->>H: 500 "Failed to fetch current settings"
    else Err(mailbox)
        SA-->>H: 500 "Actor communication error"
    end
    H->>H: current = full_settings.visualisation.graphs.knowledge.physics (:493)
    Note over H: ADR-2041 — the field is `knowledge`, `logseq` is a deserialisation alias only. See VC-09.15
    H->>H: normalize_physics_keys(patch) — snake_case and legacy aliases to canonical camelCase (:504-506)
    H->>H: merge patch onto the snapshot then serde_json::from_value::<PhysicsSettings>
    alt merge fails
        H-->>CL: 400 "Invalid settings value"
    end
    H->>VA: validate_physics_settings(&new_physics) (:524)
    alt validation fails
        VA-->>CL: 400 "Validation failed: ..."
    end
    Note over H,VA: continues in VC-06.3
```

## VC-06.3 PUT /api/settings/physics — phase 2 write-back, GPU propagation, persistence
```mermaid
sequenceDiagram
    autonumber
    participant H as update_physics_settings<br/>src/settings/api/settings_routes.rs:469
    participant SA as OptimizedSettingsActor<br/>src/actors/optimized_settings_actor.rs:805
    participant GPU as GPUComputeActor<br/>via state.get_gpu_compute_addr()
    participant GM as GPUManagerActor<br/>state.gpu_manager_addr
    participant GS as GraphServiceSupervisor<br/>state.graph_service_addr
    participant DB as SqliteSettingsRepository<br/>src/adapters/sqlite_settings_repository.rs:415

    H->>H: full_settings.visualisation.graphs.knowledge.physics = new_physics (:533)
    H->>SA: send(UpdateSettings { settings: full_settings })
    alt Ok(Ok(()))
        H->>H: SimulationParams::from(&new_physics) (:546)
        rect rgb(236,236,244)
        Note over H,GM: GPU propagation with a startup fallback
        alt state.get_gpu_compute_addr() is Some
            H->>GPU: send(UpdateSimulationParams { params })
        else direct address not yet cached
            Note over H,GM: fallback window is roughly the first 6s of startup while the async init task completes (:562-564)
            alt state.gpu_manager_addr is Some
                H->>GM: send(UpdateSimulationParams) — routed
            else
                H->>H: error — physics will NOT propagate to GPU
            end
        end
        opt GPUComputeActor address available
            H->>GPU: do_send(UpdateClusteringParams { algorithm, resolution, iterations })
            Note over H,GPU: community-detector params cannot ride in the 172-byte repr-C SimParams<br/>so they are dispatched separately — this is what makes the Physics tab controls live (:579-584)
        end
        end
        H->>GS: send(UpdateSimulationParams)
        H->>GS: send(ForceResumePhysics { reason: "Physics settings updated via API" }) (:601-605)
        Note over GS: without ForceResumePhysics a converged system stays paused and the change is invisible (:598-599)
        H->>DB: set_setting("physics", SettingValue::Json(...), Some("Physics simulation settings")) (:616-622)
        alt persist fails
            DB-->>H: warn only — the response still succeeds
        end
        H-->>H: 200 with the new PhysicsSettings JSON (:634)
    else Ok(Err(e))
        SA-->>H: 500 "Failed to update physics settings"
    else Err(mailbox)
        SA-->>H: 500 "Actor communication error"
    end
    Note over H,DB: DIVERGENCE — the physics PUT sends NO BroadcastMessage to other sessions.<br/>Rendering (:826-839) and node-filter (:954-972) DO broadcast via client_manager_addr.<br/>A second open session keeps stale physics until it re-reads. See VC-06.4
```

## VC-06.4 Broadcast asymmetry — which settings reach other sessions
```mermaid
sequenceDiagram
    autonumber
    participant H1 as update_rendering_settings<br/>src/settings/api/settings_routes.rs:798
    participant H2 as update_node_filter_settings<br/>src/settings/api/settings_routes.rs:910
    participant H3 as update_physics_settings<br/>src/settings/api/settings_routes.rs:469
    participant CC as ClientCoordinatorActor<br/>state.client_manager_addr
    participant B as other browser sessions

    H1->>CC: do_send(BroadcastMessage { message }) (:837-838)
    Note over H1,CC: payload built at :829-835, logged "Rendering settings change broadcast sent to connected clients"
    CC->>B: WS frame — other sessions converge
    H2->>CC: do_send(BroadcastMessage { message }) (:972)
    Note over H2,CC: payload built at :957, comment "Propagate node filter changes to all connected clients via broadcast" (:954)
    CC->>B: WS frame
    H3--xCC: no BroadcastMessage is sent
    Note over H3,B: RESOLVED ADR-2047 — physics now emits through the single<br/>broadcast_settings_change emitter, closing the asymmetry. RESOLVED ADR-2080 (vc-clients) —<br/>the client now CONSUMES settingsUpdated: nodeFilter applies its payload directly, other<br/>categories re-read via getSectionPaths/getSettingsByPaths, write-echo and stale timestamps<br/>are dropped. Before both, the server emitted settingsUpdated while the client validator knew<br/>only settings_update, so every broadcast fell through to Unknown message type.<br/>Still silent by choice: constraints, quality-gates, visual, profiles.
```

## VC-06.5 OptimizedSettingsActor message surface
```mermaid
classDiagram
    class OptimizedSettingsActor {
      <<src/actors/optimized_settings_actor.rs>>
      +started app_state.rs:1175
      +with_actors(repo, gs_addr, None)
    }
    class WarmCacheMessage {
      +Handler impl at line 765
    }
    class GetSettings {
      +Handler impl at line 778
    }
    class UpdateSettings {
      +AppFullSettings settings
      +Handler impl at line 805
    }
    class GetSettingByPath {
      +Handler impl at line 816
    }
    class GetSettingsByPaths {
      +Handler impl at line 847
    }
    class SetSettingsByPaths {
      +Handler impl at line 906
    }
    class UpdatePhysicsFromAutoBalance {
      +Handler impl at line 1093
    }
    class GetPerformanceMetrics {
      +Handler impl at line 1159
    }
    class ClearCaches {
      +Handler impl at line 1174
    }
    class ReloadSettings {
      +Handler impl at line 1187
    }
    OptimizedSettingsActor <|.. WarmCacheMessage
    OptimizedSettingsActor <|.. GetSettings
    OptimizedSettingsActor <|.. UpdateSettings
    OptimizedSettingsActor <|.. GetSettingByPath
    OptimizedSettingsActor <|.. GetSettingsByPaths
    OptimizedSettingsActor <|.. SetSettingsByPaths
    OptimizedSettingsActor <|.. UpdatePhysicsFromAutoBalance
    OptimizedSettingsActor <|.. GetPerformanceMetrics
    OptimizedSettingsActor <|.. ClearCaches
    OptimizedSettingsActor <|.. ReloadSettings
```

## VC-06.6 ProtectedSettingsActor — API keys, client tokens and user records
```mermaid
sequenceDiagram
    autonumber
    participant H as nostr_handler api-keys routes<br/>src/handlers/nostr_handler.rs:58-59
    participant PS as ProtectedSettingsActor<br/>src/app_state.rs:1211, Addr src/app_state.rs:356
    participant ST as ProtectedSettings store

    Note over PS: started with ProtectedSettings::default() — src/app_state.rs:1211
    H->>PS: GetApiKeys (handler src/actors/protected_settings_actor.rs:33)
    PS->>ST: read
    H->>PS: UpdateUserApiKeys (:81)
    PS->>ST: write
    H->>PS: GetUser (:142)
    par token lifecycle
        H->>PS: StoreClientToken (:65)
    and
        H->>PS: ValidateClientToken (:49)
    and
        H->>PS: CleanupExpiredTokens (:97)
    end
    par settings persistence
        H->>PS: MergeSettings (:112)
    and
        H->>PS: SaveSettings (:127)
    end
    Note over H,ST: INVARIANT — protected settings hold API keys and client tokens, a separate actor<br/>from OptimizedSettingsActor so the secret surface is not served by the /api/settings routes
    Note over H,ST: session-token realm and the get_session expiry gap see VC-03.5 and VC-05
```

## VC-06.7 Hexagonal hop — SettingsRepository port to the SQLite adapter
```mermaid
sequenceDiagram
    autonumber
    participant A as caller<br/>route or OptimizedSettingsActor
    participant P as SettingsRepository trait<br/>src/ports/settings_repository.rs:8
    participant AD as SqliteSettingsRepository<br/>src/adapters/sqlite_settings_repository.rs:378
    participant DB as SQLite via tokio_rusqlite

    Note over P: ADR-090 A6 slice 3 — src/ports/settings_repository.rs is a 14-line SHIM.<br/>The canonical trait lives in visionclaw_domain::ports::settings_repository. See VC-07.
    A->>P: get_setting(key)
    P->>AD: impl SettingsRepository for SqliteSettingsRepository (:378)
    AD->>DB: SELECT (:382)
    AD->>AD: decode_setting_value (:156) — JSON text to SettingValue
    alt row absent
        AD-->>A: Ok(None)
    else decode error
        AD-->>A: map_json_err (:151) to SettingsRepositoryError
    end
    A->>P: set_setting(key, value, description)
    P->>AD: set_setting (:415)
    AD->>AD: encode_setting_value (:161), current_owner_pubkey() (:128)
    AD->>DB: UPSERT
    par other trait methods
        A->>AD: delete_setting (:451)
    and
        A->>AD: has_setting (:472)
    and
        A->>AD: get_settings_batch (:493)
    and
        A->>AD: set_settings_batch (:578)
    and
        A->>AD: list_settings(prefix) (:616)
    end
    Note over AD: flatten_json (:171) / unflatten_pairs (:194) / insert_at_path (:202)<br/>store settings as flat dotted key rows and rebuild the nested JSON on read
    Note over AD,DB: adapter-only surface beyond the port — open (:247), from_connection (:261), connection (:267),<br/>get_file_sha1s (:276), upsert_file_sha1s (:295), get_sync_config (:323), set_sync_config (:344),<br/>clear_sync_metadata (:365). The corpus-sync SHA1 ledger shares this database. See VC-21.
    Note over AD,DB: errors are normalised by map_db_err (:140) and map_rusqlite_err (:146)
```

## VC-06.8 Validation and field-mapping helpers
```mermaid
flowchart TB
    IN["incoming settings JSON"]
    IN --> N1["normalize_physics_keys<br/>src/settings/api/settings_routes.rs:506-508<br/>snake_case and legacy names to canonical camelCase"]
    N1 --> V1["validate_physics_settings<br/>src/settings/api/settings_routes.rs:526"]
    V1 --> OUT["PhysicsSettings"]
    subgraph FIX["src/handlers/settings_validation_fix.rs"]
        F1["validate_physics_settings_complete(&Value) :5"]
        F2["validate_constraint(&Value) :67"]
        F3["convert_to_snake_case_recursive(&mut Value) :107"]
        F4["get_complete_field_mappings() -> HashMap :151"]
        F5["apply_field_mappings(&mut Value, &mappings) :270"]
    end
    subgraph DOM["canonical validators re-exported by src/config/mod.rs:28-31"]
        D1["validate_bloom_glow_settings"]
        D2["validate_hex_color"]
        D3["validate_percentage"]
        D4["validate_port"]
        D5["validate_width_range"]
    end
    V1 -.-> DOM
    FIX -.-> IN
    NOTE["the two families are parallel — settings_routes.rs carries its own normalise/validate pair<br/>while settings_validation_fix.rs holds a second, more complete set. Confirm which the<br/>live PUT path calls before relying on either."]
    FIX --- NOTE
```

## VC-06.9 Generated client types — src/bin/generate_types.rs
```mermaid
sequenceDiagram
    autonumber
    participant B as generate_types::main<br/>src/bin/generate_types.rs:6
    participant D as AppFullSettings<br/>visionclaw_domain::config
    participant F as client/src/types/generated/settings.ts

    B->>D: read the settings type graph
    B->>B: emit camelCase TypeScript
    B->>F: fs::write(output_path, &camel_case_code) (src/bin/generate_types.rs:27)
    Note over B,F: output_path is the literal "client/src/types/generated/settings.ts" (:18)<br/>the parent directory is created if absent (:19-21)
    B->>F: fs::metadata(output_path) then log the byte size (:34)
    Note over D,F: ADR-2041 — the generated types emit `knowledge`, never `logseq`.<br/>path_accessible_impls resolves both segments server-side (src/config/path_accessible_impls.rs:160 and :185)<br/>Full alias lifecycle see VC-09.15
    Note over B: server-side YAML is snake_case, the JSON and TS surface is camelCase —<br/>the serde alias behaviour is asserted at boot, src/main.rs:299-318
```

## VC-06.10 RESOLVED ADR-2046 — the dead SettingsActor surface and what replaced it
```mermaid
flowchart TB
    ADR["ADR-2046 — remove the dead SettingsActor<br/>and the orphaned src/config copies"]
    SA["DELETED src/settings/settings_actor.rs<br/>SettingsActor, 14 message types, 14 Handler impls<br/>never started at runtime"]
    EXP["DELETED re-export block in src/settings/mod.rs<br/>the ADR-2046 comment at :13-16 records the removal<br/>in place of the GetPhysicsSettings / LoadProfile /<br/>SaveProfile / SettingsActor re-exports"]
    TST["DELETED src/handlers/tests/settings_tests.rs<br/>the only start() caller — it was already commented out<br/>of the module tree and referenced two absent modules"]
    MOD["src/settings/mod.rs:17-18<br/>the surviving re-exports are auth_extractor and models only"]
    LIVE["LIVE actor is OptimizedSettingsActor<br/>src/app_state.rs:353 field, started at :1161<br/>see VC-06.1 for the live round trip"]

    ADR --> SA
    ADR --> EXP
    ADR --> TST
    SA --> MOD
    EXP --> MOD
    TST --> MOD
    MOD --> LIVE

    N1["RESOLVED ADR-2046 (2026-09-05) — this section used to draw SettingsActor, its<br/>message catalogue and three DIVERGENCE notes about a surface that never ran.<br/>All of it is deleted, so the divergences are closed rather than restated."]
    LIVE --- N1
```
