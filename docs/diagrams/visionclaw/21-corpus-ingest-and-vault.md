---
id: VC-21
title: Corpus ingest (GitHub/local vault) and the vault-migrate converter
area: visionclaw
governing:
  - docs/VAULT-corpus-format.md
  - docs/BASELINE-architecture.md
adrs: [ADR-2014, ADR-2040, ADR-2041, ADR-2042, ADR-2070]
sources:
  - src/services/github_sync_service.rs
  - src/services/github/content_enhanced.rs
  - src/services/parsers/knowledge_graph_parser.rs
  - src/services/file_service.rs
  - src/services/local_file_sync_service.rs
  - src/bin/sync_local.rs
  - src/bin/sync_github.rs
  - src/bin/validate_md.rs
  - src/actors/graph_state_actor.rs
  - crates/visionclaw-domain/src/vault/mod.rs
  - crates/visionclaw-domain/src/vault/link.rs
  - crates/visionclaw-domain/src/models/node.rs
  - crates/vault-migrate/src/lib.rs
  - crates/vault-migrate/src/main.rs
  - crates/vault-migrate/src/report.rs
  - crates/vault-migrate/src/convert.rs
  - crates/vault-migrate/src/paths.rs
  - crates/vault-migrate/src/frontmatter.rs
  - crates/vault-migrate/src/obsidian.rs
  - crates/vault-migrate/tests/integration.rs
  - crates/visionclaw-domain/src/config/visualisation.rs
  - crates/visionclaw-domain/src/config/graph_type.rs
  - src/protocols/binary_settings_protocol.rs
  - src/actors/optimized_settings_actor.rs
  - src/handlers/settings_handler/helpers.rs
  - src/config/path_accessible_impls.rs
  - data/settings.yaml
  - scripts/pre-commit-validate.sh
  - src/handlers/validation_handler.rs
verified_commit: bed6b617d
---

## VC-21.1 GitHubSyncService::sync_graphs — full ingest path
```mermaid
sequenceDiagram
    autonumber
    participant Bin as sync_github.rs<br/>src/bin/sync_github.rs:65
    participant Svc as GitHubSyncService::sync_graphs<br/>src/services/github_sync_service.rs:264
    participant With as sync_graphs_with<br/>src/services/github_sync_service.rs:272
    participant API as EnhancedContentAPI<br/>src/services/github/content_enhanced.rs:12
    participant SHA as filter_changed_files<br/>src/services/github_sync_service.rs:2113
    participant SQL as sync_db (SQLite)<br/>src/services/github_sync_service.rs:2138
    participant Batch as process_batch_incremental<br/>src/services/github_sync_service.rs:1390
    participant File as process_fetched_file<br/>src/services/github_sync_service.rs:1586
    participant KG as kg_repo (Oxigraph)<br/>src/services/github_sync_service.rs:1511

    Bin->>Svc: sync_graphs()
    Svc->>With: sync_graphs_with(false)
    With->>API: list_markdown_files_via_tree()<br/>content_enhanced.rs:31
    alt Trees API fails
        With->>API: list_markdown_files("")<br/>content_enhanced.rs:161
    end
    With->>With: VaultIndex::from_identities(files)<br/>ADR-2040 sV1 full listing, not the SHA1 subset
    Note over With: INVARIANT: index built before the SHA1 filter or an incremental sync mints phantom stubs for unchanged pages
    With->>With: force_full_sync = force_full_override or base_path_changed or FORCE_FULL_SYNC
    alt FORCE_FULL_SYNC=1 or true (env)
        Note over With: bypasses the SHA1 filter entirely - github_sync_service.rs:325
        With->>With: files_to_process = files.clone()
    else incremental
        With->>SHA: filter_changed_files(files)
        SHA->>SQL: get_file_sha1s()
        SQL-->>SHA: HashMap path to sha
        SHA-->>With: files where sha differs (keyed on full repo path)
    end
    opt force_full_sync
        With->>KG: clear_graph()
    end
    loop chunks(BATCH_SIZE=50) - github_sync_service.rs:32,371
        With->>Batch: process_batch_incremental(batch, vault_ctx)
        loop PARALLEL_FETCHES=8 concurrent downloads
            Batch->>API: fetch_file_content(download_url)
        end
        Batch->>File: process_fetched_file(file, content, vault_ctx)
        File->>File: jsonld_ingest::parse_canonical_entity(content, path)<br/>github_sync_service.rs:1604
        alt Ok(Some(entity)) - JSON-LD Page/Class fence present
            File->>File: build_node_from_entity + page_name_to_id(slug)
            Note over File: canonical path claims the node before the publish gate runs (VAULT V4)
        else Ok(None) - no JSON-LD blocks
            File->>File: process_plain_vault_file(file, content, vault_ctx)<br/>github_sync_service.rs:1780
            Note over File: see VC-21.2 for the inclusion gate applied here
        else Err(parse failure)
            File->>File: skip (debug log only)
        end
        Batch->>KG: batch_add_nodes(real_nodes) / batch_add_nodes_if_absent(stub_nodes)<br/>github_sync_service.rs:1511,1522
        Batch->>KG: batch_add_edges(immediate_edges) - same-batch endpoints only
    end
    With->>With: deferred_edges partition: resolvable vs dangling<br/>github_sync_service.rs:1826
    With->>KG: batch_add_edges(resolvable)
    Note over With,KG: DIVERGENCE: dangling wikilinks mint no linked_page stub - they fold into wikilink_count weight plus co-citation springs (FANOUT_NODE_THRESHOLD, default 3)
    With->>SQL: update_file_metadata(all_files_to_process)
    With-->>Bin: SyncStatistics{total_files,kg_files_processed,total_nodes,total_edges,errors}
```

