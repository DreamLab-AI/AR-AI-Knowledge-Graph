---
id: AB-04
title: Five-slot adapter spine, dispatch middleware and connect lifecycle
area: agentbox
governing:
  - agentbox/docs/BASELINE-container.md
adrs: [ADR-2004, ADR-2005, ADR-2035, ADR-2036, ADR-2037]
sources:
  - agentbox/management-api/adapters/index.js
  - agentbox/management-api/adapters/base.js
  - agentbox/management-api/adapters/lifecycle.js
  - agentbox/management-api/adapters/contract-versions.js
  - agentbox/management-api/adapters/errors.js
  - agentbox/management-api/adapters/manifest-loader.js
  - agentbox/management-api/adapters/memory/embedded-ruvector.js
  - agentbox/management-api/adapters/memory/off.js
  - agentbox/management-api/adapters/beads/local-sqlite.js
  - agentbox/management-api/adapters/pods/_solid-http-base.js
  - agentbox/management-api/adapters/pods/local-solid-rs.js
  - agentbox/management-api/adapters/events/local-jsonl.js
  - agentbox/management-api/adapters/orchestrator/local-process-manager.js
  - agentbox/management-api/observability/metrics.js
  - agentbox/management-api/middleware/privacy-filter.js
  - agentbox/management-api/lib/pod-signer.js
  - agentbox/management-api/server.js
  - agentbox/tests/contract/adapter-lifecycle.contract.spec.js
  - agentbox/tests/contract/memory.contract.spec.js
  - agentbox/agentbox.sh
verified_commit: b00c28a0d
---

## AB-04.1 resolveAdapters — slot to implementation resolution
```mermaid
flowchart TD
    M["agentbox.toml [adapters]<br/>manifest-loader.js loadManifest()"] --> RA["resolveAdapters(manifest)<br/>adapters/index.js:153"]
    RA --> LOOP["for slot of SLOTS<br/>index.js:157"]
    LOOP --> S1["SLOTS = beads, pods, memory, events, orchestrator<br/>index.js:17"]
    S1 --> IMPL["impl = adapterDecls[slot] || 'off'<br/>index.js:154-158"]
    IMPL --> REQ["requireImpl(slot, impl)<br/>index.js:99"]
    REQ --> P1{"require adapters/&lt;slot&gt;/&lt;impl&gt;.js<br/>index.js:100-102"}
    P1 -->|found| CFG
    P1 -->|"MODULE_NOT_FOUND and impl==='off'"| PH["fallback adapters/&lt;slot&gt;/placeholder.js<br/>index.js:110-112"]
    PH -->|found| CFG
    PH -->|missing| UAI["throw UnknownAdapterImpl(slot, impl)<br/>index.js:19 / :116"]
    P1 -->|"MODULE_NOT_FOUND, impl!=='off'"| UAI
    CFG["slotConfig(slot, impl, manifest)<br/>index.js:35"] --> CM["memory + external-pg<br/>conninfo from integrations.ruvector_external<br/>index.js:41-43"]
    CFG --> CB["beads / events + external<br/>externalUrl from federation.external_url<br/>index.js:46-49"]
    CFG --> CP["pods: buildPodNip98(manifest)<br/>index.js:56 → lib/pod-signer.js"]
    CP --> CP2["local-solid-rs → baseUrl<br/>http://sp.bind:sp.port default 127.0.0.1:8484<br/>index.js:66-70"]
    CFG --> CO["orchestrator + stdio-bridge<br/>externalUrl + protocol default 'stdio'<br/>index.js:76-81"]
    CM --> NEW
    CB --> NEW
    CP2 --> NEW
    CO --> NEW
    NEW["new AdapterClass(cfg)<br/>index.js:161-168"] --> INST["instrumentAdapter(adapter, slot, impl, manifest)<br/>index.js:131"]
    INST --> META["adapter._implName = impl<br/>adapter._slot = slot<br/>index.js:171-173"]
    META --> OUT["returns beads, pods, memory, events, orchestrator<br/>index.js:176-178"]
    UAI --> FAIL["startup aborts — no silent 'off' substitution<br/>INVARIANT ADR-2004"]
```

