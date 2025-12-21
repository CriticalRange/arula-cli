use chrono::{DateTime, Utc};
use std::time::Instant;
use uuid::Uuid;

/// A single message in a conversation.
#[derive(Debug, Clone)]
pub struct MessageEntry {
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub added_at: Instant, // For entry animation
    parsed_timestamp: DateTime<Utc>,
    /// Tool call ID for tracking streaming bash output (only set for tool messages)
    pub tool_call_id: Option<String>,
    /// Duration in seconds the AI spent thinking (only set for completed thinking messages)
    pub thinking_duration_secs: Option<f32>,
}

impl MessageEntry {
    /// Creates a new user message.
    pub fn user(content: String, timestamp: String) -> Self {
        let parsed_timestamp = DateTime::parse_from_rfc3339(&timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Self {
            role: "User".to_string(),
            content,
            timestamp,
            added_at: Instant::now(),
            parsed_timestamp,
            tool_call_id: None,
            thinking_duration_secs: None,
        }
    }

    /// Creates a new AI (Arula) message.
    pub fn ai(content: String, timestamp: String) -> Self {
        let parsed_timestamp = DateTime::parse_from_rfc3339(&timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Self {
            role: "Arula".to_string(),
            content,
            timestamp,
            added_at: Instant::now(),
            parsed_timestamp,
            tool_call_id: None,
            thinking_duration_secs: None,
        }
    }

    /// Returns true if this is a user message.
    pub fn is_user(&self) -> bool {
        self.role.to_lowercase() == "user"
    }

    /// Returns true if this is an AI message.
    pub fn is_ai(&self) -> bool {
        self.role.to_lowercase() == "arula"
    }

    /// Returns true if this is a Tool message.
    pub fn is_tool(&self) -> bool {
        self.role.to_lowercase() == "tool"
    }

    /// Returns true if this is a Thinking message.
    pub fn is_thinking(&self) -> bool {
        self.role.to_lowercase() == "thinking"
    }

    /// Creates a new Tool message with an optional tool_call_id for tracking streaming output.
    pub fn tool(content: String, timestamp: String, tool_call_id: Option<String>) -> Self {
        let parsed_timestamp = DateTime::parse_from_rfc3339(&timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Self {
            role: "Tool".to_string(),
            content,
            timestamp,
            added_at: Instant::now(),
            parsed_timestamp,
            tool_call_id,
            thinking_duration_secs: None,
        }
    }

    /// Creates a new Thinking message (for AI reasoning/thoughts).
    pub fn thinking(content: String, timestamp: String) -> Self {
        let parsed_timestamp = DateTime::parse_from_rfc3339(&timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Self {
            role: "Thinking".to_string(),
            content,
            timestamp,
            added_at: Instant::now(),
            parsed_timestamp,
            tool_call_id: None,
            thinking_duration_secs: None,
        }
    }

    /// Appends text to the message content.
    pub fn append(&mut self, text: &str) {
        self.content.push_str(text);
    }

    /// Returns a human-readable relative time string.
    pub fn relative_time(&self) -> String {
        arula_core::utils::time::relative_time(self.parsed_timestamp)
    }

    /// Returns the animation progress (0.0 to 1.0) based on time since added.
    pub fn animation_progress(&self) -> f32 {
        let elapsed = self.added_at.elapsed().as_secs_f32();
        let duration = 0.5; // 500ms slide-in
        (elapsed / duration).clamp(0.0, 1.0)
    }
}

/// A chat session with message history.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub messages: Vec<MessageEntry>,
    pub is_streaming: bool,
    /// Buffer for AI content - prevents incomplete messages before tool calls
    ai_buffer: String,
}

impl Session {
    /// Creates a new empty session.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            messages: Vec::new(),
            is_streaming: false,
            ai_buffer: String::new(),
        }
    }

    /// Creates a session from saved UiEvents.
    pub fn from_events(id: Uuid, events: &[arula_core::session_manager::UiEvent]) -> Self {
        let mut session = Self {
            id,
            messages: Vec::new(),
            is_streaming: false,
            ai_buffer: String::new(),
        };

        for event in events {
            match event {
                arula_core::session_manager::UiEvent::UserMessage { content, timestamp } => {
                    session.add_user_message(content.clone(), timestamp.clone());
                }
                arula_core::session_manager::UiEvent::AiMessage { content, timestamp } => {
                    session.add_ai_message(content.clone(), timestamp.clone());
                }
                arula_core::session_manager::UiEvent::Thinking(_, text) => {
                    session.append_thinking_message(text.clone(), Utc::now().to_rfc3339());
                }
                arula_core::session_manager::UiEvent::ToolCallStart(_, tool_call_id, _name, display_args) => {
                    session.add_tool_message(
                        format!("{} {}", session.get_tool_icon("tool"), display_args),
                        Utc::now().to_rfc3339(),
                        Some(tool_call_id.clone()),
                    );
                }
                arula_core::session_manager::UiEvent::ToolCallResult(_, _name, _success, _result_summary) => {
                    // For simplicity, we'll just mark the tool as complete
                    // The actual display is handled by the update_tool_message
                }
                _ => {}
            }
        }

        // Flush any remaining AI buffer
        session.flush_ai_buffer(Utc::now().to_rfc3339());
        
        session
    }

    /// Gets a tool icon for a given tool name.
    fn get_tool_icon(&self, name: &str) -> &'static str {
        match name.to_lowercase().as_str() {
            "execute_bash" => "⚡",
            "read_file" => "📄",
            "write_file" => "📝",
            "edit_file" => "✏️",
            "list_directory" => "📁",
            "search_files" => "🔍",
            "web_search" => "🌐",
            "mcp_call" => "🔌",
            "visioneer" => "👁️",
            _ => "🔧",
        }
    }

