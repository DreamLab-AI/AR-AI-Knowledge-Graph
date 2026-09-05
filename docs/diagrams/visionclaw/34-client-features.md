---
id: VC-34
title: Client feature directories — API and WebSocket surface
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2041, ADR-2006, ADR-2074, ADR-2077]
sources:
  - client/src/features/settings/config/settings.ts
  - client/src/features/control-center/registry/settingsRegistry.ts
  - client/src/features/control-center/registry/manifest.ts
  - client/src/features/control-center/registry/groups/motion.ts
  - client/src/features/control-center/hooks/useSettingField.ts
  - client/src/store/settingsStore.ts
  - client/src/store/autoSaveManager.ts
  - client/src/api/settings/endpoints.ts
  - client/src/api/layoutApi.ts
  - crates/visionclaw-domain/src/config/app_settings.rs
  - src/bin/generate_types.rs
  - client/src/types/generated/settings.ts
  - client/scripts/emit-settings-manifest.ts
  - client/src/features/control-center/registry/settings-manifest.json
  - client/src/features/ontology/services/jss/contextLoader.ts
  - client/src/features/ontology/services/JssOntologyService.ts
  - client/src/features/ontology/services/jss/schemaParser.ts
  - client/src/features/ontology/services/jss/classExtractor.ts
  - client/src/features/ontology/services/jss/axiomHandler.ts
  - client/src/features/ontology/services/jss/inferenceClient.ts
  - client/src/store/websocketStore.ts
  - client/src/features/ontology/hooks/useInferenceService.ts
  - client/src/features/ontology/services/sparqlService.ts
  - client/src/features/ontology/services/inferredAxiomsService.ts
  - client/src/features/ontology/hooks/useHierarchyData.ts
  - client/src/features/ontology/hooks/useConstraintStats.ts
  - client/src/features/ontology/hooks/useOntologyWebSocket.ts
  - client/src/features/control-center/ControlCenter.tsx
  - client/src/features/control-center/panels/SettingsPanel.tsx
  - client/src/features/control-center/panels/SolidPanel.tsx
  - client/src/features/control-center/panels/OntologyPanel.tsx
  - client/src/features/solid/components/SolidTabContent.tsx
  - client/src/features/ontology/components/OntologyTabContent.tsx
  - client/src/features/control-center/governance/AcspCaseQueue.tsx
  - client/src/features/control-center/governance/useBrokerCaseQueue.ts
  - client/src/features/control-center/governance/brokerCaseQueue.ts
  - client/src/features/control-center/status/useConnectionTelemetry.ts
  - client/src/features/control-center/status/useSpacePilot.ts
  - client/src/features/control-center/kpi/useKpiSummary.ts
  - client/src/features/control-center/macros/useMacro.ts
  - client/src/features/control-center/echo/echoPulseBus.ts
  - client/src/features/bots/hooks/useAgentPolling.ts
  - client/src/features/bots/services/AgentPollingService.ts
  - client/src/features/bots/contexts/BotsDataContext.tsx
  - client/src/features/bots/services/BotsWebSocketIntegration.ts
  - client/src/features/bots/hooks/useAgentActionFeed.ts
  - client/src/features/bots/components/AgentDetailPanel.tsx
  - client/src/services/WebSocketEventBus.ts
  - client/src/features/monitoring/components/HealthDashboard.tsx
  - client/src/features/monitoring/hooks/useHealthService.ts
  - client/src/features/analytics/components/ShortestPathControls.tsx
  - client/src/features/analytics/hooks/useSemanticService.ts
  - client/src/features/analytics/store/analyticsStore.ts
  - client/src/features/analytics/store/nodeAnalyticsStore.ts
  - client/src/store/websocket/binaryProtocol.ts
  - client/src/features/graph/components/GemNodes.tsx
  - client/src/features/graph/components/ClusterHulls.tsx
  - client/src/features/physics/hooks/usePhysicsService.ts
  - client/src/features/command-palette/CommandRegistry.ts
  - client/src/features/command-palette/hooks/useCommandPalette.ts
  - client/src/features/command-palette/defaultCommands.ts
  - client/src/features/command-palette/components/CommandPalette.tsx
  - client/src/features/help/HelpRegistry.ts
  - client/src/features/help/components/HelpProvider.tsx
  - client/src/features/onboarding/components/OnboardingEventHandler.tsx
  - client/src/features/onboarding/hooks/useOnboarding.ts
  - client/src/features/onboarding/flows/defaultFlows.ts
  - client/src/features/design-system/patterns/MarkdownRenderer.tsx
  - client/src/features/design-system/animations.ts
  - client/src/api/workspaceApi.ts
  - client/src/hooks/useWorkspaces.ts
  - client/src/features/solid/hooks/useSolidPod.ts
  - client/src/features/solid/hooks/useSolidContainer.ts
  - client/src/features/solid/hooks/useSolidResource.ts
  - client/src/features/solid/components/PodBrowser.tsx
  - client/src/features/solid/components/PodSettings.tsx
  - client/src/features/solid/components/ResourceEditor.tsx
  - client/src/services/SolidPodService.ts
  - client/src/features/visualisation/components/CommandInput.tsx
  - client/src/features/visualisation/components/EmbeddingCloudLayer.tsx
  - client/src/features/visualisation/components/TransientBeamsLayer.tsx
  - client/src/features/visualisation/hooks/useTransientBeams.ts
  - client/src/store/transientBeamStore.ts
  - client/src/features/voice/pttAgentBinding.ts
  - client/src/features/voice/usePushToTalkAgentBinding.ts
  - client/src/services/PushToTalkService.ts
  - client/src/services/VoiceWebSocketService.ts