## AB-04.2 Adapter class hierarchy across the three implementation classes
```mermaid
classDiagram
    class BaseAdapter {
        +String slot
        +String impl
        +String CONTRACT_VERSION
        +Boolean enabled
        +constructor(slot, impl, contractVersion) base.js:20
    }
    class EmbeddedRuvectorMemoryAdapter {
        +store(key, value, namespace) embedded-ruvector.js:76
        +search(query, opts) embedded-ruvector.js:98
        +retrieve(key, namespace) embedded-ruvector.js:119
        +del(key, namespace) embedded-ruvector.js:137
        +list(namespace) embedded-ruvector.js:149
        -_ns(namespace) embedded-ruvector.js:63
    }
    class OffMemoryAdapter {
        +store() throws AdapterDisabled off.js:19
        +search() throws AdapterDisabled off.js:20
        +retrieve() throws AdapterDisabled off.js:21
        +del() throws AdapterDisabled off.js:22
        +list() throws AdapterDisabled off.js:23
    }
    class LocalSqliteBeadsAdapter {
        +createEpic(opts) local-sqlite.js:78
        +createChild(opts) local-sqlite.js:113
        +claim(id, actor) local-sqlite.js:145
        +close(id, outcome) local-sqlite.js:168
        +addDependency(childId, blockerId, type) local-sqlite.js:195
        +getReady(filter) local-sqlite.js:236
        +show(id) local-sqlite.js:259
    }
    class SolidHttpPodsAdapter {
        -_nip98 signer or null _solid-http-base.js:43
        -_fetch signed or raw _solid-http-base.js:47
        +write(uri, body, contentType) _solid-http-base.js:74
        +read(uri) _solid-http-base.js:90
        +patch(uri, patch) _solid-http-base.js:105
        +del(uri) _solid-http-base.js:120
        +list(container, opts) _solid-http-base.js:133
    }
    class LocalSolidRsPodsAdapter {
        +probeCapabilities() local-solid-rs.js:51
        +list(container, opts) local-solid-rs.js:72
        +patch(uri, patch, opts) local-solid-rs.js:126
    }
    class LocalJsonlEventsAdapter {
        +dispatch(event) local-jsonl.js:61
        +subscribe(filter, handler) local-jsonl.js:94
        +unsubscribe(subscriptionId) local-jsonl.js:106
        -_initChain() local-jsonl.js:122
    }
    class LocalProcessManagerOrchestratorAdapter {
        +spawnAgent(spec) local-process-manager.js:41
        +streamEvent(agentId, handler) local-process-manager.js:102
        +listAgents() local-process-manager.js:115
        +handleGovernanceDecision(event) local-process-manager.js:133
        +terminateAgent(agentId) local-process-manager.js:303
    }
    BaseAdapter <|-- EmbeddedRuvectorMemoryAdapter
    BaseAdapter <|-- OffMemoryAdapter
    BaseAdapter <|-- LocalSqliteBeadsAdapter
    BaseAdapter <|-- SolidHttpPodsAdapter
    SolidHttpPodsAdapter <|-- LocalSolidRsPodsAdapter
    BaseAdapter <|-- LocalJsonlEventsAdapter
    BaseAdapter <|-- LocalProcessManagerOrchestratorAdapter
```

## AB-04.3 instrumentAdapter — prototype-chain walk that installs the wrappers
```mermaid
sequenceDiagram
    autonumber
    participant RA as resolveAdapters<br/>adapters/index.js:153
    participant IA as instrumentAdapter<br/>adapters/index.js:131
    participant PR as Object.getPrototypeOf chain<br/>index.js:133-146
    participant WD as wrapDispatch<br/>observability/metrics.js:125
    participant A as adapter instance

    RA->>IA: instrumentAdapter(new AdapterClass(cfg), slot, impl, manifest)
    IA->>PR: proto = getPrototypeOf(adapter)
    loop while proto and proto !== Object.prototype (index.js:134)
        PR-->>IA: getOwnPropertyNames(proto)
        loop for each name
            alt name in NON_DISPATCH or name startsWith underscore
                IA->>IA: skip — index.js:129 and :137
            else descriptor value is not a function
                IA->>IA: skip — index.js:139
            else
                IA->>WD: wrapDispatch(slot, impl, name, desc.value.bind(adapter), manifest)
                WD-->>IA: instrumentedDispatch
                IA->>A: adapter[name] = instrumentedDispatch (own property)
            end
        end
        IA->>PR: proto = getPrototypeOf(proto)
    end
    IA-->>RA: adapter (own props now shadow the prototype methods)
    Note over IA,PR: NON_DISPATCH = constructor, connect, disconnect — index.js:129
    Note over IA,PR: connect() stays unwrapped so lifecycle.js owns its failure semantics
    Note over PR,A: pods walks LocalSolidRsPodsAdapter then SolidHttpPodsAdapter then BaseAdapter
    Note over A: INVARIANT ADR-2004 — every durable-state call rides one of the five slots
```

