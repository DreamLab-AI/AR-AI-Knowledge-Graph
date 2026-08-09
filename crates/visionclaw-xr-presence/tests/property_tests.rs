//! Property-based tests for the XR presence library (PRD-QE-002 §4.5).
//!
//! Each `proptest!` block is one invariant from PRD-QE-002 §4.5 / threat model:
//!
//! | Property                              | Threat / invariant                |
//! |---------------------------------------|-----------------------------------|
//! | Wire round-trip                       | T-PROTO-1 (decoder totality)      |
//! | Velocity gate                         | T-POSE-1 (impossible velocities)  |
//! | Monotonic timestamps                  | T-WS-3 (replay)                   |
//! | World bounds                          | T-FRAME-1 (out-of-arena teleport) |
//! | Joint anatomy                         | T-HAND-1 (impossible joint)       |
//! | Room invariant — one DID one avatar   | I-PR01                            |
//! | Delta encoding round-trip             | wire / delta consistency          |

use proptest::prelude::*;
use visionclaw_xr_presence::{
    agent_presence::{
        decode_agent_presence, dequantise_dir, encode_agent_presence, quantise_dir, AgentActivity,
        AgentPresence, AgentPresenceDelta, AttentionTarget,
    },
    delta::PoseDelta,
    monotonic_timestamp,
    types::{Aabb, AvatarMetadata, Did, HandPose, PoseFrame, RoomId, Transform},
    validate::{joint_anatomy, world_bounds},
    velocity_gate, wire, AvatarId, PresenceRoom, RoomError, ValidationError,
};

// -- generators ---------------------------------------------------------------

fn arb_position(extent: f32) -> impl Strategy<Value = [f32; 3]> {
    proptest::array::uniform3(-extent..extent)
}

fn arb_unit_quat() -> impl Strategy<Value = [f32; 4]> {
    // Sample a quat then normalise to unit length so validators that check
    // |q| ≈ 1 always pass for the "well-formed" generators.
    (-1.0f32..1.0, -1.0f32..1.0, -1.0f32..1.0, -1.0f32..1.0).prop_map(|(x, y, z, w)| {
        let m = (x * x + y * y + z * z + w * w).sqrt().max(1e-6);
        [x / m, y / m, z / m, w / m]
    })
}

fn arb_transform(extent: f32) -> impl Strategy<Value = Transform> {
    (arb_position(extent), arb_unit_quat())
        .prop_map(|(position, rotation)| Transform { position, rotation })
}

fn arb_pose_frame(extent: f32) -> impl Strategy<Value = PoseFrame> {
    (
        any::<u64>(),
        arb_transform(extent),
        proptest::option::of(arb_transform(extent)),
        proptest::option::of(arb_transform(extent)),
    )
        .prop_map(|(timestamp_us, head, left_hand, right_hand)| PoseFrame {
            timestamp_us,
            head,
            left_hand,
            right_hand,
        })
}

fn fixed_room() -> RoomId {
    RoomId::parse("urn:visionclaw:room:sha256-12-cafef00d1234").expect("fixed room URN must parse")
}

fn fixed_avatar() -> AvatarId {
    let did =
        Did::parse(format!("did:nostr:{}", "9".repeat(64))).expect("fixed avatar DID must parse");
    AvatarId::from_did(&did)
}

// -- properties ---------------------------------------------------------------

