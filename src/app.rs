use crate::api::agent::{AgentOptionsBuilder, ContentBlock};
use crate::api::agent_client::AgentClient;
use crate::utils::chat::{ChatMessage, MessageType};
use crate::utils::config::Config;
use crate::utils::tool_call::{execute_bash_tool, ToolCall, ToolCallResult};
use anyhow::Result;
use chrono::Utc;
use futures::StreamExt;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use termimad::MadSkin;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Render markdown inline using termimad for ExternalPrinter output
fn render_markdown_line(line: &str) -> String {
    let mut skin = MadSkin::default();

    // Configure colors similar to OutputHandler
    use termimad::crossterm::style::Color as TMColor;
    skin.bold.set_fg(TMColor::Yellow);
    skin.italic.set_fg(TMColor::Cyan);
    skin.inline_code.set_fg(TMColor::Cyan);
    skin.code_block.set_fg(TMColor::Green);

    // Render inline markdown
    skin.inline(line).to_string()
}

/// Format a code block with simple borders for ExternalPrinter
fn format_code_block(content: &str) -> String {
    use nu_ansi_term::Color;

    let mut result = String::new();

    // Top border
    result.push_str(&Color::DarkGray.paint("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓\n").to_string());

    // Content lines with borders
    for line in content.lines() {
        result.push_str(&Color::DarkGray.paint("┃ ").to_string());
        result.push_str(&Color::White.paint(line).to_string());
        result.push('\n');
    }

    // Bottom border
    result.push_str(&Color::DarkGray.paint("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛\n").to_string());

    result
}

/// Format tool call with icon and human-readable description
fn format_tool_call(tool_name: &str, arguments: &str) -> String {
    use nu_ansi_term::Color;

    // Parse arguments to extract key information
    let args: Result<Value, _> = serde_json::from_str(arguments);

    let (icon, description) = match tool_name {
        "list_directory" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            ("📂", format!("Listing directory: {}", path))
        },
        "read_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            ("📖", format!("Reading file: {}", path))
        },
        "write_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            ("✍️", format!("Writing file: {}", path))
        },
        "edit_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            ("✏️", format!("Editing file: {}", path))
        },
        "execute_bash" => {
            let command = args.as_ref()
                .ok()
                .and_then(|v| v.get("command"))
                .and_then(|c| c.as_str())
                .unwrap_or("unknown");
            // Truncate long commands
            let display_cmd = if command.len() > 50 {
                format!("{}...", &command[..47])
            } else {
                command.to_string()
            };
            ("⚙️", format!("Running: {}", display_cmd))
        },
        "search_files" => {
            let query = args.as_ref()
                .ok()
                .and_then(|v| v.get("query"))
                .and_then(|q| q.as_str())
                .unwrap_or("unknown");
            ("🔍", format!("Searching for: {}", query))
        },
        _ => ("🔧", format!("Running tool: {}", tool_name))
    };

    // Format with loading spinner and colored description
    format!("{} {}",
        Color::Cyan.paint(icon),
        Color::White.dimmed().paint(description)
    )
}

/// Summarize tool result in a human-readable format
fn summarize_tool_result(result_value: &Value) -> String {
    // Check for error in Err wrapper first (e.g., {"Err": "error message"})
    if let Some(err_value) = result_value.get("Err") {
        if let Some(err_str) = err_value.as_str() {
            return format!("Error: {}", err_str);
        }
        // If Err value is not a string, show it as JSON
        return format!("Error: {}", serde_json::to_string_pretty(err_value).unwrap_or_else(|_| err_value.to_string()));
    }

    // Try to parse as our standard tool result format
    if let Some(ok_result) = result_value.get("Ok") {
        // list_directory results
        if let Some(entries) = ok_result.get("entries") {
            if let Some(arr) = entries.as_array() {
                let files = arr.iter().filter(|e| e.get("file_type").and_then(|t| t.as_str()) == Some("file")).count();
                let dirs = arr.iter().filter(|e| e.get("file_type").and_then(|t| t.as_str()) == Some("directory")).count();
                return format!("Found {} files and {} directories", files, dirs);
            }
        }

        // execute_bash results
        if let Some(stdout) = ok_result.get("stdout") {
            if let Some(stdout_str) = stdout.as_str() {
                let stderr = ok_result.get("stderr").and_then(|s| s.as_str()).unwrap_or("");
                let exit_code = ok_result.get("exit_code").and_then(|c| c.as_i64()).unwrap_or(0);

                if exit_code == 0 {
                    if !stdout_str.trim().is_empty() {
                        return format!("Command succeeded:\n{}", stdout_str.trim());
                    } else {
                        return "Command succeeded (no output)".to_string();
                    }
                } else {
                    return format!("Command failed (exit code {}):\n{}", exit_code, stderr);
                }
            }
        }

        // read_file results
        if let Some(lines) = ok_result.get("lines") {
            return format!("Read {} lines", lines);
        }

        // write_file/edit_file results
        if let Some(message) = ok_result.get("message") {
            if let Some(msg_str) = message.as_str() {
                return msg_str.to_string();
            }
        }

        // search_files results
        if let Some(total_matches) = ok_result.get("total_matches") {
            let files_searched = ok_result.get("files_searched").and_then(|f| f.as_i64()).unwrap_or(0);
            return format!("Found {} matches in {} files", total_matches, files_searched);
        }

        // Generic success with success flag
        if ok_result.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            return "Success".to_string();
        }
    }

    // Fallback: show compact JSON
    serde_json::to_string_pretty(result_value).unwrap_or_else(|_| result_value.to_string())
}

