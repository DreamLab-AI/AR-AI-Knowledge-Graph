use crate::error::ValidationError;
use crate::types::{Aabb, HandPose, PoseFrame, Transform};

pub const DEFAULT_MAX_VELOCITY_MPS: f32 = 20.0;
pub const DEFAULT_HAND_REACH_M: f32 = 1.2;
pub const DEFAULT_MIN_FRAME_INTERVAL_US: u64 = 8_000;
const QUAT_UNIT_LO: f32 = 0.99;
const QUAT_UNIT_HI: f32 = 1.01;

pub fn velocity_gate(
    prev: &PoseFrame,
    next: &PoseFrame,
    max_mps: f32,
) -> Result<(), ValidationError> {
    // P2-07: NaN positions bypass the comparison (NaN > X is always false).
    // Reject any frame containing NaN or infinite coordinates before computing
    // velocity, since such positions are physically impossible. This covers the
    // head plus whichever hand slots are populated in either frame.
    let mut positions: Vec<&[f32; 3]> = vec![&prev.head.position, &next.head.position];
    for hand in [
        prev.left_hand.as_ref(),
        next.left_hand.as_ref(),
        prev.right_hand.as_ref(),
        next.right_hand.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        positions.push(&hand.position);
    }
    for pos in positions {
        for &v in pos.iter() {
            if v.is_nan() || v.is_infinite() {
                return Err(ValidationError::VelocityExceeded {
                    observed_mps: f32::INFINITY,
                    limit_mps: max_mps,
                });
            }
        }
    }

    if next.timestamp_us <= prev.timestamp_us {
        return Err(ValidationError::NonMonotonicTimestamp {
            prev_us: prev.timestamp_us,
            next_us: next.timestamp_us,
        });
    }
    let dt_us = next.timestamp_us - prev.timestamp_us;
    let dt_s = (dt_us as f64) / 1_000_000.0;

    // Gate the head and every hand slot present in BOTH frames against the same
    // velocity ceiling. A teleporting hand is as much a spoof/abuse signal as a
    // teleporting head, so hand slots are no longer exempt.
    let mut pairs: Vec<(&Transform, &Transform)> = vec![(&prev.head, &next.head)];
    if let (Some(p), Some(n)) = (prev.left_hand.as_ref(), next.left_hand.as_ref()) {
        pairs.push((p, n));
    }
    if let (Some(p), Some(n)) = (prev.right_hand.as_ref(), next.right_hand.as_ref()) {
        pairs.push((p, n));
    }
    for (p, n) in pairs {
        let dx = n.position[0] - p.position[0];
        let dy = n.position[1] - p.position[1];
        let dz = n.position[2] - p.position[2];
        let dist = ((dx * dx + dy * dy + dz * dz) as f64).sqrt();
        let observed_mps = (dist / dt_s) as f32;
        if observed_mps > max_mps {
            return Err(ValidationError::VelocityExceeded {
                observed_mps,
                limit_mps: max_mps,
            });
        }
    }
    Ok(())
}

pub fn world_bounds(transform: &Transform, bounds: &Aabb) -> Result<(), ValidationError> {
    if !bounds.contains(&transform.position) {
        return Err(ValidationError::OutOfBounds {
            x: transform.position[0],
            y: transform.position[1],
            z: transform.position[2],
        });
    }
    let mag = transform.quaternion_magnitude();
    if !(QUAT_UNIT_LO..=QUAT_UNIT_HI).contains(&mag) {
        return Err(ValidationError::NonUnitQuaternion {
            mag,
            lo: QUAT_UNIT_LO,
            hi: QUAT_UNIT_HI,
        });
    }
    Ok(())
}

pub fn monotonic_timestamp(prev_us: u64, next_us: u64) -> Result<(), ValidationError> {
    match next_us.cmp(&prev_us) {
        std::cmp::Ordering::Greater => {
            let dt = next_us - prev_us;
            if dt < DEFAULT_MIN_FRAME_INTERVAL_US {
                return Err(ValidationError::IntervalTooShort {
                    dt_us: dt,
                    min_us: DEFAULT_MIN_FRAME_INTERVAL_US,
                });
            }
            Ok(())
        }
        std::cmp::Ordering::Equal => Err(ValidationError::DuplicateTimestamp { ts_us: next_us }),
        std::cmp::Ordering::Less => {
            Err(ValidationError::NonMonotonicTimestamp { prev_us, next_us })
        }
    }
}

/// Anatomical sanity check on hand poses. v1 enforces only the wrist quaternion
/// unit-norm; full MANO joint flexion gates per `xr-godot-threat-model.md`
/// T-HAND-1 land once the gdext hand tracker exposes joint angles.
pub fn joint_anatomy(left_hand: &HandPose, right_hand: &HandPose) -> Result<(), ValidationError> {
    for hand in [left_hand, right_hand] {
        let mag = hand.wrist.quaternion_magnitude();
        if !(QUAT_UNIT_LO..=QUAT_UNIT_HI).contains(&mag) {
            return Err(ValidationError::NonUnitQuaternion {
                mag,
                lo: QUAT_UNIT_LO,
                hi: QUAT_UNIT_HI,
            });
        }
        // TODO(PRD-008-followup): MANO per-joint flexion ranges once joints are populated.
        for joint in &hand.joints {
            let m = (joint[0] * joint[0]
                + joint[1] * joint[1]
                + joint[2] * joint[2]
                + joint[3] * joint[3])
                .sqrt();
            if !(QUAT_UNIT_LO..=QUAT_UNIT_HI).contains(&m) {
                return Err(ValidationError::NonUnitQuaternion {
                    mag: m,
                    lo: QUAT_UNIT_LO,
                    hi: QUAT_UNIT_HI,
                });
            }
        }
    }
    Ok(())
}

