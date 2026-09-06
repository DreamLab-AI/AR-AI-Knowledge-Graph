---
id: VC-30
title: React client boot sequence and state layer
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2074, ADR-2077]
sources:
  - client/src/app/main.tsx
  - client/src/app/App.tsx
  - client/src/app/AppInitializer.tsx
  - client/src/app/MainLayout.tsx
  - client/src/contexts/ApplicationModeContext.tsx
  - client/src/store/settingsStore.ts
  - client/src/store/settings/coreSlice.ts
  - client/src/store/settings/physicsSlice.ts
  - client/src/store/settings/persistenceSlice.ts
  - client/src/store/settings/settingsTypes.ts
  - client/src/store/settings/subscriberTrie.ts
  - client/src/store/websocketStore.ts
  - client/src/store/websocket/storeState.ts
  - client/src/store/timelineStore.ts
  - client/src/store/transientBeamStore.ts
  - client/src/store/workerErrorStore.ts
  - client/src/store/autoSaveManager.ts
  - client/src/store/settingsRetryManager.ts
  - client/src/hooks/useSelectiveSettingsStore.ts
  - client/src/services/WebSocketRegistry.ts
  - client/src/services/WebSocketEventBus.ts
  - client/src/services/VoiceWebSocketService.ts
  - client/src/services/solidPod/podNotifications.ts
  - client/src/store/websocket/index.ts
  - client/src/store/websocket/solidWebSocket.ts
  - client/src/features/bots/services/BotsWebSocketIntegration.ts
  - client/src/services/platformManager.ts
  - client/src/services/remoteLogger.ts
  - client/src/services/api/UnifiedApiClient.ts
  - client/src/services/api/authInterceptor.ts
  - client/src/api/settingsApi.ts
  - client/src/api/settings/endpoints.ts
  - client/src/api/analyticsApi.ts
  - client/src/api/constraintsApi.ts
  - client/src/api/exportApi.ts
  - client/src/api/graphExpandApi.ts
  - client/src/api/layoutApi.ts
  - client/src/api/workspaceApi.ts
  - client/src/features/graph/managers/graphDataManager.ts
  - client/src/features/graph/managers/dataManager/restClient.ts
  - client/src/features/graph/managers/dataManager/nodeUtils.ts
  - client/src/features/graph/managers/dataManager/topology.ts
  - client/src/features/graph/managers/dataManager/wsClient.ts
  - client/src/features/graph/hooks/useGraphDataSubscription.ts
  - client/src/features/graph/components/GraphManager.tsx
  - client/src/features/graph/managers/graphWorkerProxy.ts
  - client/src/features/graph/workers/graph.worker.ts
  - client/src/features/graph/__tests__/sabOrTransferDetection.test.ts
  - client/src/features/graph/components/GraphCanvas.tsx
  - client/src/features/graph/components/NodeDetailPanel.tsx
  - client/src/features/graph/components/TimelineScrubber.tsx
  - client/src/features/control-center/ControlCenter.tsx
  - client/src/features/control-center/hooks/useSettingField.ts
  - client/src/features/bots/components/BotsVisualization.tsx
  - client/src/components/WorkerErrorModal.tsx
  - client/src/features/visualisation/components/EmbeddingCloudLayer.tsx
  - client/src/features/visualisation/hooks/useTransientBeams.ts
  - client/src/features/graph/components/GemNodes.tsx
  - client/src/features/graph/components/GlassEdges.tsx
  - client/src/features/bots/components/BotsNode.tsx
  - client/src/features/visualisation/components/TransientBeamsLayer.tsx
  - client/src/store/websocket/binaryProtocol.ts
  - client/src/features/control-center/hooks/useRevealSetting.ts
  - client/src/features/control-center/primitives/NostrAuthControl.tsx
  - client/src/features/control-center/status/StatusFlyout.tsx
  - client/src/services/nostrAuthService.ts
