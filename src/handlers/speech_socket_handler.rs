use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use log::{debug, error, info};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::app_state::AppState;
use crate::actors::messages::GetSettings;
use crate::types::speech::SpeechOptions;
use tokio::sync::broadcast;
use futures::FutureExt;

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
struct SetProviderRequest {
    provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct STTActionRequest {
    action: String, // "start" or "stop"
    language: Option<String>,
    model: Option<String>,
}

pub struct SpeechSocket {
    id: String,
    app_state: Arc<AppState>,
    heartbeat: Instant,
    audio_rx: Option<broadcast::Receiver<Vec<u8>>>,
    transcription_rx: Option<broadcast::Receiver<String>>,
}

impl SpeechSocket {
    pub fn new(id: String, app_state: Arc<AppState>) -> Self {
        let (audio_rx, transcription_rx) = if let Some(speech_service) = &app_state.speech_service {
            (
                Some(speech_service.subscribe_to_audio()),
                Some(speech_service.subscribe_to_transcriptions())
            )
        } else {
            (None, None)
        };

        Self {
            id,
            app_state,
            heartbeat: Instant::now(),
            audio_rx,
            transcription_rx,
        }
    }

    // Helper method to handle heartbeat
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

    // Process text-to-speech request
    async fn process_tts_request(app_state: Arc<AppState>, req: TextToSpeechRequest) -> Result<(), String> {
        if let Some(speech_service) = &app_state.speech_service {
            // Get default settings from app state, handling optional Kokoro settings
            let settings = app_state.settings_addr.send(GetSettings).await
                .map_err(|e| format!("Settings actor mailbox error: {}", e))?
                .map_err(|e| format!("Failed to get settings: {}", e))?;
            let kokoro_config = settings.kokoro.as_ref(); // Get Option<&KokoroSettings>

            // Provide defaults if Kokoro config or specific fields are None
            let default_voice = kokoro_config.and_then(|k| k.default_voice.clone()).unwrap_or_else(|| "default_voice_placeholder".to_string()); // Provide a sensible default
            let default_speed = kokoro_config.and_then(|k| k.default_speed).unwrap_or(1.0);
            let default_stream = kokoro_config.and_then(|k| k.stream).unwrap_or(true); // Default to streaming?

            // Create options with defaults or provided values
            let options = SpeechOptions {
                voice: req.voice.unwrap_or(default_voice),
                speed: req.speed.unwrap_or(default_speed),
                stream: req.stream.unwrap_or(default_stream),
            };

            // Send request to TTS service
            match speech_service.text_to_speech(req.text, options).await {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to process TTS request: {}", e)),
            }
        } else {
            Err("Speech service is not available".to_string())
        }
    }
}

impl Actor for SpeechSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("[SpeechSocket] Client connected: {}", self.id);

        // Start heartbeat
        self.start_heartbeat(ctx);

        // Send welcome message
        let welcome = json!({
            "type": "connected",
            "message": "Connected to speech service"
        });

        ctx.text(welcome.to_string());

        // Start listening for audio data
        if let Some(mut rx) = self.audio_rx.take() {
            let addr = ctx.address();

            ctx.spawn(Box::pin(async move {
                while let Ok(audio_data) = rx.recv().await {
                    // Send audio data to the client
                    if addr.try_send(AudioChunkMessage(audio_data)).is_err() {
                        break;
                    }
                }
            }.into_actor(self)));
        }

