---
id: VC-24
title: ACSP — governed decision/elevation pipeline
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2006]
sources:
  - src/actors/elevation_actor.rs
  - src/actors/decision_elevation_actor.rs
  - src/services/decision_elevation.rs
  - src/services/decision_service.rs
  - src/services/broker_events.rs
  - src/services/proposal_spine.rs
  - src/services/nostr_bead_publisher.rs
  - src/services/nostr_bridge.rs
  - src/services/acsp/client.rs
  - src/services/acsp/events.rs
  - src/services/acsp/mod.rs
  - src/services/ontology_enrichment_service.rs
  - src/adapters/sqlite_enrichment_repository.rs
  - src/handlers/enrichment_proposals_handler.rs
  - src/handlers/broker_inbox_handler.rs
  - src/handlers/decision_handler.rs
  - src/web_contract/ledger.rs
  - src/web_contract/reducer.rs
  - src/web_contract/ritual.rs
  - src/web_contract/state.rs
  - src/web_contract/trail.rs
  - src/app_state.rs
verified_commit: b00c28a0d
---

## VC-24.1 Enrichment-proposal lifecycle (real status values)

```mermaid
stateDiagram-v2
    [*] --> pending: SqliteEnrichmentRepository.create_or_update<br/>sqlite_enrichment_repository.rs:298 status DEFAULT pending
    pending --> approved: status_for_outcome outcome approve accept accepted promote<br/>sqlite_enrichment_repository.rs:170
    pending --> rejected: status_for_outcome outcome starts_with reject<br/>sqlite_enrichment_repository.rs:171
    pending --> reviewed: status_for_outcome fallback amend delegate etc<br/>sqlite_enrichment_repository.rs:172
    approved --> elevated: ElevationActor GOV-2 terminal_for_pr_state Merged<br/>elevation_actor.rs:663,1172 repo.set_status
    approved --> abandoned: ElevationActor GOV-2 terminal_for_pr_state ClosedUnmerged<br/>elevation_actor.rs:663,1172 repo.set_status
    elevated --> [*]
    abandoned --> [*]
    rejected --> [*]
    reviewed --> [*]
    note right of approved
      set_status only exists for the class-elevation
      (ElevationActor) case family. DecisionElevationActor
      has no enrichment_repo field at all (grep confirms) so
      its decision_elevated decision_abandoned terminal
      states (kind-31404) never reach this SQLite table -
      they are forum-only, matching the ADR-2006 closeout
      line current elevation processing owns pending state
    end note
```

## VC-24.2 Enrichment-proposal creation — the real call chain

```mermaid
sequenceDiagram
    autonumber
    participant EA as ElevationActor.run_cycle<br/>elevation_actor.rs:840
    participant ACSP as AcspClient.publish<br/>services/acsp/client.rs:64
    participant REPO as SqliteEnrichmentRepository<br/>adapters/sqlite_enrichment_repository.rs:261
    Note over EA: RunCycle scans owl_class frontier stubs<br/>ranked by voice demand then graph degree
    EA->>EA: case_for + pending_proposal build<br/>elevation_actor.rs:350,792-794
    EA->>ACSP: publish build_action_request kind 31402<br/>elevation_actor.rs:799 acsp/events.rs:307
    alt publish Ok
        EA->>REPO: create_or_update StoredProposal status pending<br/>elevation_actor.rs:802
        REPO-->>EA: Ok case row upserted
    else publish Err
        EA->>EA: warn voice pending-case persist failed<br/>elevation_actor.rs:803 (no row written)
    end
    Note over EA,REPO: DOC-DRIFT - brief hint named ontology_enrichment_service then<br/>sqlite_enrichment_repository then proposal_spine as the creation chain.<br/>grep confirms no such call exists - ontology_enrichment_service.rs<br/>only classifies GraphData nodes/edges during github_sync_service parse<br/>(sync_local.rs:59) and proposal_spine::governed_commit is used only<br/>by decision_service.rs:677 and ontology_mutation_service.rs, never by<br/>the enrichment-proposal store. The real creator is ElevationActor<br/>(above) plus the decide-route stub fallback (VC-24.3)
```

