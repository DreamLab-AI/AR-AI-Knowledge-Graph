---
id: AB-15
title: Capability gating, spend caps and consultants
area: agentbox
governing:
  - agentbox/docs/GOVERNANCE-capabilities.md
  - agentbox/docs/SECURITY-profiles.md
adrs: [ADR-2020, ADR-2031, ADR-2033]
sources:
  - agentbox/agentbox.toml
  - agentbox/management-api/lib/system-manifest.js
  - agentbox/scripts/agentbox-config-validate.js
  - agentbox/services/agentbox-ops/src/cost_cap/mod.rs
  - agentbox/services/agentbox-ops/src/bin/tree-search-cap.rs
  - agentbox/management-api/lib/pay402.js
  - agentbox/management-api/routes/payments.js
  - agentbox/management-api/lib/llm-marketplace.js
  - agentbox/management-api/routes/llm-marketplace.js
  - agentbox/management-api/lib/headroom.js
  - agentbox/management-api/lib/headroom-mcp-tools.js
  - agentbox/management-api/routes/system.js
  - agentbox/mcp/consultants/shared/consultant-base.js
  - agentbox/mcp/consultants/shared/model-diversity.js
  - agentbox/mcp/consultants/shared/spawn-cli.js
  - agentbox/mcp/consultants/shared/memory-logger.js
  - agentbox/mcp/consultants/antigravity/server.js
  - agentbox/services/agentbox-manifest/src/main.rs
  - agentbox/services/agentbox-manifest/src/tomlval.rs
  - agentbox/services/agentbox-manifest/src/tui_write.rs
  - agentbox/config/entrypoint-unified.sh
  - agentbox/skills/tree-search-coder/SKILL.md
  - agentbox/skills/build-with-quality/scripts/deepsec-gate.sh
  - agentbox/docs/adr/ADR-2020-capability-gating.md
  - agentbox/docs/adr/ADR-2031-consultant-model-manifest-projection.md
  - agentbox/docs/adr/ADR-2033-deepsec-security-gate.md
  - agentbox/.deepsec-gate/deepsec.config.mjs
  - agentbox/lib/npm-cli.nix
  - agentbox/mcp/servers/lib/ontology-retrieval.js
  - agentbox/scripts/ci/check-manifest-catalogue.js
  - agentbox/services/agentbox-ops/src/cost_cap/ledger.rs
  - agentbox/skills/mcp.json
  - agentbox/agentbox.sh
  - agentbox/flake.nix
verified_commit: bed6b617d
---

## AB-15.1 Gate lattice — manifest to package set to supervisor to runtime trace
```mermaid
flowchart TD
    subgraph M["agentbox.toml — declared gates"]
        M1["[skills.tree_search_coder]<br/>agentbox.toml:620"]
        M2["[toolchains].deepsec<br/>agentbox.toml:1295-1310"]
        M3["[security.deepsec]<br/>agentbox.toml:1560"]
        M4["[dream_machine]<br/>agentbox.toml:1609"]
        M5["[payments]<br/>agentbox.toml:1062"]
        M6["[consultants.*]<br/>agentbox.toml:861-889"]
    end
    subgraph N["Nix package set — rebuild class"]
        N1["flake.nix deepsecPkg closure"]
        N2["lib/npm-cli.nix makeNpmService"]
    end
    subgraph S["supervisord blocks — boot class"]
        S1["dream-engine program"]
        S2["management-api program"]
    end
    subgraph C["CATALOGUE entries<br/>management-api/lib/system-manifest.js:39"]
        C1["tree-search-coder<br/>apply_class rebuild<br/>:293"]
        C2["deepsec<br/>gates toolchains.deepsec + security.deepsec.enabled<br/>apply_class rebuild<br/>:127"]
        C3["dream-machine<br/>apply_class boot<br/>:198"]
        C4["payments<br/>apply_class boot<br/>:222"]
        C5["consultants<br/>apply_class boot<br/>:227"]
    end
    R["runtime trace<br/>GET /v1/system stateOf()<br/>system-manifest.js:275"]

    M1 -->|"rebuild"| C1
    M2 -->|"rebuild — bakes npm closure"| N1
    N1 --> C2
    M3 -->|"live — policy read at run time"| C2
    M4 -->|"boot — entrypoint reconciles, dream-engine self-reads /etc/agentbox.toml"| S1
    S1 --> C3
    M5 -->|"boot"| S2
    S2 --> C4
    M6 -->|"boot"| S2
    S2 --> C5
    C1 --> R
    C2 --> R
    C3 --> R
    C4 --> R
    C5 --> R
    N2 -.->|"rebuild — aci_shell/tree_search_coder npm closures"| C1

    note1["PROPOSED ADR-2077: a procedure ADR carrying the exact five-build<br/>nix rebuild sequence - closure identity and runtime trace are proved<br/>separately, a gate that cannot pass is a named exception, and the<br/>receipt lands in docs/reference/gap-close-evidence"]
    C1 -.-> note1
```

