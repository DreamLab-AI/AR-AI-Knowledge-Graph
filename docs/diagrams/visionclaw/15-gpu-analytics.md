---
id: VC-15
title: GPU analytics kernels and pathfinding
area: visionclaw
governing:
  - docs/GPU-wire-abi.md
adrs: [ADR-2007, ADR-2053, ADR-2054, ADR-2061]
sources:
  - src/actors/gpu/clustering_actor.rs
  - src/actors/gpu/analytics_supervisor.rs
  - src/actors/gpu/anomaly_detection_actor.rs
  - src/actors/gpu/pagerank_actor.rs
  - src/actors/gpu/shortest_path_actor.rs
  - src/actors/gpu/connected_components_actor.rs
  - src/actors/gpu/graph_analytics_supervisor.rs
  - src/actors/gpu/gpu_manager_actor.rs
  - src/actors/gpu/resource_supervisor.rs
  - src/actors/graph_service_supervisor.rs
  - src/actors/graph_state_actor.rs
  - src/app_state.rs
  - src/handlers/api_handler/analytics/mod.rs
  - src/handlers/api_handler/analytics/clustering_handlers.rs
  - src/handlers/api_handler/analytics/clustering.rs
  - src/handlers/api_handler/analytics/real_gpu_functions.rs
  - src/handlers/api_handler/analytics/community.rs
  - src/handlers/api_handler/analytics/anomaly.rs
  - src/handlers/api_handler/analytics/pagerank_handlers.rs
  - src/handlers/api_handler/analytics/sssp_handlers.rs
  - src/handlers/api_handler/analytics/pathfinding.rs
  - src/handlers/semantic_pathfinding_handler.rs
  - src/handlers/mod.rs
  - src/services/semantic_pathfinding_service.rs
  - src/adapters/gpu_semantic_analyzer.rs
  - src/adapters/actix_semantic_adapter.rs
  - src/application/semantic_service.rs
  - src/actors/semantic_processor_actor.rs
  - crates/visionclaw-domain/src/ports/gpu_semantic_analyzer.rs
  - src/gpu/visual_analytics.rs
  - src/gpu/mod.rs
  - crates/visionclaw-analytics-oracle/src/lib.rs
  - crates/visionclaw-gpu/src/cuda_sources/gpu_clustering_kernels.cu
  - crates/visionclaw-gpu/src/cuda_sources/pagerank.cu
  - crates/visionclaw-gpu/src/cuda_sources/gpu_landmark_apsp.cu
  - docs/GPU-wire-abi.md
  - crates/visionclaw-gpu/tests/analytics_oracle_conformance.rs
verified_commit: 7a20db228
---

## VC-15.1 POST /analytics/clustering/run — spectral/kmeans/louvain/default dispatch with CPU fallback

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RT as Route<br/>mod.rs:176
    participant RC as run_clustering<br/>clustering_handlers.rs:20
    participant PC as perform_clustering<br/>clustering_handlers.rs:332
    participant RG as real_gpu_functions<br/>real_gpu_functions.rs:100-269
    participant GM as GPUManagerActor<br/>gpu_manager_actor.rs:438
    participant AS as AnalyticsSupervisor<br/>analytics_supervisor.rs:642
    participant CA as ClusteringActor<br/>clustering_actor.rs:1304

    C->>RT: POST /analytics/clustering/run
    RT->>RC: run_clustering(ClusteringRequest)
    RC->>RC: CLUSTERING_TASKS.insert(task_id, status=running) :33-46
    RC-->>C: 200 ClusteringResponse task_id (async, result not ready yet)
    RC->>PC: tokio::spawn perform_clustering :52-53
    PC->>PC: graph_service_addr.send(GetGraphData) :340
    PC->>PC: mcp_client.query_agent_list() :354
    alt request.method == spectral
        PC->>RG: perform_gpu_spectral_clustering :373
    else request.method == kmeans
        PC->>RG: perform_gpu_kmeans_clustering :377
    else request.method == louvain
        PC->>RG: perform_gpu_louvain_clustering :380
    else unrecognised method
        PC->>RG: perform_gpu_default_clustering :384
        Note right of RG: node_count under 100 kmeans<br/>100 to 999 spectral<br/>else louvain (real_gpu_functions.rs:260-268)
    end
    RG->>GM: PerformGPUClustering{method,params,task_id}
    GM->>AS: PerformGPUClustering forward (gpu_manager_actor.rs:438-459)
    AS->>CA: PerformGPUClustering forward (analytics_supervisor.rs:648-666)
    alt method == dbscan
        CA->>CA: perform_dbscan_clustering :516
    else method in [louvain,leiden,communities]
        CA->>CA: perform_community_detection :333
    else
        CA->>CA: perform_kmeans_clustering :176 (kmeans/spectral share the k-means path)
    end
    alt gpu_manager_addr present and GPU call Ok
        CA-->>AS: Ok(cluster list)
        AS-->>GM: Ok(cluster list)
        GM-->>RG: Ok(cluster list)
        RG-->>PC: clusters
    else gpu_manager_addr absent OR GPU call Err
        RG->>RG: generate_cpu_fallback_clustering :334-360
        alt agents non-empty
            RG->>RG: generate_agent_based_clusters (clustering_handlers.rs:398)
        else
            RG->>RG: generate_label_propagation_clusters (real topology CPU fallback)
        end
    end
    PC->>GM: WriteClusterAnalytics{clusters} do_send (clustering_handlers.rs:69)
    GM->>AS: WriteClusterAnalytics forward
    AS->>CA: WriteClusterAnalytics (clustering_actor.rs:1238)
    CA->>CA: write_cluster_id_from_assignments (ADR-031 D3 single writer)
    PC->>PC: CLUSTERING_TASKS[task_id].status = completed :79-81
    Note over PC,RG: DEAD CODE - clustering.rs:6 perform_clustering (a second, unused PerformGPUClustering-only dispatcher with validate_clustering_params) has no caller anywhere in the tree, clustering_handlers.rs:332 is the live perform_clustering