## VC-21.2 Frontmatter inclusion gate (ADR-2040 s V4, supersedes ADR-2014)
```mermaid
sequenceDiagram
    autonumber
    participant Plain as process_plain_vault_file<br/>src/services/github_sync_service.rs:1780
    participant Parser as KnowledgeGraphParser::parse_with_index<br/>src/services/parsers/knowledge_graph_parser.rs:83
    participant Vault as vault::parse<br/>crates/visionclaw-domain/src/vault/mod.rs:149
    participant Gate as page_is_kg_included<br/>src/services/github_sync_service.rs:2304

    Plain->>Parser: parse_with_index(content, vault_path, vault_index)
    Parser->>Vault: vault::parse(content) -> PageMeta{public,owl_class,...}
    Plain->>Plain: is_ontology = parsed.nodes[0].owl_class_iri.is_some()
    alt is_ontology == true
        Note over Plain: ADR-2014: formal data (owl-class) ingests unconditionally, bypasses publish gate
        Plain->>Plain: node ingested regardless of public flag
    else is_ontology == false
        Plain->>Gate: page_is_kg_included(content)
        Gate->>Vault: vault::parse(content).is_kg_included()
        alt frontmatter public: true (YAML boolean)
            Vault-->>Gate: true
        else frontmatter owl-class non-empty
            Vault-->>Gate: true
            Note over Vault: ADR-2040 D2: owl-class in frontmatter also satisfies the gate directly, not only via canonical JSON-LD path
        else legacy Logseq public:: true in LEADING property block only
            Vault-->>Gate: true
            Note over Vault: ADR-2040 D3: bounded legacy tolerance, ends at review_trigger (first sync of converted corpus, or 2026-12-01)
        else public:: true found mid-body or inside a fence
            Vault-->>Gate: false
            Note over Vault: DIVERGENCE: narrower than pre-ADR-2040 is_public_file, which matched public:: true ANYWHERE (file_service.rs legacy) - deliberate narrowing
        else absent both public and owl-class
            Vault-->>Gate: false
            Note over Vault: INVARIANT: fail-closed - no frontmatter or public absent/false and no owl-class means private
        end
        alt Gate returns false
            Plain-->>Plain: skip page (debug log, no node/edge emitted)
        else Gate returns true
            Plain-->>Plain: node ingested as page / linked_page population
        end
    end
    Note over Gate: RESOLVED ADR-2070: the VAULT readers table now cites github_sync_service.rs:2304 for this fn (call site :1822)
    Note over Plain: superseded owl:class:: (double-colon, ADR-2014 original) still recognised only via legacy_properties_anywhere enrichment (file_service.rs:728), never the gate itself
```

