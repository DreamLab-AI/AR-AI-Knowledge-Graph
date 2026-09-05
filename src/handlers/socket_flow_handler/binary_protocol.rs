use actix::prelude::*;
use log::{debug, error, info, trace, warn};

use crate::utils::binary_protocol;

use super::types::{SocketFlowServer, WEBSOCKET_RATE_LIMITER};

/// Handle incoming binary WebSocket messages (position updates, voice data, broadcast acks).
impl SocketFlowServer {
    pub(crate) fn handle_binary_message(
        &mut self,
        data: &[u8],
        ctx: &mut <Self as Actor>::Context,
    ) {
        if !WEBSOCKET_RATE_LIMITER.is_allowed(&self.client_ip) {
            warn!(
                "Position update rate limit exceeded for client: {}",
                self.client_ip
            );
            let error_msg = serde_json::json!({
                "type": "rate_limit_warning",
                "message": "Update rate too high, some updates may be dropped",
                "retry_after": WEBSOCKET_RATE_LIMITER.reset_time(&self.client_ip).as_secs()
            });
            if let Ok(msg_str) = serde_json::to_string(&error_msg) {
                ctx.text(msg_str);
            }
            return;
        }

        info!("Received binary message, length: {}", data.len());
        self.last_activity = std::time::Instant::now();

        // Try new binary protocol first
        use crate::utils::binary_protocol::{BinaryProtocol, Message as ProtocolMessage};

        match BinaryProtocol::decode_message(data) {
            Ok(ProtocolMessage::VoiceData { audio }) => {
                info!("Received voice data: {} bytes", audio.len());
                let response = serde_json::json!({
                    "type": "voice_ack",
                    "bytes": audio.len(),
                    "message": "Voice data received but not yet processed"
                });
                if let Ok(msg_str) = serde_json::to_string(&response) {
                    ctx.text(msg_str);
                }
                return;
            }
            Ok(ProtocolMessage::BroadcastAck {
                sequence_id,
                nodes_received,
                timestamp,
            }) => {
                // True end-to-end backpressure: client confirms receipt of position broadcast
                use crate::actors::messages::ClientBroadcastAck;

                trace!(
                    "Received BroadcastAck: seq={}, nodes={}, timestamp={}",
                    sequence_id,
                    nodes_received,
                    timestamp
                );

                self.client_manager_addr.do_send(ClientBroadcastAck {
                    sequence_id,
                    nodes_received,
                    timestamp,
                    client_id: self.client_id,
                });
                return;
            }
            Err(e) => {
                debug!("New protocol decode failed ({}), trying legacy protocol", e);
            }
        }

        // Fall back to legacy binary node data protocol.
        //
        // #11: this branch drives a server-side physics `SimulationStep` (a write
        // to shared graph state). It must be authenticated — an anonymous client
        // must not be able to trigger layout recomputation. Matches the VULN-01
        // gating on the JSON drag path (`handle_node_drag_*`), which also rejects
        // `pubkey.is_none()`. Reject + warn for anon clients before decoding.
        if self.pubkey.is_none() {
            warn!(
                "Ignoring legacy binary node-data write from unauthenticated client: {}",
                self.client_ip
            );
            let error_msg = serde_json::json!({
                "type": "error",
                "message": "Authentication required for position writes",
                "recoverable": false,
            });
            if let Ok(msg_str) = serde_json::to_string(&error_msg) {
                ctx.text(msg_str);
            }
            return;
        }

        match binary_protocol::decode_node_data(data) {
            Ok(nodes) => {
                info!("Decoded {} nodes from binary message", nodes.len());
                let _nodes_vec: Vec<_> = nodes.clone().into_iter().collect();

                {
                    let app_state = self.app_state.clone();
                    let nodes_vec: Vec<_> = nodes.clone().into_iter().collect();

                    let fut = async move {
                        for (node_id, node_data) in &nodes_vec {
                            if *node_id < 5 {
                                debug!(
                                    "Processing binary update for node ID: {} with position [{:.3}, {:.3}, {:.3}]",
                                    node_id, node_data.x, node_data.y, node_data.z
                                );
                            }
                        }

                        info!(
                            "Received {} node positions from client (feedback loop disabled)",
                            nodes_vec.len()
                        );
                        info!("Updated node positions from binary data (preserving server-side properties)");
                        info!("Preparing to recalculate layout after client-side node position update");

                        use crate::actors::messages::GetSettingByPath;
                        let settings_addr = app_state.settings_addr.clone();

                        if let Ok(Ok(_iterations_val)) = settings_addr
                            .send(GetSettingByPath {
                                path: "visualisation.graphs.knowledge.physics.iterations"
                                    .to_string(),
                            })
                            .await
                        {
                            if let Ok(Ok(_spring_val)) = settings_addr
                                .send(GetSettingByPath {
                                    path: "visualisation.graphs.knowledge.physics.spring_k"
                                        .to_string(),
                                })
                                .await
                            {
                                if let Ok(Ok(_repulsion_val)) = settings_addr
                                    .send(GetSettingByPath {
                                        path: "visualisation.graphs.knowledge.physics.repel_k"
                                            .to_string(),
                                    })
                                    .await
                                {
                                    use crate::actors::messages::SimulationStep;
                                    if let Err(e) =
                                        app_state.graph_service_addr.send(SimulationStep).await
                                    {
                                        error!("Failed to trigger simulation step: {}", e);
                                    } else {
                                        info!("Successfully triggered layout recalculation");
                                    }
                                }
                            }
                        }
                    };

                    let fut = fut.into_actor(self);
                    ctx.spawn(fut.map(|_, _, _| ()));
                }
            }
            Err(e) => {
                error!("Failed to decode binary message: {}", e);
                let error_msg = serde_json::json!({
                    "type": "error",
                    "message": format!("Failed to decode binary message: {}", e),
                    "recoverable": true,
                    "details": {
                        "data_length": data.len(),
                        "expected_item_size": 26,
                        "remainder": data.len() % 26
                    }
                });
                if let Ok(msg_str) = serde_json::to_string(&error_msg) {
                    ctx.text(msg_str);
                }
            }
        }
    }
}
