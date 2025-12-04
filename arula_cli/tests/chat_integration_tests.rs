//! Integration tests for the chat module

use arula_cli::chat::{
    ChatMessage, EnhancedChatMessage, MessageType, ChatRole
};
use serde_json;
use chrono::{Local, Utc};

#[test]
fn test_chat_message_lifecycle() {
    // Create a user message
    let mut message = ChatMessage::new_user_message("Hello, ARULA!");

    assert_eq!(message.message_type, MessageType::User);
    assert_eq!(message.content, "Hello, ARULA!");
    assert!(message.tool_call_json.is_none());
    assert!(message.timestamp <= Local::now());

    // Convert to tool call
    let tool_call_json = r#"{"name": "bash_tool", "arguments": "echo hello"}"#;
    message.tool_call_json = Some(tool_call_json.to_string());
    message.message_type = MessageType::ToolCall;

    assert_eq!(message.message_type, MessageType::ToolCall);
    assert_eq!(message.tool_call_json, Some(tool_call_json.to_string()));
}

#[test]
fn test_enhanced_chat_message_workflow() {
    // Create user message
    let user_message = EnhancedChatMessage {
        role: ChatRole::User,
        content: "List files in current directory".to_string(),
        timestamp: Utc::now(),
        tool_calls: None,
        tool_results: None,
    };

    // Simulate AI response with tool calls
    let ai_response = EnhancedChatMessage {
        role: ChatRole::Assistant,
        content: "I'll list the files for you".to_string(),
        timestamp: Utc::now(),
        tool_calls: Some(vec![
            serde_json::json!({
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "bash_tool",
                    "arguments": "{\"command\": \"ls -la\"}"
                }
            })
        ]),
        tool_results: None,
    };

    // Simulate tool result
    let tool_result = EnhancedChatMessage {
        role: ChatRole::Tool,
        content: "Tool execution completed".to_string(),
        timestamp: Utc::now(),
        tool_calls: None,
        tool_results: Some(vec![
            serde_json::json!({
                "tool_call_id": "call_1",
                "success": true,
                "result": "total 8\ndrwxr-xr-x  2 user user 4096 Jan 1 12:00 .\ndrwxr-xr-x 10 user user 4096 Jan 1 11:00 .."
            })
        ]),
    };

    // Verify the workflow
    assert_eq!(user_message.role, ChatRole::User);
    assert!(ai_response.tool_calls.is_some());
    assert_eq!(ai_response.tool_calls.as_ref().unwrap().len(), 1);
    assert!(tool_result.tool_results.is_some());
    assert_eq!(tool_result.tool_results.as_ref().unwrap().len(), 1);
}

#[test]
fn test_chat_conversation_flow() {
    let mut conversation = Vec::new();

    // Add system message
    conversation.push(ChatMessage::new_system_message(
        "You are a helpful AI assistant."
    ));

    // Add user message
    conversation.push(ChatMessage::new_user_message(
        "What can you help me with?"
    ));

    // Add assistant response
    conversation.push(ChatMessage::new_arula_message(
        "I can help you with coding, file operations, and running commands."
    ));

    // Add tool call
    conversation.push(ChatMessage::new_tool_call(
        "Execute command".to_string(),
        r#"{"name": "bash_tool", "arguments": "pwd"}"#.to_string()
    ));

    // Verify conversation structure
    assert_eq!(conversation.len(), 4);
    assert_eq!(conversation[0].message_type, MessageType::System);
    assert_eq!(conversation[1].message_type, MessageType::User);
    assert_eq!(conversation[2].message_type, MessageType::Arula);
    assert_eq!(conversation[3].message_type, MessageType::ToolCall);

    // Verify content
    assert_eq!(conversation[1].content, "What can you help me with?");
    assert_eq!(conversation[2].content, "I can help you with coding, file operations, and running commands.");
}