## AB-15.2 A gated tool call — off, on, and spend-capped branches
```mermaid
sequenceDiagram
    autonumber
    participant Agent as Claude session
    participant Skill as tree-search-coder skill<br/>skills/tree-search-coder/SKILL.md
    participant CLI as tree-search-cap bin<br/>services/agentbox-ops/src/bin/tree-search-cap.rs:1
    participant Limiter as Limiter::reserve<br/>services/agentbox-ops/src/cost_cap/mod.rs:307
    participant Ledger as Ledger #40;flock#41;<br/>services/agentbox-ops/src/cost_cap/ledger.rs

    rect rgb(235,235,235)
    Note over Agent,Skill: Branch A — [skills.tree_search_coder].enabled = false
    Agent->>Skill: invoke tree-search-coder
    Skill->>CLI: tree-search-cap reserve --run r1 --estimate 0.10
    CLI->>Limiter: reserve_at#40;run_id, estimate, now#41;
    Limiter-->>CLI: Err CapabilityDisabled skill=tree_search_coder
    CLI-->>Skill: exit 3, JSON ok=false refused=capability_disabled
    Note over Skill,Agent: INVARIANT: byte-identical-when-off — no candidate dispatched, no kernel session, no ledger row written
    end

    rect rgb(220,240,220)
    Note over Agent,Ledger: Branch B — gate on, under cap
    Agent->>Skill: invoke tree-search-coder N=3
    Skill->>CLI: reserve --run r2 --estimate 0.10
    CLI->>Limiter: reserve_at#40;"r2", 0.10, now#41;
    Limiter->>Ledger: with_state#40;exclusive flock#41;
    Ledger-->>Limiter: run.committed=0.00, outstanding=0.00
    Limiter-->>CLI: Ok Reservation id=res-... remaining_usd=0.40
    CLI-->>Skill: exit 0, reservation JSON
    Skill->>Skill: dispatch sparc:coder branch in fresh kernel
    Skill->>CLI: settle --run r2 --reservation res-... --actual 0.09
    CLI->>Limiter: settle_at#40;stub, 0.09, Completed, now#41;
    Limiter-->>CLI: Settlement committed_usd=0.09 remaining_usd=0.41
    CLI-->>Skill: exit 0
    end

    rect rgb(250,225,225)
    Note over Agent,Ledger: Branch C — gate on, spend-capped refusal
    Agent->>Skill: invoke tree-search-coder branch 6
    Skill->>CLI: reserve --run r3 --estimate 0.13
    CLI->>Limiter: reserve_at#40;"r3", 0.13, now#41;
    Limiter->>Ledger: with_state — committed 0.38 + outstanding 0.00 + 0.13 #62; cap 0.50
    Limiter-->>CLI: Err SpendCapExceeded cap_usd=0.50 committed_usd=0.38 requested_usd=0.13
    CLI-->>Skill: exit 3, JSON refused=spend_cap_exceeded
    Note over Skill: INVARIANT: tree-search-coder never auto-routed, always carries spend_cap_usd (ADR-2020)
    end
```

## AB-15.3 Boot projection — manifest to catalogue to runtime state
```mermaid
sequenceDiagram
    autonumber
    participant TOML as agentbox.toml<br/>agentbox/agentbox.toml:1560
    participant Entry as entrypoint-unified.sh<br/>agentbox/config/entrypoint-unified.sh:1650
    participant MB as agentbox-manifest bin<br/>agentbox/services/agentbox-manifest/src/main.rs:184
    participant Sup as supervisord<br/>dream-engine program
    participant API as management-api<br/>routes/system.js:33
    participant SM as system-manifest.js<br/>CATALOGUE + buildSystemView<br/>:39,:274

    Note over Entry: boot — Phase reconciles every restart
    Entry->>MB: agentbox-manifest toml-string --manifest /etc/agentbox.toml --path consultants.antigravity.model
    MB->>TOML: tomlval::parse_file_lenient(&manifest)<br/>tomlval.rs:56 get()
    alt value present and is string
        MB-->>Entry: prints the model string, exit 0
    else missing, non-string, unparseable manifest
        MB-->>Entry: prints empty string, exit 0
        Note over MB: fail-open — toml-string always exits 0 (main.rs:19)
    end
    Entry->>Entry: export AGENTBOX_ANTIGRAVITY_MODEL only if not already set
    Entry->>Sup: reconcile [dream_machine] enabled — dream-engine reads /etc/agentbox.toml itself at start
    Note over Sup: apply_class boot — dream-engine self-reads the manifest,<br/>so a restart (not a hot edit) is what applies a dream_machine change

    Note over API: live — every GET /v1/system request
    API->>SM: buildSystemView(manifest, adapters)
    loop for each CATALOGUE entry
        SM->>SM: resolveGate(manifest, entry.gate) -- tomlval-equivalent dotted lookup<br/>system-manifest.js:170
        SM->>SM: stateOf(manifest, entry) -- on #124; off #124; available<br/>system-manifest.js:186
    end
    SM-->>API: apply_classes, core, surfaces, modules, counts
    API-->>API: reply.send #40;generated_at, ...view, execution#41;
    Note over SM: RESOLVED ADR-2069 #40;2026-09-05#41;: catalogue drift now FAILS the build.<br/>scripts/ci/check-manifest-catalogue.js rejects any agentbox.toml boolean key that is neither<br/>catalogued nor in an explicit BASELINE, and the BASELINE is a ratchet — an entry that has since<br/>been catalogued, or has left the toml, fails too, so the list can only shrink. 17 baselined keys<br/>are real capabilities still owed a CATALOGUE entry and warn on every run
```

