//! Audio Router — User-scoped voice channel multiplexer
//!
//! Routes audio between four planes:
//!   Plane 1: User mic → Turbo Whisper STT → agent commands (private per-user)
//!   Plane 2: Agent response → Kokoro TTS → owner's ears (private per-user)
//!   Plane 3: User mic → LiveKit SFU → all users (public spatial voice chat)
//!   Plane 4: Agent TTS → LiveKit SFU at agent position → all users (public spatial)
//!
//! Each user gets an isolated session with their own broadcast channels.
//! Push-to-talk (PTT) controls whether mic audio goes to Plane 1 (agent commands)
//! or Plane 3 (voice chat). When PTT is held, audio routes to STT for agent control.
//! When PTT is released, audio routes to LiveKit for spatial voice chat.

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Per-user voice session with isolated audio channels
#[derive(Debug)]
pub struct UserVoiceSession {
    pub user_id: String,
    /// Private channel: TTS audio meant only for this user
    pub private_audio_tx: broadcast::Sender<Vec<u8>>,
    /// Private channel: transcription results for this user
    pub transcription_tx: broadcast::Sender<String>,
    /// Agent IDs owned by this user
    pub owned_agents: Vec<String>,
    /// Whether the user is currently in PTT (push-to-talk) mode
    pub ptt_active: bool,
    /// LiveKit participant ID for spatial audio
    pub livekit_participant_id: Option<String>,
    /// User's 3D position in the XR world (for spatial audio)
    pub spatial_position: [f32; 3],
}

/// Agent voice identity — each agent has a distinct voice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVoiceIdentity {
    pub agent_id: String,
    pub agent_type: String,
    pub owner_user_id: String,
    /// Kokoro voice preset ID (e.g., "af_sarah", "am_adam")
    pub voice_id: String,
    /// Speech speed multiplier
    pub speed: f32,
    /// Agent's 3D position in XR world
    pub position: [f32; 3],
    /// Whether voice is public (all users hear spatially) or private (owner only)
    pub public_voice: bool,
}

/// Audio Router: manages per-user sessions and agent voice routing
pub struct AudioRouter {
    /// Active user sessions keyed by user_id
    sessions: Arc<RwLock<HashMap<String, UserVoiceSession>>>,
    /// Agent voice identities keyed by agent_id
    agent_voices: Arc<RwLock<HashMap<String, AgentVoiceIdentity>>>,
    /// Default agent voice presets by agent_type
    default_voice_presets: Arc<RwLock<HashMap<String, VoicePreset>>>,
    /// Global audio broadcast for legacy compatibility (non-user-scoped clients)
    global_audio_tx: broadcast::Sender<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoicePreset {
    pub voice_id: String,
    pub speed: f32,
}

/// Default voice presets for different agent types
fn default_agent_voice_presets() -> HashMap<String, VoicePreset> {
    let mut presets = HashMap::new();
    presets.insert("researcher".to_string(), VoicePreset { voice_id: "af_sarah".to_string(), speed: 1.0 });
    presets.insert("coder".to_string(), VoicePreset { voice_id: "am_adam".to_string(), speed: 1.1 });
    presets.insert("analyst".to_string(), VoicePreset { voice_id: "bf_emma".to_string(), speed: 1.0 });
    presets.insert("optimizer".to_string(), VoicePreset { voice_id: "am_michael".to_string(), speed: 0.95 });
    presets.insert("coordinator".to_string(), VoicePreset { voice_id: "af_heart".to_string(), speed: 1.0 });
    presets
}

impl AudioRouter {
    pub fn new() -> Self {
        let (global_audio_tx, _) = broadcast::channel(100);

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            agent_voices: Arc::new(RwLock::new(HashMap::new())),
            default_voice_presets: Arc::new(RwLock::new(default_agent_voice_presets())),
            global_audio_tx,
        }
    }

    /// Register a new user voice session
    pub async fn register_user(&self, user_id: &str) -> (broadcast::Receiver<Vec<u8>>, broadcast::Receiver<String>) {
        let mut sessions = self.sessions.write().await;

        if let Some(existing) = sessions.get(user_id) {
            info!("User {} already registered, returning existing channels", user_id);
            return (
                existing.private_audio_tx.subscribe(),
                existing.transcription_tx.subscribe(),
            );
        }

        let (audio_tx, audio_rx) = broadcast::channel(100);
        let (transcription_tx, transcription_rx) = broadcast::channel(100);

        let session = UserVoiceSession {
            user_id: user_id.to_string(),
            private_audio_tx: audio_tx,
            transcription_tx,
            owned_agents: Vec::new(),
            ptt_active: false,
            livekit_participant_id: None,
            spatial_position: [0.0, 0.0, 0.0],
        };

        sessions.insert(user_id.to_string(), session);
        info!("Registered voice session for user {}", user_id);

        (audio_rx, transcription_rx)
    }

    /// Unregister a user voice session
    pub async fn unregister_user(&self, user_id: &str) {
        let mut sessions = self.sessions.write().await;
        if sessions.remove(user_id).is_some() {
            info!("Unregistered voice session for user {}", user_id);
        }

        // Clean up any agents owned by this user
        let mut agents = self.agent_voices.write().await;
        agents.retain(|_, v| v.owner_user_id != user_id);
    }