## VC-21.3 Node-type derivation: file to page / linked_page / owl_*
```mermaid
flowchart TD
    F["markdown file (post-gate)"] --> J{"JSON-LD Page/Class fence?<br/>jsonld_ingest::parse_canonical_entity"}
    J -->|Ok some entity| PN["build_node_from_entity<br/>metadata[type]=page (canonical)"]
    J -->|Ok none, plain vault page| KP["KnowledgeGraphParser::create_page_node<br/>knowledge_graph_parser.rs:145"]
    KP --> OC{"meta.owl_class.is_some()?<br/>vault::parse(content).owl_class"}
    OC -->|yes| ON["node_type = ontology_node<br/>owl_class_iri set<br/>knowledge_graph_parser.rs:198"]
    OC -->|no| PG["node_type = page<br/>metadata[public]=true"]
    KP --> WL["[[wikilink]] targets"]
    WL --> VR{"VaultContext::resolve target<br/>crates/visionclaw-domain/src/vault/link.rs:215"}
    VR -->|exact identity match, target has /| RES1["resolves to real page identity"]
    VR -->|bare target, exactly one basename match| RES2["resolves to that page's full identity"]
    VR -->|bare target, several matches| RES3["same-folder wins, else first in sorted path order (ambiguity reported)"]
    VR -->|no match anywhere| STUB["linked_page stub id"]
    STUB -.->|DIVERGENCE| DROP["dropped at ingest - process_plain_vault_file:1780 skips is_stub nodes, folds into wikilink_count + co-citation weight instead"]
    PN --> CLS["GraphStateActor::classify_node<br/>src/actors/graph_state_actor.rs:238"]
    ON --> CLS
    PG --> CLS
    CLS --> POP["Node::population_type()<br/>crates/visionclaw-domain/src/models/node.rs:271"]
    POP -->|page or linked_page| KGSET["knowledge_node_ids"]
    POP -->|owl_class or ontology_node| OCSET["ontology_class_ids"]
    POP -->|owl_individual| OISET["ontology_individual_ids"]
    POP -->|owl_property| OPSET["ontology_property_ids"]
    POP -->|agent or bot| AGSET["agent_node_ids"]
    POP -->|unknown, owl_class_iri set| OCSET
    POP -->|unknown, no owl_class_iri| KGSET
```

## VC-21.4 local_file_sync_service + sync_local binary
```mermaid
sequenceDiagram
    autonumber
    participant Bin as sync_local.rs<br/>src/bin/sync_local.rs:75
    participant Svc as LocalFileSyncService::sync_with_github_delta<br/>src/services/local_file_sync_service.rs:103
    participant Scan as scan_local_pages<br/>src/services/local_file_sync_service.rs:319
    participant SHA as calculate_file_sha1<br/>src/services/local_file_sync_service.rs:368
    participant GH as fetch_github_sha_map / fetch_and_update_file<br/>src/services/local_file_sync_service.rs:347,379
    participant Proc as process_file_content<br/>src/services/local_file_sync_service.rs:414
    participant KGP as KnowledgeGraphParser::parse<br/>src/services/parsers/knowledge_graph_parser.rs:68
    participant Onto as OntologyParser::parse + onto_repo.save_ontology

    Bin->>Svc: sync_with_github_delta()
    Svc->>Scan: scan_local_pages() over LOCAL_PAGES_DIR="/app/data/pages"<br/>local_file_sync_service.rs:31
    Svc->>GH: fetch_github_sha_map()
    loop for each local file
        Svc->>SHA: calculate_file_sha1(local_file)
        alt github_sha differs from local_sha
            Svc->>GH: fetch_and_update_file(local_file, file_name)
        else unchanged or GitHub unavailable
            Svc->>Svc: fs::read_to_string(local_file)
        end
        Svc->>Proc: process_file_content(file_name, content, sha)
        alt vault inclusion gate - public: true or a non-empty owl-class<br/>local_file_sync_service.rs:510 via page_is_kg_included :44
            Proc->>KGP: kg_parser.parse(content, file_name)
            Note over Proc: RESOLVED ADR-2096 (2026-09-05): the gate is visionclaw_domain::vault::parse(content).is_kg_included()<br/>the same delegation FileService and GitHubSyncService use - frontmatter is now seen and quoted markers no longer leak
            Proc->>Proc: enrichment_service.enrich_graph(parsed)
            Proc->>Proc: stats.kg_files_processed += 1
        end
        alt content contains ONTOLOGY_BLOCK_MARKER "### OntologyBlock"<br/>local_file_sync_service.rs:34,553
            Proc->>Onto: onto_parser.parse(content, file_name)
            Onto->>Onto: onto_repo.save_ontology(classes, properties, axioms)
            Proc->>Proc: stats.ontology_files_processed += 1
        end
        opt neither public:: nor OntologyBlock present
            Proc->>Proc: stats.skipped_files += 1
        end
        opt (index+1) % BATCH_SIZE==0 or last file - BATCH_SIZE=50, local_file_sync_service.rs:30
            Svc->>Svc: save_batch(nodes, edges)
        end
    end
    Svc-->>Bin: SyncStatistics
    Bin->>Bin: println! summary (files_synced_from_local, files_updated_from_github, ...)
```