## AB-15.4 Apply-class lifecycle of a gate
```mermaid
stateDiagram-v2
    [*] --> Declared
    Declared: Declared in agentbox.toml, e.g. security.deepsec.enabled
    Declared --> LiveApplied: apply_class live -- operator edits, next request reads it
    Declared --> BootPending: apply_class boot -- entrypoint reconciles every restart
    Declared --> RebuildPending: apply_class rebuild -- changes the Nix image composition

    LiveApplied --> LiveApplied: read at operation time, no restart needed
    note right of LiveApplied
        e.g. security.deepsec fail_on, max_duration -- read per deepsec-gate.sh run
    end note

    BootPending --> BootApplied: agentbox.sh restart -- entrypoint-unified.sh re-runs
    BootApplied --> BootPending: manifest edited again while running -- drift until next restart
    note right of BootApplied
        e.g. dream_machine.enabled -- dream-engine self-reads manifest at process start
    end note

    RebuildPending --> RebuildApplied: agentbox.sh rebuild -- flake.nix recomposes package set plus supervisor block
    note right of RebuildPending
        e.g. toolchains.deepsec, skills.tree_search_coder, skills.aci_shell
        operator MUST rebuild -- a restart alone leaves the old package set
    end note

    LiveApplied --> [*]
    BootApplied --> [*]
    RebuildApplied --> [*]
```

## AB-15.5 tree-search-coder spend cap — manifest to validator to enforcement
```mermaid
sequenceDiagram
    autonumber
    participant Op as Operator
    participant Val as agentbox-config-validate.js<br/>agentbox/scripts/agentbox-config-validate.js:1332
    participant TOML as agentbox.toml<br/>[skills.tree_search_coder]<br/>agentbox.toml:620-625
    participant Skill as tree-search-coder SKILL.md<br/>agentbox/skills/tree-search-coder/SKILL.md:78
    participant CLI as tree-search-cap<br/>src/bin/tree-search-cap.rs:1
    participant Lim as Limiter<br/>cost_cap/mod.rs:307,376

    Op->>Val: node agentbox-config-validate.js agentbox.toml
    Val->>TOML: read max_candidates, per_branch_timeout_s, spend_cap_usd
    alt max_candidates undeclared or #62; 5
        Val-->>Op: W051 token spend scales linearly with N
    end
    alt spend_cap_usd not a positive number
        Val-->>Op: W052 no default-unlimited mode for tree-search
    end
    alt tree_search_coder.enabled and NOT code_interpreter.enabled
        Val-->>Op: E052 branch scoring requires kernel execution
    end
    Note over Val: INVARIANT: tree-search-coder always carries a spend_cap_usd (ADR-2020)

    Op->>Skill: explicit invocation only — Note: NEVER auto-routed (SKILL.md:9)
    Skill->>CLI: tree-search-cap config
    CLI->>Lim: CapConfig::load(manifest)
    Lim-->>CLI: enabled, spend_cap_usd=0.50, max_candidates=5, per_branch_timeout_s=60
    loop candidate 0..N-1 (max_candidates)
        Skill->>CLI: reserve --run RUN_ID --estimate E_i
        CLI->>Lim: reserve_at(RUN_ID, E_i, now)
        alt admitted #62;#61; max_candidates
            Lim-->>CLI: Err CandidateLimitExceeded
            CLI-->>Skill: exit 3
            Note over Skill: halt loop, return best-so-far candidate
        else committed+outstanding+E_i #62; cap
            Lim-->>CLI: Err SpendCapExceeded
            CLI-->>Skill: exit 3
            Note over Skill: halt loop, return best-so-far candidate (SKILL.md:81 enforced spend_cap_usd)
        else granted
            Lim-->>CLI: Ok Reservation
            CLI-->>Skill: exit 0
            Skill->>Skill: sparc:coder generates candidate in fresh KernelSession (ADR-018)
            opt branch exceeds per_branch_timeout_s mid-run
                Skill->>Lim: guard_branch(reservation, now) via CLI wrapper
                Lim-->>Skill: Err BranchTimeout
            end
            Skill->>CLI: settle --run RUN_ID --reservation res-... --actual A_i [--failed]
            CLI->>Lim: settle_at(stub, A_i, outcome, now)
            Lim-->>CLI: Settlement committed_usd, remaining_usd, overran_estimate
        end
    end
    Skill-->>Op: winning candidate, scored by assertion-pass count, tie-break shortest code
```

