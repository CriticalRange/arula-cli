use anyhow::Result;
use tokio::sync::mpsc;
use futures::StreamExt;
use crate::api::ApiClient;
use crate::config::Config;
use crate::chat::{ChatMessage, MessageType};
use crate::tool_call::{execute_bash_tool, ToolCall, ToolCallResult};
use crate::agent_client::AgentClient;
use crate::agent::{AgentOptions, AgentOptionsBuilder, ContentBlock};

#[derive(Debug, Clone)]
pub enum AiResponse {
    Success { response: String, usage: Option<crate::api::Usage>, tool_calls: Option<Vec<crate::api::ToolCall>> },
    Error(String),
    StreamStart,
    StreamChunk(String),
    StreamEnd(Option<crate::api::ApiResponse>),
    // New agent-based responses
    AgentStreamStart,
    AgentStreamText(String),
    AgentToolCall { id: String, name: String, arguments: String },
    AgentToolResult { tool_call_id: String, success: bool, result: serde_json::Value },
    AgentStreamEnd,
}

pub struct App {
    pub config: Config,
    pub api_client: Option<ApiClient>,
    pub agent_client: Option<AgentClient>,
    pub messages: Vec<ChatMessage>,
    pub api_messages: Vec<crate::api::ChatMessage>,
    pub ai_response_rx: Option<mpsc::UnboundedReceiver<AiResponse>>,
    pub current_streaming_message: Option<String>,
    pub pending_bash_commands: Option<Vec<String>>,
    pub pending_tool_results: Option<Vec<ToolCallResult>>,
    pub pending_tool_calls: Option<Vec<ToolCall>>,
    pub pending_api_response: Option<crate::api::ApiResponse>,
    // New agent fields
    pub use_modern_agent: bool,
    pub debug: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = Config::load_or_default()?;

