// src/models/ragflow_chat.rs
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RagflowChatRequest {
    pub question: String,
    pub session_id: Option<String>,
    pub stream: Option<bool>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RagflowChatResponse {
    pub answer: String,
    pub session_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ragflow_chat_request_deserialize_minimal() {
        let json = r#"{"question":"What is AI?"}"#;
        let req: RagflowChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.question, "What is AI?");
        assert!(req.session_id.is_none());
        assert!(req.stream.is_none());
    }

    #[test]
    fn ragflow_chat_request_deserialize_full() {
        let json = r#"{"question":"Tell me more","sessionId":"ses-1","stream":true}"#;
        let req: RagflowChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.question, "Tell me more");
        assert_eq!(req.session_id.as_deref(), Some("ses-1"));
        assert_eq!(req.stream, Some(true));
    }

    #[test]
    fn ragflow_chat_response_serializes_with_camel_case() {
        let resp = RagflowChatResponse {
            answer: "42".to_string(),
            session_id: "sess-abc".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            json.contains("sessionId"),
            "expected camelCase key, got: {}",
            json
        );
        assert!(json.contains("\"42\""));
    }
}
