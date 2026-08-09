//! ADR-031 D2/D7 — Golden wire snapshot + offset round-trip for the 52 B
//! NodeAnalytics V3 record. Fails CI on any stride/offset regression.
//!
//! The record under contract (per-node, little-endian):
//!   id@0(u32) position@4(12B) velocity@16(12B) sssp_distance@28(f32)
//!   sssp_parent@32(i32) cluster_id@36(u32) anomaly@40(f32) community@44(u32)
//!   centrality@48(f32)  -> total stride 52 B.
//!
//! The host fixture encoder (`fx::encode_record_52`) is asserted to be
//! byte-identical to the production encoder once the lead lands the 52 B layout
//! (see the production-binding test below, currently #[ignore]).

#[path = "analytics_fixtures.rs"]
mod fx;

use fx::*;

// ---------------------------------------------------------------------------
// Offset assertions — these are the contract. Any reorder/resize breaks them.
// ---------------------------------------------------------------------------

#[test]
fn wire_offsets_match_contract() {
    assert_eq!(OFF_ID, 0, "id @0");
    assert_eq!(OFF_POSITION, 4, "position @4");
    assert_eq!(OFF_VELOCITY, 16, "velocity @16");
    assert_eq!(OFF_SSSP_DISTANCE, 28, "sssp_distance @28");
    assert_eq!(OFF_SSSP_PARENT, 32, "sssp_parent @32");
    assert_eq!(OFF_CLUSTER_ID, 36, "cluster_id @36");
    assert_eq!(OFF_ANOMALY, 40, "anomaly @40");
    assert_eq!(OFF_COMMUNITY, 44, "community @44");
    assert_eq!(OFF_CENTRALITY, 48, "centrality @48 (the new D2 slot)");
}

#[test]
fn stride_is_52_not_48() {
    assert_eq!(WIRE_V3_ITEM_SIZE_52, 52, "post-D2 per-node stride is 52 B");
    assert_ne!(
        WIRE_V3_ITEM_SIZE_52, WIRE_V3_ITEM_SIZE_48,
        "the centrality append must actually grow the stride (48 -> 52)"
    );
    // centrality occupies exactly the appended 4 bytes [48, 52).
    assert_eq!(OFF_CENTRALITY + 4, WIRE_V3_ITEM_SIZE_52);
}

// ---------------------------------------------------------------------------
// Golden snapshot — a fixed fixture node encoded to bytes. Any drift in field
// order, endianness, or stride changes these bytes and fails CI.
// ---------------------------------------------------------------------------

/// Fixed, human-auditable fixture node.
fn golden_node() -> (WirePosFx, NodeAnalyticsFx) {
    let pos = WirePosFx {
        id: 0x0000_002A, // 42
        pos: [1.0, 2.0, 3.0],
        vel: [0.5, -0.5, 0.25],
        sssp_distance: 7.5,
        sssp_parent: 13,
    };
    let an = NodeAnalyticsFx {
        cluster_id: 3,     // 1-based
        community_id: 9,   // DISTINCT from cluster_id (dup-write regression guard)
        anomaly: 1.75,     // real LOF ratio
        centrality: 0.125, // normalised PageRank
    };
    (pos, an)
}

#[test]
fn golden_wire_snapshot_52_bytes() {
    let (pos, an) = golden_node();
    let bytes = encode_record_52(&pos, &an);
    assert_eq!(bytes.len(), 52, "record must be exactly 52 bytes");

    // GOLDEN BYTES (little-endian). Computed from golden_node(); pinning these
    // makes any offset/order/endianness regression fail CI.
    let expected: [u8; 52] = [
        // id@0  = 42
        0x2A, 0x00, 0x00, 0x00, // pos@4 = 1.0, 2.0, 3.0
        0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x40,
        // vel@16 = 0.5, -0.5, 0.25
        0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00, 0xBF, 0x00, 0x00, 0x80, 0x3E,
        // sssp_distance@28 = 7.5
        0x00, 0x00, 0xF0, 0x40, // sssp_parent@32 = 13
        0x0D, 0x00, 0x00, 0x00, // cluster_id@36 = 3
        0x03, 0x00, 0x00, 0x00, // anomaly@40 = 1.75
        0x00, 0x00, 0xE0, 0x3F, // community@44 = 9
        0x09, 0x00, 0x00, 0x00, // centrality@48 = 0.125
        0x00, 0x00, 0x00, 0x3E,
    ];
    assert_eq!(
        bytes.as_slice(),
        &expected[..],
        "golden 52 B wire snapshot mismatch — a wire layout regression occurred"
    );
}

