---
id: VC-18
title: Analytics support handlers and the analytics WebSocket
area: visionclaw
governing:
  - docs/PROTOCOL-registry.md
  - docs/GPU-wire-abi.md
adrs: [ADR-2007, ADR-2009, ADR-2059]
sources:
  - src/handlers/api_handler/analytics/mod.rs
  - src/handlers/api_handler/analytics/feature_flags_handlers.rs
  - src/handlers/api_handler/analytics/insights_handlers.rs
  - src/handlers/api_handler/analytics/params_handlers.rs
  - src/handlers/api_handler/analytics/performance_handlers.rs
  - src/handlers/api_handler/analytics/stress_handlers.rs
  - src/handlers/api_handler/analytics/websocket_integration.rs
  - src/handlers/api_handler/analytics/state.rs
  - src/handlers/api_handler/analytics/types.rs
  - src/handlers/api_handler/analytics/sssp_handlers.rs
  - src/handlers/api_handler/ontology/mod.rs
  - src/actors/gpu/analytics_telemetry.rs
verified_commit: b00c28a0d
---

## VC-18.1 /analytics scope — auth wrapper and route table

```mermaid
flowchart TD
    SCOPE["web::scope(/analytics)<br/>mod.rs:169"]
    W1["wrap RequireAuth::authenticated().mutations_only()<br/>mod.rs:170"]
    WS["web::resource(/ws) wrap RequireAuth::authenticated()<br/>mod.rs:269-273"]

    subgraph POSTS["Authenticated mutations - POST"]
        P1["/params :172 update_analytics_params"]
        P2["/constraints :173 update_constraints"]
        P3["/focus :174 set_focus"]
        P4["/kernel-mode :175 set_kernel_mode"]
        P5["/feature-flags :204 update_feature_flags"]
        P6["/stress-majorization/trigger :188"]
        P7["/stress-majorization/reset-safety :192"]
        P8["/stress-majorization/params :196"]
        P9["/stress-majorization/configure :200"]
        P10["clustering + community + anomaly + sssp + pagerank + pathfinding<br/>see VC-15"]
    end
    subgraph GETS["Public read-only telemetry - GET"]
        G1["/params :238 get_analytics_params"]
        G2["/constraints :239 get_constraints"]
        G3["/stats :240 get_performance_stats"]
        G4["/gpu-metrics :241 get_gpu_metrics"]
        G5["/gpu-status :242 get_gpu_status"]
        G6["/gpu-features :243 get_gpu_features"]
        G7["/insights :251 get_ai_insights"]
        G8["/insights/realtime :252 get_realtime_insights"]
        G9["/dashboard-status :262 get_dashboard_status"]
        G10["/health-check :263 get_health_check"]
        G11["/feature-flags :264 get_feature_flags"]
        G12["/stress-majorization/stats :255"]
        G13["/stress-majorization/config :259"]
        G14["/sssp/status :253 get_sssp_status"]
    end

    SCOPE --> W1
    W1 --> POSTS
    W1 --> GETS
    SCOPE --> WS

    N1["INVARIANT ADR-2009 mutations_only() means every POST needs an authenticated pubkey while GETs stay public as harmless operational telemetry"]
    N2["The /ws resource takes the STRICTER wrap - RequireAuth::authenticated() with no mutations_only(), so even the read-only stream requires auth"]
    N3["update_feature_flags additionally takes an AuthenticatedUser extractor feature_flags_handlers.rs:30 - belt and braces on top of the scope wrap"]
    W1 -.- N1
    WS -.- N2
    P5 -.- N3
```

