//! Session Manager - High-level session orchestration
//!
//! Handles AI streaming sessions, tool execution, and communication with UI layers.
//! This module encapsulates all backend logic, keeping frontend layers pure.

use crate::api::api::ChatMessage;
use crate::api::models::{
    AnthropicFetcher, ModelCacheManager, ModelFetcher, OllamaFetcher, OpenAIFetcher,
    OpenRouterFetcher, ZaiFetcher,
};
use crate::utils::config::Config;
use crate::{AgentBackend, SessionConfig, SessionRunner, StreamEvent};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Read the comprehensive system prompt from ARULA_SYSTEM_PROMPT.md
/// This is the primary system prompt that defines ARULA's behavior
fn read_base_system_prompt() -> Option<String> {
    // Check multiple locations for the system prompt
    let possible_paths = [
        // Current directory (for development)
        std::path::PathBuf::from("ARULA_SYSTEM_PROMPT.md"),
        // Executable directory
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("ARULA_SYSTEM_PROMPT.md")))
            .unwrap_or_default(),
        // Home directory config
        dirs::home_dir()
            .map(|p| p.join(".arula").join("ARULA_SYSTEM_PROMPT.md"))
            .unwrap_or_default(),
    ];

    for path in &possible_paths {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                return Some(content);
            }
        }
    }
    None
}

/// Read PROJECT.manifest from current directory (project-specific context)
fn read_project_manifest() -> Option<String> {
    let manifest_path = std::path::Path::new("PROJECT.manifest");

    if manifest_path.exists() {
        std::fs::read_to_string(manifest_path).ok()
    } else {
        None
    }
}

/// Default base prompt if ARULA_SYSTEM_PROMPT.md is not found
const DEFAULT_BASE_PROMPT: &str = r#"# ARULA - Autonomous AI Interface

You are ARULA, an advanced AI coding assistant designed for software engineering tasks.

## CORE PRINCIPLES

1. **Be concise and direct** - Keep responses short unless detail is requested
2. **Use tools for actions** - Don't output code, use tools to implement changes  
3. **Read before editing** - Always understand existing code before making changes
4. **Follow conventions** - Match existing code style, patterns, and libraries
5. **Verify your work** - Run tests/lint when available

## TOOL USAGE

- Call tools directly when actions are needed
- Read files before editing them
- Never commit unless explicitly asked
- Batch independent operations when possible

## CODE QUALITY

- Never assume libraries are available - check first
- Never add comments unless asked
- Never expose secrets or credentials
- Ensure code is immediately runnable

## COMMUNICATION

- Be technical and to the point
- Don't start with "Great", "Certainly", "Sure"
- Provide brief summaries after completing tasks
- Don't end responses with questions
"#;

/// Build system prompt with layered content
/// Priority: Base System Prompt -> PROJECT.manifest
fn build_system_prompt_with_manifest() -> String {
    let mut prompt_parts = Vec::new();

    // 1. Base system prompt (comprehensive or default)
    if let Some(base_prompt) = read_base_system_prompt() {
        prompt_parts.push(base_prompt);
    } else {
        prompt_parts.push(DEFAULT_BASE_PROMPT.to_string());
    }

    // 2. Add PROJECT.manifest from current directory (project context)
    if let Some(manifest) = read_project_manifest() {
        prompt_parts.push(format!(
            "\n====\n\n## PROJECT CONTEXT\n\nThe following PROJECT.manifest defines this project:\n\n{}", 
            manifest
        ));
    }

    prompt_parts.join("\n")
}

