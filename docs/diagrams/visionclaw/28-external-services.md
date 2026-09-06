---
id: VC-28
title: External services — outbound integrations
area: visionclaw
governing:
  - docs/BASELINE-architecture.md
adrs: [ADR-2066]
sources:
  - src/services/ragflow_service.rs
  - src/handlers/ragflow_handler.rs
  - src/services/perplexity_service.rs
  - src/handlers/image_gen_handler.rs
  - src/services/github_pr_service.rs
  - src/services/github/config.rs
  - src/services/speech_service.rs
  - src/handlers/quic_transport_handler.rs
  - src/app_state.rs
  - src/config/feature_access.rs
  - src/handlers/fastwebsockets_handler.rs
  - src/handlers/mod.rs
verified_commit: 7a20db228
---
## VC-28.1 ragflow_service — outbound RAGFlow agent API
```mermaid
sequenceDiagram
    autonumber
    participant H as ragflow_handler
    participant RS as RAGFlowService<br/>src/services/ragflow_service.rs:94
    participant ENV as env
    participant RF as RAGFlow API<br/>base_url/api/v1/agents

    H->>RS: RAGFlowService::new(settings)<br/>src/services/ragflow_service.rs:102
    RS->>ENV: env::var RAGFLOW_API_KEY:107
    RS->>ENV: env::var RAGFLOW_API_BASE_URL:118
    RS->>ENV: env::var RAGFLOW_AGENT_ID:129
    alt any var missing or empty (144-165)
        RS-->>H: Err RAGFlowError::ParseError
    else all present
        RS-->>H: Ok(RAGFlowService{client,api_key,base_url,agent_id})
    end

    rect rgb(225,230,250)
    Note over RS,RF: trust boundary - outbound HTTPS to configured RAGFLOW_API_BASE_URL
    H->>RS: create_session(user_id) - :177
    RS->>RF: POST {base_url}/api/v1/agents/{agent_id}/sessions?user_id=.. - :189<br/>header Authorization Bearer api_key
    alt status is_success - :199
        RF-->>RS: 200 {data:{id}}
        RS-->>H: Ok(session_id) via result[data][id] - :203
    else non-2xx - :212
        RF-->>RS: status + body
        RS-->>H: Err StatusError(status,body) - :218
    else JSON missing data.id - :205
        RS-->>H: Err ParseError(Failed to parse session ID) - :207
    end
    end

    rect rgb(225,230,250)
    H->>RS: send_chat_message(session_id,message,stream_preference) - :362
    RS->>RF: POST {base_url}/api/v1/agents/{agent_id}/completions - :386<br/>CompletionRequest{question,stream,session_id,sync_dsl:false}
    alt !status.is_success - :396
        RF-->>RS: status + body
        RS-->>H: Err StatusError - :402
    else stream_preference=false - :405
        RF-->>RS: 200 {data:{answer,session_id}}
        alt data.answer present - :415-419
            RS-->>H: Ok(ChatResponse::Buffered{answer,session_id}) - :433
        else data.answer missing - :420-423
            RS-->>H: Err ParseError Answer not found in non-streamed RAGFlow response - :421-423
        end
        Note right of RS: DOC-CORRECTED 2026-09-05: the missing-answer ParseError branch is now drawn<br/>code at ragflow_service.rs:414-423 was already correct - the diagram omitted it
    else stream_preference=true - :438
        RF-->>RS: SSE bytes_stream "data: {...}"
        loop each SSE line - :446
            RS->>RS: parse JSON, extract data.answer chunk - :465-475
        end
        RS-->>H: Ok(ChatResponse::Streaming(byte_stream)) - :499
    end
    end
```
## VC-28.2 ragflow_handler — routes and dead handler
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant CFG as config()<br/>src/handlers/ragflow_handler.rs:620
    participant H as EnhancedRagFlowHandler<br/>:192
    participant N as NostrService
    participant RS as RAGFlowService

    Note over CFG: scope /ragflow registers 6 routes - :625-638
    C->>CFG: POST /ragflow/session -> create_session - :139,626
    C->>CFG: POST /ragflow/message -> send_message - :51,627
    C->>CFG: POST /ragflow/chat -> handler.chat_enhanced - :208,628
    C->>CFG: POST /ragflow/session/enhanced -> create_session_enhanced - :415,632
    C->>CFG: GET /ragflow/history/{session_id} -> get_session_history - :170,635
    C->>CFG: GET /ragflow/history/enhanced/{session_id} -> get_session_history_enhanced - :475,636

    rect rgb(225,230,250)
    C->>H: chat_enhanced(req,state,payload) - :208
    alt rate limit exceeded - :216
        H-->>C: 429 TooManyRequests - :221
    else payload > MAX_REQUEST_SIZE - :229
        H-->>C: 413 PayloadTooLarge - :231
    else missing X-Nostr-Pubkey - :245
        H-->>C: 401 Unauthorized - :251
    else missing Authorization Bearer - :383
        H-->>C: 401 Unauthorized - :393
    else nostr_service.validate_session fails - :275
        H->>N: validate_session(pubkey,token) - :275
        N-->>H: false
        H-->>C: 401 invalid_session - :280
    else no ragflow access and not power user - :289
        H-->>C: 403 insufficient_permissions - :294
    else state.ragflow_service is None - :342
        H-->>C: 503 RAGFlow service unavailable
    else ok
        H->>RS: create_session or send_chat_message (see VC-28.1)
        RS-->>H: ChatResponse
        H-->>C: 200 JSON or text/event-stream
    end
    end

    Note over H: RESOLVED ADR-2066: handle_ragflow_chat was defined but wired into no route<br/>(zero route references tree-wide) and has been deleted, with its now-unused<br/>RagflowChatRequest/Responder imports. RagflowChatResponse is kept - chat_enhanced uses it.