## VC-21.5 sync_github binary (full remote pull)
```mermaid
sequenceDiagram
    autonumber
    participant Main as sync_github.rs::main<br/>src/bin/sync_github.rs:17
    participant Onto as OxigraphOntologyRepository::open<br/>src/bin/sync_github.rs:29
    participant KGR as OxigraphGraphRepository::from_store<br/>src/bin/sync_github.rs:33
    participant SQL as SqliteSettingsRepository::open<br/>src/bin/sync_github.rs:41
    participant GHC as GitHubClient::new + GitHubConfig::from_env<br/>src/bin/sync_github.rs:47-50
    participant Svc as GitHubSyncService::new<br/>src/bin/sync_github.rs:53

    Main->>Main: dotenvy::dotenv()
    Main->>Onto: open(DATA_DIR/oxigraph) - DATA_DIR default "./data"
    Main->>KGR: from_store(onto_repo.store().clone())
    Main->>SQL: open(DATA_DIR/settings.sqlite3)
    Main->>GHC: GitHubConfig::from_env() then GitHubClient::new(config, settings)
    Main->>Svc: GitHubSyncService::new(content_api, kg_repo, onto_repo, sqlite_settings_repo)
    Main->>Svc: sync_graphs()
    Note over Main,Svc: see VC-21.1 for the full sync_graphs sequence
    Svc-->>Main: SyncStatistics
    Main->>Main: println! total_files/kg_files_processed/ontology_files_processed/total_nodes/total_edges
    opt stats.errors non-empty
        Main->>Main: print first 10 errors, then "...and N more"
    end
```

## VC-21.6 validate_md binary (JSON-LD validator CLI)
```mermaid
sequenceDiagram
    autonumber
    participant Main as validate_md.rs::main<br/>src/bin/validate_md.rs:27
    participant V as Validator::new<br/>crates/visionclaw-ontology/src/services/jsonld_validator/mod.rs:236
    participant VF as Validator::validate_markdown_file<br/>crates/visionclaw-ontology/src/services/jsonld_validator/mod.rs:249

    Main->>Main: args = env::args().skip(1)
    alt args empty
        Main-->>Main: eprintln usage, ExitCode::from(2)
    end
    Main->>V: Validator::new() - CanonicalContext::v1 + OwlProfile::default
    alt validator init fails
        Main-->>Main: eprintln error, ExitCode::from(2)
    end
    loop for each path argument
        Main->>VF: validate_markdown_file(path)
        VF->>VF: extract_jsonld_blocks(markdown)
        alt blocks empty and contains_unfenced_jsonld(markdown)
            VF-->>Main: ValidationIssue::error(MissingCodeFenceMarker)
        end
        loop for each fenced json-ld block
            VF->>VF: serde_json::from_str + validate_jsonld_block
        end
        VF-->>Main: Vec~ValidationIssue~
        alt issues empty
            Main->>Main: print path OK
        else has issues
            loop each issue
                alt severity Error
                    Main->>Main: total_errors+=1, print_issue
                else severity Warning
                    Main->>Main: total_warnings+=1, print_issue
                end
            end
        end
    end
    Main->>Main: print_summary(total_files, total_errors, total_warnings, failed_files)
    alt total_errors > 0
        Main-->>Main: ExitCode::from(1)
    else
        Main-->>Main: ExitCode::SUCCESS
    end
    Note over Main: INVARIANT: exit 1 on any Error-severity issue is the contract pre-commit-validate.sh depends on (validate_md.rs:12-13)
```