## VC-24.3 POST /api/enrichment-proposals/{id}/decide — decide route

```mermaid
sequenceDiagram
    autonumber
    participant BR as agentbox broker-bridge
    participant H as decide<br/>handlers/enrichment_proposals_handler.rs:291
    participant AUTH as require_agent_key<br/>handlers/enrichment_proposals_handler.rs:137
    participant AD as apply_decision<br/>handlers/enrichment_proposals_handler.rs:340
    participant REPO as SqliteEnrichmentRepository
    participant OXI as OxigraphOntologyRepository.append_derived_summary<br/>handlers/enrichment_proposals_handler.rs:410
    participant ACSPC as AppState.acsp_client<br/>app_state.rs:320
    BR->>H: POST /api/enrichment-proposals/:id/decide X-Agent-Key
    H->>AUTH: require_agent_key compares X-Agent-Key to VISIONCLAW_AGENT_KEY<br/>handlers/enrichment_proposals_handler.rs:131,143
    alt key invalid or missing
        AUTH-->>H: 401 Unauthorized
        H-->>BR: 401 invalid or missing X-Agent-Key header
    else key valid
        H->>AD: apply_decision case_id body
        AD->>AD: record_decision pure core mints activity_urn<br/>handlers/enrichment_proposals_handler.rs:162
        opt case unknown to store
            AD->>REPO: create_or_update stub status pending is_new_case<br/>handlers/enrichment_proposals_handler.rs:360-378
        end
        AD->>REPO: record_decision atomic INSERT decision + UPDATE proposal.status<br/>handlers/enrichment_proposals_handler.rs:391 sqlite_enrichment_repository.rs:480
        alt outcome approve and attributed pubkey hex
            AD->>OXI: append_derived_summary owner_did activity_urn summary_triples
            OXI-->>AD: Ok fenced :summary write landed
            AD->>REPO: mark_writeback_committed case_id activity_urn now_ms
            Note right of AD: writeback_committed=true only on real Oxigraph write -<br/>writeback_triggered=true just means outcome qualified (approve+attributed)
        else unattributed or non-approve
            Note right of AD: writeback_committed stays false - no owner DID<br/>to scope an owner-less KG node, by design
        end
        AD->>AD: derive_kernel_decision via DecisionOrchestrator<br/>handlers/enrichment_proposals_handler.rs:225 (domain broker kernel, ~936 LOC)
        Note over AD: DIVERGENCE: BrokerActor never merged (BASELINE-architecture.md<br/>Known divergences) - main uses this stateless ACSP producer plus the<br/>cherry-picked storage-agnostic domain kernel (BrokerCase/DecisionOrchestrator)<br/>invoked here, never a resurrected actor+transport
        alt case newly entered queue
            AD->>AD: broker_events.broadcast_new_case broker:new_case<br/>handlers/enrichment_proposals_handler.rs:449-454
        end
        AD->>AD: broker_events.broadcast_case_decided broker:case_decided<br/>handlers/enrichment_proposals_handler.rs:457-463
        alt ACSP client configured (:469-481)
            AD->>ACSPC: publish build_action_response kind 31403<br/>handlers/enrichment_proposals_handler.rs:472
            alt publish Ok
                ACSPC-->>AD: event id
                Note right of AD: forum_projection=published<br/>handlers/enrichment_proposals_handler.rs:481
            else publish Err
                Note right of AD: forum_projection=failed - decision recorded<br/>locally but NOT visible in forum broker_decisions<br/>handlers/enrichment_proposals_handler.rs:485-487
            end
        else no AcspClient (FORUM_RELAY_URL unset)
            Note right of AD: forum_projection=skipped - degraded, not silent<br/>handlers/enrichment_proposals_handler.rs:492-494
        end
        AD-->>H: DecideResponse success writeback_triggered writeback_committed forum_projection
        H-->>BR: 200 OK
    end
```

