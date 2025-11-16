use crate::agent::{AgentOptions, AgentOptionsBuilder, ContentBlock};
use crate::agent_client::AgentClient;
use crate::chat::{ChatMessage, MessageType};
use crate::config::Config;
use crate::tool_call::{execute_bash_tool, ToolCall, ToolCallResult};
use anyhow::Result;
use futures::StreamExt;
use std::fs;
use std::path::Path;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Debug print helper that checks ARULA_DEBUG environment variable
fn debug_print(msg: &str) {
    if std::env::var("ARULA_DEBUG").is_ok() {
        eprintln!("{}", msg);
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
}

impl App {
    pub fn new() -> Result<Self> {
        let config = Config::load_or_default()?;

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
        })
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Build comprehensive system prompt from ARULA.md files
    fn build_system_prompt() -> String {
        let mut prompt_parts = Vec::new();

        // Base ARULA personality
        prompt_parts.push("You are ARULA, an Autonomous AI Interface assistant. You help users with coding, shell commands, and general software development tasks. Be concise, helpful, and provide practical solutions.".to_string());

        // Available tools documentation
        prompt_parts.push(r#"
## Available Tools

You have access to the following tools:

1. **execute_bash** - Execute bash/shell commands
   - Use for running commands, checking files, installing packages, etc.
   - Parameters: command (string)

2. **read_file** - Read file contents with optional line range
   - Parameters: path (string), start_line (optional), end_line (optional)

3. **write_file** - Create or overwrite a file with content
   - Parameters: path (string), content (string)

4. **edit_file** - Edit files with various operations (replace, insert, delete, append, prepend)
   - Parameters: path (string), operation (object)

5. **list_directory** - List directory contents with optional recursion
   - Parameters: path (string), show_hidden (bool), recursive (bool)

6. **search_files** - Fast parallel search with gitignore support
   - Search for text patterns across files efficiently
   - Respects .gitignore, .git/info/exclude, and global gitignore
   - Uses parallel walker for fast searching in large codebases
   - Parameters:
     * query (required): Text pattern to search for
     * path (optional): Directory to search (default: current directory)
     * file_pattern (optional): Filter by file pattern (e.g., '*.rs', '*.txt')
     * case_sensitive (optional): Case-sensitive search (default: false)
     * max_results (optional): Maximum results to return (default: 100)
   - Returns: Array of matches with file path, line number, line content, and match positions
   - Use this tool when you need to find text across multiple files or search the codebase

When searching for code or text patterns, prefer using search_files over grep commands for better performance and gitignore support.
"#.to_string());

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
            .model(&self.config.ai.model)
            .auto_execute_tools(true)
            .max_tool_iterations(1000)
            .debug(self.debug)
            .build();

        self.agent_client = Some(AgentClient::new(
            self.config.ai.provider.clone(),
            self.config.ai.api_url.clone(),
            self.config.ai.api_key.clone(),
            self.config.ai.model.clone(),
            agent_options,
        ));

        Ok(())
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    pub fn set_model(&mut self, model: &str) {
        self.config.ai.model = model.to_string();
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

        // Convert chat messages to API format for agent
        let api_messages: Vec<crate::api::ChatMessage> = self
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
                crate::api::ChatMessage {
                    role,
                    content: Some(m.content.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                }
            })
            .collect();

        // Send message using modern agent in background
        let msg = message.to_string();
        tokio::spawn(async move {
            match agent_client.query(&msg, Some(api_messages)).await {
                Ok(mut stream) => {
                    let _ = tx.send(AiResponse::AgentStreamStart);

                    while let Some(content_block) = stream.next().await {
                        match content_block {
                            ContentBlock::Text { text } => {
                                let _ = tx.send(AiResponse::AgentStreamText(text));
                            }
                            ContentBlock::ToolCall {
                                id,
                                name,
                                arguments,
                            } => {
                                let _ = tx.send(AiResponse::AgentToolCall {
                                    id,
                                    name,
                                    arguments,
                                });
                            }
                            ContentBlock::ToolResult {
                                tool_call_id,
                                result,
                            } => {
                                let _ = tx.send(AiResponse::AgentToolResult {
                                    tool_call_id,
                                    success: result.success,
                                    result: result.data,
                                });
                            }
                            ContentBlock::Error { error } => {
                                // Convert error to AgentStreamText to maintain compatibility
                                let _ = tx.send(AiResponse::AgentStreamText(format!(
                                    "[Error] {}",
                                    error
                                )));
                                break;
                            }
                        }
                    }

                    let _ = tx.send(AiResponse::AgentStreamEnd);
                }
                Err(e) => {
                    let _ = tx.send(AiResponse::AgentStreamText(format!(
                        "[Error] Failed to send message via agent: {}",
                        e
                    )));
                    let _ = tx.send(AiResponse::AgentStreamEnd);
                }
            }
        });

        Ok(())
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
                            id: _,
                            name,
                            arguments,
                        } => {
                            // Add tool call message to chat history
                            self.messages.push(ChatMessage::new(
                                MessageType::ToolCall,
                                format!("🔧 Tool call: {}({})", name, arguments),
                            ));
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
                        }
                        AiResponse::AgentStreamEnd => {
                            if let Some(full_message) = self.current_streaming_message.take() {
                                self.messages
                                    .push(ChatMessage::new(MessageType::Arula, full_message));
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
        // Create a new token for future requests
        self.cancellation_token = CancellationToken::new();
        // Clear the response receiver so is_waiting_for_response() returns false
        self.ai_response_rx = None;
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
}
