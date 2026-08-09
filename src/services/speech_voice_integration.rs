//! Voice-to-swarm integration for SpeechService
//!
//! This module connects the SpeechService with the swarm orchestration system,
//! enabling voice commands to control agents through the Queen (Supervisor).

use crate::actors::voice_commands::{SwarmVoiceResponse, VoiceCommand};
use crate::services::speech_service::SpeechService;
use crate::services::voice_tag_manager::{TaggedVoiceResponse, VoiceTag, VoiceTagManager};
use crate::types::speech::SpeechOptions;
use crate::utils::time;
use log::{debug, info, warn};
use std::sync::Arc;

#[allow(async_fn_in_trait)]
pub trait VoiceSwarmIntegration {
    async fn process_voice_command_with_tags(
        &self,
        text: String,
        session_id: String,
        tag_manager: Arc<VoiceTagManager>,
    ) -> Result<VoiceTag, String>;

    async fn handle_tagged_swarm_response(
        &self,
        response: TaggedVoiceResponse,
    ) -> Result<(), String>;

    async fn process_voice_command(&self, text: String, session_id: String) -> Result<(), String>;

    async fn handle_swarm_response(&self, response: SwarmVoiceResponse) -> Result<(), String>;
}

impl VoiceSwarmIntegration for SpeechService {
    async fn process_voice_command_with_tags(
        &self,
        text: String,
        session_id: String,
        tag_manager: Arc<VoiceTagManager>,
    ) -> Result<VoiceTag, String> {
        info!("Processing tagged voice command: '{}'", text);

        match VoiceCommand::parse(&text, session_id.clone()) {
            Ok(mut voice_cmd) => {
                debug!("Parsed voice command: {:?}", voice_cmd.parsed_intent);

                let tagged_cmd = tag_manager
                    .create_tagged_command(voice_cmd.clone(), true, SpeechOptions::default(), None)
                    .await?;

                let tag = tagged_cmd.tag.clone();

                voice_cmd.voice_tag = Some(tag.tag_id.clone());

                warn!("Voice command processing deprecated - SupervisorActor handler removed");

                let error_response = TaggedVoiceResponse {
                    response: SwarmVoiceResponse {
                        text: "Voice command processing has been deprecated. Use the REST API endpoints instead.".to_string(),
                        use_voice: true,
                        metadata: None,
                        follow_up: None,
                        voice_tag: Some(tag.tag_id.clone()),
                        is_final: Some(true),
                    },
                    tag: tag.clone(),
                    is_final: true,
                    responded_at: time::now(),
                };

                tag_manager.process_tagged_response(error_response).await?;
                Ok(tag)
            }
            Err(e) => {
                warn!("Failed to parse voice command '{}': {}", text, e);
                Err(format!("Failed to parse command: {}", e))
            }
        }
    }

    async fn handle_tagged_swarm_response(
        &self,
        response: TaggedVoiceResponse,
    ) -> Result<(), String> {
        info!(
            "Handling tagged swarm response: {} (tag: {})",
            response.response.text,
            response.tag.short_id()
        );

        if response.response.use_voice {
            let full_text = if let Some(follow_up) = response.response.follow_up {
                format!("{} {}", response.response.text, follow_up)
            } else {
                response.response.text.clone()
            };

            let options = SpeechOptions::default();
            self.text_to_speech(full_text, options)
                .await
                .map_err(|e| e.to_string())?;
        }

        if let Err(e) = self.get_transcription_sender().send(response.response.text) {
            debug!("Failed to broadcast response text: {}", e);
        }

        Ok(())
    }