```
## VC-28.3 perplexity_service — outbound Perplexity query API
```mermaid
sequenceDiagram
    autonumber
    participant Caller
    participant PS as PerplexityService<br/>src/services/perplexity_service.rs:35
    participant SET as AppFullSettings.perplexity
    participant PX as Perplexity API<br/>settings.perplexity.api_url

    Caller->>PS: PerplexityService::new_with_settings(settings) - :53
    PS->>SET: read perplexity.timeout - :62
    PS->>PS: Client::builder().timeout(secs) - :66-68<br/>default 30s if unset - :63

    rect rgb(225,230,250)
    Caller->>PS: query(query,conversation_id) - :92
    PS->>SET: settings.perplexity - :99
    alt perplexity config is None - :101
        PS-->>Caller: Err Perplexity settings not configured - :104
    else api_url missing - :112
        PS-->>Caller: Err Perplexity API URL not configured
    else api_key missing - :116
        PS-->>Caller: Err Perplexity API Key not configured
    else model missing - :120
        PS-->>Caller: Err Perplexity model not configured
    else configured
        PS->>PX: POST api_url - :137<br/>header Authorization Bearer api_key<br/>QueryRequest{query,conversation_id,model,max_tokens,temperature,top_p,presence_penalty,frequency_penalty}
        alt !status.is_success - :144
            PX-->>PS: status + body
            PS-->>Caller: Err Perplexity API error - :150-153
        else 200
            PX-->>PS: {content,link}
            PS-->>Caller: Ok(content) - :157
        end
    end
    end
    Note over PS: DOC-CORRECTED 2026-09-05: endpoint config is read from AppFullSettings.perplexity - api_url, api_key, model at :99-120<br/>not from PERPLEXITY_* env vars - only PERPLEXITY_ENABLED_PUBKEYS gates feature access at feature_access.rs:20 and<br/>PERPLEXITY_API_KEY is read solely as a readiness expectation signal at app_state.rs:1516 - configuration.md:220 and<br/>how-to/agent-orchestration.md:449 corrected
