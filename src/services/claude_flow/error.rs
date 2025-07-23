use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConnectorError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Transport error: {0}")]
    Transport(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("HTTP error: {0}")]
    Http(String),
    
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
    
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("Timeout error")]
    Timeout,
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("MCP error: code={code}, message={message}")]
    McpError { code: i32, message: String },
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, ConnectorError>;