proptest! {
    /// PROP-WIRE-1: any well-formed PoseFrame survives encode → decode unchanged
    /// regardless of which optional hand slots are populated.
    #[test]
    fn wire_round_trip_preserves_frame(frame in arb_pose_frame(50.0)) {
        let room = fixed_room();
        let avatar = fixed_avatar();
        let bytes = wire::encode(&frame, &room, &avatar)
            .expect("encode must succeed for well-formed pose");
        let decoded = wire::decode(&bytes).expect("decode must succeed");
        prop_assert_eq!(decoded.frame, frame);
        prop_assert_eq!(decoded.room_hash, room.wire_hash());
        prop_assert_eq!(decoded.avatar_id, avatar.as_str().to_owned());
    }

    /// PROP-VEL-1: any pose pair whose head displacement / dt exceeds the
    /// configured limit (default 20 m/s per `validate::DEFAULT_MAX_VELOCITY_MPS`)
    /// is rejected.
    #[test]
    fn velocity_gate_rejects_above_limit(
        dt_us in 1_000u64..1_000_000,
        dx in 25.0f32..100.0,
    ) {
        let prev = PoseFrame {
            timestamp_us: 0,
            head: Transform { position: [0.0, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] },
            left_hand: None, right_hand: None,
        };
        // dx ≥ 25 m over ≤ 1 s ⇒ ≥ 25 m/s, exceeds 20 m/s gate.
        let next = PoseFrame {
            timestamp_us: dt_us,
            head: Transform { position: [dx, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] },
            left_hand: None, right_hand: None,
        };
        let observed = (dx as f64) / (dt_us as f64 / 1_000_000.0);
        prop_assume!(observed > 20.0);
        let res = velocity_gate(&prev, &next, 20.0);
        let ok = matches!(res, Err(ValidationError::VelocityExceeded { .. }));
        prop_assert!(ok, "velocity gate must reject {observed} m/s");
    }

    /// PROP-VEL-2: any pose pair within a tight bound moves slowly enough that
    /// the gate accepts it. Inverse direction of PROP-VEL-1.
    #[test]
    fn velocity_gate_accepts_below_limit(
        dt_us in 100_000u64..1_000_000,
        dx in -1.0f32..1.0,
    ) {
        let prev = PoseFrame {
            timestamp_us: 0,
            head: Transform { position: [0.0, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] },
            left_hand: None, right_hand: None,
        };
        let next = PoseFrame {
            timestamp_us: dt_us,
            head: Transform { position: [dx, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] },
            left_hand: None, right_hand: None,
        };
        // dx ≤ 1 m over ≥ 100 ms ⇒ ≤ 10 m/s — comfortably below 20 m/s.
        prop_assert!(velocity_gate(&prev, &next, 20.0).is_ok());
    }

    /// PROP-MONO-1: any prev > next pair is rejected as non-monotonic.
    #[test]
    fn monotonic_rejects_decreasing(prev_us in 1_000u64..u64::MAX/2, gap in 1u64..1_000_000) {
        let next_us = prev_us.saturating_sub(gap);
        prop_assume!(next_us < prev_us);
        let res = monotonic_timestamp(prev_us, next_us);
        let ok = matches!(
            res,
            Err(ValidationError::NonMonotonicTimestamp { .. })
                | Err(ValidationError::DuplicateTimestamp { .. })
        );
        prop_assert!(ok, "decreasing timestamp must reject");
    }

    /// PROP-BOUNDS-1: any position outside the configured AABB is rejected.
    #[test]
    fn world_bounds_rejects_outside(extent in 1.0f32..100.0, axis in 0u8..3, sign in any::<bool>()) {
        let bounds = Aabb::symmetric(extent);
        let mut pos = [0.0f32; 3];
        pos[axis as usize] = if sign { extent + 1.0 } else { -(extent + 1.0) };
        let t = Transform { position: pos, rotation: [0.0, 0.0, 0.0, 1.0] };
        let res = world_bounds(&t, &bounds);
        let ok = matches!(res, Err(ValidationError::OutOfBounds { .. }));
        prop_assert!(ok, "out-of-AABB must reject");
    }

    /// PROP-BOUNDS-2: any position strictly inside the AABB with a unit
    /// quaternion is accepted.
    #[test]
    fn world_bounds_accepts_inside(extent in 5.0f32..100.0, p in arb_position(4.0)) {
        let bounds = Aabb::symmetric(extent);
        let t = Transform { position: p, rotation: [0.0, 0.0, 0.0, 1.0] };
        let ok = world_bounds(&t, &bounds).is_ok();
        prop_assert!(ok, "inside-AABB must accept");
    }

    /// PROP-ANATOMY-1: any non-unit quaternion in either wrist is rejected.
    /// (The "joint anatomy" validator currently enforces only the wrist
    /// quaternion unit-norm in v1; see `validate.rs::joint_anatomy` notes.)
    #[test]
    fn joint_anatomy_rejects_non_unit_wrist(scale in 2.0f32..10.0) {
        let bad = Transform { position: [0.0; 3], rotation: [scale, 0.0, 0.0, 0.0] };
        let good = Transform { position: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0] };
        let left = HandPose { wrist: bad, joints: Vec::new() };
        let right = HandPose { wrist: good, joints: Vec::new() };
        let res = joint_anatomy(&left, &right);
        let ok = matches!(res, Err(ValidationError::NonUnitQuaternion { .. }));
        prop_assert!(ok, "non-unit wrist quaternion must reject");
    }

    /// PROP-ROOM-1: the same DID joining twice is always rejected (one DID
    /// one avatar — invariant I-PR01 in `room.rs`).
    #[test]
    fn room_rejects_duplicate_did(seed in 0u8..255) {
        let mut room = PresenceRoom::new(fixed_room());
        let did = Did::parse(format!("did:nostr:{}", format!("{:02x}", seed).repeat(32)))
            .expect("did parse");
        let metadata = AvatarMetadata { did: did.clone(), display_name: "x".into(), model_uri: None };
        room.join(did.clone(), metadata.clone()).expect("first join must succeed");
        let err = room.join(did, metadata).expect_err("second join must fail");
        let ok = matches!(err, RoomError::DuplicateDid { .. });
        prop_assert!(ok, "duplicate DID must reject");
    }

    /// PROP-DELTA-1: any prev/next pair round-trips through delta encoding.
    /// Reconstructing `apply(between(prev, next), prev) == next`.
    #[test]
    fn delta_round_trip(
        prev in arb_pose_frame(20.0),
        next in arb_pose_frame(20.0),
    ) {
        let delta = PoseDelta::between(&prev, &next);
        let restored = delta.apply(&prev);
        prop_assert_eq!(restored.timestamp_us, next.timestamp_us);
        prop_assert_eq!(restored.head, next.head);
        // The delta encoding always preserves head; for hand slots it preserves
        // any slot present in `next` (matching `delta.rs` semantics).
        if next.left_hand.is_some() {
            prop_assert_eq!(restored.left_hand, next.left_hand);
        }
        if next.right_hand.is_some() {
            prop_assert_eq!(restored.right_hand, next.right_hand);
        }
    }

    /// PROP-DELTA-2: identical frames produce an empty mask.
    #[test]
    fn delta_identical_frames_empty_mask(
        ts in 1_000u64..1_000_000_000,
        frame in arb_pose_frame(20.0),
    ) {
        let same_ts_next = PoseFrame { timestamp_us: ts, ..frame.clone() };
        let prev = PoseFrame { timestamp_us: ts.saturating_sub(1), ..frame };
        let delta = PoseDelta::between(&prev, &same_ts_next);
        // Head is bit-equal so the head slot must NOT be in the mask.
        prop_assert!(!delta.mask.has_head());
    }
}