```
## VC-28.4 image_gen_handler — ComfyUI submit path (user session)
```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant IG as submit_image_job<br/>src/handlers/image_gen_handler.rs:317-318
    participant ENV as env
    participant CF as ComfyUI native API<br/>COMFYUI_URL :31
    participant SOLID as Solid proxy<br/>SOLID_INTERNAL_URL :41

    IG->>ENV: comfyui_base() = COMFYUI_URL or http://comfyui:8188 - :30-32
    C->>IG: POST /image-gen/submit - :778
    alt no Nostr session - :278
        IG-->>C: 401 Authentication required - :281
    else authed
        IG->>IG: build_flux2_workflow(body,seed,prefix) - :296
        IG->>CF: POST {comfyui_base}/prompt - :312<br/>{prompt:workflow,client_id:job_id} - client timeout 300s :299
        alt connection error - :318
            CF-->>IG: reqwest Err
            IG-->>C: 503 ComfyUI unreachable - :320
        else non-2xx - :327
            CF-->>IG: status + body
            IG-->>C: 400 ComfyUI rejected workflow - :330
        else no prompt_id in response - :346
            IG-->>C: 500 No prompt_id in ComfyUI response - :349
        else accepted - :356
            loop attempt 0..60 - sleep 5s - :363-364
                IG->>CF: GET {comfyui_base}/history/{prompt_id} - :366
                CF-->>IG: {[prompt_id]:{outputs}} or poll error (logged, retried) - :368-372
            end
            alt output_filename still None after 60 attempts (~5min) - :399
                IG-->>C: 504 GatewayTimeout - :402
            else found
                IG->>CF: GET {comfyui_base}/view?filename=..&subfolder=..&type=output - :413,419
                alt GET or bytes() fails - :422,429
                    IG-->>C: 500 Failed to fetch/GET image - :423,430
                else bytes ok
                    IG->>SOLID: PUT {solid_base}/api/solid/pods/{npub}/{folder}/{job}.png - :448<br/>header Authorization forwarded from client - :451-457
                    alt PUT succeeds (2xx or 201) - :463
                        SOLID-->>IG: stored
                        IG-->>C: 200 {job_id,pod_image_url,comfyui_filename,seed} - :477
                    else PUT fails or errors - :467,471
                        SOLID-->>IG: non-2xx or reqwest Err
                        IG-->>C: 200 pod_image_url:null - store skipped, warn logged - :468,472
                    end
                end
            end
        end
    end
```
## VC-28.5 image_gen_handler — ComfyUI Salad agent path, status, health
```mermaid
sequenceDiagram
    autonumber
    participant A as MCP Agent
    participant AS as agent_submit_image_job<br/>src/handlers/image_gen_handler.rs:535-541
    participant ENV as env
    participant SAL as ComfyUI Salad wrapper<br/>COMFYUI_SALAD_URL :36
    participant POD as embedded solid-pod-rs

    ENV-->>AS: agent_key() = VISIONCLAW_AGENT_KEY or changeme-agent-key - :45-47
    A->>AS: POST /image-gen/agent-submit - :779<br/>header X-Agent-Key
    alt X-Agent-Key != agent_key() - :505
        AS-->>A: 401 Invalid or missing X-Agent-Key - :506
    else authed
        AS->>SAL: POST {comfyui_salad}/prompt - :549<br/>{prompt:workflow} - client timeout 360s :536
        alt unreachable - :555
            SAL-->>AS: reqwest Err
            AS-->>A: 503 ComfyUI Salad API unreachable - :557
        else non-2xx - :563
            AS-->>A: 400 ComfyUI rejected workflow - :565
        else no images array - :592
            AS-->>A: 500 No images in Salad response - :595
        else base64 decode fails - :619
            AS-->>A: 500 Failed to decode base64 image - :621
        else ok
            SAL-->>AS: {id,images:[base64],filenames,stats} - :570
            opt feature solid-pod-embed - :646
                AS->>POD: storage.exists/create_container/put - :665-680
                POD-->>AS: Ok(url) or None on failure (warn, non-fatal) - :686-688
            end
            AS-->>A: 200 {job_id,pod_image_url,comfyui_filename,seed} - :629
        end
    end

    Note over AS: DIVERGENCE: try_store_in_pod is a no-op returning None when<br/>the solid-pod-embed feature is disabled - :693-702

    participant GJ as get_job_status<br/>:705
    A->>GJ: GET /image-gen/status/{job_id} - :780
    GJ->>SAL: GET {comfyui_base}/history/{job_id} - :708,710
    alt request errors - :732
        GJ-->>A: 503 ComfyUI unreachable
    else body has job_id key - :714
        GJ-->>A: 200 status completed + outputs - :715
    else not found yet - :721
        GJ-->>A: 200 status pending or unknown by HTTP status - :723-727
    end

    participant HL as health<br/>:740
    A->>HL: GET /image-gen/health - :777
    HL->>SAL: GET {comfyui_base}/system_stats - :747 (timeout 5s :742)
    alt 2xx - :751
        HL-->>A: 200 status ok, vram_free/total - :753-758
    else non-2xx or unreachable - :760,764
        HL-->>A: 200 status degraded (never a 5xx) - :760-768
    end
