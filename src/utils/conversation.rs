//! Conversation history management for ARULA CLI
//!
//! This module provides structures and utilities for saving and loading
//! conversation history with AI, including messages, tool calls, and metadata.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};

/// Conversation format version for compatibility
pub const CONVERSATION_VERSION: &str = "1.0";

/// Complete conversation history with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Format version
    pub version: String,
    /// Conversation metadata
    pub metadata: ConversationMetadata,
    /// AI configuration snapshot
    pub config_snapshot: ConfigSnapshot,
    /// All messages in the conversation
    pub messages: Vec<Message>,
    /// Usage statistics
    pub statistics: Statistics,
}

/// Metadata about the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetadata {
    /// Unique conversation ID
    pub conversation_id: String,
    /// User-friendly title (auto-generated or user-defined)
    pub title: String,
    /// When the conversation was created
    pub created_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
    /// Total number of messages
    pub message_count: usize,
    /// AI model used
    pub model: String,
    /// AI provider (openai, anthropic, ollama, etc.)
    pub provider: String,
    /// Optional tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Snapshot of AI configuration at conversation start
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub provider: String,
    pub model: String,
    pub api_endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID
    pub id: String,
    /// When the message was created
    pub timestamp: DateTime<Utc>,
    /// Role: "user", "assistant", or "tool"
    pub role: String,
    /// Message content (text or tool result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    /// Tool call information (for assistant messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID (for tool result messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool name (for tool result messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Message metadata
    pub metadata: MessageMetadata,
}

/// Tool call information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Tool arguments as JSON string
    pub arguments: String,
    /// When the tool was called
    pub timestamp: DateTime<Utc>,
}

/// Metadata for individual messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// Token count (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<usize>,
    /// Finish reason for assistant messages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// Execution time for tool messages (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<u64>,
    /// Whether tool execution was successful
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
}

/// Conversation usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    pub total_user_messages: usize,
    pub total_assistant_messages: usize,
    pub total_tool_calls: usize,
    pub total_tool_results: usize,
    pub successful_tool_calls: usize,
    pub failed_tool_calls: usize,
    #[serde(default)]
    pub total_tokens_used: usize,
    #[serde(default)]
    pub user_tokens: usize,
    #[serde(default)]
    pub assistant_tokens: usize,
    #[serde(default)]
    pub tool_tokens: usize,
    /// Total conversation duration in seconds
    #[serde(default)]
    pub duration_seconds: u64,
}

impl Conversation {
    /// Create a new conversation
    pub fn new(model: String, provider: String, api_endpoint: String) -> Self {
        let now = Utc::now();
        let conversation_id = Self::generate_id();

        Self {
            version: CONVERSATION_VERSION.to_string(),
            metadata: ConversationMetadata {
                conversation_id: conversation_id.clone(),
                title: "New Conversation".to_string(),
                created_at: now,
                updated_at: now,
                message_count: 0,
                model: model.clone(),
                provider: provider.clone(),
                tags: Vec::new(),
            },
            config_snapshot: ConfigSnapshot {
                provider,
                model,
                api_endpoint,
                temperature: None,
                max_tokens: None,
            },
            messages: Vec::new(),
            statistics: Statistics {
                total_user_messages: 0,
                total_assistant_messages: 0,
                total_tool_calls: 0,
                total_tool_results: 0,
                successful_tool_calls: 0,
                failed_tool_calls: 0,
                total_tokens_used: 0,
                user_tokens: 0,
                assistant_tokens: 0,
                tool_tokens: 0,
                duration_seconds: 0,
            },
        }
    }

    /// Generate a unique conversation ID
    fn generate_id() -> String {
        let now = Utc::now();
        let random_suffix: String = (0..6)
            .map(|_| format!("{:x}", fastrand::u8(0..16)))
            .collect();
        format!(
            "conv_{}_{}",
            now.format("%Y_%m_%d_%H%M%S"),
            random_suffix
        )
    }

    /// Generate a unique message ID
    fn generate_message_id(&self) -> String {
        format!("msg_{:03}", self.messages.len() + 1)
    }

