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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
    System,
    Tool,
}

impl std::fmt::Display for ChatRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatRole::User => write!(f, "user"),
            ChatRole::Assistant => write!(f, "assistant"),
            ChatRole::System => write!(f, "system"),
            ChatRole::Tool => write!(f, "tool"),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<Vec<serde_json::Value>>,
}

impl Default for EnhancedChatMessage {
    fn default() -> Self {
        Self {
            role: ChatRole::User,
            content: String::new(),
            timestamp: chrono::Utc::now(),
            tool_calls: None,
            tool_results: None,
        }
    }
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

    // Test helper methods
    pub fn new_user_message(content: &str) -> Self {
        Self::new(MessageType::User, content.to_string())
    }

    pub fn new_arula_message(content: &str) -> Self {
        Self::new(MessageType::Arula, content.to_string())
    }

    pub fn new_system_message(content: &str) -> Self {
        Self::new(MessageType::System, content.to_string())
    }

    pub fn new_error_message(content: &str) -> Self {
        Self::new(MessageType::Error, content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, Utc};
    use serde_json;

    #[test]
    fn test_message_type_display() {
        assert_eq!(MessageType::User.to_string(), "user");
        assert_eq!(MessageType::Arula.to_string(), "arula");
        assert_eq!(MessageType::System.to_string(), "system");
        assert_eq!(MessageType::Success.to_string(), "success");
        assert_eq!(MessageType::Error.to_string(), "error");
        assert_eq!(MessageType::Info.to_string(), "info");
        assert_eq!(MessageType::ToolCall.to_string(), "tool_call");
        assert_eq!(MessageType::ToolResult.to_string(), "tool_result");
    }

    #[test]
    fn test_chat_role_display() {
        assert_eq!(ChatRole::User.to_string(), "user");
        assert_eq!(ChatRole::Assistant.to_string(), "assistant");
        assert_eq!(ChatRole::System.to_string(), "system");
        assert_eq!(ChatRole::Tool.to_string(), "tool");
    }

    #[test]
    fn test_message_type_equality() {
        assert_eq!(MessageType::User, MessageType::User);
        assert_ne!(MessageType::User, MessageType::Arula);
        assert_ne!(MessageType::System, MessageType::Error);
    }

    #[test]
    fn test_chat_role_equality() {
        assert_eq!(ChatRole::User, ChatRole::User);
        assert_ne!(ChatRole::User, ChatRole::Assistant);
        assert_ne!(ChatRole::System, ChatRole::Tool);
    }

    #[test]
    fn test_chat_message_new() {
        let message = ChatMessage::new(MessageType::User, "Hello, world!".to_string());

        assert_eq!(message.message_type, MessageType::User);
        assert_eq!(message.content, "Hello, world!");
        assert!(message.tool_call_json.is_none());
        assert!(message.timestamp <= Local::now());
    }

    #[test]
    fn test_chat_message_new_tool_call() {
        let tool_json = r#"{"name": "bash_tool", "arguments": "ls -la"}"#;
        let message = ChatMessage::new_tool_call("Executing command".to_string(), tool_json.to_string());

        assert_eq!(message.message_type, MessageType::ToolCall);
        assert_eq!(message.content, "Executing command");
        assert_eq!(message.tool_call_json, Some(tool_json.to_string()));
        assert!(message.timestamp <= Local::now());
    }

    #[test]
    fn test_chat_message_helper_methods() {
        let user_msg = ChatMessage::new_user_message("User input");
        assert_eq!(user_msg.message_type, MessageType::User);
        assert_eq!(user_msg.content, "User input");

        let arula_msg = ChatMessage::new_arula_message("AI response");
        assert_eq!(arula_msg.message_type, MessageType::Arula);
        assert_eq!(arula_msg.content, "AI response");

        let system_msg = ChatMessage::new_system_message("System info");
        assert_eq!(system_msg.message_type, MessageType::System);
        assert_eq!(system_msg.content, "System info");

        let error_msg = ChatMessage::new_error_message("Error occurred");
        assert_eq!(error_msg.message_type, MessageType::Error);
        assert_eq!(error_msg.content, "Error occurred");
    }

    #[test]
    fn test_enhanced_chat_message_default() {
        let message = EnhancedChatMessage::default();

        assert_eq!(message.role, ChatRole::User);
        assert_eq!(message.content, "");
        assert!(message.tool_calls.is_none());
        assert!(message.tool_results.is_none());
        assert!(message.timestamp <= Utc::now());
    }

    #[test]
    fn test_enhanced_chat_message_manual_creation() {
        let tool_calls = vec![serde_json::json!({
            "name": "bash_tool",
            "arguments": "echo hello"
        })];

        let tool_results = vec![serde_json::json!({
            "success": true,
            "output": "hello"
        })];

        let timestamp = Utc::now();
        let message = EnhancedChatMessage {
            role: ChatRole::Assistant,
            content: "I'll help you".to_string(),
            timestamp,
            tool_calls: Some(tool_calls.clone()),
            tool_results: Some(tool_results.clone()),
        };

        assert_eq!(message.role, ChatRole::Assistant);
        assert_eq!(message.content, "I'll help you");
        assert_eq!(message.timestamp, timestamp);
        assert_eq!(message.tool_calls, Some(tool_calls));
        assert_eq!(message.tool_results, Some(tool_results));
    }

    #[test]
    fn test_chat_message_serialization() {
        let message = ChatMessage::new_user_message("Test message");

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(message.message_type, deserialized.message_type);
        assert_eq!(message.content, deserialized.content);
        assert_eq!(message.tool_call_json, deserialized.tool_call_json);
    }

    #[test]
    fn test_chat_message_with_tool_call_serialization() {
        let tool_json = r#"{"name": "read_file", "arguments": "{\"path\": \"test.txt\"}"}"#;
        let message = ChatMessage::new_tool_call("Reading file".to_string(), tool_json.to_string());

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.message_type, MessageType::ToolCall);
        assert_eq!(deserialized.content, "Reading file");
        assert_eq!(deserialized.tool_call_json, Some(tool_json.to_string()));
    }

