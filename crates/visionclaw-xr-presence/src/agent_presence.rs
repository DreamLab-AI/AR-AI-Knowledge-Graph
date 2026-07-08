//! Agent co-presence codec (opcode 0x44) — the additive sibling of the 0x43
//! avatar-pose frame that carries an agent's *social* state rather than its
//! skeletal pose: discrete activity (`idle`/`working`/`awaiting_approval`/
//! `speaking`), a quantised gaze direction, and an attention target.
//!
//! Adopted by ADR-130 Decision 4 from the copresence research brief
//! (§"Networked presence replication"): one high-rate quantised channel plus one
//! reliable channel for discrete state. The two logical channels share this one
//! codec — [`AgentPresenceDelta::channel`] tells the transport which socket to
//! send a delta on: a delta that touches `state` or `attention` is
//! [`PresenceChannel::Reliable`]; a gaze-only delta is
//! [`PresenceChannel::HighRate`] (10–20 Hz, client-side interpolated).
//!
//! Unchanged fields are elided per agent via a field mask, mirroring the 0x43
//! `transform_mask` idiom in [`crate::delta`]. Gaze directions are compared at
//! wire resolution (16-bit per component) when computing a delta, so sub-quantum
//! float jitter never puts a spurious gaze update on the wire.
//!
//! Wire layout (little-endian):
//! ```text
//! [u8  opcode = 0x44]
//! [u16 body_len_LE]              // bytes that follow this field
//! [u64 seq_LE]
//! [u16 agent_count_LE]
//! per agent:
//!   [u32 local_id_LE]           // server-assigned presence id (see presence.rs)
//!   [u8  field_mask]            // bit0 state, bit1 gaze, bit2 attention
//!   if bit0: [u8 activity]      // 0 idle, 1 working, 2 awaiting_approval, 3 speaking
//!   if bit1: [i16 gx][i16 gy][i16 gz]   // quantised unit gaze dir
//!   if bit2: [u8 attn_tag]      // 0 none, 1 user, 2 node
//!            if tag == 2: [u32 node_id_LE]
//! ```

use bytes::{BufMut, Bytes, BytesMut};

use crate::error::WireError;

/// Agent presence frame opcode. Additive sibling of 0x43 (`wire::OPCODE_AVATAR_POSE`)
/// under ADR-061's single-binary-protocol umbrella.
pub const OPCODE_AGENT_PRESENCE: u8 = 0x44;

const FIELD_STATE: u8 = 0b001;
const FIELD_GAZE: u8 = 0b010;
const FIELD_ATTENTION: u8 = 0b100;
const FIELD_ALL: u8 = FIELD_STATE | FIELD_GAZE | FIELD_ATTENTION;

const ATTN_NONE: u8 = 0;
const ATTN_USER: u8 = 1;
const ATTN_NODE: u8 = 2;

const HEADER_FIXED: usize = 1 + 2; // opcode + body_len
const SEQ_BYTES: usize = 8;
const COUNT_BYTES: usize = 2;

/// Magnitude the quantiser maps a unit component to. `i16::MAX` keeps the
/// mapping symmetric about zero (`-32767..=32767`), leaving `i16::MIN` unused so
/// dequantise never divides by a value the encoder cannot emit.
const QUANT_SCALE: f32 = 32_767.0;

/// Discrete agent activity. Wire byte 0..=3. Drives avatar colour/motion in the
/// Godot client per the research brief (idle bob/dim, working pulse,
/// awaiting-approval saturated colour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentActivity {
    Idle,
    Working,
    AwaitingApproval,
    Speaking,
}

impl AgentActivity {
    pub fn as_u8(self) -> u8 {
        match self {
            AgentActivity::Idle => 0,
            AgentActivity::Working => 1,
            AgentActivity::AwaitingApproval => 2,
            AgentActivity::Speaking => 3,
        }
    }

    pub fn from_u8(b: u8) -> Result<Self, WireError> {
        match b {
            0 => Ok(AgentActivity::Idle),
            1 => Ok(AgentActivity::Working),
            2 => Ok(AgentActivity::AwaitingApproval),
            3 => Ok(AgentActivity::Speaking),
            other => Err(WireError::BadAgentState { state: other }),
        }
    }
}