## AB-04.4 wrapDispatch — the middleware order actually wired
```mermaid
sequenceDiagram
    autonumber
    participant C as Route handler<br/>management-api/routes/*.js
    participant L1 as Layer 1 observability<br/>metrics.js:125 wrapDispatch
    participant L2 as Layer 2 privacy filter<br/>privacy-filter.js:649
    participant FN as raw adapter method<br/>bound at index.js:140
    participant PM as prom-client registry<br/>metrics.js:19 and :27
    participant LOG as stdout JSON log<br/>metrics.js:150-164

    C->>L1: adapter.store(key, value, namespace)
    rect rgb(232, 244, 255)
        L1->>L1: startHrTime = process.hrtime.bigint() (metrics.js:130)
        L1->>L1: executionId = uris.mint(kind event) (metrics.js:134)
        L1->>L2: await privacyWrapped(...args) (metrics.js:140)
        rect rgb(255, 242, 230)
            L2->>L2: isMutationMethod(methodName) (privacy-filter.js:652)
            L2->>FN: fn(...args) after redaction or pass-through
            FN-->>L2: result
        end
        L2-->>L1: result
        L1->>PM: adapterDispatchTotal.labels(slot, method, impl, 'success').inc()
        L1->>PM: adapterDurationSeconds.labels(slot, method, impl).observe(seconds)
        L1->>LOG: msg adapter_dispatch with slot, method, impl, duration_ms, execution_id, outcome
    end
    L1-->>C: result
    Note over L1,L2: privacyWrapped is built ONCE at wrap time — metrics.js:127
    Note over L1,FN: Layer 1 timing encloses Layer 2 so redaction latency is inside the span (metrics.js:138-139)
    Note over C,FN: DIVERGENCE — BASELINE-container "Adapter spine" states dispatch is wrapped<br/>observability then privacy then JSON-LD. Only Layers 1 and 2 are in the chain.<br/>metrics.js:110-112 states Layer 3 JSON-LD is ADR-012 and the CALLER must invoke<br/>encoder.dispatch after this wrapper returns.
    Note over C,FN: RESOLVED ADR-2036: the two-layer wrap is the decided design,<br/>not a gap. JSON-LD encoding is a per-surface gated stage invoked<br/>by the owning route — ordering is enforced by the privacy marker<br/>via assertPrivacyFilterApplied (privacy-filter.js:595). ADR-2005 superseded.
    Note over C,FN: This matches BASELINE "Adapter acceptance qualification 2026-09-04" — encoding is a separate caller action
    Note over L1,PM: On throw the same counters record outcome error — metrics.js:167-175
```

## AB-04.5 Layer 2 privacy filter — write-op gating and fail policy
```mermaid
sequenceDiagram
    autonumber
    participant L1 as wrapDispatch<br/>metrics.js:140
    participant PF as privacyFilteredDispatch<br/>privacy-filter.js:654
    participant POL as _slotPolicy(slot, manifest)<br/>privacy-filter.js:655
    participant OPF as OPF redactor<br/>_callOpfFields privacy-filter.js:700
    participant FN as raw adapter method

    L1->>PF: privacyWrapped(...args)
    PF->>POL: resolve policy for slot
    alt not a mutation method OR policy === 'off' (privacy-filter.js:661)
        PF->>PF: _markValueArg(args) — stamp traversal (privacy-filter.js:662)
        PF->>FN: fn(...args)
        FN-->>PF: result
    else mutation under an active policy
        PF->>PF: collectLeaves(args, methodName) (privacy-filter.js:685)
        alt truncated — cycle or depth limit (privacy-filter.js:687)
            PF->>PF: fail('payload could not be fully traversed')
        end
        alt leaves.length === 0 (privacy-filter.js:693)
            PF->>FN: fn(...args) after _markValueArg
        else
            PF->>OPF: _callOpfFields(texts, slot, methodName)
            OPF-->>PF: redacted[]
            alt a non-content field changed (privacy-filter.js:705-709)
                PF->>PF: opfIdentifierPii.inc(dirty.length) then fail(IdentifierPiiDetected)
            else
                PF->>PF: finalArgs = applyRedactions(args, leaves, redacted) (privacy-filter.js:716)
                PF->>FN: fn(...finalArgs)
            end
        end
    end
    FN-->>PF: result
    PF-->>L1: result
    Note over PF: fail() branches on policy — strict throws AdapterWriteRejected<br/>(privacy-filter.js:669-675), otherwise fail-open counter and caller continues<br/>(privacy-filter.js:677-682)
    Note over PF,OPF: Identifier and unclassified fields are SCREENED never rewritten — only role 'content' leaves are redacted in place
    Note over PF: assertPrivacyFilterApplied(payload, slot) at privacy-filter.js:595 verifies the layer was traversed (DDD-004 L08)
```

