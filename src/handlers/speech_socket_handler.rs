use crate::actors::messages::GetSettings;
use crate::app_state::AppState;
use crate::types::speech::SpeechOptions;
use actix::prelude::*;
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
// DEPRECATED: HybridHealthManager removed - use TaskOrchestratorActor instead
use futures::FutureExt;
use tokio::sync::broadcast;

// Constants for heartbeat
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

// Define message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextToSpeechRequest {
    text: String,
    voice: Option<String>,
    speed: Option<f32>,
    stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SetProviderRequest {
    provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct STTActionRequest {
    action: String,
    language: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoiceCommandRequest {
    text: String,
    session_id: Option<String>,
    respond_via_voice: Option<bool>,
    /// COM-15 / D6: the selected agent's `did:nostr` this command is addressed
    /// to. When present (and canonical), the command takes the governed voice
    /// path (signed 31402 → `/v1/voice-intent` → Kokoro ack) instead of the
    /// global settings assistant. Threaded from the client graph selection
    /// through the PTT-start binding.
    actor_did: Option<String>,
    /// V3 (PRD-023 WP-10): the STT confidence for this utterance, normalised to
    /// `[0, 1]`. When present and below the configured threshold, the governed
    /// path holds the command for a clarification turn instead of dispatching.
    /// Absent (`None`) means the STT layer reported no confidence — the command
    /// is not blocked on missing telemetry (the intent gate still applies).
    confidence: Option<f32>,
}

/// COM-15 / D6: the PTT-start binding message. Threads the selected agent's
/// `did:nostr` (from graph selection) onto this socket's `AudioRouter` session
/// so a following spoken command has a verifiable target.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetPttRequest {
    active: bool,
    actor_did: Option<String>,
}

/// V3 (PRD-023 WP-10): the outcome of the governed voice path's confidence gate.
enum GovernedVoiceResult {
    /// The gate passed (or the repair resolved) — the command dispatched; carries
    /// the spoken acknowledgement text.
    Dispatched(String),
    /// The gate held the utterance — the system spoke a clarification and did NOT
    /// dispatch; carries the clarification prompt and the ambiguous slot name.
    Clarified { prompt: String, slot: String },
}

pub struct SpeechSocket {
    id: String,
    app_state: Arc<AppState>,
    _hybrid_manager: Option<()>,
    heartbeat: Instant,
    audio_rx: Option<broadcast::Receiver<Vec<u8>>>,
    transcription_rx: Option<broadcast::Receiver<String>>,
}

impl SpeechSocket {
    pub fn new(id: String, app_state: Arc<AppState>, _hybrid_manager: Option<()>) -> Self {
        let (audio_rx, transcription_rx) = if let Some(speech_service) = &app_state.speech_service {
            (
                Some(speech_service.subscribe_to_audio()),
                Some(speech_service.subscribe_to_transcriptions()),
            )
        } else {
            (None, None)
        };

        Self {
            id,
            app_state,
            _hybrid_manager: None,
            heartbeat: Instant::now(),
            audio_rx,
            transcription_rx,
        }
    }

    fn start_heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                info!("SpeechSocket client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }

    async fn process_tts_request(
        app_state: Arc<AppState>,
        req: TextToSpeechRequest,
    ) -> Result<(), String> {
        if let Some(speech_service) = &app_state.speech_service {
            let settings = app_state
                .settings_addr
                .send(GetSettings)
                .await
                .map_err(|e| format!("Settings actor mailbox error: {}", e))?
                .map_err(|e| format!("Failed to get settings: {}", e))?;
            let kokoro_config = settings.kokoro.as_ref();

            let default_voice = kokoro_config
                .and_then(|k| k.default_voice.clone())
                .unwrap_or_else(|| "af_sarah".to_string());
            let default_speed = kokoro_config.and_then(|k| k.default_speed).unwrap_or(1.0);
            let default_stream = kokoro_config.and_then(|k| k.stream).unwrap_or(true);

            let options = SpeechOptions {
                voice: req.voice.unwrap_or(default_voice),
                speed: req.speed.unwrap_or(default_speed),
                stream: req.stream.unwrap_or(default_stream),
                format: "opus".to_string(),
            };

            match speech_service.text_to_speech(req.text, options).await {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to process TTS request: {}", e)),
            }
        } else {
            Err("Speech service is not available".to_string())
        }
    }

    /// COM-15 / V1 / D6 (PRD-023 WP-5): the governed voice path. A spoken
    /// command addressed to a selected agent's `did:nostr` is signed into a
    /// kind-31402 and POSTed to agentbox `/v1/voice-intent`; on accepted
    /// dispatch a Kokoro TTS acknowledgement plays and the standing
    /// `CANARY-VC-COM15-PTT` records a live fire. Returns the spoken ack text for
    /// the WS `voice_response`. `Err` lets the caller fall back to the settings
    /// assistant (never the settings assistant when the governed loop succeeds —
    /// the falsification statement's second clause).
    async fn process_voice_intent(
        app_state: Arc<AppState>,
        transcript: String,
        actor_did: String,
    ) -> Result<String, String> {
        let client = app_state
            .voice_intent_client
            .as_ref()
            .ok_or_else(|| "governed voice loop unconfigured".to_string())?;

        let accepted = client
            .dispatch(&transcript, &actor_did, 200)
            .await
            .map_err(|e| e.to_string())?;

        let ack = crate::services::voice_intent_client::ack_sentence(&accepted, &actor_did);

        // COM-15 AC3: speak the acknowledgement over the Kokoro TTS path.
        if let Some(speech_service) = &app_state.speech_service {
            if let Err(e) = speech_service
                .text_to_speech(ack.clone(), SpeechOptions::default())
                .await
            {
                error!("[SpeechSocket] governed-voice ack TTS failed: {}", e);
            }
        }

        // The full loop carried one utterance end to end → record a live fire on
        // the standing canary (observed traffic, never a synthetic probe).
        let evidence = format!(
            "voice→31402→/v1/voice-intent accepted (event {:?}, verb '{}') → Kokoro ack",
            accepted.event_id, accepted.intent.verb
        );
        if let Err(e) = app_state
            .liveness_harness
            .observe(
                crate::services::liveness_harness::CANARY_COM15_PTT,
                &evidence,
            )
            .await
        {
            error!(
                "[SpeechSocket] failed to record CANARY-VC-COM15-PTT fire: {}",
                e
            );
        }

        Ok(ack)
    }

    /// V3 (PRD-023 WP-10): the confidence gate in front of the governed voice
    /// dispatch. A low-confidence or under-specified spoken command is NOT
    /// dispatched; instead the system speaks a targeted clarification (naming
    /// what it heard and the ambiguous slot), holds a pending clarification for
    /// this session, and merges the operator's next utterance before it will
    /// dispatch. On the adequate/resolved path it delegates to
    /// [`Self::process_voice_intent`] with the (possibly merged) transcript.
    ///
    /// `session_key` scopes the pending-clarification state (the client
    /// `session_id`, or this socket's id when absent).
    async fn process_governed_voice(
        app_state: Arc<AppState>,
        session_key: String,
        transcript: String,
        actor_did: String,
        confidence: Option<f32>,
    ) -> Result<GovernedVoiceResult, String> {
        use crate::services::voice_clarification::{GateOutcome, PendingClarification};

        let gate = app_state.clarification_gate;

        // A repair turn if a clarification is pending for this session; else a
        // fresh evaluation. `take_*` reads-and-clears, so a resolved repair does
        // not leave stale pending state.
        let pending = app_state
            .voice_context_manager
            .take_pending_clarification(&session_key)
            .await
            .and_then(|tok| PendingClarification::from_token(&tok));

        let outcome = match &pending {
            Some(p) => gate.merge(p, &transcript, confidence),
            None => gate.evaluate(&transcript, confidence),
        };

        match outcome {
            GateOutcome::Clarify { prompt, pending } => {
                let slot = pending.slot.slot_name().to_string();
                // Hold the clarification for the next utterance — this is the
                // assignment of the previously-dead pending_clarification field.
                app_state
                    .voice_context_manager
                    .set_pending_clarification(&session_key, Some(pending.to_token()))
                    .await;

                // Speak the clarification over the Kokoro TTS path (the operator
                // hears what was misunderstood, and does not get a dispatch).
                if let Some(speech_service) = &app_state.speech_service {
                    if let Err(e) = speech_service
                        .text_to_speech(prompt.clone(), SpeechOptions::default())
                        .await
                    {
                        error!("[SpeechSocket] clarification TTS failed: {}", e);
                    }
                }

                // A clarification turn is observed on live traffic → record a
                // fire on the one-shot V3 canary (never a synthetic probe).
                let evidence =
                    format!("low-confidence/ambiguous utterance held for repair (slot {slot})");
                if let Err(e) = app_state
                    .liveness_harness
                    .observe(
                        crate::services::liveness_harness::CANARY_V3_REPAIR,
                        &evidence,
                    )
                    .await
                {
                    error!(
                        "[SpeechSocket] failed to record CANARY-VC-V3-REPAIR fire: {}",
                        e
                    );
                }

                Ok(GovernedVoiceResult::Clarified { prompt, slot })
            }
            GateOutcome::Dispatch { transcript } => {
                let ack =
                    SpeechSocket::process_voice_intent(app_state, transcript, actor_did).await?;
                Ok(GovernedVoiceResult::Dispatched(ack))
            }
        }
    }

    fn is_swarm_command(&self, text: &str) -> bool {
        let text_lower = text.to_lowercase();
        text_lower.contains("swarm")
            || text_lower.contains("spawn agents")
            || text_lower.contains("create hive")
            || text_lower.contains("start swarm")
            || text_lower.contains("stop swarm")
            || text_lower.contains("agent status")
            || text_lower.contains("docker hive")
    }

    fn handle_swarm_voice_command(&self, _text: &str, ctx: &mut ws::WebsocketContext<Self>) {
        let error_msg = json!({
            "type": "error",
            "message": "Swarm voice commands deprecated - use API endpoints instead"
        })
        .to_string();
        ctx.text(error_msg);
    }
}

impl Actor for SpeechSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("[SpeechSocket] Client connected: {}", self.id);

        self.start_heartbeat(ctx);

        let welcome = json!({
            "type": "connected",
            "message": "Connected to speech service"
        });

        ctx.text(welcome.to_string());

        if let Some(mut rx) = self.audio_rx.take() {
            let addr = ctx.address();

            ctx.spawn(Box::pin(
                async move {
                    while let Ok(audio_data) = rx.recv().await {
                        if addr.try_send(AudioChunkMessage(audio_data)).is_err() {
                            break;
                        }
                    }
                }
                .into_actor(self),
            ));
        }

        if let Some(mut rx) = self.transcription_rx.take() {
            let addr = ctx.address();

            ctx.spawn(Box::pin(
                async move {
                    while let Ok(transcription_text) = rx.recv().await {
                        if addr
                            .try_send(TranscriptionMessage(transcription_text))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                .into_actor(self),
            ));
        }
    }
}

// Message type for audio data
struct AudioChunkMessage(Vec<u8>);

impl Message for AudioChunkMessage {
    type Result = ();
}

impl Handler<AudioChunkMessage> for SpeechSocket {
    type Result = ();

    fn handle(&mut self, msg: AudioChunkMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.binary(msg.0);
    }
}

// Message type for transcription data
struct TranscriptionMessage(String);

impl Message for TranscriptionMessage {
    type Result = ();
}

impl Handler<TranscriptionMessage> for SpeechSocket {
    type Result = ();

    fn handle(&mut self, msg: TranscriptionMessage, ctx: &mut Self::Context) -> Self::Result {
        let message = json!({
            "type": "transcription",
            "data": {
                "text": msg.0,
                "isFinal": true,
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            }
        });
        ctx.text(message.to_string());
    }
}

// Message type for error data
struct ErrorMessage(String);

impl Message for ErrorMessage {
    type Result = ();
}

impl Handler<ErrorMessage> for SpeechSocket {
    type Result = ();

    fn handle(&mut self, msg: ErrorMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SpeechSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.heartbeat = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // Handle plain-text heartbeat before JSON parsing
                if text.trim() == "ping" {
                    self.heartbeat = Instant::now();
                    ctx.text("pong");
                    return;
                }
                debug!("[SpeechSocket] Received text: {}", text);
                self.heartbeat = Instant::now();

                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(msg) => {
                        let msg_type = msg.get("type").and_then(|t| t.as_str());
                        match msg_type {
                            Some("tts") => {
                                if let Ok(tts_req) =
                                    serde_json::from_value::<TextToSpeechRequest>(msg)
                                {
                                    let app_state = self.app_state.clone();
                                    let addr = ctx.address();
                                    let fut = async move {
                                        if let Err(e) =
                                            Self::process_tts_request(app_state, tts_req).await
                                        {
                                            let error_msg = json!({
                                                "type": "error",
                                                "message": e
                                            });
                                            let _ =
                                                addr.try_send(ErrorMessage(error_msg.to_string()));
                                        }
                                    };
                                    ctx.spawn(fut.into_actor(self));
                                } else {
                                    ctx.text(json!({"type": "error", "message": "Invalid TTS request format"}).to_string());
                                }
                            }
                            Some("stt") => {
                                if let Ok(stt_req) = serde_json::from_value::<STTActionRequest>(msg)
                                {
                                    match stt_req.action.as_str() {
                                        "start" => {
                                            if let Some(speech_service) =
                                                &self.app_state.speech_service
                                            {
                                                use crate::types::speech::TranscriptionOptions;
                                                let options = TranscriptionOptions {
                                                    language: stt_req.language,
                                                    model: stt_req.model,
                                                    temperature: None,
                                                    stream: true,
                                                };

                                                let speech_service = speech_service.clone();
                                                let addr = ctx.address();
                                                let fut = async move {
                                                    match speech_service
                                                        .start_transcription(options)
                                                        .await
                                                    {
                                                        Ok(_) => {
                                                            let msg = json!({
                                                                "type": "stt_started",
                                                                "message": "Transcription started"
                                                            })
                                                            .to_string();
                                                            let _ =
                                                                addr.try_send(ErrorMessage(msg));
                                                        }
                                                        Err(e) => {
                                                            let msg = json!({
                                                                "type": "error",
                                                                "message": format!("Failed to start transcription: {}", e)
                                                            }).to_string();
                                                            let _ =
                                                                addr.try_send(ErrorMessage(msg));
                                                        }
                                                    }
                                                };
                                                ctx.spawn(fut.into_actor(self));
                                            }
                                        }
                                        "stop" => {
                                            if let Some(speech_service) =
                                                &self.app_state.speech_service
                                            {
                                                let speech_service = speech_service.clone();
                                                let addr = ctx.address();
                                                let fut = async move {
                                                    match speech_service.stop_transcription().await
                                                    {
                                                        Ok(_) => {
                                                            let msg = json!({
                                                                "type": "stt_stopped",
                                                                "message": "Transcription stopped"
                                                            })
                                                            .to_string();
                                                            let _ =
                                                                addr.try_send(ErrorMessage(msg));
                                                        }
                                                        Err(e) => {
                                                            let msg = json!({
                                                                "type": "error",
                                                                "message": format!("Failed to stop transcription: {}", e)
                                                            }).to_string();
                                                            let _ =
                                                                addr.try_send(ErrorMessage(msg));
                                                        }
                                                    }
                                                };
                                                ctx.spawn(fut.into_actor(self));
                                            }
                                        }
                                        _ => {
                                            ctx.text(json!({"type": "error", "message": "Invalid STT action"}).to_string());
                                        }
                                    }
                                } else {
                                    ctx.text(json!({"type": "error", "message": "Invalid STT request format"}).to_string());
                                }
                            }
                            Some("set_ptt") => {
                                // COM-15 / D6: the PTT-start binding message threads
                                // the selected agent's did:nostr onto this socket's
                                // AudioRouter session (keyed by the socket id).
                                if let Ok(ptt_req) = serde_json::from_value::<SetPttRequest>(msg) {
                                    let app_state = self.app_state.clone();
                                    let user_id = self.id.clone();
                                    let fut = async move {
                                        let _ =
                                            app_state.audio_router.register_user(&user_id).await;
                                        app_state
                                            .audio_router
                                            .set_ptt_with_target(
                                                &user_id,
                                                ptt_req.active,
                                                ptt_req.actor_did,
                                            )
                                            .await;
                                    };
                                    ctx.spawn(fut.into_actor(self));
                                } else {
                                    ctx.text(json!({"type": "error", "message": "Invalid set_ptt request format"}).to_string());
                                }
                            }
                            Some("voice_command") => {
                                if let Ok(voice_req) =
                                    serde_json::from_value::<VoiceCommandRequest>(msg)
                                {
                                    // COM-15: a command addressed to a selected
                                    // agent's did:nostr takes the GOVERNED voice
                                    // path (signed 31402 → /v1/voice-intent → Kokoro
                                    // ack). Only an UNBOUND command reaches the
                                    // settings assistant — a bound command never
                                    // does (falsification clause 2). A malformed DID
                                    // is dropped here, not silently misrouted.
                                    let bound_did = voice_req.actor_did.clone().filter(|d| {
                                        crate::services::voice_intent_client::is_canonical_did(d)
                                    });
                                    if let Some(actor_did) = bound_did {
                                        let app_state = self.app_state.clone();
                                        let addr = ctx.address();
                                        let text = voice_req.text.clone();
                                        // V3: STT confidence + per-session key for
                                        // the clarification/repair gate.
                                        let confidence = voice_req.confidence;
                                        let session_key = voice_req
                                            .session_id
                                            .clone()
                                            .unwrap_or_else(|| self.id.clone());
                                        let fut = async move {
                                            match SpeechSocket::process_governed_voice(
                                                app_state.clone(),
                                                session_key,
                                                text.clone(),
                                                actor_did.clone(),
                                                confidence,
                                            )
                                            .await
                                            {
                                                Ok(GovernedVoiceResult::Dispatched(ack)) => {
                                                    let msg = json!({
                                                        "type": "voice_response",
                                                        "data": {
                                                            "text": ack,
                                                            "isFinal": true,
                                                            "governed": true,
                                                            "actorDid": actor_did,
                                                            "timestamp": std::time::SystemTime::now()
                                                                .duration_since(std::time::UNIX_EPOCH)
                                                                .unwrap_or_default()
                                                                .as_millis()
                                                        }
                                                    }).to_string();
                                                    let _ = addr.try_send(ErrorMessage(msg));
                                                }
                                                Ok(GovernedVoiceResult::Clarified {
                                                    prompt,
                                                    slot,
                                                }) => {
                                                    // V3: a clarification turn — the
                                                    // command is NOT dispatched; the
                                                    // operator is asked to repair.
                                                    let msg = json!({
                                                        "type": "voice_clarification",
                                                        "data": {
                                                            "text": prompt,
                                                            "slot": slot,
                                                            "isFinal": false,
                                                            "governed": true,
                                                            "actorDid": actor_did,
                                                            "timestamp": std::time::SystemTime::now()
                                                                .duration_since(std::time::UNIX_EPOCH)
                                                                .unwrap_or_default()
                                                                .as_millis()
                                                        }
                                                    }).to_string();
                                                    let _ = addr.try_send(ErrorMessage(msg));
                                                }
                                                Err(e) => {
                                                    // Honest failure: a bound command
                                                    // is NEVER re-routed to the
                                                    // settings assistant. The client
                                                    // is told the governed loop is
                                                    // unreachable/unconfigured.
                                                    let msg = json!({
                                                        "type": "voice_intent_error",
                                                        "message": format!("governed voice dispatch failed: {}", e),
                                                        "actorDid": actor_did
                                                    }).to_string();
                                                    let _ = addr.try_send(ErrorMessage(msg));
                                                }
                                            }
                                        };
                                        ctx.spawn(fut.into_actor(self));
                                    } else if self.is_swarm_command(&voice_req.text) {
                                        self.handle_swarm_voice_command(&voice_req.text, ctx);
                                    } else if let Some(speech_service) =
                                        &self.app_state.speech_service
                                    {
                                        let speech_service = speech_service.clone();
                                        let addr = ctx.address();
                                        let fut = async move {
                                            let session_id =
                                                voice_req.session_id.unwrap_or_else(|| {
                                                    uuid::Uuid::new_v4().to_string()
                                                });

                                            match speech_service
                                                .process_voice_command_with_tags(
                                                    voice_req.text.clone(),
                                                    session_id,
                                                )
                                                .await
                                            {
                                                Ok(response) => {
                                                    let msg = json!({
                                                        "type": "voice_response",
                                                        "data": {
                                                            "text": response,
                                                            "isFinal": true,
                                                            "timestamp": std::time::SystemTime::now()
                                                                .duration_since(std::time::UNIX_EPOCH)
                                                                .unwrap_or_default()
                                                                .as_millis()
                                                        }
                                                    }).to_string();
                                                    let _ = addr.try_send(ErrorMessage(msg));
                                                }
                                                Err(_) => {
                                                    match speech_service
                                                        .process_voice_command(voice_req.text)
                                                        .await
                                                    {
                                                        Ok(response) => {
                                                            let msg = json!({
                                                                "type": "voice_response",
                                                                "data": {
                                                                    "text": response,
                                                                    "isFinal": true,
                                                                    "timestamp": std::time::SystemTime::now()
                                                                        .duration_since(std::time::UNIX_EPOCH)
                                                                        .unwrap_or_default()
                                                                        .as_millis()
                                                                }
                                                            }).to_string();
                                                            let _ =
                                                                addr.try_send(ErrorMessage(msg));
                                                        }
                                                        Err(e) => {
                                                            let msg = json!({
                                                                "type": "error",
                                                                "message": format!("Voice command failed: {}", e)
                                                            }).to_string();
                                                            let _ =
                                                                addr.try_send(ErrorMessage(msg));
                                                        }
                                                    }
                                                }
                                            }
                                        };
                                        ctx.spawn(fut.into_actor(self));
                                    } else {
                                        ctx.text(json!({"type": "error", "message": "Speech service not available"}).to_string());
                                    }
                                } else {
                                    ctx.text(json!({"type": "error", "message": "Invalid voice command format"}).to_string());
                                }
                            }
                            _ => {
                                ctx.text(
                                    json!({"type": "error", "message": "Unknown message type"})
                                        .to_string(),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        ctx.text(
                            json!({"type": "error", "message": format!("Invalid JSON: {}", e)})
                                .to_string(),
                        );
                    }
                }
            }
            Ok(ws::Message::Binary(bin)) => {
                debug!(
                    "[SpeechSocket] Received binary audio data: {} bytes",
                    bin.len()
                );
                self.heartbeat = Instant::now();

                if let Some(speech_service) = &self.app_state.speech_service {
                    let audio_data = bin.to_vec();

                    let speech_service = speech_service.clone();
                    let fut = async move {
                        if let Err(e) = speech_service.process_audio_chunk(audio_data).await {
                            error!("Failed to process audio chunk: {}", e);
                        }
                    }
                    .boxed()
                    .into_actor(self);

                    ctx.spawn(fut);
                }
            }
            Ok(ws::Message::Close(reason)) => {
                info!("[SpeechSocket] Client disconnected: {}", self.id);
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

// Handler for the WebSocket route
pub async fn speech_socket_handler(
    req: HttpRequest,
    stream: web::Payload,
    app_state: web::Data<AppState>,
    _hybrid_manager: Option<()>,
) -> Result<HttpResponse, actix_web::Error> {
    // SECURITY: WebSocket token validation at upgrade time.
    // Extracts token from Authorization header or query string.
    // Currently allows but logs unauthenticated connections -- enforcement will come
    // when all clients send tokens.
    {
        let token = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|s| s.to_string())
            .or_else(|| {
                let query = req.query_string();
                url::form_urlencoded::parse(query.as_bytes())
                    .find(|(k, _)| k == "token")
                    .map(|(_, v)| v.to_string())
            });

        if token.as_deref().unwrap_or("").is_empty() {
            let client_ip = req
                .peer_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            log::warn!(
                "SECURITY: Rejected unauthenticated WebSocket upgrade on /ws/speech from {}",
                client_ip
            );
            return Ok(HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "Authentication required"})));
        }
    }

    let socket_id = format!("speech_{}", uuid::Uuid::new_v4());
    let socket = SpeechSocket::new(socket_id, app_state.into_inner(), None);

    match ws::start(socket, &req, stream) {
        Ok(response) => {
            info!("[SpeechSocket] WebSocket connection established");
            Ok(response)
        }
        Err(e) => {
            error!("[SpeechSocket] Failed to start WebSocket: {}", e);
            Err(e)
        }
    }
}
