---
id: VC-27
title: Agent estate integration — MCP relay, discovery, monitoring, ingest
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
  - docs/IDENTIFIER-taxonomy.md
adrs: [ADR-2025]
sources:
  - src/services/bots_client.rs
  - src/services/mcp_relay_manager.rs
  - src/handlers/mcp_relay_handler.rs
  - src/services/multi_mcp_agent_discovery.rs
  - src/handlers/multi_mcp_websocket_handler.rs
  - src/actors/multi_mcp_visualization_actor.rs
  - src/actors/agent_monitor_actor.rs
  - src/actors/task_orchestrator_actor.rs
  - src/services/management_api_client.rs
  - src/app_state.rs
  - src/services/agent_visualization_protocol.rs
  - src/services/agent_visualization_processor.rs
  - src/handlers/bots_visualization_handler.rs
  - src/handlers/memory_flash_handler.rs
  - src/agent_events/ingest.rs
  - src/agent_events/hub.rs
  - src/agent_events/schema.rs
  - src/agent_events/provenance.rs
  - src/services/acsp/client.rs
verified_commit: bed6b617d
---

## VC-27.1 BotsClient — legacy `:9500` MCP-TCP poller (superseded path)

```mermaid
sequenceDiagram
    autonumber
    participant Caller as caller<br/>src/services/bots_client.rs:138
    participant BC as BotsClient<br/>src/services/bots_client.rs:113
    participant MCP as McpTcpClient<br/>utils/mcp_tcp_client.rs
    participant GSS as GraphServiceSupervisor<br/>actors/graph_service_supervisor.rs:422

    Caller->>BC: connect(_bots_url) - bots_client.rs:138
    BC->>MCP: test_connection() - :144
    alt server unreachable
        MCP-->>BC: Ok(false) or Err
        BC-->>Caller: Err("MCP server is not reachable") - :159-164
    else reachable
        BC->>MCP: initialize_session() - :148
        BC->>BC: start_polling() - :167
        loop every 2s (tokio interval, :178)
            BC->>MCP: query_agent_list() - :183
            alt agents non-empty
                BC->>BC: Agent::from(mcp_agent) map - :188-189
                BC->>BC: agents.write().await = converted - :192-194
                opt graph_service_addr set
                    BC->>GSS: do_send(UpdateBotsGraph{agents}) - :202-205
                end
            else empty list
                BC->>BC: clear stored agents if non-empty - :207-211
            else query_agent_list Err
                BC->>BC: debug log, keep stale snapshot - :214-216
            end
        end
    end
    Note over BC,MCP: RESOLVED ADR-2088 (estate) - get_status() misreported on THREE axes, not one: host<br/>"agentic-workstation", port 9090 and an unconditional connected=true (:228,233-234). It now reports<br/>self.mcp_client.host/.port (the values resolved at :115-121) and a real AtomicBool connection state<br/>set from the actual test_connection() outcome. Two tokio tests cover it.
    Note over Caller,MCP: DIVERGENCE: agent_events/ingest.rs:12-15 marks this :9500 snapshot path<br/>as untouched/legacy - agent_action events use a separate /wss/agent-events ingest (see VC-27.13)
```

## VC-27.2 McpRelayManager — multi-agent-container lifecycle via docker exec

```mermaid
sequenceDiagram
    autonumber
    participant Caller as ensure_mcp_ready()<br/>src/services/mcp_relay_manager.rs:277
    participant MRM as McpRelayManager<br/>mcp_relay_manager.rs:43
    participant CB as CircuitBreaker<br/>failure_threshold=3, recovery_timeout=30s (:45-53)
    participant Docker as docker exec<br/>multi-agent-container

    Caller->>MRM: check_mcp_container() - :265
    Docker-->>MRM: docker ps -q -f name=multi-agent-container
    alt container absent
        MRM-->>Caller: Err("multi-agent-container is not running") - :279
    else container present
        Caller->>MRM: ensure_relay_running() - :160
        MRM->>MRM: health_manager.check_service_now("mcp-relay") - :161
        MRM->>CB: execute(check_relay_status_internal) - :64-73
        CB->>Docker: exec multi-agent-container pgrep -f mcp-server - :92-94
        alt pgrep succeeds (already running)
            Docker-->>MRM: status.success()=true
            MRM-->>Caller: Ok(()) - :175-177
        else not running
            MRM->>Docker: exec -d multi-agent-container bash -c 'cd /app then npm run mcp:start' - :181-190
            MRM->>MRM: std::thread::sleep(2s) - :197
            MRM->>Docker: re-check pgrep - :199
            alt now running
                MRM-->>Caller: Ok(())
            else still failing
                MRM-->>Caller: Err("MCP relay started but not running") - :202
            end
        end
        Caller->>Caller: tokio::time::sleep(1s) - :286
    end
    par background health loop
        loop every 30s (start_health_monitoring, :244-247)
            MRM->>MRM: health_manager.check_service_now("mcp-relay") - :249
        end
    end
    Note over MRM,CB: INVARIANT: CircuitBreakerConfig failure_rate_threshold=0.5, success_threshold=2,<br/>half_open_max_requests=3, minimum_request_threshold=5 (:45-53)
```