```

## VC-15.2 POST /analytics/clustering/dbscan — direct RunDBSCAN, GPU-only, precondition refusals

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RT as Route<br/>mod.rs:179
    participant RD as run_dbscan_clustering<br/>clustering_handlers.rs:211
    participant GM as GPUManagerActor<br/>gpu_manager_actor.rs:392
    participant AS as AnalyticsSupervisor<br/>analytics_supervisor.rs:538
    participant CA as ClusteringActor<br/>clustering_actor.rs:1209

    C->>RT: POST /analytics/clustering/dbscan {epsilon,minPoints}
    RT->>RD: run_dbscan_clustering(body)
    alt epsilon <= 0.0
        RD-->>C: 200 success=false epsilon must be positive :231-236
    else minPoints == 0
        RD-->>C: 200 success=false minPoints must be at least 1 :237-242
    else gpu_manager_addr is None
        RD-->>C: 200 success=false GPU compute not available :325-328
    else
        RD->>GM: RunDBSCAN{epsilon,min_points}
        GM->>AS: RunDBSCAN forward (gpu_manager_actor.rs:392-412)
        AS->>CA: RunDBSCAN forward (analytics_supervisor.rs:538-566)
        CA->>CA: perform_dbscan_clustering :516
        alt epsilon not finite or <= 0.0
            CA-->>AS: Err epsilon must be finite and > 0 :537-542
        else min_points == 0
            CA-->>AS: Err min_points must be >= 1 :543-545
        else position_clustering_refusal (MIN_NODES_FOR_CLUSTERING=3, MIN_POSITION_SPREAD=1e-3)
            CA-->>AS: Err DBSCAN clustering refused :561-565
        else
            CA->>CA: unified_compute.run_dbscan_clustering(epsilon,min_points) :569
            CA->>CA: record_execution(Dbscan, Gpu) :577
            CA->>CA: write_cluster_id_from_assignments (node_analytics single writer) :642-659
            CA-->>AS: Ok(DBSCANResult)
        end
        AS-->>GM: propagate result
        GM-->>RD: propagate result
        alt Ok
            RD->>RD: CLUSTERING_TASKS.insert(task_id, completed) :264-276
            RD-->>C: 200 success=true clusters, stats, gpuAccelerated=true
        else Err(e)
            RD-->>C: 200 success=false error (clustering_handlers.rs:308-315)
        end
    end
```