/// Debug print helper that checks ARULA_DEBUG environment variable
fn debug_print(msg: &str) {
    if std::env::var("ARULA_DEBUG").is_ok() {
        println!("🔧 DEBUG: {}", msg);
    }
}

/// Log AI request/response to .arula/debug.log for debugging
fn log_ai_interaction(request: &str, context: &[crate::api::api::ChatMessage], response_start: Option<&str>) {
    let log_dir = Path::new(".arula");
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(log_dir);
    }

    let log_path = log_dir.join("ai_debug.log");

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let _ = writeln!(file, "\n=== AI Interaction at {} ===", timestamp);
        let _ = writeln!(file, "USER REQUEST: {}", request);
        let _ = writeln!(file, "CONTEXT MESSAGES ({} total):", context.len());

        for (i, msg) in context.iter().enumerate() {
            let _ = writeln!(file, "  [{}]: {} -> {}",
                i, msg.role,
                msg.content.as_ref().unwrap_or(&"(no content)".to_string())
            );
        }

        if let Some(start) = response_start {
            let _ = writeln!(file, "AI RESPONSE START: {}", start);
        }
        let _ = writeln!(file, "=====================================");
    }
}

/// Log AI response chunks to .arula/debug.log
fn log_ai_response_chunk(chunk: &str) {
    let log_path = Path::new(".arula").join("ai_debug.log");

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let _ = writeln!(file, "CHUNK: {}", chunk);
    }
}

/// Log AI response completion to .arula/debug.log
fn log_ai_response_complete(final_response: &str) {
    let log_path = Path::new(".arula").join("ai_debug.log");

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let _ = writeln!(file, "COMPLETE RESPONSE: {}", final_response);
        let _ = writeln!(file, "=== END AI Interaction ===\n");
    }
}

#[derive(Debug, Clone)]
pub enum AiResponse {
    AgentStreamStart,
    AgentStreamText(String),
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
    // ExternalPrinter sender for concurrent output while read_line() is active
    pub external_printer: Option<crossbeam_channel::Sender<String>>,
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
}