pub fn hand_reach(head: &Transform, hand: &Transform, limit_m: f32) -> Result<(), ValidationError> {
    let dx = hand.position[0] - head.position[0];
    let dy = hand.position[1] - head.position[1];
    let dz = hand.position[2] - head.position[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    if dist > limit_m {
        return Err(ValidationError::HandReachExceeded {
            observed_m: dist,
            limit_m,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pf(ts_us: u64, x: f32) -> PoseFrame {
        PoseFrame {
            timestamp_us: ts_us,
            head: Transform {
                position: [x, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            left_hand: None,
            right_hand: None,
        }
    }

    #[test]
    fn velocity_within_gate_passes() {
        let prev = pf(0, 0.0);
        let next = pf(100_000, 0.5);
        velocity_gate(&prev, &next, 20.0).unwrap();
    }

    #[test]
    fn velocity_above_gate_rejected() {
        let prev = pf(0, 0.0);
        let next = pf(10_000, 5.0);
        let err = velocity_gate(&prev, &next, 20.0).unwrap_err();
        assert!(matches!(err, ValidationError::VelocityExceeded { .. }));
    }

    #[test]
    fn velocity_gate_rejects_nan_position() {
        let prev = pf(0, 0.0);
        let mut next = pf(100_000, 0.5);
        next.head.position[0] = f32::NAN;
        let err = velocity_gate(&prev, &next, 20.0).unwrap_err();
        assert!(matches!(err, ValidationError::VelocityExceeded { .. }));
    }

    #[test]
    fn velocity_gate_rejects_infinity_position() {
        let prev = pf(0, 0.0);
        let mut next = pf(100_000, 0.5);
        next.head.position[1] = f32::INFINITY;
        let err = velocity_gate(&prev, &next, 20.0).unwrap_err();
        assert!(matches!(err, ValidationError::VelocityExceeded { .. }));
    }

    #[test]
    fn out_of_bounds_rejected() {
        let bounds = Aabb::symmetric(50.0);
        let t = Transform {
            position: [100.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        assert!(matches!(
            world_bounds(&t, &bounds),
            Err(ValidationError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn non_unit_quaternion_rejected() {
        let bounds = Aabb::symmetric(50.0);
        let t = Transform {
            position: [0.0, 0.0, 0.0],
            rotation: [2.0, 0.0, 0.0, 0.0],
        };
        assert!(matches!(
            world_bounds(&t, &bounds),
            Err(ValidationError::NonUnitQuaternion { .. })
        ));
    }

    #[test]
    fn monotonic_duplicate_rejected() {
        assert!(matches!(
            monotonic_timestamp(100, 100),
            Err(ValidationError::DuplicateTimestamp { .. })
        ));
    }

    #[test]
    fn monotonic_backwards_rejected() {
        assert!(matches!(
            monotonic_timestamp(200, 100),
            Err(ValidationError::NonMonotonicTimestamp { .. })
        ));
    }

    #[test]
    fn monotonic_interval_too_short_rejected() {
        assert!(matches!(
            monotonic_timestamp(0, 100),
            Err(ValidationError::IntervalTooShort { .. })
        ));
    }

    fn tf(x: f32, y: f32, z: f32) -> Transform {
        Transform {
            position: [x, y, z],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn velocity_gate_rejects_teleporting_hand() {
        // Head barely moves, but the left hand teleports 5m in 10ms → far over
        // the 20 m/s ceiling. Previously hand slots were unchecked and this
        // passed.
        let mut prev = pf(0, 0.0);
        prev.left_hand = Some(tf(0.0, 0.0, 0.0));
        let mut next = pf(10_000, 0.01);
        next.left_hand = Some(tf(5.0, 0.0, 0.0));
        let err = velocity_gate(&prev, &next, 20.0).unwrap_err();
        assert!(matches!(err, ValidationError::VelocityExceeded { .. }));
    }

    #[test]
    fn velocity_gate_allows_slow_hand() {
        let mut prev = pf(0, 0.0);
        prev.right_hand = Some(tf(0.0, 0.0, 0.0));
        let mut next = pf(100_000, 0.1);
        next.right_hand = Some(tf(0.1, 0.0, 0.0));
        velocity_gate(&prev, &next, 20.0).unwrap();
    }

    #[test]
    fn velocity_gate_rejects_nan_hand_position() {
        let mut prev = pf(0, 0.0);
        prev.left_hand = Some(tf(0.0, 0.0, 0.0));
        let mut next = pf(100_000, 0.1);
        next.left_hand = Some(tf(f32::NAN, 0.0, 0.0));
        let err = velocity_gate(&prev, &next, 20.0).unwrap_err();
        assert!(matches!(err, ValidationError::VelocityExceeded { .. }));
    }

    #[test]
    fn hand_reach_within_limit_passes() {
        let head = tf(0.0, 0.0, 0.0);
        let hand = tf(0.5, 0.0, 0.0);
        hand_reach(&head, &hand, DEFAULT_HAND_REACH_M).unwrap();
    }

    #[test]
    fn hand_reach_beyond_limit_rejected() {
        let head = tf(0.0, 0.0, 0.0);
        let hand = tf(3.0, 0.0, 0.0);
        let err = hand_reach(&head, &hand, DEFAULT_HAND_REACH_M).unwrap_err();
        assert!(matches!(err, ValidationError::HandReachExceeded { .. }));
    }
}
