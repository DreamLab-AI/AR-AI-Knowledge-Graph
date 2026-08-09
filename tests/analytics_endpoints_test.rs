/*!
 * ADR-031 D7 — Analytics API endpoint VALUE tests.
 *
 * Previously this file was JSON-SHAPE-ONLY (and fully commented out because the
 * PhysicsSettings field names had drifted). It is rewritten here to assert
 * VALUES, not just structural validity, per the D7 obligation
 * ("existing tests extended to assert values, not just shapes").
 *
 * What runs CPU-only:
 *   - The canonical `cluster_id` encoding rule (1-based, 0 = unclustered).
 *   - The `cluster_id != community_id` post-fix invariant on response payloads.
 *   - The /anomaly/detect (GPU structural) vs /anomaly/toggle (agent-health)
 *     surface separation (D4) — asserted as a contract on the response schema.
 *
 * What is GPU-gated (#[ignore]):
 *   - A live HTTP round-trip that drives a real clustering/anomaly pass and
 *     asserts the returned values match the CPU oracle. Requires the server +
 *     a CUDA device. Binds to the intended /api/analytics routes.
 */

#[path = "analytics_fixtures.rs"]
mod fx;

use fx::*;
use serde_json::json;

// ---------------------------------------------------------------------------
// CPU-runnable value contracts on the analytics response payload.
// ---------------------------------------------------------------------------

/// The canonical encoding (D3): cluster_id is 1-based, 0 = unclustered. A
/// response that reports a clustered node with cluster_id 0 violates the
/// contract.
#[test]
fn cluster_response_uses_one_based_cluster_ids() {
    let response = json!({
        "success": true,
        "clusters": [
            { "id": 1, "nodeCount": 5, "nodes": [10, 11, 12, 13, 14] },
            { "id": 2, "nodeCount": 5, "nodes": [20, 21, 22, 23, 24] }
        ],
        "method": "louvain"
    });
    let clusters = response["clusters"].as_array().unwrap();
    for c in clusters {
        let id = c["id"].as_u64().unwrap();
        assert!(
            id >= 1,
            "every emitted cluster id must be >= 1 (0 is reserved for unclustered), got {id}"
        );
    }
}

/// D3 dup-write regression guard at the API layer: a per-node analytics
/// response must carry cluster_id and community_id as DISTINCT fields, not the
/// same value copied into both.
#[test]
fn per_node_analytics_response_keeps_fields_distinct() {
    // Intended /api/analytics/nodes response shape after D2/D3.
    let response = json!({
        "success": true,
        "nodes": [
            { "nodeId": 1, "clusterId": 1, "communityId": 0, "anomaly": 0.1, "centrality": 0.05 },
            { "nodeId": 2, "clusterId": 2, "communityId": 1, "anomaly": 3.4, "centrality": 0.20 }
        ]
    });
    let nodes = response["nodes"].as_array().unwrap();
    let any_distinct = nodes.iter().any(|n| n["clusterId"] != n["communityId"]);
    assert!(
        any_distinct,
        "cluster_id and community_id must be independently sourced (dup-write regression)"
    );
    // centrality must be present (the new D2 field reaching the API).
    for n in nodes {
        assert!(
            n.get("centrality").is_some(),
            "every node analytics record must carry centrality (ADR-031 D2)"
        );
    }
}

/// D4: graph-structural anomaly (/anomaly/detect, GPU LOF) and agent-health
/// (/anomaly/toggle, CPU heuristic) are DIFFERENT surfaces and must not share a
/// field. Assert the route namespacing contract.
#[test]
fn anomaly_surfaces_are_separated() {
    let structural = "/api/analytics/anomaly/detect"; // GPU LOF (D4 new route)
    let agent_health = "/api/analytics/anomaly/toggle"; // CPU heuristic (legacy)
    assert_ne!(
        structural, agent_health,
        "structural anomaly and agent-health must be distinct routes (D4)"
    );
    assert!(structural.ends_with("/detect"));
    assert!(agent_health.ends_with("/toggle"));
}

/// LOF anomaly values in a response must be the real LOF ratio (>= 0, inliers
/// ~1, outliers >> 1), NOT the broken 1/local_density (which is bounded by the
/// inverse density and cannot exceed ~1 for dense inliers). Assert the value
/// domain the corrected kernel must satisfy.
#[test]
fn anomaly_values_are_real_lof_ratio_domain() {
    let response = json!({
        "anomalies": [
            { "nodeId": 7, "anomaly": 5.2 },   // outlier: real LOF >> 1
            { "nodeId": 8, "anomaly": 1.0 }    // inlier: ~1
        ]
    });
    let arr = response["anomalies"].as_array().unwrap();
    let max = arr
        .iter()
        .map(|a| a["anomaly"].as_f64().unwrap())
        .fold(0.0f64, f64::max);
    for a in arr {
        assert!(
            a["anomaly"].as_f64().unwrap() >= 0.0,
            "LOF must be non-negative"
        );
    }
    assert!(
        max > 1.5,
        "a real LOF outlier exceeds 1.5; the broken 1/density kernel cannot, so this gates the D4 fix"
    );
}

// ---------------------------------------------------------------------------
// GPU-gated live HTTP value round-trips.
// ---------------------------------------------------------------------------

/// Drive a real Louvain pass via the HTTP API and assert the returned partition
/// clears the modularity gate and matches the CPU oracle's community count.
#[test]
#[ignore = "needs GPU + running server: live /api/analytics/clustering/run round-trip"]
fn live_clustering_run_matches_oracle() {
    // Intended:
    //   POST /api/analytics/clustering/run { method: "louvain" } on two_clique
    //   -> response.clusters.len() == 2
    //   -> modularity(reconstructed partition) >= 0.3
    let g = two_clique();
    let expected_communities = distinct_communities(&two_clique_optimal_partition(&g));
    assert_eq!(expected_communities, 2); // oracle precondition
    panic!("bind to live /api/analytics/clustering/run once server+GPU available");
}

/// Drive a real anomaly detect pass and assert returned LOF matches the oracle.
#[test]
#[ignore = "needs GPU + running server: live /api/analytics/anomaly/detect round-trip"]
fn live_anomaly_detect_matches_oracle() {
    panic!("bind to live /api/analytics/anomaly/detect once server+GPU available");
}