impl App {
    pub fn new() -> Result<Self> {
        let config = Config::load_or_default()?;

        // Create persistent tracking channel
        let (tracking_tx, tracking_rx) = std::sync::mpsc::channel();

        Ok(Self {
            config,
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
            external_printer: None,
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
        })
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Set ExternalPrinter sender for concurrent output
    pub fn set_external_printer(&mut self, sender: crossbeam_channel::Sender<String>) {
        self.external_printer = Some(sender);
    }

    /// Build comprehensive system prompt from ARULA.md files
    fn build_system_prompt() -> String {
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

        prompt_parts.join("\n")
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

    pub fn initialize_agent_client(&mut self) -> Result<()> {
        // Initialize modern agent client with default options
        let agent_options = AgentOptionsBuilder::new()
            .system_prompt(&Self::build_system_prompt())
            .model(&self.config.get_model())
            .auto_execute_tools(true)
            .max_tool_iterations(1000)
            .debug(self.debug)
            .build();

        self.agent_client = Some(AgentClient::new(
            self.config.active_provider.clone(),
            self.config.get_api_url(),
            self.config.get_api_key(),
            self.config.get_model(),
            agent_options,
        ));

        Ok(())
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
                debug_print(&format!("DEBUG: send_to_ai - agent_client is None, returning error"));
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
        let track_tx = self.tracking_tx.clone().expect("Tracking channel not initialized");

        // Debug: Print current message count
        debug_print(&format!("DEBUG: Total messages in self.messages: {}", self.messages.len()));
        for (i, msg) in self.messages.iter().enumerate() {
            debug_print(&format!("DEBUG: [{}] {:?} -> {}", i, msg.message_type,
                if msg.content.len() > 50 {
                    // Use char boundaries to safely truncate
                    let safe_end = msg.content.char_indices().nth(50)
                        .map(|(idx, _)| idx)
                        .unwrap_or(msg.content.len());
                    format!("{}...", &msg.content[..safe_end])
                } else {
                    msg.content.clone()
                }
            ));
        }

        // Convert chat messages to API format for agent
        let api_messages: Vec<crate::api::api::ChatMessage> = self
            .messages
            .iter()
            .filter(|m| {
                m.message_type != MessageType::ToolCall && m.message_type != MessageType::ToolResult
            })
            .map(|m| {
                let role = match m.message_type {
                    MessageType::User => "user".to_string(),
                    MessageType::Arula => "assistant".to_string(),
                    _ => "system".to_string(),
                };
                crate::api::api::ChatMessage {
                    role,
                    content: Some(m.content.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                }
            })
            .collect();

        debug_print(&format!("DEBUG: API messages after filtering: {}", api_messages.len()));

        // Log the AI interaction for debugging
        log_ai_interaction(message, &api_messages, None);

        // Send message using modern agent in background
        let msg = message.to_string();
        let cancel_token = self.cancellation_token.clone();
        let external_printer = self.external_printer.clone();
        let shared_conv = self.shared_conversation.clone();
        let auto_save = self.auto_save_conversations;
        let handle = tokio::spawn(async move {
            // Track message content and tool calls for conversation history
            let mut accumulated_text = String::new();
            let mut tool_calls_list: Vec<(String, String, String)> = Vec::new(); // (id, name, args)

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    // Request was cancelled
                    let _ = tx.send(AiResponse::AgentStreamEnd);
                    if let Some(ref printer) = external_printer {
                        let _ = printer.send("\n".to_string());
                    }
                }
                _result = async {
                    match agent_client.query(&msg, Some(api_messages)).await {
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
                                                // Accumulate text for tracking
                                                accumulated_text.push_str(&text);

                                                // Log response chunk
                                                log_ai_response_chunk(&text);

                                                let _ = tx.send(AiResponse::AgentStreamText(text.clone()));
                                                if let Some(ref printer) = external_printer {
                                                    // TODO: Fix terminal scroll positioning
                                                    // Currently, content may not scroll to keep input visible at bottom
                                                    // This needs investigation - may require different approach or reedline config changes

                                                    // Render markdown for each line
                                                    for line in text.lines() {
                                                        let rendered = render_markdown_line(line);
                                                        let _ = printer.send(format!("{}\n", rendered));
                                                    }
                                                    // If text doesn't end with newline, send one anyway
                                                    if !text.ends_with('\n') && !text.is_empty() {
                                                        let _ = printer.send("\n".to_string());
                                                    }
                                                }
                                            }
                                            Some(ContentBlock::ToolCall {
                                                id,
                                                name,
                                                arguments,
                                            }) => {
                                                // Track tool call
                                                tool_calls_list.push((id.clone(), name.clone(), arguments.clone()));

                                                let _ = tx.send(AiResponse::AgentToolCall {
                                                    id: id.clone(),
                                                    name: name.clone(),
                                                    arguments: arguments.clone(),
                                                });
                                                if let Some(ref printer) = external_printer {
                                                    // Format tool call with icon and human-readable name
                                                    let tool_display = format_tool_call(&name, &arguments);
                                                    let _ = printer.send(format!("{}\n", tool_display));
                                                }
                                            }
                                            Some(ContentBlock::ToolResult {
                                                tool_call_id,
                                                result,
                                            }) => {
                                                let status = if result.success { "✓" } else { "✗" };

                                                // Clone result data for all uses
                                                let result_data = result.data.clone();

                                                let _ = tx.send(AiResponse::AgentToolResult {
                                                    tool_call_id: tool_call_id.clone(),
                                                    success: result.success,
                                                    result: result_data.clone(),
                                                });

                                                // Send tracking command for tool result
                                                let _ = track_tx.send(TrackingCommand::ToolResult {
                                                    tool_call_id,
                                                    tool_name: "unknown".to_string(),
                                                    result: result_data.clone(),
                                                    success: result.success,
                                                    execution_time_ms: 100,
                                                });

                                                if let Some(ref printer) = external_printer {
                                                    // Summarize the result in a compact format
                                                    let summary = summarize_tool_result(&result_data);
                                                    let result_display = format!("  {} {}", status, summary);
                                                    let _ = printer.send(format!("{}\n", result_display));
                                                }
                                            }
                                            Some(ContentBlock::Error { error }) => {
                                                // Convert error to AgentStreamText to maintain compatibility
                                                let error_msg = format!("[Error] {}", error);
                                                let _ = tx.send(AiResponse::AgentStreamText(error_msg.clone()));
                                                if let Some(ref printer) = external_printer {
                                                    let _ = printer.send(format!("{}\n", error_msg));
                                                }
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
                            if let Some(ref printer) = external_printer {
                                let _ = printer.send("\n".to_string());
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("**Error:** Failed to send message via agent: {}", e);
                            let _ = tx.send(AiResponse::AgentStreamText(error_msg.clone()));
                            let _ = tx.send(AiResponse::AgentStreamEnd);
                            if let Some(ref printer) = external_printer {
                                let error_line = render_markdown_line(&error_msg);
                                let _ = printer.send(format!("{}\n\n", error_line));
                            }
                        }
                    }
                } => {}
            }
        });

        // Store the task handle so we can abort it on cancellation
        self.current_task_handle = Some(handle);

        Ok(())
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
                debug_print(&format!("DEBUG: Processing {} tracking commands", cmds.len()));
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
                        debug_print(&format!("DEBUG: Tracking assistant message ({} chars)", content.len()));
                    }
                    self.track_assistant_message(&content);
                }
                TrackingCommand::ToolCall { id, name, arguments } => {
                    if self.debug {
                        debug_print(&format!("DEBUG: Tracking tool call: {}", name));
                    }
                    self.track_tool_call(id, name, arguments);
                }
                TrackingCommand::ToolResult { tool_call_id, tool_name, result, success, execution_time_ms } => {
                    if self.debug {
                        debug_print(&format!("DEBUG: Tracking tool result: {} ({})", tool_name, if success { "success" } else { "failed" }));
                    }
                    self.track_tool_result(tool_call_id, tool_name, result, success, execution_time_ms);
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
                                msg.push_str(&text);
                            }
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

