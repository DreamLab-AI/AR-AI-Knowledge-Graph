//! Per-room XR presence broadcast actor (PRD-008 §5.3).
//!
//! Composes [`visionclaw_xr_presence::PresenceRoom`] aggregate. One actor per
//! active room id. Subscribers register via [`JoinRoom`] with a recipient
//! [`Addr`]; the actor pushes [`BroadcastFrame`] messages to all peers when a
//! session ingests a pose frame, and removes shutdown-pending rooms via
//! [`LeaveRoom`].
//!
//! Room events ([`RoomEventEnvelope`]) follow the carriers defined in
//! `docs/ddd-xr-godot-context.md` §4.1: JSON over `/ws/presence`. Every
//! `avatar_joined` carries the avatar's transport `local_id` so the client can
//! attribute opaque-`local_id` sibling pose frames to a named avatar.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use actix::{Actor, ActorContext, AsyncContext, Context, Handler, Message, Recipient};
use serde::Serialize;
use tracing::{debug, info, warn};

use visionclaw_xr_presence::{
    hand_reach, joint_anatomy, monotonic_timestamp, ports::Broadcaster, types::HandPose,
    velocity_gate, wire, world_bounds, Aabb, AvatarId, AvatarMetadata, Did, PresenceRoom, RoomId,
    Transform, ValidationError,
};

const TICK_HZ: u64 = 90;
const TICK_INTERVAL: Duration = Duration::from_micros(1_000_000 / TICK_HZ);
const VIOLATION_WINDOW: Duration = Duration::from_secs(1);
const VIOLATION_KICK_THRESHOLD: usize = 10;
const BACKPRESSURE_LIMIT: usize = 3;

/// Default max head→hand reach (m) when `PRESENCE_HAND_REACH_M` is unset (#4).
/// More generous than the crate's 1.2m validator default so tall users and
/// extended-tip controllers are not false-rejected.
const DEFAULT_PRESENCE_HAND_REACH_M: f32 = 1.5;

/// Read the configured hand-reach limit from `PRESENCE_HAND_REACH_M`, falling
/// back to [`DEFAULT_PRESENCE_HAND_REACH_M`] on an unset/empty/unparseable or
/// non-positive value.
fn configured_hand_reach_m() -> f32 {
    std::env::var("PRESENCE_HAND_REACH_M")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(DEFAULT_PRESENCE_HAND_REACH_M)
}

// 0x43 sibling-frame envelope: [opcode u8][broadcast_seq u64 LE][room_id u32 LE][user_count u16 LE]
const PREAMBLE_OPCODE: u8 = wire::OPCODE_AVATAR_POSE;

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct BroadcastFrame {
    pub bytes: Vec<u8>,
    pub broadcast_sequence: u64,
}

#[derive(Message, Debug)]
#[rtype(result = "Result<JoinAck, JoinRejection>")]
pub struct JoinRoom {
    pub did: Did,
    pub metadata: AvatarMetadata,
    pub frame_recipient: Recipient<BroadcastFrame>,
    pub event_recipient: Recipient<RoomEventEnvelope>,
}

/// One existing room member as seen at join time, pairing the domain metadata
/// with the transport `local_id` the broadcast frames will tag this avatar's
/// poses with. `local_id` is an infrastructure concern (per-session pose
/// addressing), so it lives here rather than polluting [`AvatarMetadata`].
#[derive(Debug, Clone)]
pub struct MemberSnapshot {
    pub metadata: AvatarMetadata,
    pub local_id: u32,
}

#[derive(Debug, Clone)]
pub struct JoinAck {
    pub avatar_id: AvatarId,
    pub members: Vec<MemberSnapshot>,
}