    /// Converts session messages to UiEvents for saving conversations.
    pub fn to_ui_events(&self) -> Vec<arula_core::session_manager::UiEvent> {
        let mut events = Vec::new();
        
        for msg in &self.messages {
            match msg.role.as_str() {
                "User" => {
                    events.push(arula_core::session_manager::UiEvent::UserMessage {
                        content: msg.content.clone(),
                        timestamp: msg.timestamp.clone(),
                    });
                }
                "Arula" => {
                    events.push(arula_core::session_manager::UiEvent::AiMessage {
                        content: msg.content.clone(),
                        timestamp: msg.timestamp.clone(),
                    });
                }
                "Thinking" => {
                    events.push(arula_core::session_manager::UiEvent::Thinking(self.id, msg.content.clone()));
                }
                "Tool" => {
                    // For tool messages, we'll create a simple ToolCallStart and ToolCallResult pair
                    let tool_call_id = msg.tool_call_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    events.push(arula_core::session_manager::UiEvent::ToolCallStart(
                        self.id,
                        tool_call_id.clone(),
                        "tool".to_string(),
                        msg.content.clone(),
                    ));
                    events.push(arula_core::session_manager::UiEvent::ToolCallResult(
                        self.id,
                        "tool".to_string(),
                        true,
                        msg.content.clone(),
                    ));
                }
                _ => {}
            }
        }
        
        events
    }

    /// Finalizes any pending thinking messages by calculating their duration.
    /// Called when a new non-thinking message is about to be added.
    fn finalize_thinking_messages(&mut self) {
        for msg in self.messages.iter_mut() {
            if msg.is_thinking() && msg.thinking_duration_secs.is_none() {
                // Calculate duration from when thinking started (minimum 1 second)
                let duration = msg.added_at.elapsed().as_secs_f32().max(1.0);
                msg.thinking_duration_secs = Some(duration);
            }
        }
    }

    /// Adds a user message to the session.
    pub fn add_user_message(&mut self, content: String, timestamp: String) {
        self.finalize_thinking_messages();
        self.messages.push(MessageEntry::user(content, timestamp));
    }

    /// Adds or appends to an AI message using buffered approach.
    /// Content is buffered until substantial to prevent incomplete messages before tool calls.
    pub fn append_ai_message(&mut self, content: String, timestamp: String) {
        // Add content to the buffer
        self.ai_buffer.push_str(&content);

        // Only flush buffer when we have substantial content (prevents "I" before tools)
        // 15 chars is enough to have a meaningful start like "I'll read the"
        if self.ai_buffer.len() >= 15 {
            self.finalize_thinking_messages();
            
            // Check if we should append to the last AI message or create a new one
            // We append if the last message is an AI message (streaming continuation)
            if let Some(last) = self.messages.last_mut() {
                if last.is_ai() {
                    // Append buffer content to existing AI message
                    last.content.push_str(&self.ai_buffer);
                    self.ai_buffer.clear();
                    return;
                }
            }
            
            // Create a new AI message (first chunk after user message or tool)
            self.messages.push(MessageEntry::ai(self.ai_buffer.clone(), timestamp));
            self.ai_buffer.clear();
        }
    }

