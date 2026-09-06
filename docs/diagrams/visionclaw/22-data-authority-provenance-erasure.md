---
id: VC-22
title: Data authority, provenance and erasure
area: visionclaw
governing:
  - docs/DATA-authority-erasure.md
  - docs/BASELINE-architecture.md
adrs: [ADR-2004, ADR-2015, ADR-2016, ADR-2017, ADR-2069, ADR-2070]
sources:
  - src/adapters/sqlite_settings_repository.rs
  - src/adapters/sqlite_canary_repository.rs
  - src/adapters/sqlite_kpi_repository.rs
  - src/adapters/sqlite_enrichment_repository.rs
  - src/adapters/oxigraph_graph_repository.rs
  - crates/visionclaw-adapters/src/oxigraph_ontology_repository.rs
  - crates/visionclaw-adapters/src/provenance_emitter.rs
  - src/services/provenance_writer.rs
  - src/services/provenance_trace.rs
  - src/handlers/ingest_writeback_handler.rs
  - src/handlers/enrichment_proposals_handler.rs
  - src/handlers/trace_handler.rs
  - src/services/github_sync_service.rs
  - src/services/role_store.rs
  - src/middleware/rbac_gate.rs
  - src/main.rs
  - src/handlers/socket_flow_handler/position_updates.rs
  - crates/visionclaw-domain/src/utils/visibility_filter.rs
  - client/src/services/solidPod/agentMemory.ts
  - client/src/services/solidPod/ldpClient.ts
  - client/src/services/SolidPodService.ts
  - scripts/backup-sqlite.sh
  - agentbox/management-api/routes/git-bridge.js
  - docker-compose.unified.yml
  - src/app_state.rs
  - src/services/ontology_mutation_service.rs
verified_commit: 7a20db228
---

## VC-22.1 Write-master per data class

```mermaid
flowchart LR
    AuthContent["Authored content<br/>public: true / owl-class markdown"] --> GitHub["GitHub upstream<br/>src/services/github_sync_service.rs"]
    GitHub -->|"rebuild_assert_graph:973 CLEAR+INSERT"| AssertGraph["GRAPH_ONTOLOGY :assert<br/>oxigraph_ontology_repository.rs:48"]
    GitHub -->|"clear_graph:1226"| KnowledgeGraph["GRAPH_KNOWLEDGE<br/>oxigraph_ontology_repository.rs:50"]

    VisIntent["Visibility intent<br/>visibility + owner_pubkey"] --> SettingsDB["settings.sqlite3 settings table<br/>sqlite_settings_repository.rs:61-68"]
    SettingsDB -->|"projected by"| VisFilter["is_dropped_for<br/>visibility_filter.rs:67-77"]

    AssertGraph -.->|"reasoner run: clear_inferred_graph:691"| InferredGraph["GRAPH_ONTOLOGY_INFERRED<br/>oxigraph_ontology_repository.rs:49 derived, never primary"]

    OpState["Operational state"] --> SqliteFour["4x SQLite WAL: settings/enrichment/kpi/liveness<br/>src/adapters/sqlite_*_repository.rs"]

    VectorMem["Vector / agent memory"] --> RuVector["RuVector external Postgres<br/>mcp__claude-flow__memory_* -> ruvector-postgres:5432"]

    EventJournal["Event journal / provenance"] --> ProvGraph["GRAPH_PROVENANCE append-only<br/>oxigraph_ontology_repository.rs:57"]

    AuditRBAC["Audit evidence RBAC/auth"] --> RoleTable["user_roles table in settings.sqlite3<br/>role_store.rs:46-52"]

    Credentials["Credentials"] --> DotEnv[".env plaintext filesystem"]

    DerivedWB["Derived write-back only<br/>append_derived_quads:728"] --> SummaryGraph["GRAPH_ONTOLOGY_SUMMARY<br/>oxigraph_ontology_repository.rs:63"]
    DerivedWB --> ObservedGraph["GRAPH_ONTOLOGY_OBSERVED<br/>oxigraph_ontology_repository.rs:64"]

    DivLegacy["DIVERGENCE: legacy ADRs assign primacy to Oxigraph 132 / Pod 050-052 / GitHub 051 / RuVector 030 / provenance 033-034-124-128<br/>code resolves as this matrix, legacy prose not reconciled<br/>docs/DATA-authority-erasure.md:96-100"]
    AssertGraph -.-> DivLegacy
    RuVector -.-> DivLegacy

    DivCreds["PROPOSED ADR-2104: execute the SOPS rollout or formally withdraw ADR-109 - plaintext .env is documented as the interim state either way<br/>a 43MB scripts/sops binary dated 2026-05-09 is present and gitignored, but no .sops.yaml, secrets.enc.yaml or sops-env.sh exists<br/>docs/DATA-authority-erasure.md:106-107"]
    DotEnv -.-> DivCreds
```