/// Where an agent's attention (and thus its gaze cone) is directed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionTarget {
    /// Not attending to anyone in particular (idle scan / ambient).
    None,
    /// Attending to the local user (mutual gaze).
    User,
    /// Attending to a referenced graph node (deixis during its task).
    GraphNode(u32),
}

/// Which transport channel a delta should ride. Discrete transitions (activity,
/// attention target) must not be dropped, so they take the reliable channel;
/// gaze is a continuous signal that tolerates loss and is interpolated
/// client-side, so it rides the lossy high-rate channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceChannel {
    HighRate,
    Reliable,
}

/// A full snapshot of one agent's social presence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentPresence {
    pub state: AgentActivity,
    /// Unit gaze/attention direction in room space. Renormalised on decode.
    pub gaze_dir: [f32; 3],
    pub attention: AttentionTarget,
}

impl AgentPresence {
    pub fn new(state: AgentActivity, gaze_dir: [f32; 3], attention: AttentionTarget) -> Self {
        Self {
            state,
            gaze_dir,
            attention,
        }
    }
}

/// A per-agent presence delta: only the fields that changed are `Some`. Built by
/// [`AgentPresenceDelta::between`] (elides unchanged fields) or
/// [`AgentPresenceDelta::full`] (a keyframe carrying every field).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentPresenceDelta {
    pub local_id: u32,
    pub state: Option<AgentActivity>,
    pub gaze_dir: Option<[f32; 3]>,
    pub attention: Option<AttentionTarget>,
}

impl AgentPresenceDelta {
    /// A keyframe: every field present. Sent on join and periodically so a late
    /// or lossy subscriber can recover the full state.
    pub fn full(local_id: u32, p: &AgentPresence) -> Self {
        Self {
            local_id,
            state: Some(p.state),
            gaze_dir: Some(p.gaze_dir),
            attention: Some(p.attention),
        }
    }

    /// Delta from `prev` to `next`. Gaze is compared at wire resolution so
    /// sub-quantum jitter is not transmitted; state and attention compare exactly.
    pub fn between(local_id: u32, prev: &AgentPresence, next: &AgentPresence) -> Self {
        let state = (prev.state != next.state).then_some(next.state);
        let gaze_dir =
            (quantise_dir(prev.gaze_dir) != quantise_dir(next.gaze_dir)).then_some(next.gaze_dir);
        let attention = (prev.attention != next.attention).then_some(next.attention);
        Self {
            local_id,
            state,
            gaze_dir,
            attention,
        }
    }

    pub fn field_mask(&self) -> u8 {
        let mut mask = 0u8;
        if self.state.is_some() {
            mask |= FIELD_STATE;
        }
        if self.gaze_dir.is_some() {
            mask |= FIELD_GAZE;
        }
        if self.attention.is_some() {
            mask |= FIELD_ATTENTION;
        }
        mask
    }

    /// A delta carries no field (heartbeat) — still a valid frame element.
    pub fn is_empty(&self) -> bool {
        self.field_mask() == 0
    }

    /// Discrete transitions (state, attention) demand the reliable channel; a
    /// gaze-only (or empty) delta rides the high-rate channel.
    pub fn channel(&self) -> PresenceChannel {
        if self.state.is_some() || self.attention.is_some() {
            PresenceChannel::Reliable
        } else {
            PresenceChannel::HighRate
        }
    }

    /// Reconstruct the full presence by applying this delta over the last-known
    /// snapshot. Fields absent from the delta are inherited from `base`.
    pub fn apply(&self, base: &AgentPresence) -> AgentPresence {
        AgentPresence {
            state: self.state.unwrap_or(base.state),
            gaze_dir: self.gaze_dir.unwrap_or(base.gaze_dir),
            attention: self.attention.unwrap_or(base.attention),
        }
    }
}

/// One decoded frame: a batch of agent deltas with the broadcast sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentPresenceBatch {
    pub seq: u64,
    pub deltas: Vec<AgentPresenceDelta>,
}