## VC-18.2 Feature flags — get, update, and what they actually gate

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant FH as feature_flags_handlers<br/>src/handlers/api_handler/analytics/feature_flags_handlers.rs:9
    participant ST as FEATURE_FLAGS static<br/>analytics/state.rs:15
    participant SS as sssp_handlers<br/>analytics/sssp_handlers.rs:109
    participant ON as ontology check_feature_enabled<br/>api_handler/ontology/mod.rs:332

    C->>FH: GET /analytics/feature-flags :264
    FH->>ST: FEATURE_FLAGS.lock().await :10
    ST-->>FH: FeatureFlags struct - TWO bools after ADR-2059 (was nine)
    FH-->>C: flags plus a description map covering only the surviving two
    C->>FH: POST /analytics/feature-flags :204
    Note over FH: requires AuthenticatedUser extractor :30 on top of the scope wrap
    FH->>ST: FEATURE_FLAGS.lock().await :35 then *flags = request.into_inner() :36
    Note over FH: WHOLESALE REPLACE - no field merge, a partial POST silently resets omitted flags to the body's values
    FH-->>C: success plus the new flags

    rect rgb(250,228,228)
        Note over ST,ON: What the flags actually do - the reason seven were removed
        ON->>ST: if !flags.ontology_validation :334
        Note over ON: ontology_validation is the ONLY flag read as a real GATE - it short-circuits ontology validation with an ErrorResponse
        SS->>ST: flags.sssp_integration written :45 and :93, read for display :109
        Note over SS: sssp_integration is written by /sssp/toggle and echoed by /sssp/status - it gates nothing
    end
    Note over ST: RESOLVED ADR-2059 - the seven flags that gated nothing are REMOVED from the struct, the Default and the description map. Only ontology_validation (a real gate) and sssp_integration (display-only, documented as such) remain
    Note over ST: OPEN ADR-2059 - FEATURE_FLAGS remains process-global, unpersisted and shared across all users, and update does a wholesale replace. Deliberately unchanged and routed to vc-knowledge, who owns the one reading side
```

## VC-18.3 Params handlers — analytics params, constraints, focus, kernel mode

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant PH as params_handlers<br/>src/handlers/api_handler/analytics/params_handlers.rs:17
    participant SET as OptimizedSettingsActor
    participant GPU as GPU compute addr<br/>app_state.get_gpu_compute_addr()

    C->>PH: GET /analytics/params :238
    PH->>SET: send(GetSettings) :20
    alt settings unavailable
        PH->>PH: create_default_analytics_params() :344
    end
    PH-->>C: AnalyticsParamsResponse

    C->>PH: POST /analytics/params :172
    PH->>GPU: send(UpdateVisualAnalyticsParams{...}) :63
    Note over GPU: handled by ForceComputeActor at force_compute_actor.rs:3659

    C->>PH: GET /analytics/constraints :239
    PH->>GPU: send(GetConstraints) :112
    C->>PH: POST /analytics/constraints :173
    PH->>GPU: send(UpdateConstraints{constraint_data}) :159
    opt update succeeded
        PH->>GPU: send(GetConstraints) :163 to echo back the applied set
    end
    Note over GPU: residency change re-derives ENABLE_CONSTRAINTS next step - see VC-11.5 and VC-16.6

    C->>PH: POST /analytics/focus :174
    PH->>PH: set_focus :203 parses SetFocusRequest into a FocusRegion
    PH->>GPU: send(UpdateVisualAnalyticsParams{...}) :296

    C->>PH: POST /analytics/kernel-mode :175
    PH->>PH: set_kernel_mode :357
    PH->>GPU: send(SetComputeMode{mode: compute_mode}) :383
    Note over GPU: handled by ForceComputeActor at force_compute_actor.rs:3238
    alt gpu addr is None
        Note over PH: every branch above degrades to an error response rather than panicking - get_gpu_compute_addr() returns Option
    end
```

## VC-18.4 Performance handlers — stats, GPU metrics, status and features

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant PF as performance_handlers<br/>src/handlers/api_handler/analytics/performance_handlers.rs:55
    participant TEL as analytics_telemetry<br/>src/actors/gpu/analytics_telemetry.rs
    participant GPU as GPU compute addr

    C->>PF: GET /analytics/stats :240
    PF->>PF: get_performance_stats :55
    C->>PF: GET /analytics/gpu-metrics :241
    PF->>PF: get_gpu_metrics :149
    PF->>TEL: analytics_telemetry::snapshot() and total_cpu_fallbacks()
    Note over TEL: per-kernel GPU-vs-fallback counters are PROCESS-GLOBAL, recorded by the GPU analytics actors via analytics_telemetry::record_execution
    Note over PF: telemetry is attached on EVERY response including failure branches, with no actor round-trip :156-162
    alt gpu addr present
        PF->>GPU: send(GetGPUMetrics) :168
        alt Ok(Ok(metrics))
            PF-->>C: metrics plus analytics_execution
        else Ok(Err(e)) or mailbox error
            PF-->>C: error response, still carrying analytics_execution
        end
    else no GPU
        PF-->>C: degraded response, still carrying analytics_execution
    end
    C->>PF: GET /analytics/gpu-status :242
    PF->>PF: get_gpu_status :197
    C->>PF: GET /analytics/gpu-features :243
    PF->>PF: get_gpu_features :281
    Note over TEL: INVARIANT a non-zero total_cpu_fallbacks means the zero-fallback intent has been violated this process lifetime - it is the observable signal for the trust gap in VC-15.13