## VC-27.3 MCPRelayActor — `/ws/mcp-relay` session lifecycle (connect/retry/teardown)

```mermaid
sequenceDiagram
    autonumber
    participant Client as WS client
    participant H as mcp_relay_handler()<br/>src/handlers/mcp_relay_handler.rs:441
    participant A as MCPRelayActor<br/>mcp_relay_handler.rs:38, Actor impl :199
    participant O as Orchestrator WS<br/>ORCHESTRATOR_WS_URL default ws://multi-agent-container:3002/ws (:77-78)

    Client->>H: GET /ws/mcp-relay (upgrade) - main.rs:1033
    H->>H: extract Bearer token or ?token= - :452-463
    alt token empty
        H-->>Client: 401 "Authentication required" - :474-476
        Note over H: SECURITY: logged but not yet enforced on all clients (:447-450)
    else token present
        H->>A: ws::start(MCPRelayActor::new()) - :479
        A->>A: started() - register health endpoint, run_interval 30s ping+check (:220-232)
        A->>A: run_interval 60s circuit-breaker stats log (:234-250)
        A->>O: connect_to_orchestrator() - circuit_breaker.execute(connect_async, timeout) (:93-112)
        alt connect ok
            O-->>A: ws_stream split into tx/rx - :120-124
            A->>A: do_send(SetOrchestratorTx(tx)) - :124
            loop forward orchestrator->client (rx.next())
                O-->>A: Text/Binary/Ping/Close - :132-172
                A->>Client: ctx.text(msg) or ctx.binary(msg) - :279,289
            end
        else connect fails or times out
            A->>A: retry_delay = min(5s * 2^(attempts-1), 60s) - :185-188
            A->>A: sleep(retry_delay) then do_send("retry") - :191-192
            A->>A: connect_to_orchestrator() again - :276
        end
    end
    Client->>A: ws::Message::Text (JSON)
    alt type == "ping"
        A-->>Client: {"type":"pong", timestamp} - :320-328
    else forward to orchestrator
        alt orchestrator_tx set and healthy
            A->>O: send Text/Binary, 5s timeout (:355-372,404-421)
        else unhealthy or absent
            A-->>Client: {"type":"error", message} - :337-346,375-384
        end
    end
    Client->>A: ws::Message::Close
    A->>A: ctx.stop() - :427
    A->>A: stopped() logs - :255-257
```

## VC-27.4 MultiMcpAgentDiscovery — per-server agent + tool discovery