/// Encode a batch of agent-presence deltas into an opcode-0x44 frame.
pub fn encode_agent_presence(seq: u64, deltas: &[AgentPresenceDelta]) -> Result<Bytes, WireError> {
    let mut body_len = SEQ_BYTES + COUNT_BYTES;
    for d in deltas {
        body_len += 4 + 1; // local_id + mask
        if d.state.is_some() {
            body_len += 1;
        }
        if d.gaze_dir.is_some() {
            body_len += 6;
        }
        if let Some(a) = d.attention {
            body_len += 1;
            if matches!(a, AttentionTarget::GraphNode(_)) {
                body_len += 4;
            }
        }
    }
    if body_len > u16::MAX as usize {
        return Err(WireError::FrameTooLarge {
            len: body_len,
            max: u16::MAX as usize,
        });
    }

    let mut buf = BytesMut::with_capacity(HEADER_FIXED + body_len);
    buf.put_u8(OPCODE_AGENT_PRESENCE);
    buf.put_u16_le(body_len as u16);
    buf.put_u64_le(seq);
    buf.put_u16_le(deltas.len() as u16);
    for d in deltas {
        buf.put_u32_le(d.local_id);
        buf.put_u8(d.field_mask());
        if let Some(s) = d.state {
            buf.put_u8(s.as_u8());
        }
        if let Some(g) = d.gaze_dir {
            let q = quantise_dir(g);
            buf.put_i16_le(q[0]);
            buf.put_i16_le(q[1]);
            buf.put_i16_le(q[2]);
        }
        if let Some(a) = d.attention {
            match a {
                AttentionTarget::None => buf.put_u8(ATTN_NONE),
                AttentionTarget::User => buf.put_u8(ATTN_USER),
                AttentionTarget::GraphNode(id) => {
                    buf.put_u8(ATTN_NODE);
                    buf.put_u32_le(id);
                }
            }
        }
    }
    Ok(buf.freeze())
}

/// Decode an opcode-0x44 agent-presence frame. Total over arbitrary input: every
/// read is bounds-checked, so a malformed or truncated buffer returns a
/// [`WireError`] and never panics (fuzz contract, see `fuzz/`).
pub fn decode_agent_presence(bytes: &[u8]) -> Result<AgentPresenceBatch, WireError> {
    if bytes.len() < HEADER_FIXED {
        return Err(WireError::TooShort {
            need: HEADER_FIXED,
            got: bytes.len(),
        });
    }
    if bytes[0] != OPCODE_AGENT_PRESENCE {
        return Err(WireError::BadOpcode {
            found: bytes[0],
            expected: OPCODE_AGENT_PRESENCE,
        });
    }
    let body_len = u16::from_le_bytes([bytes[1], bytes[2]]) as usize;
    if bytes.len() < HEADER_FIXED + body_len {
        return Err(WireError::LengthMismatch {
            declared: body_len,
            actual: bytes.len().saturating_sub(HEADER_FIXED),
        });
    }
    let body = &bytes[HEADER_FIXED..HEADER_FIXED + body_len];
    if body.len() < SEQ_BYTES + COUNT_BYTES {
        return Err(WireError::TooShort {
            need: SEQ_BYTES + COUNT_BYTES,
            got: body.len(),
        });
    }

    let mut cursor = 0usize;
    let seq = u64::from_le_bytes(read_array::<8>(body, cursor)?);
    cursor += SEQ_BYTES;
    let count = u16::from_le_bytes(read_array::<2>(body, cursor)?) as usize;
    cursor += COUNT_BYTES;

    let mut deltas = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        let local_id = u32::from_le_bytes(read_array::<4>(body, cursor)?);
        cursor += 4;
        let mask = *body.get(cursor).ok_or(WireError::LengthMismatch {
            declared: body_len,
            actual: body.len(),
        })?;
        cursor += 1;
        if mask & !FIELD_ALL != 0 {
            return Err(WireError::BadTransformCount { count: mask });
        }

        let state = if mask & FIELD_STATE != 0 {
            let b = *body.get(cursor).ok_or(WireError::LengthMismatch {
                declared: body_len,
                actual: body.len(),
            })?;
            cursor += 1;
            Some(AgentActivity::from_u8(b)?)
        } else {
            None
        };

        let gaze_dir = if mask & FIELD_GAZE != 0 {
            let gx = i16::from_le_bytes(read_array::<2>(body, cursor)?);
            cursor += 2;
            let gy = i16::from_le_bytes(read_array::<2>(body, cursor)?);
            cursor += 2;
            let gz = i16::from_le_bytes(read_array::<2>(body, cursor)?);
            cursor += 2;
            Some(dequantise_dir([gx, gy, gz]))
        } else {
            None
        };

        let attention = if mask & FIELD_ATTENTION != 0 {
            let tag = *body.get(cursor).ok_or(WireError::LengthMismatch {
                declared: body_len,
                actual: body.len(),
            })?;
            cursor += 1;
            match tag {
                ATTN_NONE => Some(AttentionTarget::None),
                ATTN_USER => Some(AttentionTarget::User),
                ATTN_NODE => {
                    let id = u32::from_le_bytes(read_array::<4>(body, cursor)?);
                    cursor += 4;
                    Some(AttentionTarget::GraphNode(id))
                }
                other => return Err(WireError::BadAttentionTag { tag: other }),
            }
        } else {
            None
        };

        deltas.push(AgentPresenceDelta {
            local_id,
            state,
            gaze_dir,
            attention,
        });
    }

    Ok(AgentPresenceBatch { seq, deltas })
}