## VC-24.4 ElevationActor — draft → PR → GOV-2 merge poll → concept_elevated

```mermaid
sequenceDiagram
    autonumber
    participant CYCLE as run_interval CYCLE_INTERVAL 600s<br/>elevation_actor.rs:59,722
    participant EA as ElevationActor<br/>elevation_actor.rs:118
    participant ACSP as AcspClient
    participant GH as GitHubPRService.create_ontology_pr
    participant POLL as run_interval PR_POLL_INTERVAL 120s<br/>elevation_actor.rs:62,738
    CYCLE->>EA: RunCycle (frontier scan)
    EA->>ACSP: publish build_action_request kind 31402<br/>acsp/events.rs:307
    Note over EA: pending case stored in-memory HashMap<br/>self.pending (elevation_actor.rs:123)
    ACSP-->>EA: CaseDecision via run_decision_subscription<br/>acsp/client.rs:96 (kind 31403, since Timestamp::now)
    alt action approve
        EA->>EA: approve_with_gate runs GOV-7 EL++ consistency gate<br/>elevation_actor.rs:1020,656-660 WhelkInferenceEngine.check_axiom_set
        alt gate inconsistent
            EA->>EA: record synthetic reject decision, case BLOCKED<br/>elevation_actor.rs:1038-1049
        else gate consistent
            EA->>GH: create_ontology_pr draft class page<br/>elevation_actor.rs:1067
            GH-->>EA: pr_url
            EA->>EA: elevating.insert case_id TrackedPr pr_url<br/>elevation_actor.rs:124-127
        end
    else action reject/amend/delegate
        EA->>EA: record_decision, rejected_count+=1<br/>elevation_actor.rs:977-996
    end
    loop every PR_POLL_INTERVAL=120s
        POLL->>EA: PollPrs<br/>elevation_actor.rs:1123,1126
        alt no GitHub token configured
            Note right of EA: DEGRADED - concept_elevated can never fire<br/>elevation_actor.rs:1131-1136
        else token present
            EA->>GH: pr_state pr_url
            alt PrState Merged
                GH-->>EA: Merged
                EA->>ACSP: publish build_case_status_update concept_elevated kind 31404<br/>elevation_actor.rs:663,1159-1165
                EA->>EA: repo.set_status case_id elevated<br/>elevation_actor.rs:1172
            else PrState ClosedUnmerged
                GH-->>EA: ClosedUnmerged
                EA->>ACSP: publish elevation_abandoned kind 31404
                EA->>EA: repo.set_status case_id abandoned
            else PrState Open
                GH-->>EA: Open (continue tracking, no event)
            end
        end
    end
```

## VC-24.5 GOV-2 terminal PR state mapping + publish-failure warn path

```mermaid
sequenceDiagram
    autonumber
    participant POLL as PollPrs handler<br/>elevation_actor.rs:1126
    participant MAP as terminal_for_pr_state<br/>elevation_actor.rs:663-670
    participant ACSP as AcspClient.publish
    participant REPO as SqliteEnrichmentRepository.set_status
    POLL->>MAP: terminal_for_pr_state(PrState)
    alt PrState::Merged
        MAP-->>POLL: Some concept_elevated, elevated
    else PrState::ClosedUnmerged
        MAP-->>POLL: Some elevation_abandoned, abandoned
    else PrState::Open
        MAP-->>POLL: None (keep polling, continue)
    end
    opt terminal status resolved
        POLL->>ACSP: publish build_case_status_update PANEL_ID case_id event_status pr_url<br/>elevation_actor.rs:1159-1165
        alt publish Err
            ACSP-->>POLL: Err(e)
            POLL->>POLL: warn GOV-2 31404 publish failed for case_id<br/>elevation_actor.rs:1169
            Note right of POLL: publish failure does NOT block the store write below -<br/>the two facts (forum visibility vs durable terminal status)<br/>are independent, mirroring the writeback_triggered/committed split
        else publish Ok
            ACSP-->>POLL: event id
        end
        POLL->>REPO: set_status case_id store_status<br/>elevation_actor.rs:1172-1176
        alt set_status Err
            REPO-->>POLL: Err(e)
            POLL->>POLL: warn GOV-2 terminal store status persist failed for case_id<br/>elevation_actor.rs:1174-1176
        else set_status Ok
            POLL->>POLL: resolved.push(case_id) - remove from elevating map
        end
    end
    Note over POLL,REPO: DIVERGENCE: ADR-2006 is PARTIAL (BASELINE-architecture.md<br/>ACSP workflow closeout 2026-09-04). This poll has no failure/restart<br/>receipt: a crash between the 31404 publish and set_status leaves the<br/>case tracked only in the in-process elevating HashMap - a process<br/>restart loses it silently, with no signed-event/request correlation<br/>or case-authority record surviving the restart
```