## AB-04.6 connectAdapters — per-slot deadline, not one aggregate budget
```mermaid
sequenceDiagram
    autonumber
    participant SV as server.js boot<br/>server.js:1240-1258
    participant LC as connectAdapters<br/>adapters/lifecycle.js:217
    participant CO as connectOneSlot<br/>adapters/lifecycle.js:177
    participant AD as adapter.connect()
    participant T as deadline timer<br/>lifecycle.js:193-195

    SV->>LC: connectAdapters(slots, adapters, manifest, logger, resolveOff) — server.js:1242
    par all five slots concurrently (lifecycle.js:232)
        LC->>LC: timeoutMs = connectTimeoutFor(slot, manifest) (lifecycle.js:235 and :117)
        alt adapter missing or no connect() hook (lifecycle.js:238)
            LC->>LC: state = 'off' when enabled === false else 'ready'
        else
            LC->>CO: connectOneSlot(adapter, slot, timeoutMs, timers)
            CO->>AD: Promise.resolve().then(() => adapter.connect()) (lifecycle.js:182)
            CO->>T: setTimeout(timeoutMs) — deliberately NOT unref'd (lifecycle.js:189-195)
            CO->>CO: await Promise.race([observed, timeout]) (lifecycle.js:197)
            alt resolved first
                CO-->>LC: settled 'resolved'
                LC->>LC: state 'ready', log Adapter connected (lifecycle.js:251-252)
            else rejected first
                CO-->>LC: settled 'rejected'
            else deadline first
                CO-->>LC: settled 'timeout' plus latePromise (lifecycle.js:199)
            end
        end
    end
    LC-->>SV: readiness map and healthy flag (lifecycle.js:312-313)
    SV->>SV: app.adapters[slot] = resolvedAdapters[slot] (server.js:1255)
    SV->>SV: Object.assign(adapterHealth, toLegacyHealth(readiness)) (server.js:1256)
    SV->>SV: app.decorate('adapterReadiness', readiness) (server.js:1257)
    Note over LC,T: DEFAULT_CONNECT_TIMEOUT_MS = 10000 — lifecycle.js:70
    Note over LC: Manifest override [adapters] connect_timeout_ms scalar or per-slot map — lifecycle.js:106-107, non-positive values ignored (lifecycle.js:124)
    Note over SV,LC: DOC-DRIFT — BASELINE-container says server.js:1206 connects all five slots under a 10<br/>s TOTAL budget. The code races ONE deadline PER SLOT in adapters/lifecycle.js:217,<br/>wired from server.js:1241-1242. Aggregate wall-clock is bounded by the slowest single<br/>slot (lifecycle.js:31-35).
    Note over SV,LC: RESOLVED ADR-2035: BASELINE-container.md:83 now documents the<br/>per-slot deadline (lifecycle.js:217, :70, :106-107). The code was<br/>already correct — only the doc changed.
    Note over CO,T: Timeout is a CONNECT FAILURE identical in consequence to an explicit rejection — lifecycle.js:32-33
```

## AB-04.7 Failure path — quarantine before replacement, fail-closed slots abort
```mermaid
sequenceDiagram
    autonumber
    participant LC as connectAdapters<br/>lifecycle.js:217
    participant Q as quarantineAdapter<br/>lifecycle.js:140
    participant RO as resolveOff callback<br/>server.js:1247-1250
    participant IDX as resolveAdapters<br/>index.js:153
    participant PX as process.exit(1)<br/>lifecycle.js:307

    LC->>LC: failureMode = timeout or rejected (lifecycle.js:256)
    LC->>LC: record state 'unavailable' with reason and durationMs (lifecycle.js:258)
    opt outcome.latePromise present (lifecycle.js:264)
        LC->>LC: attach handler recording lateSettle and warn "settled AFTER its deadline — the slot stays withdrawn"
    end
    rect rgb(255, 232, 232)
        LC->>Q: quarantineAdapter(adapter, slot, failureMode plus reason) — lifecycle.js:274
        Q->>Q: collect own props and whole prototype chain (lifecycle.js:142-148)
        Q->>Q: replace every callable with async thrower AdapterQuarantined (lifecycle.js:159)
        Q->>Q: skip QUARANTINE_EXEMPT constructor and disconnect, and underscore-private (lifecycle.js:150)
        Q-->>LC: adapter._quarantined = true (lifecycle.js:164)
    end
    alt slot in FAIL_CLOSED_SLOTS — orchestrator (lifecycle.js:276 and :73)
        LC->>LC: log error "Fail-closed adapter slot failed to connect — FATAL"
        LC->>PX: fatals drained after Promise.all (lifecycle.js:303-309)
        PX-->>LC: startup aborts
    else degrade path
        LC->>RO: resolveOff(slot)
        RO->>IDX: resolveAdapters({adapters: {slot: 'off'}})[slot]
        alt replacement built
            IDX-->>LC: off adapter
            LC->>LC: adapters[slot] = offSlot, record.state = 'disabled' (lifecycle.js:289-291)
        else replacement threw (lifecycle.js:292)
            LC->>LC: record.state = 'unavailable', failureMode 'replacement-failed' (lifecycle.js:294-295)
            LC->>LC: slot stays QUARANTINED — dispatch throws AdapterQuarantined not AdapterDisabled
        end
    end
    Note over Q: Quarantine happens BEFORE any replacement so no window exists in which a half-connected adapter is reachable (lifecycle.js:272-273)
    Note over Q: AdapterQuarantined carries statusCode 503 — lifecycle.js:89
    Note over LC,RO: DIVERGENCE closed in code — BASELINE "Adapter acceptance qualification 2026-09-04"<br/>flags that replacement can fail. lifecycle.js:292-299 now makes that loud and<br/>fail-closed rather than leaving the broken original wired.
    Note over LC,RO: RESOLVED ADR-2035: recorded as the decided policy — quarantine<br/>before replace (lifecycle.js:272-274), 'unavailable' on replacement<br/>failure (lifecycle.js:292-295).
    Note over LC: disconnect() stays callable through quarantine so shutdown can release what the adapter opened (lifecycle.js:42-43 and :76)
```