## VC-15.3 POST /analytics/community/detect — LabelPropagation/Louvain/Leiden with the modularity gate

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RT as Route<br/>mod.rs:180
    participant RCD as run_community_detection<br/>mod.rs:69
    participant GC as run_gpu_community_detection<br/>community.rs:51
    participant GM as GPUManagerActor<br/>gpu_manager_actor.rs:369
    participant AS as AnalyticsSupervisor<br/>analytics_supervisor.rs:514
    participant CA as ClusteringActor<br/>clustering_actor.rs:1192

    C->>RT: POST /analytics/community/detect {algorithm}
    RT->>RCD: run_community_detection(request)
    RCD->>GC: run_gpu_community_detection(app_state, request)
    alt cfg not(feature = gpu)
        GC-->>RCD: Err GPU features not enabled in this build (community.rs:263-268)
    else app_state.get_gpu_compute_addr() is None
        GC-->>RCD: Err GPU compute actor not available :64
    else
        GC->>GC: map algorithm string to CommunityDetectionAlgorithm :66-76
        GC->>GM: RunCommunityDetection{params}
        GM->>AS: RunCommunityDetection forward (gpu_manager_actor.rs:369-389)
        AS->>CA: RunCommunityDetection forward (analytics_supervisor.rs:514-536)
        CA->>CA: perform_community_detection :333
        alt unified_compute.num_nodes < MIN_NODES_FOR_CLUSTERING (3)
            CA-->>AS: Err community detection refused too few nodes :369-378
        else algorithm == LabelPropagation
            CA->>CA: run_community_detection_label_propagation :385
        else algorithm == Louvain
            CA->>CA: run_louvain_community_detection :396
        else algorithm == Leiden
            CA->>CA: run_leiden_community_detection :410
        end
        CA->>CA: record_execution(kernel, Gpu) per-branch :391/402/415
        alt modularity >= MODULARITY_GATE (0.3)
            CA->>CA: apply_modularity_gate writes node_analytics.community_id :473
            Note over CA: INVARIANT ADR-031 D3 - community detection is the single writer of community_id, cluster_id is untouched here
        else modularity < 0.3
            CA->>CA: partition rejected, community_id reset to 0 :478-484
        end
        CA-->>AS: Ok(CommunityDetectionResult)
        AS-->>GM: propagate
        GM-->>GC: propagate
        GC->>GC: convert_gpu_result_to_communities :97
        GC-->>RCD: Ok(CommunityDetectionResponse)
    end
    alt Ok
        RCD-->>C: 200 CommunityDetectionResponse
    else Err(e)
        RCD-->>C: 500 success=false communities=[] modularity=0.0 (mod.rs:78-88)
    end
```

## VC-15.4 POST /analytics/anomaly/detect — LOF / Z-score / DBSCAN-noise structural anomaly

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RT as Route<br/>mod.rs:182
    participant RAD as run_anomaly_detection<br/>mod.rs:93
    participant GA as run_gpu_anomaly_detection<br/>anomaly.rs:25
    participant GM as GPUManagerActor (gpu_manager_addr)
    participant AS as AnalyticsSupervisor<br/>analytics_supervisor.rs:568
    participant ADA as AnomalyDetectionActor<br/>anomaly_detection_actor.rs:112

    C->>RT: POST /analytics/anomaly/detect {method,kNeighbors,threshold}
    RT->>RAD: run_anomaly_detection(request)
    RAD->>GA: run_gpu_anomaly_detection(method,k_neighbors,radius,threshold)
    alt app_state.gpu_manager_addr is None
        GA-->>RAD: Err GPU manager actor not available :40
    else unsupported method string
        GA-->>RAD: Err Unsupported anomaly detection method :45
    else
        GA->>GA: validate_anomaly_params :59
        GA->>GM: RunAnomalyDetection{params}
        GM->>AS: RunAnomalyDetection forward
        AS->>ADA: RunAnomalyDetection forward (analytics_supervisor.rs:568-591)
        alt ADA.shared_context is None
            ADA-->>AS: Err GPU not initialized :121-126
        else ADA.gpu_state.num_nodes == 0
            ADA-->>AS: Err No nodes available for anomaly detection :131-134
        else k_neighbors >= num_nodes
            ADA-->>AS: Err k_neighbors must be less than total nodes :141-146
        else
            alt method == LocalOutlierFactor
                ADA->>ADA: run_lof_anomaly_detection(k,threshold) :201
                ADA->>ADA: record_execution(Lof, Gpu) :206
            else method == ZScore
                ADA->>ADA: get_node_positions then run_zscore_anomaly_detection :231-246
            else method == DBSCAN (internal, spatial-outlier heuristic)
                ADA->>ADA: run_dbscan_clustering(eps,min_pts=3), label==-1 is anomaly :275-289
            end
            ADA->>ADA: node_analytics.anomaly = anomaly_score (single writer) :381-412
            ADA-->>AS: Ok(AnomalyResult)
        end
        AS-->>GM: propagate
        GM-->>GA: propagate
        alt Ok(result)
            GA->>GA: convert_gpu_anomaly_result_to_anomalies :74
            GA-->>RAD: Ok(Vec anomalies)
        else Err(e)
            GA-->>RAD: Err(e) :81
        end
    end
    alt Ok
        RAD-->>C: 200 success=true anomalies, total, method
    else Err(e)
        RAD-->>C: 500 success=false anomalies=[] total=0 (mod.rs:119-129)
    end
```