## VC-24.6 DecisionElevationActor — parallel path vs ElevationActor

```mermaid
sequenceDiagram
    autonumber
    participant DS as DecisionService.record_decision<br/>services/decision_service.rs:515 maybe_elevate
    participant SIG as is_significant<br/>services/decision_elevation.rs:69
    participant SINK as ActorElevationSink.elevate<br/>actors/decision_elevation_actor.rs:549
    participant DEA as DecisionElevationActor<br/>actors/decision_elevation_actor.rs:8,459,510
    participant ACSP as AcspClient
    participant GH as GitHubPRService.create_ontology_pr<br/>decision_elevation_actor.rs:397
    Note over DS: governed write door already committed the DecisionRecord<br/>quads via proposal_spine::governed_commit (decision_service.rs:677)<br/>BEFORE maybe_elevate runs - elevation is fire-and-forget, fail-open
    DS->>SIG: is_significant(input, acsp_approved=false)<br/>decision_service.rs:525
    alt not significant (routine/edgeless)
        SIG-->>DS: false
        Note right of DS: runtime-only, no case opened - ADR-050 policy
    else significant (mutation/causal/precedent/influenced edges)
        SIG-->>DS: true
        DS->>SINK: elevate(ElevatedDecision)
        SINK->>DEA: try_send ElevateDecision (actor mailbox, non-blocking)<br/>decision_elevation_actor.rs:551-553
        DEA->>DEA: draft_decision_page, open case CASE_PREFIX vc-decelev-<br/>decision_elevation_actor.rs:41,43,300
        DEA->>ACSP: publish build_action_request kind 31402 PANEL_ID vc-decision-elevation
        ACSP-->>DEA: CaseDecision kind 31403
        alt action approve
            DEA->>GH: create_ontology_pr decision page (NO consistency gate)<br/>decision_elevation_actor.rs:397-420
            Note right of DEA: Deliberately leaner than ElevationActor (module doc :12-14):<br/>decisions are ABox prov:Activity individuals adding no TBox<br/>axioms, so there is NO EL++ Whelk gate here (contrast VC-24.4<br/>approve_with_gate GOV-7)
            GH-->>DEA: pr_url
            DEA->>DEA: elevating.insert case_id TrackedPr (in-memory only)
        else reject/amend/delegate
            DEA->>DEA: rejected_count+=1, publish_state
        end
    end
    loop every PR_POLL_INTERVAL=120s
        DEA->>DEA: PollPrs calls terminal_for_pr_state<br/>decision_elevation_actor.rs:459-465
        alt Merged
            DEA->>ACSP: publish build_case_status_update decision_elevated kind 31404<br/>decision_elevation_actor.rs:510
        else ClosedUnmerged
            DEA->>ACSP: publish decision_abandoned kind 31404
        end
    end
    Note over DEA: DIVERGENCE: unlike ElevationActor, DecisionElevationActor has<br/>NO enrichment_repo / SqliteEnrichmentRepository field at all (grep<br/>confirms) - the GOV-2 terminal event above is published to the forum<br/>but never persisted to any durable store. Pending cases live ONLY in<br/>self.pending (HashMap), so a process restart loses every open decision-<br/>elevation case with no reconciliation path. This is the concrete code<br/>evidence for the ADR-2006 closeout line current elevation processing<br/>owns pending state and for failure/restart receipts unproven
```