```
## VC-28.6 github_pr_service — outbound GitHub REST API (git data + PR)
```mermaid
sequenceDiagram
    autonumber
    participant Caller as ElevationActor/agent
    participant GH as GitHubPRService<br/>src/services/github_pr_service.rs:21
    participant ENV as env
    participant API as api.github.com<br/>:245

    GH->>ENV: GitHubPRService::new() - :127<br/>token via github_token_from_env() - config.rs:30<br/>PRIVATE_REPO_GITHUB_PAT or legacy LOGSEQ_PRIVATE_REPO_GITHUB - config.rs:23,25
    GH->>ENV: GITHUB_OWNER/GITHUB_REPO_OWNER - :129-133
    GH->>ENV: GITHUB_REPO/GITHUB_REPO_NAME - :135-139
    GH->>ENV: GITHUB_BRANCH/GITHUB_BASE_BRANCH default main - :141-145

    rect rgb(225,230,250)
    Caller->>GH: create_ontology_pr(file_path,content,title,body,agent_ctx) - :273
    alt token.is_empty() - :281
        GH-->>Caller: Err PRIVATE_REPO_GITHUB_PAT not configured - :282
    else configured
        GH->>API: GET repos/{owner}/{repo}/git/ref/heads/{base_branch} - get_ref_sha :324,328
        GH->>API: POST git/blobs {content,encoding:utf-8} - create_blob :348,357
        GH->>API: POST git/trees {base_tree,tree:[{path,mode:100644,type:blob,sha}]} - create_tree :378,395
        GH->>API: POST git/commits {message,tree,parents:[base_sha]} - create_commit :418,431
        GH->>API: POST git/refs {ref:refs/heads/{branch},sha} - create_ref :454,461
        alt create_ref 422 branch exists - :472
            GH->>API: PATCH git/refs/heads/{branch} {sha,force:true} - update_ref :489,497
        end
        GH->>API: POST pulls {title,body,head,base,labels:[ontology,agent-proposed]} - create_pull_request :516,531
        alt create_pull_request 422 PR exists - :543
            GH->>API: GET pulls?head={owner}:{branch}&state=open - get_existing_pr_url :563,571
        end
        alt any step non-2xx (not the handled 422s) - :364,404,440,470,540
            API-->>GH: status + body
            GH-->>Caller: Err "<step> failed (<status>): <body>"
        else json parse fails on any response
            GH-->>Caller: Err "Failed to parse <x> response: <e>"
        else success
            GH-->>Caller: Ok(pr.html_url) - :321,559
        end
    end
    end

    rect rgb(225,230,250)
    Caller->>GH: pr_state(pr_ref) - GOV-2 poll :207
    alt token empty - :208
        GH-->>Caller: Err cannot poll PR state - :210
    else no PR number parseable - :213
        GH-->>Caller: Err cannot extract a PR number - :214
    else
        GH->>API: GET pulls/{number} - :215,219
        alt non-2xx - :225
            GH-->>Caller: Err Get PR state failed - :228
        else 200
            API-->>GH: {state,merged_at,merged} - :231
            GH->>GH: classify_pr_state - :195<br/>merged_at/merged=true -> Merged, closed -> ClosedUnmerged, else Open
            GH-->>Caller: Ok(PrState)
        end
    end
    end
    Note over GH,API: every request carries Authorization Bearer token + Accept<br/>application/vnd.github+json + User-Agent VisionClaw-OntologyAgent/1.0 - headers() :250-267