## VC-15.5 PageRank compute / cached result / cache clear — direct-addr, bypasses AnalyticsSupervisor

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RTc as Route POST /pagerank/compute<br/>mod.rs:206
    participant RTr as Route GET /pagerank/result<br/>mod.rs:266
    participant RTx as Route POST /pagerank/clear<br/>mod.rs:210
    participant CP as compute_pagerank<br/>pagerank_handlers.rs:112
    participant GP as get_pagerank_result<br/>pagerank_handlers.rs:181
    participant CL as clear_pagerank_cache<br/>pagerank_handlers.rs:236
    participant PRA as PageRankActor (data.analytics.pagerank)<br/>pagerank_actor.rs:433

    C->>RTc: POST /analytics/pagerank/compute {dampingFactor,maxIterations}
    RTc->>CP: compute_pagerank(payload)
    alt data.analytics.pagerank is None
        CP-->>C: 500 success=false PageRank actor not available :163-169
    else
        CP->>PRA: ComputePageRank{params} (direct Addr, no GPUManagerActor hop)
        PRA->>PRA: run_pagerank_centrality(damping,max_iter,epsilon,normalize,use_optimized) :477
        PRA->>PRA: record_execution(Pagerank, Gpu) :486
        PRA->>PRA: calculate_statistics, extract_top_nodes(10) :526-534
        PRA->>PRA: last_result cache = Some(result) :546
        PRA->>PRA: publish_centrality (ADR-031 D3 single writer of centrality slot 48) :550
        PRA-->>CP: Ok(PageRankResult)
        CP-->>C: 200 PageRankResponse success=true
    end
    C->>RTr: GET /analytics/pagerank/result
    RTr->>GP: get_pagerank_result
    GP->>PRA: GetPageRankResult
    alt cached=Some
        PRA-->>GP: Some(PageRankResult) (get_cached_result :404)
        GP-->>C: 200 cached=true result
    else cached=None
        PRA-->>GP: None
        GP-->>C: 200 cached=false result=null :198-206
    end
    C->>RTx: POST /analytics/pagerank/clear
    RTx->>CL: clear_pagerank_cache
    CL->>PRA: ClearPageRankCache
    PRA->>PRA: clear_cache sets last_result=None :409
    PRA-->>CL: ()
    CL-->>C: 200 PageRank cache cleared
    Note over CP,PRA: DIVERGENCE - PageRank never routes through GPUManagerActor/AnalyticsSupervisor at runtime even though gpu_manager_actor.rs:744 and analytics_supervisor.rs:593 define Handler-ComputePageRank, nothing sends ComputePageRank to those addresses, only data.analytics.pagerank is used
```

## VC-15.6 POST /analytics/pathfinding/sssp and /pathfinding/apsp — GPU ShortestPathActor, direct-addr

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RTs as Route POST /pathfinding/sssp<br/>mod.rs:213
    participant RTa as Route POST /pathfinding/apsp<br/>mod.rs:217
    participant CS as compute_sssp<br/>pathfinding.rs:135
    participant CAp as compute_apsp<br/>pathfinding.rs:200
    participant SPA as ShortestPathActor (data.shortest_path_actor)<br/>shortest_path_actor.rs:214

    C->>RTs: POST /analytics/pathfinding/sssp {sourceIdx,maxDistance,delta}
    RTs->>CS: compute_sssp(payload)
    alt data.shortest_path_actor is None
        CS-->>C: 500 Shortest path actor not available :177-182
    else
        CS->>SPA: ComputeSSP{source_idx,max_distance,delta}
        alt SPA.shared_context is None
            SPA-->>CS: Err GPU context not initialized :230-232
        else
            SPA->>SPA: unified_compute.run_sssp(source_idx,delta) :239
            SPA->>SPA: record_execution(Sssp, Gpu) :247
            SPA->>SPA: publish distances to node_sssp map, parent=-1 always (no predecessor array) :310-330
            SPA-->>CS: Ok(SSSPResult{distances,nodes_reached,max_distance})
        end
        CS-->>C: 200/500 SSSPResponse
    end
    C->>RTa: POST /analytics/pathfinding/apsp {numLandmarks,seed}
    RTa->>CAp: compute_apsp(payload)
    CAp->>SPA: ComputeAPSP{num_landmarks,seed}
    SPA-->>CAp: Err APSP disabled by NFR-7, O(n^2) memory forbidden, use SSSP per source (shortest_path_actor.rs:359-364)
    CAp-->>C: 500 APSPResponse success=false
    Note over SPA: RESOLVED ADR-2054 - the #if 0 quarantined APSP kernel body is REMOVED. ComputeAPSP is retained and still refuses explicitly under NFR-7, so the documented route returns a clear error rather than 404
    Note over CS,SPA: RESOLVED ADR-2053 - the standalone spawn is gone and the route now sends<br/>ComputeSSP to data.gpu_manager_addr, forwarded GPUManagerActor to GraphAnalyticsSupervisor<br/>to the SUPERVISED ShortestPathActor, which does receive SetSharedGPUContext.<br/>The supervisor gained Handler-ComputeSSP and Handler-ComputeAPSP because the routes send<br/>those, not ComputeShortestPaths. Landed by vc-core.
```