```mermaid
sequenceDiagram
    autonumber
    participant Caller as start_discovery()<br/>src/services/multi_mcp_agent_discovery.rs:169
    participant D as MultiMcpAgentDiscovery<br/>multi_mcp_agent_discovery.rs:62
    participant CF as claude-flow server<br/>host=CLAUDE_FLOW_HOST port=MCP_TCP_PORT default 9500 (:91-95)
    participant RS as ruv-swarm server<br/>host=RUV_SWARM_HOST port=RUV_SWARM_PORT default 9501 (:108-112)
    participant DAA as daa server<br/>host=DAA_HOST port=DAA_PORT default 9502 (:125-129)

    Caller->>D: initialize_default_servers() - :83-141
    D->>D: insert claude-flow/ruv-swarm/daa McpServerConfig - :86-135
    Caller->>D: start_discovery() - :169
    loop while discovery_running (tokio::spawn, :187-261)
        par concurrent per enabled server (:192-252)
            D->>CF: discover_server_agents -> discover_claude_flow_agents - :300-386
            CF-->>D: query_server_info / query_agent_list / query_swarm_status - :330,351,374
            D->>RS: discover_ruv_swarm_agents - :388-473
            RS-->>D: same three-call pattern, server_type=RuvSwarm - :417-462
            D->>DAA: discover_daa_agents - :475-570
            DAA-->>D: same pattern, server_type=Daa - :497-560
        end
        alt discover_server_agents Ok
            D->>D: insert server_info/agents/topology, successful_discoveries+=1 - :204-233
        else Err (connect/timeout)
            D->>D: failed_discoveries+=1, is_connected=false - :235-248
        end
        D->>D: tokio::time::sleep(1000ms) - :257
    end
    Note over D,CF: RESOLVED ADR-2083 (estate) - WIRED, not removed: the per-server values are deliberate, so the<br/>loop now sleeps the minimum interval across ENABLED servers, with a named MIN_DISCOVERY_INTERVAL_MS floor so a<br/>misconfigured 0 cannot spin it, and a named fallback when no server is enabled. The flat 1000ms sleep is gone.
    Note over CF,DAA: supported_tools fallback differs per server when query_server_info fails:<br/>claude-flow=[agent_list,swarm_status,server_info] (:341-345), ruv-swarm=[swarm_init,agent_spawn,daa_init,neural_train,benchmark_run] (:337-343 in ruv block), daa=[daa_agent_create,daa_workflow_create,daa_knowledge_share,daa_learning_status]
```

## VC-27.5 MultiMcpVisualizationWs — `/multi-mcp/ws` session and opcodes

```mermaid
sequenceDiagram
    autonumber
    participant Client as WS client
    participant H as multi_mcp_visualization_ws()<br/>src/handlers/multi_mcp_websocket_handler.rs:779
    participant Ws as MultiMcpVisualizationWs<br/>multi_mcp_websocket_handler.rs:111, Actor :425

    Client->>H: GET /multi-mcp/ws (upgrade) - configure_multi_mcp_routes :858-864
    H->>H: require Bearer/token or 401 - :787-814
    H->>Ws: ws::start(MultiMcpVisualizationWs::new) - :816
    Ws->>Ws: started() - start_heartbeat, register 3 health endpoints, start_position_updates - :428-452
    Ws->>Ws: run_interval 30s perform_health_checks() - :454-456
    Ws->>Ws: run_interval 60s recovery-if-idle-300s + circuit stats log - :458-480
    Ws->>Ws: send_discovery_data(ctx) - :482
    loop position updates (PerformanceMode: HighFreq=16ms/Normal=100ms/Low=1000ms, OnDemand=none) - :142-151
        Ws->>Ws: do_send(RequestAgentUpdate)
    end
    loop heartbeat every 5s - :155-166
        alt no pong for >30s
            Ws-->>Client: ctx.close(None) - :161
        else
            Ws-->>Client: ctx.ping(b"ping") - :165
        end
    end
    Client->>Ws: Text "ping" (plain)
    Ws-->>Client: "pong" - :502-505
    Client->>Ws: Text JSON {action, data} - ClientRequest :632-636
    alt action == configure
        Ws->>Ws: handle_client_config(ClientConfig{subscription_filters,performance_mode}) - :511-519
    else action == request_discovery
        Ws->>Ws: handle_discovery_request(ctx) - :520-522
    else action == request_agents
        Ws->>Ws: do_send(RequestAgentUpdate), degrade under open circuit breaker - :523-559
    else action == request_performance
        alt has_healthy_services() true
            Ws->>Ws: do_send(RequestPerformanceUpdate) - :575
        else degraded
            Ws-->>Client: cached "performance_data" status=degraded - :563-573
        end
    else action == request_topology
        Ws->>Ws: do_send(RequestTopologyUpdate{swarm_id}) - :578-587
    else unknown action
        Ws-->>Client: send_error_response("Unknown action: ...") - :589-595
    end
    Client->>Ws: ws::Message::Close
    Ws->>Ws: log final circuit-breaker stats, ctx.close(reason) - :602-620
    Note over Ws: RESOLVED ADR-2094 (2026-09-05): has_healthy_services (:253) is a pure atomic read of a cached verdict<br/>one monitor task started at connection init publishes it and stops when the client drops (:209) - no per-call spawn
    Note over H,Ws: DOC-DRIFT: GET /multi-mcp/status (get_mcp_server_status, :819-846) returns a<br/>hardcoded two-server JSON literal, not live MultiMcpAgentDiscovery state (:822-838)
    Note over H,Ws: DOC-DRIFT: POST /multi-mcp/refresh (refresh_mcp_discovery, :848-856) ignores<br/>app_state and never calls MultiMcpAgentDiscovery::start_discovery - it only echoes success
```