## VC-22.2 SQLite schemas (four write-master databases)

```mermaid
erDiagram
    SETTINGS {
        text key PK
        text owner_pubkey PK
        text value
        text description
        integer updated_at
    }
    PHYSICS_PROFILES {
        text profile_name PK
        text owner_pubkey PK
        text settings_json
        integer updated_at
    }
    AUDIT_LOG {
        integer id PK
        integer occurred_at
        text actor_pubkey
        text request_method
        text request_path
        integer status_code
        text detail_json
    }
    SYNC_FILE_METADATA {
        text file_name PK
        text sha1
        integer updated_at
    }
    USER_ROLES {
        text pubkey PK
        text role
        text assigned_by
        integer updated_at
    }
    LIVENESS_CANARIES {
        text canary_id PK
        text description
        text kind
        text owner_repo
        text wave
        text sha_at_registration
        integer registered_at_ms
    }
    CANARY_FIRES {
        integer id PK
        text canary_id FK
        text evidence
        text sha
        integer fired_at_ms
    }
    KPI_AGENT_EVENTS {
        integer id PK
        integer event_id
        integer source_agent_id
        integer action_type
        integer observed_at_ms
        text agent_did
        text handoff_id
        integer token_count
    }
    KPI_SNAPSHOTS {
        integer id PK
        text kpi
        real value
        real confidence
        integer sample_count
        integer computed_at_ms
        text sha
    }
    KPI_LINEAGE {
        integer id PK
        integer snapshot_id FK
        text source_kind
        text source_ref
        real contribution
    }
    ENRICHMENT_PROPOSALS {
        text case_id PK
        text category
        text source_iri
        text proposal_json
        text status
        integer created_at
        integer updated_at
    }
    ENRICHMENT_DECISIONS {
        integer id PK
        text case_id FK
        text outcome
        integer attributed
        text broker_pubkey
        integer writeback_triggered
        integer writeback_committed
        integer writeback_committed_at_ms
        text activity_urn
        text owner_did
        integer decided_at_ms
    }

    ENRICHMENT_PROPOSALS ||--o{ ENRICHMENT_DECISIONS : "case_id (sqlite_enrichment_repository.rs:87)"
    LIVENESS_CANARIES ||--o{ CANARY_FIRES : "canary_id (sqlite_canary_repository.rs:64)"
    KPI_SNAPSHOTS ||--o{ KPI_LINEAGE : "snapshot_id (sqlite_kpi_repository.rs:101)"
    SETTINGS ||--|| USER_ROLES : "same settings.sqlite3 connection (role_store.rs:5, app_state.rs:723-726)"
```

## VC-22.3 Derived-writeback fence (`POST /api/ingest/writeback`, ADR-2015)

```mermaid
sequenceDiagram
    autonumber
    participant GB as GitBridge<br/>agentbox management-api/routes/git-bridge.js:733
    participant WBH as writeback handler<br/>src/handlers/ingest_writeback_handler.rs:75
    participant APD as apply_decision<br/>src/handlers/enrichment_proposals_handler.rs:340
    participant REPO as SqliteEnrichmentRepository<br/>src/adapters/sqlite_enrichment_repository.rs:489
    participant ONTO as append_derived_quads<br/>oxigraph_ontology_repository.rs:730

    GB->>WBH: POST /api/ingest/writeback decision block (ingest_writeback_handler.rs:103)
    WBH->>WBH: attribution_pubkey approvedBy did:nostr or hex (ingest_writeback_handler.rs:62-70)
    WBH->>APD: apply_decision(case_id, BrokerDecisionRequest) (ingest_writeback_handler.rs:93-98)
    APD->>REPO: record_decision INSERT decision + UPDATE proposal.status (sqlite_enrichment_repository.rs:503-542)
    alt writeback_triggered AND attributed (enrichment_proposals_handler.rs:408)
        APD->>ONTO: append_derived_summary(owner_did, activity_urn, triples) (enrichment_proposals_handler.rs:411)
        alt graph in DERIVED_FENCE assert or inferred
            ONTO--xAPD: rejected fenced graph not writable via derived path (oxigraph_ontology_repository.rs:744-747)
        else graph is summary or observed
            ONTO->>ONTO: INSERT DATA GRAPH GRAPH_ONTOLOGY_SUMMARY/OBSERVED (oxigraph_ontology_repository.rs:782-791)
            ONTO-->>APD: Ok(quad count)
            APD->>REPO: mark_writeback_committed case_id activity_urn (enrichment_proposals_handler.rs:423-424)
        end
    else unattributed or no writeback trigger
        Note over APD: decision recorded, no KG write - unattributed approval writes no fact (ingest_writeback_handler.rs:21-23)
    end
    APD-->>WBH: HttpResponse (broker:case_decided broadcast)
    Note over ONTO: INVARIANT: only summary/observed writable, assert/inferred rejected in the repo method itself (ADR-2015, oxigraph_ontology_repository.rs:70 DERIVED_FENCE)
```

