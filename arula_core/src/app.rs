//! Application state and AI interaction management
//!
//! This module contains the main `App` struct that orchestrates all application
//! functionality including:
//!
//! - Configuration management
//! - AI provider client initialization and communication
//! - Tool registry and execution
//! - Conversation history and persistence
//! - Model caching for responsive model selection
//! - Git state tracking for branch restoration
//!
//! # Architecture
//!
//! The `App` struct serves as the central coordinator between:
//! - `Config` - Settings and provider configuration
//! - `AgentClient` - AI provider communication
//! - `ToolRegistry` - Built-in and MCP tools
//! - `Conversation` - Message history persistence
//!
//! # Example
//!
//! ```rust,ignore
//! use arula_cli::app::App;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut app = App::new()?;
//!     app.initialize_tool_registry().await?;
//!     app.send_to_ai("Hello, AI!").await?;
//!     Ok(())
//! }
//! ```

use crate::api::agent::{AgentOptionsBuilder, ContentBlock};
use crate::api::agent_client::AgentClient;
use crate::utils::chat::{ChatMessage, MessageType};
use crate::utils::config::Config;
use crate::utils::debug::{
    debug_print, log_ai_interaction, log_ai_response_chunk, log_ai_response_complete,
};
use crate::utils::git_state::GitStateTracker;
use crate::utils::tool_call::{execute_bash_tool, ToolCall, ToolCallResult};
use anyhow::Result;
use futures::StreamExt;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub enum AiResponse {
    AgentStreamStart,
    AgentStreamText(String),
    /// Thinking/reasoning content started (shows pulsing animation)
    AgentThinkingStart,
    /// Thinking/reasoning content chunk
    AgentThinkingContent(String),
    /// Thinking/reasoning content ended
    AgentThinkingEnd,
    /// Legacy reasoning content (for backward compatibility)
    AgentReasoningContent(String),
    AgentToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    AgentToolResult {
        tool_call_id: String,
        success: bool,
        result: serde_json::Value,
    },
    AgentStreamEnd,
}

/// Commands for tracking conversation history from background task
#[derive(Debug)]
enum TrackingCommand {
    AssistantMessage(String),
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
        success: bool,
        execution_time_ms: u64,
    },
}

pub struct App {
    pub config: Config,
    pub agent_client: Option<AgentClient>,
    // Cached tool registry to avoid repeated MCP discovery
    pub cached_tool_registry: Option<crate::api::agent::ToolRegistry>,
    // Git state tracker for branch restoration
    pub git_state_tracker: GitStateTracker,
    pub messages: Vec<ChatMessage>,
    pub ai_response_rx: Option<mpsc::UnboundedReceiver<AiResponse>>,
    pub current_streaming_message: Option<String>,
    pub pending_bash_commands: Option<Vec<String>>,
    pub pending_tool_results: Option<Vec<ToolCallResult>>,
    pub pending_tool_calls: Option<Vec<ToolCall>>,
    pub debug: bool,
    // Cancellation token for stopping API requests
    pub cancellation_token: CancellationToken,
    // Task handle for aborting in-flight requests
    pub current_task_handle: Option<tokio::task::JoinHandle<()>>,
    // Model caches for all providers
    pub openrouter_models: Arc<Mutex<Option<Vec<String>>>>,
    pub openai_models: Arc<Mutex<Option<Vec<String>>>>,
    pub anthropic_models: Arc<Mutex<Option<Vec<String>>>>,
    pub ollama_models: Arc<Mutex<Option<Vec<String>>>>,
    pub zai_models: Arc<Mutex<Option<Vec<String>>>>,
    // Conversation tracking
    pub current_conversation: Option<crate::utils::conversation::Conversation>,
    pub auto_save_conversations: bool,
    tracking_rx: Option<std::sync::mpsc::Receiver<TrackingCommand>>,
    tracking_tx: Option<std::sync::mpsc::Sender<TrackingCommand>>,
    // Shared conversation for immediate saving from background tasks
    pub shared_conversation: Arc<Mutex<Option<crate::utils::conversation::Conversation>>>,
    // Pending init message to be sent to AI
    pub pending_init_message: Option<String>,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = Config::load_or_default()?;

        // Create persistent tracking channel
        let (tracking_tx, tracking_rx) = std::sync::mpsc::channel();

        Ok(Self {
            config,
            agent_client: None,
            cached_tool_registry: None,
            git_state_tracker: GitStateTracker::new("."),
            messages: Vec::new(),
            ai_response_rx: None,
            current_streaming_message: None,
            pending_bash_commands: None,
            pending_tool_results: None,
            pending_tool_calls: None,
            debug: false,
            cancellation_token: CancellationToken::new(),
            current_task_handle: None,
            openrouter_models: Arc::new(Mutex::new(None)),
            openai_models: Arc::new(Mutex::new(None)),
            anthropic_models: Arc::new(Mutex::new(None)),
            ollama_models: Arc::new(Mutex::new(None)),
            zai_models: Arc::new(Mutex::new(None)),
            current_conversation: None,
            auto_save_conversations: true, // Default to auto-save
            tracking_rx: Some(tracking_rx),
            tracking_tx: Some(tracking_tx),
            shared_conversation: Arc::new(Mutex::new(None)),
            pending_init_message: None,
        })
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Reload configuration from file and reinitialize agent client if needed
    pub fn reload_config(&mut self) -> Result<()> {
        // Reload configuration from file
        self.config = Config::load_or_default()?;

        // Clear cached tool registry to force refresh with new config
        self.cached_tool_registry = None;

        // Reinitialize agent client with new configuration
        self.initialize_agent_client()?;

        Ok(())
    }

    /// Initialize cached tool registry with MCP discovery (run once at startup)
    pub async fn initialize_tool_registry(&mut self) -> Result<()> {
        if self.cached_tool_registry.is_none() {
            eprintln!("🔧 Initializing tool registry with MCP discovery...");
            match crate::tools::tools::create_default_tool_registry_with_mcp(&self.config).await {
                Ok(registry) => {
                    self.cached_tool_registry = Some(registry);
                    eprintln!("✅ Tool registry initialized successfully");
                }
                Err(e) => {
                    eprintln!("⚠️ Failed to initialize tool registry with MCP: {}", e);
                    eprintln!("🔧 Falling back to basic tool registry...");
                    let registry = crate::tools::tools::create_basic_tool_registry();
                    self.cached_tool_registry = Some(registry);
                    eprintln!("✅ Basic tool registry initialized successfully");
                }
            }
        }
        Ok(())
    }

    /// Initialize git state tracking (load saved state from previous crash)
    pub async fn initialize_git_state(&mut self) -> Result<()> {
        // Load any saved git state from previous crash
        if let Some(branch) = self.git_state_tracker.load_branch_from_disk().await? {
            eprintln!(
                "🔧 GitState: Loaded saved branch from previous session: {:?}",
                branch
            );
        }
        Ok(())
    }

    /// Get the cached tool registry, initializing it if necessary
    pub fn get_tool_registry(&mut self) -> &crate::api::agent::ToolRegistry {
        if self.cached_tool_registry.is_none() {
            // This should not happen if initialize_tool_registry was called, but handle gracefully
            let registry = crate::tools::tools::create_basic_tool_registry();
            self.cached_tool_registry = Some(registry);
        }
        self.cached_tool_registry.as_ref().unwrap()
    }

