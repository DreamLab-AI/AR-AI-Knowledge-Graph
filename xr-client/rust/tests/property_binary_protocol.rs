//! Property tests for the Protocol V3 graph position frame codec.
//!
//! Layout (server `src/utils/binary_protocol.rs`, ADR-031 Analytics Extension):
//! ```text
//! [u8 version = 0x03]
//! [{ u32_le node_id, f32_le[3] position, f32_le[3] velocity,
//!    f32_le sssp_dist, i32_le sssp_parent, u32_le cluster_id,
//!    f32_le anomaly, u32_le community, f32_le centrality }; N]
//! ```
//! Each node record is 52 bytes. There is no explicit count field; the decoder
//! infers N from `(payload.len() / 52)` and reads id/pos/vel from the first 28
//! bytes, ignoring the analytics tail.

use proptest::prelude::*;
use visionclaw_xr_gdext::binary_protocol::{decode_position_frame, NODE_ID_MASK, PROTOCOL_V3};

/// Build a valid V3 frame from a slice of (node_id, position, velocity),
/// padding each record to the full 52-byte layout with a zeroed analytics tail.
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
        out.extend_from_slice(&f32::INFINITY.to_le_bytes()); // sssp_dist
        out.extend_from_slice(&(-1i32).to_le_bytes()); // sssp_parent
        out.extend_from_slice(&0u32.to_le_bytes()); // cluster_id
        out.extend_from_slice(&0f32.to_le_bytes()); // anomaly
        out.extend_from_slice(&0u32.to_le_bytes()); // community
        out.extend_from_slice(&0f32.to_le_bytes()); // centrality
    }
    out
}

proptest! {
    /// PROP-BIN-1: any (node_id within the 26-bit mask, [f32;3], [f32;3]) tuple
    /// round-trips exactly through build_frame + decode_position_frame. The id is
    /// masked because the high 6 bits encode node-type flags, not identity.
    #[test]
    fn single_record_round_trip(
        raw_id in any::<u32>(),
        pos in proptest::array::uniform3(any::<f32>()),
        vel in proptest::array::uniform3(any::<f32>()),
    ) {
        let node_id = raw_id & NODE_ID_MASK;
        let frame = build_frame(&[(node_id, pos, vel)]);
        let decoded = decode_position_frame(&frame).unwrap();
        prop_assert_eq!(decoded.len(), 1);
        prop_assert_eq!(decoded[0].node_id, node_id);
        // Bitwise comparison handles NaN/special floats: the raw bytes must
        // round-trip identically.
        for i in 0..3 {
            prop_assert_eq!(
                decoded[0].position[i].to_bits(),
                pos[i].to_bits(),
                "position[{}] mismatch", i
            );
            prop_assert_eq!(
                decoded[0].velocity[i].to_bits(),
                vel[i].to_bits(),
                "velocity[{}] mismatch", i
            );
        }
    }

    /// PROP-BIN-2: a frame with N records (0..50) always decodes to exactly N
    /// NodeUpdate entries.
    #[test]
    fn frame_count_preserved(n in 0usize..50) {
        let records: Vec<(u32, [f32; 3], [f32; 3])> = (0..n)
            .map(|i| (i as u32, [i as f32; 3], [0.0f32; 3]))
            .collect();
        let frame = build_frame(&records);
        let decoded = decode_position_frame(&frame).unwrap();
        prop_assert_eq!(decoded.len(), n);
    }

    /// PROP-BIN-3: multi-record round-trip preserves ordering and all fields.
    #[test]
    fn multi_record_round_trip(
        n in 1usize..20,
        seed in any::<u32>(),
    ) {
        let records: Vec<(u32, [f32; 3], [f32; 3])> = (0..n)
            .map(|i| {
                let id = seed.wrapping_add(i as u32) & NODE_ID_MASK;
                let f = id as f32;
                (id, [f, f + 1.0, f + 2.0], [f * 0.1, f * 0.2, f * 0.3])
            })
            .collect();
        let frame = build_frame(&records);
        let decoded = decode_position_frame(&frame).unwrap();
        prop_assert_eq!(decoded.len(), n);
        for (i, (id, pos, vel)) in records.iter().enumerate() {
            prop_assert_eq!(decoded[i].node_id, *id);
            prop_assert_eq!(decoded[i].position, *pos);
            prop_assert_eq!(decoded[i].velocity, *vel);
        }
    }
}