/// Events emitted by the session manager for UI updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UiEvent {
    /// User message sent to the AI
    UserMessage {
        content: String,
        timestamp: String,
    },
    /// AI message received from the model
    AiMessage {
        content: String,
        timestamp: String,
    },
    StreamStarted(Uuid),
    Token(Uuid, String, bool), // session_id, text, is_final
    Thinking(Uuid, String),
    ToolCallStart(Uuid, String, String, String), // session_id, id, name, display_info
    ToolCallResult(Uuid, String, bool, String),  // session_id, name, success, summary
    /// Bash output line streamed during command execution
    BashOutputLine(Uuid, String, String, bool), // session_id, tool_call_id, line, is_stderr
    /// Ask question - AI needs user input
    AskQuestion {
        session_id: Uuid,
        tool_call_id: String,
        question: String,
        options: Option<Vec<String>>,
    },
    StreamFinished(Uuid),
    StreamErrored(Uuid, String),
    /// Conversation starters generated
    ConversationStarters(Vec<String>),
    /// Generated title for the conversation
    ConversationTitle(String),
}

/// Manages AI streaming sessions and communication with UI layers.
///
/// This is the main backend orchestrator - frontend code should use this
/// instead of directly interacting with API clients or stream handlers.
pub struct SessionManager {
    runtime: Runtime,
    events: broadcast::Sender<UiEvent>,
    runner: SessionRunner<AgentBackend>,
    config: Config,
    /// Unified model cache
    model_cache: Arc<ModelCacheManager>,
    /// Active session cancellation tokens
    cancellation_tokens: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
}