## VC-27.6 MultiMcpVisualizationActor — message set and periodic ticks

```mermaid
classDiagram
    class MultiMcpVisualizationActor {
        +HashMap(String,McpServerConfig) mcp_servers
        +HashMap(String,Position) agent_positions
        +HashMap(String,AgentInit) agents
        +HashMap(String,ConnectionInit) connections
        +HashMap(String,McpServerMetrics) server_metrics
        +LayoutAlgorithm layout_algorithm
        +Duration update_interval = 33ms
        +Vec~Recipient~ subscribers
        +SwarmTopologyData topology_data
        +GlobalPerformanceMetrics global_metrics
    }
    class MultiMcpVisualizationMessage {
        <<enum, rtype Result-unit-String>>
        Initialize servers,layout,physics,visual
        UpdateAgentPositions server_id,positions,timestamp
        AddAgent server_id,agent,position
        RemoveAgent server_id,agent_id
        UpdateAgentStatus server_id,agent_id,status,metadata
        AddConnection connection
        RemoveConnection connection_id
        UpdateServerMetrics server_id,metrics
        Subscribe recipient
        Unsubscribe recipient
        ChangeLayout algorithm
        AnalyzeTopology
        GetVisualizationState
        Reset
    }
    class MultiMcpVisualizationResponse {
        <<enum, rtype unit>>
        VisualizationState agents,positions,connections,servers,metrics,topology,global_metrics
        TopologyAnalysis topology_data,recommendations
        PerformanceMetrics global_metrics,server_metrics
    }
    class LayoutAlgorithm {
        <<enum, default ForceDirected>>
        ForceDirected attraction_strength,repulsion_strength,damping_factor
        Hierarchical server_separation,layer_height,node_spacing
        Circular radius_base,radius_increment,angular_spacing
        Grid grid_spacing,cluster_size,padding
    }
    MultiMcpVisualizationActor ..> MultiMcpVisualizationMessage : Handler impl at line 264
    MultiMcpVisualizationActor ..> MultiMcpVisualizationResponse : constructs
    MultiMcpVisualizationActor --> LayoutAlgorithm : layout_algorithm field
    note "run_interval 33ms update_visualization (245-247), 10s analyze_topology (249-251), 5s collect_global_metrics (253-255), all from started() at 242-256"
    note "RESOLVED ADR-2089 (estate): Subscribe and Unsubscribe both had zero senders, so the whole broadcast path was dead. 188 lines removed."
```

## VC-27.7 AgentMonitorActor — Management API poll loop and debounce

```mermaid
sequenceDiagram
    autonumber
    participant Sup as AppState / supervisor
    participant AM as AgentMonitorActor<br/>src/actors/agent_monitor_actor.rs:169, new() :203
    participant MAC as ManagementApiClient<br/>host=MANAGEMENT_API_HOST port=MANAGEMENT_API_PORT default 9090 (:209-214)
    participant GSS as GraphServiceSupervisor

    Sup->>AM: started() - is_connected=true, do_send(InitializeActor) - :355-361
    AM->>AM: handle(InitializeActor) - run_later(100ms) poll_agent_statuses + schedule_next_poll - :372-383
    loop self-rescheduling poll (schedule_next_poll, :336-342)
        AM->>MAC: tokio::join!(list_tasks(), get_system_status()) - :254-255
        alt tasks_result Ok
            AM->>AM: task_to_agent_status per active task - :297-301
            AM->>AM: do_send(ProcessAgentStatuses{agents,telemetry}) - :303
        else tasks_result Err
            AM->>AM: do_send(RecordPollFailure) - :307
        end
        AM->>AM: next_poll_delay() - base 15s (:233), doubles per consecutive_poll_failures (max shift 5), capped 90s - :325-332
    end
    AM->>AM: handle(ProcessAgentStatuses) - :492
    opt agents empty and MOCK_AGENTS=true/1
        AM->>AM: build_mock_swarm_agents() 5 mock agents - :387-487
    end
    AM->>AM: golden-angle spiral position per agent, poll_offset round-robin (ADR-031 item 1) - :519-554
    AM->>AM: decide_bots_graph_emit(count, last_nonempty, consecutive_empty) - :561
    alt roster non-empty
        AM->>GSS: do_send(UpdateBotsGraph{agents}) - :574-576
    else roster empty and consecutive_empty < EMPTY_CONFIRM_THRESHOLD=2
        AM->>AM: suppress emit - debounce a transient blip - :577-583
    else roster empty and confirmed (2nd consecutive empty)
        AM->>GSS: do_send(UpdateBotsGraph{agents: []}) - clears once - :569-576
    end
    Sup->>AM: TaskStatusChanged{agent_type,running_task_count} (from TaskOrchestratorActor, ADR-031 item 3)
    AM->>AM: poll_agent_statuses(ctx) immediate re-poll - :647
    Note over AM,MAC: INVARIANT: idle cadence is 15s (not 3s) to share agentbox's per-key rate-limit<br/>bucket with task creation - backoff cap 90s exceeds agentbox's 60s continueExceeding window (:233,315-324)
    Note over AM,GSS: DIVERGENCE (roster-clobber fix): an empty Management API poll is "no information"<br/>not "all agents died" - only a confirmed 2nd consecutive empty poll clears the graph (:557-567)
```