## AB-15.6 Full HTTP 402 challenge, payment, retry, settle
```mermaid
sequenceDiagram
    autonumber
    participant Agent as Agentbox consumer<br/>[payments.consumer]<br/>agentbox.toml:1081
    participant Merchant as External merchant endpoint
    participant Classify as pay402.classify()<br/>lib/pay402.js:169
    participant Pay as /v1/pay/* routes<br/>routes/payments.js:145-677
    participant Pod as solid-pod-rs<br/>127.0.0.1:8484

    Agent->>Merchant: original request
    Merchant-->>Agent: 402 Payment Required<br/>body.accepts[] or X-Pay-Currency: sats
    Agent->>Classify: classify({status:402, headers, body})
    alt agentbox-ledger — X-Pay-Currency sats + deposit_endpoint, or accepts[] entry
        Classify-->>Agent: scheme agentbox-ledger, payable = CONSUMER_ENABLED==="true"
        Note over Classify: both header and accepts[] present and cost_sats != accepts amount<br/>-> scheme unknown, reason amount-mismatch (pay402.js:198)
    else x402 — integer x402Version + accepts[] with scheme/network
        Classify-->>Agent: scheme x402, payable false #40;version!=1 -> reason unsupported-version#41;
    else l402 — status 401 or 402, WWW-Authenticate L402#124;LSAT
        Classify-->>Agent: scheme l402, payable false always #40;macaroon+invoice required, ln-prefix checked#41;
    else unknown
        Classify-->>Agent: scheme unknown, payable false, reason null — terminal, fail-closed
    end

    rect rgb(220,240,220)
    Note over Agent,Pod: payable path — [payments.consumer].enabled required (agentbox.toml:1082)
    Agent->>Pay: POST /v1/pay/estimate {endpoint, units}
    Pay-->>Agent: 200 estimated_sats, hold_sats = ceil(estimated*HOLD_BUFFER_RATIO)
    Agent->>Pay: GET /v1/pay/balance (NIP-98 auth)
    Pay->>Pod: GET /pay/.balance
    Pod-->>Pay: balance_sats
    Pay-->>Agent: 200 balance_sats, dream_balance
    alt balance_sats #62;#61; required
        Agent->>Merchant: retry original request with payment proof
        Merchant-->>Agent: 200 OK — settled
    else insufficient balance
        Agent->>Pay: POST /v1/pay/deposit {txo_uri, amount_sats}
        Pay->>Pod: POST /pay/.deposit
        Pod-->>Pay: new_balance
        Pay-->>Agent: 200 credited, new_balance, dream_balance
        Agent->>Merchant: retry original request with payment proof
        Merchant-->>Agent: 200 OK — settled
    end
    end
    Note over Agent: DIVERGENCE: #91;payments.consumer#93;.enabled = false by default #40;agentbox.toml:1082#41;,<br/>#91;payments.broadcast#93;.enabled = false and well_known = false #40;agentbox.toml:1090-1092#41; — see AB-11.15
```

## AB-15.7 routes/payments.js — GET info, GET balance, POST deposit
```mermaid
sequenceDiagram
    autonumber
    participant C as Caller
    participant R as paymentRoutes<br/>routes/payments.js:139
    participant Pod as solid-pod-rs<br/>127.0.0.1:8484

    Note over R: all routes behind global onRequest auth hook — bearer or NIP-98 (payments.js:29)

    C->>R: GET /v1/pay/info<br/>payments.js:145
    R->>Pod: GET /pay/.info
    alt pod-rs reachable
        Pod-->>R: 200 upstream chains
        R-->>C: 200 chains, base_cost_sats, dream_per_sat, tiers, operator_did
    else pod-rs unreachable
        Note over R: logged, falls through to local config — never a 502 here (payments.js:172)
        R-->>C: 200 chains=#91;btc#93; #40;local default#41;, tiers from TIER_MULTIPLIERS
    end

    C->>R: GET /v1/pay/balance<br/>payments.js:203
    alt callerDid(req) is null — bearer auth, no identity
        R-->>C: 401 identity-required
    else NIP-98 identity present
        R->>Pod: GET /pay/.balance #40;Authorization forwarded#41;
        alt pod-rs error
            Pod-->>R: non-2xx or unreachable
            R-->>C: err.statusCode or 502 payment-service-error
        else ok
            Pod-->>R: 200 balance_sats
            R-->>C: 200 did, balance_sats, dream_balance=balance_sats*DREAM_PER_SAT, X-Balance header
        end
    end

    C->>R: POST /v1/pay/deposit {txo_uri, amount_sats}<br/>payments.js:278
    alt callerDid(req) is null
        R-->>C: 401 identity-required
    else identified
        R->>Pod: POST /pay/.deposit {txo_uri, amount_sats}
        alt pod-rs error
            Pod-->>R: non-2xx or unreachable
            R-->>C: err.statusCode or 502 payment-service-error
        else ok
            Pod-->>R: 200 new_balance
            R-->>C: 200 credited=true, txo_uri, amount_sats, new_balance, dream_balance
        end
    end
```