```

## VC-18.5 Stress-majorization handlers

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant SH as stress_handlers<br/>src/handlers/api_handler/analytics/stress_handlers.rs:12
    participant GPU as GPU compute addr
    participant SMA as StressMajorizationActor<br/>src/actors/gpu/stress_majorization_actor.rs:315

    C->>SH: POST /stress-majorization/trigger :188
    SH->>GPU: send(TriggerStressMajorization) :17
    GPU->>SMA: TriggerStressMajorization :315
    alt no GPU context
        Note over SMA: Err "GPU not available for stress majorization" :95 - hard failure, no CPU path (see VC-11.7)
    end
    C->>SH: GET /stress-majorization/stats :255
    SH->>GPU: send(GetStressMajorizationStats) :43
    C->>SH: POST /stress-majorization/reset-safety :192
    SH->>GPU: send(ResetStressMajorizationSafety) :70
    GPU->>SMA: ResetStressMajorizationSafety :334
    C->>SH: POST /stress-majorization/params :196
    SH->>SH: build UpdateStressMajorizationParams :98
    SH->>GPU: send(msg) :102
    GPU->>SMA: UpdateStressMajorizationParams :347
    C->>SH: POST /stress-majorization/configure :200
    SH->>GPU: send(config.into_inner()) :131
    GPU->>SMA: ConfigureStressMajorization :410
    C->>SH: GET /stress-majorization/config :259
    SH->>GPU: send(GetStressMajorizationConfig) :158
    GPU->>SMA: GetStressMajorizationConfig :475
    Note over SH,SMA: All six handlers guard on get_gpu_compute_addr() being Some and return an error response otherwise
    Note over SMA: ADR-2060 bit5 ENABLE_STRESS_MAJORIZATION is RESERVED, declared but never set - stress majorization is CPU-side per simulation_params.rs:77 and is not a GPU force channel
```

## VC-18.6 Insights handlers

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant IH as insights_handlers<br/>src/handlers/api_handler/analytics/insights_handlers.rs:12
    participant GS as GraphServiceSupervisor
    participant ST as CLUSTERING_TASKS and ANOMALY_STATE<br/>analytics/state.rs
    participant GPU as GPU compute addr

    C->>IH: GET /analytics/insights :251
    IH->>GS: send(GetGraphData) :15
    IH->>ST: CLUSTERING_TASKS.lock() :20 and ANOMALY_STATE.lock() :21
    IH-->>C: derived AI insights
    C->>IH: GET /analytics/insights/realtime :252
    IH->>GS: send(GetGraphData) :140
    IH->>ST: CLUSTERING_TASKS.lock() :151 and ANOMALY_STATE.lock() :152
    opt gpu addr present :201
        IH->>GPU: send(GetPhysicsStats) :203
    end
    C->>IH: GET /analytics/dashboard-status :262
    IH->>IH: gpu_available = get_gpu_compute_addr().is_some() :232
    IH->>ST: CLUSTERING_TASKS.lock() :233 and ANOMALY_STATE.lock() :234
    C->>IH: GET /analytics/health-check :292
    IH-->>C: health summary
    Note over ST: ANOMALY_STATE is the AGENT-HEALTH heuristic fed by MCP telemetry via /anomaly/toggle - it is NOT node_analytics.anomaly, which comes from the GPU LOF kernel (mod.rs:182-184)
    Note over IH,ST: All four insights endpoints are pure reads over graph data plus process-global state - none dispatch a GPU kernel