## VC-27.8 TaskOrchestratorActor — CreateTask/Interrupt/Drain message handlers

```mermaid
sequenceDiagram
    autonumber
    participant H as VisionClaw API handler
    participant TO as TaskOrchestratorActor<br/>src/actors/task_orchestrator_actor.rs:47, new() :66
    participant MAC as ManagementApiClient<br/>services/management_api_client.rs
    participant AM as AgentMonitorActor (agent_monitor_addr)

    TO->>TO: started() - do_send(InitializeActor) - :136-141
    TO->>TO: handle(InitializeActor) - run_interval 300s cache cleanup (Completed/Failed >5min old) - :158-177

    H->>TO: CreateTask{agent,task,provider,claude_flow_agent_id} - :185-198
    alt accepting_tasks == false (draining, ADR-031 item 7)
        TO-->>H: Err("Task creation rejected: actor is draining") - :301-306
    else running_count >= max_concurrent_tasks (MAX_CONCURRENT_TASKS env, default 20)
        TO-->>H: Err("At capacity: N/max tasks running") - ADR-031 item 2 (:308-330)
    else capacity available
        TO->>MAC: create_task_with_retry - max_retries=3, retry_delay=2s*attempt (:75-76,90-131)
        loop up to 3 attempts
            MAC-->>TO: Err -> sleep(retry_delay * attempts), retry - :113-127
        end
        alt final attempt Ok
            TO->>TO: active_tasks.insert(task_id, TaskState{status:Running,...}) - :367-380
            opt agent_monitor_addr set
                TO->>AM: do_send(TaskStatusChanged{agent_type,running_task_count}) - ADR-031 item 3 (:390-395)
            end
            TO-->>H: Ok(TaskResponse)
        else exhausted retries
            TO-->>H: Err(e.to_string()) - :359,399
        end
    end

    H->>TO: InterruptAgentTask{id} - :219-236
    alt id is a known local task_id
        TO->>TO: resolved = id (fast path, no round-trip) - :453,457-458
    else id not local
        TO->>MAC: list_tasks() - :464
        alt task_id match or claude_flow_agent_id match found
            TO->>TO: resolved = matched task_id - :479-487
        else no match
            TO-->>H: Err(InterruptError::Unresolved) - :488-490
        end
    end
    TO->>MAC: stop_task(resolved) - :493-497
    TO-->>H: Ok(resolved) or Err(InterruptError::Stop)

    H->>TO: DrainTasksBeforeShutdown{timeout_secs} - :621-626
    TO->>TO: accepting_tasks = false - :642
    loop every 1s until deadline (:646-663)
        alt running == 0
            TO->>TO: ctx.stop() - all tasks drained - :654-655
        else deadline exceeded
            TO->>TO: ctx.stop() - drain timeout, remaining tasks abandoned - :657-661
        end
    end
    Note over TO: DIVERGENCE: InterruptAgentTask deliberately never matches the role-label `agent`<br/>field ("coder"/"researcher") - only task_id or claude_flow_agent_id, to avoid stopping the wrong task (:474-478)
    Note over TO,AM: INVARIANT (ADR-031 item 3): every CreateTask success pushes TaskStatusChanged so<br/>AgentMonitorActor re-polls immediately rather than waiting its 15s idle cadence (see VC-27.7)
```

## VC-27.9 ManagementApiClient — agentbox management-api HTTP calls