    /// Generate a meaningful title from a user message
    fn generate_title_from_message(content: &str) -> Option<String> {
        let content = content.trim();

        // Skip empty or very short messages
        if content.len() < 3 {
            return None;
        }

        // Common greetings that don't make good titles
        let greetings = ["hi", "hello", "hey", "yo", "sup", "good morning", "good afternoon"];
        if greetings.iter().any(|greeting| content.eq_ignore_ascii_case(greeting)) {
            return None;
        }

        // Split into words and take first meaningful words
        let words: Vec<&str> = content
            .split_whitespace()
            .take(6) // Take first 6 words max
            .collect();

        if words.is_empty() {
            return None;
        }

        // Build title, ensuring it's not too long
        let mut title = words.join(" ");

        // Remove trailing punctuation that doesn't look good in titles
        title = title.trim_end_matches(|c| matches!(c, '.' | ',' | ';' | ':' | '!' | '?')).to_string();

        // Ensure title is reasonable length
        if title.len() > 60 {
            if let Some(space_pos) = title.rfind(' ') {
                title.truncate(space_pos);
            } else {
                title.truncate(57);
                title.push_str("...");
            }
        }

        // Capitalize first letter
        if !title.is_empty() {
            title = title.chars().enumerate().map(|(i, c)| {
                if i == 0 {
                    c.to_uppercase().collect::<String>()
                } else {
                    c.to_string()
                }
            }).collect();
        }

        Some(title)
    }

    /// Add a user message
    pub fn add_user_message(&mut self, content: String) -> String {
        let msg_id = self.generate_message_id();
        let message = Message {
            id: msg_id.clone(),
            timestamp: Utc::now(),
            role: "user".to_string(),
            content: Some(serde_json::Value::String(content.clone())),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            metadata: MessageMetadata {
                token_count: None,
                finish_reason: None,
                execution_time_ms: None,
                success: None,
            },
        };

        self.messages.push(message);
        self.metadata.message_count = self.messages.len();
        self.metadata.updated_at = Utc::now();
        self.statistics.total_user_messages += 1;

        // Auto-generate title for first user message if still default
        if self.messages.len() == 1 && self.metadata.title == "New Conversation" {
            if let Some(generated_title) = Self::generate_title_from_message(&content) {
                self.metadata.title = generated_title;
            }
        }

        msg_id
    }

    /// Add an assistant message
    pub fn add_assistant_message(&mut self, content: String, tool_calls: Option<Vec<ToolCall>>) -> String {
        let msg_id = self.generate_message_id();
        let message = Message {
            id: msg_id.clone(),
            timestamp: Utc::now(),
            role: "assistant".to_string(),
            content: Some(serde_json::Value::String(content)),
            tool_calls: tool_calls.clone(),
            tool_call_id: None,
            tool_name: None,
            metadata: MessageMetadata {
                token_count: None,
                finish_reason: if tool_calls.is_some() { Some("tool_use".to_string()) } else { Some("end_turn".to_string()) },
                execution_time_ms: None,
                success: None,
            },
        };

        self.messages.push(message);
        self.metadata.message_count = self.messages.len();
        self.metadata.updated_at = Utc::now();
        self.statistics.total_assistant_messages += 1;

        if let Some(calls) = tool_calls {
            self.statistics.total_tool_calls += calls.len();
        }

        msg_id
    }

    /// Add a tool result message
    pub fn add_tool_result(&mut self, tool_call_id: String, tool_name: String, content: serde_json::Value, success: bool, execution_time_ms: u64) -> String {
        let msg_id = self.generate_message_id();
        let message = Message {
            id: msg_id.clone(),
            timestamp: Utc::now(),
            role: "tool".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
            tool_name: Some(tool_name),
            metadata: MessageMetadata {
                token_count: None,
                finish_reason: None,
                execution_time_ms: Some(execution_time_ms),
                success: Some(success),
            },
        };

        self.messages.push(message);
        self.metadata.message_count = self.messages.len();
        self.metadata.updated_at = Utc::now();
        self.statistics.total_tool_results += 1;

        if success {
            self.statistics.successful_tool_calls += 1;
        } else {
            self.statistics.failed_tool_calls += 1;
        }

        msg_id
    }

    /// Set the conversation title
    pub fn set_title(&mut self, title: String) {
        self.metadata.title = title;
        self.metadata.updated_at = Utc::now();
    }

    /// Add a tag to the conversation
    pub fn add_tag(&mut self, tag: String) {
        if !self.metadata.tags.contains(&tag) {
            self.metadata.tags.push(tag);
            self.metadata.updated_at = Utc::now();
        }
    }