verified_commit: b00c28a0d
---
## VC-34.1 settings — field edit to server PUT
```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant SS as SettingSlider<br/>client/src/features/control-center/primitives/SettingSlider.tsx
    participant USF as useSettingField<br/>client/src/features/control-center/hooks/useSettingField.ts:49
    participant Store as useSettingsStore.updateSettings<br/>client/src/store/settingsStore.ts
    participant ASM as autoSaveManager<br/>client/src/store/autoSaveManager.ts:52
    participant LA as layoutApi.setMode<br/>client/src/api/layoutApi.ts
    participant EP as settingsApi endpoints<br/>client/src/api/settings/endpoints.ts:364
    participant SRV as Rust settings handlers

    U->>SS: drag slider
    SS->>USF: set(next)
    USF->>Store: updateSettings(draft => draft[path]=next)
    Note over Store: immer draft mutate, path is the FROZEN backend contract (settingsRegistry.ts:5)
    Store-->>ASM: queueChanges (internal, autoSaveManager.queueChange path -> value)
    USF->>ASM: queueChange(path, next) - explicit second queue, verbatim legacy redundancy
    alt path in LAYOUT_SETTING_PATHS (qualityGates.layoutMode or visualisation.graphs.knowledge.physics.layoutAlgorithm)
        USF->>LA: setMode(next, 800)
        LA->>SRV: POST layout mode change
    end
    ASM->>ASM: scheduleFlush - clearTimeout then setTimeout 500ms DEBOUNCE_DELAY
    Note right of ASM: INVARIANT - CLIENT_ONLY_PATHS (auth.nostr.connected, auth.nostr.publicKey) never leave the browser
    loop every queueChange before flush
        USF->>ASM: further edits reset the 500ms timer
    end
    ASM->>EP: updateSettingsByPaths(updates) on flush
    EP->>EP: bucket path by prefix - physics/rendering/qualityGates/nodeFilter/constraints/visual/local-only
    par physics bucket
        EP->>SRV: PUT /api/settings/physics
    and rendering bucket
        EP->>SRV: PUT /api/settings/rendering
    and qualityGates bucket
        EP->>SRV: PUT /api/settings/quality-gates
    and visual bucket
        EP->>SRV: PUT /api/settings/visual
    end
    alt PUT returns 401
        EP-->>USF: window CustomEvent settings-auth-failed
    else PUT fails (network/5xx)
        EP-->>ASM: settingsRetryManager.addFailedUpdate(path, value, message)
    else PUT ok
        SRV-->>EP: 200
    end
```
## VC-34.2 settings — type generation and QA manifest (build-time, not runtime)
```mermaid
flowchart TD
    RustStruct["AppFullSettings struct<br/>crates/visionclaw-domain/src/config/app_settings.rs:79"] -->|cargo run --bin generate_types| Gen["generate_typescript_interfaces()<br/>src/bin/generate_types.rs:56"]
    Gen -->|convert_to_camel_case| Out["client/src/types/generated/settings.ts"]
    Note1["settingsUIDefinition.ts and unifiedSettingsConfig.ts no longer exist. The chain is<br/>registry/settingsRegistry.ts + registry/manifest.ts (settingsRegistry.ts:6 names the old<br/>file only inside a zero-drift-test description). NOT doc drift - docs/explanation/<br/>control-center.md:88,110,296 already describes unifiedSettingsConfig.ts as deleted. It was<br/>the swarm brief that was stale, not the governing docs. Verified 2026-09-05"]
    Groups["registry/groups/*.ts<br/>motion, look, labels, quality, atmosphere, immersion, intelligence, system, agents, decisions, provenance"] --> ManifestTs["GROUP_DATA<br/>client/src/features/control-center/registry/manifest.ts:23"]
    ManifestTs --> Registry["settingsRegistry.ts:30 REGISTRY - icons attached"]
    ManifestTs -->|npm run gen:manifest, ts-node| Emit["client/scripts/emit-settings-manifest.ts:18 buildManifest()"]
    Emit --> ManifestJson["registry/settings-manifest.json - 224 fields, 11 groups"]
    ManifestJson -->|consumed by| QA["browser-automation coverage phase: assert every testid present+interactive"]
    ADR["ADR-2041: graph settings key renamed logseq to knowledge<br/>touches settings.ts, app_settings.rs, generate_types.rs output"] -.-> RustStruct
    ADR -.-> Out
```
## VC-34.3 ontology — JSS fetch, PATCH and WebSocket refresh (contextLoader is the shared base)
```mermaid
sequenceDiagram
    autonumber
    participant Svc as JssOntologyService<br/>client/src/features/ontology/services/JssOntologyService.ts:58
    participant CL as contextLoader.fetchWithAuth<br/>client/src/features/ontology/services/jss/contextLoader.ts:22
    participant SP as schemaParser.fetchJsonLd<br/>client/src/features/ontology/services/jss/schemaParser.ts:64
    participant CE as classExtractor.buildHierarchyFromJsonLd<br/>client/src/features/ontology/services/jss/classExtractor.ts
    participant AH as axiomHandler.patchOntology<br/>client/src/features/ontology/services/jss/axiomHandler.ts:39
    participant IC as InferenceClient<br/>client/src/features/ontology/services/jss/inferenceClient.ts:26
    participant WSS as webSocketService.connectSolid<br/>client/src/store/websocketStore.ts
    participant JSS as JSS Solid server

    Svc->>Svc: initialize() - loadIntoStore then connectWebSocket
    Svc->>SP: fetchOntologyJsonLd(options)
    SP->>SP: isCacheValid(cache) check TTL
    alt cache miss or skipCache
        SP->>CL: getOntologyUrl() = SOLID_POD_BASE_URL + ONTOLOGY_RESOURCE_PATH
        CL->>CL: nostrAuth.isAuthenticated - dev mode Bearer dev-session-token or NIP-98 signRequest
        CL->>JSS: fetch GET credentials include
        JSS-->>CL: 200 JSON-LD body
    else cache hit
        SP-->>Svc: cached JsonLdOntology
    end
    Svc->>CE: buildHierarchyFromJsonLd(jsonLd)
    CE-->>Svc: OntologyHierarchy
    Svc->>Svc: useOntologyStore.getState().setHierarchy/setMetrics/setLoaded
    Svc->>IC: connectWebSocket()
    IC->>IC: connect - guard on JSS_WS_URL (VITE_JSS_WS_URL) else warn and return
    IC->>WSS: webSocketService.connectSolid()
    IC->>WSS: subscribeSolidResource(ontologyUrl, handleNotification)
    WSS-->>IC: SolidNotification type pub on resource change
    IC->>SP: invalidateCache then fetchJsonLd skipCache true
    IC->>CE: buildHierarchyFromJsonLd(jsonLd)
    IC->>Svc: useOntologyStore.setHierarchy - full_refresh event fanned to onResourceChange callbacks
    Note over Svc,JSS: see VC-26 for the Solid pod data path and VC-33 for auth token issuance
    opt user edits an axiom (OntologyContribution.tsx)
        Svc->>AH: patchOntology(cache, sparqlUpdate)
        AH->>CL: fetchWithAuth(getOntologyUrl(), method PATCH)
        CL->>JSS: PATCH content-type application/sparql-update
        JSS-->>AH: PatchResult status
    end
```
## VC-34.4 ontology — inference REST, SPARQL console and reasoning report
```mermaid
sequenceDiagram
    autonumber
    participant Panel as OntologyPanel / InferencePanel / SparqlConsole<br/>client/src/features/ontology/components/
    participant UIS as useInferenceService<br/>client/src/features/ontology/hooks/useInferenceService.ts:49
    participant Sparql as sparqlService.runSparqlSelect<br/>client/src/features/ontology/services/sparqlService.ts:73
    participant Inferred as inferredAxiomsService<br/>client/src/features/ontology/services/inferredAxiomsService.ts:168
    participant Hier as useHierarchyData<br/>client/src/features/ontology/hooks/useHierarchyData.ts:57
    participant Stats as useConstraintStats<br/>client/src/features/ontology/hooks/useConstraintStats.ts:25
    participant UAC as unifiedApiClient
    participant SRV as Rust /api/inference and /api/ontology handlers

    Panel->>UIS: runInference(request)
    UIS->>UAC: POST /inference/run
    UAC->>SRV: HTTP POST
    Panel->>UIS: validateOntology(request)
    UIS->>UAC: POST /inference/validate
    Panel->>UIS: getClassification / getConsistencyReport
    UIS->>UAC: GET /inference/classification, /inference/consistency
    opt cache clear
        UIS->>UAC: DELETE /inference/cache/{ontologyId}
    end
    Panel->>Sparql: runSparqlSelect(query)
    Sparql->>Sparql: isReadOnlySelect(query) guard - rejects non-SELECT
    Sparql->>UAC: POST /ontology/sparql (SPARQL_ENDPOINT)
    UAC->>SRV: query executed against Oxigraph
    Panel->>Inferred: fetchReasoningReport()
    Inferred->>UAC: GET /ontology/inferred (INFERRED_ENDPOINT)
    Note right of Inferred: INVARIANT - urn:ngm:graph:ontology:inferred<br/>is a read-only projection, never hand-edited
    Hier->>Hier: fetch(url) direct - hierarchy tree endpoint, TTL-refetch on options change
    loop every 5000ms (intervalMs default)
        Stats->>Stats: poll constraint stats while enabled
    end
```
## VC-34.5 ontology — server-pushed validation over WebSocket
```mermaid
sequenceDiagram
    autonumber
    participant SRV as Rust ontology validation handler
    participant WSS as webSocketService<br/>client/src/store/websocketStore.ts
    participant WSHook as useOntologyWebSocket<br/>client/src/features/ontology/hooks/useOntologyWebSocket.ts:49
    participant Store as useOntologyStore

    par validation update
        SRV-->>WSS: ontology_validation_update message
        WSS-->>WSHook: onMessage callback
        WSHook->>Store: setValidating / setViolations / setMetrics
    and full load
        SRV-->>WSS: ontology_loaded message with constraintGroups
        WSS-->>WSHook: setLoaded(true)
    end
    Note over SRV,Store: see VC-32 for the WebSocket frame/opcode machinery itself
```
## VC-34.6 control-center — shell composition (SettingsPanel hosts Solid and Ontology panels)
```mermaid
flowchart TD
    CC["ControlCenter.tsx:32<br/>ControlCenter component"] --> Dock["GlassDock<br/>primitives/GlassDock.tsx"]
    CC --> MacroBar["MacroBar<br/>macros/MacroBar.tsx"]
    CC --> CmdInput["CommandInput<br/>visualisation/components/CommandInput.tsx"]
    CC --> SP["SettingsPanel<br/>panels/SettingsPanel.tsx:15"]
    SP -->|REGISTRY, GROUP_BY_ID, ALL_FIELDS| Reg["settingsRegistry.ts:30"]
    SP -->|PANELS list solid+ontology| Man["manifest.ts:38"]
    SP --> SolidPanel["SolidPanel<br/>panels/SolidPanel.tsx:7"]
    SP --> OntPanel["OntologyPanel<br/>panels/OntologyPanel.tsx:7"]
    SolidPanel --> SolidTab["SolidTabContent<br/>client/src/features/solid/components/SolidTabContent.tsx"]
    OntPanel --> OntTab["OntologyTabContent<br/>client/src/features/ontology/components/OntologyTabContent.tsx"]
    SP -->|SearchInput| DS["design-system SearchInput.tsx"]
    CC --> Hotkeys["useControlCenterHotkeys<br/>hooks/useControlCenterHotkeys.ts"]
    CC --> Reveal["useRevealSetting<br/>hooks/useRevealSetting.ts"]
    CC -->|SpaceDriver| SPD["SpaceDriverService - see VC-37 for device wire protocol"]
    Note1["AgentOpsSurface.tsx and StatusSurface/StatusFlyout mount alongside SettingsPanel under ControlCenter's tab state - see useControlCenterUI.ts"]
```
## VC-34.7 control-center — ACSP governance case queue (ADR-2006)
```mermaid
sequenceDiagram
    autonumber
    participant Q as AcspCaseQueue.tsx<br/>client/src/features/control-center/governance/AcspCaseQueue.tsx:13
    participant H as useBrokerCaseQueue<br/>client/src/features/control-center/governance/useBrokerCaseQueue.ts:32
    participant Pure as brokerCaseQueue.ts pure helpers<br/>parseBrokerEvent:41, toCaseView:81, applyBrokerEvent:116
    participant UAC as unifiedApiClient
    participant WSS as webSocketService<br/>client/src/store/websocketStore.ts
    participant SRV as broker_events.rs handlers (WS-9/WS-12)

    Q->>H: mount
    H->>UAC: getData GET /broker/inbox
    UAC->>SRV: HTTP GET
    alt unauthenticated or unavailable
        SRV-->>H: error
        H->>H: fail-soft - logger.debug, empty queue, no crash
    else ok
        SRV-->>H: cases[]
        H->>Pure: toCaseView(c) per case, openCaseIds(inbox)
        H-->>Q: cases, openCount
    end
    H->>WSS: onMessage subscribe (multiplexed graph socket)
    loop live broker events
        SRV-->>WSS: broker:new_case / broker:case_decided
        WSS-->>H: message
        H->>Pure: parseBrokerEvent(message)
        H->>Pure: applyBrokerEvent(openIds, event) - ambient count updates instantly
        H->>H: refresh() - full case list re-fetched for metadata
    end
    Q->>H: decide(caseId, outcome, reasoning)
    H->>UAC: POST /broker/cases/{caseId}/decide
    UAC->>SRV: HTTP POST WS-9 operator route
    SRV-->>H: success
    H->>H: optimistically close caseId locally then refresh()
    Note right of H: INVARIANT ADR-2006 - ACSP decisions require human approval through this exact route, never auto-decided
```
## VC-34.8 control-center — status telemetry, KPI polling and echo/macro bus
```mermaid
sequenceDiagram
    autonumber
    participant Status as StatusSurface / StatusFlyout<br/>client/src/features/control-center/status/StatusSurface.tsx
    participant WsStat as useWebSocketStatus<br/>client/src/features/control-center/status/useConnectionTelemetry.ts:28
    participant LM as useLayoutMotion<br/>client/src/features/control-center/status/useConnectionTelemetry.ts:61
    participant SPHook as useSpacePilot (control-center)<br/>client/src/features/control-center/status/useSpacePilot.ts:32
    participant Kpi as KpiPanel/useKpiSummary<br/>client/src/features/control-center/kpi/useKpiSummary.ts:25
    participant UAC as unifiedApiClient
    participant Macro as useMacro<br/>client/src/features/control-center/macros/useMacro.ts:65
    participant Echo as echoPulseBus<br/>client/src/features/control-center/echo/echoPulseBus.ts:39
    participant WSS as websocketStore

    Status->>WsStat: subscribe websocketStore connection state
    Status->>LM: read useWebSocketStore statistics for motion readout
    Status->>SPHook: read SpaceDriver connected/deviceName state
    Note right of SPHook: see VC-37 for SpaceDriver<br/>WebHID wire protocol - not redrawn here
    Kpi->>UAC: getData GET /kpi/summary
    UAC-->>Kpi: raw payload
    Kpi->>Kpi: normaliseKpiSummary(raw) -> KpiTileView[]
    Macro->>Macro: invoke() - useSettingsStore.updateSettings + autoSaveManager.queueChange (same path as VC-34.1)
    Macro->>Echo: emitEchoPulse(detail) - CustomEvent visionclaw:echo-pulse
    Echo-->>Status: EchoPulseLayer subscribeEchoPulse renders transient ring
    Note over Status,WSS: see VC-32 for the underlying WebSocket connection lifecycle
```
## VC-34.9 bots — adaptive REST polling of the agent population
```mermaid
sequenceDiagram
    autonumber
    participant Ctx as BotsDataContext<br/>client/src/features/bots/contexts/BotsDataContext.tsx
    participant Hook as useAgentPolling<br/>client/src/features/bots/hooks/useAgentPolling.ts:111
    participant Svc as AgentPollingService<br/>client/src/features/bots/services/AgentPollingService.ts:68
    participant UAC as unifiedApiClient
    participant SRV as GraphStateActor bots_graph_data store

    Ctx->>Hook: mount
    Hook->>Svc: subscribe(callback) then start()
    Svc->>Svc: subscriberCount++ - only subscriber 1 begins poll()
    loop poll every currentInterval (2000ms active / 10000ms idle)
        Svc->>UAC: getData GET /bots/data
        UAC->>SRV: HTTP GET
        Note right of SRV: bots_graph_data is SEPARATE from the main graph<br/>UpdateBotsGraph - never merged, /graph/data?graph_type=agent stays empty
        SRV-->>Svc: envelope {success, data:{nodes, edges}} or bare body (older builds)
        Svc->>Svc: unwrap envelope.data ?? envelope defensively
        Svc->>Svc: hashData(data) - skip callback fan-out if unchanged
        Svc->>Svc: updateActivityLevel - activeRatio>0.2 or hasChanged or tasks pending -> active 2000ms else idle 10000ms
        alt hasChanged
            Svc->>Hook: callback(data) fan-out to all subscribers
            Hook->>Hook: transformAgentData(data) / isGenuineAgentNode filter
        end
        alt request throws
            Svc->>Svc: handlePollingError - retryCount++, retryDelay backoff
            break retryCount >= maxRetries (3)
                Svc->>Svc: stop() - polling halted, error surfaced via errorCallbacks
            end
        end
    end
    Hook-->>Ctx: unmount -> Svc.stop() - subscriberCount-- only clears timer at zero
```
## VC-34.10 bots — WebSocket integration, action feed and interrupt
```mermaid
sequenceDiagram
    autonumber
    participant WSI as BotsWebSocketIntegration<br/>client/src/features/bots/services/BotsWebSocketIntegration.ts:9
    participant WSS as webSocketService<br/>client/src/store/websocketStore.ts
    participant Bus as webSocketEventBus<br/>client/src/services/WebSocketEventBus.ts
    participant Feed as useAgentActionFeed<br/>client/src/features/bots/hooks/useAgentActionFeed.ts:103
    participant WSStore as useWebSocketStore<br/>client/src/store/websocketStore.ts
    participant Panel as AgentDetailPanel<br/>client/src/features/bots/components/AgentDetailPanel.tsx:58
    participant UAC as unifiedApiClient
    participant SRV as interrupt_task handler

    WSI->>WSS: onConnectionStatusChange, onMessage, onBinaryMessage, on('bots-position-update')
    WSS-->>WSI: message.type == graph-update | botsGraphUpdate | bots-full-update
    WSI->>Bus: emit message:bots
    WSI->>WSI: emit knowledge-graph-update / bots-graph-update / bots-full-update
    Note over WSI,WSS: see VC-32 for binary frame layout and opcode registry
    WSStore->>Feed: on(0x23 AGENT_ACTION) binary decode
    Feed->>Feed: agentActionVerb(actionType) - live feed of decoded actions
    Panel->>Panel: interruptible check - resolution unresolved or interruptible false
    alt agent is interruptible
        Panel->>UAC: POST /bots/interrupt {task_id, agent_id}
        UAC->>SRV: HTTP POST
        SRV-->>Panel: response.message - status done
    else externally spawned, no terminate verb
        Panel->>Panel: interruptStatus = not-interruptible<br/>Externally spawned - not interruptible from here
    end
    Note right of Panel: D2 steering - task_id sent as-is, server resolves the active task before stopping
```
## VC-34.11 monitoring — health dashboard poll and MCP relay control
```mermaid
sequenceDiagram
    autonumber
    participant HD as HealthDashboard<br/>client/src/features/monitoring/components/HealthDashboard.tsx:19
    participant Hook as useHealthService<br/>client/src/features/monitoring/hooks/useHealthService.ts:48
    participant UAC as unifiedApiClient
    participant SRV as Rust /health handlers

    HD->>Hook: mount, pollHealth default true
    Hook->>Hook: fetchHealth() immediately
    par
        Hook->>UAC: GET /health
    and
        Hook->>UAC: GET /health/physics
    end
    UAC->>SRV: HTTP GET
    SRV-->>Hook: HealthStatus, PhysicsHealth
    loop every pollInterval (default 5000ms)
        Hook->>Hook: setInterval(fetchHealth, pollInterval)
    end
    HD->>Hook: startMCPRelay()
    Hook->>UAC: POST /health/mcp/start
    HD->>Hook: getMCPLogs()
    Hook->>UAC: GET /health/mcp/logs
    Note right of HD: HD composes design-system Card/Badge/Button/Toast primitives - see VC-34.16
```
## VC-34.12 analytics — semantic service REST, SSSP store and wire-decoded per-node analytics
```mermaid
sequenceDiagram
    autonumber
    participant SPC as ShortestPathControls<br/>client/src/features/analytics/components/ShortestPathControls.tsx:38
    participant USS as useSemanticService<br/>client/src/features/analytics/hooks/useSemanticService.ts:85
    participant AS as useAnalyticsStore<br/>client/src/features/analytics/store/analyticsStore.ts:268
    participant UAC as unifiedApiClient
    participant WSBin as binaryProtocol receive path<br/>client/src/store/websocket/binaryProtocol.ts:384
    participant NAS as nodeAnalyticsStore<br/>client/src/features/analytics/store/nodeAnalyticsStore.ts:35
    participant Gem as GemNodes / ClusterHulls<br/>client/src/features/graph/components/

    USS->>UAC: GET /api/semantic/statistics
    USS->>UAC: POST /api/semantic/communities
    USS->>UAC: POST /api/semantic/centrality
    SPC->>USS: computeShortestPath(request)
    USS->>UAC: POST /api/semantic/shortest-path
    USS->>UAC: POST /api/semantic/constraints
    USS->>UAC: POST /api/semantic/cache/invalidate
    AS->>UAC: POST /api/analytics/shortest-path (currentResult, metrics, loading, error)
    Note over AS,UAC: analyticsStore and useSemanticService hit distinct endpoints<br/>for the same shortest-path feature - not unified
    WSBin->>NAS: ingest(parsedNodes) - wire offsets 36/40/44/48 plus sssp_distance@28
    Note right of NAS: ADR-03 D7, ADR-031 D2/D6 - stride 5 buffer<br/>[clusterId, anomalyScore, communityId, centrality, ssspDistance]
    Gem->>NAS: getIndexedBuffer(nodeIdToIndexMap) every render
    Note over WSBin,Gem: see VC-32 for the binary frame layout, VC-15 for GPU-side computation,<br/>VC-31 for the graph feature's own render-pipeline internals (not redrawn here)
```
## VC-34.13 physics — live simulation control (distinct from settings persistence)
```mermaid
sequenceDiagram
    autonumber
    participant Hook as usePhysicsService<br/>client/src/features/physics/hooks/usePhysicsService.ts:86
    participant UAC as unifiedApiClient
    participant SRV as Rust /api/physics handlers

    Hook->>UAC: GET /api/physics/status
    Hook->>UAC: POST /api/physics/start {simulation_id}
    Hook->>UAC: POST /api/physics/stop
    Hook->>UAC: POST /api/physics/parameters (params)
    Hook->>UAC: POST /api/physics/step
    Hook->>UAC: POST /api/physics/reset
    Hook->>UAC: POST /api/physics/optimize-layout
    Hook->>UAC: POST /api/physics/nodes/pin {nodes}
    Hook->>UAC: POST /api/physics/nodes/unpin {node_ids}
    Hook->>UAC: POST /api/physics/forces/apply {forces}
    loop after every mutating call
        Hook->>UAC: GET /api/physics/status - refetch to confirm applied state
    end
    UAC->>SRV: HTTP request
    Note right of Hook: /api/physics/* (this hook) is a live-simulation control<br/>surface, separate from /api/settings/physics (VC-34.1) which persists PhysicsSettings config
```
## VC-34.14 command-palette, help and onboarding — CustomEvent dispatch chain
```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant CP as CommandPalette.tsx<br/>client/src/features/command-palette/components/CommandPalette.tsx
    participant UCP as useCommandPalette<br/>client/src/features/command-palette/hooks/useCommandPalette.ts:9
    participant Reg as CommandRegistry<br/>client/src/features/command-palette/CommandRegistry.ts:6
    participant Def as defaultCommands.ts<br/>client/src/features/command-palette/defaultCommands.ts
    participant Help as helpRegistry / HelpProvider<br/>client/src/features/help/HelpRegistry.ts:3
    participant Onb as OnboardingEventHandler<br/>client/src/features/onboarding/components/OnboardingEventHandler.tsx:11
    participant Store as useSettingsStore

    U->>CP: Ctrl+K opens palette, types query
    CP->>UCP: filter Reg.getCommands() by query/category
    U->>CP: select command
    CP->>Reg: command.handler() execute
    alt id nav.help
        Reg->>Def: window.dispatchEvent(show-help)
    else id help.search
        Reg->>Def: window.dispatchEvent(search-help)
        Def->>Help: HelpProvider listens, opens search UI
    else id help.tour
        Reg->>Def: window.dispatchEvent(start-tour)
        Def->>Onb: window.addEventListener start-tour
        Onb->>Onb: startFlow(welcomeFlow, forceRestart true)
    else id settings.reset
        Reg->>Store: resetSettings() then window.location.reload()
    else id settings.export
        Reg->>Store: exportSettings() - Blob download, client-only
    else id settings.import
        Reg->>Store: importSettings(text) then reload
    else id system.save
        Reg->>Store: flushPendingUpdates() - forces autoSaveManager.forceFlush (VC-34.1)
    end
    Note over CP,Onb: help.tour is the only command-palette to onboarding link -<br/>onboarding otherwise self-contained via localStorage.onboarding.completedFlows
```
## VC-34.15 design-system — presentational primitives, no network calls of its own
```mermaid
flowchart LR
    DS["design-system/components/*.tsx<br/>Alert, Badge, Button, Card, Dialog, Input,<br/>Select, Slider, Switch, Tabs, Toast, Tooltip, SearchInput..."]
    MD["MarkdownRenderer.tsx<br/>client/src/features/design-system/patterns/MarkdownRenderer.tsx"]
    Anim["animations.ts<br/>client/src/features/design-system/animations.ts"]
    HD["monitoring/components/HealthDashboard.tsx:13-17<br/>Card, Label, Button, Badge, Toast"]
    SPC["analytics/components/ShortestPathControls.tsx:2-21<br/>Card, Select, Input, Tabs, Toast, ScrollArea"]
    SP["control-center/panels/SettingsPanel.tsx:22<br/>SearchInput"]
    DS --> HD
    DS --> SPC
    DS --> SP
    Anim -.-> DS
    MD -.-> DS
    Note1["No fetch/axios/unifiedApiClient/WebSocket anywhere under<br/>client/src/features/design-system - a pure component library, 32 importers across other feature dirs"]
```
## VC-34.16 contributor-studio and the workspace stub — both deleted (RESOLVED ADR-2077)
```mermaid
flowchart TD
    Was["client/src/features/contributor-studio/ held only an empty types/<br/>client/src/features/workspace/ held only an empty components/"] --> Gone["Both directory trees DELETED"]
    Gone --> R["RESOLVED ADR-2077: neither held a single source file and nothing<br/>imported either name. The feature tree no longer advertises two<br/>features that do not exist - client/src/features now holds 15<br/>directories, all with source."]
    Gone --> Keep["The workspace IMPLEMENTATION was never in the feature dir and is<br/>untouched: client/src/api/workspaceApi.ts and<br/>client/src/hooks/useWorkspaces.ts - drawn in VC-34.17"]
```
## VC-34.17 workspace — CRUD REST plus realtime WebSocket (implementation lives outside features/)
```mermaid
sequenceDiagram
    autonumber
    participant Comp as consumer component
    participant Hook as useWorkspaces<br/>client/src/hooks/useWorkspaces.ts:35
    participant API as workspaceApi<br/>client/src/api/workspaceApi.ts:136
    participant UAC as unifiedApiClient
    participant SRV as Rust /workspace handlers
    participant WS as ws://host/ws/workspaces<br/>client/src/hooks/useWorkspaces.ts:363

    Note over Comp,Hook: RESOLVED ADR-2077: the empty client/src/features/workspace/ stub<br/>is deleted. The real implementation always lived in client/src/hooks<br/>and client/src/api and is unchanged.
    Comp->>Hook: mount, initialLoad true
    Hook->>Hook: isValidCache check - 5 min cacheTimeout, skip fetch on hit
    Hook->>API: fetchWorkspaces({page, limit, ...filters})
    API->>UAC: GET /workspace/list?params
    UAC->>SRV: HTTP GET
    SRV-->>API: {success, data:{workspaces, total, page, hasMore}}
    API->>API: transformDates - createdAt/updatedAt/lastAccessed to Date
    Hook->>WS: new WebSocket ws(s)://host/ws/workspaces
    loop server pushes
        SRV-->>WS: {type: workspace_updated|workspace_created|workspace_deleted, data}
        WS-->>Hook: onmessage - optimisticallyUpdateWorkspace or splice state
    end
    alt WS closes with code != 1000
        WS-->>Hook: onclose
        Hook->>Hook: setTimeout(connectWebSocket, 5000) - manual reconnect
    end
    Comp->>Hook: createWorkspace(data)
    Hook->>API: createWorkspace - validates name length <=100
    API->>UAC: POST /workspace/create
    Comp->>Hook: updateWorkspace(id, data)
    Hook->>Hook: optimisticallyUpdateWorkspace(id, data) - applied before network
    Hook->>API: updateWorkspace -> PUT /workspace/{id}
    alt PUT fails
        Hook->>Hook: refresh() - full re-fetch discards optimistic state
    end
    Comp->>Hook: deleteWorkspace / toggleFavorite / archiveWorkspace
    Hook->>API: DELETE /workspace/{id} | POST /workspace/{id}/favorite | POST /workspace/{id}/archive
```
## VC-34.18 solid — feature-owned Pod UI components (own composition only)
```mermaid
flowchart TD
    Tab["SolidTabContent.tsx:6<br/>client/src/features/solid/components/SolidTabContent.tsx"] --> PS["PodSettings.tsx:6"]
    Tab --> PB["PodBrowser.tsx:6"]
    Tab --> RE["ResourceEditor.tsx:6"]
    PS -->|useSolidPod| USP["useSolidPod.ts:32<br/>checkPod, createPod, deletePod, initPod auto-provision"]
    PB -->|useSolidPod + useSolidContainer| USC["useSolidContainer.ts:31<br/>containerPath listing"]
    RE -->|useSolidResource| USR["useSolidResource.ts:30"]
    USP --> SPS["solidPodService<br/>client/src/services/SolidPodService.ts"]
    USC --> SPS
    USR --> SPS
    USP -->|useNostrAuth| Auth["nostrAuth / useNostrAuth"]
    PS -.-> DSComp["design-system Button/Card/Badge/Input/Label - see VC-34.15"]
    PB -.-> DSComp
    RE -.-> DSComp
    Note1["publicUrl() in useSolidPod.ts:16-20 rewrites internal JSS Docker<br/>hostnames (visionclaw-jss/jss/localhost) to the /solid/ public proxy path"]
    Note2["see VC-33 for Nostr NIP-98 auth issuance, VC-26 for the JSS<br/>Solid pod data path itself - not redrawn here"]
```
## VC-34.19 visualisation — CommandInput natural-language command bar
```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant CI as CommandInput.tsx<br/>client/src/features/visualisation/components/CommandInput.tsx
    participant UAC as unifiedApiClient
    participant Pod as SolidPodService<br/>client/src/services/SolidPodService.ts
    participant SRV as Rust handlers

    U->>CI: types command, submit
    CI->>CI: parseCommand(cmd) - keyword match against lower-cased text
    alt matches save/load/list view
        CI->>Pod: dynamic import SolidPodService then loadGraphView/listGraphViews
        opt view.physics present
            CI->>UAC: PUT /settings/physics view.physics
        end
        opt view.nodeTypeVisibility present
            CI->>CI: useSettingsStore.updateSettings draft mutate
        end
    else matches inject/show agent or swarm (and not node/only)
        CI->>UAC: POST /bots/mock-agents {agents: [...]}
        UAC->>SRV: HTTP POST
        CI->>CI: window.dispatchEvent visionclaw:status with injected count
    else no deterministic keyword match
        CI->>CI: dispatchToSettingsLLM(command)
        CI->>UAC: POST /bots/settings-command {command, settingsContext}
        Note right of CI: agent applies changes back through /api/settings/* -<br/>see VC-34.1 for that write path
    end
    CI->>CI: executeAction(action) - generic path: strip leading /api,<br/>route via unifiedApiClient.request(method, path, body)
    Note over CI,SRV: raw fetch would send no auth and get 401 -<br/>unifiedApiClient's interceptor injects NIP-98/dev token (VC-33)
```
## VC-34.20 visualisation — EmbeddingCloudLayer and TransientBeamsLayer
```mermaid
sequenceDiagram
    autonumber
    participant ECL as EmbeddingCloudLayer.tsx<br/>client/src/features/visualisation/components/EmbeddingCloudLayer.tsx
    participant WSStore as useWebSocketStore
    participant Store as transientBeamStore<br/>client/src/store/transientBeamStore.ts
    participant TBH as useTransientBeams<br/>client/src/features/visualisation/hooks/useTransientBeams.ts:24
    participant TBL as TransientBeamsLayer<br/>client/src/features/visualisation/components/TransientBeamsLayer.tsx:72

    ECL->>ECL: enabled flag check - lazy, JSON never bundled
    ECL->>ECL: fetch VITE_EMBEDDING_CLOUD_URL or default /embedding-cloud.json:132
    Note right of ECL: about 8MB file lives in public/, AbortController on unmount
    ECL->>ECL: slice to maxPoints, build keyIndexMap and nsIndexMap
    WSStore->>ECL: on('memoryFlash', event) - key/namespace lookup into index maps
    ECL->>ECL: semanticBurstColor - flashes matched point(s)
    Note over Store,TBH: 0x23 AGENT_ACTION frame decoded in binaryProtocol.ts<br/>pushes into transientBeamStore - see VC-32 for the frame layout
    TBH->>Store: read beams, pruneExpired()
    TBL->>TBH: beams, prune() called every R3F frame
    TBL->>TBL: beamRadius prop default 0.35 - matches AgentVisualSettings.beamRadius default (VC-34.1 schema)
    Note over ECL,TBL: node/edge render layers themselves are VC-31 -<br/>this diagram covers only these two features' own data intake
```
## VC-34.21 voice — push-to-talk agent binding (graph selection to governed dispatch)
```mermaid
sequenceDiagram
    autonumber
    participant Graph as visionclaw:node-selected event<br/>window CustomEvent
    participant Hook as usePushToTalkAgentBinding<br/>client/src/features/voice/usePushToTalkAgentBinding.ts:41
    participant Pure as pttAgentBinding.ts pure helpers<br/>client/src/features/voice/pttAgentBinding.ts
    participant PTT as PushToTalkService<br/>client/src/services/PushToTalkService.ts
    participant VWS as VoiceWebSocketService<br/>client/src/services/VoiceWebSocketService.ts

    Hook->>PTT: activate(userId) on mount
    Graph-->>Hook: node-selected {nodeId, did_nostr, metadata}
    Hook->>Pure: resolveSelectedAgentDid(detail)
    Pure->>Pure: isCanonicalDid - did:nostr:64-hex regex (ADR-125 I1)
    Hook->>PTT: setSelectedAgentDid(did or null)
    Note right of Pure: a non-agent node metadata carries no DID, resolves null,<br/>so PTT unbinds and never targets a non-agent
    PTT->>Hook: onServerNotify(active, did) - every PTT edge
    Hook->>VWS: setPtt(active, did) - server session carries target
    Hook->>Hook: handleTranscription(text, isFinal) fed from useVoiceInteraction
    Hook->>Pure: shouldDispatchGoverned(ptt.getState(), did)
    alt state commanding and did canonical
        Hook->>VWS: sendVoiceCommand(text, did) - signed 31402 to v1/voice-intent
    else not commanding or unbound
        Hook->>Hook: inert - falls through to settings assistant path (VC-34.19)
    end
    Note over Graph,VWS: see VC-35 for the STT capture / audio round-trip itself - not redrawn here
```