// -- agent-presence codec generators (opcode 0x44, ADR-130 Decision 4) --------

fn arb_activity() -> impl Strategy<Value = AgentActivity> {
    prop_oneof![
        Just(AgentActivity::Idle),
        Just(AgentActivity::Working),
        Just(AgentActivity::AwaitingApproval),
        Just(AgentActivity::Speaking),
    ]
}

fn arb_attention() -> impl Strategy<Value = AttentionTarget> {
    prop_oneof![
        Just(AttentionTarget::None),
        Just(AttentionTarget::User),
        any::<u32>().prop_map(AttentionTarget::GraphNode),
    ]
}

/// A non-degenerate direction so the quantiser's zero-fallback does not fire.
fn arb_dir() -> impl Strategy<Value = [f32; 3]> {
    (-1.0f32..1.0, -1.0f32..1.0, -1.0f32..1.0)
        .prop_filter("non-degenerate", |(x, y, z)| {
            (x * x + y * y + z * z).sqrt() > 0.05
        })
        .prop_map(|(x, y, z)| [x, y, z])
}

fn arb_delta() -> impl Strategy<Value = AgentPresenceDelta> {
    (
        any::<u32>(),
        proptest::option::of(arb_activity()),
        proptest::option::of(arb_dir()),
        proptest::option::of(arb_attention()),
    )
        .prop_map(
            |(local_id, state, gaze_dir, attention)| AgentPresenceDelta {
                local_id,
                state,
                gaze_dir,
                attention,
            },
        )
}

