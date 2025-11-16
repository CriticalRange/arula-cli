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
    ToolCall,   // For displaying tool call boxes
    ToolResult, // For displaying tool execution results
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
            MessageType::ToolCall => write!(f, "tool_call"),
            MessageType::ToolResult => write!(f, "tool_result"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub timestamp: DateTime<Local>,
    pub message_type: MessageType,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_json: Option<String>, // Store the raw JSON for tool calls
}

impl ChatMessage {
    #[allow(dead_code)]
    pub fn new(message_type: MessageType, content: String) -> Self {
        Self {
            timestamp: Local::now(),
            message_type,
            content,
            tool_call_json: None,
        }
    }

    pub fn new_tool_call(content: String, tool_call_json: String) -> Self {
        Self {
            timestamp: Local::now(),
            message_type: MessageType::ToolCall,
            content,
            tool_call_json: Some(tool_call_json),
        }
    }
}