## VC-21.7 vault-migrate: one file through conversion
```mermaid
sequenceDiagram
    autonumber
    participant Run as run(opts)<br/>crates/vault-migrate/src/lib.rs:105
    participant Map as paths::map_page<br/>crates/vault-migrate/src/paths.rs:57
    participant Conv as convert::convert_page<br/>crates/vault-migrate/src/convert.rs:39
    participant FM as frontmatter::parse_leading_block/map_properties<br/>crates/vault-migrate/src/frontmatter.rs:80
    participant Coll as resolve_collisions<br/>crates/vault-migrate/src/lib.rs:361
    participant Rep as Report<br/>crates/vault-migrate/src/report.rs:57
    participant FS as write_atomically / apply<br/>crates/vault-migrate/src/lib.rs:653,694

    Run->>Run: validate(opts) - graph is dir, in_place xor out, dirty-tree refusal<br/>lib.rs:502
    Run->>Run: collect_tree(source/pages) - walkdir follow_links(true)<br/>lib.rs:551
    Run->>Map: map_page(rel) for each pages/ file
    alt already Ns/Title.md (folder layout)
        Map-->>Run: (same path, page_name) - idempotent
    else Ns___Title.md or Ns%2FTitle.md
        Map-->>Run: (Ns/Title.md, "Ns/Title") - decode_page_name
    else not a .md file
        Map-->>Run: None -> Action::Copy verbatim
    end
    Run->>Conv: convert_page(text, page_name) [rayon par_iter]
    Conv->>FM: parse_existing(lines) - already-Obsidian frontmatter check
    alt opens with closed --- block
        FM-->>Conv: stats.already_obsidian = true
    end
    Conv->>FM: parse_leading_block(rest) + map_properties(pairs)
    Conv->>Conv: merge_existing_wins(existing, mapped)
    alt title echoes page identity or its leaf, no confirming H1
        Conv->>Conv: fm.map.remove(title) then stats.title_echo_removed += 1
    end
    alt no public key AND body declares "public:: true"
        Conv->>Conv: fm.map.insert(public, true) then stats.public_promoted_from_body += 1
        Note over Conv: pre-vault reader parity - vault gate reads frontmatter only, so promote rather than silently privatise
    end
    Conv->>Conv: body::rewrite(body_lines) - tasks, embeds, multiword tags, asset paths
    Conv-->>Run: PageResult{content, stats}
    Run->>Run: actions.push(Action::Write{rel, content, source})
    Run->>Run: claimed = starter .obsidian/ config only where nothing claims the path<br/>lib.rs:287-294
    Run->>Coll: resolve_collisions(actions, on_collision, source)
    alt CollisionPolicy::Fail and collisions non-empty
        Coll-->>Run: bail! collision_failure_message - refuses the run, writes nothing
    else CollisionPolicy::Suffix
        Coll->>Coll: next_free_suffixed - deterministic " (2)", " (3)" naming
    end
    alt opts.check
        Run->>Run: differs(action, dest) per action - content or size compare
    else opts.dry_run
        Run->>Run: skip write, report only
    else write mode
        Run->>FS: apply(action, dest) -> write_atomically (tmp + rename)
        opt opts.in_place
            Run->>Run: rename_moved_originals - remove pre-move original only after new path verified non-empty<br/>lib.rs:738
        end
    end
    Run->>Rep: tally(rep, stats) + collect_leftovers(rel, stats)
    Run-->>Run: RunOutcome{report, drift, drift_examples}
    Note over Coll: DIVERGENCE ADR-2042 converter closeout - a synthetic mixed-layout collision was shown to map two pages<br/>to one output with exit zero and one body lost - destination uniqueness and input/output accounting<br/>are required before promotion (docs/VAULT-corpus-format.md Converter closeout qualification)
    Note over Rep: DIVERGENCE the same closeout records that an explicit --report PATH still writes during --dry-run -<br/>the code declares this the one permitted side effect and records it in report_side_effects (report.rs:68-72)
```

