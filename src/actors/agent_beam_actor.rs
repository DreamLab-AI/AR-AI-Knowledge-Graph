//! `AgentBeamActor` — the server half of the agent-embodiment render
//! (ADR-059 §4, Phase 2b).
//!
//! Closes the last broadcast gap in the embodiment loop: the `/wss/agent-events`
//! ingest already parses, validates, and publishes every inbound
//! [`AgentActionEnvelope`] to the process-global [`agent_events::hub`], and the
//! browser already fully decodes the `0x23 AGENT_ACTION` binary frame (with its
//! colour map) — but **nothing was broadcasting that frame**. This actor is the
//! missing link: it subscribes to the hub, projects each envelope onto the
//! identity-blind `0x23` wire frame via
//! [`AgentActionEnvelope::to_binary_event`] →
//! [`AgentActionEvent::encode`](crate::utils::binary_protocol::AgentActionEvent::encode),
//! and hands it to [`ClientCoordinatorActor`] for fan-out to every connected
//! client (reusing the established `BroadcastNodePositions` dispatch loop via the
//! [`BroadcastAgentActionFrame`] message).
//!
//! ## Transport path
//!
//! The idiomatic actix `BroadcastStream` + `ctx.add_stream` route is unavailable
//! here: `BroadcastStream` is gated behind `tokio-stream`'s `sync` feature, which
//! is not enabled in this workspace (and the brief forbids adding a dependency
//! just for the beam). So `started` spawns a single lightweight tokio task that
//! loops `recv().await` on the hub receiver and forwards each encoded frame to
//! the coordinator with `do_send` (fire-and-forget). Backpressure uses
//! `broadcast`'s drop-oldest semantics (ADR-059 open-question 3: the beam is
//! duration-based / last-write-wins, so a lagged receiver resyncs on the next
//! frame).
//!
//! ## ID-space contract (LANE A DECISION — Lane B must match)
//!
//! The `0x23` frame carries `source_agent_id` (agent id-space) and
//! `target_node_id` (KG id-space); the client distinguishes agent nodes by the
//! `AGENT_NODE_FLAG = 0x80000000` high bit on the id. **This actor sets that flag
//! on `source_agent_id` immediately before encode** (see [`stamp_agent_flag`]),
//! unconditionally and idempotently, so the wire frame is always
//! client-resolvable regardless of whether the upstream agentbox envelope
//! pre-flagged the id. Rationale: the binary `0x23` frame is, by design, the
//! identity-blind GPU projection (`schema.rs` docstring) and this actor is the
//! single authoritative producer of that frame on the server; centralising the
//! flag-stamp here guarantees one consistent rule rather than depending on every
//! upstream producer to remember it. The KG `target_node_id` is passed through
//! untouched — it is a plain KG-space id with no flag.

use actix::prelude::*;
use log::{debug, error, info, warn};
use tokio::sync::broadcast::error::RecvError;

use crate::actors::messages::BroadcastAgentActionFrame;
use crate::actors::ClientCoordinatorActor;
use crate::agent_events;

/// Agent-node high-bit flag (mirrors `binary_protocol::AGENT_NODE_FLAG`). The
/// client resolves the source node of a beam as an *agent* node iff this bit is
/// set on `source_agent_id`. Kept as a local `const` rather than re-exporting the
/// private `binary_protocol` constant to avoid widening that module's surface.
const AGENT_NODE_FLAG: u32 = 0x80000000;

/// Stamp the agent-node flag onto an agent id-space identifier (idempotent).
#[inline]
fn stamp_agent_flag(source_agent_id: u32) -> u32 {
    source_agent_id | AGENT_NODE_FLAG
}

/// Subscribes to the agent-action hub and broadcasts encoded `0x23` frames.
pub struct AgentBeamActor {
    /// Fan-out target. The coordinator owns the client registry and the binary
    /// `send_binary` dispatch loop; this actor never touches client state.
    coordinator: Addr<ClientCoordinatorActor>,
}

impl AgentBeamActor {
    pub fn new(coordinator: Addr<ClientCoordinatorActor>) -> Self {
        Self { coordinator }
    }
}

impl Actor for AgentBeamActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        let coordinator = self.coordinator.clone();
        let mut rx = agent_events::hub::subscribe();

        // Single forwarding task: recv envelope → project to 0x23 frame → fan out.
        // Runs on the actix/tokio runtime; `do_send` never blocks. The task ends
        // when the hub sender is dropped (process shutdown) — `RecvError::Closed`.
        actix::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(envelope) => {
                        let frame = encode_beam_frame(&envelope);
                        if let Err(e) = coordinator.try_send(BroadcastAgentActionFrame(frame)) {
                            error!(
                                "AgentBeamActor: failed to dispatch agent-action frame: {e}"
                            );
                        }

                        // GLUON (attractive force) — DEFERRED. See `gluon_deferral_note`.
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        // Drop-oldest backpressure: acceptable for a duration-based,
                        // last-write-wins beam. Resync transparently on next frame.
                        warn!(
                            "AgentBeamActor: hub lagged, {skipped} frame(s) skipped — resyncing"
                        );
                    }
                    Err(RecvError::Closed) => {
                        info!("AgentBeamActor: hub closed — forwarding task exiting");
                        break;
                    }
                }
            }
        });

        info!("AgentBeamActor: started — subscribed to agent-events hub (ADR-059 Phase 2b)");
    }
}