## AB-04.8 Per-slot readiness lifecycle and the legacy health collapse
```mermaid
stateDiagram-v2
    [*] --> Unresolved
    Unresolved --> Constructed: resolveAdapters index.js:161-168
    Constructed --> Instrumented: instrumentAdapter index.js:131
    Instrumented --> NoConnectHook: adapter.connect not a function lifecycle.js:238
    Instrumented --> Connecting: connectOneSlot lifecycle.js:177

    NoConnectHook --> Off: enabled === false lifecycle.js:240
    NoConnectHook --> Ready: otherwise lifecycle.js:240

    Connecting --> Ready: settled resolved lifecycle.js:250
    Connecting --> Rejected: settled rejected lifecycle.js:256
    Connecting --> TimedOut: deadline won lifecycle.js:256

    Rejected --> Quarantined: quarantineAdapter lifecycle.js:274
    TimedOut --> Quarantined: quarantineAdapter lifecycle.js:274

    Quarantined --> Fatal: slot in FAIL_CLOSED_SLOTS lifecycle.js:276
    Quarantined --> Disabled: off replacement wired lifecycle.js:289-291
    Quarantined --> Unavailable: replacement-failed lifecycle.js:294-295

    Fatal --> [*]: process.exit(1) lifecycle.js:307

    note right of Ready
        toLegacyHealth lifecycle.js:324
        ready maps to healthy gauge 2
    end note
    note right of Off
        off maps to off gauge 0
        setAdapterHealth metrics.js:202-204
    end note
    note right of Disabled
        collapses to degraded gauge 1
        dispatch throws AdapterDisabled errors.js
    end note
    note right of Unavailable
        also collapses to degraded
        dispatch throws AdapterQuarantined 503
    end note
    note left of TimedOut
        late settle is recorded never re-armed
        lifecycle.js:264-270
    end note
```

## AB-04.9 memory.store — embedded-ruvector dispatch through both layers
```mermaid
sequenceDiagram
    autonumber
    participant R as routes/memory.js
    participant L1 as Layer 1 wrapDispatch<br/>metrics.js:125
    participant L2 as Layer 2 privacy<br/>privacy-filter.js:654
    participant MA as EmbeddedRuvectorMemoryAdapter<br/>memory/embedded-ruvector.js:55
    participant NS as _ns(namespace)<br/>embedded-ruvector.js:63
    participant PG as ruvector-postgres sidecar<br/>db ruvector 5432

    R->>L1: app.adapters.memory.store(key, value, namespace)
    L1->>L2: privacyWrapped(key, value, namespace)
    L2->>L2: isMutationMethod('store') is true — redaction path runs
    L2->>MA: store(key, redactedValue, namespace)
    MA->>NS: resolve effective namespace
    MA->>PG: upsert into memory_entries with 384-dim embedding
    PG-->>MA: row id
    MA-->>L2: result
    L2-->>L1: result
    L1->>L1: adapterDispatchTotal.labels('memory','store',impl,'success').inc()
    L1-->>R: result
    alt impl resolved to off (adapters/memory/off.js:14)
        R->>L1: store(...)
        L1->>L2: privacyWrapped(...)
        L2->>MA: OffMemoryAdapter.store()
        MA-->>R: throw AdapterDisabled('memory') — off.js:19
    end
    alt impl resolved to external-pg
        Note over MA,PG: slotConfig supplies conninfo from integrations.ruvector_external — index.js:41-43
    end
    alt slot quarantined by lifecycle.js:274
        MA-->>R: throw AdapterQuarantined statusCode 503 — lifecycle.js:78-90
    end
    Note over MA,PG: contract version memory 1.0.0 — contract-versions.js:10
    Note over R,PG: sibling read methods search embedded-ruvector.js:98, retrieve :119, del :137, list :149
    Note over L1: Every one of those methods is wrapped identically by instrumentAdapter index.js:131
```

