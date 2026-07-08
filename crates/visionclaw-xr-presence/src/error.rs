use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WireError {
    #[error("frame too short: need {need} bytes, got {got}")]
    TooShort { need: usize, got: usize },

    #[error("frame declares length {declared} but buffer is {actual}")]
    LengthMismatch { declared: usize, actual: usize },

    #[error("unexpected opcode 0x{found:02x}, expected 0x{expected:02x}")]
    BadOpcode { found: u8, expected: u8 },

    #[error("invalid transform_count {count}: must be 1, 2, or 3")]
    BadTransformCount { count: u8 },

    #[error("avatar_id length {len} exceeds wire maximum {max}")]
    AvatarIdTooLong { len: usize, max: usize },

    #[error("avatar_id is not valid utf-8")]
    AvatarIdNotUtf8,

    #[error("agent-presence frame body {len} exceeds u16 maximum {max}")]
    FrameTooLarge { len: usize, max: usize },

    #[error("invalid agent activity byte {state}: must be 0..=3")]
    BadAgentState { state: u8 },

    #[error("invalid attention tag {tag}: must be 0 (none), 1 (user), or 2 (node)")]
    BadAttentionTag { tag: u8 },
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum ValidationError {
    #[error("velocity {observed_mps:.3} m/s exceeds gate {limit_mps:.3} m/s")]
    VelocityExceeded { observed_mps: f32, limit_mps: f32 },

    #[error("position [{x:.3}, {y:.3}, {z:.3}] outside world bounds")]
    OutOfBounds { x: f32, y: f32, z: f32 },

    #[error("non-monotonic timestamp: prev={prev_us} next={next_us}")]
    NonMonotonicTimestamp { prev_us: u64, next_us: u64 },

    #[error("duplicate timestamp: {ts_us} (replay)")]
    DuplicateTimestamp { ts_us: u64 },

    #[error("quaternion magnitude {mag:.4} outside unit tolerance [{lo:.4}, {hi:.4}]")]
    NonUnitQuaternion { mag: f32, lo: f32, hi: f32 },

    #[error("hand-to-head distance {observed_m:.3} m exceeds anatomical reach {limit_m:.3} m")]
    HandReachExceeded { observed_m: f32, limit_m: f32 },

    #[error("frame interval {dt_us} us below minimum {min_us} us (rate-limit)")]
    IntervalTooShort { dt_us: u64, min_us: u64 },
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RoomError {
    #[error("DID {did} already has avatar {existing} in this room")]
    DuplicateDid { did: String, existing: String },

    #[error("avatar {avatar_id} not found in room")]
    UnknownAvatar { avatar_id: String },

    #[error("invalid URN: {urn}")]
    InvalidUrn { urn: String },

    #[error("invalid DID: {did}")]
    InvalidDid { did: String },
}
