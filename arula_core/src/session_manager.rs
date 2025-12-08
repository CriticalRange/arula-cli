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
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Read ARULA.md from ~/.arula/ directory
fn read_global_arula_md() -> Option<String> {
    let home_dir = dirs::home_dir()?;
    let global_arula_path = home_dir.join(".arula").join("ARULA.md");
    
    if global_arula_path.exists() {
        std::fs::read_to_string(&global_arula_path).ok()
    } else {
        None
    }
}

/// Read ARULA.md from current directory
fn read_local_arula_md() -> Option<String> {
    let local_arula_path = std::path::Path::new("ARULA.md");
    
    if local_arula_path.exists() {
        std::fs::read_to_string(local_arula_path).ok()
    } else {
        None
    }
}

/// Build system prompt with ARULA.md content
fn build_system_prompt_with_arula() -> String {
    let mut prompt_parts = Vec::new();
    
    // Base ARULA prompt
    prompt_parts.push("You are ARULA, an Autonomous AI Interface assistant. You help users with coding, shell commands, and general software development tasks. Be concise, helpful, and provide practical solutions.".to_string());
    
    // Add global ARULA.md from ~/.arula/
    if let Some(global_arula) = read_global_arula_md() {
        prompt_parts.push(format!("\n## Global Project Instructions\n{}", global_arula));
    }
    
    // Add local ARULA.md from current directory
    if let Some(local_arula) = read_local_arula_md() {
        prompt_parts.push(format!("\n## Current Project Context\n{}", local_arula));
    }
    
    prompt_parts.join("\n")
}

/// Events emitted by the session manager for UI updates.
#[derive(Debug, Clone)]
pub enum UiEvent {
    StreamStarted(Uuid),
    Token(Uuid, String, bool), // session_id, text, is_final
    Thinking(Uuid, String),
    ToolCallStart(Uuid, String, String, String), // session_id, id, name, display_info
    ToolCallResult(Uuid, String, bool, String),  // session_id, name, success, summary
    /// Bash output line streamed during command execution
    BashOutputLine(Uuid, String, String, bool), // session_id, tool_call_id, line, is_stderr
    StreamFinished(Uuid),
    StreamErrored(Uuid, String),
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
        let backend = AgentBackend::new(config, build_system_prompt_with_arula())?;
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

    /// Updates the backend with new configuration.
    pub fn update_backend(&mut self, config: &Config) -> anyhow::Result<()> {
        let backend = AgentBackend::new(config, build_system_prompt_with_arula())?;
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

    /// Helper function to summarize tool results
    fn summarize_tool_result(result: &serde_json::Value, success: bool) -> String {
        // Debug: log the actual result structure
        if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
            eprintln!("🔧 TOOL RESULT DEBUG: {}", serde_json::to_string(&result).unwrap_or_default());
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
        
        // Check for directory listing entries
        if let Some(entries) = data.get("entries").and_then(|e| e.as_array()) {
            return format!("{} items found", entries.len());
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
        
        
        // Handle error case
        if !success {
            if let Some(err) = result.get("Err") {
                if let Some(err_str) = err.as_str() {
                    return first_line(err_str, 80);
                }
                if let Ok(json_str) = serde_json::to_string(err) {
                    return first_line(&json_str, 80);
                }
            }
            return "Error".to_string();
        }
        
        // Fallback: try to show something useful
        if result.is_string() {
            return first_line(result.as_str().unwrap_or("Done"), 80);
        }
        
        "Done".to_string()
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
        let api_url = self.config.get_api_url();
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
        self.runtime.spawn(async move {
            let fetcher = ZaiFetcher;
            let models = fetcher.fetch_models("", None).await;
            cache.cache("zai", models);
        });
    }

    /// Get cached Z.AI models.
    pub fn get_cached_zai_models(&self) -> Option<Vec<String>> {
        self.model_cache.get_cached("zai")
    }
}