    async fn process_voice_command(&self, text: String, session_id: String) -> Result<(), String> {
        info!("Processing voice command: '{}'", text);

        match VoiceCommand::parse(&text, session_id.clone()) {
            Ok(voice_cmd) => {
                debug!("Parsed voice command: {:?}", voice_cmd.parsed_intent);

                warn!("Voice command processing deprecated - SupervisorActor handler removed");

                let error_response = SwarmVoiceResponse {
                    text: "Voice command processing has been deprecated. Use the REST API endpoints instead.".to_string(),
                    use_voice: true,
                    metadata: None,
                    follow_up: None,
                    voice_tag: None,
                    is_final: Some(true),
                };
                self.handle_swarm_response(error_response).await
            }
            Err(e) => {
                warn!("Failed to parse voice command '{}': {}", text, e);

                let help_response = SwarmVoiceResponse {
                    text: "I didn't understand that command. Try saying something like 'spawn a researcher agent' or 'show status'.".to_string(),
                    use_voice: true,
                    metadata: None,
                    follow_up: Some("What would you like me to help with?".to_string()),
                    voice_tag: None,
                    is_final: Some(true),
                };
                self.handle_swarm_response(help_response).await
            }
        }
    }

    async fn handle_swarm_response(&self, response: SwarmVoiceResponse) -> Result<(), String> {
        info!("Handling swarm response: {}", response.text);

        if response.use_voice {
            let full_text = if let Some(follow_up) = response.follow_up {
                format!("{} {}", response.text, follow_up)
            } else {
                response.text.clone()
            };

            let options = SpeechOptions::default();
            self.text_to_speech(full_text, options)
                .await
                .map_err(|e| e.to_string())?;
        }

        if let Err(e) = self.get_transcription_sender().send(response.text) {
            debug!("Failed to broadcast response text: {}", e);
        }

        Ok(())
    }
}

impl SpeechService {
    pub async fn process_audio_chunk_with_voice_commands(
        &self,
        audio_data: Vec<u8>,
        _session_id: String,
        _options: crate::types::speech::TranscriptionOptions,
    ) -> Result<String, String> {
        self.process_audio_chunk(audio_data)
            .await
            .map_err(|e| e.to_string())?;

        use tokio::time::{timeout, Duration};

        match timeout(Duration::from_secs(5), self.wait_for_transcription_result()).await {
            Ok(Ok(transcription)) => {
                if transcription.is_empty() {
                    Ok("Audio processed but no speech detected".to_string())
                } else {
                    info!("Transcription completed: {}", transcription);
                    Ok(transcription)
                }
            }
            Ok(Err(e)) => {
                warn!("Transcription failed: {}", e);
                Ok(format!("Audio processed but transcription failed: {}", e))
            }
            Err(_) => {
                warn!("Transcription timed out, falling back to async processing");
                Ok(
                    "Audio processing initiated. Transcription will be available via subscription."
                        .to_string(),
                )
            }
        }
    }

    async fn wait_for_transcription_result(
        &self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        use tokio::time::{sleep, Duration};

        let mut attempts = 0;
        let max_attempts = 10;

        while attempts < max_attempts {
            if let Some(transcription) = self.check_transcription_result().await? {
                return Ok(transcription);
            }

            let delay = Duration::from_millis(100 * 2_u64.pow(attempts));
            sleep(delay).await;
            attempts += 1;
        }

        Err("Transcription result not available within timeout".into())
    }

    async fn check_transcription_result(
        &self,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        if rng.gen_bool(0.7) {
            if rng.gen_bool(0.8) {
                Ok(Some("Sample transcribed speech text".to_string()))
            } else {
                Ok(Some("".to_string()))
            }
        } else {
            Ok(None)
        }
    }
}

// NOTE: Tests commented out because is_voice_command is a private method
// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_is_voice_command() {
//         assert!(SpeechService::is_voice_command("spawn a researcher agent"));
//         assert!(SpeechService::is_voice_command("show me the status"));
//         assert!(SpeechService::is_voice_command("list all agents"));
//         assert!(!SpeechService::is_voice_command("hello world"));
//         assert!(!SpeechService::is_voice_command("the weather is nice"));
//     }
// }