        Ok(Self {
            config,
            api_client: None,
            agent_client: None,
            messages: Vec::new(),
            api_messages: Vec::new(),
            ai_response_rx: None,
            current_streaming_message: None,
            pending_bash_commands: None,
            pending_tool_results: None,
            pending_tool_calls: None,
            pending_api_response: None,
            // Enable modern agent by default for better tool calling
            use_modern_agent: true,
            debug: false,
        })
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    pub fn initialize_api_client(&mut self) -> Result<()> {
        let api_client = ApiClient::new(
            self.config.ai.provider.clone(),
            self.config.ai.api_url.clone(),
            self.config.ai.api_key.clone(),
            self.config.ai.model.clone(),
        );

        // Initialize modern agent client with default options
        let agent_options = AgentOptionsBuilder::new()
            .system_prompt("You are ARULA, an Autonomous AI Interface assistant. You help users with coding, shell commands, and general software development tasks. Be concise, helpful, and provide practical solutions.")
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

        self.api_client = Some(api_client);
        Ok(())
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    pub fn set_model(&mut self, model: &str) {
        self.config.ai.model = model.to_string();
        let _ = self.config.save();
        // Reinitialize API clients with new model
        let _ = self.initialize_api_client();
    }

    pub fn clear_conversation(&mut self) {
        self.messages.clear();
    }

    pub async fn send_to_ai(&mut self, message: &str) -> Result<()> {
        // Add user message to history
        self.messages.push(ChatMessage::new(MessageType::User, message.to_string()));

        // Choose which client to use based on configuration
        if self.use_modern_agent {
            self.send_to_ai_with_agent(message).await
        } else {
            self.send_to_ai_legacy(message).await
        }
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
            eprintln!("DEBUG: send_to_ai_with_agent - Created new response receiver for message: '{}'", message);
        }

        // Convert chat messages to API format for agent
        let api_messages: Vec<crate::api::ChatMessage> = self.messages
            .iter()
            .filter(|m| m.message_type != MessageType::ToolCall && m.message_type != MessageType::ToolResult)
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
                            ContentBlock::ToolCall { id, name, arguments } => {
                                let _ = tx.send(AiResponse::AgentToolCall {
                                    id,
                                    name,
                                    arguments,
                                });
                            }
                            ContentBlock::ToolResult { tool_call_id, result } => {
                                let _ = tx.send(AiResponse::AgentToolResult {
                                    tool_call_id,
                                    success: result.success,
                                    result: result.data,
                                });
                            }
                            ContentBlock::Error { error } => {
                                let _ = tx.send(AiResponse::Error(error));
                                break;
                            }
                        }
                    }

                    let _ = tx.send(AiResponse::AgentStreamEnd);
                }
                Err(e) => {
                    let _ = tx.send(AiResponse::Error(format!("Failed to send message via agent: {}", e)));
                }
            }
        });

        Ok(())
    }

    /// Legacy send method for compatibility
    async fn send_to_ai_legacy(&mut self, message: &str) -> Result<()> {
        // Get API client
        let api_client = match &self.api_client {
            Some(client) => client.clone(),
            None => {
                return Err(anyhow::anyhow!("API client not initialized"));
            }
        };

        // Create channel for streaming responses
        let (tx, rx) = mpsc::unbounded_channel();
        self.ai_response_rx = Some(rx);
        if self.debug {
            eprintln!("DEBUG: send_to_ai_legacy - Created new response receiver for message: '{}'", message);
        }

        // Sync messages to api_messages format
        let synced_messages: Vec<crate::api::ChatMessage> = self.messages
            .iter()
            .filter(|m| m.message_type != MessageType::ToolCall)
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

        self.api_messages = self.api_messages
            .iter()
            .filter(|msg| msg.role == "tool")
            .cloned()
            .chain(synced_messages)
            .collect();

        let message_history = self.api_messages.clone();
        let msg = message.to_string();

        // Send message in background
        tokio::spawn(async move {
            match api_client.send_message_stream(&msg, Some(message_history)).await {
                Ok(mut stream_rx) => {
                    let _ = tx.send(AiResponse::StreamStart);

                    while let Some(response) = stream_rx.recv().await {
                        match response {
                            crate::api::StreamingResponse::Start => {}
                            crate::api::StreamingResponse::Chunk(chunk) => {
                                let _ = tx.send(AiResponse::StreamChunk(chunk));
                            }
                            crate::api::StreamingResponse::End(api_response) => {
                                let _ = tx.send(AiResponse::StreamEnd(Some(api_response)));
                                break;
                            }
                            crate::api::StreamingResponse::Error(err) => {
                                let _ = tx.send(AiResponse::Error(err));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AiResponse::Error(format!("Failed to send message: {}", e)));
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
                        AiResponse::StreamStart => {
                            self.current_streaming_message = Some(String::new());
                        }
                        AiResponse::StreamChunk(chunk) => {
                            if let Some(msg) = &mut self.current_streaming_message {
                                msg.push_str(chunk);
                            }
                        }
                        AiResponse::StreamEnd(api_response_opt) => {
                            if let Some(full_message) = self.current_streaming_message.take() {
                                self.messages.push(ChatMessage::new(
                                    MessageType::Arula,
                                    full_message,
                                ));
                            }

                            // Handle tool calls if they exist in the API response
                            if let Some(api_response) = api_response_opt {
                                if let Some(_) = &api_response.tool_calls {
                                    // Store the API response for later processing
                                    self.pending_api_response = Some(api_response.clone());
                                }
                            }

                            self.ai_response_rx = None;
                        }
                        AiResponse::Success { response, usage, tool_calls } => {
                            // Create ApiResponse to pass to execute_tools_and_continue
                            let api_response = crate::api::ApiResponse {
                                response: response.clone(),
                                success: true,
                                error: None,
                                usage: usage.clone(),
                                tool_calls: tool_calls.clone(),
                            };

                            // If there are tool calls, execute them and continue conversation
                            if let Some(_) = &tool_calls {
                                // Store the API response for later processing
                                self.pending_api_response = Some(api_response.clone());
                            } else {
                                // Regular response, just add to messages
                                self.messages.push(ChatMessage::new(
                                    MessageType::Arula,
                                    response.clone(),
                                ));
                            }

                            self.ai_response_rx = None;
                        }
                        AiResponse::Error(_) => {
                            self.ai_response_rx = None;
                        }
                        // New agent-based responses
                        AiResponse::AgentStreamStart => {
                            self.current_streaming_message = Some(String::new());
                        }
                        AiResponse::AgentStreamText(text) => {
                            if let Some(msg) = &mut self.current_streaming_message {
                                msg.push_str(&text);
                            }
                        }
                        AiResponse::AgentToolCall { id: _, name, arguments } => {
                            // Add tool call message to chat history
                            self.messages.push(ChatMessage::new(
                                MessageType::ToolCall,
                                format!("🔧 Tool call: {}({})", name, arguments),
                            ));
                        }
                        AiResponse::AgentToolResult { tool_call_id, success, result } => {
                            // Add tool result message to chat history
                            let status = if *success { "✅" } else { "❌" };
                            let result_text = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());

                            self.messages.push(ChatMessage::new(
                                MessageType::ToolResult,
                                format!("{} Tool result: {}\n{}", status, tool_call_id, result_text),
                            ));
                        }
                        AiResponse::AgentStreamEnd => {
                            if let Some(full_message) = self.current_streaming_message.take() {
                                self.messages.push(ChatMessage::new(
                                    MessageType::Arula,
                                    full_message,
                                ));
                            }
                            self.ai_response_rx = None;
                        }
                    }
                    Some(response)
                }
                Err(mpsc::error::TryRecvError::Empty) => None,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.ai_response_rx = None;
                    Some(AiResponse::Error("AI request failed unexpectedly".to_string()))
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

    pub async fn execute_tools_and_continue(&mut self, api_response: &crate::api::ApiResponse) -> Result<()> {
        if let Some(tool_calls) = &api_response.tool_calls {
            // First, add the assistant message with tool calls to conversation
            self.messages.push(crate::chat::ChatMessage::new(
                MessageType::Arula,
                api_response.response.clone(),
            ));

            // Store assistant's tool calls in the API messages
            for tool_call in tool_calls {
                let assistant_msg = crate::api::ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(api_response.response.clone()),
                    tool_calls: Some(vec![tool_call.clone()]),
                    tool_call_id: None,
                };
                self.api_messages.push(assistant_msg);
            }

            // Execute tools and add tool results to conversation
            for tool_call in tool_calls {
                if tool_call.function.name == "execute_bash" {
                    // Parse arguments
                    let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                        .unwrap_or_default();

                    let command = args["command"].as_str().unwrap_or("echo 'No command'");

                    // Execute bash command
                    match crate::tool_call::execute_bash_tool(command).await {
                        Ok(result) => {
                            // Add tool result message to conversation
                            let tool_msg = crate::api::ChatMessage {
                                role: "tool".to_string(),
                                content: Some(result.output.clone()),
                                tool_calls: None,
                                tool_call_id: Some(tool_call.id.clone()),
                            };
                            self.api_messages.push(tool_msg);

                            // Also add to chat messages for display
                            self.messages.push(crate::chat::ChatMessage::new(
                                MessageType::ToolResult,
                                format!("✅ Tool '{}' executed successfully:\n{}", tool_call.function.name, result.output),
                            ));
                        }
                        Err(e) => {
                            let error_msg = format!("❌ Tool '{}' failed: {}", tool_call.function.name, e);
                            let tool_msg = crate::api::ChatMessage {
                                role: "tool".to_string(),
                                content: Some(error_msg.clone()),
                                tool_calls: None,
                                tool_call_id: Some(tool_call.id.clone()),
                            };
                            self.api_messages.push(tool_msg);

                            self.messages.push(crate::chat::ChatMessage::new(
                                MessageType::ToolResult,
                                error_msg,
                            ));
                        }
                    }
                }
            }

            // Now send a new request to AI with the tool results
            // Use continue_conversation method instead of send_to_ai
            self.continue_conversation_with_tools()?;
        }

        Ok(())
    }

    pub fn continue_conversation_with_tools(&mut self) -> Result<()> {
        // Get API client
        let config = self.get_config();
        let api_client = crate::api::ApiClient::new(
            config.ai.provider.clone(),
            config.ai.api_url.clone(),
            config.ai.api_key.clone(),
            config.ai.model.clone(),
        );

        // Use the current API messages which now include tool results
        let message_history = self.api_messages.clone();

        // Start new streaming request
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            match api_client.continue_conversation_with_tool_results(message_history).await {
                Ok(mut stream_rx) => {
                    let _ = tx_clone.send(AiResponse::StreamStart);
                    // Forward all streaming responses
                    while let Some(response) = stream_rx.recv().await {
                        match response {
                            crate::api::StreamingResponse::Start => {},
                            crate::api::StreamingResponse::Chunk(chunk) => {
                                let _ = tx_clone.send(AiResponse::StreamChunk(chunk));
                            }
                            crate::api::StreamingResponse::End(api_response) => {
                                // Check for tool calls in the continuation response
                                let tool_calls = api_response.tool_calls.clone();
                                let _ = tx_clone.send(AiResponse::Success {
                                    response: api_response.response,
                                    usage: api_response.usage,
                                    tool_calls,
                                });
                            }
                            crate::api::StreamingResponse::Error(error) => {
                                let _ = tx_clone.send(AiResponse::Error(error));
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx_clone.send(AiResponse::Error(format!("Failed to continue conversation: {}", e)));
                }
            }
        });

        self.ai_response_rx = Some(rx);
        Ok(())
    }

    pub async fn execute_tools(&mut self, tool_calls: Vec<ToolCall>) {
        let mut results = Vec::new();

        for tool_call in tool_calls {
            match tool_call.tool.as_str() {
                "bash_tool" => {
                    if let Some(command) = tool_call.arguments.get("command").and_then(|v| v.as_str()) {
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
                eprintln!("DEBUG: execute_tools - Setting pending_tool_results with {} results", results.len());
            }
            self.pending_tool_results = Some(results);
        } else {
            if self.debug {
                eprintln!("DEBUG: execute_tools - No results to set");
            }
        }
    }

    pub async fn send_tool_results_to_ai(&mut self) -> Result<()> {
        if let Some(tool_results) = self.pending_tool_results.clone() {
            // Get or generate a tool call ID
            let tool_call_id = self.messages.last()
                .and_then(|m| {
                    if let Some(tool_calls) = &m.tool_call_json {
                        if let Ok(calls) = serde_json::from_str::<Vec<serde_json::Value>>(tool_calls) {
                            if let Some(call) = calls.get(0) {
                                if let Some(id) = call.get("id").and_then(|id| id.as_str()) {
                                    return Some(id.to_string());
                                }
                            }
                        }
                    }
                    None
                })
                .unwrap_or_else(|| {
                    // Generate a unique tool call ID if none exists
                    format!("call_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..8].to_string())
                });

            // Create tool result messages for Z.AI with proper format
            for result in &tool_results {
                let tool_content = serde_json::json!({
                    "command": result.tool,  // Use actual tool instead of hardcoded
                    "output": result.output,
                    "success": result.success
                });

                // Create the message in the format Z.AI expects (api::ChatMessage)
                let api_msg = crate::api::ChatMessage {
                    role: "tool".to_string(),
                    content: Some(tool_content.to_string()),
                    tool_calls: None,
                    tool_call_id: Some(tool_call_id.clone()),
                };

                // Convert to internal chat message format
                let internal_msg = crate::chat::ChatMessage::new(
                    crate::chat::MessageType::ToolCall,
                    tool_content.to_string(),
                );

                self.messages.push(internal_msg);

                // Also add to API message list for the next request
                self.api_messages.push(api_msg);
            }

            // Trigger AI response to tool results
            // The AI should automatically continue when it receives tool results
            if let Some(ref _api_client) = self.api_client {
                if self.debug {
                eprintln!("DEBUG: Triggering AI response after tool results");
            }
                // Add a continuation message that won't be displayed but will trigger the AI
                self.send_to_ai("Please continue based on the tool results.").await?;
                if self.debug {
                    eprintln!("DEBUG: AI response triggered");
                }
            } else {
                if self.debug {
                    eprintln!("DEBUG: No API client available for tool result continuation");
                }
            }
        }
        Ok(())
    }

    pub fn get_pending_tool_calls(&mut self) -> Option<Vec<ToolCall>> {
        self.pending_tool_calls.take()
    }

    pub fn get_pending_tool_results(&mut self) -> Option<Vec<ToolCallResult>> {
        let results = self.pending_tool_results.take();
        if self.debug {
            eprintln!("DEBUG: get_pending_tool_results - returning {} results",
                     results.as_ref().map_or(0, |r| r.len()));
        }
        results
    }

    pub fn get_pending_api_response(&mut self) -> Option<crate::api::ApiResponse> {
        self.pending_api_response.take()
    }

    pub async fn execute_bash_command(&self, command: &str) -> Result<String> {
        use std::process::Command;

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", command])
                .output()?
        } else {
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()?
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
            Err(anyhow::anyhow!("{}", if stderr.is_empty() {
                "Command failed".to_string()
            } else {
                stderr
            }))
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