impl SessionManager {
    /// Creates a new session manager with the given configuration.
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let backend = AgentBackend::new(config, build_system_prompt_with_manifest())?;
        let runtime = Runtime::new()?;
        let (events, _) = broadcast::channel(128);
        let runner = SessionRunner::new(backend);
        Ok(Self {
            runtime,
            events,
            runner,
            config: config.clone(),
            model_cache: Arc::new(ModelCacheManager::new(30)), // 30 min TTL
            cancellation_tokens: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Get a clone of the backend (for use in async contexts like conversation starters)
    pub fn backend_clone(&self) -> AgentBackend {
        self.runner.backend_clone()
    }

    /// Updates the backend with new configuration.
    pub fn update_backend(&mut self, config: &Config) -> anyhow::Result<()> {
        let backend = AgentBackend::new(config, build_system_prompt_with_manifest())?;
        self.runner = SessionRunner::new(backend);
        self.config = config.clone();
        Ok(())
    }

    /// Signals that streaming should stop for the given session.
    /// This cancels the background task and sends a finished event.
    pub fn stop_stream(&self, session_id: Uuid) {
        // Cancel the background task
        if let Ok(tokens) = self.cancellation_tokens.lock() {
            if let Some(token) = tokens.get(&session_id) {
                token.cancel();
            }
        }
        // Send finished event to update UI
        let _ = self.events.send(UiEvent::StreamFinished(session_id));
    }

    /// Helper function to get display name for tools
    fn get_tool_display_name(name: &str) -> String {
        match name.to_lowercase().as_str() {
            "execute_bash" => "Shell".to_string(),
            "read_file" => "Read".to_string(),
            "write_file" => "Write".to_string(),
            "edit_file" => "Edit".to_string(),
            "list_directory" => "List".to_string(),
            "search_files" => "Search".to_string(),
            "web_search" => "Web".to_string(),
            "mcp_call" => "MCP".to_string(),
            "visioneer" => "Vision".to_string(),
            "ask_question" => "Question".to_string(),
            _ => name.to_string(),
        }
    }

    /// Helper function to format tool arguments
    fn format_tool_args(arguments: &str) -> String {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(arguments) {
            if let Some(obj) = args.as_object() {
                obj.iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                arguments.to_string()
            }
        } else {
            arguments.to_string()
        }
    }

    /// Helper function to format error messages with context
    fn format_error_with_context(err: &str, result: &serde_json::Value) -> String {
        // Clean up common error patterns
        let cleaned = err
            .trim()
            .replace("error: ", "")
            .replace("Error: ", "")
            .replace("ERROR: ", "");

        // Truncate if too long
        let truncated = if cleaned.len() > 100 {
            format!("{}...", &cleaned[..100])
        } else {
            cleaned.to_string()
        };

        // Try to extract tool name for context
        if let Some(tool) = result.get("tool").and_then(|t| t.as_str()) {
            format!("{}: {}", Self::capitalize_first(tool), truncated)
        } else {
            format!("Error: {}", truncated)
        }
    }

    /// Helper function to capitalize first letter
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Helper function to summarize tool results
    fn summarize_tool_result(result: &serde_json::Value, success: bool) -> String {
        // Debug: log the actual result structure
        if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
            eprintln!(
                "🔧 TOOL RESULT DEBUG: {}",
                serde_json::to_string(&result).unwrap_or_default()
            );
        }

        // Helper to extract first meaningful line
        fn first_line(s: &str, max_chars: usize) -> String {
            let trimmed = s.trim();
            let first = trimmed.lines().next().unwrap_or(trimmed);
            first.chars().take(max_chars).collect()
        }

        // The result can be in two formats:
        // 1. Wrapped: {"Ok": {...}} or {"Err": ...}
        // 2. Direct: {...} (the actual result)
        let data = if let Some(ok) = result.get("Ok") {
            ok
        } else if result.get("Err").is_some() {
            // Error case - handled below
            result
        } else {
            // Direct result (not wrapped)
            result
        };

        // Check for bash/shell command results with exit_code structure
        // Check for exit_code field (bash command result)
        if let Some(exit_code) = data.get("exit_code").and_then(|c| c.as_i64()) {
            if exit_code != 0 {
                // Command failed - show stderr if available
                if let Some(stderr) = data.get("stderr").and_then(|s| s.as_str()) {
                    let trimmed = stderr.trim();
                    if !trimmed.is_empty() {
                        return first_line(trimmed, 80);
                    }
                }
                return format!("Exit code: {}", exit_code);
            } else {
                // Command succeeded - show stdout
                if let Some(stdout) = data.get("stdout").and_then(|s| s.as_str()) {
                    let trimmed = stdout.trim();
                    if !trimmed.is_empty() {
                        return first_line(trimmed, 80);
                    }
                }
                return "Done".to_string();
            }
        }

        // Check for stdout/stderr without exit_code
        if let Some(stdout) = data.get("stdout").and_then(|s| s.as_str()) {
            let trimmed = stdout.trim();
            if !trimmed.is_empty() {
                return first_line(trimmed, 80);
            }
        }

        if let Some(stderr) = data.get("stderr").and_then(|s| s.as_str()) {
            let trimmed = stderr.trim();
            if !trimmed.is_empty() {
                return first_line(trimmed, 80);
            }
        }

        // Check for output field
        if let Some(output) = data.get("output").and_then(|s| s.as_str()) {
            let trimmed = output.trim();
            if !trimmed.is_empty() {
                return first_line(trimmed, 80);
            }
        }

        // Check for directory listing entries - show actual file list
        if let Some(entries) = data.get("entries").and_then(|e| e.as_array()) {
            let count = entries.len();
            // Build a list of file names for display
            let file_list: Vec<String> = entries
                .iter()
                .filter_map(|entry| {
                    let name = entry.get("name")?.as_str()?;
                    let file_type = entry
                        .get("file_type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("file");
                    let icon = if file_type == "directory" {
                        "📁"
                    } else {
                        "📄"
                    };
                    Some(format!("{} {}", icon, name))
                })
                .collect();

            if file_list.is_empty() {
                return format!("{} items found", count);
            } else {
                // Return count header plus file list
                return format!("{} items:\n{}", count, file_list.join("\n"));
            }
        }

        // Check for find_files result - show actual files found
        if let Some(files) = data.get("files").and_then(|f| f.as_array()) {
            let total_matches = data.get("total_matches").and_then(|m| m.as_u64()).unwrap_or(0) as usize;
            let limit_reached = data.get("limit_reached").and_then(|l| l.as_bool()).unwrap_or(false);

            if files.is_empty() {
                return format!("No files found");
            }

            // Show up to 5 files with their sizes
            let file_count = files.len().min(5);
            let mut file_list = Vec::new();

            for i in 0..file_count {
                if let Some(file) = files.get(i) {
                    let path = file.get("path").and_then(|p| p.as_str()).unwrap_or("?");
                    let size = file.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                    let file_type = file.get("file_type").and_then(|t| t.as_str()).unwrap_or("file");

                    let icon = if file_type == "directory" {
                        "📁"
                    } else if file_type == "symlink" {
                        "🔗"
                    } else {
                        "📄"
                    };

                    // Format size nicely
                    let size_str = if size == 0 {
                        String::new()
                    } else if size < 1024 {
                        format!("{}B", size)
                    } else if size < 1024 * 1024 {
                        format!("{}KB", size / 1024)
                    } else {
                        format!("{}MB", size / (1024 * 1024))
                    };

                    // Show just the filename, not full path
                    let filename = path.split('/').last().unwrap_or(path);

                    if size_str.is_empty() {
                        file_list.push(format!("{} {}", icon, filename));
                    } else {
                        file_list.push(format!("{} {} ({})", icon, filename, size_str));
                    }
                }
            }

            let mut result = format!("Found {} file{}", total_matches, if total_matches != 1 { "s" } else { "" });
            if limit_reached {
                result.push_str(" (showing first 5)");
            }
            result.push_str(":\n");
            result.push_str(&file_list.join("\n"));

            return result;
        }

        // Check for search_files result - show matches found
        if let Some(files) = data.get("files").and_then(|f| f.as_array()) {
            static EMPTY_VEC: std::sync::LazyLock<Vec<serde_json::Value>> = std::sync::LazyLock::new(Vec::new);
            let total_matches = data.get("total_matches").and_then(|m| m.as_u64()).unwrap_or(0) as usize;
            let files_searched = data.get("files_searched").and_then(|f| f.as_u64()).unwrap_or(0) as usize;
            let limit_reached = data.get("limit_reached").and_then(|l| l.as_bool()).unwrap_or(false);

            if files.is_empty() {
                return format!("No matches found in {} files searched", files_searched);
            }

            // Show up to 3 files with match counts and context
            let file_count = files.len().min(3);
            let mut file_list = Vec::new();

            for i in 0..file_count {
                if let Some(file) = files.get(i) {
                    let path = file.get("path").and_then(|p| p.as_str()).unwrap_or("?");
                    let matches = file.get("matches")
                        .and_then(|m| m.as_array())
                        .unwrap_or(&EMPTY_VEC);

                    // Show just the filename, not full path
                    let filename = path.split('/').last().unwrap_or(path);

                    // Get first match as example
                    if let Some(first_match) = matches.first() {
                        let line_num = first_match.get("line_number").and_then(|l| l.as_u64()).unwrap_or(0);
                        let line_content = first_match.get("line_content").and_then(|l| l.as_str()).unwrap_or("");

                        // Truncate line content if too long
                        let truncated = if line_content.len() > 60 {
                            format!("{}...", &line_content[..60])
                        } else {
                            line_content.to_string()
                        };

                        if matches.len() == 1 {
                            file_list.push(format!("📄 {} (line {}): {}", filename, line_num, truncated));
                        } else {
                            file_list.push(format!("📄 {} ({} matches, first at line {}): {}",
                                filename, matches.len(), line_num, truncated));
                        }
                    } else {
                        file_list.push(format!("📄 {}", filename));
                    }
                }
            }

            let mut result = format!("Found {} match{} in {} file{}",
                total_matches,
                if total_matches != 1 { "es" } else { "" },
                files_searched,
                if files_searched != 1 { "s" } else { "" });
            if limit_reached {
                result.push_str(" (showing first 3 files)");
            }
            result.push_str(":\n");
            result.push_str(&file_list.join("\n"));

            return result;
        }

        // Check for file content
        if let Some(content) = data.get("content").and_then(|c| c.as_str()) {
            return format!("{} chars", content.len());
        }

        // If data is a string directly
        if data.is_string() {
            let s = data.as_str().unwrap_or("Done");
            return first_line(s, 80);
        }

        // Handle error case - provide helpful, explanatory messages
        if !success {
            // Check for error field first (from ToolResult)
            if let Some(err) = data.get("error").and_then(|e| e.as_str()) {
                return Self::format_error_with_context(err, &result);
            }
            
            // Check for wrapped Err format
            if let Some(err) = result.get("Err") {
                if let Some(err_str) = err.as_str() {
                    return Self::format_error_with_context(err_str, &result);
                }
                if let Ok(json_str) = serde_json::to_string(err) {
                    return Self::format_error_with_context(&json_str, &result);
                }
            }
            
            // Check for stderr with non-zero exit code (bash commands)
            if let Some(exit_code) = data.get("exit_code").and_then(|c| c.as_i64()) {
                if exit_code != 0 {
                    if let Some(stderr) = data.get("stderr").and_then(|s| s.as_str()) {
                        let trimmed = stderr.trim();
                        if !trimmed.is_empty() {
                            return format!("Command failed (exit {}): {}", exit_code, first_line(trimmed, 80));
                        }
                    }
                    return format!("Command failed with exit code {}", exit_code);
                }
            }
            
            // Generic error with tool name context
            if let Some(tool_name) = data.get("tool").and_then(|t| t.as_str()) {
                return format!("{}: Failed - {}", Self::capitalize_first(tool_name), 
                    data.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error"));
            }
            
            return "Error: Operation failed".to_string();
        }

        // Check for edit_file result - show raw diff like git
        if let Some(diff) = data.get("diff").and_then(|d| d.as_str()) {
            if !diff.is_empty() {
                return diff.to_string();
            }
        }

        // Fallback: try to show something useful
        if result.is_string() {
            return first_line(result.as_str().unwrap_or("Done"), 80);
        }

        "Done".to_string()
    }

    /// Generate a conversation title from the first user message
    fn generate_conversation_title(tx: broadcast::Sender<UiEvent>, prompt: String) {
        // Use a simple heuristic-based title generation
        // This is faster than using AI and works well for most cases
        
        // Skip empty or very short messages
        if prompt.trim().len() < 3 {
            let _ = tx.send(UiEvent::ConversationTitle("New Chat".to_string()));
            return;
        }

        // Common greetings that don't make good titles
        const GREETINGS: &[&str] = &[
            "hi", "hello", "hey", "yo", "sup", "good morning", "good afternoon",
            "how are you", "how's it going",
        ];
        let prompt_lower = prompt.trim().to_lowercase();
        if GREETINGS.iter().any(|g| prompt_lower.starts_with(g) || prompt_lower.contains(g)) {
            let _ = tx.send(UiEvent::ConversationTitle("New Chat".to_string()));
            return;
        }

        // Split into words and take first few meaningful words
        let words: Vec<&str> = prompt
            .split_whitespace()
            .filter(|w| w.len() > 1) // Skip single-letter words
            .take(6) // Take first 6 words max
            .collect();

        if words.is_empty() {
            let _ = tx.send(UiEvent::ConversationTitle("New Chat".to_string()));
            return;
        }

        // Build title
        let mut title = words.join(" ");
        
        // Remove trailing punctuation
        title = title
            .trim_end_matches(['.', ',', ';', ':', '!', '?'])
            .to_string();

        // Ensure reasonable length
        if title.len() > 50 {
            if let Some(space_pos) = title.rfind(' ') {
                title.truncate(space_pos);
            } else {
                title.truncate(47);
                title.push_str("...");
            }
        }

        // Capitalize first letter
        if !title.is_empty() {
            let mut chars: Vec<char> = title.chars().collect();
            if let Some(first) = chars.get_mut(0) {
                *first = first.to_uppercase().next().unwrap_or(*first);
            }
            title = chars.into_iter().collect();
        }

        let _ = tx.send(UiEvent::ConversationTitle(title));
    }

    /// Starts a streaming session for the given prompt with conversation history.
    pub fn start_stream(
        &self,
        session_id: Uuid,
        prompt: String,
        history: Option<Vec<ChatMessage>>,
        session_config: SessionConfig,
    ) -> anyhow::Result<()> {
        let tx = self.events.clone();
        let runner = self.runner.clone();

        // Check if this is a new conversation (no history) for title generation
        let is_new_conversation = history.as_ref().map_or(true, |h| h.is_empty());

        // Create a cancellation token for this session
        let cancel_token = CancellationToken::new();
        let cancel_token_clone = cancel_token.clone();

        // Store the token so it can be cancelled later
        if let Ok(mut tokens) = self.cancellation_tokens.lock() {
            tokens.insert(session_id, cancel_token);
        }

        let tokens_ref = self.cancellation_tokens.clone();

        self.runtime.spawn(async move {
            let _ = tx.send(UiEvent::StreamStarted(session_id));

            // If this is a new conversation, generate a title from the first user message
            if is_new_conversation {
                Self::generate_conversation_title(tx.clone(), prompt.clone());
            }

            match runner.stream_session(prompt, history, session_config) {
                Ok(mut stream) => {
                    // Track tool call IDs to names
                    let mut tool_id_to_name: HashMap<String, String> = HashMap::new();

                    loop {
                        tokio::select! {
                            // Check for cancellation
                            _ = cancel_token_clone.cancelled() => {
                                // Cancelled by user
                                break;
                            }
                            // Process next stream event
                            event = stream.next() => {
                                match event {
                                    Some(StreamEvent::Start { .. }) => {}
                                    Some(StreamEvent::Text { text }) => {
                                        let _ = tx.send(UiEvent::Token(session_id, text, false));
                                    }
                                    Some(StreamEvent::Reasoning { text }) => {
                                        // Don't mix thinking into the main response - send separately
                                        let _ = tx.send(UiEvent::Thinking(session_id, text));
                                    }
                                    Some(StreamEvent::ToolCall { id, name, arguments }) => {
                                        // Store the mapping of tool call ID to tool name
                                        tool_id_to_name.insert(id.clone(), name.clone());

                                        let display_name = Self::get_tool_display_name(&name);
                                        let args_display = Self::format_tool_args(&arguments);
                                        let _ = tx.send(UiEvent::ToolCallStart(
                                            session_id,
                                            id.clone(),
                                            name.clone(),
                                            format!("{} • {}", display_name, args_display),
                                        ));
                                    }
                                    Some(StreamEvent::ToolResult { tool_call_id, result }) => {
                                        // Look up the actual tool name from the ID
                                        let tool_name = tool_id_to_name
                                            .get(&tool_call_id)
                                            .cloned()
                                            .unwrap_or_else(|| "unknown".to_string());

                                        let summary =
                                            Self::summarize_tool_result(&result.data, result.success);
                                        let _ = tx.send(UiEvent::ToolCallResult(
                                            session_id,
                                            tool_name, // Send the actual tool name, not the ID
                                            result.success,
                                            summary,
                                        ));
                                    }
                                    Some(StreamEvent::BashOutputLine { tool_call_id, line, is_stderr }) => {
                                        let _ = tx.send(UiEvent::BashOutputLine(
                                            session_id,
                                            tool_call_id,
                                            line,
                                            is_stderr,
                                        ));
                                    }
                                    Some(StreamEvent::AskQuestion { tool_call_id, question, options }) => {
                                        let _ = tx.send(UiEvent::AskQuestion {
                                            session_id,
                                            tool_call_id,
                                            question,
                                            options,
                                        });
                                    }
                                    Some(StreamEvent::Finished) => {
                                        let _ = tx.send(UiEvent::Token(session_id, String::new(), true));
                                        let _ = tx.send(UiEvent::StreamFinished(session_id));
                                        break;
                                    }
                                    Some(StreamEvent::Error(err)) => {
                                        let _ = tx.send(UiEvent::StreamErrored(session_id, err));
                                        break;
                                    }
                                    None => {
                                        // Stream ended
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    let _ = tx.send(UiEvent::StreamErrored(session_id, err.to_string()));
                }
            }

            // Clean up the cancellation token
            if let Ok(mut tokens) = tokens_ref.lock() {
                tokens.remove(&session_id);
            }
        });

        Ok(())
    }

    /// Get a broadcast receiver for UI events
    pub fn subscribe(&self) -> broadcast::Receiver<UiEvent> {
        self.events.subscribe()
    }

    // ==================== Model Fetching ====================

    /// Fetch OpenAI models asynchronously and cache them.
    pub fn fetch_openai_models(&self) {
        let cache = self.model_cache.clone();
        let api_key = self.config.get_api_key();
        self.runtime.spawn(async move {
            let fetcher = OpenAIFetcher;
            let models = fetcher.fetch_models(&api_key, None).await;
            cache.cache("openai", models);
        });
    }

    /// Get cached OpenAI models.
    pub fn get_cached_openai_models(&self) -> Option<Vec<String>> {
        self.model_cache.get_cached("openai")
    }

    /// Fetch Anthropic models asynchronously and cache them.
    pub fn fetch_anthropic_models(&self) {
        let cache = self.model_cache.clone();
        let api_key = self.config.get_api_key();
        self.runtime.spawn(async move {
            let fetcher = AnthropicFetcher;
            let models = fetcher.fetch_models(&api_key, None).await;
            cache.cache("anthropic", models);
        });
    }

    /// Get cached Anthropic models.
    pub fn get_cached_anthropic_models(&self) -> Option<Vec<String>> {
        self.model_cache.get_cached("anthropic")
    }

    /// Fetch Ollama models asynchronously and cache them.
    pub fn fetch_ollama_models(&self) {
        let cache = self.model_cache.clone();
        // Get Ollama-specific URL, not the active provider's URL
        // This is important when the active provider is different (e.g., Z.AI)
        let api_url = self.config.providers
            .get("ollama")
            .and_then(|p| p.api_url.clone())
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        self.runtime.spawn(async move {
            let fetcher = OllamaFetcher;
            let models = fetcher.fetch_models("", Some(&api_url)).await;
            cache.cache("ollama", models);
        });
    }

    /// Get cached Ollama models.
    pub fn get_cached_ollama_models(&self) -> Option<Vec<String>> {
        self.model_cache.get_cached("ollama")
    }

    /// Fetch OpenRouter models asynchronously and cache them.
    pub fn fetch_openrouter_models(&self) {
        let cache = self.model_cache.clone();
        let api_key = self.config.get_api_key();
        self.runtime.spawn(async move {
            let fetcher = OpenRouterFetcher;
            let models = fetcher.fetch_models(&api_key, None).await;
            cache.cache("openrouter", models);
        });
    }

    /// Get cached OpenRouter models.
    pub fn get_cached_openrouter_models(&self) -> Option<Vec<String>> {
        self.model_cache.get_cached("openrouter")
    }

    /// Fetch Z.AI models asynchronously and cache them.
    pub fn fetch_zai_models(&self) {
        let cache = self.model_cache.clone();
        let config = self.config.clone();
        
        self.runtime.spawn(async move {
            let fetcher = ZaiFetcher;
            let api_key = config.get_api_key();
            let models = fetcher.fetch_models(&api_key, None).await;
            cache.cache("zai", models);
        });
    }

    /// Get cached Z.AI models.
    pub fn get_cached_zai_models(&self) -> Option<Vec<String>> {
        self.model_cache.get_cached("zai")
    }

    // ==================== Conversation Starters ====================

    /// Generate 3 contextual conversation starter suggestions based on project context.
    /// This makes a lightweight API call that doesn't get added to conversation history.
    pub fn generate_conversation_starters(&self) {
        let backend = self.backend_clone();
        let config = self.config.clone();
        let events = self.events.clone();
        
        self.runtime.spawn(async move {
            let starters = fetch_starters_internal(backend, &config).await;
            let _ = events.send(UiEvent::ConversationStarters(starters));
        });
    }
}

/// Internal async function to fetch conversation starters from AI.
async fn fetch_starters_internal(
    backend: AgentBackend,
    config: &Config,
) -> Vec<String> {
    // Build system prompt with PROJECT.manifest context
    let system_prompt = build_system_prompt_with_manifest();
    
    let prompt = r#"Based on the PROJECT.manifest context, suggest exactly 3 short, actionable conversation starters 
that would be useful for a developer working on this project. Each starter should:
- Be 5-10 words maximum
- Start with a verb (e.g., "Add", "Fix", "Refactor", "Test")
- Be specific and actionable
- Relate to common development tasks for this codebase

Return ONLY a JSON array of 3 strings, nothing else. Example format:
["Add user authentication", "Fix memory leak in parser", "Add unit tests for API"]"#;

    let client = match backend.create_client_with_prompt(config, system_prompt) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client for conversation starters: {e}");
            return vec![
                "Review recent changes".to_string(),
                "Run tests and fix issues".to_string(),
                "Add new feature".to_string(),
            ];
        }
    };

    // Make a single, non-streaming request to get starters
    match client.query(prompt, None).await {
        Ok(mut stream) => {
            let mut response = String::new();
            while let Some(block) = stream.next().await {
                match block {
                    crate::api::agent::ContentBlock::Text { text } => {
                        response.push_str(&text);
                    }
                    crate::api::agent::ContentBlock::Reasoning { reasoning: _ } => {
                        eprintln!("DEBUG: Ignoring reasoning block in starters");
                    }
                    _ => {
                        eprintln!("DEBUG: Unexpected block type in starters");
                    }
                }
            }
            
            eprintln!("DEBUG: Raw starters response: {response}");
            eprintln!("DEBUG: Response length: {}", response.len());
            
            // Try to parse as JSON array directly
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                eprintln!("DEBUG: Parsed as JSON");
                if let Some(arr) = json.as_array() {
                    let starters: Vec<String> = arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s.len() < 100)
                        .take(3)
                        .collect();
                    
                    if !starters.is_empty() {
                        eprintln!("DEBUG: Parsed starters: {:?}", starters);
                        return starters;
                    }
                }
            }
            
            // Try extracting from markdown code blocks
            let cleaned = response
                .replace("```json", "")
                .replace("```", "")
                .trim()
                .to_string();
            
            eprintln!("DEBUG: Cleaned response: {cleaned}");
            
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&cleaned) {
                eprintln!("DEBUG: Parsed cleaned as JSON");
                if let Some(arr) = json.as_array() {
                    let starters: Vec<String> = arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s.len() < 100)
                        .take(3)
                        .collect();
                    
                    if !starters.is_empty() {
                        eprintln!("DEBUG: Parsed starters from cleaned JSON: {:?}", starters);
                        return starters;
                    }
                }
            }
            
            // Fallback: extract lines that look like suggestions
            let extracted: Vec<String> = response
                .lines()
                .filter_map(|l| {
                    let line = l.trim();
                    // Skip empty lines and markdown markers
                    if line.is_empty() 
                        || line.starts_with('{') 
                        || line.starts_with('[')
                        || line.starts_with("```")
                        || line.to_lowercase().starts_with("here")
                        || line.to_lowercase().starts_with("sure")
                        || line.to_lowercase().starts_with("certainly")
                    {
                        return None;
                    }
                    
                    // Remove common list markers and quotes
                    let cleaned = line
                        .trim_start_matches(|c| c == '-' || c == '*' || c == '+' || c == '.' || c == ')')
                        .trim_start_matches(|c: char| c.is_numeric())
                        .trim_start_matches(|c| c == '.' || c == ')' || c == ']')
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .trim_matches(',')
                        .trim()
                        .to_string();
                    
                    if cleaned.len() > 5 && cleaned.len() < 100 {
                        Some(cleaned)
                    } else {
                        None
                    }
                })
                .take(3)
                .collect();
            
            if !extracted.is_empty() {
                eprintln!("DEBUG: Extracted starters from lines: {:?}", extracted);
                extracted
            } else {
                eprintln!("DEBUG: No starters found, using defaults");
                vec![
                    "Review recent changes".to_string(),
                    "Run tests and fix issues".to_string(),
                    "Add new feature".to_string(),
                ]
            }
        }
        Err(e) => {
            eprintln!("Failed to fetch conversation starters: {e}");
            vec![
                "Review recent changes".to_string(),
                "Run tests and fix issues".to_string(),
                "Add new feature".to_string(),
            ]
        }
    }
}