## VC-22.4 Provenance write — two producers, one append-only graph (ADR-2016)

```mermaid
sequenceDiagram
    autonumber
    participant MUT as OntologyMutationService<br/>src/services/ontology_mutation_service.rs:202
    participant EMITTER as reify_activity<br/>crates/visionclaw-adapters/src/provenance_emitter.rs:307
    participant WRITER as build_assertion_version<br/>src/services/provenance_writer.rs:353
    participant T2 as caller transaction spine<br/>src/services/provenance_writer.rs:7-9
    participant STORE as GRAPH_PROVENANCE urn:ngm:graph:provenance<br/>oxigraph_ontology_repository.rs:57

    par Activity-record path (ADR-127 / ADR-2016)
        MUT->>EMITTER: emit_activity_nonfatal(store, record) (provenance_emitter.rs:446, called at ontology_mutation_service.rs:202)
        EMITTER->>EMITTER: build quads Activity/Agent/Entity triad (provenance_emitter.rs:137-251)
        EMITTER->>STORE: store.insert(Quad) x n, INSERT only (provenance_emitter.rs:32-33 header invariant)
    and Assertion-version path (ADR-049, reconciled PRD-022 WS-2)
        WRITER->>WRITER: build_assertion_version pure builder, no store I/O (provenance_writer.rs:353-471)
        WRITER-->>T2: AssertionVersionQuads.provenance_quads (re-exports GRAPH_PROVENANCE at provenance_writer.rs:87)
        T2->>STORE: execute quads in one atomic transaction (caller-owned, T2 per module doc)
    end
    Note over STORE: INVARIANT: append-only - no DELETE, DROP or CLEAR issued against this graph (ADR-2016,<br/>provenance_emitter.rs:32-33), verified by append_only_verified test (provenance_emitter.rs:908)
    Note over WRITER: Retraction only ADDS dl:validTo and deletes nothing - history never removed (provenance_writer.rs:218,478-502)
    Note over EMITTER,WRITER: PROPOSED ADR-2102: keep the append-only invariant and satisfy erasure by crypto-shredding the per-subject key so<br/>quad structure and hash chain survive while plaintext does not - the alternative is to declare provenance out of erasure<br/>scope explicitly - ADR-2102 exists to force that choice rather than leave it to omission
```

## VC-22.5 Unified provenance trace read path (`GET /api/trace`)

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant TH as unified_trace<br/>src/handlers/trace_handler.rs:35
    participant SVC as ProvenanceTraceService<br/>src/services/provenance_trace.rs:291
    participant KPI as SqliteKpiRepository<br/>src/adapters/sqlite_kpi_repository.rs:352 trajectories_since
    participant ENR as SqliteEnrichmentRepository<br/>src/adapters/sqlite_enrichment_repository.rs:718 provenance_decisions_since
    participant JOIN as build_trace<br/>src/services/provenance_trace.rs:174
    participant LH as LivenessHarness<br/>src/handlers/trace_handler.rs:64

    C->>TH: GET /api/trace agent window_ms (trace_handler.rs:76)
    TH->>SVC: ProvenanceTraceService::new(enrichment_repo, kpi_repo) (trace_handler.rs:39-42)
    TH->>SVC: query(window, agent_did) (trace_handler.rs:44)
    SVC->>KPI: trajectories_since(cutoff) (provenance_trace.rs:316)
    SVC->>ENR: provenance_decisions_since(cutoff) (provenance_trace.rs:321)
    opt agent filter present
        SVC->>SVC: retain rows where agent_did matches (provenance_trace.rs:327-328)
    end
    SVC->>JOIN: build_trace(trajectories, decisions, pod_marks empty, pod_source_available false) (provenance_trace.rs:332-337)
    JOIN-->>SVC: ProvenanceTrace sources_present sources_absent joins
    alt joins_multiple_source_kinds true
        TH->>LH: observe(CANARY_REC11_TRACE, evidence) (trace_handler.rs:63-65)
    else single source or none
        Note over TH: no canary fire, only one live source kind
    end
    TH-->>C: 200 JSON ProvenanceTrace
    Note over SVC,ENR: this trace reads only the two SQLite-backed sources - it does NOT query the Oxigraph GRAPH_PROVENANCE<br/>append-only graph ADR-2016 covers (ADR-2016 consumer closeout 2026-09-05)
    Note over JOIN: pod_git_mark source is hardcoded pod_source_available false, always reported sources_absent here (provenance_trace.rs:336)