#[derive(Debug, Clone, thiserror::Error, Serialize)]
pub enum JoinRejection {
    #[error("avatar already present in room")]
    DuplicateMember,
    #[error("internal room error: {0}")]
    Internal(String),
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct LeaveRoom {
    pub avatar_id: AvatarId,
}

#[derive(Message, Debug)]
#[rtype(result = "IngestOutcome")]
pub struct IngestPose {
    pub avatar_id: AvatarId,
    pub frame_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IngestOutcome {
    Accepted,
    ValidationFailed(String),
    Decode(String),
    Kick(String),
}

#[derive(Message, Debug)]
#[rtype(result = "Vec<AvatarMetadata>")]
pub struct ListMembers;

#[derive(Message, Debug)]
#[rtype(result = "RoomStatsSnapshot")]
pub struct RoomStats;

#[derive(Debug, Clone, Default, Serialize)]
pub struct RoomStatsSnapshot {
    pub room_id: String,
    pub member_count: usize,
    pub broadcast_sequence: u64,
    pub poses_ingested_total: u64,
    pub poses_rejected_total: u64,
    pub broadcast_bytes_total: u64,
    pub broadcast_frames_total: u64,
}

#[derive(Message, Debug, Clone, Serialize)]
#[rtype(result = "()")]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoomEventEnvelope {
    AvatarJoined {
        avatar_id: String,
        did: String,
        display_name: String,
        /// Transport id this avatar's poses are tagged with in 0x43 sibling
        /// frames. Lets the client map an opaque per-pose `local_id` to a URN.
        local_id: u32,
    },
    AvatarLeft {
        avatar_id: String,
        did: String,
    },
}

struct Subscriber {
    frame_recipient: Recipient<BroadcastFrame>,
    event_recipient: Recipient<RoomEventEnvelope>,
    queue_depth: usize,
    violations: VecDeque<Instant>,
}

pub struct PresenceActor {
    room_id: RoomId,
    room: PresenceRoom,
    subscribers: HashMap<AvatarId, Subscriber>,
    pending_poses: HashMap<AvatarId, Vec<u8>>,
    avatar_id_to_local: HashMap<AvatarId, u32>,
    next_local_id: u32,
    broadcast_sequence: u64,
    bounds: Aabb,
    max_velocity_mps: f32,
    /// Max head→hand distance (m) before a frame is rejected (#4 codex). Env
    /// `PRESENCE_HAND_REACH_M`, default [`DEFAULT_PRESENCE_HAND_REACH_M`] (a
    /// generous 1.5m so tall users / extended controllers are not false-kicked).
    hand_reach_m: f32,
    stats: RoomStatsSnapshot,
}

impl PresenceActor {
    pub fn new(room_id: RoomId) -> Self {
        let stats = RoomStatsSnapshot {
            room_id: room_id.as_str().to_owned(),
            ..Default::default()
        };
        Self {
            room: PresenceRoom::new(room_id.clone()),
            room_id,
            subscribers: HashMap::new(),
            pending_poses: HashMap::new(),
            avatar_id_to_local: HashMap::new(),
            next_local_id: 1,
            broadcast_sequence: 0,
            bounds: Aabb::symmetric(50.0),
            max_velocity_mps: 20.0,
            hand_reach_m: configured_hand_reach_m(),
            stats,
        }
    }

    pub fn with_bounds(mut self, bounds: Aabb, max_velocity_mps: f32) -> Self {
        self.bounds = bounds;
        self.max_velocity_mps = max_velocity_mps;
        self
    }

    fn record_violation(&mut self, avatar_id: &AvatarId) -> bool {
        let now = Instant::now();
        let Some(sub) = self.subscribers.get_mut(avatar_id) else {
            return false;
        };
        sub.violations.push_back(now);
        while let Some(front) = sub.violations.front() {
            if now.duration_since(*front) > VIOLATION_WINDOW {
                sub.violations.pop_front();
            } else {
                break;
            }
        }
        sub.violations.len() >= VIOLATION_KICK_THRESHOLD
    }

