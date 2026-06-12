use visionclaw_xr_presence::wire::{decode, encode, OPCODE_AVATAR_POSE};
use visionclaw_xr_presence::{AvatarId, Did, PoseFrame, RoomId, Transform};

fn room() -> RoomId {
    RoomId::parse("urn:visionclaw:room:sha256-12-deadbeefcafe").unwrap()
}

fn avatar() -> AvatarId {
    let did = Did::parse(format!("did:nostr:{}", "f".repeat(64))).unwrap();
    AvatarId::from_did(&did)
}

#[test]
fn encode_starts_with_0x43_opcode() {
    let frame = PoseFrame {
        timestamp_us: 1,
        head: Transform::identity(),
        left_hand: None,
        right_hand: None,
    };
    let bytes = encode(&frame, &room(), &avatar()).unwrap();
    assert_eq!(bytes[0], OPCODE_AVATAR_POSE);
}

#[test]
fn round_trip_preserves_timestamp_and_pose() {
    let frame = PoseFrame {
        timestamp_us: 1_700_000_000_000,
        head: Transform {
            position: [1.5, 2.5, 3.5],
            // 90° yaw: [0, sin(45°), 0, cos(45°)].
            rotation: [
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
            ],
        },
        left_hand: Some(Transform {
            position: [-0.3, 1.2, -0.4],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }),
        right_hand: Some(Transform {
            position: [0.3, 1.2, -0.4],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }),
    };
    let bytes = encode(&frame, &room(), &avatar()).unwrap();
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded.frame, frame);
    assert_eq!(decoded.avatar_id, avatar().as_str());
    assert_eq!(decoded.room_hash, room().wire_hash());
}

#[test]
fn round_trip_head_only() {
    let frame = PoseFrame {
        timestamp_us: 42,
        head: Transform {
            position: [0.0, 1.6, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        },
        left_hand: None,
        right_hand: None,
    };
    let bytes = encode(&frame, &room(), &avatar()).unwrap();
    let decoded = decode(&bytes).unwrap();
    assert!(decoded.frame.left_hand.is_none());
    assert!(decoded.frame.right_hand.is_none());
}