## VC-21.8 vault-migrate: per-file lifecycle
```mermaid
stateDiagram-v2
    [*] --> Discovered
    Discovered --> Classified: paths.map_page / map_journal (lib.rs:148,165)
    Classified --> CopiedVerbatim: hidden dir, non-md, dot-directory, or whiteboards
    Classified --> Converting: convert_page / convert_journal (convert.rs:39,45)
    Converting --> FrontmatterMapped: frontmatter.parse_leading_block + map_properties
    FrontmatterMapped --> BodyRewritten: body.rewrite (convert.rs:157)
    BodyRewritten --> Planned: Action.Write pushed (lib.rs:260)
    CopiedVerbatim --> Planned: Action.Copy pushed
    Planned --> CollisionChecked: resolve_collisions (lib.rs:305)
    CollisionChecked --> Rejected: on_collision=fail and collision found
    CollisionChecked --> Suffixed: on_collision=suffix
    CollisionChecked --> Unclaimed: no collision
    Suffixed --> Unclaimed
    Unclaimed --> DriftCompared: opts.check
    Unclaimed --> Skipped: opts.dry_run
    Unclaimed --> Written: write mode - write_atomically/copy_atomically (lib.rs:653,665)
    Written --> OriginalRemoved: opts.in_place and new path verified non-empty (lib.rs:738)
    Written --> [*]
    OriginalRemoved --> [*]
    DriftCompared --> [*]
    Skipped --> [*]
    Rejected --> [*]
    note right of Rejected
        ADR-2042: never silently keeps one body and
        discards the other - refuses the whole run
    end note
```

## VC-21.9 vault-migrate report structs
```mermaid
classDiagram
    class Report {
        +String source
        +String output
        +String mode
        +usize pages_total
        +usize pages_converted
        +usize pages_already_obsidian
        +Rules rules
        +Leftovers leftovers
        +List~Collision~ collisions
        +List~String~ report_side_effects
        +List~String~ errors
        +to_json() String
        +summary() String
    }
    class Rules {
        +usize public_true
        +usize aliases
        +usize namespace_moved
        +usize journals_renamed
        +usize embeds
        +usize tasks
        +usize multiword_tags
        +usize asset_paths
        +usize collapsed_dropped
        +usize id_dropped
        +usize title_echo_removed
        +usize public_promoted_from_body
    }
    class Leftovers {
        +List~FileCount~ block_refs
        +List~FileCount~ body_properties
        +List~FileCount~ scheduled_deadline
        +List~String~ whiteboards
    }
    class FileCount {
        +String file
        +usize count
    }
    class Collision {
        +String destination
        +List~String~ sources
        +String resolution
    }
    Report --> Rules
    Report --> Leftovers
    Report --> Collision
    Leftovers --> FileCount
    note for Report "report.rs:57 - governing doc V6, ADR-2042 D4: machine-readable evidence artefact, never guesses"
```

## VC-21.10 V1 vault layout (Obsidian vault root)
```mermaid
flowchart TD
    ROOT["VAULT_ROOT (env) - single path authority<br/>VAULT-corpus-format.md sV1"] --> OBS[".obsidian/ - app/appearance/core-plugins/community-plugins/hotkeys committed"]
    ROOT --> PAGES["pages/ - authored pages, GitHub sync base path stays 'pages'"]
    ROOT --> JOUR["journals/YYYY-MM-DD.md - excluded from KG ingest"]
    ROOT --> ASSETS["assets/ - vault-root-relative links"]
    ROOT --> TEMPL["templates/ (optional)"]
    PAGES --> TITLE["<Title>.md"]
    PAGES --> NS["<Namespace>/<Title>.md<br/>was <Namespace>___<Title>.md"]
    JOUR -.->|excluded at listing time| SKIP1["content_enhanced.rs:120 entry_path.contains(/journals/)"]
    OBS -.->|excluded at listing time| SKIP2["content_enhanced.rs:121-122 .obsidian and .trash (ADR-2040 D6)"]
    NS -.->|vault-migrate decodes| DECODE["decode_page_name: ___ and %2F/%2f -> / (paths.rs:24)"]
    subgraph excl["Listing exclusions (content_enhanced.rs:117-122, :235-241)"]
        BAK["/bak/"]
        LOGSEQ["/logseq/"]
        RECYCLE["/.recycle/"]
        TRASH["/.trash/"]
    end
    ROOT -.-> excl
    note1["Path authority: agentbox vault manifest section - see AB-05"]
```