    /// Set PTT (push-to-talk) state for a user
    pub async fn set_ptt(&self, user_id: &str, active: bool) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(user_id) {
            session.ptt_active = active;
            debug!("User {} PTT: {}", user_id, if active { "ACTIVE" } else { "RELEASED" });
        }
    }

    /// Check if a user's PTT is active
    pub async fn is_ptt_active(&self, user_id: &str) -> bool {
        let sessions = self.sessions.read().await;
        sessions.get(user_id).map(|s| s.ptt_active).unwrap_or(false)
    }

    /// Register an agent with a voice identity
    pub async fn register_agent(
        &self,
        agent_id: &str,
        agent_type: &str,
        owner_user_id: &str,
        position: [f32; 3],
        public_voice: bool,
    ) {
        let presets = self.default_voice_presets.read().await;
        let preset = presets.get(agent_type).cloned().unwrap_or(VoicePreset {
            voice_id: "af_heart".to_string(),
            speed: 1.0,
        });

        let identity = AgentVoiceIdentity {
            agent_id: agent_id.to_string(),
            agent_type: agent_type.to_string(),
            owner_user_id: owner_user_id.to_string(),
            voice_id: preset.voice_id,
            speed: preset.speed,
            position,
            public_voice,
        };

        // Add agent to owner's session
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(owner_user_id) {
                if !session.owned_agents.contains(&agent_id.to_string()) {
                    session.owned_agents.push(agent_id.to_string());
                }
            }
        }

        self.agent_voices.write().await.insert(agent_id.to_string(), identity);
        info!("Registered agent {} (type={}) for user {}", agent_id, agent_type, owner_user_id);
    }

    /// Update an agent's spatial position
    pub async fn update_agent_position(&self, agent_id: &str, position: [f32; 3]) {
        let mut agents = self.agent_voices.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.position = position;
        }
    }

    /// Get voice identity for an agent (used to select Kokoro voice preset for TTS)
    pub async fn get_agent_voice(&self, agent_id: &str) -> Option<AgentVoiceIdentity> {
        self.agent_voices.read().await.get(agent_id).cloned()
    }

    /// Route TTS audio to the correct destination based on agent ownership and publicity
    pub async fn route_agent_audio(
        &self,
        agent_id: &str,
        audio_data: Vec<u8>,
    ) -> Result<(), String> {
        let agents = self.agent_voices.read().await;
        let agent = agents.get(agent_id).ok_or_else(|| format!("Unknown agent: {}", agent_id))?;

        if agent.public_voice {
            // Plane 4: spatial audio — send to global broadcast AND private channel
            // (LiveKit injection happens at the client/bridge layer)
            let _ = self.global_audio_tx.send(audio_data.clone());

            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&agent.owner_user_id) {
                let _ = session.private_audio_tx.send(audio_data);
            }
            debug!("Routed public spatial audio for agent {}", agent_id);
        } else {
            // Plane 2: private response — only to owner
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&agent.owner_user_id) {
                session.private_audio_tx.send(audio_data).map_err(|e| {
                    format!("Failed to send private audio to user {}: {}", agent.owner_user_id, e)
                })?;
                debug!("Routed private audio for agent {} to user {}", agent_id, agent.owner_user_id);
            } else {
                warn!("No session for agent {} owner {}", agent_id, agent.owner_user_id);
            }
        }

        Ok(())
    }

    /// Route transcription text to the correct user
    pub async fn route_transcription(
        &self,
        user_id: &str,
        text: String,
    ) -> Result<(), String> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(user_id) {
            session.transcription_tx.send(text).map_err(|e| {
                format!("Failed to send transcription to user {}: {}", user_id, e)
            })?;
        } else {
            warn!("No session for user {} — transcription dropped", user_id);
        }
        Ok(())
    }

    /// Get a subscriber for a specific user's private audio channel
    pub async fn subscribe_user_audio(&self, user_id: &str) -> Option<broadcast::Receiver<Vec<u8>>> {
        let sessions = self.sessions.read().await;
        sessions.get(user_id).map(|s| s.private_audio_tx.subscribe())
    }

    /// Get a subscriber for a specific user's transcription channel
    pub async fn subscribe_user_transcriptions(&self, user_id: &str) -> Option<broadcast::Receiver<String>> {
        let sessions = self.sessions.read().await;
        sessions.get(user_id).map(|s| s.transcription_tx.subscribe())
    }

    /// Get the global audio broadcast for legacy (non-user-scoped) clients
    pub fn subscribe_global_audio(&self) -> broadcast::Receiver<Vec<u8>> {
        self.global_audio_tx.subscribe()
    }

    /// Get all agents owned by a user
    pub async fn get_user_agents(&self, user_id: &str) -> Vec<AgentVoiceIdentity> {
        let sessions = self.sessions.read().await;
        let agent_ids = match sessions.get(user_id) {
            Some(session) => session.owned_agents.clone(),
            None => return Vec::new(),
        };
        drop(sessions);

        let agents = self.agent_voices.read().await;
        agent_ids
            .iter()
            .filter_map(|id| agents.get(id).cloned())
            .collect()
    }

    /// Update user's spatial position (for XR presence sync)
    pub async fn update_user_position(&self, user_id: &str, position: [f32; 3]) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(user_id) {
            session.spatial_position = position;
        }
    }

    /// Set a user's LiveKit participant ID
    pub async fn set_livekit_participant(&self, user_id: &str, participant_id: String) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(user_id) {
            session.livekit_participant_id = Some(participant_id);
        }
    }

    /// Get summary of active voice sessions (for monitoring)
    pub async fn get_status(&self) -> AudioRouterStatus {
        let sessions = self.sessions.read().await;
        let agents = self.agent_voices.read().await;

        AudioRouterStatus {
            active_users: sessions.len(),
            active_agents: agents.len(),
            users_with_ptt: sessions.values().filter(|s| s.ptt_active).count(),
            spatial_agents: agents.values().filter(|a| a.public_voice).count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRouterStatus {
    pub active_users: usize,
    pub active_agents: usize,
    pub users_with_ptt: usize,
    pub spatial_agents: usize,
}