```

## VC-18.7 Analytics WebSocket — connect, subscribe, stream

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RT as RequireAuth::authenticated()<br/>mod.rs:270
    participant WSH as gpu_analytics_websocket<br/>analytics/websocket_integration.rs:503
    participant A as GpuAnalyticsWebSocket actor<br/>websocket_integration.rs:95

    C->>RT: GET /analytics/ws upgrade
    alt not authenticated
        Note over RT: rejected before the upgrade - the stream is auth-gated even though it is read-only
    else authenticated
        RT->>WSH: gpu_analytics_websocket :503
        WSH->>A: ws::start(GpuAnalyticsWebSocket::new(app_state)) :539
        A->>A: Actor::started :385
        A->>C: message_type "connected" with clientId and a capabilities map :391-400
        Note over A: capabilities advertise gpuMetrics, clusteringProgress, anomalyAlerts, insightsUpdates, realTimeUpdates
        loop every clamped interval
            A->>A: ctx.run_interval(interval) :365
            A->>C: send_gpu_metrics / send_clustering_progress / send_anomaly_alerts / send_insights_update
            alt heartbeat older than 60s :367
                Note over A: client considered dead and the session is dropped
            end
        end
    end
    Note over A: MIN_UPDATE_INTERVAL_MS 100 :11 and MAX_UPDATE_INTERVAL_MS 60000 :12 bound the push cadence
    Note over A: This stream is JSON over text frames - it is NOT the binary 0x03/0x05 position socket, which is /wss (see VC-13)
```

## VC-18.8 Analytics WebSocket — inbound message handling

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant A as GpuAnalyticsWebSocket StreamHandler<br/>websocket_integration.rs:421

    C->>A: ws::Message::Text
    A->>A: heartbeat = Instant::now() :425
    alt JSON parses as AnalyticsWebSocketMessage :427
        alt message_type == "updateSubscriptions"
            A->>A: parse SubscriptionPreferences :436
            A->>A: clamp update_interval_ms to MIN 100 and MAX 60000 :438-441
            A->>C: "subscriptionsUpdated" echoing the clamped prefs :447-454
        else message_type == "requestImmediateUpdate"
            A->>C: send_gpu_metrics + send_clustering_progress + send_anomaly_alerts + send_insights_update :457-460
        else message_type == "ping"
            A->>C: "pong" with a server timestamp :463-470
        else unknown type
            Note over A: logged and ignored - no error frame is returned to the client
        end
    else malformed JSON
        Note over A: parse failure is logged, the frame is dropped, the session stays open
    end
    Note over A: INVARIANT the interval clamp is applied on INGEST so a client cannot request a faster push than 100 ms or a slower keepalive than 60 s
    Note over A: Message envelope AnalyticsWebSocketMessage :16 carries message_type, data, timestamp and an optional client_id
```

## VC-18.9 Analytics process-global state

```mermaid
classDiagram
    class FEATURE_FLAGS {
        +static_Lazy_Arc_Mutex state_rs_L15
        +ontology_validation_GATE_retained()
        +sssp_integration_DISPLAY_ONLY_retained()
        +seven_inert_flags_REMOVED_ADR_2059()
    }
    class ANOMALY_STATE {
        +static_Lazy_Arc_Mutex state_rs_L14
        +agent_health_heuristic_from_MCP_telemetry()
        +written_by_anomaly_toggle mod_L184
        +NOT_node_analytics_anomaly()
    }
    class CLUSTERING_TASKS {
        +static_Lazy_Arc_Mutex()
        +read_by_insights_handlers L20_L151_L233()
    }
    class AnalyticsTelemetry {
        +snapshot()
        +total_cpu_fallbacks()
        +record_execution()
        +process_global_per_kernel_counters()
    }
    FEATURE_FLAGS --> ANOMALY_STATE : both re-exported at mod_rs_L42
    CLUSTERING_TASKS --> ANOMALY_STATE : read together by insights

    note for FEATURE_FLAGS "ADR-2059: only ontology_validation gates - the inert seven are removed"
    note for FEATURE_FLAGS "OPEN ADR-2059: still process-global, unpersisted, shared"
    note for ANOMALY_STATE "Agent-health heuristic, distinct from the GPU LOF anomaly score on the wire"
    note for AnalyticsTelemetry "Non-zero total_cpu_fallbacks violates the zero-fallback intent"
```