## VC-15.7 POST /analytics/sssp/compute — CPU Dijkstra via GraphServiceSupervisor, bypasses the GPU actor entirely

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RT as Route<br/>mod.rs:186
    participant CS as compute_sssp<br/>sssp_handlers.rs:165
    participant GSS as GraphServiceSupervisor (graph_service_addr)<br/>graph_service_supervisor.rs:1668
    participant GSA as GraphStateActor<br/>graph_state_actor.rs:1273

    C->>RT: POST /analytics/sssp/compute {sourceNode}
    RT->>CS: compute_sssp(request)
    CS->>GSS: ComputeShortestPaths{source_node_id}
    GSS->>GSA: forward to graph_state (graph_service_supervisor.rs:1677-1687)
    GSA->>GSA: compute_shortest_paths(source_node_id) :749, plain Dijkstra over self.graph_data.edges with a BTreeSet frontier
    alt source_node_id not in node_map
        GSA-->>GSS: Err Source node not found :753-755
    else
        GSA->>GSA: reconstruct predecessor chain into path_map :802-812
        GSA-->>GSS: Ok(PathfindingResult{distances,paths,computation_time_ms})
    end
    GSS-->>CS: propagate
    alt Ok
        CS-->>C: 200 success=true SSSP computation started
    else Err(e)
        CS-->>C: 500 success=false error (sssp_handlers.rs:194-199)
    end
    Note over GSA: DIVERGENCE - this is a second, independent CPU-only SSSP implementation, it never touches ShortestPathActor or the GPU kernel used by VC-15.6, and unlike GPU ComputeSSP it does return reconstructed hop paths
    Note over GSS,GSA: DEAD CODE - GPUManagerActor::Handler-ComputeShortestPaths (gpu_manager_actor.rs:698) and GraphAnalyticsSupervisor::Handler-ComputeShortestPaths (graph_analytics_supervisor.rs:376, converts to ComputeSSP) are unreachable, only sssp_handlers.rs:181 sends ComputeShortestPaths and it targets graph_service_addr, not gpu_manager_addr
```

## VC-15.8 POST /analytics/pathfinding/connected-components — GPU with CPU fallback, direct-addr

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RT as Route<br/>mod.rs:226
    participant CC as compute_connected_components<br/>pathfinding.rs:266
    participant CCA as ConnectedComponentsActor (data.connected_components_actor)<br/>connected_components_actor.rs:233

    C->>RT: POST /analytics/pathfinding/connected-components {maxIterations}
    RT->>CC: compute_connected_components(payload)
    alt data.connected_components_actor is None
        CC-->>C: 500 Connected components actor not available :310-315
    else
        CC->>CCA: ComputeConnectedComponents{max_iterations}
        alt CCA.shared_context is None
            CCA-->>CC: Err GPU context not initialized :291-293
        else
            CCA->>CCA: unified_compute.run_connected_components_gpu(max_iterations) :258
            alt GPU call Ok
                CCA->>CCA: record_execution(ConnectedComponents, Gpu) :261-264
            else GPU call Err(e)
                CCA->>CCA: record_execution(ConnectedComponents, CpuFallback), gated warn :277-280
                CCA->>CCA: compute_components_cpu(num_nodes, self.cached_edges, max_iterations) :282-286
            end
            CCA->>CCA: analyze_components(labels) :296
            CCA-->>CC: Ok(ConnectedComponentsResult{execution_path})
        end
        CC-->>C: 200/500 ConnectedComponentsResponse
    end
    Note over CCA: RESOLVED ADR-2053 - the standalone spawn is gone and the route now sends<br/>ComputeConnectedComponents to data.gpu_manager_addr, forwarded to the SUPERVISED<br/>ConnectedComponentsActor. Stats reads use GetSupervisedComponentsStats, a Result-returning<br/>wrapper, because the bare stats message has no Default and absence must stay explicit.
    Note over CCA: RESOLVED ADR-2054 - UpdateComponentEdges had zero senders and is REMOVED along with its handler, so the degenerate empty-cached_edges CPU path is gone
```