## AB-15.8 routes/payments.js — POST estimate, POST buy, POST withdraw
```mermaid
sequenceDiagram
    autonumber
    participant C as Caller
    participant R as paymentRoutes<br/>routes/payments.js:139
    participant Pod as solid-pod-rs<br/>127.0.0.1:8484

    C->>R: POST /v1/pay/estimate {endpoint, units}<br/>payments.js:359
    alt TIER_MULTIPLIERS#91;endpoint#93; undefined
        R-->>C: 400 unknown-tier
    else valid tier
        R->>R: perUnitSats = BASE_COST_SATS*multiplier<br/>hold_sats = ceil#40;estimated*HOLD_BUFFER_RATIO#41;
        R-->>C: 200 estimated_sats, hold_sats, dream_tokens, breakdown #40;local computation, no pod-rs call#41;
    end

    C->>R: POST /v1/pay/buy {amount}<br/>payments.js:446
    alt callerDid(req) is null
        R-->>C: 401 identity-required
    else identified
        R->>Pod: GET /pay/.balance
        alt pod-rs error
            Pod-->>R: non-2xx or unreachable
            R-->>C: err.statusCode or 502 payment-service-error
        else currentBalance #60; costSats #40;ceil#40;amount/DREAM_PER_SAT#41;#41;
            R-->>C: 402 insufficient-balance, required_sats, balance_sats, X-Cost header
        else sufficient balance
            R->>Pod: POST /pay/.buy {dream_amount, cost_sats}
            alt pod-rs error
                Pod-->>R: non-2xx or unreachable
                R-->>C: err.statusCode or 502 payment-service-error
            else ok
                Pod-->>R: 200 new_sat_balance
                R-->>C: 200 purchased=true, dream_amount, cost_sats, new_sat_balance, new_dream_balance
            end
        end
    end

    C->>R: POST /v1/pay/withdraw {amount}<br/>payments.js:569
    alt callerDid(req) is null
        R-->>C: 401 identity-required
    else floor#40;amount/DREAM_PER_SAT#41; #60;#61; 0
        R-->>C: 400 amount-too-small
    else identified and non-zero
        R->>Pod: GET /pay/.balance
        alt pod-rs error
            Pod-->>R: non-2xx or unreachable
            R-->>C: err.statusCode or 502 payment-service-error
        else currentDreamBalance #60; amount
            R-->>C: 402 insufficient-dream-balance, required_dream, dream_balance
        else sufficient DREAM balance
            R->>Pod: POST /pay/.withdraw {dream_amount, sats_received}
            alt pod-rs error
                Pod-->>R: non-2xx or unreachable
                R-->>C: err.statusCode or 502 payment-service-error
            else ok
                Pod-->>R: 200 new_sat_balance
                R-->>C: 200 withdrawn=true, dream_burned, sats_received, new_sat_balance, new_dream_balance
            end
        end
    end
```

## AB-15.9 LLM marketplace — advertise, discover, request, grant, receipt, revoke
```mermaid
sequenceDiagram
    autonumber
    participant Prov as Provider
    participant Cons as Consumer
    participant R as llmMarketplaceRoutes<br/>routes/llm-marketplace.js:37
    participant OB as Orderbook<br/>lib/llm-marketplace.js:214
    participant Gate as authorityGate.guard<br/>see AB-14.x

    Prov->>R: POST /v1/llm/advertise {model, context_window, endpoint}<br/>:84
    alt validateAdvertisement fails
        R-->>Prov: 422 validation_failed
    else valid
        R->>OB: addAdvertisement(pubkey, body) -- key #96;pubkey:model#96;, replaceable (kind 38300)
        R-->>Prov: 200 event, urn, stored=true
    end

    Cons->>R: GET /v1/llm/discover ?min_context_window&max_cost_per_m_token&capabilities<br/>:164
    R->>OB: findMatches(filter) or getAdvertisements(provider)
    OB-->>R: matching advertisements
    R-->>Cons: 200 advertisements, count

    Cons->>R: POST /v1/llm/request {token_budget, min_capabilities, ...}<br/>:205 (kind 38301)
    R->>OB: findMatches(body)
    OB-->>R: candidate ads
    R-->>Cons: 200 event, matches, count

    alt provider grants
        Prov->>R: POST /v1/llm/grant {request_event_id, grantee_pubkey, model, token_allocation}<br/>:259
        R->>OB: addGrant(grantId, {providerPubkey, consumerPubkey, tokenAllocation, tokensUsed:0, expiresAt})
        R-->>Prov: 201 event #40;kind 38302#41;, grant_id, urn
    else provider denies
        Prov->>R: POST /v1/llm/deny {request_event_id, grantee_pubkey, reason}<br/>:325
        R-->>Prov: 200 event #40;kind 38303#41;
    end

    Cons->>R: POST /v1/llm/receipt {grant_id, consumer_pubkey, model, tokens_used}<br/>:362
    R->>OB: recordUsage(grant_id, tokens_used)
    alt tokensUsed + tokens_used #62; tokenAllocation, or grant not found
        OB-->>R: false
        R-->>Cons: 402 budget_exceeded
    else within allocation
        OB-->>R: true, running total updated
        R-->>Cons: 200 event #40;kind 38304#41;, accepted=true, urn
    end

    Prov->>R: POST /v1/llm/revoke {grant_id, reason}<br/>:422
    opt authorityEnabled -- mandate_revoke is a zero-tolerance action class
        R->>Gate: guard#40;actionClass: mandate_revoke#41; -- kind-31402 ActionRequest, blocks for signed kind-31403 (see AB-14.x)
        alt gate.decision != allow
            Gate-->>R: DENY -- fail-closed, no timeout/reject/unverified escape
            R-->>Prov: 403 authority_denied
        end
    end
    R->>OB: revokeGrant(grant_id)
    R-->>Prov: 200 event #40;kind 38305#41;, revoked=true

    Note over R: GET /v1/llm/grants #40;:512, pruneExpired + getActiveGrants#40;pubkey#41;#41; and<br/>GET /v1/llm/stats #40;:534, orderbook.stats#40;#41; + KINDS#41; are read-only summaries, not shown above
```

