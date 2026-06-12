//! Edge-case integration tests for the binary protocol public API.
//!
//! Wire format is Protocol V3 (`0x03` version byte + N x 52-byte node records).
//! The decoder reads id/pos/vel from the first 28 bytes of each record and
//! ignores the 24-byte analytics tail (sssp/cluster/anomaly/community/centrality).

use bytes::Bytes;
use visionclaw_xr_gdext::binary_protocol::{
    decode_position_frame, ingest_frame, NODE_RECORD_BYTES, PROTOCOL_V3,
};

/// Helper: build a valid V3 frame from a slice of (node_id, position, velocity).
/// Each record is padded to the full 52-byte layout with a zeroed analytics tail.
fn build_frame(records: &[(u32, [f32; 3], [f32; 3])]) -> Vec<u8> {
    let mut out = vec![PROTOCOL_V3];
    for (id, pos, vel) in records {
        out.extend_from_slice(&id.to_le_bytes());
        for v in pos {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in vel {
            out.extend_from_slice(&v.to_le_bytes());
        }
        // Analytics tail (24 bytes): sssp_dist, sssp_parent, cluster_id,
        // anomaly, community, centrality — ignored by the decoder.
        out.extend_from_slice(&f32::INFINITY.to_le_bytes());
        out.extend_from_slice(&(-1i32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0f32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0f32.to_le_bytes());
    }
    out
}

#[test]
fn empty_candidates_returns_none() {
    // A version-only frame decodes to an empty vec, which means any downstream
    // "find first matching node" logic gets nothing.
    let frame = vec![PROTOCOL_V3];
    let decoded = decode_position_frame(&frame).unwrap();
    assert!(decoded.is_empty());
    assert_eq!(decoded.first(), None);
}

#[test]
fn large_frame_100_nodes() {
    let records: Vec<(u32, [f32; 3], [f32; 3])> = (0u32..100)
        .map(|i| {
            let f = i as f32;
            (i, [f, f * 2.0, f * 3.0], [f * 0.01, f * 0.02, f * 0.03])
        })
        .collect();
    let frame = build_frame(&records);

    // Verify frame size: 1-byte version header + 100 * 52-byte records.
    assert_eq!(frame.len(), 1 + 100 * NODE_RECORD_BYTES);

    let decoded = decode_position_frame(&frame).unwrap();
    assert_eq!(decoded.len(), 100);

    for (i, update) in decoded.iter().enumerate() {
        let f = i as f32;
        assert_eq!(update.node_id, i as u32);
        assert_eq!(update.position, [f, f * 2.0, f * 3.0]);
        assert_eq!(update.velocity, [f * 0.01, f * 0.02, f * 0.03]);
    }
}

#[test]
fn header_only_zero_nodes() {
    let frame = vec![PROTOCOL_V3];
    let decoded = decode_position_frame(&frame).unwrap();
    assert_eq!(decoded, vec![]);
}

#[test]
fn ingest_frame_100_nodes_fires_all_callbacks() {
    let records: Vec<(u32, [f32; 3], [f32; 3])> =
        (0u32..100).map(|i| (i, [i as f32; 3], [0.0; 3])).collect();
    let frame = build_frame(&records);
    let mut count = 0usize;
    let mut last_id: Option<u32> = None;
    ingest_frame(Bytes::from(frame), |u| {
        count += 1;
        last_id = Some(u.node_id);
    });
    assert_eq!(count, 100);
    assert_eq!(last_id, Some(99));
}