## VC-21.11 Settings and wire vocabulary — the bounded `logseq` alias (V7, ADR-2041)
```mermaid
flowchart TD
    Persisted["persisted settings.yaml<br/>visualisation.graphs.logseq.* (legacy) or .knowledge.* (current)"]
    Persisted --> Serde["GraphsSettings.knowledge<br/>serde(alias = logseq)<br/>crates/visionclaw-domain/src/config/visualisation.rs:513-517"]
    Serde --> Canon["canonical in-memory key: knowledge"]
    Norm["normalise_graph_type<br/>logseq or knowledge maps to knowledge<br/>config/graph_type.rs:17"] --> Canon
    Lookup["graph lookup helpers<br/>get knowledge else get logseq<br/>graph_type.rs:26,32"] --> Canon
    PathMatch["path matcher accepts both segments<br/>.graphs.knowledge. or .graphs.logseq.<br/>graph_type.rs:38"] --> Canon
    Wire["binary settings wire path rewrite<br/>.graphs.logseq. replaced with .graphs.knowledge.<br/>src/protocols/binary_settings_protocol.rs:84-85"] --> Canon
    Actor["OptimizedSettingsActor accepts both<br/>src/actors/optimized_settings_actor.rs:516,527<br/>physics prefix branch :940-941"] --> Canon
    Helpers["settings_handler helpers get knowledge else logseq<br/>src/handlers/settings_handler/helpers.rs:34"] --> Canon
    Valid["validation_handler get knowledge else logseq<br/>src/handlers/validation_handler.rs:157"] --> Canon
    Access["path_accessible_impls match knowledge or logseq<br/>src/config/path_accessible_impls.rs:160"] --> Canon
    Canon --> Client["client renders the knowledge graph with the same colours (EXP-V06)"]
    Inv["INVARIANT: the serde alias is the whole compatibility surface -<br/>one canonical key on write, both accepted on read (graph_type.rs:1-9)"]
    Canon --> Inv
    Div["DIVERGENCE ADR-2041 settings-migration acceptance - scoped complete/staged rename only.<br/>Eleven helper/migration tests pass, but actual persistence, patches, mixed-version transport,<br/>key precedence, malformed values, binary registration order, restart/rollback and a named<br/>alias-retirement release still need separate receipts"]
    Inv --> Div
```

## VC-21.12 `owl-class` class-marker grammar — the typed inclusion half
```mermaid
sequenceDiagram
    autonumber
    participant Y as frontmatter scalar<br/>crates/visionclaw-domain/src/vault/mod.rs:378
    participant CM as is_class_marker<br/>crates/visionclaw-domain/src/vault/mod.rs:419
    participant PM as PageMeta<br/>crates/visionclaw-domain/src/vault/mod.rs:64
    participant G as is_kg_included<br/>crates/visionclaw-domain/src/vault/mod.rs:129
    Y->>CM: candidate value for owl-class or owl:class (mod.rs:378-381)
    CM->>CM: trim, reject any whitespace or control character (mod.rs:419)
    alt absolute IRI beginning http:// https:// or urn:
        CM-->>PM: accepted, meta.owl_class = Some(v) (mod.rs:380,552)
    else CURIE prefix:local - prefix starts with a letter then letters digits _ - . and local non-empty
        CM-->>PM: accepted, meta.owl_class = Some(v) (mod.rs:380,552)
    else bare word with no colon, or a coerced YAML boolean or number
        CM-->>PM: rejected, meta.owl_class_rejected = Some(v) retained verbatim (mod.rs:381,553)
    end
    PM->>G: is_kg_included()
    alt public true or owl_class is Some
        G-->>PM: true - page becomes a KG node (mod.rs:129-130)
    else public false or absent and owl_class None
        G-->>PM: false - fail-closed, a rejected marker does NOT open the gate (mod.rs:71-74)
    end
    Note over CM,G: INVARIANT the class marker is a policy, not any scalar that renders to a non-empty string -<br/>owl_class_rejected keeps the offending value so an author can be told why (mod.rs:68-75)
    Note over CM: RESOLVED ADR-2070 the VAULT Inclusion closeout qualification now records this as closed -<br/>is_class_marker (mod.rs:419) enforces an absolute IRI or prefix:local CURIE grammar and rejects into<br/>owl_class_rejected (mod.rs:68-75), so owl-class true and owl-class 42 shut the gate
    Note over G: DIVERGENCE that same closeout also records that an explicit public false plus a class marker<br/>remains included by policy - is_kg_included is an OR, so a valid owl-class overrides public false (mod.rs:129-130)
```