                            // Track tool result in conversation (assuming 100ms execution time as placeholder)
                            self.track_tool_result(
                                tool_call_id.clone(),
                                "unknown".to_string(), // Tool name not available in this context
                                result.clone(),
                                *success,
                                100
                            );
                        }
                        AiResponse::AgentStreamEnd => {
                            if let Some(full_message) = self.current_streaming_message.take() {
                                self.messages
                                    .push(ChatMessage::new(MessageType::Arula, full_message.clone()));

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
        } else {
            if self.debug {
                debug_print("DEBUG: execute_tools - No results to set");
            }
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
                                        if let Some(architecture) = model_info["architecture"].as_object() {
                                            if let Some(modality) = architecture["modality"].as_str() {
                                                if modality.contains("text") || modality.contains("text->text") {
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
            "glm-4.6".to_string(),
            "glm-4.5".to_string(),
            "glm-4.5-air".to_string(),
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

        let request = client.get(&format!("{}/api/tags", api_url));

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

            let provider_config = self.config.get_active_provider_config()
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
            format!("{}...", &cleaned[..max_len-3])
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
        use crate::utils::conversation::Conversation;
        use crate::utils::chat::MessageType;

        let current_dir = std::env::current_dir()?;
        let conversation = Conversation::load(&current_dir, conversation_id)?;

        // Convert conversation messages to chat messages
        self.messages.clear();
        for msg in &conversation.messages {
            match msg.role.as_str() {
                "user" => {
                    if let Some(content) = &msg.content {
                        if let Some(text) = content.as_str() {
                            self.messages.push(ChatMessage::new(MessageType::User, text.to_string()));
                        }
                    }
                }
                "assistant" => {
                    if let Some(content) = &msg.content {
                        if let Some(text) = content.as_str() {
                            self.messages.push(ChatMessage::new(MessageType::Arula, text.to_string()));
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
                                tool_call_json.to_string()
                            ));
                        }
                    }
                }
                "tool" => {
                    // Tool result messages
                    if let Some(content) = &msg.content {
                        // Convert the content (which is JSON) to a string for display
                        let content_str = if content.is_object() || content.is_array() {
                            serde_json::to_string_pretty(content).unwrap_or_else(|_| content.to_string())
                        } else if let Some(text) = content.as_str() {
                            text.to_string()
                        } else {
                            content.to_string()
                        };

                        self.messages.push(ChatMessage::new(MessageType::ToolResult, content_str));
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
                        debug_print(&format!("DEBUG: Syncing {} new messages from shared to current",
                            shared_conv.messages.len() - conv.messages.len()));
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
    pub fn track_tool_result(&mut self, tool_call_id: String, tool_name: String, result: serde_json::Value, success: bool, execution_time_ms: u64) {
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

        let provider_config = self.config.get_active_provider_config()
            .expect("Active provider not found");
        let model = provider_config.model.clone();
        let provider = self.config.active_provider.clone();
        let endpoint = provider_config.api_url.clone().unwrap_or_default();

        self.current_conversation = Some(Conversation::new(model, provider, endpoint));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::Config;
    use serde_json::json;

    fn create_test_app() -> App {
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