```mermaid
sequenceDiagram
    autonumber
    participant Boot as AppState::new<br/>src/app_state.rs:1262-1271
    participant MAC as ManagementApiClient<br/>src/services/management_api_client.rs:27, new() :180-199
    participant API as agentbox management-api<br/>base_url = http://MANAGEMENT_API_HOST:MANAGEMENT_API_PORT (default agentic-workstation:9090)

    Boot->>Boot: validate_security_env_vars() - :78-167
    alt MANAGEMENT_API_KEY unset, insecure-default-listed, or <16 chars
        Boot->>Boot: log SECURITY CONFIGURATION ERROR, panic on Err - :135-157
    else key valid
        Boot->>MAC: ManagementApiClient::new(host, port, mgmt_api_key) - :1271, client.rs:180
        MAC->>MAC: reqwest Client::builder().timeout(30s).connect_timeout(10s) - :183-187
    end

    MAC->>API: POST /v1/tasks (create_task_with_context) - Authorization: Bearer api_key - :244-291
    alt status 202/200
        API-->>MAC: TaskResponse{task_id,...} - :295-305
    else other status
        MAC-->>MAC: Err(ApiError(text, status)) - :306-312
    else transport failure
        MAC-->>MAC: Err(NetworkError) - :291
    end

    MAC->>API: GET /v1/tasks/{task_id} (get_task_status) - :315-343
    MAC->>API: GET /v1/tasks (list_tasks) - :345-373
    MAC->>API: DELETE /v1/tasks/{task_id} (stop_task) - :375-400
    MAC->>API: GET /v1/status (get_system_status) - :402-430
    MAC->>API: POST /v1/briefs (create_brief) - :433-482
    MAC->>API: POST /v1/briefs/{id}/execute (execute_brief) - :489-533
    MAC->>API: POST /v1/briefs/{id}/debrief (create_debrief) - :539-580
    MAC->>API: GET /health (health_check, no auth header) - :586-597
    Note over MAC,API: every call above shares the same alt: 200/2xx Ok(json) else Err(ApiError(body,status)),<br/>and Err(NetworkError) on transport failure (repeated at each call site, e.g. :328-342,388-399)
    Note over Boot,MAC: RESOLVED ADR-2094 (2026-09-05): AgentMonitorActor::new calls the same validate_security_env_vars AppState uses (app_state.rs:82)<br/>a missing or weak MANAGEMENT_API_KEY is a boot error and the client is an Option, never an empty-string key (agent_monitor_actor.rs:235-267)
```

## VC-27.10 agent_visualization_protocol — outbound wire message envelope

```mermaid
classDiagram
    class AgentVisualizationMessage {
        <<enum>>
        Initialize InitializeMessage
        PositionUpdate PositionUpdateMessage
        StateUpdate StateUpdateMessage
        ConnectionUpdate ConnectionUpdateMessage
        MetricsUpdate MetricsUpdateMessage
    }
    class InitializeMessage {
        +i64 timestamp
        +String swarm_id
        +Option~String~ session_uuid
        +String topology
        +List~AgentInit~ agents
        +List~ConnectionInit~ connections
        +VisualConfig visual_config
        +PhysicsConfig physics_config
        +HashMap_String_Position positions
    }
    class PositionUpdateMessage {
        +i64 timestamp
        +List~PositionUpdate~ positions
    }
    class StateUpdateMessage {
        +i64 timestamp
        +List~AgentStateUpdate~ updates
    }
    class ConnectionUpdateMessage {
        +i64 timestamp
        +List~ConnectionInit~ added
        +List~String~ removed
        +List~ConnectionStateUpdate~ updated
    }
    class MetricsUpdateMessage {
        +i64 timestamp
        +SwarmMetrics overall
        +List~AgentMetrics~ agent_metrics
    }
    AgentVisualizationMessage --> InitializeMessage : serde rename init
    AgentVisualizationMessage --> PositionUpdateMessage : serde rename positions
    AgentVisualizationMessage --> StateUpdateMessage : serde rename state
    AgentVisualizationMessage --> ConnectionUpdateMessage : serde rename connections
    AgentVisualizationMessage --> MetricsUpdateMessage : serde rename metrics
    note "top-level enum is serde(tag = type),<br/>internally tagged (protocol.rs 6-23)"
    note "AgentInit (44-69): id,name,agent_type,<br/>status,color,shape,size,health,cpu,memory,<br/>activity,tasks_active,tasks_completed,<br/>success_rate,tokens,token_rate,<br/>capabilities List~String~,created_at i64"
    note "PositionUpdate (89-98): id,x,y,z f32<br/>plus vx,vy,vz Option~f32~"
    note "ConnectionInit (72-80): id,source,target,<br/>strength,flow_rate,color,active bool"
    note "AgentStateUpdate (107-123): id plus<br/>status,health,cpu,memory,activity,<br/>tasks_active,current_task all Option~T~<br/>- a partial differential update"
```