verified_commit: 7a20db228
---
## VC-30.1 Provider nesting and top-level render states
```mermaid
sequenceDiagram
    autonumber
    participant Main as main.tsx<br/>client/src/app/main.tsx:31
    participant App as App<br/>client/src/app/App.tsx:29
    participant Auth as useNostrAuth<br/>client/src/app/App.tsx:35
    participant Tip as TooltipProvider<br/>client/src/app/App.tsx:153
    participant Help as HelpProvider<br/>client/src/app/App.tsx:154
    participant Onb as OnboardingProvider<br/>client/src/app/App.tsx:155
    participant EB as ErrorBoundary<br/>client/src/app/App.tsx:156
    participant AMP as ApplicationModeProvider<br/>client/src/contexts/ApplicationModeContext.tsx:43
    participant AI as AppInitializer<br/>client/src/app/AppInitializer.tsx:79
    participant ML as MainLayout<br/>client/src/app/MainLayout.tsx:131

    Main->>App: ReactDOM.createRoot().render(<StrictMode><App/></StrictMode>)<br/>main.tsx:31-35
    App->>Auth: useNostrAuth()<br/>App.tsx:35
    alt isAuthLoading true
        App-->>Main: return LoadingScreen "Checking authentication..."<br/>App.tsx:107-109
    end
    Note over App: skipAuth = VITE_PUBLIC_DEMO hostname junkiejarvis.com OR (dev AND ?skipAuth=true|?test=visual)<br/>App.tsx:113-119
    alt !authenticated && !skipAuth
        App-->>Main: return OnboardingWizard<br/>App.tsx:122-127
    end
    App->>Tip: mount delayDuration=300 skipDelayDuration=100<br/>App.tsx:153
    Tip->>Help: mount
    Help->>Onb: mount
    Onb->>EB: mount
    EB->>AMP: mount
    AMP->>AMP: renderContent() switch(initializationState)<br/>App.tsx:129-148
    alt initializationState==loading
        AMP-->>App: LoadingScreen "Connecting to server..."<br/>App.tsx:131-132
        AMP->>AI: mount AppInitializer(onInitialized,onError)<br/>App.tsx:161-163
    else initializationState==error
        AMP-->>App: div Error Initializing Application + Retry button<br/>App.tsx:133-140
    else initializationState==initialized
        AMP->>ML: mount BotsDataProvider > MainLayout<br/>App.tsx:143-145
        AMP->>AMP: mount ConnectionWarning, CommandPalette,<br/>DebugControlPanel, WorkerErrorModal<br/>App.tsx:165-170
    end
    Note over App: useEffect(authenticated,user): setAuthenticated/setUser on settingsStore,<br/>solidPodService.connectWebSocket() else disconnect()<br/>App.tsx:43-67
    Note over App: useEffect(initialized): initializeCommandPalette, registerSettingsCommands,<br/>registerSettingsHelp, registerOnboardingCommands - first-visit onboarding<br/>dispatched after 1000ms via localStorage hasVisited flag<br/>App.tsx:71-89
```
## VC-30.2 AppInitializer — worker, settings, websocket and data fan-out
```mermaid
sequenceDiagram
    autonumber
    participant AI as AppInitializer.initApp<br/>client/src/app/AppInitializer.tsx:83
    participant LS as loadServices<br/>client/src/app/AppInitializer.tsx:12
    participant IM as innovationManager<br/>client/src/app/AppInitializer.tsx:26
    participant WP as graphWorkerProxy<br/>client/src/app/AppInitializer.tsx:7
    participant GDM as graphDataManager<br/>client/src/app/AppInitializer.tsx:8
    participant WES as useWorkerErrorStore<br/>client/src/app/AppInitializer.tsx:5
    participant Set as useSettingsStore.initialize<br/>client/src/app/AppInitializer.tsx:80
    participant WS as webSocketService<br/>client/src/app/AppInitializer.tsx:6
    participant App as onInitialized/onError<br/>client/src/app/App.tsx:91-104

    AI->>LS: loadServices().catch(...)<br/>fire-and-forget, AppInitializer.tsx:87
    LS->>IM: innovationManager.initialize({enableSync,enableComparison,<br/>enableAnimations,enableAdvancedInteractions,performanceMode:'balanced'})<br/>AppInitializer.tsx:26-32
    par race
        IM-->>LS: init resolves
    and
        Note over LS: timeout 5000ms rejects "Innovation Manager initialization timeout"<br/>AppInitializer.tsx:34-36
    end
    LS->>LS: Promise.allSettled(serviceLoaders) - rejection logged as warn, non-fatal<br/>AppInitializer.tsx:49-60

    AI->>WP: graphWorkerProxy.initialize()<br/>AppInitializer.tsx:95
    AI->>GDM: graphDataManager.ensureWorkerReady()<br/>AppInitializer.tsx:98
    alt worker init throws or workerReady false
        AI->>WES: setWorkerError('graph visualization engine failed to initialize', details)<br/>AppInitializer.tsx:117-120
        Note over AI: details narrows to "SharedArrayBuffer is not available" when<br/>typeof SharedArrayBuffer === 'undefined' - AppInitializer.tsx:111-112
        AI->>AI: continue without fully initialized worker (non-fatal)<br/>AppInitializer.tsx:122-123
    end
    AI->>WES: setRetryHandler(() => initializeWorker())<br/>AppInitializer.tsx:127-132
    AI->>Set: await initialize()<br/>AppInitializer.tsx:139
    Note over AI: applies system.debug settings: debugState.enableDebug/enableDataDebug/<br/>enablePerformanceDebug - AppInitializer.tsx:143-156
    par WebSocket connect (non-blocking of data fetch)
        AI->>WS: initializeWebSocket(settings)<br/>AppInitializer.tsx:165
        alt websocketService.connect() throws
            Note over AI: caught, logged, non-fatal - AppInitializer.tsx:167-169
        end
    and Initial data fetch
        AI->>GDM: graphDataManager.fetchInitialData()<br/>AppInitializer.tsx:177
        alt fetch throws
            AI->>GDM: setGraphData({nodes:[],edges:[]})<br/>AppInitializer.tsx:182
        end
    end
    AI->>AI: await Promise.all([wsPromise,dataPromise])<br/>AppInitializer.tsx:187
    alt outer try succeeds
        AI->>App: onInitialized()<br/>AppInitializer.tsx:190
    else outer try throws (e.g. Set.initialize rejects)
        AI->>App: onError(error)<br/>AppInitializer.tsx:195
    end
    Note over AI: INVARIANT: subscribe_position_updates sent exactly ONCE per established<br/>connection via hasSubscribedToPositions module flag, cleared on disconnect<br/>AppInitializer.tsx:65-72,277-279
```
## VC-30.3 Zustand store catalogue
```mermaid
classDiagram
    class useSettingsStore {
      client/src/store/settingsStore.ts:26
      persist middleware name="graph-viz-settings-v2"
      storage localStorage via createJSONStorage settingsStore.ts:35
      partialize authenticated,user,isPowerUser,partialSettings settingsStore.ts:36-45
      merge() deep-merges persisted.partialSettings over current settingsStore.ts:46-68
    }
    class CoreSlice {
      partialSettings DeepPartial~Settings~ coreSlice.ts:67
      settings DeepPartial~Settings~ coreSlice.ts:68
      loadedPaths Set~string~ coreSlice.ts:69
      loadingSections Set~string~ coreSlice.ts:70
      initialized bool coreSlice.ts:71
      authenticated bool coreSlice.ts:72
      settingsSyncEnabled bool coreSlice.ts:75
      +initialize() coreSlice.ts:82
      +get(path) coreSlice.ts:194
      +set(path,value,skipServerSync) coreSlice.ts:226
      +subscribe(path,cb,immediate) coreSlice.ts:255
      +ensureLoaded(paths) coreSlice.ts:269
      +loadSection(section) coreSlice.ts:329
      +updateSettings(updater) coreSlice.ts:365
      +updateComputeMode(mode) coreSlice.ts:436
      +updateConstraints(constraints) coreSlice.ts:447
    }
    class PhysicsSlice {
      +updatePhysics(graphName,params) physicsSlice.ts:19
      +updateWarmupSettings(settings) physicsSlice.ts:122
      +notifyPhysicsUpdate() no-op since 2026-06-03 physicsSlice.ts:133-143
      +updateTweening(graphName,params) physicsSlice.ts:145
      +notifyTweeningUpdate(params) dispatches window CustomEvent tweeningSettingsUpdated physicsSlice.ts:190
    }
    class PersistenceSlice {
      +getByPath(path) persistenceSlice.ts:25
      +setByPath(path,value) persistenceSlice.ts:36
      +batchUpdate(updates) persistenceSlice.ts:48
      +flushPendingUpdates() persistenceSlice.ts:70
      +resetSettings() persistenceSlice.ts:74
      +exportSettings() persistenceSlice.ts:93
      +importSettings(json) persistenceSlice.ts:114
    }
    class SubscriberTrie {
      subscriberTrieRoot SubscriberTrieNode subscriberTrie.ts:13
      +getOrCreateTrieNode(path,create) subscriberTrie.ts:18
      +collectDescendants(node,out) subscriberTrie.ts:34
      +collectMatchedCallbacks(changedPaths) subscriberTrie.ts:41
      +scheduleNotify(callbacks) requestAnimationFrame batch subscriberTrie.ts:78
    }
    class useWebSocketStore {
      client/src/store/websocketStore.ts:9-16 re-export shim (36 lines)
      real impl client/src/store/websocket/index.ts:516 lines
      socket WebSocket~null~ storeState.ts:48
      isConnected bool storeState.ts:49
      isServerReady bool storeState.ts:50
      connectionState ConnectionState storeState.ts:51
      solidSubscriptions Map~string,Set~ storeState.ts:55
      nodeTypeMap Map~number,NodeType~ storeState.ts:56
      messageQueue QueuedMessage[] storeState.ts:57
      statistics WebSocketStatistics storeState.ts:31-44
      reconnectInterval=1000 maxReconnectAttempts=10 storeState.ts:69-71
    }
    class useTimelineStore {
      client/src/store/timelineStore.ts:88
      active bool timelineStore.ts:91
      diffMode bool timelineStore.ts:92
      t,t1,t2 string~null~ timelineStore.ts:93-95
      domainMinMs,domainMaxMs number timelineStore.ts:96-97
      validSubjects,addedSubjects,retractedSubjects,knownSubjects Set~string~ timelineStore.ts:98-101
      +loadStateAt(t,recordedAsOf) timelineStore.ts:105
      +loadDiff(t1,t2,recordedAsOf) timelineStore.ts:132
      +setDiffMode(on) timelineStore.ts:173
      +reset() timelineStore.ts:187
      DEFAULT_DOMAIN_LOOKBACK_MS = 30d timelineStore.ts:33
    }
    class useTransientBeamStore {
      client/src/store/transientBeamStore.ts:64
      beams TransientBeam[] transientBeamStore.ts:65
      +pushBeams(events) transientBeamStore.ts:67
      +pruneExpired() transientBeamStore.ts:90
      +clear() transientBeamStore.ts:103
      DEFAULT_BEAM_DURATION_MS=1500 MAX_TRANSIENT_BEAMS=256 MIN_BEAM_DURATION_MS=400<br/>transientBeamStore.ts:23-27
    }
    class useWorkerErrorStore {
      client/src/store/workerErrorStore.ts:23
      hasWorkerError bool workerErrorStore.ts:24
      transientErrorCount number workerErrorStore.ts:27
      retryWorkerInit fn~null~ workerErrorStore.ts:28
      +setWorkerError(msg,details) workerErrorStore.ts:30
      +recordTransientError(context) workerErrorStore.ts:36
      +resetTransientErrors() workerErrorStore.ts:50
      +setRetryHandler(handler) workerErrorStore.ts:74
      TRANSIENT_THRESHOLD=30 workerErrorStore.ts:21
    }
    class AutoSaveManager {
      client/src/store/autoSaveManager.ts:13 singleton autoSaveManager.ts:152
      pendingChanges Map~string,any~ autoSaveManager.ts:14
      DEBOUNCE_DELAY=500ms autoSaveManager.ts:18
      CLIENT_ONLY_PATHS auth.nostr.connected,auth.nostr.publicKey autoSaveManager.ts:34-37
      +queueChange(path,value) autoSaveManager.ts:52
      +queueChanges(changes) autoSaveManager.ts:73
      +forceFlush() autoSaveManager.ts:103
      -flushPendingChanges() calls settingsApi.updateSettingsByPaths autoSaveManager.ts:112
    }
    class SettingsRetryManager {
      client/src/store/settingsRetryManager.ts:20 singleton settingsRetryManager.ts:203
      maxRetries=3 baseRetryDelay=1000ms maxRetryDelay=30000ms settingsRetryManager.ts:23-25
      +addFailedUpdate(path,value,error) settingsRetryManager.ts:32
      -processRetryQueue() polled every 5000ms settingsRetryManager.ts:144,55
      -calculateRetryDelay(attempts) exp backoff + jitter settingsRetryManager.ts:131
      dispatches window CustomEvent settings-retry-failed on maxRetries exhausted settingsRetryManager.ts:123-125
    }
    useSettingsStore *-- CoreSlice
    useSettingsStore *-- PhysicsSlice
    useSettingsStore *-- PersistenceSlice
    CoreSlice --> SubscriberTrie : subscribe and updateSettings walk trie
    CoreSlice --> AutoSaveManager : set and updateSettings queueChange coreSlice L247 L395
    PersistenceSlice --> AutoSaveManager : setByPath direct settingsApi write persistenceSlice L43
    AutoSaveManager --> SettingsRetryManager : flush failure delegates autoSaveManager L130-135
    useTimelineStore ..> useTransientBeamStore : independent, no direct coupling
```
## VC-30.4 Settings subscriber trie, selective hooks and autosave/retry pipeline
```mermaid
sequenceDiagram
    autonumber
    participant Comp as Component<br/>useSelectiveSetting/useSettingsSubscription<br/>client/src/hooks/useSelectiveSettingsStore.ts:14,102
    participant Store as useSettingsStore.updateSettings<br/>client/src/store/settings/coreSlice.ts:365
    participant Trie as subscriberTrie<br/>client/src/store/settings/subscriberTrie.ts
    participant ASM as autoSaveManager<br/>client/src/store/autoSaveManager.ts:13
    participant API as settingsApi.updateSettingsByPaths<br/>client/src/store/autoSaveManager.ts:124
    participant Retry as settingsRetryManager<br/>client/src/store/settingsRetryManager.ts:20

    Comp->>Store: useSettingsStore(state => state.get(path))<br/>useSelectiveSettingsStore.ts:22-25 (Zustand selector memoisation)
    Comp->>Store: subscribe(path, callback, immediate=false)<br/>useSelectiveSettingsStore.ts:131
    Store->>Trie: getOrCreateTrieNode(path).subscribers.add(callback)<br/>coreSlice.ts:256, subscriberTrie.ts:18
    Note over Comp,Store: immediate=true fires callback synchronously against get().initialized<br/>coreSlice.ts:258-259

    Store->>Store: produce(partialSettings,updater) via immer, findChangedPaths()<br/>coreSlice.ts:368-370
    alt changedPaths.length == 0
        Store-->>Comp: no-op return
    end
    Store->>ASM: autoSaveManager.queueChanges(batchChanges)<br/>coreSlice.ts:395
    alt !isInitialized
        ASM-->>Store: DROPPED (not initialized)<br/>autoSaveManager.ts:74-77
    else path in CLIENT_ONLY_PATHS (auth.nostr.*)
        ASM-->>Store: skipped, never leaves client<br/>autoSaveManager.ts:81-83
    else
        ASM->>ASM: pendingChanges.set(path,value), scheduleFlush()<br/>debounce 500ms via clearTimeout/setTimeout<br/>autoSaveManager.ts:86-99
    end
    Store->>Trie: collectMatchedCallbacks(changedPaths)<br/>coreSlice.ts:433, subscriberTrie.ts:41
    Trie->>Trie: walk ancestor prefixes + collectDescendants(exactNode)<br/>subscriberTrie.ts:44-59
    Store->>Trie: scheduleNotify(matched) - adds to pendingNotifyCallbacks,<br/>one requestAnimationFrame per batch<br/>subscriberTrie.ts:78-83
    Trie->>Comp: flushNotifyCallbacks() invokes each callback, errors caught+logged<br/>subscriberTrie.ts:67-75

    Note over ASM: after DEBOUNCE_DELAY=500ms, flushPendingChanges() fires<br/>autoSaveManager.ts:97-99,112
    ASM->>API: settingsApi.updateSettingsByPaths(updates)<br/>autoSaveManager.ts:124
    alt API resolves
        API-->>ASM: SUCCESS - pendingChanges already cleared pre-send<br/>autoSaveManager.ts:118-125
    else API rejects
        ASM->>Retry: settingsRetryManager.addFailedUpdate(path,value,error.message)<br/>per failed update, autoSaveManager.ts:130-135
        Retry->>Retry: retryQueue.set(path,{attempts,lastAttempt,error})<br/>settingsRetryManager.ts:40-46
        Retry->>Retry: startRetryProcessor() window.setInterval 5000ms<br/>settingsRetryManager.ts:141-148
        loop every 5000ms while retryQueue non-empty
            Retry->>Retry: calculateRetryDelay(attempts)=min(1000*2^(attempts-1),30000)+jitter<br/>settingsRetryManager.ts:131-137
            alt now - lastAttempt >= delay
                Retry->>API: settingsApi.updateSettingsByPaths (batch) or updateSettingByPath (single)<br/>settingsRetryManager.ts:86,106
                alt retry succeeds
                    Retry->>Retry: retryQueue.delete(path)<br/>settingsRetryManager.ts:90,107
                else attempts >= maxRetries=3
                    Retry->>Retry: dispatchEvent 'settings-retry-failed' {path,value,error}<br/>settingsRetryManager.ts:123-125
                end
            end
        end
        Retry->>Retry: stopRetryProcessor() when retryQueue.size==0<br/>settingsRetryManager.ts:57-59
    end
```
## VC-30.5 WebSocketRegistry and WebSocketEventBus fan-out across three sockets
```mermaid
sequenceDiagram
    autonumber
    participant Graph as websocket/index.ts connect()<br/>client/src/store/websocket/index.ts:171
    participant Voice as VoiceWebSocketService<br/>client/src/services/VoiceWebSocketService.ts:87
    participant Solid as solidWebSocket store adapter<br/>client/src/store/websocket/solidWebSocket.ts:71
    participant Pod as podNotificationManager<br/>client/src/services/solidPod/podNotifications.ts:92
    participant Reg as webSocketRegistry<br/>client/src/services/WebSocketRegistry.ts:22
    participant Bus as webSocketEventBus<br/>client/src/services/WebSocketEventBus.ts:39
    participant Bots as BotsWebSocketIntegration<br/>client/src/features/bots/services/BotsWebSocketIntegration.ts:53

    Note over Reg: connections Map~string,RegistryEntry~ keyed by name - WebSocketRegistry.ts:23
    Graph->>Reg: register("graph", url, socket)<br/>websocket/index.ts:171
    Voice->>Reg: register(REGISTRY_NAME="voice", url, socket)<br/>VoiceWebSocketService.ts:87
    Solid->>Pod: connect() - the adapter registers nothing of its own<br/>solidWebSocket.ts:81
    Pod->>Reg: register(REGISTRY_NAME="solid-pod", url, socket)<br/>podNotifications.ts:20,92
    alt name already registered
        Reg->>Reg: unregister(name) first - removes old listeners, prevents orphaned handlers<br/>WebSocketRegistry.ts:33-35
    end
    Reg->>Reg: attach open/close/error listeners updating state label<br/>readyStateLabel() CONNECTING|OPEN|CLOSING|CLOSED|unknown<br/>WebSocketRegistry.ts:44-58,150-163
    Reg->>Bus: emit("registry:registered",{name,url})<br/>WebSocketRegistry.ts:63

    par each socket also emits directly
        Graph->>Bus: emit("connection:open",{name:"graph",url})<br/>websocket/index.ts:172
    and
        Voice->>Bus: emit("connection:open",{name:"voice",url})<br/>VoiceWebSocketService.ts:88
    and
        Pod->>Bus: emit("connection:open",{name:"solid-pod",url})<br/>podNotifications.ts:93
        Pod-->>Solid: onLifecycle{type:"open"} -> isSolidConnected, emit("solid-connected")<br/>podNotifications.ts:94, solidWebSocket.ts:36-41
    end
    Note over Solid,Pod: INVARIANT: ADR-2100 - the store adapter is NOT a fourth registrant. It opened its own<br/>socket to the same VITE_JSS_WS_URL and registered as "solid-store" with a 10-attempt ladder -<br/>both are deleted. One socket, one registry entry ("solid-pod"), one 5-attempt ladder - see VC-32.16

    Bots->>Bus: emit("message:bots",{data:message})<br/>BotsWebSocketIntegration.ts:53
    Voice->>Bus: emit("message:voice",{data:message})<br/>VoiceWebSocketService.ts:141
    Pod->>Bus: emit("message:pod",{data:msg})<br/>podNotifications.ts:99
    Note over Bus: RESOLVED ADR-2077: the message:graph event type is DELETED. It was<br/>declared in the union and payload map but never emitted, so a switch over<br/>the bus events is now exhaustive over events that actually fire.

    Bus->>Bus: on(event,handler) adds to handlers Map~string,Set~Handler~~<br/>WebSocketEventBus.ts:47-54, returns unsubscribe closure calling off()
    Bus->>Bus: emit(event,data) iterates handlers Set, try/catch per handler,<br/>logs and continues on throw - WebSocketEventBus.ts:80-93

    alt connection error (any socket)
        Voice->>Bus: emit("connection:error",{name,error})<br/>VoiceWebSocketService.ts:113
        Graph->>Bus: emit("connection:error",{name:"graph",error:errorMessage})<br/>websocket/index.ts:248
    end
    alt connection close (any socket)
        Graph->>Reg: unregister("graph")<br/>websocket/index.ts:219,278
        Reg->>Reg: removeEventListener for each tracked listener, delete from connections<br/>WebSocketRegistry.ts:74-79
        Reg->>Bus: emit("registry:unregistered",{name})<br/>WebSocketRegistry.ts:81
        Graph->>Bus: emit("connection:close",{name,code,reason})<br/>websocket/index.ts:220
    end
    Note over Reg: RESOLVED ADR-2077: closeAll() is DELETED - it had zero callers.<br/>register, unregister, get, getEntry, getAll, size and readyStateLabel<br/>remain. Nothing can mass-close sockets. no caller ever wanted that.
```
## VC-30.6 platformManager — UA sniff, XR capability probe, event dispatch
```mermaid
sequenceDiagram
    autonumber
    participant Caller as PlatformManager.initialize<br/>client/src/services/platformManager.ts:348
    participant Store as usePlatformStore<br/>client/src/services/platformManager.ts:72
    participant Nav as navigator.xr<br/>client/src/services/platformManager.ts:105
    participant Win as window resize listener<br/>client/src/services/platformManager.ts:146-150

    Caller->>Store: initialize()<br/>platformManager.ts:98
    Store->>Store: detectPlatform()<br/>platformManager.ts:102,167
    Store->>Store: userAgent = navigator.userAgent<br/>platformManager.ts:168
    alt userAgent includes "Quest"
        alt includes "Quest 3"
            Store->>Store: platform="quest3" xrDeviceType="quest"<br/>platformManager.ts:174-175
        else includes "Quest 2"
            Store->>Store: platform="quest2"<br/>platformManager.ts:176-177
        else
            Store->>Store: platform="quest"<br/>platformManager.ts:178-179
        end
    else userAgent includes "Pico"|"PICO"
        Store->>Store: platform="pico" xrDeviceType="pico"<br/>platformManager.ts:184-187
    else /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i test<br/>platformManager.ts:189
        Store->>Store: platform="mobile" xrDeviceType="mobile-xr"<br/>platformManager.ts:190-192
    else
        Store->>Store: platform="desktop" xrDeviceType="desktop-xr"<br/>platformManager.ts:195-197
    end
    Note over Store: performanceTier/maxTextureSize/memoryLimited per platform<br/>quest3: high,4096,true - quest2: medium,2048,true - quest: low,2048,true<br/>pico: medium,2048,true - mobile: low,2048,true - desktop: high,8192,false<br/>platformManager.ts:204-235
    Store->>Store: hasTouchscreen via navigator.maxTouchPoints or ontouchstart<br/>platformManager.ts:238-239
    alt prevPlatform !== platform
        Store->>Store: dispatchEvent("platformchange",{platform})<br/>platformManager.ts:260-261
    end

    alt navigator.xr exists
        Caller->>Nav: navigator.xr.isSessionSupported("immersive-vr")<br/>platformManager.ts:108
        Caller->>Nav: navigator.xr.isSessionSupported("immersive-ar")<br/>platformManager.ts:110
        Nav-->>Store: vrSupported, arSupported
        Store->>Store: capabilities.xrSupported = vrSupported||arSupported<br/>platformManager.ts:112-120
        alt isSessionSupported throws
            Note over Caller: caught, logged, capabilities keep prior defaults<br/>platformManager.ts:123-125
        end
        Store->>Store: capabilities.handTrackingSupported = isQuest()<br/>platformManager.ts:132-139
    else navigator.xr undefined
        Note over Store: isWebXRSupported=false, xrSupported stays false (init default)<br/>platformManager.ts:44,92
    end

    Caller->>Win: window.addEventListener("resize", () => detectPlatform())<br/>platformManager.ts:146-150
    Caller->>Store: set({initialized:true, isWebXRSupported})<br/>platformManager.ts:153-158

    Note over Store: setXRMode(enabled) dispatches "xrmodechange" only on actual change<br/>platformManager.ts:288-295
    Note over Store: setXRSessionState(state) dispatches "xrsessionstatechange"<br/>states: inactive|starting|active|ending|error<br/>platformManager.ts:5,297-304
    Store->>Store: dispatchEvent(event,data) iterates listeners.get(event) Set,<br/>try/catch per callback<br/>platformManager.ts:307-318
    Note over Caller: PlatformManager class (platformManager.ts:332) is a backwards-compat<br/>singleton adapter (getInstance) wrapping usePlatformStore.getState() calls -<br/>exported as platformManager, platformManager.ts:394
```
## VC-30.7 remoteLogger — console interception, buffering, batch POST, failure path
```mermaid
sequenceDiagram
    autonumber
    participant Mod as module load<br/>client/src/services/remoteLogger.ts:314
    participant RL as RemoteLogger<br/>client/src/services/remoteLogger.ts:14
    participant Con as console.log/debug/info/warn/error<br/>client/src/services/remoteLogger.ts:49
    participant Timer as flushTimer setInterval<br/>client/src/services/remoteLogger.ts:139
    participant Srv as POST serverEndpoint<br/>client/src/services/remoteLogger.ts:179

    Mod->>RL: new RemoteLogger()<br/>remoteLogger.ts:24,314
    RL->>RL: enabled = !VITE_REMOTE_LOGGING_DISABLED<br/>remoteLogger.ts:26-27
    RL->>RL: serverEndpoint = VITE_API_URL + "/api/client-logs"<br/>remoteLogger.ts:31-33
    alt enabled
        RL->>Timer: startFlushTimer() setInterval every flushInterval=1000ms<br/>remoteLogger.ts:35-36,139-147
        RL->>Con: interceptConsole() wraps log/debug/info/warn/error,<br/>calls originalConsole then this.log(level,'console',...)<br/>remoteLogger.ts:37,49-83
    end
    RL->>RL: window.addEventListener("beforeunload", () => flush(true))<br/>remoteLogger.ts:41-45

    Con->>RL: log(level,'console',formatArgs(args)[,extractStack(args) for error])<br/>remoteLogger.ts:59-82
    RL->>RL: buffer.push(entry) - entry carries userAgent,url,timestamp,stack,data<br/>remoteLogger.ts:110-130
    alt buffer.length >= maxBufferSize=50
        RL->>RL: flush() immediately<br/>remoteLogger.ts:133-135,17
    end

    loop every flushInterval=1000ms while buffer non-empty
        Timer->>RL: flush()<br/>remoteLogger.ts:142-146
    end
    RL->>RL: logs = buffer.splice, payload={logs,sessionId,timestamp}<br/>remoteLogger.ts:161-169
    alt sync=true (beforeunload path)
        RL->>Srv: navigator.sendBeacon(serverEndpoint, blob) if available<br/>remoteLogger.ts:173-176
    else async flush
        RL->>Srv: fetch(serverEndpoint, POST, Content-Type application-json)<br/>remoteLogger.ts:179-185
        alt response not ok
            RL->>RL: consecutiveFailures++<br/>remoteLogger.ts:188
            alt status==404 && consecutiveFailures >= maxConsecutiveFailures=3
                RL->>RL: setEnabled(false) - auto-disable, endpoint unavailable<br/>remoteLogger.ts:189-192,220-228
            else status!=404
                RL->>RL: console.warn "Failed to send logs" (non-recursive - raw console, not intercepted)<br/>remoteLogger.ts:193-194
            end
        else response ok
            RL->>RL: consecutiveFailures = 0<br/>remoteLogger.ts:197
        end
        alt fetch throws
            RL->>RL: buffer = logs.concat(buffer) - re-queue for next flush<br/>remoteLogger.ts:200-206
        end
    end
    Note over RL: sessionId persisted in sessionStorage key remote-logger-session,<br/>format session-{Date.now()}-{random36}<br/>remoteLogger.ts:210-217

    Mod->>RL: setTimeout(() => logXRInfo(), 1000) on module load<br/>remoteLogger.ts:320-324
    RL->>RL: logXRInfo(): webXRSupported='xr' in navigator, isQuestDevice via<br/>/OculusBrowser|Quest/i test, questVersion via /Quest\s*(\d+)?/i match<br/>remoteLogger.ts:271-309
    opt navigator.xr present
        RL->>RL: isSessionSupported('immersive-vr'/'immersive-ar') logged as<br/>separate 'xr-capabilities' entries<br/>remoteLogger.ts:281-295
    end
```
## VC-30.8 api layer — two parallel HTTP stacks and their interceptor chains
```mermaid
sequenceDiagram
    autonumber
    participant Mod as main.tsx bootstrap<br/>client/src/app/main.tsx:28-29
    participant UAC as unifiedApiClient<br/>client/src/services/api/UnifiedApiClient.ts:68,466
    participant AuthI as authInterceptor<br/>client/src/services/api/authInterceptor.ts:41,93
    participant Nostr as nostrAuth<br/>client/src/services/nostrAuthService.ts
    participant Fetch as window.fetch<br/>UnifiedApiClient.ts:226
    participant Ax as axios (global instance)<br/>client/src/api/settings/endpoints.ts:4,36

    Mod->>UAC: initializeAuthInterceptor(unifiedApiClient)<br/>main.tsx:28, authInterceptor.ts:103-110
    UAC->>UAC: setInterceptors({onRequest:authRequestInterceptor,<br/>onResponse:authResponseInterceptor})<br/>authInterceptor.ts:104-107, UnifiedApiClient.ts:116-118

    Note over UAC: request(method,url,data,config) sets body via JSON.stringify<br/>for POST/PUT/PATCH unless string/ArrayBuffer/FormData<br/>UnifiedApiClient.ts:316-343
    UAC->>UAC: executeWithRetry(url,config,attempt=0)<br/>UnifiedApiClient.ts:151
    UAC->>AuthI: interceptors.onRequest(finalConfig,fullUrl)<br/>UnifiedApiClient.ts:186-188
    AuthI->>AuthI: headers['X-Request-ID']=uuidv4()<br/>authInterceptor.ts:50-51
    alt nostrAuth.isAuthenticated() && isDevMode()
        AuthI->>AuthI: Authorization: Bearer dev-session-token<br/>+ X-Nostr-Pubkey<br/>authInterceptor.ts:60-63
    else authenticated, not dev mode
        AuthI->>Nostr: signRequest(fullUrl,method,body) NIP-98<br/>authInterceptor.ts:73
        Nostr-->>AuthI: token
        AuthI->>AuthI: Authorization: Nostr {token}<br/>authInterceptor.ts:74
    else not authenticated
        Note over AuthI: no auth headers sent<br/>authInterceptor.ts:80-81
    end
    UAC->>Fetch: fetch(fullUrl,{headers,signal:abortController.signal})<br/>UnifiedApiClient.ts:211-226
    Note over UAC: per-request AbortController, setTimeout(timeoutMs=DEFAULT_TIMEOUT=30000ms)<br/>aborts on timeout<br/>UnifiedApiClient.ts:202-209
    alt fetch throws AbortError
        UAC->>UAC: createApiError('Request timeout or cancelled',0,'Timeout')<br/>UnifiedApiClient.ts:231-236
    else fetch throws other
        UAC->>UAC: createApiError('Network error: ...',0,'Network Error')<br/>UnifiedApiClient.ts:239-244
    end
    alt !response.ok
        UAC->>AuthI: interceptors.onError(apiError)<br/>UnifiedApiClient.ts:288-294
        AuthI->>AuthI: authResponseInterceptor: warnIfServerInReleaseMode(status,sentDevToken)<br/>one-shot console.warn on 401+dev-token (ADR-06 sD1)<br/>authInterceptor.ts:26-39,93-101
        alt retryCondition true (attempt<3, status>=500 or 0, not 401/403)
            UAC->>UAC: delay = retryDelay(1000) * 2^attempt, retry<br/>UnifiedApiClient.ts:47-58,170-173
        else
            UAC-->>Mod: throw apiError
        end
    else response.ok
        UAC->>AuthI: interceptors.onResponse(apiResponse)<br/>UnifiedApiClient.ts:301-303
        UAC-->>Mod: ApiResponse~T~{data,status,statusText,headers}
    end

    rect rgb(252,242,225)
    Note over Ax: RESOLVED ADR-2074: settingsApi still uses its own global axios instance<br/>(a transport choice), but its interceptor no longer reimplements the auth<br/>branch - it calls the shared computeAuthHeaders from authInterceptor.ts:52.<br/>All four HTTP transports now share one signing helper. endpoints.ts:37
    Ax->>Ax: axios.interceptors.request.use: dev-session-token or<br/>signRequest() NIP-98, same branching as authInterceptor.ts<br/>endpoints.ts:36-61
    Ax->>Ax: axios.get/put(`${API_BASE}/api/settings/...`)<br/>API_BASE='' - relative, proxied by Vite/nginx<br/>endpoints.ts:30,75-76,90-91
    end

    Note over Mod: Endpoint modules on UnifiedApiClient: analyticsApi.ts (AnalyticsAPI class),<br/>constraintsApi.ts, exportApi.ts (exportGraph/shareGraph), graphExpandApi.ts<br/>(fetchNodeRelations/expandNode), layoutApi.ts, workspaceApi.ts (WorkspaceApiError)<br/>client/src/api/analyticsApi.ts:123, constraintsApi.ts:86, exportApi.ts:69,106,<br/>graphExpandApi.ts:69,84, layoutApi.ts:47, workspaceApi.ts:62,80,136
```
## VC-30.9 graphDataManager — fetchInitialData, setGraphData, id maps, cache
```mermaid
sequenceDiagram
    autonumber
    participant Caller as AppInitializer<br/>client/src/app/AppInitializer.tsx:177
    participant GDM as GraphDataManager<br/>client/src/features/graph/managers/graphDataManager.ts:25
    participant Rest as fetchGraphData<br/>client/src/features/graph/managers/dataManager/restClient.ts
    participant Gate as dropLinkedPageStubs<br/>client/src/features/graph/managers/dataManager/nodeUtils.ts
    participant Topo as buildNodeIdMaps/setDataAndNotify<br/>client/src/features/graph/managers/dataManager/topology.ts
    participant WP as graphWorkerProxy<br/>client/src/features/graph/managers/graphDataManager.ts:6

    Caller->>GDM: fetchInitialData()<br/>graphDataManager.ts:195
    GDM->>GDM: includeLinkedPages = settings.nodeFilter.includeLinkedPages ?? false<br/>graphDataManager.ts:198-199
    GDM->>Rest: fetchGraphData(graphType,graphTypeFilter,!includeLinkedPages)<br/>graphDataManager.ts:200-204
    Rest->>Rest: normalise edge.source/target via String(), node.id via String()<br/>restClient.ts:107,121
    Rest-->>GDM: GraphData{nodes,edges}
    GDM->>GDM: setGraphData(validatedData)<br/>graphDataManager.ts:206,227
    alt currentData.nodes.length == 0
        GDM->>GDM: scheduleEmptyDataRetry(1,retryTimeout,fetchInitialData,...)<br/>T6 fix - backend up but Oxigraph empty<br/>graphDataManager.ts:212-219
    end
    GDM-->>Caller: return currentData (lastGraphData ?? validatedData)<br/>graphDataManager.ts:208,222

    Note over GDM: setGraphData(data) - population gate + id-map build + cache + worker delivery<br/>graphDataManager.ts:227
    GDM->>Gate: dropLinkedPageStubs(data,includeLinkedPages)<br/>strips ~14.7k of 17.1k linked_page wikilink stubs<br/>graphDataManager.ts:240
    GDM->>GDM: nodes.map(ensureNodeHasValidPosition)<br/>graphDataManager.ts:244-248
    GDM->>Topo: buildNodeIdMaps(nodes,nodeIdMap,reverseNodeIdMap)<br/>graphDataManager.ts:256
    GDM->>Topo: setDataAndNotify(validatedData,lastGraphDataHash,listeners,setter)<br/>ADR-03 D5 single cached delivery path<br/>graphDataManager.ts:259-267
    Topo-->>GDM: onGraphDataChange listeners fire with validatedData
    alt graphWorkerProxy.isReady()
        GDM->>WP: setGraphTopology(validatedData)<br/>ADR-03 D7 - worker gets topology directly from the manager,<br/>NOT from any React state<br/>graphDataManager.ts:270-272
        alt setGraphTopology throws
            Note over GDM: caught, logged warn, topology delivery to worker skipped this pass<br/>graphDataManager.ts:273-275
        end
    end

    Note over GDM: mergeGraphData(newNodes,newEdges,anchorNodeId,anchorPosition) - additive expansion,<br/>seeds new nodes on golden-angle ring around LIVE anchor position (worker/SAB,<br/>not stale cached node.position), dedups nodes/edges by String()-coerced id<br/>graphDataManager.ts:333-406
    Note over GDM: getCachedGraphData() always returns null by design - worker data is<br/>async-only, callers must use fallback positioning<br/>graphDataManager.ts:125-128
```
## VC-30.10 graphDataManager binary path and the dataWithPositions forwarding invariant
```mermaid
sequenceDiagram
    autonumber
    participant WS as websocketService.onBinaryMessage<br/>client/src/app/AppInitializer.tsx:210
    participant GDM as GraphDataManager.updateNodePositions<br/>client/src/features/graph/managers/graphDataManager.ts:433
    participant Heal as maybeSelfHealUnknownNodes<br/>client/src/features/graph/managers/graphDataManager.ts:451
    participant Frame as handleBinaryFrame<br/>client/src/features/graph/managers/dataManager/wsClient.ts:19
    participant WP as graphWorkerProxy.processBinaryFrame<br/>client/src/features/graph/managers/dataManager/wsClient.ts:45
    participant Sub as useGraphDataSubscription<br/>client/src/features/graph/hooks/useGraphDataSubscription.ts:23
    participant GM as GraphManager<br/>client/src/features/graph/components/GraphManager.tsx:310

    WS->>GDM: updateNodePositions(positionData: ArrayBuffer)<br/>graphDataManager.ts:433
    GDM->>Heal: maybeSelfHealUnknownNodes(positionData) - sampled every 120 frames<br/>graphDataManager.ts:435,451-452
    opt sample frame && nodeIdMap non-empty && no active quality/authority filter
        Heal->>Heal: parseBinaryNodeData, check reverseNodeIdMap.has(getActualNodeId(id))<br/>graphDataManager.ts:464-465
        alt unknown ids found && cooldown (30000ms) elapsed
            Heal->>GDM: fetchInitialData() - topology hash makes false-positive refetch a no-op<br/>graphDataManager.ts:461-471
        end
    end
    GDM->>Frame: handleBinaryFrame(positionData,lastBinaryUpdateTime,onUpdateTime)<br/>graphDataManager.ts:436-440
    alt now - lastUpdateTime < 16ms
        Frame-->>GDM: throttled, skip (~60fps cap)<br/>wsClient.ts:27
    else
        Frame->>WP: processBinaryFrame(new Uint8Array(positionData)) - zero-copy transfer,<br/>newest-wins discipline inside the worker (ADR-03 D7)<br/>wsClient.ts:44-46
        Frame->>Frame: resetTransientErrors() on success<br/>wsClient.ts:47
        alt processBinaryFrame throws
            Frame->>Frame: recordTransientError('updateNodePositions')<br/>wsClient.ts:64-65
        end
    end

    rect rgb(230,250,230)
    Note over Sub,GM: INVARIANT (historical bug, now fixed): useGraphDataSubscription's<br/>handleGraphUpdate builds dataWithPositions (String()-coerced ids,<br/>getPositionForNode() fallback for (0,0,0) nodes, edge source/target<br/>recovery) and the caller MUST forward THAT object, not the original<br/>data, onward - useGraphDataSubscription.ts:69-100,117
    Sub->>Sub: onGraphData(dataWithPositions)<br/>useGraphDataSubscription.ts:117
    Sub->>GM: setGraphData(dataWithPositions) via onGraphData callback<br/>GraphManager.tsx:310-311
    Note over Sub: this React graphData state feeds RENDER only - the worker's topology<br/>comes from GraphDataManager.setGraphData -> graphWorkerProxy.setGraphTopology<br/>directly (VC-30.9), an entirely separate path from this hook
    end
    Note over Sub: 5000ms fallback timer seeds 3 synthetic nodes if no real data<br/>arrived by then (lastProcessedGraphRef still null)<br/>useGraphDataSubscription.ts:141-155
```
## VC-30.11 graph worker handoff — proxy init, SAB-vs-Comlink capability, dispose
```mermaid
sequenceDiagram
    autonumber
    participant AI as AppInitializer<br/>client/src/app/AppInitializer.tsx:95
    participant Proxy as GraphWorkerProxy<br/>client/src/features/graph/managers/graphWorkerProxy.ts:107
    participant Comlink as comlink wrap/transfer<br/>client/src/features/graph/managers/graphWorkerProxy.ts:33,161,248
    participant W as GraphWorker<br/>client/src/features/graph/workers/graph.worker.ts:16,571-572
    participant Test as sabOrTransferDetection.test.ts<br/>client/src/features/graph/__tests__/sabOrTransferDetection.test.ts:16

    Note over Proxy: WORKER_USES_SAB = SAB_CAPABLE && !FORCE_COMLINK, computed ONCE at<br/>module load - SAB_CAPABLE = typeof SharedArrayBuffer!=='undefined' &&<br/>self.crossOriginIsolated===true, FORCE_COMLINK = VITE_FORCE_COMLINK==='1'<br/>graphWorkerProxy.ts:71-82
    Test->>Proxy: import('../managers/graphWorkerProxy') under mocked globals<br/>via vi.resetModules() per case<br/>sabOrTransferDetection.test.ts:17-19,36,45,58
    alt crossOriginIsolated=false
        Test->>Test: expect(WORKER_USES_SAB).toBe(false)<br/>sabOrTransferDetection.test.ts:30-38
    else SharedArrayBuffer undefined
        Test->>Test: expect(WORKER_USES_SAB).toBe(false)<br/>sabOrTransferDetection.test.ts:40-50
    else both SAB and crossOriginIsolated true
        Test->>Test: expect(WORKER_USES_SAB).toBe(true)<br/>sabOrTransferDetection.test.ts:52-60
    end

    AI->>Proxy: graphWorkerProxy.initialize()<br/>graphWorkerProxy.ts:139
    Proxy->>Comlink: new Worker(new URL('../workers/graph.worker.ts',import.meta.url),<br/>{type:'module'})<br/>graphWorkerProxy.ts:152-155
    Proxy->>Comlink: workerApi = wrap~GraphWorkerType~(worker)<br/>graphWorkerProxy.ts:161
    Proxy->>W: workerApi.initialize() - handshake, resolves immediately<br/>graphWorkerProxy.ts:164, graph.worker.ts:103-106
    alt WORKER_USES_SAB
        Proxy->>Proxy: sharedBuffer = new SharedArrayBuffer(POSITION_BUFFER_BYTES)<br/>MAX_NODES=50000 * POSITION_FLOATS_PER_NODE=4 * 4 bytes<br/>graphWorkerProxy.ts:102-105,172-173
        Proxy->>W: workerApi.setupSharedPositions(sharedBuffer)<br/>graphWorkerProxy.ts:174, graph.worker.ts:199-203
        alt SAB attach throws
            Proxy->>Proxy: degrade to Comlink transfer path, sharedBuffer=null<br/>graphWorkerProxy.ts:178-185
        end
    end
    Proxy->>Proxy: isInitialized=true<br/>graphWorkerProxy.ts:188

    Note over Proxy: D7 surface is EXACTLY 4 methods - processBinaryFrame, getPositions,<br/>setGraphTopology, dispose. Legacy methods (tick,getGraphData,<br/>getAnalyticsBuffer,reheatSimulation,updateForcePhysicsSettings) throw<br/>deprecation shims - graphWorkerProxy.ts:1-15,356-392
    AI->>Proxy: setGraphTopology(graph)<br/>graphWorkerProxy.ts:308
    Proxy->>W: workerApi.setGraphData(graph)<br/>graphWorkerProxy.ts:312, graph.worker.ts:115
    W->>W: ensureNodeHasValidPosition per node, String()-coerce node.id,<br/>build nodeIdMap/reverseNodeIdMap/nodeIndexMap, edge adjacency maps<br/>graph.worker.ts:117,132-157
    W->>W: initPositionBuffers preserving old positions by nodeIndexMap match<br/>graph.worker.ts:159-166
    W->>W: syncToSharedBuffer(), graphDataLoaded=true<br/>graph.worker.ts:185,188
    opt pendingBinaryFrames queued (FIX 4 race guard)
        W->>W: replay each queued frame via processBinaryData()<br/>graph.worker.ts:189-196,263-267
    end

    rect rgb(235,235,252)
    Note over Proxy,W: processBinaryFrame single-flight (ADR-03 D2): _binaryFrameInFlight guard,<br/>_pendingLatest newest-wins slot - frames arriving mid-flight collapse to one<br/>graphWorkerProxy.ts:122-124,205-241
    Proxy->>Comlink: transfer(frame.buffer,[frame.buffer]) - neuters caller's ArrayBuffer<br/>graphWorkerProxy.ts:248
    alt WORKER_USES_SAB
        Proxy->>W: workerApi.processBinaryFrame(transferable) - returns void,<br/>renderer reads SAB view directly<br/>graphWorkerProxy.ts:250-255, graph.worker.ts:214-218
    else Comlink transfer mode
        Proxy->>W: workerApi.processBinaryFrame(transferable)<br/>graphWorkerProxy.ts:259-261
        W-->>Proxy: transfer(out,[out]) - full stride-3 currentPositions,<br/>NOT the stride-4 [nodeId,x,y,z] update array<br/>graph.worker.ts:219-223
        Proxy->>Proxy: lastTransferredView = new Float32Array(returned)<br/>graphWorkerProxy.ts:262-264
    end
    alt pending frame arrived during dispatch
        Proxy->>Proxy: queueMicrotask(() => processBinaryFrame(next)) - drains newest-wins slot<br/>graphWorkerProxy.ts:230-238
    end
    end

    Note over Proxy: getPositionsSync() - SAB mode returns sharedPositionView directly,<br/>Comlink mode returns lastTransferredView - called from useFrame per-frame<br/>render, avoids Promise allocation<br/>graphWorkerProxy.ts:294-299
    AI->>Proxy: dispose()<br/>graphWorkerProxy.ts:317
    Proxy->>W: worker.terminate() - no graceful worker-side RPC, hard terminate<br/>graphWorkerProxy.ts:318-321
    Proxy->>Proxy: clear sharedBuffer/sharedPositionView/lastTransferredView,<br/>reset _binaryFrameInFlight/_pendingLatest/isInitialized/initPromise<br/>graphWorkerProxy.ts:322-329
```
## VC-30.12 Store to component dependency graph
```mermaid
flowchart LR
    settingsStore["useSettingsStore<br/>client/src/store/settingsStore.ts:26"]
    websocketStore["useWebSocketStore<br/>client/src/store/websocketStore.ts:9"]
    timelineStore["useTimelineStore<br/>client/src/store/timelineStore.ts:88"]
    beamStore["useTransientBeamStore<br/>client/src/store/transientBeamStore.ts:64"]
    workerErrStore["useWorkerErrorStore<br/>client/src/store/workerErrorStore.ts:23"]

    GraphManager["GraphManager<br/>client/src/features/graph/components/GraphManager.tsx:44-61"]
    GraphCanvas["GraphCanvas<br/>client/src/features/graph/components/GraphCanvas.tsx:101"]
    NodeDetailPanel["NodeDetailPanel<br/>client/src/features/graph/components/NodeDetailPanel.tsx:61"]
    GemNodes["GemNodes<br/>client/src/features/graph/components/GemNodes.tsx"]
    GlassEdges["GlassEdges<br/>client/src/features/graph/components/GlassEdges.tsx"]
    TimelineScrubber["TimelineScrubber<br/>client/src/features/graph/components/TimelineScrubber.tsx:77-89"]
    ControlCenter["ControlCenter<br/>client/src/features/control-center/ControlCenter.tsx:21"]
    SettingField["useSettingField hook<br/>client/src/features/control-center/hooks/useSettingField.ts:50"]
    RevealSetting["useRevealSetting hook<br/>client/src/features/control-center/hooks/useRevealSetting.ts"]
    NostrAuthControl["NostrAuthControl<br/>client/src/features/control-center/primitives/NostrAuthControl.tsx"]
    StatusFlyout["StatusFlyout<br/>client/src/features/control-center/status/StatusFlyout.tsx"]
    BotsVisualization["BotsVisualization<br/>client/src/features/bots/components/BotsVisualization.tsx:36-38"]
    BotsNode["BotsNode<br/>client/src/features/bots/components/BotsNode.tsx"]
    WorkerErrorModal["WorkerErrorModal<br/>client/src/components/WorkerErrorModal.tsx:7"]
    AppInitializer["AppInitializer<br/>client/src/app/AppInitializer.tsx:117,127"]
    EmbeddingCloudLayer["EmbeddingCloudLayer<br/>client/src/features/visualisation/components/EmbeddingCloudLayer.tsx:110,254"]
    useTransientBeams["useTransientBeams hook<br/>client/src/features/visualisation/hooks/useTransientBeams.ts:25-26"]
    TransientBeamsLayer["TransientBeamsLayer<br/>client/src/features/visualisation/components/TransientBeamsLayer.tsx"]
    binaryProtocol["binaryProtocol.ts handleAgentAction<br/>client/src/store/websocket/binaryProtocol.ts"]

    settingsStore --> GraphManager
    settingsStore --> GraphCanvas
    settingsStore --> NodeDetailPanel
    settingsStore --> TimelineScrubber
    settingsStore --> ControlCenter
    settingsStore --> SettingField
    settingsStore --> RevealSetting
    settingsStore --> NostrAuthControl
    settingsStore --> BotsVisualization
    settingsStore --> AppInitializer

    websocketStore --> EmbeddingCloudLayer
    websocketStore -.->|main.tsx dev-mode window expose only| AppInitializer

    timelineStore --> TimelineScrubber

    beamStore --> useTransientBeams
    useTransientBeams --> TransientBeamsLayer
    binaryProtocol -->|pushTransientBeams| beamStore

    workerErrStore --> WorkerErrorModal
    workerErrStore --> AppInitializer

    subgraph GraphRenderTree["Graph render tree"]
        GraphManager --> GemNodes
        GraphManager --> GlassEdges
        GraphManager --> BotsNode
    end

    subgraph ControlCenterTree["Control Center tree"]
        ControlCenter --> SettingField
        ControlCenter --> RevealSetting
        ControlCenter --> StatusFlyout
    end
```
