use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageType {
    User,
    Arula,
    System,
    Success,
    Error,
    Info,
}

impl std::fmt::Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageType::User => write!(f, "user"),
            MessageType::Arula => write!(f, "arula"),
            MessageType::System => write!(f, "system"),
            MessageType::Success => write!(f, "success"),
            MessageType::Error => write!(f, "error"),
            MessageType::Info => write!(f, "info"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub timestamp: DateTime<Local>,
    pub message_type: MessageType,
    pub content: String,
}

impl ChatMessage {
    #[allow(dead_code)]
    pub fn new(message_type: MessageType, content: String) -> Self {
        Self {
            timestamp: Local::now(),
            message_type,
            content,
        }
    }
}