## VC-24.7 broker_inbox_handler and decision_handler routes

```mermaid
sequenceDiagram
    autonumber
    participant BR as agentbox broker-bridge.js
    participant CFG as configure_routes scope /broker RequireAuth::power_user<br/>handlers/broker_inbox_handler.rs:163-176
    participant INBOX as inbox<br/>handlers/broker_inbox_handler.rs:133
    participant CASE as case_by_id<br/>handlers/broker_inbox_handler.rs:144
    participant STORE as enrichment_proposals_handler::store<br/>handlers/enrichment_proposals_handler.rs:551-597
    participant DEC as decision_handler.record_decision<br/>handlers/decision_handler.rs:145
    participant DSVC as DecisionService.record_decision<br/>services/decision_service.rs
    BR->>CFG: GET /api/broker/inbox
    CFG->>INBOX: power_user auth passed
    INBOX->>STORE: store::all() ALL_LIMIT=500<br/>enrichment_proposals_handler.rs:560,577
    STORE-->>INBOX: Vec EnrichmentProposal from durable repo (same store as decide route)
    INBOX-->>BR: 200 cases[] total (broker-bridge.js:233 shape)
    BR->>CFG: GET /api/broker/cases/:id
    CFG->>CASE: power_user auth passed
    CASE->>STORE: store::get(id)
    alt found
        STORE-->>CASE: EnrichmentProposal
        CASE-->>BR: 200 BrokerCase
    else not found
        CASE-->>BR: 404 not-found
    end
    BR->>CFG: POST /api/broker/cases/:id/decide (REC-2/D3 control-centre operator path)
    CFG->>CFG: routes to enrichment_proposals_handler::decide_as_operator<br/>broker_inbox_handler.rs:173-175 (see VC-24.3 apply_decision core)
    Note over BR,CASE: decide_as_operator and the X-Agent-Key decide route<br/>funnel through the SAME apply_decision core (VC-24.3) - only<br/>the auth differs (session power-user vs service credential)
    Note over DEC: decision_handler.rs is a DIFFERENT governed surface -<br/>PRD-022 W-B / ADR-048 DecisionRecord graph writes, unrelated to<br/>the broker enrichment-proposal queue above
    DEC->>DEC: auth: RequireAuth::authenticated(), deciding principal =<br/>auth.pubkey ONLY, never a body field (decision_handler.rs:9-13)
    DEC->>DSVC: record_decision(auth.pubkey, DecisionInput, idempotency_key, signature)<br/>decision_handler.rs:165-167
    alt success
        DSVC-->>DEC: decision_urn, quads_written, replayed, receipt, gates
        DEC-->>BR: 200 (see VC-24.6 for maybe_elevate fan-out on this same call)
    else CONFLICT_BLOCKED_PREFIX
        DEC-->>BR: 409 conflict_blocked blockingConflicts preExisting
    else IDEMPOTENCY_CONFLICT_PREFIX
        DEC-->>BR: 409 idempotency_conflict
    else ENVELOPE_REJECTED_PREFIX
        DEC-->>BR: 403 envelope_rejected
    end
```

## VC-24.8 nostr_bead_publisher / nostr_bridge — governance event publish