    /// Get the conversation file path
    pub fn get_file_path(&self, base_dir: &Path) -> PathBuf {
        base_dir
            .join(".arula")
            .join("conversations")
            .join(format!("{}.json", self.metadata.conversation_id))
    }

    /// Save the conversation to disk
    pub fn save(&self, base_dir: &Path) -> Result<()> {
        let file_path = self.get_file_path(base_dir);

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Serialize and save
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(file_path, json)?;

        Ok(())
    }

    /// Load a conversation from disk
    pub fn load(base_dir: &Path, conversation_id: &str) -> Result<Self> {
        let file_path = base_dir
            .join(".arula")
            .join("conversations")
            .join(format!("{}.json", conversation_id));

        let json = std::fs::read_to_string(file_path)?;
        let conversation: Self = serde_json::from_str(&json)?;

        Ok(conversation)
    }

    /// List all conversations in a directory
    pub fn list_all(base_dir: &Path) -> Result<Vec<ConversationSummary>> {
        let conversations_dir = base_dir.join(".arula").join("conversations");

        if !conversations_dir.exists() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();

        for entry in std::fs::read_dir(conversations_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                // Read just the metadata for efficiency
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(conv) = serde_json::from_str::<Conversation>(&json) {
                        summaries.push(ConversationSummary {
                            conversation_id: conv.metadata.conversation_id,
                            title: conv.metadata.title,
                            created_at: conv.metadata.created_at,
                            updated_at: conv.metadata.updated_at,
                            message_count: conv.metadata.message_count,
                            model: conv.metadata.model,
                            provider: conv.metadata.provider,
                        });
                    }
                }
            }
        }

        // Sort by updated_at descending (most recent first)
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(summaries)
    }

    /// Delete a conversation file
    pub fn delete(base_dir: &Path, conversation_id: &str) -> Result<()> {
        let file_path = base_dir
            .join(".arula")
            .join("conversations")
            .join(format!("{}.json", conversation_id));

        if file_path.exists() {
            std::fs::remove_file(file_path)?;
        }

        Ok(())
    }

    /// Update duration statistics
    pub fn update_duration(&mut self) {
        if let (Some(first), Some(last)) = (self.messages.first(), self.messages.last()) {
            let duration = last.timestamp.signed_duration_since(first.timestamp);
            self.statistics.duration_seconds = duration.num_seconds() as u64;
        }
    }
}

/// Summary of a conversation for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub conversation_id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub model: String,
    pub provider: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_conversation() {
        let conv = Conversation::new(
            "claude-sonnet-4-5".to_string(),
            "anthropic".to_string(),
            "https://api.anthropic.com/v1".to_string(),
        );

        assert_eq!(conv.version, CONVERSATION_VERSION);
        assert_eq!(conv.messages.len(), 0);
        assert_eq!(conv.statistics.total_user_messages, 0);
    }

    #[test]
    fn test_add_messages() {
        let mut conv = Conversation::new(
            "claude-sonnet-4-5".to_string(),
            "anthropic".to_string(),
            "https://api.anthropic.com/v1".to_string(),
        );

        conv.add_user_message("Hello".to_string());
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.statistics.total_user_messages, 1);

        conv.add_assistant_message("Hi there!".to_string(), None);
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.statistics.total_assistant_messages, 1);
    }

    #[test]
    fn test_add_tool_result() {
        let mut conv = Conversation::new(
            "claude-sonnet-4-5".to_string(),
            "anthropic".to_string(),
            "https://api.anthropic.com/v1".to_string(),
        );

        let tool_calls = vec![ToolCall {
            id: "tool_001".to_string(),
            name: "read_file".to_string(),
            arguments: r#"{"path": "test.txt"}"#.to_string(),
            timestamp: Utc::now(),
        }];

        conv.add_assistant_message("Reading file".to_string(), Some(tool_calls));
        conv.add_tool_result(
            "tool_001".to_string(),
            "read_file".to_string(),
            serde_json::json!({"Ok": {"content": "test", "lines": 1}}),
            true,
            150,
        );

        assert_eq!(conv.statistics.total_tool_calls, 1);
        assert_eq!(conv.statistics.successful_tool_calls, 1);
    }
}