        // Start listening for transcription data
        if let Some(mut rx) = self.transcription_rx.take() {
            let addr = ctx.address();

            ctx.spawn(Box::pin(async move {
                while let Ok(transcription_text) = rx.recv().await {
                    // Send transcription to the client
                    if addr.try_send(TranscriptionMessage(transcription_text)).is_err() {
                        break;
                    }
                }
            }.into_actor(self)));
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
        // Send binary audio data to the client
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
        // Send transcription as JSON to the client
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
        // Send error message to the client
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
                debug!("[SpeechSocket] Received text: {}", text);
                self.heartbeat = Instant::now();

                // Parse the message
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(msg) => {
                        // Process based on message type
                        let msg_type = msg.get("type").and_then(|t| t.as_str());
                        match msg_type {
                            Some("tts") => {
                                // Parse as TextToSpeechRequest
                                if let Ok(tts_req) = serde_json::from_value::<TextToSpeechRequest>(msg) {
                                    // Process TTS request
                                    let app_state = self.app_state.clone();
                                    let addr = ctx.address();
                                    let fut = async move {
                                        if let Err(e) = Self::process_tts_request(app_state, tts_req).await {
                                            let error_msg = json!({
                                                "type": "error",
                                                "message": e
                                            });
                                            let _ = addr.try_send(ErrorMessage(error_msg.to_string()));
                                        }
                                    };
                                    ctx.spawn(fut.into_actor(self));
                                } else {
                                    ctx.text(json!({"type": "error", "message": "Invalid TTS request format"}).to_string());
                                }
                            }
                            Some("stt") => {
                                // Parse as STT action request
                                if let Ok(stt_req) = serde_json::from_value::<STTActionRequest>(msg) {
                                    match stt_req.action.as_str() {
                                        "start" => {
                                            if let Some(speech_service) = &self.app_state.speech_service {
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
                                                    match speech_service.start_transcription(options).await {
                                                        Ok(_) => {
                                                            let msg = json!({
                                                                "type": "stt_started",
                                                                "message": "Transcription started"
                                                            }).to_string();
                                                            let _ = addr.try_send(ErrorMessage(msg));
                                                        },
                                                        Err(e) => {
                                                            let msg = json!({
                                                                "type": "error",
                                                                "message": format!("Failed to start transcription: {}", e)
                                                            }).to_string();
                                                            let _ = addr.try_send(ErrorMessage(msg));
                                                        }
                                                    }
                                                };
                                                ctx.spawn(fut.into_actor(self));
                                            }
                                        },
                                        "stop" => {
                                            if let Some(speech_service) = &self.app_state.speech_service {
                                                let speech_service = speech_service.clone();
                                                let addr = ctx.address();
                                                let fut = async move {
                                                    match speech_service.stop_transcription().await {
                                                        Ok(_) => {
                                                            let msg = json!({
                                                                "type": "stt_stopped",
                                                                "message": "Transcription stopped"
                                                            }).to_string();
                                                            let _ = addr.try_send(ErrorMessage(msg));
                                                        },
                                                        Err(e) => {
                                                            let msg = json!({
                                                                "type": "error",
                                                                "message": format!("Failed to stop transcription: {}", e)
                                                            }).to_string();
                                                            let _ = addr.try_send(ErrorMessage(msg));
                                                        }
                                                    }
                                                };
                                                ctx.spawn(fut.into_actor(self));
                                            }
                                        },
                                        _ => {
                                            ctx.text(json!({"type": "error", "message": "Invalid STT action"}).to_string());
                                        }
                                    }
                                } else {
                                    ctx.text(json!({"type": "error", "message": "Invalid STT request format"}).to_string());
                                }
                            }
                            _ => {
                                ctx.text(json!({"type": "error", "message": "Unknown message type"}).to_string());
                            }
                        }
                    }
                    Err(e) => {
                        ctx.text(json!({"type": "error", "message": format!("Invalid JSON: {}", e)}).to_string());
                    }
                }
            }
            Ok(ws::Message::Binary(bin)) => {
                debug!("[SpeechSocket] Received binary audio data: {} bytes", bin.len());
                self.heartbeat = Instant::now();

                // Process audio chunk for STT
                if let Some(speech_service) = &self.app_state.speech_service {
                    let audio_data = bin.to_vec();

                    // Clone the speech service Arc to move into the future
                    let speech_service = speech_service.clone();
                    let fut = async move {
                        if let Err(e) = speech_service.process_audio_chunk(audio_data).await {
                            error!("Failed to process audio chunk: {}", e);
                        }
                    }.boxed().into_actor(self);

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
) -> Result<HttpResponse, Error> {
    let socket_id = format!("speech_{}", uuid::Uuid::new_v4());
    let socket = SpeechSocket::new(socket_id, app_state.into_inner());

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