```

## VC-22.6 Erasure — `deleteAgentMemory` Pod path (RuVector tombstone gap)

```mermaid
sequenceDiagram
    autonumber
    participant UI as SolidPodService.deleteAgentMemory<br/>client/src/services/SolidPodService.ts:366
    participant MEM as agentMemory.deleteAgentMemory<br/>client/src/services/solidPod/agentMemory.ts:191
    participant LDP as deleteResource<br/>client/src/services/solidPod/ldpClient.ts:206
    participant POD as Solid Pod HTTP server

    UI->>MEM: deleteAgentMemory(podPath, agentId, key) (SolidPodService.ts:368)
    MEM->>MEM: sanitizePreferenceKey(key), build container path (agentMemory.ts:192-193)
    MEM->>LDP: deleteResource(containerPath + safeKey + .jsonld) (agentMemory.ts:194)
    LDP->>POD: fetchWithAuth method DELETE (ldpClient.ts:208)
    alt response ok or 404
        POD-->>LDP: 2xx or 404 (already absent)
        LDP-->>MEM: true
    else other error status
        POD-->>LDP: 4xx/5xx
        LDP-->>MEM: false, logs DELETE failed (ldpClient.ts:210-212)
    end
    MEM-->>UI: boolean deleted-or-absent
    rect rgb(255, 230, 230)
        Note over MEM,POD: DIVERGENCE: no reverse tombstone to RuVector - embedding row persists and stays searchable (docs/DATA-authority-erasure.md:86-90)
        Note over MEM: this Pod-side delete never calls mcp__claude-flow__memory_delete or any RuVector endpoint - RuVector is<br/>external Postgres reached only via MCP, no delete-propagation hook exists in this path
    end
```

## VC-22.7 SQLite online backup (`scripts/backup-sqlite.sh`, ADR-2017)

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator/cron
    participant SH as backup-sqlite.sh
    participant CT as visionclaw_container
    participant SQL as sqlite3 CLI in container

    OP->>SH: scripts/backup-sqlite.sh (scripts/backup-sqlite.sh:1)
    SH->>SH: detect_mode docker or host (backup-sqlite.sh line ~130)
    loop for each db in REQUIRED_DBS + OPTIONAL_DBS (backup-sqlite.sh:75-76)
        SH->>CT: docker exec test -f db (backup_one_docker, backup-sqlite.sh:172)
        alt db present
            SH->>SQL: sqlite3 db .backup tmp (ONLINE BACKUP API)
            SQL-->>SH: PRAGMA integrity_check = ok
            SH->>SH: docker cp tmp to DEST/db
            opt VERIFY_RESTORE=1 (default)
                SH->>SH: verify_restore copy to scratch, SELECT count from expected tables (backup-sqlite.sh:110-129)
            end
        else db absent
            alt is_required(db) (backup-sqlite.sh:88)
                SH->>SH: record missing_required (backup-sqlite.sh:222-223)
            else optional db missing
                SH->>SH: log absent, continue (backup-sqlite.sh:229)
            end
        end
    end
    alt missing_required non-empty (backup-sqlite.sh:242-245)
        SH--xOP: die required database(s) missing, refuse to publish incomplete backup set, no manifest written
    else all required present
        SH->>SH: write MANIFEST.txt with sha256 per db
        SH-->>OP: DONE - N databases in DEST
    end
    Note over SH: PROPOSED ADR-2103: Oxigraph gains a point-in-time snapshot as a REQUIRED backup member with a declared RPO from the<br/>schedule and an RTO from a measured restore drill - GRAPH_PROVENANCE is restore-only because it cannot be re-derived<br/>extends ADR-2017, which stays the per-class write-master authority
```