#[test]
fn test_message_serialization_roundtrip() {
    let original = ChatMessage::new_tool_call(
        "Write file".to_string(),
        r#"{"name": "write_file", "arguments": "{\"path\": \"test.txt\", \"content\": \"Hello World\"}"}"#.to_string()
    );

    // JSON roundtrip
    let json_str = serde_json::to_string(&original).unwrap();
    let json_restored: ChatMessage = serde_json::from_str(&json_str).unwrap();

    assert_eq!(original.message_type, json_restored.message_type);
    assert_eq!(original.content, json_restored.content);
    assert_eq!(original.tool_call_json, json_restored.tool_call_json);

    // YAML roundtrip
    let yaml_str = serde_yaml::to_string(&original).unwrap();
    let yaml_restored: ChatMessage = serde_yaml::from_str(&yaml_str).unwrap();

    assert_eq!(original.message_type, yaml_restored.message_type);
    assert_eq!(original.content, yaml_restored.content);
    assert_eq!(original.tool_call_json, yaml_restored.tool_call_json);
}

#[test]
fn test_enhanced_message_complex_scenario() {
    // Simulate a complex interaction with multiple tools
    let message = EnhancedChatMessage {
        role: ChatRole::Assistant,
        content: "I'll help you analyze this project. Let me first check the directory structure and then read the main file.".to_string(),
        timestamp: Utc::now(),
        tool_calls: Some(vec![
            serde_json::json!({
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "bash_tool",
                    "arguments": "{\"command\": \"find . -type f -name '*.rs' | head -10\"}"
                }
            }),
            serde_json::json!({
                "id": "call_2",
                "type": "function",
                "function": {
                    "name": "read_file",
                    "arguments": "{\"path\": \"src/main.rs\"}"
                }
            })
        ]),
        tool_results: Some(vec![
            serde_json::json!({
                "tool_call_id": "call_1",
                "success": true,
                "result": "./src/main.rs\n./src/lib.rs\n./src/config.rs\n..."
            }),
            serde_json::json!({
                "tool_call_id": "call_2",
                "success": true,
                "result": "fn main() {\n    println!(\"Hello, world!\");\n}"
            })
        ]),
    };

    // Serialize and verify
    let serialized = serde_json::to_string(&message).unwrap();
    let deserialized: EnhancedChatMessage = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.role, ChatRole::Assistant);
    assert!(deserialized.tool_calls.is_some());
    assert!(deserialized.tool_results.is_some());

    let tool_calls = deserialized.tool_calls.unwrap();
    let tool_results = deserialized.tool_results.unwrap();

    assert_eq!(tool_calls.len(), 2);
    assert_eq!(tool_results.len(), 2);

    // Verify tool call details
    let first_call = &tool_calls[0];
    assert_eq!(first_call["id"], "call_1");
    assert_eq!(first_call["function"]["name"], "bash_tool");

    // Verify tool result details
    let first_result = &tool_results[0];
    assert_eq!(first_result["tool_call_id"], "call_1");
    assert_eq!(first_result["success"], true);
}

#[test]
fn test_message_edge_cases() {
    // Test with very long content
    let long_content = "x".repeat(10000);
    let long_message = ChatMessage::new_user_message(&long_content);

    assert_eq!(long_message.content.len(), 10000);

    // Should serialize and deserialize correctly
    let serialized = serde_json::to_string(&long_message).unwrap();
    let deserialized: ChatMessage = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.content.len(), 10000);

    // Test with special characters and Unicode
    let unicode_message = ChatMessage::new_arula_message(
        "Special chars: !@#$%^&*()🚀🎉中文字符"
    );

    let serialized = serde_json::to_string(&unicode_message).unwrap();
    let deserialized: ChatMessage = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.content, "Special chars: !@#$%^&*()🚀🎉中文字符");

    // Test with JSON content in tool calls
    let json_tool_call = ChatMessage::new_tool_call(
        "Complex tool call".to_string(),
        r#"{"nested": {"array": [1, 2, 3], "object": {"key": "value"}}, "unicode": "🚀"}"#.to_string()
    );

    let serialized = serde_json::to_string(&json_tool_call).unwrap();
    let deserialized: ChatMessage = serde_json::from_str(&serialized).unwrap();
    let tool_call_json = deserialized.tool_call_json.as_ref().unwrap();
    assert!(tool_call_json.contains("nested"));
    assert!(tool_call_json.contains("🚀"));
}