    #[test]
    fn test_enhanced_chat_message_serialization() {
        let message = EnhancedChatMessage {
            role: ChatRole::Assistant,
            content: "Hello!".to_string(),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_results: None,
        };

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: EnhancedChatMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(message.role, deserialized.role);
        assert_eq!(message.content, deserialized.content);
        assert_eq!(message.tool_calls, deserialized.tool_calls);
        assert_eq!(message.tool_results, deserialized.tool_results);
    }

    #[test]
    fn test_enhanced_chat_message_with_tool_calls_serialization() {
        let tool_calls = vec![serde_json::json!({
            "name": "write_file",
            "arguments": "{\"path\": \"output.txt\", \"content\": \"Hello World\"}"
        })];

        let message = EnhancedChatMessage {
            role: ChatRole::Assistant,
            content: "Creating file".to_string(),
            timestamp: Utc::now(),
            tool_calls: Some(tool_calls),
            tool_results: None,
        };

        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: EnhancedChatMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.role, ChatRole::Assistant);
        assert_eq!(deserialized.content, "Creating file");
        assert!(deserialized.tool_calls.is_some());
        assert_eq!(deserialized.tool_calls.unwrap().len(), 1);
        assert!(deserialized.tool_results.is_none());
    }

    #[test]
    fn test_message_type_debug_format() {
        let user_type = MessageType::User;
        let debug_str = format!("{:?}", user_type);
        assert_eq!(debug_str, "User");

        let tool_call_type = MessageType::ToolCall;
        let debug_str = format!("{:?}", tool_call_type);
        assert_eq!(debug_str, "ToolCall");
    }

    #[test]
    fn test_chat_role_debug_format() {
        let user_role = ChatRole::User;
        let debug_str = format!("{:?}", user_role);
        assert_eq!(debug_str, "User");

        let assistant_role = ChatRole::Assistant;
        let debug_str = format!("{:?}", assistant_role);
        assert_eq!(debug_str, "Assistant");
    }

    #[test]
    fn test_chat_message_debug_format() {
        let message = ChatMessage::new_user_message("Test debug");
        let debug_str = format!("{:?}", message);

        assert!(debug_str.contains("ChatMessage"));
        assert!(debug_str.contains("Test debug"));
    }

    #[test]
    fn test_enhanced_chat_message_debug_format() {
        let message = EnhancedChatMessage {
            role: ChatRole::Tool,
            content: "Tool result".to_string(),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_results: None,
        };

        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("EnhancedChatMessage"));
        assert!(debug_str.contains("Tool"));
        assert!(debug_str.contains("Tool result"));
    }

    #[test]
    fn test_chat_message_clone() {
        let original = ChatMessage::new_tool_call("Command".to_string(), "{}".to_string());
        let cloned = original.clone();

        assert_eq!(original.message_type, cloned.message_type);
        assert_eq!(original.content, cloned.content);
        assert_eq!(original.tool_call_json, cloned.tool_call_json);
        // Note: timestamps might differ slightly due to Local::now()
    }

    #[test]
    fn test_enhanced_chat_message_clone() {
        let tool_calls = vec![serde_json::json!({"test": "data"})];
        let original = EnhancedChatMessage {
            role: ChatRole::Assistant,
            content: "Test".to_string(),
            timestamp: Utc::now(),
            tool_calls: Some(tool_calls.clone()),
            tool_results: None,
        };

        let cloned = original.clone();
        assert_eq!(original.role, cloned.role);
        assert_eq!(original.content, cloned.content);
        assert_eq!(original.timestamp, cloned.timestamp);
        assert_eq!(original.tool_calls, cloned.tool_calls);
    }

    #[test]
    fn test_edge_cases() {
        // Test with empty strings
        let empty_message = ChatMessage::new(MessageType::User, "".to_string());
        assert_eq!(empty_message.content, "");

        let empty_enhanced = EnhancedChatMessage {
            role: ChatRole::System,
            content: "".to_string(),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_results: None,
        };
        assert_eq!(empty_enhanced.content, "");

        // Test with very long content
        let long_content = "x".repeat(10000);
        let long_message = ChatMessage::new(MessageType::User, long_content.clone());
        assert_eq!(long_message.content.len(), 10000);

        // Should serialize and deserialize correctly
        let serialized = serde_json::to_string(&long_message).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.content.len(), 10000);
    }

    #[test]
    fn test_message_type_all_variants() {
        // Ensure all variants can be created and displayed
        let types = vec![
            MessageType::User,
            MessageType::Arula,
            MessageType::System,
            MessageType::Success,
            MessageType::Error,
            MessageType::Info,
            MessageType::ToolCall,
            MessageType::ToolResult,
        ];

        for msg_type in types {
            let message = ChatMessage::new(msg_type.clone(), "test".to_string());
            assert_eq!(message.message_type, msg_type);

            let display_str = msg_type.to_string();
            assert!(!display_str.is_empty());
        }
    }

    #[test]
    fn test_chat_role_all_variants() {
        // Ensure all roles can be created and displayed
        let roles = vec![
            ChatRole::User,
            ChatRole::Assistant,
            ChatRole::System,
            ChatRole::Tool,
        ];

        for role in roles {
            let message = EnhancedChatMessage {
                role: role.clone(),
                content: "test".to_string(),
                timestamp: Utc::now(),
                tool_calls: None,
                tool_results: None,
            };
            assert_eq!(message.role, role);

            let display_str = role.to_string();
            assert!(!display_str.is_empty());
        }
    }

    #[test]
    fn test_complex_tool_scenarios() {
        // Test message with multiple tool calls
        let tool_calls = vec![
            serde_json::json!({
                "name": "read_file",
                "arguments": "{\"path\": \"config.txt\"}"
            }),
            serde_json::json!({
                "name": "write_file",
                "arguments": "{\"path\": \"output.txt\", \"content\": \"Hello\"}"
            }),
        ];

        let tool_results = vec![
            serde_json::json!({
                "tool_call_id": "call_1",
                "success": true,
                "result": "file content here"
            }),
            serde_json::json!({
                "tool_call_id": "call_2",
                "success": true,
                "result": "file written"
            }),
        ];

        let message = EnhancedChatMessage {
            role: ChatRole::Assistant,
            content: "I'll help with both operations".to_string(),
            timestamp: Utc::now(),
            tool_calls: Some(tool_calls),
            tool_results: Some(tool_results),
        };

        // Serialize and verify
        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: EnhancedChatMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.role, ChatRole::Assistant);
        assert_eq!(deserialized.content, "I'll help with both operations");

        let deserialized_calls = deserialized.tool_calls.unwrap();
        assert_eq!(deserialized_calls.len(), 2);

        let deserialized_results = deserialized.tool_results.unwrap();
        assert_eq!(deserialized_results.len(), 2);
    }

    #[test]
    fn test_yaml_serialization() {
        let message = ChatMessage::new_user_message("YAML test");

        let yaml_str = serde_yaml::to_string(&message).unwrap();
        let deserialized: ChatMessage = serde_yaml::from_str(&yaml_str).unwrap();

        assert_eq!(message.message_type, deserialized.message_type);
        assert_eq!(message.content, deserialized.content);
    }

    #[test]
    fn test_json_roundtrip_consistency() {
        let original = EnhancedChatMessage {
            role: ChatRole::Tool,
            content: "Tool execution result".to_string(),
            timestamp: Utc::now(),
            tool_calls: Some(vec![serde_json::json!({"command": "ls"})]),
            tool_results: Some(vec![serde_json::json!({"output": "file1.txt\nfile2.txt"})]),
        };

        // JSON roundtrip
        let json_str = serde_json::to_string(&original).unwrap();
        let json_deserialized: EnhancedChatMessage = serde_json::from_str(&json_str).unwrap();

        assert_eq!(original.role, json_deserialized.role);
        assert_eq!(original.content, json_deserialized.content);
        assert_eq!(original.tool_calls, json_deserialized.tool_calls);
        assert_eq!(original.tool_results, json_deserialized.tool_results);
    }
}