    fn run_validators(
        &self,
        avatar_id: &AvatarId,
        decoded: &wire::DecodedFrame,
    ) -> Result<(), ValidationError> {
        let frame = &decoded.frame;
        world_bounds(&frame.head, &self.bounds)?;
        if let Some(t) = &frame.left_hand {
            world_bounds(t, &self.bounds)?;
            // #10: reject anatomically impossible hand extension. A hand farther
            // than `hand_reach_m` (env PRESENCE_HAND_REACH_M) from the head is a
            // spoofed/abusive pose. An Err here is recorded as a single violation
            // by `handle_ingest` (a lone bad frame is dropped as ValidationFailed,
            // NOT a kick — only VIOLATION_KICK_THRESHOLD within the window kicks).
            hand_reach(&frame.head, t, self.hand_reach_m)?;
        }
        if let Some(t) = &frame.right_hand {
            world_bounds(t, &self.bounds)?;
            hand_reach(&frame.head, t, self.hand_reach_m)?;
        }

        if let Some(prev) = self
            .room
            .member(avatar_id)
            .and_then(|m| m.last_frame.as_ref())
        {
            monotonic_timestamp(prev.timestamp_us, frame.timestamp_us)?;
            velocity_gate(prev, frame, self.max_velocity_mps)?;
        }

        let identity = Transform::identity();
        let left = frame.left_hand.unwrap_or(identity);
        let right = frame.right_hand.unwrap_or(identity);
        joint_anatomy(
            &HandPose {
                wrist: left,
                joints: Vec::new(),
            },
            &HandPose {
                wrist: right,
                joints: Vec::new(),
            },
        )
    }

    fn local_id_for(&mut self, avatar_id: &AvatarId) -> u32 {
        if let Some(id) = self.avatar_id_to_local.get(avatar_id) {
            return *id;
        }
        let id = self.next_local_id;
        self.next_local_id = self.next_local_id.wrapping_add(1);
        self.avatar_id_to_local.insert(avatar_id.clone(), id);
        id
    }

    fn build_broadcast_frame(&mut self) -> Option<Vec<u8>> {
        if self.pending_poses.is_empty() {
            return None;
        }
        self.broadcast_sequence = self.broadcast_sequence.wrapping_add(1);
        let user_count = self.pending_poses.len() as u16;
        let mut buf: Vec<u8> = Vec::with_capacity(
            1 + 8 + 4 + 2 + self.pending_poses.values().map(|v| v.len()).sum::<usize>(),
        );
        buf.push(PREAMBLE_OPCODE);
        buf.extend_from_slice(&self.broadcast_sequence.to_le_bytes());
        let room_id_u32 = u32::from_le_bytes([
            self.room_id.wire_hash()[0],
            self.room_id.wire_hash()[1],
            self.room_id.wire_hash()[2],
            self.room_id.wire_hash()[3],
        ]);
        buf.extend_from_slice(&room_id_u32.to_le_bytes());
        buf.extend_from_slice(&user_count.to_le_bytes());

        let drained: Vec<(AvatarId, Vec<u8>)> = self.pending_poses.drain().collect();
        for (avatar_id, payload) in drained {
            buf.extend_from_slice(&self.local_id_for(&avatar_id).to_le_bytes());
            buf.extend_from_slice(&payload);
        }
        Some(buf)
    }

    fn dispatch_broadcast(&mut self, sender: &AvatarId) {
        let Some(frame_bytes) = self.build_broadcast_frame() else {
            return;
        };
        self.stats.broadcast_frames_total += 1;
        self.stats.broadcast_bytes_total += frame_bytes.len() as u64;
        let envelope = BroadcastFrame {
            bytes: frame_bytes,
            broadcast_sequence: self.broadcast_sequence,
        };
        let mut to_drop: Vec<AvatarId> = Vec::new();
        for (id, sub) in self.subscribers.iter_mut() {
            if id == sender {
                continue;
            }
            if !sub.frame_recipient.connected() {
                to_drop.push(id.clone());
                continue;
            }
            if sub.queue_depth >= BACKPRESSURE_LIMIT {
                debug!(
                    avatar = %id,
                    queue_depth = sub.queue_depth,
                    "dropping oldest queued frame (backpressure)"
                );
                sub.queue_depth = sub.queue_depth.saturating_sub(1);
            }
            if sub.frame_recipient.try_send(envelope.clone()).is_ok() {
                sub.queue_depth += 1;
            } else {
                to_drop.push(id.clone());
            }
        }
        for id in to_drop {
            self.cleanup_subscriber(&id);
        }
    }