## AB-04.10 beads.addDependency and beads.getReady — the 1.1.0 work-DAG pair
```mermaid
sequenceDiagram
    autonumber
    participant R as routes/beads.js
    participant L1 as Layer 1 wrapDispatch<br/>metrics.js:125
    participant L2 as Layer 2 privacy<br/>privacy-filter.js:654
    participant BA as LocalSqliteBeadsAdapter<br/>beads/local-sqlite.js:56
    participant DB as local sqlite bead store

    R->>L1: addDependency(childId, blockerId, type)
    L1->>L2: privacyWrapped(childId, blockerId, type)
    L2->>BA: addDependency(childId, blockerId, type default 'blocks') — local-sqlite.js:195
    BA->>DB: insert into bead_deps
    DB-->>BA: ok
    BA-->>L1: result
    L1->>L1: adapterDurationSeconds.labels('beads','addDependency',impl).observe(s)
    L1-->>R: result

    R->>L1: getReady(filter)
    L1->>L2: privacyWrapped(filter)
    L2->>L2: isMutationMethod('getReady') is false — pass-through, _markValueArg only (privacy-filter.js:661-663)
    L2->>BA: getReady(filter) — local-sqlite.js:236
    BA->>DB: select beads with no unmet blocker in bead_deps
    DB-->>BA: ready rows
    BA->>BA: _hydrate(row) — local-sqlite.js:274
    BA-->>L1: ready beads
    L1-->>R: ready beads
    Note over BA: beads contract 1.1.0 is the only non-1.0.0 pin — contract-versions.js:7-8 records it as additive for addDependency plus dependency-aware getReady
    Note over BA,DB: other slot methods createEpic :78, createChild :113, claim :145, close :168, show :259
    Note over L2: read methods traverse the privacy layer but are never redacted — only mutations are
```

## AB-04.11 pods.write — NIP-98 origination through the signed fetch
```mermaid
sequenceDiagram
    autonumber
    participant R as routes/pod-git.js or linked-objects.js
    participant L1 as Layer 1 wrapDispatch<br/>metrics.js:125
    participant L2 as Layer 2 privacy<br/>privacy-filter.js:654
    participant PA as LocalSolidRsPodsAdapter<br/>pods/local-solid-rs.js:27
    participant SF as _signedFetch<br/>_solid-http-base.js:57
    participant SG as nip98 signer<br/>lib/pod-signer.js:81
    participant SP as solid-pod-rs<br/>127.0.0.1:8484

    Note over PA: cfg.baseUrl resolved at index.js:66-70 from integrations.solid_pod_rs bind and port
    R->>L1: pods.write(uri, body, contentType)
    L1->>L2: privacyWrapped(uri, body, contentType)
    L2->>PA: write(uri, body, contentType default application/ld+json) — _solid-http-base.js:74
    PA->>SF: this._fetch(base + uri) with method PUT
    alt signer wired — buildPodNip98 returned a function (index.js:56, pod-signer.js:32)
        SF->>SF: hasAuth check on existing headers (_solid-http-base.js:60)
        SF->>SG: nip98(method, url, body) — pod-signer.js:81
        SG-->>SF: Authorization header value
        alt header is falsy
            SF->>SP: _rawFetch without Authorization (_solid-http-base.js:63)
        else
            SF->>SP: _rawFetch with Authorization header (_solid-http-base.js:64)
        end
    else signer null — gate off, unsigned and byte-identical to prior behaviour (index.js:54-55)
        SF->>SP: _rawFetch unsigned — this._fetch is _rawFetch (_solid-http-base.js:47)
    end
    SP-->>PA: HTTP response
    PA->>PA: _assert(res, [200, 201]) — _solid-http-base.js:150
    PA-->>L1: uri, status, created_at
    L1-->>R: result
    Note over R,SG: buildPodNip98 failure is warned not fatal — onError callback logs "pods NIP-98 signing disabled" (index.js:57-59)
    Note over PA: local-solid-rs overrides probeCapabilities :51, list :72, patch :126 over the shared base
    Note over PA,SP: pods contract 1.0.0 — contract-versions.js:9
```