## VC-22.8 Dual-writes: fact fan-out to two stores

```mermaid
sequenceDiagram
    autonumber
    participant SYNC as GithubSyncService<br/>src/services/github_sync_service.rs:992 rebuild_assert_graph
    participant KG as GRAPH_KNOWLEDGE<br/>oxigraph_ontology_repository.rs:50
    participant AG as GRAPH_ONTOLOGY assert<br/>oxigraph_ontology_repository.rs:48
    participant APD as apply_decision<br/>src/handlers/enrichment_proposals_handler.rs:340
    participant SQ as enrichment_decisions row<br/>sqlite_enrichment_repository.rs:509-527
    participant SUM as GRAPH_ONTOLOGY_SUMMARY<br/>oxigraph_ontology_repository.rs:63
    participant PROV as GRAPH_PROVENANCE<br/>oxigraph_ontology_repository.rs:57

    critical content sync fan-out (github_sync_service.rs:992-1043)
        SYNC->>KG: load_graph, ingest KG nodes (github_sync_service.rs:994-996)
        SYNC->>AG: save_ontology_graph atomic CLEAR GRAPH assert then INSERT DATA (github_sync_service.rs:1034-1043)
    end
    Note over SYNC,AG: authoritative store (GitHub) commits first via sync run - the Oxigraph projection is regenerated, never hand-edited (docs/DATA-authority-erasure.md:61-63)

    critical decision fan-out (enrichment_proposals_handler.rs:382-436)
        APD->>SQ: record_decision INSERT+UPDATE one transaction (sqlite_enrichment_repository.rs:503-542)
        APD->>SUM: append_derived_summary INSERT DATA :summary (enrichment_proposals_handler.rs:411, oxigraph_ontology_repository.rs:782-786)
        SUM->>PROV: prov:wasGeneratedBy marker on each subject (oxigraph_ontology_repository.rs:818-824)
    end
    Note over APD,SUM: no cross-store 2PC - the durable decision commits in SQLite first (writeback_triggered=true) - the Oxigraph<br/>write is best-effort, writeback_committed flips only on Ok (enrichment_proposals_handler.rs:407-435)
```

## VC-22.9 RBAC open-by-default posture on a read

```mermaid
sequenceDiagram
    autonumber
    participant C as Anonymous or unassigned client
    participant GATE as RbacGateMiddleware<br/>src/middleware/rbac_gate.rs:230
    participant REQ as required_level<br/>src/middleware/rbac_gate.rs:138
    participant BOOT as main.rs RBAC bootstrap<br/>src/main.rs:717-740
    participant RS as RoleStore.effective_role<br/>src/services/role_store.rs:359

    Note over BOOT: RBAC_PUBLIC_READS is fail-closed in code - .unwrap_or(false) (rbac_gate.rs:126-133) - and set on only by compose (docker-compose.unified.yml:93)
    BOOT->>BOOT: RBAC_ALLOW_OWNERLESS env check (main.rs:735-737, const at role_store.rs:33)
    alt no Owner assigned AND RBAC_ALLOW_OWNERLESS=1 (main.rs:740-746)
        BOOT->>BOOT: warn, run owner-less, only POWER_USER_PUBKEYS to Admin fallback applies
    else no Owner assigned and flag unset
        BOOT--xBOOT: FATAL refuse to start, fail-closed (main.rs:747-756)
    end
    C->>GATE: GET /api/graph/data (safe method)
    GATE->>REQ: required_level(GET, path, public_reads=true)
    alt public_reads true (rbac_gate.rs:156-161)
        REQ-->>GATE: None - public route, no auth required
        GATE-->>C: 200 passthrough (rbac_gate.rs:248-253)
    else public_reads false
        REQ-->>GATE: Some(ReadOnly)
        GATE->>RS: effective_role(pubkey, is_power_user) for a mutating/gated route
        alt explicit assignment exists
            RS-->>GATE: stored UserRole (role_store.rs:361)
        else no assignment AND is_power_user
            RS-->>GATE: Admin (role_store.rs:363-364)
        else no assignment, not power user
            RS-->>GATE: default_role, Editor unless RBAC_DEFAULT_ROLE=viewer (role_store.rs:366, RBAC_ALLOW_OWNERLESS_ENV sibling)
        else lookup error
            RS-->>GATE: Viewer, fails closed (role_store.rs:369-371)
        end
    end
    rect rgb(255, 230, 230)
    Note over BOOT,RS: CORRECTED ADR-2070 (raised by estate ADR-2087) - the CODE fails closed:<br/>public_reads_enabled() ends .unwrap_or(false) (rbac_gate.rs:126-133) and main.rs:730-735 refuses to<br/>start owner-less unless RBAC_ALLOW_OWNERLESS is set. The shipped compose inverts both<br/>(docker-compose.unified.yml:93,94, ${VAR:-1}), so an unassigned pubkey resolves to Editor<br/>(role_store.rs:359). The open posture is ADR-2027's deliberate demo default and stays.
    end
```