    /// Adds a complete non-streaming AI response.
    /// This always creates a separate message bubble.
    pub fn add_ai_message(&mut self, content: String, timestamp: String) {
        self.finalize_thinking_messages();
        
        // For non-streaming responses, always create a new message bubble
        if !content.is_empty() {
            self.messages.push(MessageEntry::ai(content, timestamp));
        }
        
        // Clear any buffered content
        self.ai_buffer.clear();
    }

    /// Flushes any pending AI buffer content as a message.
    /// Called when stream ends to commit any remaining content.
    pub fn flush_ai_buffer(&mut self, timestamp: String) {
        if !self.ai_buffer.is_empty() {
            // Check if we should append to the last AI message or create a new one
            if let Some(last) = self.messages.last_mut() {
                if last.is_ai() {
                    // Append to existing AI message
                    last.content.push_str(&self.ai_buffer);
                    self.ai_buffer.clear();
                    return;
                }
            }
            
            // Create a new AI message if there's no existing one to append to
            self.messages
                .push(MessageEntry::ai(self.ai_buffer.clone(), timestamp));
        }
        self.ai_buffer.clear();
    }

    /// Adds a Tool message, discarding any incomplete AI buffer content.
    pub fn add_tool_message(
        &mut self,
        content: String,
        timestamp: String,
        tool_call_id: Option<String>,
    ) {
        // Discard any incomplete AI content in the buffer (prevents "I" before tools)
        self.ai_buffer.clear();

        // Finalize any pending thinking messages
        self.finalize_thinking_messages();

        // Always create a new tool message bubble for each tool interaction
        self.messages
            .push(MessageEntry::tool(content, timestamp, tool_call_id));
    }

    /// Updates the last tool message with completion status.
    /// If the last message is not a tool message, creates a new one.
    pub fn update_tool_message(&mut self, content: String, timestamp: String) {
        // Try to update the last tool message
        if let Some(last) = self.messages.last_mut() {
            if last.is_tool() {
                // Update the existing tool message
                last.content = content;
                return;
            }
        }

        // If no tool message exists or last message isn't a tool, create new one
        self.add_tool_message(content, timestamp, None);
    }

    /// Adds or appends to a Thinking message.
    pub fn append_thinking_message(&mut self, content: String, timestamp: String) {
        if let Some(last) = self.messages.last_mut() {
            if last.is_thinking() {
                last.append(&content);
                return;
            }
        }
        // Trim leading whitespace for the first chunk
        let trimmed = content.trim_start();
        if !trimmed.is_empty() {
            self.messages
                .push(MessageEntry::thinking(trimmed.to_string(), timestamp));
        }
    }

    /// Converts session messages to ChatMessage format for API calls.
    /// Includes user, AI, and tool messages for full conversation context.
    /// Excludes thinking messages as they're internal reasoning.
    pub fn get_chat_history(&self) -> Vec<arula_core::api::api::ChatMessage> {
        self.messages
            .iter()
            .filter(|msg| msg.is_user() || msg.is_ai() || msg.is_tool())
            .map(|msg| {
                if msg.is_tool() {
                    // Tool messages contain the result of tool execution
                    // Pass through tool_call_id for provider correlation
                    arula_core::api::api::ChatMessage {
                        role: "tool".to_string(),
                        content: Some(msg.content.clone()),
                        tool_calls: None,
                        tool_call_id: msg.tool_call_id.clone(), // Pass through the ID
                        tool_name: Some("tool_result".to_string()), // Generic name for Ollama compatibility
                    }
                } else {
                    arula_core::api::api::ChatMessage {
                        role: if msg.is_user() {
                            "user".to_string()
                        } else {
                            "assistant".to_string()
                        },
                        content: Some(msg.content.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                        tool_name: None,
                    }
                }
            })
            .collect()
    }

    /// Sets the streaming state.
    pub fn set_streaming(&mut self, streaming: bool) {
        self.is_streaming = streaming;
    }

    /// Gets the current streaming state.
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }

      /// Clears all messages from the session.
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.ai_buffer.clear();
        self.is_streaming = false;
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