## VC-15.9 POST /analytics/pathfinding/path — point-to-point A* / Bidirectional Dijkstra / Semantic / SSSP dispatch

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant RT as Route<br/>mod.rs:222
    participant PTP as compute_point_to_point<br/>pathfinding.rs:400
    participant GR as graph_repository<br/>pathfinding.rs:410
    participant AS as AStarPathfinder<br/>services/pathfinding.rs
    participant BD as BidirectionalDijkstra<br/>services/pathfinding.rs
    participant SEM as SemanticPathfinder+JaccardEmbedding<br/>services/pathfinding.rs
    participant SPA as ShortestPathActor (data.shortest_path_actor)

    C->>RT: POST /analytics/pathfinding/path {sourceId,targetId,algorithm,query}
    RT->>PTP: compute_point_to_point(payload)
    PTP->>GR: graph_repository.get_graph() :410
    alt graph fetch Err
        PTP-->>C: 500 Failed to retrieve graph data :414-419
    else
        alt algorithm == Astar
            PTP->>AS: AStarPathfinder::find_path(graph,source,target) :424
        else algorithm == Bidirectional
            PTP->>BD: BidirectionalDijkstra::find_path(graph,source,target) :450
        else algorithm == Semantic
            alt query is empty or missing
                PTP-->>C: 500 Query string is required for semantic pathfinding :483-490
            else
                PTP->>SEM: SemanticPathfinder(alpha).find_path(graph,source,target,query) :493-497
            end
        else algorithm == Sssp
            PTP->>PTP: locate source_idx by node id in graph_data.nodes :527-531
            alt source not found
                PTP-->>C: 500 Source node not found in graph :533-542
            else
                PTP->>SPA: ComputeSSP{source_idx} :545-549
                SPA-->>PTP: SSSPResult (GPU path, same actor and same DIVERGENCE as VC-15.6)
            end
        end
        PTP-->>C: 200 PointToPointResponse result (json_val per algorithm)
    end
```

## VC-15.10 POST /pathfinding/semantic-path, /query-traversal, /chunk-traversal — CPU semantic A*

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant CFG as configure_pathfinding_routes<br/>semantic_pathfinding_handler.rs:115
    participant FSP as find_semantic_path<br/>semantic_pathfinding_handler.rs:29
    participant QT as query_traversal<br/>semantic_pathfinding_handler.rs:60
    participant CT as chunk_traversal<br/>semantic_pathfinding_handler.rs:90
    participant GSA as GraphStateActor<br/>graph_state_actor.rs
    participant SVC as SemanticPathfindingService<br/>semantic_pathfinding_service.rs:86

    Note over CFG: mounted at web-scope pathfinding by main.rs:1093, distinct from the analytics-scope path family in VC-15.6 to VC-15.9

    C->>CFG: POST /pathfinding/semantic-path {startId,endId,query}
    CFG->>FSP: find_semantic_path(request)
    FSP->>GSA: GetGraphData
    alt Ok(graph_data)
        FSP->>SVC: find_semantic_path(graph,start_id,end_id,query) :98
        SVC->>SVC: A* over build_adjacency_list with calculate_semantic_cost per edge :108-193
        alt max_explored (1000) reached before goal
            SVC-->>FSP: None :136-138
        else path found
            SVC-->>FSP: Some(PathResult{path,cost,relevance,explanation}) :128-133
        end
        alt Some(path)
            FSP-->>C: 200 path
        else None
            FSP-->>C: error_json No path found :52
        end
    else Err(e)
        FSP-->>C: error_json Graph error :55
    end

    C->>CFG: POST /pathfinding/query-traversal {startId,query,maxNodes}
    CFG->>QT: query_traversal(request)
    QT->>GSA: GetGraphData
    alt query is None
        QT-->>C: error_json Query parameter required :82
    else
        QT->>SVC: query_traversal(graph,start_id,query,max_nodes unwrap_or 50) :202
        SVC-->>QT: results
        QT-->>C: 200 results
    end

    C->>CFG: POST /pathfinding/chunk-traversal {startId,maxNodes}
    CFG->>CT: chunk_traversal(request)
    CT->>GSA: GetGraphData
    CT->>SVC: chunk_traversal(graph,start_id,max_nodes unwrap_or 50) :299
    SVC-->>CT: results
    CT-->>C: 200 results
```