## AB-04.12 events.dispatch — hash-chained local JSONL sink
```mermaid
sequenceDiagram
    autonumber
    participant R as routes/agent-events.js
    participant L1 as Layer 1 wrapDispatch<br/>metrics.js:125
    participant L2 as Layer 2 privacy<br/>privacy-filter.js:654
    participant EA as LocalJsonlEventsAdapter<br/>events/local-jsonl.js:37
    participant CH as _initChain<br/>local-jsonl.js:122
    participant F as _filePath JSONL sink<br/>local-jsonl.js:112

    R->>L1: events.dispatch(event)
    L1->>L2: privacyWrapped(event)
    L2->>EA: dispatch(event) — local-jsonl.js:61
    EA->>CH: ensure chain head loaded
    EA->>F: _append(record) — local-jsonl.js:134
    F-->>EA: appended
    EA-->>L1: ack
    L1->>L1: adapterDispatchTotal.labels('events','dispatch',impl,'success').inc()
    L1-->>R: ack
    opt subscriber wiring
        R->>EA: subscribe(filter, handler) — local-jsonl.js:94
        R->>EA: unsubscribe(subscriptionId) — local-jsonl.js:106
    end
    Note over R,EA: DOC-DRIFT — BASELINE-container and ADR-2004 discussion name the operation<br/>events.publish. The implemented slot method is dispatch(event) at<br/>events/local-jsonl.js:61. No publish() exists on the local impl.
    Note over EA: event payload shape is pinned by adapters/events/agent-execution-event.schema.json
    Note over EA,F: events contract 1.0.0 — contract-versions.js:11
```

## AB-04.13 orchestrator.spawnAgent — the only fail-closed slot
```mermaid
sequenceDiagram
    autonumber
    participant R as routes/tasks.js or sessions-boundary.js
    participant L1 as Layer 1 wrapDispatch<br/>metrics.js:125
    participant L2 as Layer 2 privacy<br/>privacy-filter.js:654
    participant OA as LocalProcessManagerOrchestratorAdapter<br/>orchestrator/local-process-manager.js:21
    participant P as spawned agent process

    R->>L1: orchestrator.spawnAgent(spec)
    L1->>L2: privacyWrapped(spec)
    L2->>OA: spawnAgent(spec) — local-process-manager.js:41
    OA->>P: launch process
    P-->>OA: agentId
    OA-->>L1: agentId
    L1-->>R: agentId
    opt stream and control
        R->>OA: streamEvent(agentId, handler) — local-process-manager.js:102
        R->>OA: listAgents() — local-process-manager.js:115
        R->>OA: handleGovernanceDecision(event) — local-process-manager.js:133
        R->>OA: terminateAgent(agentId) — local-process-manager.js:303
    end
    Note over OA: INVARIANT ADR-2004 — orchestrator is the sole member of FAIL_CLOSED_SLOTS<br/>(lifecycle.js:73). Connect rejection, deadline expiry and quarantine are all equally<br/>fatal (lifecycle.js:50-53).
    Note over R,OA: DOC-DRIFT — the slot method is spawnAgent(spec) at local-process-manager.js:41, not spawn()
    Note over OA: stdio-bridge impl takes externalUrl plus protocol default stdio — index.js:76-81
    Note over OA,P: orchestrator contract 1.0.0 — contract-versions.js:12
```

## AB-04.14 Contract-version pins per slot
```mermaid
flowchart LR
    CV["contract-versions.js:6<br/>module.exports"] --> B["beads 1.1.0<br/>contract-versions.js:8"]
    CV --> P["pods 1.0.0<br/>contract-versions.js:9"]
    CV --> M["memory 1.0.0<br/>contract-versions.js:10"]
    CV --> E["events 1.0.0<br/>contract-versions.js:11"]
    CV --> O["orchestrator 1.0.0<br/>contract-versions.js:12"]
    B --> SUP["super(slot, impl, CONTRACT_VERSIONS[slot])<br/>base.js:20-26"]
    P --> SUP
    M --> SUP
    E --> SUP
    O --> SUP
    SUP --> INST["this.CONTRACT_VERSION on every instance<br/>base.js:26"]
    INST --> MAN["system-manifest emits impl plus contract_version<br/>per core-layer slot entry"]
    B -.-> NOTE1["1.1.0 additive — addDependency plus<br/>dependency-aware getReady bead_deps work-DAG"]
    P -.-> NOTE2["DIVERGENCE BASELINE Known divergences —<br/>stale placeholders despite live churn<br/>a breaking change would need a MAJOR bump that has not happened"]
    M -.-> NOTE2
    E -.-> NOTE2
    O -.-> NOTE2
    SUP -.-> NOTE3["base.js:21-23 throws when slot, impl or<br/>contractVersion is missing — no unpinned adapter can construct"]
```