#[inline]
fn read_array<const N: usize>(body: &[u8], at: usize) -> Result<[u8; N], WireError> {
    let end = at.checked_add(N).ok_or(WireError::LengthMismatch {
        declared: N,
        actual: body.len(),
    })?;
    if end > body.len() {
        return Err(WireError::LengthMismatch {
            declared: end,
            actual: body.len(),
        });
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&body[at..end]);
    Ok(out)
}

/// Quantise a direction to three signed 16-bit components. The input is first
/// normalised; a degenerate (near-zero) vector falls back to `[0, 0, -1]`
/// (camera-forward), matching the interaction-ray fallback so a lost gaze reads
/// as "looking ahead" rather than snapping to an axis.
pub fn quantise_dir(d: [f32; 3]) -> [i16; 3] {
    let n = normalise(d);
    [
        quantise_component(n[0]),
        quantise_component(n[1]),
        quantise_component(n[2]),
    ]
}

/// Inverse of [`quantise_dir`]: renormalise the decoded components to a unit
/// vector. A zero triple (never emitted by the encoder) falls back to `[0,0,-1]`.
pub fn dequantise_dir(q: [i16; 3]) -> [f32; 3] {
    normalise([
        q[0] as f32 / QUANT_SCALE,
        q[1] as f32 / QUANT_SCALE,
        q[2] as f32 / QUANT_SCALE,
    ])
}

fn quantise_component(v: f32) -> i16 {
    (v * QUANT_SCALE).round().clamp(-QUANT_SCALE, QUANT_SCALE) as i16
}