#[test]
fn test_message_type_and_role_consistency() {
    // Test that all message types can be displayed
    let all_types = vec![
        MessageType::User,
        MessageType::Arula,
        MessageType::System,
        MessageType::Success,
        MessageType::Error,
        MessageType::Info,
        MessageType::ToolCall,
        MessageType::ToolResult,
    ];

    for msg_type in all_types {
        let display_str = msg_type.to_string();
        assert!(!display_str.is_empty());
        assert!(display_str.is_ascii()); // Should be ASCII for compatibility
    }

    // Test that all chat roles can be displayed
    let all_roles = vec![
        ChatRole::User,
        ChatRole::Assistant,
        ChatRole::System,
        ChatRole::Tool,
    ];

    for role in all_roles {
        let display_str = role.to_string();
        assert!(!display_str.is_empty());
        assert!(display_str.is_ascii());
    }
}

#[test]
fn test_message_ordering_and_timestamps() {
    let now = Local::now();

    // Create messages with delays to ensure different timestamps
    let message1 = ChatMessage::new_user_message("First message");
    std::thread::sleep(std::time::Duration::from_millis(1));
    let message2 = ChatMessage::new_user_message("Second message");
    std::thread::sleep(std::time::Duration::from_millis(1));
    let message3 = ChatMessage::new_user_message("Third message");

    // Verify timestamps are in chronological order
    assert!(message1.timestamp <= message2.timestamp);
    assert!(message2.timestamp <= message3.timestamp);
    assert!(message1.timestamp <= now + chrono::Duration::seconds(1));

    // Test with UTC timestamps for enhanced messages
    let utc_now = Utc::now();
    let enhanced1 = EnhancedChatMessage {
        role: ChatRole::User,
        content: "First".to_string(),
        timestamp: utc_now,
        tool_calls: None,
        tool_results: None,
    };

    std::thread::sleep(std::time::Duration::from_millis(1));
    let later = Utc::now();
    let enhanced2 = EnhancedChatMessage {
        role: ChatRole::User,
        content: "Second".to_string(),
        timestamp: later,
        tool_calls: None,
        tool_results: None,
    };

    assert!(enhanced1.timestamp <= enhanced2.timestamp);
}

#[test]
fn test_message_comparison_and_equality() {
    // Test MessageType equality
    assert_eq!(MessageType::User, MessageType::User);
    assert_ne!(MessageType::User, MessageType::Arula);
    assert_ne!(MessageType::System, MessageType::Error);

    // Test ChatRole equality
    assert_eq!(ChatRole::User, ChatRole::User);
    assert_ne!(ChatRole::User, ChatRole::Assistant);
    assert_ne!(ChatRole::System, ChatRole::Tool);

    // Test ChatMessage equality
    let message1 = ChatMessage::new_user_message("Test");
    let message2 = ChatMessage::new_user_message("Test");
    // Note: timestamps will likely be different, so direct equality might not work

    assert_eq!(message1.message_type, message2.message_type);
    assert_eq!(message1.content, message2.content);

    // Test with same timestamp (manually set)
    let timestamp = Local::now();
    let message3 = ChatMessage {
        timestamp,
        message_type: MessageType::User,
        content: "Test".to_string(),
        tool_call_json: None,
    };
    let message4 = ChatMessage {
        timestamp,
        message_type: MessageType::User,
        content: "Test".to_string(),
        tool_call_json: None,
    };

    // Note: ChatMessage doesn't derive PartialEq, so we can't test direct equality
    // But we can test individual fields
    assert_eq!(message3.timestamp, message4.timestamp);
    assert_eq!(message3.message_type, message4.message_type);
    assert_eq!(message3.content, message4.content);
}

#[test]
fn test_message_debug_formats() {
    let message = ChatMessage::new_tool_call(
        "Debug test".to_string(),
        r#"{"test": true}"#.to_string()
    );

    let debug_str = format!("{:?}", message);
    assert!(debug_str.contains("ChatMessage"));
    assert!(debug_str.contains("ToolCall"));
    assert!(debug_str.contains("Debug test"));

    let enhanced = EnhancedChatMessage {
        role: ChatRole::Assistant,
        content: "Debug enhanced".to_string(),
        timestamp: Utc::now(),
        tool_calls: Some(vec![serde_json::json!({"debug": true})]),
        tool_results: None,
    };

    let debug_str = format!("{:?}", enhanced);
    assert!(debug_str.contains("EnhancedChatMessage"));
    assert!(debug_str.contains("Assistant"));
    assert!(debug_str.contains("Debug enhanced"));
}