## AB-15.10 headroom compression-capacity model and its 3 MCP tools
```mermaid
sequenceDiagram
    autonumber
    participant MCP as MCP caller
    participant Tools as headroom-mcp-tools.js<br/>handleTool()<br/>lib/headroom-mcp-tools.js:111
    participant HR as headroom.js<br/>compress/retrieve/stats<br/>lib/headroom.js:1
    participant Addon as headroom_napi.node<br/>#40;Rust N-API addon#41;
    participant TOML as #91;compression#93; #91;compression.slots#93;<br/>agentbox.toml

    Note over HR: DIVERGENCE: this is a context-COMPRESSION capacity model<br/>#40;ttl_minutes, max_entries, target_ratio#41;, not a USD spend/budget cap —<br/>no financial "budget" field exists in headroom.js #40;lib/headroom.js:69-84#41;

    MCP->>Tools: call headroom_compress {content, content_type}<br/>:65
    Tools->>HR: detectContentType#40;content#41; -- when content_type="auto"
    HR->>Addon: addon.detectContentType#40;content#41;
    Tools->>HR: smartCrush #124; compressLog #124; compressDiff#40;content#41; by detected type
    HR->>HR: _readManifestConfig#40;#41; -- caches #91;compression#93; from loadManifest#40;#41;<br/>:56 enabled, backend, ttl_minutes, max_entries, target_ratio
    HR->>TOML: read #91;compression.slots#93;.#123;memory,pods,beads,orchestrator#125;
    Note over HR: INVARIANT: slots.events is hard-coded false always -- the audit trail is never compressed #40;headroom.js:15,77#41;
    HR->>Addon: addon.smartCrush#40;content, {targetRatio}#41; #40;or compressLog/compressDiff#41;
    alt addon absent, compression.enabled=false, or ratio #62;#61; 1.0
        HR-->>Tools: passthrough {compressed:false, content unchanged}
        Note over HR: fail-open contract -- headroom.js:6-9
    else beneficial compression
        Addon-->>HR: {compressed, ratio, ccrEntries}
        HR-->>Tools: {compressed:true, ratio, ccrEntries}
    end
    Tools-->>MCP: {compressed, ratio, ccr_entries}

    MCP->>Tools: call headroom_retrieve {hash}<br/>:34
    Tools->>HR: ccrRetrieve#40;hash#41;
    HR->>Addon: addon.ccrRetrieve#40;hash#41;
    alt entry found
        Addon-->>HR: Buffer
        HR-->>Tools: decompressed content
        Tools-->>MCP: {content, size_bytes}
    else expired or evicted
        Addon-->>HR: null
        HR-->>Tools: null
        Tools-->>MCP: {error: expired}
    end

    MCP->>Tools: call headroom_stats {}<br/>:53
    Tools->>HR: ccrStats#40;#41;
    HR->>Addon: addon.ccrStats#40;#41;
    Addon-->>HR: entries, bytes_stored, hit_count, miss_count
    HR-->>Tools: normalised stats
    Tools-->>MCP: {entries, bytes_stored, hit_count, miss_count, hit_rate}
```