## VC-27.11 AgentVisualizationProcessor — `/api/visualization/agents/ws` init/refresh

```mermaid
sequenceDiagram
    autonumber
    participant Client as WS client
    participant H as agent_visualization_ws()<br/>src/handlers/bots_visualization_handler.rs:197
    participant Ws as AgentVisualizationWs<br/>bots_visualization_handler.rs:17, Actor :78
    participant Proto as AgentVisualizationProtocol<br/>services/agent_visualization_protocol.rs:630
    participant Proc as AgentVisualizationProcessor<br/>services/agent_visualization_processor.rs:182, new() :195

    Client->>H: GET /api/visualization/agents/ws - configure_routes :498-513
    H->>Ws: ws::start(AgentVisualizationWs::new) - :202
    Ws->>Ws: started() - do_send(InitConnection), start_heartbeat, start_position_updates - :81-89
    Ws->>Ws: handle(InitConnection) -> send_init_state(ctx) - :114-119
    Ws->>Proto: create_init_message("swarm-001","hierarchical", agents=Vec::new()) - :46-47
    Proto->>Proc: create_visualization_packet(agents, swarm_id, topology) - protocol.rs:972
    Proc->>Proc: process_agents() - color/shape/animation, spherical fallback position, glow_intensity - :211-293
    Proc->>Proc: create_connections(), create_clusters() - :455-479
    Proc-->>Proto: AgentVisualizationData{swarm,agents,connections,physics_config,...}
    Proto-->>Ws: init_json (AgentInit list mapped from VisualizedAgent) - protocol.rs:978-989
    Ws-->>Client: ctx.text(init_json) - :50
    loop position updates every 16ms (:57-63)
        Ws->>Proto: create_position_update() - :1106
        opt buffered updates present
            Ws-->>Client: ctx.text(update_json)
        end
    end
    loop heartbeat every 5s (:65-74)
        alt no pong for >10s
            Ws-->>Client: ctx.stop() - :68-71
        else
            Ws-->>Client: ctx.ping(b"ping") - :73
        end
    end
    Client->>Ws: Text {action} - :159-176
    alt action == refresh
        Ws->>Ws: send_init_state(ctx) again - :163
    else action == pause_updates or resume_updates
        Ws->>Ws: debug log only, no actual pause/resume effect - :165-170
    else unknown action
        Ws->>Ws: warn "Unknown client action" - :172
    end
    Note over Ws,Proto: PROPOSED ADR-2066 addendum: send_init_state still reports an empty roster, now explicit rather than<br/>disguised - the fake get_real_agent_data() helper is deleted. A real source exists (bots_client.get_agents_snapshot,<br/>bots_client.rs:231) but Agent lacks the profile, task counts, success_rate and timestamp AgentStatus requires, so the<br/>mapping needs a decided contract rather than invented defaults.
    Note over Ws: RESOLVED ADR-2066 addendum: the actor now carries a paused flag - pause_updates and resume_updates<br/>set it and the 16ms run_interval returns early while it is set, so the opcodes do what they advertise.
```

## VC-27.12 memory_flash_handler — `/api/memory-flash` RuVector access broadcast