/// Project an envelope onto the identity-blind `0x23` wire frame and stamp the
/// agent-node flag so the client resolves the beam's source as an agent node
/// (ID-space contract, see module doc). Pure → unit-testable.
fn encode_beam_frame(envelope: &agent_events::schema::AgentActionEnvelope) -> Vec<u8> {
    let mut binary_event = envelope.to_binary_event();
    binary_event.source_agent_id = stamp_agent_flag(binary_event.source_agent_id);

    let frame = binary_event.encode();
    debug!(
        "AgentBeamActor: 0x23 frame agent={:#010x} target={} action={} ({} bytes)",
        binary_event.source_agent_id,
        binary_event.target_node_id,
        binary_event.action_type,
        frame.len()
    );
    frame
}

/// GLUON (attractive force) — DEFERRED (ADR-059 §4, gluon sub-feature).
///
/// This is a documentation anchor, not dead production code. The intended visual
/// is a transient attractive edge tugging the agent node toward `target_node_id`
/// for `duration_ms`, which the existing spring kernel would naturally turn into
/// attraction. The mechanism that *would* implement it: inject a CSR edge
/// (weight > 0) between the two ids, TTL = `envelope.duration_ms`, keyed off the
/// beam, then auto-remove.
///
/// Deferred — NOT low-risk on the current GPU substrate:
///   1. GPU edges live in a PACKED CSR layout (`row_offsets` / `col_indices` /
///      `edge_weights`), uploaded wholesale by
///      `unified_gpu_compute::memory::initialize_graph` / `upload_edges_csr`.
///      There is no incremental edge-insert path — a single transient edge forces
///      a `resize_buffers` reallocation and a full re-upload of all three CSR
///      arrays.
///   2. `AddEdge` / `RemoveEdge` (`graph_state_actor.rs`) only mutate the
///      in-memory `node_map`; they do NOT propagate to the GPU until a full
///      `BuildGraphFromMetadata`-style rebuild. No clean per-edge GPU mutation
///      message exists today.
///   3. The SSSP and Louvain/community kernels read the SAME CSR buffers; a
///      mid-flight resize/re-upload would race concurrent kernels and destabilise
///      the simulation.
///   4. The stale ADR `class_charge` modulation buffer does NOT exist — only
///      `class_ids:i32` + `class_masses:f32` under the `physics-v2` gate, neither
///      of which is a per-edge attractive force.
///
/// Correct fix (future increment): add an incremental
/// `UpsertTransientEdge { src, tgt, weight, ttl_ms }` GPU message that appends
/// into a SEPARATE transient-edge buffer the spring kernel sums alongside the
/// static CSR, plus a TTL sweep that zeroes expired entries — avoiding any
/// reallocation of the static CSR. Left out here so the beam broadcast lands as a
/// correct, self-contained increment (correctness over completeness). The beam
/// alone already embodies the action visually.
#[allow(dead_code)]
#[inline]
fn gluon_deferral_note() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_events::schema::AgentActionEnvelope;

    fn envelope(source_agent_id: u32, target_node_id: u32) -> AgentActionEnvelope {
        AgentActionEnvelope {
            version: 3,
            id: 1,
            source_agent_id,
            target_node_id,
            action_type: 1,
            action_type_name: "update".to_string(),
            timestamp: 1_748_500_000_000,
            duration_ms: 250,
            source_urn: None,
            target_urn: None,
            pubkey: None,
            token_count: None,
            handoff_id: None,
            verification: None,
            intent: None,
            metadata: serde_json::json!({ "note": "x" }),
        }
    }

    #[test]
    fn agent_flag_is_set_and_idempotent() {
        let flagged = stamp_agent_flag(7);
        assert_eq!(flagged, 0x80000007, "high bit set, low bits preserved");
        assert_eq!(flagged & AGENT_NODE_FLAG, AGENT_NODE_FLAG, "flag bit present");
        assert_eq!(
            stamp_agent_flag(flagged),
            flagged,
            "stamping an already-flagged id is a no-op"
        );
    }

    #[test]
    fn flag_preserves_full_low_bits() {
        let raw: u32 = 0x7FFF_FFFF;
        assert_eq!(stamp_agent_flag(raw), 0xFFFF_FFFF);
        // Client recovers the original id by clearing the flag bit.
        assert_eq!(stamp_agent_flag(raw) & !AGENT_NODE_FLAG, raw);
    }

    #[test]
    fn encoded_frame_is_a_flagged_0x23_frame() {
        let frame = encode_beam_frame(&envelope(7, 4242));

        // Byte 0 is the MessageType::AgentAction tag (0x23).
        assert_eq!(frame[0], 0x23, "frame must lead with the AGENT_ACTION tag");

        // Bytes 1..5 are source_agent_id LE — must carry the agent-node flag.
        let source = u32::from_le_bytes([frame[1], frame[2], frame[3], frame[4]]);
        assert_eq!(source, 0x80000007, "source id is flagged as an agent node");
        assert_eq!(source & !AGENT_NODE_FLAG, 7, "underlying agent id preserved");

        // Bytes 5..9 are target_node_id LE — KG id-space, NOT flagged.
        let target = u32::from_le_bytes([frame[5], frame[6], frame[7], frame[8]]);
        assert_eq!(target, 4242, "KG target id passes through unflagged");
        assert_eq!(target & AGENT_NODE_FLAG, 0, "target carries no agent flag");
    }
}