```mermaid
sequenceDiagram
    autonumber
    participant CALLER as bead lifecycle caller
    participant PUB as NostrBeadPublisher<br/>services/nostr_bead_publisher.rs:29
    participant SRC as source relay NOSTR_RELAY_URL<br/>nostr_bead_publisher.rs:18
    participant BRIDGE as NostrBridge<br/>services/nostr_bridge.rs:28
    participant FORUM as forum relay FORUM_RELAY_URL<br/>nostr_bridge.rs:13
    rect rgb(226,228,246)
    Note over CALLER,SRC: trust boundary: bridge bot keypair VISIONCLAW_NOSTR_PRIVKEY<br/>shared by both publisher and bridge (nostr_bead_publisher.rs:18,<br/>nostr_bridge.rs:12)
    CALLER->>PUB: brief -> debrief cycle complete
    PUB->>PUB: from_env() validates ws:// or wss:// scheme<br/>nostr_bead_publisher.rs:44-50
    PUB->>SRC: publish kind 30001 parameterized-replaceable (NIP-33) d=bead_id
    end
    BRIDGE->>SRC: subscribe kind 30001 bead provenance events
    SRC-->>BRIDGE: event(s)
    BRIDGE->>BRIDGE: re-sign under bridge keypair, preserve source_event tag<br/>nostr_bridge.rs:6-8
    BRIDGE->>FORUM: republish as NIP-29 group message kind 9
    Note over BRIDGE,FORUM: DIVERGENCE: NIP-26 delegation not wired (BASELINE-architecture.md<br/>Known divergences) - nostr_bridge.rs re-signs under the bridge key<br/>rather than delegating the original author's signature - fail-closed<br/>NIP-26 deferred (legacy Phase 5)
```

## VC-24.9 Governance kind map (31400-31405)

```mermaid
flowchart LR
    subgraph kinds["ACSP Nostr kinds - services/acsp/events.rs:18-23"]
        K31400["31400 PanelDefinition"]
        K31401["31401 PanelState"]
        K31402["31402 ActionRequest"]
        K31403["31403 ActionResponse / CaseDecision"]
        K31404["31404 PanelUpdate / CaseStatusUpdate"]
        K31405["31405 PanelRetired"]
    end
    B31400["build_panel_definition<br/>acsp/events.rs:210"] -->|producer| K31400
    B31401["build_panel_state<br/>acsp/events.rs:219"] -->|producer| K31401
    B31402A["ElevationActor.run_cycle<br/>elevation_actor.rs:799"] -->|producer| K31402
    B31402B["DecisionElevationActor.open_case<br/>decision_elevation_actor.rs:300"] -->|producer| K31402
    B31402C["voice_intent_client.build_action_request<br/>voice_intent_client.rs:281"] -->|producer| K31402
    B31403A["build_action_response<br/>acsp/events.rs:288 enrichment_proposals_handler.rs:472"] -->|producer| K31403
    B31404A["build_case_status_update<br/>acsp/events.rs:237 elevation_actor.rs:1163"] -->|producer| K31404
    B31404B["build_panel_update<br/>acsp/events.rs:228"] -->|producer| K31404
    B31405["build_panel_retired<br/>acsp/events.rs:265"] -->|producer| K31405
    K31403 -->|consumer| C31403A["AcspClient.run_decision_subscription<br/>acsp/client.rs:96-133 filters since Timestamp::now"]
    C31403A -->|consumer| C31403B["ElevationActor.Decision handler<br/>elevation_actor.rs (Decision message)"]
    C31403A -->|consumer| C31403C["DecisionElevationActor.Decision handler<br/>decision_elevation_actor.rs:381-455"]
    K31402 -->|consumer| C31402["forum relay agent_registry gate<br/>acsp/client.rs:9-13 (relay-side, not this repo)"]
    Note1["Note: relay only accepts kinds 31400-31402 from<br/>registered pubkeys (acsp/client.rs:9) - 31403/31404/31405<br/>are consumer/admin-only, enforced relay-side"]
```

## VC-24.10 web_contract layer structure (reducer/state/ledger/trail/ritual)

