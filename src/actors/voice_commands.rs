//! Voice command integration for swarm orchestration
//!
//! This module provides voice-to-swarm command parsing and response formatting
//! with automatic preamble injection for voice-appropriate responses.

use actix::prelude::*;
use serde::{Deserialize, Serialize};
// use log::{info, debug, error};
use std::collections::HashMap;

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "Result<SwarmVoiceResponse, String>")]
pub struct VoiceCommand {
    pub raw_text: String,

    pub parsed_intent: SwarmIntent,

    pub context: Option<ConversationContext>,

    pub respond_via_voice: bool,

    pub session_id: String,

    pub voice_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmVoiceResponse {
    pub text: String,

    pub use_voice: bool,

    pub metadata: Option<HashMap<String, String>>,

    pub follow_up: Option<String>,

    pub voice_tag: Option<String>,

    pub is_final: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmIntent {
    SpawnAgent {
        agent_type: String,
        capabilities: Vec<String>,
    },
    QueryStatus {
        target: Option<String>,
    },
    ExecuteTask {
        description: String,
        priority: TaskPriority,
    },
    UpdateGraph {
        action: GraphAction,
    },
    ListAgents,
    StopAgent {
        agent_id: String,
    },
    Help,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphAction {
    AddNode { label: String },
    RemoveNode { id: String },
    AddEdge { from: String, to: String },
    Clear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    pub session_id: String,
    pub history: Vec<(String, String)>,
    pub current_agents: Vec<String>,
    pub pending_clarification: Option<String>,
    pub turn_count: usize,
}

pub struct VoicePreamble;

impl VoicePreamble {
    pub fn generate(intent: &SwarmIntent) -> String {
        let base_preamble =
            "[VOICE_MODE: Reply in 1-2 short sentences. Be conversational. No special chars.]";

        let intent_hint = match intent {
            SwarmIntent::SpawnAgent { .. } => " Confirm agent creation.",
            SwarmIntent::QueryStatus { .. } => " Summarize status briefly.",
            SwarmIntent::ExecuteTask { .. } => " Acknowledge task.",
            SwarmIntent::UpdateGraph { .. } => " Confirm graph change.",
            SwarmIntent::ListAgents => " List agents concisely.",
            SwarmIntent::StopAgent { .. } => " Confirm stopping.",
            SwarmIntent::Help => " Give brief help.",
        };

        format!("{}{}", base_preamble, intent_hint)
    }

    pub fn wrap_instruction(instruction: &str, intent: &SwarmIntent) -> String {
        format!("{}\n{}", Self::generate(intent), instruction)
    }
}

impl VoiceCommand {
    pub fn parse(text: &str, session_id: String) -> Result<Self, String> {
        let lower = text.to_lowercase();

        let parsed_intent = if lower.contains("add agent") || lower.contains("spawn") {
            let agent_type = Self::extract_agent_type(&lower)?;
            SwarmIntent::SpawnAgent {
                agent_type,
                capabilities: vec![],
            }
        } else if lower.contains("status") {
            let target = Self::extract_target(&lower);
            SwarmIntent::QueryStatus { target }
        } else if lower.contains("list agents") || lower.contains("show agents") {
            SwarmIntent::ListAgents
        } else if lower.contains("stop agent") || lower.contains("remove agent") {
            let agent_id = Self::extract_agent_id(&lower)?;
            SwarmIntent::StopAgent { agent_id }
        } else if lower.contains("add node") {
            let label = Self::extract_label(&lower)?;
            SwarmIntent::UpdateGraph {
                action: GraphAction::AddNode { label },
            }
        } else if lower.contains("help") {
            SwarmIntent::Help
        } else {
            SwarmIntent::ExecuteTask {
                description: text.to_string(),
                priority: TaskPriority::Medium,
            }
        };

        Ok(VoiceCommand {
            raw_text: text.to_string(),
            parsed_intent,
            context: None,
            respond_via_voice: true,
            session_id,
            voice_tag: None,
        })
    }

    fn extract_agent_type(text: &str) -> Result<String, String> {
        for agent in &["researcher", "coder", "analyst", "coordinator", "optimizer"] {
            if text.contains(agent) {
                return Ok(agent.to_string());
            }
        }

        if let Some(pos) = text.find("agent ") {
            let after = &text[pos + 6..];
            if let Some(word) = after.split_whitespace().next() {
                return Ok(word.to_string());
            }
        }

        Err("Could not determine agent type".to_string())
    }

    fn extract_target(text: &str) -> Option<String> {
        if text.contains("all") {
            return Some("all".to_string());
        }

        for agent in &["researcher", "coder", "analyst", "coordinator"] {
            if text.contains(agent) {
                return Some(agent.to_string());
            }
        }

        None
    }

    fn extract_agent_id(text: &str) -> Result<String, String> {
        Self::extract_agent_type(text)
    }

    fn extract_label(text: &str) -> Result<String, String> {
        for keyword in &["called", "named", "label", "with"] {
            if let Some(pos) = text.find(keyword) {
                let after = &text[pos + keyword.len()..].trim();
                if let Some(label) = after.split_whitespace().next() {
                    return Ok(label.to_string());
                }
            }
        }

        Ok("node".to_string())
    }

    pub fn format_response(response: &str) -> SwarmVoiceResponse {
        let cleaned = response
            .replace("```", "")
            .replace("**", "")
            .replace("__", "")
            .replace("##", "")
            .replace("- ", "")
            .replace("* ", "");

        let text = if cleaned.len() > 200 {
            format!("{}...", &cleaned[..197])
        } else {
            cleaned
        };

        SwarmVoiceResponse {
            text,
            use_voice: true,
            metadata: None,
            follow_up: None,
            voice_tag: None,
            is_final: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spawn_agent() {
        let cmd = VoiceCommand::parse("spawn a researcher agent", "test".to_string()).unwrap();
        match cmd.parsed_intent {
            SwarmIntent::SpawnAgent { agent_type, .. } => {
                assert_eq!(agent_type, "researcher");
            }
            _ => panic!("Wrong intent"),
        }
    }

    #[test]
    fn test_parse_status_query() {
        let cmd =
            VoiceCommand::parse("what's the status of all agents", "test".to_string()).unwrap();
        match cmd.parsed_intent {
            SwarmIntent::QueryStatus { target } => {
                assert_eq!(target, Some("all".to_string()));
            }
            _ => panic!("Wrong intent"),
        }
    }

    #[test]
    fn test_voice_preamble() {
        let intent = SwarmIntent::SpawnAgent {
            agent_type: "researcher".to_string(),
            capabilities: vec![],
        };
        let preamble = VoicePreamble::generate(&intent);
        assert!(preamble.contains("VOICE_MODE"));
        assert!(preamble.contains("Confirm agent creation"));
    }

    #[test]
    fn test_wrap_instruction() {
        let intent = SwarmIntent::QueryStatus { target: None };
        let wrapped = VoicePreamble::wrap_instruction("Get system status", &intent);
        assert!(wrapped.starts_with("[VOICE_MODE"));
        assert!(wrapped.contains("Get system status"));
    }
}