    /// Build comprehensive system prompt from ARULA.md files
    fn build_system_prompt(&self) -> String {
        let mut prompt_parts = Vec::new();

        // Base ARULA personality
        prompt_parts.push("You are ARULA, an Autonomous AI Interface assistant. You help users with coding, shell commands, and general software development tasks. Be concise, helpful, and provide practical solutions.".to_string());

        // Add development mode warning if running from cargo
        if Self::is_running_from_cargo() {
            prompt_parts.push(r#"
## Development Mode Warning

⚠️ IMPORTANT: You are running in development mode (via `cargo run`).

**DO NOT run any of the following commands:**
- `cargo build` or `cargo run` - The executable is locked and cannot be rebuilt while running
- Any rebuild/recompile commands - They will fail with "Access is denied" errors

If the user asks you to rebuild or make code changes:
1. Make the code changes to the files as requested
2. Tell the user: "Changes complete. Please exit ARULA and run `cargo build && cargo run` to rebuild and test."
3. DO NOT attempt to run cargo build/run commands yourself

The user will manually rebuild after exiting the application.
"#.to_string());
        }

        // Add tool calling instructions
        prompt_parts.push(r#"
## Tool Usage

You have access to tools for file operations, shell commands, and more. Use them when the user's request requires it.

### Available Tools:
- `execute_bash`: Run shell commands (git, npm, cargo, ls, cat, etc.)
- `read_file`: Read file contents  
- `write_file`: Create or overwrite files
- `edit_file`: Make targeted edits to existing files
- `list_directory`: List files and directories
- `search_files`: Search for patterns in files

### When to use tools:
- User asks to run a command → call execute_bash
- User asks to read/view a file → call read_file
- User asks to list/show files → call list_directory
- User asks to edit/modify a file → call read_file, then edit_file
- User asks to analyze code → call read_file to read it first

### Important:
- Only use tools when the user's request requires an action
- Don't use tools just to demonstrate capabilities
- For simple questions or conversation, just respond normally
- When you do need a tool, call it directly without asking permission

### Tool Call Format (CRITICAL):
- DO NOT output tool calls as text like `<function=tool_name>` or `</function>`
- Tools are called through the API's function calling mechanism, not as text output
- If you find yourself typing `<function=` you are doing it WRONG
- Just describe what you want to do and the system will make the tool call for you
"#.to_string());

        // Add built-in tools information
        prompt_parts.push(self.build_builtin_tools_info());

        // Read PROJECT.manifest from current directory (highest priority)
        if let Some(manifest) = Self::read_project_manifest() {
            prompt_parts.push(format!(
                "\n## Project Manifest (Primary Context)\n{}",
                manifest
            ));
        }

        // Read global ARULA.md from ~/.arula/
        if let Some(global_arula) = Self::read_global_arula_md() {
            prompt_parts.push(format!(
                "\n## Global Project Instructions\n{}",
                global_arula
            ));
        }

        // Read local ARULA.md from current directory
        if let Some(local_arula) = Self::read_local_arula_md() {
            prompt_parts.push(format!("\n## Current Project Context\n{}", local_arula));
        }

        // Add MCP tool information
        prompt_parts.push(self.build_mcp_tool_info());

        prompt_parts.join("\n")
    }

    /// Build MCP tool information for the AI
    fn build_mcp_tool_info(&self) -> String {
        let mut info = String::new();

        // MCP Tools section
        info.push_str("\n## MCP (Model Context Protocol) Tools\n");
        info.push_str("You have access to MCP servers for extended capabilities. You can call these tools directly using function calls:\n\n");

        info.push_str("### 1. mcp_call - Call a tool from a configured MCP server\n");
        info.push_str("**Usage:** Use JSON function call syntax to execute MCP tools\n");
        info.push_str("**Parameters:**\n");
        info.push_str("- `server` (string, required): The MCP server ID (e.g., \"context7\")\n");
        info.push_str("- `action` (string, required): The tool name to call on the MCP server\n");
        info.push_str("- `parameters` (object, optional): Parameters for the tool call\n\n");

        info.push_str("### 2. mcp_list_tools - List all available MCP tools\n");
        info.push_str("**Usage:** Use JSON function call syntax to discover available tools\n");
        info.push_str("**Returns:** List of all available tools from configured MCP servers\n\n");

        // Add information about configured servers
        let mcp_servers = self.config.get_mcp_servers();
        if !mcp_servers.is_empty() {
            info.push_str("### Configured MCP Servers:\n");
            for server_id in mcp_servers.keys() {
                match server_id.as_str() {
                    "context7" => {
                        info.push_str(&format!(
                            "- **{}**: Context7 library documentation server\n",
                            server_id
                        ));
                        info.push_str("  - Use for: Getting Rust library documentation, examples, and API information\n");
                        info.push_str("  - Available tools:\n");
                        info.push_str("    * resolve-library-id: Resolves a library name to Context7-compatible library ID\n");
                        info.push_str("      - Parameters: {\"libraryName\": \"<library_name>\" (string, required)}\n");
                        info.push_str("      - Example: `{\"name\": \"mcp_call\", \"parameters\": {\"server\": \"context7\", \"action\": \"resolve-library-id\", \"parameters\": {\"libraryName\": \"tokio\"}}}`\n");
                        info.push_str("    * get-library-docs: Fetches documentation for a specific library\n");
                        info.push_str("      - Parameters: {\"context7CompatibleLibraryID\": \"<library_id>\" (string, required)}\n");
                        info.push_str("      - Example: `{\"name\": \"mcp_call\", \"parameters\": {\"server\": \"context7\", \"action\": \"get-library-docs\", \"parameters\": {\"context7CompatibleLibraryID\": \"/tokio/tokio\"}}}`\n");
                        info.push_str("  - Recommended workflow: First call resolve-library-id, then use the returned ID with get-library-docs\n\n");
                    }
                    _ => {
                        info.push_str(&format!("- **{}**: Custom MCP server\n", server_id));
                        info.push_str("  - Call mcp_list_tools() to discover available tools\n\n");
                    }
                }
            }
        }

        info
    }

    /// Detect if running from cargo (development mode)
    fn is_running_from_cargo() -> bool {
        // Check if the executable path contains "target/debug" or "target\debug"
        if let Ok(exe_path) = std::env::current_exe() {
            let path_str = exe_path.to_string_lossy();
            return path_str.contains("target/debug") || path_str.contains("target\\debug");
        }
        false
    }

    /// Read ARULA.md from ~/.arula/ directory
    fn read_global_arula_md() -> Option<String> {
        let home_dir = dirs::home_dir()?;
        let global_arula_path = home_dir.join(".arula").join("ARULA.md");

        if global_arula_path.exists() {
            match fs::read_to_string(&global_arula_path) {
                Ok(content) => {
                    debug_print(&format!(
                        "DEBUG: Loaded global ARULA.md from {}",
                        global_arula_path.display()
                    ));
                    Some(content)
                }
                Err(e) => {
                    debug_print(&format!("DEBUG: Failed to read global ARULA.md: {}", e));
                    None
                }
            }
        } else {
            None
        }
    }

    /// Read ARULA.md from current directory
    fn read_local_arula_md() -> Option<String> {
        let local_arula_path = Path::new("ARULA.md");

        if local_arula_path.exists() {
            match fs::read_to_string(local_arula_path) {
                Ok(content) => {
                    debug_print("DEBUG: Loaded local ARULA.md from current directory");
                    Some(content)
                }
                Err(e) => {
                    debug_print(&format!("DEBUG: Failed to read local ARULA.md: {}", e));
                    None
                }
            }
        } else {
            None
        }
    }

    /// Read PROJECT.manifest from current directory
    fn read_project_manifest() -> Option<String> {
        let manifest_path = Path::new("PROJECT.manifest");

        if manifest_path.exists() {
            match fs::read_to_string(manifest_path) {
                Ok(content) => {
                    debug_print("DEBUG: Loaded PROJECT.manifest - primary project context available");
                    Some(format!(
                        "PROJECT MANIFEST CONTENT:\n\n{}",
                        content
                    ))
                }
                Err(e) => {
                    debug_print(&format!("DEBUG: Failed to read PROJECT.manifest: {}", e));
                    None
                }
            }
        } else {
            debug_print("DEBUG: No PROJECT.manifest found in current directory");
            None
        }
    }

    pub fn initialize_agent_client(&mut self) -> Result<()> {
        // Initialize modern agent client with default options
        let agent_options = AgentOptionsBuilder::new()
            .system_prompt(&self.build_system_prompt())
            .model(&self.config.get_model())
            .auto_execute_tools(true)
            .max_tool_iterations(1000)
            .debug(self.debug)
            .build();

        // Create a new agent client with a basic tool registry
        // MCP tools are handled separately in the streaming response
        let basic_registry = crate::tools::tools::create_basic_tool_registry();

        self.agent_client = Some(AgentClient::new_with_registry(
            self.config.active_provider.clone(),
            self.config.get_api_url(),
            self.config.get_api_key(),
            self.config.get_model(),
            agent_options,
            &self.config,
            basic_registry,
        ));

        Ok(())
    }

    fn initialize_mcp_tools_async(&mut self) {
        use crate::tools::mcp::McpTool;

        // Initialize global MCP manager in background
        let config = self.config.clone();
        tokio::spawn(async move {
            McpTool::update_global_config(config).await;
        });

        // MCP tools will be initialized lazily when needed to avoid runtime conflicts
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    pub fn get_config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    pub fn set_model(&mut self, model: &str) {
        self.config.set_model(model);
        let _ = self.config.save();
        // Reinitialize agent client with new model
        let _ = self.initialize_agent_client();
    }

    pub fn clear_conversation(&mut self) {
        self.messages.clear();
    }

    pub fn get_message_history(&self) -> &Vec<ChatMessage> {
        &self.messages
    }

    pub async fn send_to_ai(&mut self, message: &str) -> Result<()> {
        // Check if agent client is initialized
        if self.agent_client.is_none() {
            if self.debug {
                debug_print("DEBUG: send_to_ai - agent_client is None, returning error");
            }
            return Err(anyhow::anyhow!(
                "AI client not initialized. Please configure AI settings using the /config command or application menu."
            ));
        }

        // Add user message to history
        self.messages
            .push(ChatMessage::new(MessageType::User, message.to_string()));

        // Send message using the modern agent client
        self.send_to_ai_with_agent(message).await
    }

    /// Send message using the modern agent client
    async fn send_to_ai_with_agent(&mut self, message: &str) -> Result<()> {
        // Save current git branch before AI interaction
        if let Err(e) = self.git_state_tracker.save_current_branch().await {
            eprintln!("⚠️ GitState: Failed to save current branch: {}", e);
        }

        // Get agent client
        let agent_client = match &self.agent_client {
            Some(client) => client.clone(),
            None => {
                return Err(anyhow::anyhow!("Agent client not initialized"));
            }
        };

        // Create channel for streaming responses
        let (tx, rx) = mpsc::unbounded_channel();
        self.ai_response_rx = Some(rx);
        if self.debug {
            debug_print(&format!(
                "DEBUG: send_to_ai_with_agent - Created new response receiver for message: '{}'",
                message
            ));
        }

        // Clone the persistent tracking sender for this request
        // All requests share the same receiver, so tracking commands are never lost
        let track_tx = self
            .tracking_tx
            .clone()
            .expect("Tracking channel not initialized");

        // Debug: Print current message count
        debug_print(&format!(
            "DEBUG: Total messages in self.messages: {}",
            self.messages.len()
        ));
        for (i, msg) in self.messages.iter().enumerate() {
            debug_print(&format!(
                "DEBUG: [{}] {:?} -> {}",
                i,
                msg.message_type,
                if msg.content.len() > 50 {
                    // Use char boundaries to safely truncate
                    let safe_end = msg
                        .content
                        .char_indices()
                        .nth(50)
                        .map(|(idx, _)| idx)
                        .unwrap_or(msg.content.len());
                    format!("{}...", &msg.content[..safe_end])
                } else {
                    msg.content.clone()
                }
            ));
        }

        // Convert chat messages to API format for agent
        // IMPORTANT: Include tool results so AI knows what tools were already used!
        let api_messages: Vec<crate::api::api::ChatMessage> = self
            .messages
            .iter()
            .filter(|m| {
                // Skip ToolCall messages (these are UI-only) but keep ToolResult
                m.message_type != MessageType::ToolCall
            })
            .map(|m| {
                let role = match m.message_type {
                    MessageType::User => "user".to_string(),
                    MessageType::Arula => "assistant".to_string(),
                    MessageType::ToolResult => "assistant".to_string(), // Tool results go as assistant context
                    _ => "system".to_string(),
                };
                crate::api::api::ChatMessage {
                    role,
                    content: Some(m.content.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    tool_name: None,
                }
            })
            .collect();

        debug_print(&format!(
            "DEBUG: API messages after filtering: {}",
            api_messages.len()
        ));

        // Log the AI interaction for debugging
        log_ai_interaction(message, &api_messages, None);

        // Check if streaming is enabled in config
        let streaming_enabled = self.config.get_streaming_enabled();
        debug_print(&format!(
            "DEBUG: Streaming mode: {}",
            if streaming_enabled {
                "enabled"
            } else {
                "disabled"
            }
        ));

        // Send message using modern agent in background
        let msg = message.to_string();
        let cancel_token = self.cancellation_token.clone();
        // Removed external_printer since we're using custom output system
        let shared_conv = self.shared_conversation.clone();
        let auto_save = self.auto_save_conversations;
        let handle = tokio::spawn(async move {
            // Track message content and tool calls for conversation history
            let mut accumulated_text = String::new();
            let mut tool_calls_list: Vec<(String, String, String)> = Vec::new(); // (id, name, args)
            let mut thinking_started = false; // Track if we've started thinking mode

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    // Request was cancelled
                    let _ = tx.send(AiResponse::AgentStreamEnd);
                }
                _result = async {
                    // Choose streaming or non-streaming based on config
                    // query_streaming() uses true SSE streaming for real-time output
                    // query_non_streaming() waits for complete response before displaying
                    let query_result = if streaming_enabled {
                        agent_client.query_streaming(&msg, Some(api_messages)).await
                    } else {
                        agent_client.query_non_streaming(&msg, Some(api_messages)).await
                    };

                    match query_result {
                        Ok(mut stream) => {
                            let _ = tx.send(AiResponse::AgentStreamStart);

                            loop {
                                tokio::select! {
                                    _ = cancel_token.cancelled() => {
                                        // Cancelled during streaming
                                        break;
                                    }
                                    content_block = stream.next() => {
                                        match content_block {
                                            Some(ContentBlock::Text { text }) => {
                                                // End thinking mode if it was active
                                                if thinking_started {
                                                    let _ = tx.send(AiResponse::AgentThinkingEnd);
                                                    thinking_started = false;
                                                }

                                                // Accumulate text for tracking
                                                accumulated_text.push_str(&text);

                                                // Log response chunk
                                                log_ai_response_chunk(&text);

                                                let _ = tx.send(AiResponse::AgentStreamText(text.clone()));
                                            }
                                            Some(ContentBlock::Reasoning { reasoning }) => {
                                                // Use the new thinking events for better UI
                                                if !thinking_started {
                                                    let _ = tx.send(AiResponse::AgentThinkingStart);
                                                    thinking_started = true;
                                                }
                                                // Send thinking content chunk
                                                let _ = tx.send(AiResponse::AgentThinkingContent(reasoning.clone()));
                                            }
                                            Some(ContentBlock::ToolCall {
                                                id,
                                                name,
                                                arguments,
                                            }) => {
                                                // End thinking mode if it was active
                                                if thinking_started {
                                                    let _ = tx.send(AiResponse::AgentThinkingEnd);
                                                    thinking_started = false;
                                                }

                                                // Track tool call
                                                tool_calls_list.push((id.clone(), name.clone(), arguments.clone()));

                                                let _ = tx.send(AiResponse::AgentToolCall {
                                                    id: id.clone(),
                                                    name: name.clone(),
                                                    arguments: arguments.clone(),
                                                });
                                            }
                                            Some(ContentBlock::ToolResult {
                                                tool_call_id,
                                                result,
                                            }) => {
                                                let _status = if result.success { "✓" } else { "✗" };

                                                // Clone result data for all uses
                                                let result_data = result.data.clone();

                                                // Look up tool name from tracked tool calls
                                                let tool_name = tool_calls_list.iter()
                                                    .find(|(id, _, _)| id == &tool_call_id)
                                                    .map(|(_, name, _)| name.clone())
                                                    .unwrap_or_else(|| "unknown".to_string());

                                                let _ = tx.send(AiResponse::AgentToolResult {
                                                    tool_call_id: tool_call_id.clone(),
                                                    success: result.success,
                                                    result: result_data.clone(),
                                                });

                                                // Send tracking command for tool result
                                                let _ = track_tx.send(TrackingCommand::ToolResult {
                                                    tool_call_id,
                                                    tool_name,
                                                    result: result_data.clone(),
                                                    success: result.success,
                                                    execution_time_ms: 100,
                                                });

                                            }
                                            Some(ContentBlock::Error { error }) => {
                                                // Convert error to AgentStreamText to maintain compatibility
                                                let error_msg = format!("[Error] {}", error);
                                                let _ = tx.send(AiResponse::AgentStreamText(error_msg.clone()));
                                                break;
                                            }
                                            Some(ContentBlock::BashOutputLine { .. }) => {
                                                // Ignore streaming bash output in this context (CLI/Legacy)
                                                // Desktop uses SessionManager which handles this event
                                            }
                                            None => {
                                                // Stream ended
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            // IMMEDIATELY save AI response to conversation (user's brilliant idea!)
                            // This happens BEFORE printing to ExternalPrinter, ensuring JSON is updated instantly
                            if !accumulated_text.is_empty() {
                                // Log the complete response
                                log_ai_response_complete(&accumulated_text);

                                debug_print(&format!("DEBUG: Immediately saving assistant message to shared conversation ({} chars)", accumulated_text.len()));

                                if let Ok(mut conv_guard) = shared_conv.lock() {
                                    if let Some(ref mut conv) = *conv_guard {
                                        conv.add_assistant_message(accumulated_text.clone(), None);

                                        // Save to disk immediately if auto-save is enabled
                                        if auto_save {
                                            if let Ok(current_dir) = std::env::current_dir() {
                                                conv.update_duration();
                                                let _ = conv.save(&current_dir);
                                                debug_print("DEBUG: Conversation saved to disk immediately from tokio task!");
                                            }
                                        }
                                    }
                                }

                                // DON'T send tracking command - we already saved immediately above
                                // The sync will happen when user sends next message via track_user_message()
                                debug_print("DEBUG: Skipping tracking command - already saved immediately to shared_conversation");
                            }

                            for (id, name, args) in tool_calls_list {
                                debug_print(&format!("DEBUG: Sending ToolCall tracking command: {}", name));
                                if let Err(e) = track_tx.send(TrackingCommand::ToolCall {
                                    id,
                                    name,
                                    arguments: args,
                                }) {
                                    debug_print(&format!("DEBUG: ERROR - Failed to send ToolCall tracking command: {}", e));
                                }
                            }

                            let _ = tx.send(AiResponse::AgentStreamEnd);
                        }
                        Err(e) => {
                            let error_msg = format!("**Error:** Failed to send message via agent: {}", e);
                            let _ = tx.send(AiResponse::AgentStreamText(error_msg.clone()));
                            let _ = tx.send(AiResponse::AgentStreamEnd);
                        }
                    }
                } => {}
            }
        });

        // Store the task handle so we can abort it on cancellation
        self.current_task_handle = Some(handle);

        Ok(())
    }

    /// Restore git branch after AI interaction completes
    pub async fn restore_git_branch(&self) {
        if let Err(e) = self.git_state_tracker.restore_original_branch().await {
            eprintln!("⚠️ GitState: Failed to restore git branch: {}", e);
        }
    }

    /// Cleanup method to restore git branch when app exits
    pub async fn cleanup(&self) {
        eprintln!("🔧 GitState: Cleaning up and restoring git state...");
        self.restore_git_branch().await;
    }

    /// Process pending tracking commands
    pub fn process_tracking_commands(&mut self) {
        // Collect all pending commands first to avoid borrow checker issues
        let commands: Vec<TrackingCommand> = if let Some(ref rx) = self.tracking_rx {
            let mut cmds = Vec::new();
            while let Ok(cmd) = rx.try_recv() {
                cmds.push(cmd);
            }
            if !cmds.is_empty() && self.debug {
                debug_print(&format!(
                    "DEBUG: Processing {} tracking commands",
                    cmds.len()
                ));
            }
            cmds
        } else {
            Vec::new()
        };

        // Now process the collected commands
        for cmd in commands {
            match cmd {
                TrackingCommand::AssistantMessage(content) => {
                    if self.debug {
                        debug_print(&format!(
                            "DEBUG: Tracking assistant message ({} chars)",
                            content.len()
                        ));
                    }
                    self.track_assistant_message(&content);
                }
                TrackingCommand::ToolCall {
                    id,
                    name,
                    arguments,
                } => {
                    if self.debug {
                        debug_print(&format!("DEBUG: Tracking tool call: {}", name));
                    }
                    self.track_tool_call(id, name, arguments);
                }
                TrackingCommand::ToolResult {
                    tool_call_id,
                    tool_name,
                    result,
                    success,
                    execution_time_ms,
                } => {
                    if self.debug {
                        debug_print(&format!(
                            "DEBUG: Tracking tool result: {} ({})",
                            tool_name,
                            if success { "success" } else { "failed" }
                        ));
                    }
                    self.track_tool_result(
                        tool_call_id,
                        tool_name,
                        result,
                        success,
                        execution_time_ms,
                    );
                }
            }
        }
    }

    pub fn check_ai_response_nonblocking(&mut self) -> Option<AiResponse> {
        if let Some(rx) = &mut self.ai_response_rx {
            match rx.try_recv() {
                Ok(response) => {
                    match &response {
                        AiResponse::AgentStreamStart => {
                            self.current_streaming_message = Some(String::new());
                        }
                        AiResponse::AgentStreamText(text) => {
                            if let Some(msg) = &mut self.current_streaming_message {
                                msg.push_str(text);
                            }
                        }
                        AiResponse::AgentThinkingStart => {
                            // Thinking started - nothing to store yet
                        }
                        AiResponse::AgentThinkingContent(_thinking) => {
                            // Thinking content - could store for conversation history
                        }
                        AiResponse::AgentThinkingEnd => {
                            // Thinking ended - nothing to store
                        }
                        AiResponse::AgentReasoningContent(_reasoning) => {
                            // Legacy reasoning content for conversation history
                        }
                        AiResponse::AgentToolCall {
                            id,
                            name,
                            arguments,
                        } => {
                            // Add tool call message to chat history
                            self.messages.push(ChatMessage::new(
                                MessageType::ToolCall,
                                format!("🔧 Tool call: {}({})", name, arguments),
                            ));

                            // Track tool call in conversation
                            self.track_tool_call(id.clone(), name.clone(), arguments.clone());
                        }
                        AiResponse::AgentToolResult {
                            tool_call_id,
                            success,
                            result,
                        } => {
                            // Add tool result message to chat history
                            let status = if *success { "✅" } else { "❌" };
                            let result_text = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());

                            self.messages.push(ChatMessage::new(
                                MessageType::ToolResult,
                                format!(
                                    "{} Tool result: {}\n{}",
                                    status, tool_call_id, result_text
                                ),
                            ));

                            // Note: Tool result tracking with proper name is handled via TrackingCommand
                            // This is a fallback that shouldn't normally be hit since we track via the async task
                        }
                        AiResponse::AgentStreamEnd => {
                            if let Some(full_message) = self.current_streaming_message.take() {
                                self.messages.push(ChatMessage::new(
                                    MessageType::Arula,
                                    full_message.clone(),
                                ));

                                // Track assistant message in conversation
                                self.track_assistant_message(&full_message);
                            }
                            self.ai_response_rx = None;
                        }
                    }
                    Some(response)
                }
                Err(mpsc::error::TryRecvError::Empty) => None,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.ai_response_rx = None;
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn get_pending_bash_commands(&mut self) -> Option<Vec<String>> {
        self.pending_bash_commands.take()
    }

    pub fn is_waiting_for_response(&self) -> bool {
        self.ai_response_rx.is_some()
    }

    pub async fn execute_tools(&mut self, tool_calls: Vec<ToolCall>) {
        let mut results = Vec::new();

        for tool_call in tool_calls {
            match tool_call.tool.as_str() {
                "bash_tool" => {
                    if let Some(command) =
                        tool_call.arguments.get("command").and_then(|v| v.as_str())
                    {
                        if let Ok(result) = execute_bash_tool(command).await {
                            results.push(result);
                        }
                    }
                }
                _ => {
                    // Unknown tool
                    results.push(ToolCallResult {
                        tool: tool_call.tool.clone(),
                        success: false,
                        output: format!("Unknown tool: {}", tool_call.tool),
                    });
                }
            }
        }

        if !results.is_empty() {
            if self.debug {
                debug_print(&format!(
                    "DEBUG: execute_tools - Setting pending_tool_results with {} results",
                    results.len()
                ));
            }
            self.pending_tool_results = Some(results);
        } else if self.debug {
            debug_print("DEBUG: execute_tools - No results to set");
        }
    }

    pub fn get_pending_tool_calls(&mut self) -> Option<Vec<ToolCall>> {
        self.pending_tool_calls.take()
    }

    pub fn has_pending_tool_calls(&self) -> bool {
        self.pending_tool_calls.is_some()
    }

    pub fn get_pending_tool_results(&mut self) -> Option<Vec<ToolCallResult>> {
        let results = self.pending_tool_results.take();
        if self.debug {
            debug_print(&format!(
                "DEBUG: get_pending_tool_results - returning {} results",
                results.as_ref().map_or(0, |r| r.len())
            ));
        }
        results
    }

    /// Cancel the current API request
    pub fn cancel_request(&mut self) {
        self.cancellation_token.cancel();

        // Abort the task if it's still running
        if let Some(handle) = self.current_task_handle.take() {
            handle.abort();
        }

        // Create a new token for future requests
        self.cancellation_token = CancellationToken::new();
        // Clear the response receiver so is_waiting_for_response() returns false
        self.ai_response_rx = None;

        // Note: Git branch restoration on cancel would require async context
        // For now, we'll let the state be restored on next app startup
        eprintln!("🔧 GitState: Cancelled - git branch will be restored on next startup");
    }

    /// Get cached OpenRouter models, returning None if not cached
    pub fn get_cached_openrouter_models(&self) -> Option<Vec<String>> {
        match self.openrouter_models.lock() {
            Ok(cache) => cache.clone(),
            Err(e) => {
                eprintln!("Failed to lock OpenRouter models cache for reading: {}", e);
                None
            }
        }
    }

    /// Cache OpenRouter models
    pub fn cache_openrouter_models(&self, models: Vec<String>) {
        match self.openrouter_models.lock() {
            Ok(mut cache) => {
                *cache = Some(models);
            }
            Err(e) => {
                eprintln!("Failed to lock OpenRouter models cache for writing: {}", e);
            }
        }
    }

    /// Fetch OpenRouter models asynchronously (runs in background)
    pub fn fetch_openrouter_models(&self) {
        let api_key = self.config.get_api_key();
        let models_cache = self.openrouter_models.clone();

        // Clear existing cache first
        if let Ok(mut cache) = models_cache.lock() {
            *cache = None;
        }

        // Use Handle::current to get current runtime handle
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                // Fetch models in background
                let result = Self::fetch_openrouter_models_async(&api_key).await;
                match models_cache.lock() {
                    Ok(mut cache) => *cache = Some(result),
                    Err(_) => {
                        // Cache lock failed
                    }
                }
            });
        } else {
            // No runtime - show error in cache
            if let Ok(mut cache) = models_cache.lock() {
                *cache = Some(vec!["⚠️ No tokio runtime available".to_string()]);
            }
        }
    }

    /// Async function to fetch OpenRouter models
    async fn fetch_openrouter_models_async(api_key: &str) -> Vec<String> {
        use reqwest::Client;
        use std::time::Duration;

        // Create HTTP client
        let client = match Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arula-cli/1.0")
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                return vec![format!("⚠️ Failed to create HTTP client: {}", e)];
            }
        };

        // Build request
        let mut request = client.get("https://openrouter.ai/api/v1/models");

        // Add authorization header if API key is provided
        if !api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        // Make request
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    match response.json::<Value>().await {
                        Ok(json) => {
                            let mut models = Vec::new();

                            // Parse the response
                            if let Some(data) = json["data"].as_array() {
                                for model_info in data {
                                    if let Some(id) = model_info["id"].as_str() {
                                        // Filter for text-based models
                                        if let Some(architecture) =
                                            model_info["architecture"].as_object()
                                        {
                                            if let Some(modality) =
                                                architecture["modality"].as_str()
                                            {
                                                if modality.contains("text")
                                                    || modality.contains("text->text")
                                                {
                                                    models.push(id.to_string());
                                                }
                                            }
                                        } else {
                                            // Fallback: include if no architecture info
                                            models.push(id.to_string());
                                        }
                                    }
                                }
                            }

                            // Sort models alphabetically
                            models.sort();
                            models
                        }
                        Err(e) => {
                            vec![format!("⚠️ Failed to parse OpenRouter response: {}", e)]
                        }
                    }
                } else {
                    vec![format!("⚠️ OpenRouter API error: Status {}", status)]
                }
            }
            Err(e) => {
                vec![format!("⚠️ Failed to fetch OpenRouter models: {}", e)]
            }
        }
    }

    /// Get cached OpenAI models, returning None if not cached
    pub fn get_cached_openai_models(&self) -> Option<Vec<String>> {
        match self.openai_models.lock() {
            Ok(models) => models.clone(),
            Err(e) => {
                eprintln!("Failed to lock OpenAI models cache for reading: {}", e);
                None
            }
        }
    }

    /// Cache OpenAI models
    pub fn cache_openai_models(&self, models: Vec<String>) {
        match self.openai_models.lock() {
            Ok(mut models_cache) => {
                *models_cache = Some(models);
            }
            Err(e) => {
                eprintln!("Failed to lock OpenAI models cache for writing: {}", e);
            }
        }
    }

    /// Fetch OpenAI models asynchronously (runs in background)
    pub fn fetch_openai_models(&self) {
        let models_cache = self.openai_models.clone();
        let api_key = self.config.get_api_key();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                // Fetch models in background
                let result = Self::fetch_openai_models_async(&api_key).await;
                match models_cache.lock() {
                    Ok(mut cache) => *cache = Some(result),
                    Err(_) => {
                        // Cache lock failed - show error
                    }
                }
            });
        } else {
            // No runtime - show error in cache
            if let Ok(mut cache) = models_cache.lock() {
                *cache = Some(vec!["⚠️ No tokio runtime available".to_string()]);
            }
        }
    }

    /// Async function to fetch OpenAI models
    async fn fetch_openai_models_async(api_key: &str) -> Vec<String> {
        use reqwest::Client;
        use std::time::Duration;

        let client = match Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arula-cli/1.0")
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                return vec![format!("⚠️ Failed to create HTTP client: {}", e)];
            }
        };

        let mut request = client.get("https://api.openai.com/v1/models");

        if !api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    match response.json::<Value>().await {
                        Ok(json) => {
                            let mut models = Vec::new();
                            if let Some(data) = json["data"].as_array() {
                                for model_info in data {
                                    if let Some(id) = model_info["id"].as_str() {
                                        // Filter for chat models (gpt-*)
                                        if id.starts_with("gpt-") && !id.contains("-realtime-") {
                                            models.push(id.to_string());
                                        }
                                    }
                                }
                            }
                            models.sort();
                            models
                        }
                        Err(e) => {
                            vec![format!("⚠️ Failed to parse OpenAI response: {}", e)]
                        }
                    }
                } else {
                    vec![format!("⚠️ OpenAI API error: Status {}", status)]
                }
            }
            Err(e) => {
                vec![format!("⚠️ Failed to fetch OpenAI models: {}", e)]
            }
        }
    }

    /// Get cached Anthropic models, returning None if not cached
    pub fn get_cached_anthropic_models(&self) -> Option<Vec<String>> {
        match self.anthropic_models.lock() {
            Ok(models) => models.clone(),
            Err(e) => {
                eprintln!("Failed to lock Anthropic models cache for reading: {}", e);
                None
            }
        }
    }

    /// Cache Anthropic models
    pub fn cache_anthropic_models(&self, models: Vec<String>) {
        match self.anthropic_models.lock() {
            Ok(mut models_cache) => {
                *models_cache = Some(models);
            }
            Err(e) => {
                eprintln!("Failed to lock Anthropic models cache for writing: {}", e);
            }
        }
    }

    /// Fetch Anthropic models asynchronously (runs in background)
    pub fn fetch_anthropic_models(&self) {
        let models_cache = self.anthropic_models.clone();
        let api_key = self.config.get_api_key();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                // Fetch models in background
                let result = Self::fetch_anthropic_models_async(&api_key).await;
                match models_cache.lock() {
                    Ok(mut cache) => *cache = Some(result),
                    Err(_) => {
                        // Cache lock failed
                    }
                }
            });
        } else {
            // No runtime - show error in cache
            if let Ok(mut cache) = models_cache.lock() {
                *cache = Some(vec!["⚠️ No tokio runtime available".to_string()]);
            }
        }
    }

    /// Async function to fetch Anthropic models
    async fn fetch_anthropic_models_async(_api_key: &str) -> Vec<String> {
        // Anthropic doesn't have a public models endpoint, so return known models
        vec![
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
        ]
    }

    /// Get cached Ollama models, returning None if not cached
    pub fn get_cached_ollama_models(&self) -> Option<Vec<String>> {
        match self.ollama_models.lock() {
            Ok(models) => models.clone(),
            Err(e) => {
                eprintln!("Failed to lock Ollama models cache for reading: {}", e);
                None
            }
        }
    }

    /// Cache Ollama models
    pub fn cache_ollama_models(&self, models: Vec<String>) {
        match self.ollama_models.lock() {
            Ok(mut models_cache) => {
                *models_cache = Some(models);
            }
            Err(e) => {
                eprintln!("Failed to lock Ollama models cache for writing: {}", e);
            }
        }
    }

    /// Fetch Ollama models asynchronously (runs in background)
    pub fn fetch_ollama_models(&self) {
        let models_cache = self.ollama_models.clone();
        let api_url = self.config.get_api_url();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                // Fetch models in background
                let result = Self::fetch_ollama_models_async(&api_url).await;
                match models_cache.lock() {
                    Ok(mut cache) => *cache = Some(result),
                    Err(_) => {
                        // Cache lock failed
                    }
                }
            });
        } else {
            // No runtime - show error in cache
            if let Ok(mut cache) = models_cache.lock() {
                *cache = Some(vec!["⚠️ No tokio runtime available".to_string()]);
            }
        }
    }

    /// Check if the current request is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    pub async fn execute_bash_command(&self, command: &str) -> Result<String> {
        use std::process::Command;

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", command]).output()?
        } else {
            Command::new("sh").arg("-c").arg(command).output()?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(if stdout.is_empty() {
                "Command executed successfully".to_string()
            } else {
                stdout
            })
        } else {
            Err(anyhow::anyhow!(
                "{}",
                if stderr.is_empty() {
                    "Command failed".to_string()
                } else {
                    stderr
                }
            ))
        }
    }

    /// Get cached Z.AI models, returning None if not cached
    pub fn get_cached_zai_models(&self) -> Option<Vec<String>> {
        match self.zai_models.lock() {
            Ok(models) => models.clone(),
            Err(e) => {
                eprintln!("Failed to lock Z.AI models cache for reading: {}", e);
                None
            }
        }
    }

    /// Cache Z.AI models
    pub fn cache_zai_models(&self, models: Vec<String>) {
        match self.zai_models.lock() {
            Ok(mut models_cache) => {
                *models_cache = Some(models);
            }
            Err(e) => {
                eprintln!("Failed to lock Z.AI models cache for writing: {}", e);
            }
        }
    }

    /// Fetch Z.AI models asynchronously (runs in background)
    pub fn fetch_zai_models(&self) {
        let models_cache = self.zai_models.clone();
        let api_key = self.config.get_api_key();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                // Fetch models in background
                let result = Self::fetch_zai_models_async(&api_key).await;
                match models_cache.lock() {
                    Ok(mut cache) => *cache = Some(result),
                    Err(_) => {
                        // Cache lock failed
                    }
                }
            });
        } else {
            // No runtime - show error in cache
            if let Ok(mut cache) = models_cache.lock() {
                *cache = Some(vec!["⚠️ No tokio runtime available".to_string()]);
            }
        }
    }

    /// Async function to fetch Z.AI models
    async fn fetch_zai_models_async(_api_key: &str) -> Vec<String> {
        // Z.AI doesn't have a public models endpoint, so return known models
        vec![
            "GLM-4.6".to_string(),
            "GLM-4.5".to_string(),
            "GLM-4.5-AIR".to_string(),
        ]
    }

    /// Async function to fetch Ollama models
    async fn fetch_ollama_models_async(api_url: &str) -> Vec<String> {
        use reqwest::Client;
        use std::time::Duration;

        let client = match Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arula-cli/1.0")
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                return vec![format!("⚠️ Failed to create HTTP client: {}", e)];
            }
        };

        let request = client.get(format!("{}/api/tags", api_url));

        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    match response.json::<Value>().await {
                        Ok(json) => {
                            let mut models = Vec::new();
                            if let Some(models_data) = json["models"].as_array() {
                                for model_info in models_data {
                                    if let Some(name) = model_info["name"].as_str() {
                                        models.push(name.to_string());
                                    }
                                }
                            }
                            models.sort();
                            models
                        }
                        Err(e) => {
                            vec![format!("⚠️ Failed to parse Ollama response: {}", e)]
                        }
                    }
                } else {
                    vec![format!("⚠️ Ollama API error: Status {}", status)]
                }
            }
            Err(e) => {
                vec![format!("⚠️ Failed to fetch Ollama models: {}", e)]
            }
        }
    }

    fn remove_code_blocks(text: &str) -> String {
        let mut result = String::new();
        let mut in_code_block = false;

        for line in text.lines() {
            if line.trim().starts_with("```") {
                in_code_block = !in_code_block;
            } else if !in_code_block {
                result.push_str(line);
                result.push('\n');
            }
        }

        result.trim().to_string()
    }

    fn remove_tool_calls_and_code_blocks(text: &str) -> String {
        let mut result = String::new();
        let mut in_code_block = false;
        let mut in_json_block = false;
        let mut brace_count = 0;

        for line in text.lines() {
            let trimmed = line.trim();

            // Check for code blocks
            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }

            // Skip if in code block
            if in_code_block {
                continue;
            }

            // Check for JSON tool calls
            if trimmed.starts_with("{") {
                in_json_block = true;
                brace_count = 0;
            }

            if in_json_block {
                // Count braces to find end of JSON
                for ch in line.chars() {
                    if ch == '{' {
                        brace_count += 1;
                    } else if ch == '}' {
                        brace_count -= 1;
                        if brace_count == 0 {
                            in_json_block = false;
                            break;
                        }
                    }
                }
                continue;
            }

            // Add line to result
            result.push_str(line);
            result.push('\n');
        }

        // Clean up extra whitespace
        result.trim().to_string()
    }

    // ============================================================================
    // Conversation Management Methods
    // ============================================================================

    /// Start a new conversation or get current one
    pub fn ensure_conversation(&mut self) {
        if self.current_conversation.is_none() {
            use crate::utils::conversation::Conversation;

            let provider_config = self
                .config
                .get_active_provider_config()
                .expect("Active provider not found");
            let model = provider_config.model.clone();
            let provider = self.config.active_provider.clone();
            let endpoint = provider_config.api_url.clone().unwrap_or_default();

            let mut conversation = Conversation::new(model, provider, endpoint);

            // Auto-generate title from first user message if we have messages
            if !self.messages.is_empty() {
                if let Some(first_msg) = self.messages.first() {
                    if first_msg.message_type == crate::utils::chat::MessageType::User {
                        let title = Self::generate_conversation_title(&first_msg.content);
                        conversation.set_title(title);
                    }
                }
            }

            self.current_conversation = Some(conversation.clone());

            // Also update shared conversation for background tasks
            if let Ok(mut shared) = self.shared_conversation.lock() {
                *shared = Some(conversation);
            }
        }
    }

    /// Generate a title from the first user message
    fn generate_conversation_title(content: &str) -> String {
        let max_len = 50;
        let cleaned = content.trim().lines().next().unwrap_or("New Conversation");

        if cleaned.len() <= max_len {
            cleaned.to_string()
        } else {
            format!("{}...", &cleaned[..max_len - 3])
        }
    }

    /// Save current conversation to disk
    pub fn save_conversation(&mut self) -> Result<()> {
        if let Some(ref mut conv) = self.current_conversation {
            conv.update_duration();
            let current_dir = std::env::current_dir()?;
            conv.save(&current_dir)?;
        }
        Ok(())
    }

    /// Load a conversation from disk
    pub fn load_conversation(&mut self, conversation_id: &str) -> Result<()> {
        use crate::utils::chat::MessageType;
        use crate::utils::conversation::Conversation;

        let current_dir = std::env::current_dir()?;
        let conversation = Conversation::load(&current_dir, conversation_id)?;

        // Convert conversation messages to chat messages
        self.messages.clear();
        for msg in &conversation.messages {
            match msg.role.as_str() {
                "user" => {
                    if let Some(content) = &msg.content {
                        if let Some(text) = content.as_str() {
                            self.messages
                                .push(ChatMessage::new(MessageType::User, text.to_string()));
                        }
                    }
                }
                "assistant" => {
                    if let Some(content) = &msg.content {
                        if let Some(text) = content.as_str() {
                            self.messages
                                .push(ChatMessage::new(MessageType::Arula, text.to_string()));
                        }
                    }

                    // Load tool calls from assistant message
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tool_call in tool_calls {
                            // Create a JSON representation of the tool call for display
                            let tool_call_json = serde_json::json!({
                                "id": tool_call.id,
                                "name": tool_call.name,
                                "arguments": tool_call.arguments,
                            });
                            self.messages.push(ChatMessage::new(
                                MessageType::ToolCall,
                                tool_call_json.to_string(),
                            ));
                        }
                    }
                }
                "tool" => {
                    // Tool result messages
                    if let Some(content) = &msg.content {
                        // Convert the content (which is JSON) to a string for display
                        let content_str = if content.is_object() || content.is_array() {
                            serde_json::to_string_pretty(content)
                                .unwrap_or_else(|_| content.to_string())
                        } else if let Some(text) = content.as_str() {
                            text.to_string()
                        } else {
                            content.to_string()
                        };

                        self.messages
                            .push(ChatMessage::new(MessageType::ToolResult, content_str));
                    }
                }
                _ => {}
            }
        }

        self.current_conversation = Some(conversation);
        Ok(())
    }

    /// Track user message in conversation
    pub fn track_user_message(&mut self, content: &str) {
        self.ensure_conversation();

        // FIRST: Sync FROM shared_conversation TO current_conversation
        // Only copy if shared has MORE messages (i.e., new AI responses from tokio task)
        if let Ok(shared) = self.shared_conversation.lock() {
            if let Some(ref shared_conv) = *shared {
                if let Some(ref mut conv) = self.current_conversation {
                    // Only update if shared has more messages than current
                    if shared_conv.messages.len() > conv.messages.len() {
                        debug_print(&format!(
                            "DEBUG: Syncing {} new messages from shared to current",
                            shared_conv.messages.len() - conv.messages.len()
                        ));
                        // Copy ALL messages from shared to current to stay in sync
                        *conv = shared_conv.clone();
                    }
                }
            }
        }

        // THEN: Add user message to current_conversation
        if let Some(ref mut conv) = self.current_conversation {
            conv.add_user_message(content.to_string());

            // Sync back to shared conversation
            if let Ok(mut shared) = self.shared_conversation.lock() {
                if let Some(ref mut shared_conv) = *shared {
                    shared_conv.add_user_message(content.to_string());
                }
            }

            if self.auto_save_conversations {
                let _ = self.save_conversation();
            }
        }
    }

    /// Track assistant message in conversation
    pub fn track_assistant_message(&mut self, content: &str) {
        self.ensure_conversation();
        if let Some(ref mut conv) = self.current_conversation {
            conv.add_assistant_message(content.to_string(), None);

            // Sync with shared conversation (don't add if already added by tokio task)
            if let Ok(mut shared) = self.shared_conversation.lock() {
                if let Some(ref mut shared_conv) = *shared {
                    // Copy the entire conversation to shared to keep in sync
                    *shared_conv = conv.clone();
                }
            }

            if self.auto_save_conversations {
                let _ = self.save_conversation();
            }
        }
    }

    /// Track tool call in conversation
    pub fn track_tool_call(&mut self, tool_call_id: String, tool_name: String, arguments: String) {
        self.ensure_conversation();
        if let Some(ref mut conv) = self.current_conversation {
            use crate::utils::conversation::ToolCall;
            use chrono::Utc;

            let tool_call = ToolCall {
                id: tool_call_id,
                name: tool_name,
                arguments,
                timestamp: Utc::now(),
            };

            // Find the last assistant message and add tool call to it
            if let Some(last_msg) = conv.messages.last_mut() {
                if last_msg.role == "assistant" {
                    if let Some(ref mut calls) = last_msg.tool_calls {
                        calls.push(tool_call);
                    } else {
                        last_msg.tool_calls = Some(vec![tool_call]);
                    }
                }
            }

            if self.auto_save_conversations {
                let _ = self.save_conversation();
            }
        }
    }

    /// Track tool result in conversation
    pub fn track_tool_result(
        &mut self,
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
        success: bool,
        execution_time_ms: u64,
    ) {
        self.ensure_conversation();
        if let Some(ref mut conv) = self.current_conversation {
            conv.add_tool_result(tool_call_id, tool_name, result, success, execution_time_ms);

            if self.auto_save_conversations {
                let _ = self.save_conversation();
            }
        }
    }

    /// Start a new conversation (keeps current messages in memory but starts fresh tracking)
    pub fn new_conversation(&mut self) {
        use crate::utils::conversation::Conversation;

        let provider_config = self
            .config
            .get_active_provider_config()
            .expect("Active provider not found");
        let model = provider_config.model.clone();
        let provider = self.config.active_provider.clone();
        let endpoint = provider_config.api_url.clone().unwrap_or_default();

        self.current_conversation = Some(Conversation::new(model, provider, endpoint));
    }

    /// Build built-in tools information for the AI
    fn build_builtin_tools_info(&self) -> String {
        let mut info = String::new();

        info.push_str("\n## Built-in Tools\n");
        info.push_str(
            "You can call these tools directly; they will run without asking for extra approval unless noted:\n\n",
        );

        info.push_str("1) execute_bash — run shell commands\n");
        info.push_str("- `command` (string, required) — shell to execute\n");
        info.push_str("  Example: `execute_bash(command=\"echo hello && ls\")`\n\n");

        info.push_str("2) list_directory — list files/directories\n");
        info.push_str("- `path` (string, required) — directory to list\n");
        info.push_str("  Example: `list_directory(path=\".\")`\n\n");

        info.push_str("3) read_file — read a file\n");
        info.push_str("- `path` (string, required) — file to read\n");
        info.push_str("  Example: `read_file(path=\"README.md\")`\n\n");

        info.push_str("4) write_file — create/overwrite a file\n");
        info.push_str("- `path` (string, required) — file to write\n");
        info.push_str("- `content` (string, required) — data to write\n");
        info.push_str("  Example: `write_file(path=\"hello.txt\", content=\"Hello World\")`\n\n");

        info.push_str("5) edit_file — find/replace text in a file\n");
        info.push_str("- `path` (string, required)\n");
        info.push_str("- `old_text` (string, required)\n");
        info.push_str("- `new_text` (string, required)\n");
        info.push_str("  Example: `edit_file(path=\"file.txt\", old_text=\"old\", new_text=\"new\")`\n\n");

        info.push_str("6) search_files — regex search in files\n");
        info.push_str("- `path` (string, required) — root directory\n");
        info.push_str("- `pattern` (string, required) — regex to search\n");
        info.push_str("- `extensions` (string list, optional) — limit to extensions\n");
        info.push_str("- `max_results` (number, optional) — cap results\n");
        info.push_str("  Example: `search_files(path=\".\", pattern=\"TODO\", extensions=[\"rs\"], max_results=20)`\n\n");

        info.push_str("7) web_search — search the web\n");
        info.push_str("- `query` (string, required)\n");
        info.push_str("- `limit` (number, optional)\n");
        info.push_str("  Example: `web_search(query=\"latest rust release\", limit=3)`\n\n");

        info.push_str("8) visioneer — vision/automation helper\n");
        info.push_str("- `task` (string, required) — describe what to inspect or automate\n");
        info.push_str("- optional: `model`, `endpoint`, `region` depending on provider\n\n");

        info.push_str("9) analyze_context — summarize repo structure\n");
        info.push_str("- `root_path` (string, optional) — directory to scan (default: \".\")\n");
        info.push_str("- `max_files` (number, optional) — file scan cap (default: 500)\n");
        info.push_str("- `include_hidden` (boolean, optional) — scan hidden/build outputs\n");
        info.push_str("  Example: `analyze_context(root_path=\".\", max_files=400)`\n\n");

        info.push_str("10) ask_question — ask a short clarifying question\n");
        info.push_str("- `question` (string, required)\n");
        info.push_str("  Example: `ask_question(question=\"Which file should I edit?\")`\n\n");

        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::Config;
    use serde_json::json;

    fn create_test_app() -> App {
        let (_tracking_tx, tracking_rx) = std::sync::mpsc::channel();
        App {
            config: Config::default(),
            agent_client: None,
            messages: Vec::new(),
            ai_response_rx: None,
            current_streaming_message: None,
            pending_bash_commands: None,
            pending_tool_results: None,
            pending_tool_calls: None,
            debug: false,
            cancellation_token: CancellationToken::new(),
            current_task_handle: None,
            openrouter_models: Arc::new(Mutex::new(None)),
            openai_models: Arc::new(Mutex::new(None)),
            anthropic_models: Arc::new(Mutex::new(None)),
            ollama_models: Arc::new(Mutex::new(None)),
            zai_models: Arc::new(Mutex::new(None)),
            current_conversation: None,
            auto_save_conversations: false,
            tracking_rx: Some(tracking_rx),
            tracking_tx: None,
            shared_conversation: Arc::new(Mutex::new(None)),
            cached_tool_registry: None,
            git_state_tracker: GitStateTracker::new("."),
        }
    }

    #[test]
    fn test_app_creation() {
        let app = create_test_app();
        assert!(app.messages.is_empty());
        assert!(app.current_streaming_message.is_none());
        assert!(app.pending_bash_commands.is_none());
        assert!(app.pending_tool_results.is_none());
        assert!(app.pending_tool_calls.is_none());
        assert!(!app.debug);
    }

    #[test]
    fn test_debug_print() {
        // Should not panic with debug flag unset
        debug_print("test message");

        // Set debug flag
        std::env::set_var("ARULA_DEBUG", "1");
        debug_print("debug message");

        // Clean up
        std::env::remove_var("ARULA_DEBUG");
    }

    #[test]
    fn test_ai_response_variants() {
        // Test all AiResponse variants can be created
        let stream_start = AiResponse::AgentStreamStart;
        let stream_text = AiResponse::AgentStreamText("Hello".to_string());
        let tool_call = AiResponse::AgentToolCall {
            id: "call_1".to_string(),
            name: "bash_tool".to_string(),
            arguments: "{\"command\": \"echo hello\"}".to_string(),
        };
        let tool_result = AiResponse::AgentToolResult {
            tool_call_id: "call_1".to_string(),
            success: true,
            result: json!("hello"),
        };
        let stream_end = AiResponse::AgentStreamEnd;

        // Verify they can be debug formatted
        assert!(format!("{:?}", stream_start).contains("AgentStreamStart"));
        assert!(format!("{:?}", stream_text).contains("Hello"));
        assert!(format!("{:?}", tool_call).contains("bash_tool"));
        assert!(format!("{:?}", tool_result).contains("call_1"));
        assert!(format!("{:?}", stream_end).contains("AgentStreamEnd"));
    }

    #[test]
    fn test_config_integration() {
        let mut config = Config::default();
        config.set_model("test-model");

        let (_tracking_tx, tracking_rx) = std::sync::mpsc::channel();
        let app = App {
            config,
            agent_client: None,
            messages: Vec::new(),
            ai_response_rx: None,
            current_streaming_message: None,
            pending_bash_commands: None,
            pending_tool_results: None,
            pending_tool_calls: None,
            debug: true,
            cancellation_token: CancellationToken::new(),
            current_task_handle: None,
            openrouter_models: Arc::new(Mutex::new(None)),
            openai_models: Arc::new(Mutex::new(None)),
            anthropic_models: Arc::new(Mutex::new(None)),
            ollama_models: Arc::new(Mutex::new(None)),
            zai_models: Arc::new(Mutex::new(None)),
            current_conversation: None,
            auto_save_conversations: false,
            tracking_rx: Some(tracking_rx),
            tracking_tx: None,
            shared_conversation: Arc::new(Mutex::new(None)),
            cached_tool_registry: None,
            git_state_tracker: GitStateTracker::new("."),
        };

        assert_eq!(app.config.get_model(), "test-model");
        assert!(app.debug);
    }

    #[test]
    fn test_cancellation_token() {
        let app = create_test_app();

        // Token should not be cancelled initially
        assert!(!app.cancellation_token.is_cancelled());

        // Cancel the token
        app.cancellation_token.cancel();
        assert!(app.cancellation_token.is_cancelled());
    }

    #[tokio::test]
    async fn test_channel_cleanup() {
        let app = create_test_app();

        // Create a channel and assign it to the app
        let (tx, rx) = mpsc::unbounded_channel();

        // Simulate dropping the sender
        drop(tx);

        // The receiver should still work but return None when trying to receive
        let mut app_with_rx = app;
        app_with_rx.ai_response_rx = Some(rx);

        // Try to receive from the closed channel
        if let Some(mut rx) = app_with_rx.ai_response_rx.take() {
            let result = rx.try_recv();
            assert!(result.is_err());
        }
    }
}