```mermaid
flowchart TB
    subgraph L1["Layer 1 - Reducer - web_contract/reducer.rs"]
        TR["trait ContractReducer<br/>reducer.rs:69 validate + transition"]
        RE["ReducerError<br/>reducer.rs:32"]
    end
    subgraph L2["Layer 2 - State - web_contract/state.rs"]
        CS["CanonicalState<br/>state.rs:30 canonical_json + state_hash SHA-256"]
    end
    subgraph L3["Layer 3 - Ledger - web_contract/ledger.rs"]
        LE["Ledger / LedgerEntry<br/>ledger.rs:28,41 balance_sats integer-only"]
    end
    subgraph L4["Layer 4 - Trail - web_contract/trail.rs"]
        GM["GitMark 5-key envelope VERBATIM<br/>trail.rs:81 at-id genesis nick package repository"]
        BT["Blocktrails RECONSTRUCTED<br/>trail.rs:150 states txo single-use-seal chain"]
    end
    subgraph RIT["Ritual + verify - web_contract/ritual.rs"]
        TL["TrustLevel L0/L1/L2/L3<br/>ritual.rs:59 gate() HARD-REFUSES L2/L3"]
        CH["Checks registry run_all<br/>ritual.rs:111,121"]
        VF["verify() ritual.rs:204"]
    end
    TR -->|transition output| CS
    CS -->|hashed into| GM
    TR -->|balances recomputed| LE
    GM -->|commit SHA chained by| BT
    VF -->|1 recompute reducer replay genesis..event log| TR
    VF -->|2 replay ledger == stored ledger.json| LE
    VF -->|3 assert git-clean vs last gitmark commit SHA| GM
    VF -->|4 confirm trail tip is confirmed tx + prevout spent once| BT
    CH -->|3-gate registry| VF
    TL -->|capability gate| VF
    Note1["Note: this is the ADR-124/128 web-ledger contract<br/>layer (legacy numbering, not in the 20xx governing<br/>pack) - the deploy/verify ritual for gitmark.json /<br/>blocktrails.json payment contracts. It is NOT part of<br/>the ACSP proposal/case state machine (VC-24.1-24.7);<br/>included here only because the brief names it as a<br/>governance-adjacent entry point under src/"]
```

## VC-24.11 ACSP decision-projection client — app_state.rs boot + relay connect

```mermaid
sequenceDiagram
    autonumber
    participant BOOT as AppState::new<br/>app_state.rs:1359-1384
    participant ENV as env FORUM_RELAY_URL + ACSP_PANEL_NOSTR_PRIVKEY|VISIONCLAW_NOSTR_PRIVKEY
    participant ACSP as AcspClient::connect<br/>acsp/client.rs:38
    participant RELAY as forum relay (nostr_sdk Client, auto-reconnect)
    BOOT->>ENV: read FORUM_RELAY_URL, ACSP_PANEL_NOSTR_PRIVKEY.or(VISIONCLAW_NOSTR_PRIVKEY)<br/>app_state.rs:1367-1370
    alt both configured
        BOOT->>ACSP: connect(secret, relay)
        ACSP->>RELAY: add_relay + connect (nostr_sdk relay pool)<br/>acsp/client.rs:43-45
        alt connect Ok
            RELAY-->>ACSP: connected
            ACSP-->>BOOT: Arc AcspClient
            BOOT->>BOOT: acsp_client = Some(client)<br/>app_state.rs:1372 log connected - REST/bridge decisions project as kind-31403
        else connect Err
            ACSP-->>BOOT: Err(e)
            BOOT->>BOOT: acsp_client = None, warn FAILED to connect - degraded, visible<br/>app_state.rs:1376-1379
        end
    else unconfigured
        BOOT->>BOOT: acsp_client = None, info OFF - decisions record forum_projection=skipped<br/>app_state.rs:1381-1383
    end
    Note over BOOT,RELAY: this is the SAME AcspClient type ElevationActor and<br/>DecisionElevationActor each construct independently at their own<br/>startup (elevation_actor.rs:686, decision_elevation_actor.rs) -<br/>three separate relay connections under three different panel<br/>identities can exist concurrently, all using the same bridge secret
```