## AB-15.11 Consultant model projection at boot — env wins, TUI preserves (ADR-2031)
```mermaid
sequenceDiagram
    autonumber
    participant PreBoot as Pre-boot environment
    participant Entry as entrypoint-unified.sh<br/>agentbox/config/entrypoint-unified.sh:1650
    participant MB as agentbox-manifest toml-string<br/>services/agentbox-manifest/src/main.rs:264
    participant TOML as agentbox.toml<br/>[consultants.antigravity]<br/>agentbox.toml:867-871
    participant Reg as skills/mcp.json registry default
    participant Srv as antigravity server.js<br/>mcp/consultants/antigravity/server.js:20

    rect rgb(220,240,220)
    Note over PreBoot,Entry: branch A — AGENTBOX_ANTIGRAVITY_MODEL set before boot
    Entry->>Entry: #91; -z "$#123;AGENTBOX_ANTIGRAVITY_MODEL:-#125;" #93; is false -- skip projection
    Note over Entry: INVARIANT: a non-empty pre-boot env override wins (ADR-2031)
    end

    rect rgb(230,230,250)
    Note over Entry,TOML: branch B — env unset, manifest declares a model
    Entry->>MB: agentbox-manifest toml-string --manifest /etc/agentbox.toml --path consultants.antigravity.model
    MB->>TOML: tomlval::get#40;&cfg, "consultants.antigravity.model"#41;
    TOML-->>MB: Value::String#40;"gemini-3.8-flash"#41;
    MB-->>Entry: prints "gemini-3.8-flash", exit 0
    Entry->>Entry: export AGENTBOX_ANTIGRAVITY_MODEL="gemini-3.8-flash"
    end

    rect rgb(250,235,215)
    Note over Entry,TOML: branch C — env unset, manifest missing/non-string/unparseable
    Entry->>MB: agentbox-manifest toml-string ...
    MB->>TOML: parse_file_lenient -- unreadable or key absent
    MB-->>Entry: prints "" #40;empty#41;, exit 0 -- fail-open #40;main.rs:19#41;
    Entry->>Entry: export AGENTBOX_ANTIGRAVITY_MODEL="" -- registry default applies downstream
    end

    Entry->>Srv: spawn consultant-antigravity with AGENTBOX_ANTIGRAVITY_MODEL in env
    Srv->>Srv: const MODEL = process.env.AGENTBOX_ANTIGRAVITY_MODEL #124;#124; 'gemini-3.8-flash' #40;server.js:20#41;
    Srv->>Reg: skills/mcp.json carries its own registry default#40;409#41; -- consulted only if server.js's own fallback is bypassed
    Srv->>Srv: rates#40;now#41; -- tariff only defined for MODEL==='gemini-3.8-flash' #40;server.js:24-28#41;
    alt MODEL has a published tariff
        Srv-->>Srv: multiplier=2 if now #62;#61; 2027-01-01T00:00:00Z else 1
    else no tariff for MODEL
        Srv-->>Srv: rates#40;#41; returns null -> cost_usd: null #40;never an invented figure#41;
    end

    rect rgb(245,220,220)
    Note over TOML: TUI save path — tui_write.rs:37 run#40;#41;
    Note over TOML: if state has no "consultants.antigravity.model" key, carry forward<br/>the EXISTING manifest's value rather than overwrite it #40;tui_write.rs:37-48#41;
    Note over TOML: INVARIANT: a TUI save never resets an operator's model #40;ADR-2031#41;
    end
```

## AB-15.12 Consultant consult() call — consultant-base.js, model-diversity, memory-logger
```mermaid
sequenceDiagram
    autonumber
    participant Caller as Coordinator #40;Claude session#41;
    participant Base as BaseConsultant._handleConsult<br/>mcp/consultants/shared/consultant-base.js:241
    participant Onto as ontoBrain#40;#41;.ask<br/>mcp/servers/lib/ontology-retrieval.js
    participant Spawn as spawnCli<br/>mcp/consultants/shared/spawn-cli.js:29
    participant CLI as agy #124; codex #124; z #40;vendor CLI#41;
    participant Div as model-diversity.js<br/>verificationRecord/selectVerifier<br/>:196,:151
    participant Log as MemoryLogger.log<br/>mcp/consultants/shared/memory-logger.js:66
    participant API as management-api /v1/agent-events/emit

    Caller->>Base: CallToolRequest consult {question, context_excerpt, producer_family, ontology_context}
    Base->>Base: validate question non-empty #40;:242#41;
    opt ontologyAugmentEnabled#40;args#41; -- args.ontology_context===true or CONSULT_ONTOLOGY_AUGMENT=1
        Base->>Onto: ask#40;{query, model_tier:'sonnet', mode:'expand', max_tokens:1500}#41;
        alt ontology call throws
            Onto-->>Base: fail-open -- proceed ungrounded #40;:264 catch#41;
        else turtle returned
            Onto-->>Base: {turtle, seed_iris, tokens_used, degraded}
            Base->>Base: prepend Turtle to context_excerpt, coordinator's excerpt preserved whole
        end
    end
    Base->>Spawn: this.callConsult#40;{question, context_excerpt, format}#41; under _withTimeout#40;timeout_ms#41;
    Spawn->>CLI: spawn#40;cmd, args, env: scrubbed + PASSTHROUGH_ENV TLS/proxy vars#41;<br/>spawn-cli.js:16-24
    alt exit code non-zero or SIGKILL on timeout
        CLI-->>Spawn: {code!=0, killed, stderr}
        Spawn-->>Base: throws -- memlog.log#40;{ok:false, error}#41;, rethrow
    else success
        CLI-->>Spawn: {stdout, code:0}
        Spawn-->>Base: {response, model, tokens, cost_usd, citations}
    end
    Base->>Base: mint consultation_urn via uris.mint#40;kind:'activity', payload:{consultant,question}#41; #40;ADR-013 URN grammar#41;
    opt args.producer_family set -- REC-8 anti-fox closure verification
        Base->>Div: verificationRecord#40;{producerFamily, verifier:this.name, task:'closure-verification'}#41;
        Div-->>Base: {producer_family, verifier_family, anti_fox_ok}
        alt anti_fox_ok===false
            Base->>Base: logger.error -- same-family self-verification WARNING #40;:329-333#41;
        end
    end
    Base->>Log: memlog.log#40;{ok:true, question, model, tokens, cost_usd, latency_ms, verification}#41;
    opt AGENTBOX_INTELLIGENCE_DIR set and response_len is number
        Log->>Log: _writeIntelligenceSignal -- ADR-043 QualitySignal JSON #40;memory-logger.js:87-113#41;
    end
    Base->>API: POST /v1/agent-events/emit #40;Bearer MANAGEMENT_API_KEY#41; -- fire-and-forget, best-effort #40;consultant-base.js:39-43#41;
    Base-->>Caller: envelope {ok, response, model, tokens, cost_usd, citations, consultation_urn, verification}
```