## VC-15.11 Semantic analyzer hexagonal slot — the dead port is gone, the live port remains

```mermaid
classDiagram
    class SemanticAnalyzer {
        <<trait, DEAD - zero impls>>
        run_sssp(graph, source) Result~SSSPResult~
        run_clustering(graph, algorithm) Result~ClusteringResult~
        detect_communities(graph) Result~CommunityResult~
        get_shortest_path(graph, source, target) Result~Vec~u32~~
        invalidate_cache() Result~()~
    }
    note for SemanticAnalyzer "REMOVED: dead port deleted by vc-knowledge on my ADR-2054 routing"

    class GpuSemanticAnalyzer {
        <<trait, LIVE hexagonal port>>
        initialize(graph) Result~()~
        detect_communities(algorithm) Result~CommunityDetectionResult~
        compute_shortest_paths(source_node_id) Result~PathfindingResult~
        compute_sssp_distances(source_node_id) Result~Vec~f32~~
        compute_landmark_apsp(num_landmarks) Result~Vec~Vec~f32~~~
        analyze_node_importance(algorithm) Result~OptimizationResult~
        get_statistics() Result~SemanticStatistics~
    }
    note for GpuSemanticAnalyzer "visionclaw-domain/src/ports/gpu_semantic_analyzer.rs:95"

    class GpuSemanticAnalyzerAdapter {
        gpu_compute: Option~UnifiedGPUCompute~
        sssp_cache: HashMap~u32,Vec~f32~~
        apsp_cache: Option~Vec~Vec~f32~~~
        initialize_gpu(num_nodes, num_edges) Result~()~
        compute_landmark_apsp_internal(num_landmarks)
    }
    note for GpuSemanticAnalyzerAdapter "adapters/gpu_semantic_analyzer.rs:22, real CUDA path via sssp_compact.ptx + gpu_landmark_apsp.ptx"

    class ActixSemanticAdapter {
        actor_addr: Option~Addr~SemanticProcessorActor~~
        timeout: Duration
    }
    note for ActixSemanticAdapter "adapters/actix_semantic_adapter.rs:25, wraps SemanticProcessorActor over an Addr"

    class GpuSemanticAnalyzerAdapter_NoGpu {
        <<cfg not feature gpu, CPU stub>>
        compute_shortest_paths(source_node_id) Result~PathfindingResult~
    }
    note for GpuSemanticAnalyzerAdapter_NoGpu "semantic_processor_actor.rs:49, returns an empty result, no-op initialize"

    class MockSemanticAnalyzer {
        <<test double>>
    }
    note for MockSemanticAnalyzer "application/semantic_service.rs:229"

    GpuSemanticAnalyzer <|.. GpuSemanticAnalyzerAdapter
    GpuSemanticAnalyzer <|.. ActixSemanticAdapter
    GpuSemanticAnalyzer <|.. GpuSemanticAnalyzerAdapter_NoGpu
    GpuSemanticAnalyzer <|.. MockSemanticAnalyzer
```

## VC-15.12 Visual analytics GPU types (src/gpu/visual_analytics.rs) with validate() failure modes