fn normalise(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-6 || !len.is_finite() {
        return [0.0, 0.0, -1.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presence(state: AgentActivity, dir: [f32; 3], attn: AttentionTarget) -> AgentPresence {
        AgentPresence::new(state, dir, attn)
    }

    #[test]
    fn round_trip_single_full_keyframe() {
        let p = presence(
            AgentActivity::Working,
            [0.0, 0.0, -1.0],
            AttentionTarget::GraphNode(4242),
        );
        let delta = AgentPresenceDelta::full(7, &p);
        let bytes = encode_agent_presence(99, &[delta]).unwrap();
        assert_eq!(bytes[0], OPCODE_AGENT_PRESENCE);
        let batch = decode_agent_presence(&bytes).unwrap();
        assert_eq!(batch.seq, 99);
        assert_eq!(batch.deltas.len(), 1);
        let d = batch.deltas[0];
        assert_eq!(d.local_id, 7);
        assert_eq!(d.state, Some(AgentActivity::Working));
        assert_eq!(d.attention, Some(AttentionTarget::GraphNode(4242)));
        // gaze survives quantisation to within tolerance
        let g = d.gaze_dir.unwrap();
        assert!((g[2] + 1.0).abs() < 1e-3, "gaze z should be ~ -1, got {}", g[2]);
    }

    #[test]
    fn round_trip_multi_agent_mixed_masks() {
        let a = AgentPresenceDelta {
            local_id: 1,
            state: Some(AgentActivity::Speaking),
            gaze_dir: None,
            attention: Some(AttentionTarget::User),
        };
        let b = AgentPresenceDelta {
            local_id: 2,
            state: None,
            gaze_dir: Some([1.0, 0.0, 0.0]),
            attention: None,
        };
        let c = AgentPresenceDelta {
            local_id: 3,
            state: Some(AgentActivity::Idle),
            gaze_dir: Some([0.0, 1.0, 0.0]),
            attention: Some(AttentionTarget::None),
        };
        let bytes = encode_agent_presence(5, &[a, b, c]).unwrap();
        let batch = decode_agent_presence(&bytes).unwrap();
        assert_eq!(batch.deltas.len(), 3);
        assert_eq!(batch.deltas[0].state, Some(AgentActivity::Speaking));
        assert_eq!(batch.deltas[0].attention, Some(AttentionTarget::User));
        assert!(batch.deltas[0].gaze_dir.is_none());
        assert!(batch.deltas[1].state.is_none());
        assert!(batch.deltas[1].gaze_dir.is_some());
        assert_eq!(batch.deltas[2].attention, Some(AttentionTarget::None));
    }

    #[test]
    fn empty_batch_round_trips() {
        let bytes = encode_agent_presence(0, &[]).unwrap();
        let batch = decode_agent_presence(&bytes).unwrap();
        assert_eq!(batch.seq, 0);
        assert!(batch.deltas.is_empty());
    }

    #[test]
    fn heartbeat_delta_has_empty_mask_and_high_rate_channel() {
        let d = AgentPresenceDelta {
            local_id: 9,
            state: None,
            gaze_dir: None,
            attention: None,
        };
        assert!(d.is_empty());
        assert_eq!(d.channel(), PresenceChannel::HighRate);
        let bytes = encode_agent_presence(1, &[d]).unwrap();
        let batch = decode_agent_presence(&bytes).unwrap();
        assert_eq!(batch.deltas[0], d);
    }

    #[test]
    fn channel_routing_matches_field_semantics() {
        let gaze_only = AgentPresenceDelta {
            local_id: 1,
            state: None,
            gaze_dir: Some([0.0, 0.0, -1.0]),
            attention: None,
        };
        assert_eq!(gaze_only.channel(), PresenceChannel::HighRate);

        let state_change = AgentPresenceDelta {
            local_id: 1,
            state: Some(AgentActivity::AwaitingApproval),
            gaze_dir: None,
            attention: None,
        };
        assert_eq!(state_change.channel(), PresenceChannel::Reliable);

        let attention_change = AgentPresenceDelta {
            local_id: 1,
            state: None,
            gaze_dir: Some([1.0, 0.0, 0.0]),
            attention: Some(AttentionTarget::User),
        };
        assert_eq!(attention_change.channel(), PresenceChannel::Reliable);
    }

    #[test]
    fn between_elides_unchanged_fields() {
        let prev = presence(AgentActivity::Working, [0.0, 0.0, -1.0], AttentionTarget::None);
        // only gaze moves, by more than one quantum
        let next = presence(AgentActivity::Working, [0.2, 0.0, -1.0], AttentionTarget::None);
        let d = AgentPresenceDelta::between(0, &prev, &next);
        assert!(d.state.is_none(), "state unchanged must elide");
        assert!(d.attention.is_none(), "attention unchanged must elide");
        assert!(d.gaze_dir.is_some(), "gaze moved must be present");
        assert_eq!(d.channel(), PresenceChannel::HighRate);
    }

    #[test]
    fn between_elides_subquantum_gaze_jitter() {
        let prev = presence(AgentActivity::Idle, [0.0, 0.0, -1.0], AttentionTarget::None);
        // A displacement far below one 16-bit quantum must NOT produce a gaze delta.
        let next = presence(
            AgentActivity::Idle,
            [1e-6, 0.0, -1.0],
            AttentionTarget::None,
        );
        let d = AgentPresenceDelta::between(0, &prev, &next);
        assert!(d.is_empty(), "sub-quantum jitter must elide to a heartbeat");
    }

    #[test]
    fn apply_reconstructs_full_state() {
        let base = presence(AgentActivity::Idle, [0.0, 0.0, -1.0], AttentionTarget::None);
        let next = presence(
            AgentActivity::AwaitingApproval,
            [0.0, 0.0, -1.0],
            AttentionTarget::User,
        );
        let d = AgentPresenceDelta::between(0, &base, &next);
        let restored = d.apply(&base);
        assert_eq!(restored.state, AgentActivity::AwaitingApproval);
        assert_eq!(restored.attention, AttentionTarget::User);
    }

    #[test]
    fn rejects_wrong_opcode() {
        let mut bytes = encode_agent_presence(0, &[]).unwrap().to_vec();
        bytes[0] = OPCODE_AGENT_PRESENCE.wrapping_add(1);
        assert!(matches!(
            decode_agent_presence(&bytes),
            Err(WireError::BadOpcode { .. })
        ));
    }

    #[test]
    fn rejects_bad_activity_byte() {
        // hand-build a frame with an out-of-range activity byte
        let mut body = Vec::new();
        body.extend_from_slice(&0u64.to_le_bytes()); // seq
        body.extend_from_slice(&1u16.to_le_bytes()); // count
        body.extend_from_slice(&0u32.to_le_bytes()); // local_id
        body.push(FIELD_STATE); // mask
        body.push(9); // bad activity
        let mut frame = vec![OPCODE_AGENT_PRESENCE];
        frame.extend_from_slice(&(body.len() as u16).to_le_bytes());
        frame.extend_from_slice(&body);
        assert!(matches!(
            decode_agent_presence(&frame),
            Err(WireError::BadAgentState { state: 9 })
        ));
    }

    #[test]
    fn rejects_bad_attention_tag() {
        let mut body = Vec::new();
        body.extend_from_slice(&0u64.to_le_bytes());
        body.extend_from_slice(&1u16.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(FIELD_ATTENTION);
        body.push(7); // bad tag
        let mut frame = vec![OPCODE_AGENT_PRESENCE];
        frame.extend_from_slice(&(body.len() as u16).to_le_bytes());
        frame.extend_from_slice(&body);
        assert!(matches!(
            decode_agent_presence(&frame),
            Err(WireError::BadAttentionTag { tag: 7 })
        ));
    }

    #[test]
    fn rejects_truncated_body() {
        let bytes = encode_agent_presence(
            3,
            &[AgentPresenceDelta::full(
                1,
                &presence(AgentActivity::Working, [0.0, 0.0, -1.0], AttentionTarget::User),
            )],
        )
        .unwrap();
        let truncated = &bytes[..bytes.len() - 3];
        assert!(decode_agent_presence(truncated).is_err());
    }

    #[test]
    fn rejects_reserved_mask_bits() {
        let mut body = Vec::new();
        body.extend_from_slice(&0u64.to_le_bytes());
        body.extend_from_slice(&1u16.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(0b1000); // reserved bit set
        let mut frame = vec![OPCODE_AGENT_PRESENCE];
        frame.extend_from_slice(&(body.len() as u16).to_le_bytes());
        frame.extend_from_slice(&body);
        assert!(decode_agent_presence(&frame).is_err());
    }

    #[test]
    fn quantise_dequantise_preserves_direction() {
        for d in [
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.577, 0.577, -0.577],
            [-0.3, 0.8, -0.5],
        ] {
            let r = dequantise_dir(quantise_dir(d));
            let nd = normalise(d);
            let dot = nd[0] * r[0] + nd[1] * r[1] + nd[2] * r[2];
            assert!(dot > 0.9999, "angular error too large for {d:?}: dot={dot}");
        }
    }

    #[test]
    fn quantise_degenerate_falls_back_to_forward() {
        assert_eq!(dequantise_dir(quantise_dir([0.0, 0.0, 0.0])), [0.0, 0.0, -1.0]);
    }
}