```
## VC-28.7 speech_service — outbound boundary only (see VC-35 for full pipeline)
```mermaid
sequenceDiagram
    autonumber
    participant SS as SpeechService<br/>src/services/speech_service.rs:32
    participant OAI as OpenAI TTS/STT<br/>api.openai.com
    participant KOK as Kokoro TTS<br/>settings.kokoro.api_url
    participant WHI as Whisper STT<br/>settings.whisper.api_url
    participant MCP as MCP swarm TCP<br/>MCP_HOST:MCP_TCP_PORT

    Note over SS: TTSProvider::OpenAI/Kokoro - speech.rs:65-69<br/>STTProvider::Whisper/TurboWhisper/OpenAI - speech.rs:71-76

    rect rgb(225,230,250)
    alt TTSProvider::OpenAI - :280
        SS->>OAI: POST https://api.openai.com/v1/audio/speech - :289,301<br/>header Authorization Bearer settings.openai.api_key - :288
        alt api_key or config missing - :348,351
            SS-->>SS: error logged, TTS skipped, no request sent
        else non-2xx or unreachable - :309,321
            OAI-->>SS: status/error
            SS-->>SS: error logged, loop continues - :317,326
        end
    else TTSProvider::Kokoro - :355
        SS->>KOK: POST {kokoro.api_url or http://kokoro-tts-container:8880}/v1/audio/speech - :363-373,389
    else STTProvider::Whisper StartTranscription - :473
        SS->>WHI: uses {whisper.api_url or http://whisper-webui-backend:8000} - :482-485 (ready-check only, no request here)
    else STTProvider::Whisper ProcessAudioChunk - :546
        SS->>WHI: POST multipart to whisper api_url transcription endpoint - :617
        loop poll GET {api_url}/task/{identifier} - max 30 attempts x 200ms - :639-643
            WHI-->>SS: status queued/in_progress/completed/failed - :666
        end
        alt status failed or 30 attempts exceeded - :700,644-648
            SS-->>SS: error logged, transcription dropped
        else completed
            SS-->>SS: broadcast transcription text - :672
        end
    end
    end

    rect rgb(225,230,250)
    Note over SS,MCP: voice command intents relay to the multi-agent swarm over MCP TCP - :1140
    SS->>MCP: call_swarm_init/call_agent_spawn/call_agent_list/call_task_orchestrate<br/>host=MCP_HOST default multi-agent-container, port=MCP_TCP_PORT default 9500 - :1144-1146
    alt call fails (Err) - :1186,1210,1248,1286
        MCP-->>SS: error
        SS-->>SS: "Failed to ... Error: {e}" spoken back to user
    else ok
        MCP-->>SS: swarmId/agents/taskId JSON
        SS-->>SS: formatted confirmation text
    end
    end

    Note over SS: see VC-35 for the full speech pipeline - WS ingress, tag manager,<br/>voice command parsing, TTS response routing are out of scope here
```
## VC-28.8 quic_transport_handler — the postcard wire types that survived ADR-2066
```mermaid
sequenceDiagram
    autonumber
    participant GPU as position source<br/>src/handlers/fastwebsockets_handler.rs:391
    participant PT as postcard wire types<br/>src/handlers/quic_transport_handler.rs:22
    participant WS as fastwebsockets transport<br/>src/handlers/fastwebsockets_handler.rs:34
    participant C as XR/Browser client

    GPU->>PT: PostcardNodeUpdate::from(BinaryNodeData) per node (:38-53, called at fastwebsockets_handler.rs:393)
    PT->>WS: PostcardBatchUpdate { frame_id, timestamp_ms, nodes } (:71-75, built at fastwebsockets_handler.rs:397)
    WS->>WS: postcard::to_stdvec(batch) (fastwebsockets_handler.rs:373)
    WS-)C: binary WebSocket frame over the broadcast channel (fastwebsockets_handler.rs:80, :303)
    C->>WS: inbound frame, postcard::from_bytes::<PostcardBatchUpdate> (fastwebsockets_handler.rs:344, :458)
    WS->>PT: BinaryNodeData::from(PostcardNodeUpdate) back at the boundary (:55-67)

    Note over PT,WS: RESOLVED ADR-2066 (2026-09-05) — QuicTransportServer, QuicClientSession,<br/>QuicServerConfig, CongestionController, ControlMessage, the topology and delta types<br/>and the quinn, rustls and rcgen dependencies were constructed nowhere and routed<br/>nowhere. All are deleted. The file is now 101 lines of wire types only.
    Note over PT: the module is retained solely because fastwebsockets_handler.rs:34 imports<br/>PostcardBatchUpdate and PostcardNodeUpdate directly. src/handlers/mod.rs:117 keeps<br/>pub mod quic_transport_handler and dropped the pub use re-export block.
    Note over C: every client uses the WebSocket path — there is no QUIC listener and no<br/>0-RTT datagram path in the tree. See VC-13 for the live broadcast pipeline.
```
## VC-28.9 consolidated external-dependency map
```mermaid
flowchart LR
    subgraph app["visionclaw_container"]
        RS["RAGFlowService<br/>ragflow_service.rs:94"]
        PS["PerplexityService<br/>perplexity_service.rs:35"]
        IG["image_gen_handler<br/>submit_image_job:272"]
        AG["image_gen_handler<br/>agent_submit_image_job:495"]
        GH["GitHubPRService<br/>github_pr_service.rs:21"]
        SS["SpeechService<br/>speech_service.rs:32"]
    end

    RF["RAGFlow API<br/>env RAGFLOW_API_BASE_URL - no default, boot fails if unset"]
    PX["Perplexity API<br/>settings.perplexity.api_url - no env default"]
    CFU["ComfyUI native<br/>env COMFYUI_URL default http://comfyui:8188"]
    CFS["ComfyUI Salad<br/>env COMFYUI_SALAD_URL default http://comfyui:3000"]
    SOL["Solid proxy<br/>env SOLID_INTERNAL_URL default http://127.0.0.1:4001/api/solid"]
    GHA["api.github.com<br/>env PRIVATE_REPO_GITHUB_PAT / GITHUB_OWNER / GITHUB_REPO"]
    OAI["OpenAI TTS/STT<br/>settings.openai.api_key - no env default, hardcoded URL"]
    KOK["Kokoro TTS<br/>settings.kokoro.api_url default http://kokoro-tts-container:8880"]
    WHI["Whisper STT<br/>settings.whisper.api_url default http://whisper-webui-backend:8000"]
    MCPS["MCP swarm TCP<br/>env MCP_HOST default multi-agent-container, MCP_TCP_PORT default 9500"]

    RS -->|"POST /api/v1/agents/.. - fatal: no fallback"| RF
    PS -->|"POST api_url - degraded: error surfaced to caller"| PX
    IG -->|"POST /prompt, GET /history, GET /view - fatal for that job"| CFU
    AG -->|"POST /prompt - fatal for that job"| CFS
    IG -->|"PUT pods/.. - degraded: pod_image_url null, image still returned"| SOL
    GH -->|"git data + pulls API - fatal: PR/state ops fail"| GHA
    SS -->|"TTS - degraded: audio chunk skipped, chat continues"| OAI
    SS -->|"TTS - degraded: audio chunk skipped"| KOK
    SS -->|"STT - degraded: transcription dropped"| WHI
    SS -->|"voice-command relay - degraded: spoken error reply"| MCPS

    N1["RESOLVED ADR-2066 (2026-09-05) — the QuicTransportServer node and its<br/>unreachable XR QUIC 0-RTT client edge were removed from this map with the<br/>code itself. See VC-28.8 for the postcard wire types that survived."]
    SS --- N1

    classDef fatal fill:#5a1e1e,stroke:#c0392b,color:#fff
    classDef degraded fill:#4a3a10,stroke:#d4a017,color:#fff
    class RF,GHA,CFU,CFS fatal
    class PX,SOL,OAI,KOK,WHI,MCPS degraded
```