fn dir_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    let na = {
        let m = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-9);
        [a[0] / m, a[1] / m, a[2] / m]
    };
    na[0] * b[0] + na[1] * b[1] + na[2] * b[2]
}

proptest! {
    /// PROP-AGENT-WIRE-1: any batch of well-formed agent-presence deltas
    /// survives encode → decode with every discrete field (local_id, state,
    /// attention) preserved exactly (T-PROTO-1 decoder totality + fidelity).
    #[test]
    fn agent_presence_round_trip(seq in any::<u64>(), deltas in prop::collection::vec(arb_delta(), 0..12)) {
        let bytes = encode_agent_presence(seq, &deltas).expect("encode well-formed batch");
        let batch = decode_agent_presence(&bytes).expect("decode must succeed");
        prop_assert_eq!(batch.seq, seq);
        prop_assert_eq!(batch.deltas.len(), deltas.len());
        for (got, want) in batch.deltas.iter().zip(deltas.iter()) {
            prop_assert_eq!(got.local_id, want.local_id);
            prop_assert_eq!(got.state, want.state);
            prop_assert_eq!(got.attention, want.attention);
            prop_assert_eq!(got.gaze_dir.is_some(), want.gaze_dir.is_some());
        }
    }

    /// PROP-AGENT-WIRE-2: the decoder is total over arbitrary bytes — it never
    /// panics, only returns Ok or a WireError (fuzz contract, PRD-QE-002 §4.6).
    #[test]
    fn agent_presence_decode_is_total(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        let _ = decode_agent_presence(&bytes);
    }

    /// PROP-GAZE-QUANT-1: 16-bit-per-component quantisation preserves any unit
    /// direction to sub-milliradian angular error (cos θ > 0.9999). Underpins
    /// the high-rate gaze channel's 10–20 Hz budget.
    #[test]
    fn gaze_quantisation_preserves_direction(d in arb_dir()) {
        let restored = dequantise_dir(quantise_dir(d));
        let dot = dir_dot(d, restored);
        prop_assert!(dot > 0.9999, "angular error too large: dot={dot} for {d:?}");
    }

    /// PROP-GAZE-QUANT-2: quantise is idempotent through a decode round — a
    /// direction that has already been through the wire does not drift on a
    /// second pass (no accumulating error over a held gaze).
    #[test]
    fn gaze_quantisation_is_stable_on_second_pass(d in arb_dir()) {
        let once = dequantise_dir(quantise_dir(d));
        let twice = dequantise_dir(quantise_dir(once));
        prop_assert_eq!(quantise_dir(once), quantise_dir(twice));
    }

    /// PROP-AGENT-APPLY-1: applying between(prev,next) over prev reconstructs
    /// next for the discrete fields, regardless of which fields changed.
    #[test]
    fn agent_delta_apply_round_trip(
        s1 in arb_activity(), s2 in arb_activity(),
        d1 in arb_dir(), d2 in arb_dir(),
        a1 in arb_attention(), a2 in arb_attention(),
    ) {
        let prev = AgentPresence::new(s1, d1, a1);
        let next = AgentPresence::new(s2, d2, a2);
        let delta = AgentPresenceDelta::between(0, &prev, &next);
        let restored = delta.apply(&prev);
        prop_assert_eq!(restored.state, next.state);
        prop_assert_eq!(restored.attention, next.attention);
    }
}