## VC-22.10 Full-sync corpus rebuild vs runtime writers (asserted-graph fence gap)

```mermaid
sequenceDiagram
    autonumber
    participant OP as Operator (force_full_sync)
    participant SYNC as GithubSyncService.sync_graphs<br/>src/services/github_sync_service.rs:283
    participant KGREPO as kg_repo.clear_graph<br/>src/adapters/oxigraph_graph_repository.rs:1226
    participant REBUILD as rebuild_assert_graph<br/>src/services/github_sync_service.rs:992
    participant AG as GRAPH_ONTOLOGY assert
    participant DIRECT as add_owl_class/add_axiom<br/>application/ontology/directives.rs (governed write door)

    OP->>SYNC: FORCE_FULL_SYNC=1
    alt force_full_sync true (github_sync_service.rs:378,578)
        SYNC->>KGREPO: clear_graph() wipes GRAPH_KNOWLEDGE (github_sync_service.rs:379-381)
        SYNC->>REBUILD: rebuild_assert_graph(stats) (github_sync_service.rs:598)
        REBUILD->>AG: CLEAR GRAPH assert then INSERT DATA (save_ontology_graph, atomic) (github_sync_service.rs:1034-1043)
        Note over REBUILD,AG: this CLEAR wipes the ENTIRE assert graph including runtime OWL classes/axioms added via the governed write door (github_sync_service.rs:977-980)
    else incremental sync (SHA1 filter narrowed file list)
        SYNC->>SYNC: existing data left intact, no clear (github_sync_service.rs:374-377)
    end
    Note over DIRECT,AG: a class added via add_owl_class between full-syncs is NOT itself in GRAPH_PROVENANCE-protected history for the<br/>assert graph - only re-derivation from the corpus (logseq source) restores it after the next rebuild
    Note over SYNC,AG: PROPOSED ADR-2102: one durable erasure record, five store acknowledgements, and a partial erasure that is recorded<br/>and retryable rather than a silent success - agentbox ADR-2060 is the RuVector-side half and is referenced, not superseded
```

## VC-22.11 SQLite membership contract vs KPI/enrichment sole-source risk

```mermaid
flowchart TD
    Script["backup-sqlite.sh<br/>REQUIRED_DBS default: settings.sqlite3 enrichment.sqlite3 kpi.sqlite3<br/>scripts/backup-sqlite.sh:75"] --> Req["REQUIRED - missing member fails whole run, no manifest written<br/>scripts/backup-sqlite.sh:242-245"]
    Script --> Opt["OPTIONAL_DBS default: liveness.sqlite3 - missing is logged, run still succeeds<br/>scripts/backup-sqlite.sh:76,229"]
    Req --> Verify["VERIFY_RESTORE=1 default: scratch-restore + SELECT count on expected_tables_for<br/>scripts/backup-sqlite.sh:91-102,110-129"]
    Verify --> Manifest["MANIFEST.txt sha256 per db<br/>scripts/backup-sqlite.sh:249-261"]
    Opt -.-> Missing["missing_optional logged, count still gt 0 required<br/>scripts/backup-sqlite.sh:233"]

    DriftNote["RESOLVED ADR-2069: the required/optional membership contract is ratified as the design and DATA-authority-erasure.md now describes it - a missing REQUIRED_DBS member (settings/enrichment/kpi, backup-sqlite.sh:75) aborts the run and publishes no manifest (:242-245), a missing OPTIONAL_DBS member (liveness, :76) is logged and the run continues (:229). Oxigraph still has NO point-in-time backup - that bullet stands."]
    Req -.-> DriftNote
```