#[test]
fn field_bytes_land_at_declared_offsets() {
    let (pos, an) = golden_node();
    let b = encode_record_52(&pos, &an);

    // Spot-check that each field decodes from its declared offset.
    let r_u32 = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let r_i32 = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let r_f32 = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());

    assert_eq!(r_u32(OFF_ID), 42);
    assert_eq!(r_f32(OFF_POSITION), 1.0);
    assert_eq!(r_f32(OFF_VELOCITY + 4), -0.5);
    assert_eq!(r_f32(OFF_SSSP_DISTANCE), 7.5);
    assert_eq!(r_i32(OFF_SSSP_PARENT), 13);
    assert_eq!(r_u32(OFF_CLUSTER_ID), 3);
    assert_eq!(r_f32(OFF_ANOMALY), 1.75);
    assert_eq!(r_u32(OFF_COMMUNITY), 9);
    assert_eq!(
        r_f32(OFF_CENTRALITY),
        0.125,
        "centrality MUST decode from byte 48"
    );
}

// ---------------------------------------------------------------------------
// Round-trip — encode -> decode -> equal, including the multi-node stride.
// ---------------------------------------------------------------------------

#[test]
fn round_trip_single_record() {
    let (pos, an) = golden_node();
    let bytes = encode_record_52(&pos, &an);
    let (pos2, an2) = decode_record_52(&bytes);
    assert_eq!(pos, pos2, "position round-trip");
    assert_eq!(an, an2, "analytics round-trip");
}

#[test]
fn round_trip_multi_node_stride() {
    // Three distinct nodes; assert each decodes from its 52 B slice and that
    // the centrality field never bleeds across the stride boundary.
    let mk = |i: u32| {
        (
            WirePosFx {
                id: i,
                pos: [i as f32, i as f32 + 0.5, i as f32 + 1.0],
                vel: [0.0, 0.0, 0.0],
                sssp_distance: i as f32 * 2.0,
                sssp_parent: i as i32 - 1,
            },
            NodeAnalyticsFx {
                cluster_id: i + 1,
                community_id: i + 100,
                anomaly: i as f32 / 10.0,
                centrality: i as f32 / 1000.0,
            },
        )
    };
    let mut buf = Vec::new();
    let originals: Vec<_> = (0..3u32).map(mk).collect();
    for (p, a) in &originals {
        buf.extend_from_slice(&encode_record_52(p, a));
    }
    assert_eq!(buf.len(), 3 * 52);

    for (i, (p, a)) in originals.iter().enumerate() {
        let slice = &buf[i * 52..(i + 1) * 52];
        let (p2, a2) = decode_record_52(slice);
        assert_eq!(*p, p2, "node {i} position round-trip across stride");
        assert_eq!(*a, a2, "node {i} analytics round-trip across stride");
        assert_eq!(
            a2.centrality,
            i as f32 / 1000.0,
            "node {i} centrality must not bleed across the 52 B stride boundary"
        );
    }
}

// ---------------------------------------------------------------------------
// Production encoder binding — asserts the SERVER encoder produces byte-
// identical output to the fixture encoder. GATED until the lead lands the 52 B
// layout (production encoder is currently 48 B / 3-tuple analytics).
// ---------------------------------------------------------------------------

/// ADR-031 D2/D7: the production broadcast encoder must emit byte-identical
/// output to the host fixture encoder for the golden node — proving the live
/// path (the same fn the WebSocket broadcast uses) carries centrality@48 and
/// the full 52 B analytics record, not just the fixture. Closes the gap left by
/// the previously-gated placeholder now that the 52 B encoder has landed.
#[test]
fn production_encoder_matches_golden() {
    use std::collections::HashMap;
    use visionclaw_server::utils::binary_protocol::{
        encode_node_data_with_live_analytics, NodeAnalytics,
    };
    use visionclaw_server::utils::socket_flow_messages::BinaryNodeData;

    let (pos, an) = golden_node();

    let nodes: Vec<(u32, BinaryNodeData)> = vec![(
        pos.id,
        BinaryNodeData {
            node_id: pos.id,
            x: pos.pos[0],
            y: pos.pos[1],
            z: pos.pos[2],
            vx: pos.vel[0],
            vy: pos.vel[1],
            vz: pos.vel[2],
        },
    )];

    let mut analytics: HashMap<u32, NodeAnalytics> = HashMap::new();
    analytics.insert(
        pos.id,
        NodeAnalytics {
            cluster_id: an.cluster_id,
            community_id: an.community_id,
            anomaly: an.anomaly,
            centrality: an.centrality,
        },
    );

    let mut sssp: HashMap<u32, (f32, i32)> = HashMap::new();
    sssp.insert(pos.id, (pos.sssp_distance, pos.sssp_parent));

    let out = encode_node_data_with_live_analytics(&nodes, Some(&analytics), Some(&sssp));

    // Frame = 1-byte protocol-version header + one 52 B record.
    assert_eq!(out.len(), 1 + 52, "header byte + single 52 B record");

    // Must be byte-identical to the host fixture encoder for the same node.
    let fixture = encode_record_52(&pos, &an);
    assert_eq!(
        &out[1..],
        fixture.as_slice(),
        "production encoder must match the golden fixture record (centrality@48 included)"
    );
}
