use crate::utils::time;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMessage {
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

static MESSAGE_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_message_regex() -> &'static Regex {
    MESSAGE_REGEX.get_or_init(|| {
        Regex::new(
            r"(?xs)
            \*{0,2}\s*\[(?:CLIENT_)?MESSAGE\]\s*\*{0,2}  # Opening tag with optional ** and whitespace
            \s*
            (.*?)                                          # Capture content (non-greedy, . matches newlines due to s flag)
            \s*
            \*{0,2}\s*\[/(?:CLIENT_)?MESSAGE\]\s*\*{0,2}  # Closing tag with optional ** and whitespace
            "
        )
        .expect("Invalid regex pattern")
    })
}

pub fn extract_client_messages(
    text: &str,
    session_id: Option<String>,
    agent_id: Option<String>,
) -> Vec<ClientMessage> {
    let regex = get_message_regex();
    let timestamp = time::now();

    regex
        .captures_iter(text)
        .filter_map(|cap| {
            cap.get(1).map(|m| ClientMessage {
                content: m.as_str().trim().to_string(),
                timestamp,
                session_id: session_id.clone(),
                agent_id: agent_id.clone(),
            })
        })
        .collect()
}

pub struct ClientMessageStream {
    buffer: String,
    session_id: Option<String>,
    agent_id: Option<String>,
    max_buffer_size: usize,
}

impl ClientMessageStream {
    pub fn new(session_id: Option<String>, agent_id: Option<String>) -> Self {
        Self {
            buffer: String::new(),
            session_id,
            agent_id,
            max_buffer_size: 100_000,
        }
    }

    pub fn process_chunk(&mut self, chunk: &str) -> Vec<ClientMessage> {
        self.buffer.push_str(chunk);

        let messages =
            extract_client_messages(&self.buffer, self.session_id.clone(), self.agent_id.clone());

        if !messages.is_empty() {
            let regex = get_message_regex();
            self.buffer = regex.replace_all(&self.buffer, "").to_string();
        }

        if self.buffer.len() > self.max_buffer_size {
            let start_idx = self.buffer.len() - (self.max_buffer_size / 2);
            self.buffer = self.buffer[start_idx..].to_string();
        }

        messages
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_basic_message() {
        let text =
            "Some output **[CLIENT_MESSAGE]** Hello from agent **[/CLIENT_MESSAGE]** more output";
        let messages = extract_client_messages(text, None, None);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello from agent");
    }

    #[test]
    fn test_extract_multiple_messages() {
        let text = r#"
            **[CLIENT_MESSAGE]** First message **[/CLIENT_MESSAGE]**
            Some other text
            **[CLIENT_MESSAGE]** Second message **[/CLIENT_MESSAGE]**
        "#;
        let messages = extract_client_messages(text, None, None);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "First message");
        assert_eq!(messages[1].content, "Second message");
    }

    #[test]
    fn test_extract_without_asterisks() {
        let text = "[CLIENT_MESSAGE] Simple message [/CLIENT_MESSAGE]";
        let messages = extract_client_messages(text, None, None);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Simple message");
    }

    #[test]
    fn test_extract_with_session_info() {
        let text = "**[CLIENT_MESSAGE]** Test **[/CLIENT_MESSAGE]**";
        let messages = extract_client_messages(
            text,
            Some("session-123".to_string()),
            Some("agent-456".to_string()),
        );
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].session_id, Some("session-123".to_string()));
        assert_eq!(messages[0].agent_id, Some("agent-456".to_string()));
    }

    #[test]
    fn test_stream_processor() {
        let mut stream = ClientMessageStream::new(None, None);

        let messages = stream.process_chunk("**[CLIENT_MESSAGE]** Start of mes");
        assert_eq!(messages.len(), 0);

        let messages = stream.process_chunk("sage **[/CLIENT_MESSAGE]**");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Start of message");
    }

    #[test]
    fn test_short_form_message() {
        let text = "[MESSAGE] Quick update [/MESSAGE]";
        let messages = extract_client_messages(text, None, None);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Quick update");
    }

    #[test]
    fn test_multiline_message() {
        let text = r#"**[CLIENT_MESSAGE]**
        This is a multiline message
        with several lines of content
        **[/CLIENT_MESSAGE]**"#;
        let messages = extract_client_messages(text, None, None);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].content.contains("multiline message"));
    }

    #[test]
    fn test_no_messages() {
        let text = "Just regular output with no special markers";
        let messages = extract_client_messages(text, None, None);
        assert_eq!(messages.len(), 0);
    }
}