## AB-15.13 deepsec gate — invocation, route selection, egress bound
```mermaid
sequenceDiagram
    autonumber
    participant Op as build-with-quality caller
    participant Gate as deepsec-gate.sh<br/>skills/build-with-quality/scripts/deepsec-gate.sh:1
    participant TOML as [security.deepsec]<br/>agentbox.toml:1560-1571
    participant CLI as deepsec CLI<br/>#40;vercel-labs/deepsec, baked by toolchains.deepsec#41;
    participant Local as claude #124; codex CLI<br/>#40;operator's own login#41;
    participant Loom as Loom façade<br/>http://loom:8080/v1<br/>LAN-only

Op->>Gate: deepsec-gate.sh --diff #60;ref#62; #124; --diff-working #124;<br/>--diff-staged #124; --files-from #60;p#62; #124; --full #124;<br/>--scan-only<br/>:9-14
Gate->>TOML: read [security.deepsec] -- policy source order: manifest -><br/>DEEPSEC_GATE_#60;KEY#62; env -> CLI flags<br/>:64-95
Gate->>Gate: validate<br/>fail_on#40;CRITICAL#124;HIGH#124;MEDIUM#124;HIGH_BUG#124;BUG#124;LOW#41;,<br/>thinking_level, agent, model_auth<br/>:101-104

    alt P_ENABLED != true
Gate-->>Op: exit 78 EX_CONFIG -- "record the gate as SKIPPED, not<br/>passed" #40;:107-110#41;
    else deepsec not on PATH
Gate-->>Op: exit 78 EX_CONFIG -- toolchains.deepsec=true + rebuild<br/>required #40;:111-114#41;
    else available
        alt model_auth=local #40;default#41;
            Gate->>Local: verify claude/codex CLI on PATH #40;unless --scan-only#41;<br/>:122-127
Note over Gate,Local: EGRESS: snippets sent to the operator's OWN<br/>logged-in CLI session -- default route, no separate credential<br/>#40;ADR-2033 D4#41;
            Gate->>Gate: route_json = {mode:local, provider:local}
        else model_auth=direct
Gate->>Gate: require ai_provider anthropic#124;openai + ai_api_key_env<br/>set + credential present in env<br/>:131-137
Note over Gate: EGRESS: direct call to the named external vendor API<br/>using the named env-var credential -- names-only in the manifest, value<br/>stays in process env
        else model_auth=custom
            Gate->>Gate: require agent=pi + ai_api_key_env + ai_base_url<br/>:139-142
Gate->>Loom: route_json = {mode:custom, baseUrl: ai_base_url,<br/>credentialHeader: bearer}
Note over Gate,Loom: EGRESS: LAN-only alternative -- source snippets<br/>leave the box only onto the LAN Loom façade, never the public internet<br/>#40;ADR-2033 D4, GOVERNANCE-capabilities.md Loom section#41;
        end

Gate->>Gate: write .deepsec-gate/deepsec.config.mjs -- names-only route,<br/>gitignored<br/>:170-178
Gate->>CLI: run_bounded#40;timeout --signal=INT --kill-after=30s<br/>$P_MAX_DURATION deepsec scan#124;process ...#41;<br/>:190-196
Note over Gate,CLI: max_duration default 45m -- SIGINT lets deepsec<br/>checkpoint, re-run resumes #40;:14, :192#41;
        alt timeout #40;rc 124 or 137#41;
            CLI-->>Gate: killed
            Gate-->>Op: receipt.json result=TIMEOUT, exit 124 EX_TIMEOUT<br/>:220-227
        else scan-only or full and rc != 0
            CLI-->>Gate: runtime error
            Gate-->>Op: exit 70 EX_RUNTIME<br/>:232
        else PR mode #40;diff/diff-working/diff-staged/files-from#41;
CLI-->>Gate: exit 0 #40;no net-new findings#41; or 1 #40;net-new<br/>findings#41;
Gate->>CLI: deepsec export --format json --min-severity LOW --out<br/>findings.json<br/>:240
Gate->>Gate: bucket findings by severity, blocking = severity index<br/>#60;#61; fail_on index<br/>:253-263
            alt blocking findings present
                Gate-->>Op: receipt.json result=BLOCK, exit 1
            else none
                Gate-->>Op: receipt.json result=PASS, exit 0
            end
        end
    end
Note over Gate: DIVERGENCE: implementation_status partial --<br/>nodeModulesHash is still a placeholder<br/>until the first nix build .#runtime on the host #40;ADR-2033<br/>Verification#41;
```