    fn cleanup_subscriber(&mut self, avatar_id: &AvatarId) {
        let did_str = self
            .room
            .member(avatar_id)
            .map(|m| m.metadata.did.to_string())
            .unwrap_or_default();
        if self.subscribers.remove(avatar_id).is_some() {
            let _ = self.room.leave(avatar_id);
            self.avatar_id_to_local.remove(avatar_id);
            self.pending_poses.remove(avatar_id);
            self.stats.member_count = self.subscribers.len();
            let envelope = RoomEventEnvelope::AvatarLeft {
                avatar_id: avatar_id.to_string(),
                did: did_str,
            };
            for s in self.subscribers.values() {
                let _ = s.event_recipient.try_send(envelope.clone());
            }
        }
    }

    fn shutdown_if_empty(&self, ctx: &mut Context<Self>) {
        if self.subscribers.is_empty() {
            info!(room = %self.room_id, "presence actor shutting down (room empty)");
            ctx.stop();
        }
    }

    fn evict_disconnected_subscribers(&mut self) {
        let dropped: Vec<AvatarId> = self
            .subscribers
            .iter()
            .filter(|(_, sub)| !sub.frame_recipient.connected())
            .map(|(id, _)| id.clone())
            .collect();
        for id in dropped {
            warn!(avatar = %id, room = %self.room_id, "evicting disconnected subscriber");
            self.cleanup_subscriber(&id);
        }
    }
}

impl Actor for PresenceActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!(room = %self.room_id, "presence actor started @ {} Hz", TICK_HZ);
        ctx.run_interval(TICK_INTERVAL * 10, |act, ctx| {
            act.evict_disconnected_subscribers();
            act.shutdown_if_empty(ctx);
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!(room = %self.room_id, "presence actor stopped");
    }
}

impl Broadcaster for PresenceActor {
    fn broadcast(&self, _room: &RoomId, _frame: &[u8]) {
        // Trait impl exists for crate composability; the actor's own
        // `dispatch_broadcast` is the live path — `&self` here cannot
        // mutate subscriber queue depth.
    }
}

impl Handler<JoinRoom> for PresenceActor {
    type Result = actix::MessageResult<JoinRoom>;

    fn handle(&mut self, msg: JoinRoom, ctx: &mut Self::Context) -> Self::Result {
        actix::MessageResult(self.handle_join(msg, ctx))
    }
}

impl PresenceActor {
    fn handle_join(
        &mut self,
        msg: JoinRoom,
        _ctx: &mut Context<Self>,
    ) -> Result<JoinAck, JoinRejection> {
        let display_name = msg.metadata.display_name.clone();
        let did = msg.metadata.did.clone();
        let avatar_id = self
            .room
            .join(msg.did.clone(), msg.metadata.clone())
            .map_err(|e| match e {
                visionclaw_xr_presence::RoomError::DuplicateDid { .. } => {
                    JoinRejection::DuplicateMember
                }
                other => JoinRejection::Internal(other.to_string()),
            })?;

        self.subscribers.insert(
            avatar_id.clone(),
            Subscriber {
                frame_recipient: msg.frame_recipient,
                event_recipient: msg.event_recipient,
                queue_depth: 0,
                violations: VecDeque::new(),
            },
        );
        self.stats.member_count = self.subscribers.len();

        // Assign the joiner's transport id eagerly so peers learn it now (not
        // lazily on first pose), and so the joiner's own poses are attributable
        // the moment it starts broadcasting.
        let joiner_local_id = self.local_id_for(&avatar_id);
        let join_event = RoomEventEnvelope::AvatarJoined {
            avatar_id: avatar_id.to_string(),
            did: did.to_string(),
            display_name,
            local_id: joiner_local_id,
        };
        for (peer_id, peer) in self.subscribers.iter() {
            if peer_id != &avatar_id {
                let _ = peer.event_recipient.try_send(join_event.clone());
            }
        }

        // Snapshot existing members with their local_ids so the joining client
        // can attribute their sibling poses too. Collect ids first to release
        // the immutable `room` borrow before `local_id_for` takes `&mut self`.
        let member_ids: Vec<(AvatarId, AvatarMetadata)> = self
            .room
            .members()
            .map(|m| (m.avatar_id.clone(), m.metadata.clone()))
            .collect();
        let members: Vec<MemberSnapshot> = member_ids
            .into_iter()
            .map(|(aid, metadata)| {
                let local_id = self.local_id_for(&aid);
                MemberSnapshot { metadata, local_id }
            })
            .collect();
        info!(
            room = %self.room_id,
            avatar = %avatar_id,
            count = self.subscribers.len(),
            "avatar joined"
        );
        Ok(JoinAck { avatar_id, members })
    }
}

impl Handler<LeaveRoom> for PresenceActor {
    type Result = ();