```mermaid
classDiagram
    class Vec4 {
        x: f32
        y: f32
        z: f32
        t: f32
        new(x,y,z,t) Result~Vec4,GPUSafetyError~
        validate() Result~(),GPUSafetyError~
    }
    note for Vec4 "line 19, rejects non-finite components and abs greater than 1e6 (MAX_VAL, line 34)"

    class TSNode {
        position: Vec4
        velocity: Vec4
        trajectory: Vec4_8
        hierarchy_level: i32
        children: i32_4
        layer_membership: f32_16
        topology: f32_32
        betweenness_centrality: f32
        validate() Result~(),GPUSafetyError~
    }
    note for TSNode "line 81, validate at line 149 checks trajectory finiteness and hierarchy bounds"

    class TSEdge {
        source: i32
        target: i32
        weight_history: f32_8
        control_points: Vec4_2
        new(source,target) Result~TSEdge,GPUSafetyError~
        validate(max_nodes) Result~(),GPUSafetyError~
    }
    note for TSEdge "line 273, rejects negative index, self-loop, out-of-bounds index, negative weights"

    class IsolationLayer {
        layer_id: i32
        opacity: f32
        focus_center: Vec4
        focus_radius: f32
        temporal_range: f32_2
        new(layer_id) IsolationLayer
        validate() Result~(),GPUSafetyError~
    }
    note for IsolationLayer "line 406, rejects opacity outside 0..1 and temporal_range start greater than end"

    class VisualAnalyticsParams {
        total_nodes: i32
        force_scale: f32_4
        rest_length: f32
        primary_focus_node: i32
        embedding_dims: i32
        camera_position: Vec4
        validate() Result~(),GPUSafetyError~
    }
    note for VisualAnalyticsParams "line 538, default at 588, validate at 643 covers node/edge counts and force scales"

    class VisualAnalyticsGPU {
        device: Arc_CudaDevice
        nodes: CudaSlice_TSNode
        edges: CudaSlice_TSEdge
        safety_validator: Arc_GPUSafetyValidator
        max_nodes: usize
        new(max_nodes,max_edges,max_layers,cfg) Result~Self,GPUSafetyError~
    }
    note for VisualAnalyticsGPU "line 830, new() rejects zero nodes/edges, greater than 10M nodes, greater than 50M edges"

    class GPUSafetyError {
        <<enum>>
        InvalidKernelParams
        BufferBoundsExceeded
        ResourceExhaustion
        InvalidBufferSize
        DeviceError
    }

    TSNode --> Vec4
    TSEdge --> Vec4
    IsolationLayer --> Vec4
    VisualAnalyticsParams --> Vec4
    VisualAnalyticsGPU --> TSNode
    VisualAnalyticsGPU --> TSEdge
    VisualAnalyticsGPU --> IsolationLayer
    Vec4 ..> GPUSafetyError
    TSNode ..> GPUSafetyError
    TSEdge ..> GPUSafetyError
    IsolationLayer ..> GPUSafetyError
    VisualAnalyticsParams ..> GPUSafetyError
    VisualAnalyticsGPU ..> GPUSafetyError
    note for VisualAnalyticsGPU "RESOLVED ADR-2054: removed - no caller existed"
```

## VC-15.13 Analytics kernel trust status and the CPU reference oracle

```mermaid
flowchart TD
    subgraph LIVE["Live GPU kernels (crates/visionclaw-gpu/src/cuda_sources)"]
        LOUV["Louvain community detection TRUSTED<br/>gpu_clustering_kernels.cu:581 D1 fix marker, output-verified by ADR-2061"]
        PR["PageRank TRUSTED<br/>pagerank.cu:263 D8 fix marker, global two-kernel dangling-mass path, output-verified by ADR-2061"]
        DBS["DBSCAN TRUSTED<br/>gpu_clustering_kernels.cu:1079 border handling in propagate/finalise, output-verified by ADR-2061"]
        LOF["LOF local outlier factor BROKEN<br/>gpu_clustering_kernels.cu:404-417 lrd floors on the query k-distance not the neighbour<br/>so it computes a k-distance ratio and not Breunig LOF - ADR-2061"]
        ONT["Ontology constraints<br/>fixed and live, keystone wiring"]
    end

    subgraph QUARANTINED["Compile-quarantined / absent"]
        APSP["Landmark APSP<br/>gpu_landmark_apsp.cu:25 #if 0 guard, not shipped enabled"]
        EMB["Node embeddings<br/>no embedding kernels exist in the tree"]
    end

    subgraph ORACLE["Test-time reference oracle, crates/visionclaw-analytics-oracle/src/lib.rs"]
        ENC["encode_record_52 :86 / decode_record_52 :106"]
        FIX["GraphFixture :141 - two_clique :192, triangle :206, star :215, linear_chain :225, canonical_live_scale :246"]
        CPUMOD["CPU reference modularity :303"]
        CPUPR["CPU reference pagerank :342"]
        CPUDB["CPU reference dbscan :392"]
        CPULOF["CPU reference lof :443"]
    end

    LOUV -.->|"conformance test PASSES"| CPUMOD
    PR -.->|"conformance test PASSES"| CPUPR
    DBS -.->|"conformance test PASSES"| CPUDB
    LOF -.->|"conformance test FAILS 1e-3 bar"| CPULOF

    NOTE["PARTIAL ADR-2061 (2026-09-05): kernels are now output-validated against the CPU oracle<br/>PageRank max delta 3.4e-11 and DBSCAN exact and Louvain 16 of 16 communities all TRUSTED<br/>LOF BROKEN at max delta 0.702 against a 1e-3 bar - query k-distance used in place of neighbour<br/>Test crates/visionclaw-gpu/tests/analytics_oracle_conformance.rs and see docs/GPU-wire-abi.md trust table"]
    LIVE -.-> NOTE
    ORACLE -.->|"fixture crate only, not wired into any runtime request path"| LIVE
```