```mermaid
sequenceDiagram
    autonumber
    participant Caller as RuVector-aware caller
    participant H as handle_memory_flash()<br/>src/handlers/memory_flash_handler.rs:41
    participant HB as handle_memory_flash_batch()<br/>memory_flash_handler.rs:103
    participant CC as ClientCoordinatorActor
    participant Ws as all connected WS clients

    Caller->>H: POST /api/memory-flash {key,namespace,action} - MemoryFlashRequest :16-23
    H->>H: build MemoryFlashBroadcast{type=memory_flash, data{key,namespace,action,timestamp}} - :44-59
    H->>CC: send(BroadcastMessage{message: json}) - :63-65
    alt actor Ok(Ok(()))
        H-->>Caller: 200 {ok:true} - :72
    else actor Ok(Err(e))
        H-->>Caller: 200 {ok:true, warn:e} - :74-76
    else mailbox Err(e)
        H-->>Caller: 500 {ok:false, error} - :78-83
    else serialization Err
        H-->>Caller: 500 {ok:false, error: serialization failed} - :87-93
    end
    CC->>Ws: fan out memory_flash JSON to every registered client

    Caller->>HB: POST /api/memory-flash/batch {events:[...]} - MemoryFlashBatchRequest :99-101
    loop each event in body.events (:113-127)
        HB->>HB: build MemoryFlashBroadcast per event, shared timestamp - :107-122
        HB->>CC: do_send(BroadcastMessage{message: json}) - :124
    end
    HB-->>Caller: 200 {ok:true, count} - :130
    Note over H,CC: routes mounted at /api/memory-flash and /api/memory-flash/batch via<br/>configure_routes (:133-139), configured inside the /api scope (main.rs:1128)
```

## VC-27.13 `/wss/agent-events` ingest — schema validation, hub fan-out, provenance

```mermaid
sequenceDiagram
    autonumber
    participant AB as agentbox management-api<br/>Note: see ES-02
    participant H as agent_events_ws()<br/>src/agent_events/ingest.rs:291
    participant Ws as AgentEventsIngestWs<br/>ingest.rs:149, Actor :187
    participant Sch as AgentActionNotification<br/>agent_events/schema.rs
    participant Prov as provenance::record<br/>agent_events/provenance.rs:96
    participant Hub as agent_events::hub<br/>agent_events/hub.rs, capacity=256 (:22)

    rect rgb(240,230,255)
    Note over AB,H: TRUST BOUNDARY - server-to-server ingest (agentbox pushes notifications/agent_action)
    AB->>H: GET /wss/agent-events (upgrade), subprotocol vc-agent-events.v1 - :48,305-311
    H->>H: authenticate() - Bearer or ?token= via NostrService::get_session - :251-288
    alt token valid
        H->>H: session_pubkey = Some(user.pubkey) - :269
    else token invalid and ALLOW_INSECURE_DEFAULTS unset
        H-->>AB: 401 Invalid or expired authentication token - :278-279
    else token invalid and ALLOW_INSECURE_DEFAULTS set (debug/dev-auth builds only)
        H-->>H: warn, accept unauthenticated (session_pubkey=None) - :270-276
    else no token and ALLOW_INSECURE_DEFAULTS set
        H-->>H: warn, accept unauthenticated - :281-283
    else no token, insecure defaults disallowed
        H-->>AB: 401 Authentication required - :285-286
    end
    end
    H->>Ws: WsResponseBuilder::new(AgentEventsIngestWs, harness).start() - :305-311
    AB->>Ws: Text frame (JSON-RPC notifications/agent_action) - :201
    Ws->>Sch: process_frame(text) - serde_json::from_str + is_canonical() - :94-96
    alt parse fails
        Ws-->>AB: {"error":"malformed_json"} - :223-226
    else parses but not canonical (wrong method or version<3)
        Ws-->>AB: {"error":"non_canonical_envelope"} - :219-222
    else canonical
        Ws->>Prov: provenance::record(&event) - classify() + record_crossings() - :107
        Prov->>Prov: classify by pubkey: 64-hex Attributed, malformed hex Malformed, absent Anonymous - :66-72
        Prov->>Prov: cross_from_agentbox(source_urn), cross_from_agentbox(target_urn) - ADR-2025 closed kind-map (uri.rs)
        Ws->>Hub: hub::publish(event) - broadcast::Sender, drops oldest under backpressure - :132,22-25
        opt event.has_ctc() true (typed CTC field populated)
            Ws->>Ws: fire_ctc_canary() - one-shot CANARY-VC-REC3-CTC via LivenessHarness - :167-184
        end
        Ws-->>AB: no ack frame - debug log only, always published regardless of provenance status - :210-217
    end
    Ws->>Ws: Ping/Pong/Close handled - pong echo, ctx.close+stop - :233-239
    Note over Ws,Hub: DOC-DRIFT: ingest.rs:12-15 says the legacy :9500 bots_client snapshot path (VC-27.1)<br/>is untouched by design - agent_action events use this socket exclusively, a disjoint payload
    Note over Prov: SECURITY: ProvenanceStatus::Attributed means a well-formed pubkey was asserted,<br/>NOT that a signature was verified - the wire carries no sig field (provenance.rs:29-38)
```