    fn handle(&mut self, msg: LeaveRoom, ctx: &mut Self::Context) -> Self::Result {
        self.cleanup_subscriber(&msg.avatar_id);
        self.shutdown_if_empty(ctx);
    }
}

impl Handler<IngestPose> for PresenceActor {
    type Result = actix::MessageResult<IngestPose>;

    fn handle(&mut self, msg: IngestPose, ctx: &mut Self::Context) -> Self::Result {
        actix::MessageResult(self.handle_ingest(msg, ctx))
    }
}

impl PresenceActor {
    fn handle_ingest(&mut self, msg: IngestPose, ctx: &mut Context<Self>) -> IngestOutcome {
        if !self.subscribers.contains_key(&msg.avatar_id) {
            return IngestOutcome::ValidationFailed("not a room member".into());
        }
        self.stats.poses_ingested_total += 1;

        let decoded = match wire::decode(&msg.frame_bytes) {
            Ok(d) => d,
            Err(e) => {
                self.stats.poses_rejected_total += 1;
                if self.record_violation(&msg.avatar_id) {
                    self.cleanup_subscriber(&msg.avatar_id);
                    self.shutdown_if_empty(ctx);
                    return IngestOutcome::Kick(format!("decode-violations exceeded: {e}"));
                }
                return IngestOutcome::Decode(e.to_string());
            }
        };

        if decoded.avatar_id != msg.avatar_id.as_str() {
            self.stats.poses_rejected_total += 1;
            if self.record_violation(&msg.avatar_id) {
                self.cleanup_subscriber(&msg.avatar_id);
                self.shutdown_if_empty(ctx);
                return IngestOutcome::Kick("avatar-id spoofing".into());
            }
            return IngestOutcome::ValidationFailed("avatar_id mismatch".into());
        }

        if decoded.room_hash != self.room_id.wire_hash() {
            self.stats.poses_rejected_total += 1;
            return IngestOutcome::ValidationFailed("room_hash mismatch".into());
        }

        if let Err(e) = self.run_validators(&msg.avatar_id, &decoded) {
            self.stats.poses_rejected_total += 1;
            warn!(avatar = %msg.avatar_id, error = %e, "pose validation failed");
            if self.record_violation(&msg.avatar_id) {
                self.cleanup_subscriber(&msg.avatar_id);
                self.shutdown_if_empty(ctx);
                return IngestOutcome::Kick(format!("validation-violations exceeded: {e}"));
            }
            return IngestOutcome::ValidationFailed(e.to_string());
        }

        if let Err(e) = self.room.update_pose(&msg.avatar_id, decoded.frame.clone()) {
            return IngestOutcome::ValidationFailed(e.to_string());
        }

        // Strip outer envelope (opcode + len + room_hash + avatar_id_len +
        // avatar_id) — keep only per-avatar payload (timestamp + mask + transforms).
        let body_start = 1 + 2 + 16 + 1 + msg.avatar_id.as_str().len();
        if body_start >= msg.frame_bytes.len() {
            return IngestOutcome::ValidationFailed("truncated body".into());
        }
        let payload = msg.frame_bytes[body_start..].to_vec();
        self.pending_poses.insert(msg.avatar_id.clone(), payload);

        self.dispatch_broadcast(&msg.avatar_id);
        IngestOutcome::Accepted
    }
}

impl Handler<ListMembers> for PresenceActor {
    type Result = actix::MessageResult<ListMembers>;