## AB-04.15 The four validation stages — different files, different lifecycle points
```mermaid
flowchart TD
    subgraph S1["Stage 1 static schema — edit and build time"]
        A1["schema/agentbox.toml.schema.json"] --> A2["scripts/agentbox-config-validate.js"]
        A2 --> A3{"structural violation?"}
        A3 -->|yes| A4["reject before build"]
        A3 -->|"no"| A5["W0xx dead-policy warning only<br/>DIVERGENCE — advisory, does NOT hard-fail<br/>only structural schema violations reject<br/>BASELINE Known divergences"]
    end
    subgraph S2["Stage 2 boot probe — once per boot"]
        B1["server.js:1241-1242 require adapters/lifecycle"] --> B2["connectAdapters lifecycle.js:217"]
        B2 --> B3["per-slot deadline lifecycle.js:117 and :235"]
        B3 --> B4["ready | disabled | unavailable | off<br/>lifecycle.js:55-66"]
        B4 --> B5["toLegacyHealth lifecycle.js:324 → adapterHealth server.js:1256"]
    end
    subgraph S3["Stage 3 conformance — CI only, never at boot"]
        C1["tests/contract/memory.contract.spec.js"] --> C4["all three impl classes must behave identically"]
        C2["tests/contract/beads.contract.spec.js"] --> C4
        C3["tests/contract/adapter-lifecycle.contract.spec.js"] --> C4
        C5["tests/contract/pods.contract.spec.js"] --> C4
        C6["tests/contract/events.contract.spec.js"] --> C4
        C7["tests/contract/orchestrator.contract.spec.js"] --> C4
        C8["tests/contract/privacy-filter.contract.spec.js"] --> C4
        C9["tests/contract/memory-encoder-bypass.contract.spec.js"] --> C4
    end
    subgraph S4["Stage 4 SLO — continuous"]
        D1["agentbox_adapter_dispatch_total metrics.js:19"] --> D4["GET /metrics"]
        D2["agentbox_adapter_duration_seconds metrics.js:27"] --> D4
        D3["agentbox_adapter_health gauge metrics.js:36"] --> D4
        D4 --> D5["setAdapterHealth metrics.js:202-204 maps status to 0, 1 or 2"]
    end
    A4 --> B1
    A5 --> B1
    B5 --> D3
    S3 -.-> NOTE["DIVERGENCE — legacy ADR-005 conflates all four into 'contract tests'.<br/>They live in different files and fire at different lifecycle points."]
```

## AB-04.16 SLO surfacing and the agentbox.sh health exit contract
```mermaid
sequenceDiagram
    autonumber
    participant OP as operator shell
    participant SH as cmd_health<br/>agentbox.sh:1108
    participant H as GET /health<br/>server.js:564-575
    participant MT as GET /v1/meta<br/>agentbox.sh:1165
    participant PM as prom-client registry<br/>metrics.js:19 :27 :36

    OP->>SH: ./agentbox.sh health
    SH->>H: curl HEALTH_URL http://localhost:$MGMT_PORT/health (agentbox.sh:605 and :1118)
    alt curl fails
        H-->>SH: no response
        SH-->>OP: ERROR could not reach — exit 1 (agentbox.sh:1120-1121)
    else response received
        H-->>SH: status, uptime, image_hash, manifest_checksum, adapters, degraded_count, note
        SH->>SH: degraded = jq '.adapters // {} | to_entries[] | select(.value != "healthy" and .value != "off") | .key' (agentbox.sh:1134-1138)
        SH->>SH: degraded_count = jq '.degraded_count // 0' (agentbox.sh:1139)
        SH->>SH: print "adapter/<slot>: <value>" from .adapters (agentbox.sh:1143-1146)
        SH->>MT: curl http://localhost:9090/v1/meta for observability.metrics_endpoint
        MT-->>SH: metrics_endpoint
        SH->>PM: curl metrics_endpoint, print first 5 non-comment lines (agentbox.sh:1174)
        alt degraded non-empty OR degraded_count > 0
            SH-->>OP: exit 1 (agentbox.sh:1179)
        else
            SH-->>OP: exit 0
        end
    end
    Note over H: /health computes degradedCount from adapterHealth (server.js:565) and emits keys<br/>status, uptime, image_hash, manifest_checksum, adapters, degraded_count, note — there<br/>is NO services key
    Note over SH,H: DIVERGENCE — BASELINE-container Adapter spine stage 4 says "agentbox.sh health exits<br/>non-zero if any slot's gauge is 0". cmd_health reads .services (agentbox.sh:1133)<br/>which /health never emits, so the degraded list is always empty and the exit-1 branch<br/>at agentbox.sh:1179 is unreachable. It also never reads the agentbox_adapter_health<br/>gauge.
    Note over SH,H: RESOLVED ADR-2037: cmd_health now derives failure from .adapters<br/>(a slot fails when its value is neither "healthy" nor "off",<br/>agentbox.sh:1134-1138) plus .degraded_count (agentbox.sh:1139),<br/>so exit 1 at agentbox.sh:1179 is reachable.
    Note over SH: /health itself warns it is for human inspection only and points orchestrators at /ready (server.js:573)
    Note over PM: gauge values off 0, degraded 1, healthy 2 via setAdapterHealth metrics.js:202-204
```