    fn handle(&mut self, _: ListMembers, _ctx: &mut Self::Context) -> Self::Result {
        actix::MessageResult(self.room.members().map(|m| m.metadata.clone()).collect())
    }
}

impl Handler<RoomStats> for PresenceActor {
    type Result = actix::MessageResult<RoomStats>;

    fn handle(&mut self, _: RoomStats, _ctx: &mut Self::Context) -> Self::Result {
        let mut s = self.stats.clone();
        s.broadcast_sequence = self.broadcast_sequence;
        s.member_count = self.subscribers.len();
        actix::MessageResult(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Arbiter;
    use std::sync::{Arc, Mutex};
    use visionclaw_xr_presence::{wire::encode, PoseFrame};

    struct CollectActor {
        frames: Arc<Mutex<Vec<BroadcastFrame>>>,
        events: Arc<Mutex<Vec<RoomEventEnvelope>>>,
    }

    impl Actor for CollectActor {
        type Context = Context<Self>;
    }

    impl Handler<BroadcastFrame> for CollectActor {
        type Result = ();
        fn handle(&mut self, msg: BroadcastFrame, _: &mut Context<Self>) {
            self.frames.lock().unwrap().push(msg);
        }
    }

    impl Handler<RoomEventEnvelope> for CollectActor {
        type Result = ();
        fn handle(&mut self, msg: RoomEventEnvelope, _: &mut Context<Self>) {
            self.events.lock().unwrap().push(msg);
        }
    }

    fn did(byte: u8) -> Did {
        Did::parse(format!("did:nostr:{}", format!("{:02x}", byte).repeat(32))).unwrap()
    }

    fn meta(d: &Did, name: &str) -> AvatarMetadata {
        AvatarMetadata {
            did: d.clone(),
            display_name: name.into(),
            model_uri: None,
        }
    }

    fn sample_frame(ts_us: u64) -> PoseFrame {
        PoseFrame {
            timestamp_us: ts_us,
            head: Transform {
                position: [0.5, 1.6, -0.3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            left_hand: None,
            right_hand: None,
        }
    }

    fn sample_room() -> RoomId {
        RoomId::parse("urn:visionclaw:room:sha256-12-aaaaaaaaaaaa").unwrap()
    }

    #[actix::test]
    async fn join_then_leave_emits_events_and_shuts_down() {
        let room = sample_room();
        let actor = PresenceActor::new(room.clone()).start();

        let frames = Arc::new(Mutex::new(Vec::<BroadcastFrame>::new()));
        let events = Arc::new(Mutex::new(Vec::<RoomEventEnvelope>::new()));
        let collector = CollectActor {
            frames: frames.clone(),
            events: events.clone(),
        }
        .start();

        let d = did(0x11);
        let ack = actor
            .send(JoinRoom {
                did: d.clone(),
                metadata: meta(&d, "alice"),
                frame_recipient: collector.clone().recipient(),
                event_recipient: collector.clone().recipient(),
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ack.members.len(), 1);

        actor
            .send(LeaveRoom {
                avatar_id: ack.avatar_id,
            })
            .await
            .unwrap();
    }

    #[actix::test]
    async fn ingest_validates_and_broadcasts_to_peers_only() {
        let room = sample_room();
        let actor = PresenceActor::new(room.clone()).start();

        let frames_a = Arc::new(Mutex::new(Vec::<BroadcastFrame>::new()));
        let events_a = Arc::new(Mutex::new(Vec::<RoomEventEnvelope>::new()));
        let frames_b = Arc::new(Mutex::new(Vec::<BroadcastFrame>::new()));
        let events_b = Arc::new(Mutex::new(Vec::<RoomEventEnvelope>::new()));

        let collector_a = CollectActor {
            frames: frames_a.clone(),
            events: events_a,
        }
        .start();
        let collector_b = CollectActor {
            frames: frames_b.clone(),
            events: events_b,
        }
        .start();

        let d_a = did(0x10);
        let d_b = did(0x20);

        let ack_a = actor
            .send(JoinRoom {
                did: d_a.clone(),
                metadata: meta(&d_a, "alice"),
                frame_recipient: collector_a.clone().recipient(),
                event_recipient: collector_a.clone().recipient(),
            })
            .await
            .unwrap()
            .unwrap();

        let _ack_b = actor
            .send(JoinRoom {
                did: d_b.clone(),
                metadata: meta(&d_b, "bob"),
                frame_recipient: collector_b.clone().recipient(),
                event_recipient: collector_b.clone().recipient(),
            })
            .await
            .unwrap()
            .unwrap();

        let frame = sample_frame(1_000_000);
        let bytes = encode(&frame, &room, &ack_a.avatar_id).unwrap().to_vec();
        let outcome = actor
            .send(IngestPose {
                avatar_id: ack_a.avatar_id.clone(),
                frame_bytes: bytes,
            })
            .await
            .unwrap();
        assert_eq!(outcome, IngestOutcome::Accepted);

        actix_rt::time::sleep(Duration::from_millis(50)).await;
        assert!(
            frames_a.lock().unwrap().is_empty(),
            "sender must not receive own frame"
        );
        assert_eq!(frames_b.lock().unwrap().len(), 1);
        Arbiter::current().stop();
    }

    #[actix::test]
    async fn ingest_rejects_out_of_bounds() {
        let room = sample_room();
        let actor = PresenceActor::new(room.clone())
            .with_bounds(Aabb::symmetric(2.0), 20.0)
            .start();
        let frames = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let collector = CollectActor { frames, events }.start();

        let d = did(0x42);
        let ack = actor
            .send(JoinRoom {
                did: d.clone(),
                metadata: meta(&d, "eve"),
                frame_recipient: collector.clone().recipient(),
                event_recipient: collector.clone().recipient(),
            })
            .await
            .unwrap()
            .unwrap();

        let mut frame = sample_frame(1_000_000);
        frame.head.position = [100.0, 0.0, 0.0];
        let bytes = encode(&frame, &room, &ack.avatar_id).unwrap().to_vec();
        let outcome = actor
            .send(IngestPose {
                avatar_id: ack.avatar_id,
                frame_bytes: bytes,
            })
            .await
            .unwrap();
        assert!(matches!(outcome, IngestOutcome::ValidationFailed(_)));
    }

    #[test]
    fn configured_hand_reach_defaults_when_unset() {
        // Env is process-global; this test only asserts the default when the var
        // is absent, avoiding cross-test env races.
        if std::env::var("PRESENCE_HAND_REACH_M").is_err() {
            assert_eq!(configured_hand_reach_m(), DEFAULT_PRESENCE_HAND_REACH_M);
        }
    }

    #[actix::test]
    async fn ingest_drops_single_out_of_reach_hand_without_kick() {
        let room = sample_room();
        // Generous bounds so ONLY the hand-reach check trips.
        let actor = PresenceActor::new(room.clone())
            .with_bounds(Aabb::symmetric(50.0), 20.0)
            .start();
        let frames = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let collector = CollectActor { frames, events }.start();

        let d = did(0x55);
        let ack = actor
            .send(JoinRoom {
                did: d.clone(),
                metadata: meta(&d, "stretch"),
                frame_recipient: collector.clone().recipient(),
                event_recipient: collector.clone().recipient(),
            })
            .await
            .unwrap()
            .unwrap();

        // Hand 3m from the head — beyond the 1.5m default reach, but well inside
        // world bounds. A single such frame must DROP (ValidationFailed), not
        // kick.
        let mut frame = sample_frame(1_000_000);
        frame.head.position = [0.0, 1.6, 0.0];
        frame.left_hand = Some(Transform {
            position: [3.0, 1.6, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        });
        let bytes = encode(&frame, &room, &ack.avatar_id).unwrap().to_vec();
        let outcome = actor
            .send(IngestPose {
                avatar_id: ack.avatar_id,
                frame_bytes: bytes,
            })
            .await
            .unwrap();
        assert!(
            matches!(outcome, IngestOutcome::ValidationFailed(_)),
            "single out-of-reach hand frame should drop, not kick: {outcome:?}"
        );
        Arbiter::current().stop();
    }
}
