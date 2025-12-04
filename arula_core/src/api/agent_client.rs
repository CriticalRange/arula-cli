//! Modern AI Agent Client that integrates with existing API infrastructure
//!
//! This module provides a high-level agent interface that uses the modern tool calling
//! patterns while integrating with the existing reqwest-based API client.

use crate::api::agent::{AgentOptions, ContentBlock, ToolRegistry, ToolResult};
use crate::api::api::{ApiClient, ChatMessage, StreamingResponse};
use crate::tools::tools::{create_basic_tool_registry, initialize_mcp_tools};
use crate::utils::config::Config;
use crate::utils::debug::debug_print;
use anyhow::Result;
use futures::Stream;
use serde_json::json;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

/// Modern AI Agent Client
pub struct AgentClient {
    api_client: ApiClient,
    tool_registry: ToolRegistry,
    options: AgentOptions,
    config: crate::utils::config::Config,
}

impl Clone for AgentClient {
    fn clone(&self) -> Self {
        // For Clone, we need to create a new registry but Clone is sync
        // So we'll create a minimal registry without MCP tools for cloning
        let mut registry = crate::api::agent::ToolRegistry::new();

        // Register basic tools (non-MCP)
        registry.register(crate::tools::tools::BashTool::new());
        registry.register(crate::tools::tools::FileReadTool::new());
        registry.register(crate::tools::tools::FileEditTool::new());
        registry.register(crate::tools::tools::WriteFileTool::new());
        registry.register(crate::tools::tools::ListDirectoryTool::new());
        registry.register(crate::tools::tools::SearchTool::new());
        registry.register(crate::tools::tools::WebSearchTool::new());
        registry.register(crate::tools::tools::VisioneerTool::new());
        registry.register(crate::tools::tools::QuestionTool::new());

        Self {
            api_client: self.api_client.clone(),
            tool_registry: registry,
            options: self.options.clone(),
            config: self.config.clone(),
        }
    }
}

impl AgentClient {
    /// Create a new agent client with the given configuration
    pub fn new(
        provider: String,
        endpoint: String,
        api_key: String,
        model: String,
        options: AgentOptions,
        config: &crate::utils::config::Config,
    ) -> Self {
        let api_client = ApiClient::new(provider, endpoint, api_key, model);
        let tool_registry = create_basic_tool_registry();

        Self {
            api_client,
            tool_registry,
            options,
            config: config.clone(),
        }
    }

    /// Create a new agent client with a pre-initialized tool registry (shared via Arc)
    pub fn new_with_registry(
        provider: String,
        endpoint: String,
        api_key: String,
        model: String,
        options: AgentOptions,
        config: &crate::utils::config::Config,
        tool_registry: crate::api::agent::ToolRegistry,
    ) -> Self {
        let api_client = ApiClient::new(provider, endpoint, api_key, model);

        Self {
            api_client,
            tool_registry,
            options,
            config: config.clone(),
        }
    }

    /// Create an agent client from existing config
    pub fn from_config(provider: String, endpoint: String, api_key: String, model: String) -> Self {
        let options = AgentOptions::default();
        let config = Config::default(); // Create default config for MCP discovery
        Self::new(provider, endpoint, api_key, model, options, &config)
    }

    /// Send a message and get a streaming response
    pub async fn query(
        &self,
        message: &str,
        conversation_history: Option<Vec<ChatMessage>>,
    ) -> Result<Pin<Box<dyn Stream<Item = ContentBlock> + Send>>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let api_client = self.api_client.clone();
        let auto_execute_tools = self.options.auto_execute_tools;
        let max_tool_iterations = self.options.max_tool_iterations;
        let debug = self.options.debug;
        let config_clone = self.config.clone();
        let tx_clone = tx.clone();

        // Get the available tools with proper schemas from the registry
        let tools = self.tool_registry.get_openai_tools();

        // Build the messages with the system prompt
        let messages = self.build_api_messages(message, conversation_history)?;

        tokio::spawn(async move {
            // Create a new tool registry for execution in the async task
            let mut execution_registry = create_basic_tool_registry();
            if let Err(e) = initialize_mcp_tools(&mut execution_registry, &config_clone).await {
                eprintln!("⚠️ Failed to initialize MCP tools in async task: {}", e);
            }

            if let Err(e) = Self::handle_streaming_response(
                api_client,
                messages,
                tools,
                tx,
                auto_execute_tools,
                max_tool_iterations,
                debug,
                &execution_registry,
            )
            .await
            {
                let _ = tx_clone.send(ContentBlock::error(format!("Stream error: {}", e)));
            }
        });

        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }

    /// Send a message with true SSE streaming and automatic tool execution
    ///
    /// This method provides real-time streaming output while handling tool calls.
    /// Unlike `query()`, this uses true Server-Sent Events streaming for immediate
    /// text output as it's generated by the AI.
    ///
    /// # Arguments
    ///
    /// * `message` - The user's message
    /// * `conversation_history` - Optional conversation history
    ///
    /// # Returns
    ///
    /// A stream of `ContentBlock` items representing text, tool calls, and results.
    pub async fn query_streaming(
        &self,
        message: &str,
        conversation_history: Option<Vec<ChatMessage>>,
    ) -> Result<Pin<Box<dyn Stream<Item = ContentBlock> + Send>>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let api_client = self.api_client.clone();
        let auto_execute_tools = self.options.auto_execute_tools;
        let max_tool_iterations = self.options.max_tool_iterations;
        let debug = self.options.debug;
        let config_clone = self.config.clone();
        let tx_clone = tx.clone();

        // Get tools from registry
        let tools = self.tool_registry.get_openai_tools();

        // Build messages
        let messages = self.build_api_messages(message, conversation_history)?;

        tokio::spawn(async move {
            // Create tool registry for execution
            let mut execution_registry = create_basic_tool_registry();
            if let Err(e) = initialize_mcp_tools(&mut execution_registry, &config_clone).await {
                if debug {
                    debug_print(&format!("⚠️ Failed to initialize MCP tools: {}", e));
                }
            }

            if let Err(e) = Self::handle_true_streaming(
                api_client,
                messages,
                tools,
                tx,
                auto_execute_tools,
                max_tool_iterations,
                debug,
                &execution_registry,
            )
            .await
            {
                let _ = tx_clone.send(ContentBlock::error(format!("Stream error: {}", e)));
            }
        });

        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }

    /// Send a message with non-streaming mode (waits for complete response)
    ///
    /// This method waits for the complete API response before returning it.
    /// Useful for environments with limited terminal capabilities or when
    /// a complete response is preferred over streaming updates.
    ///
    /// # Arguments
    ///
    /// * `message` - The user's message
    /// * `conversation_history` - Optional conversation history
    ///
    /// # Returns
    ///
    /// A stream of `ContentBlock` items (sent as batch when complete)
    pub async fn query_non_streaming(
        &self,
        message: &str,
        conversation_history: Option<Vec<ChatMessage>>,
    ) -> Result<Pin<Box<dyn Stream<Item = ContentBlock> + Send>>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let api_client = self.api_client.clone();
        let auto_execute_tools = self.options.auto_execute_tools;
        let max_tool_iterations = self.options.max_tool_iterations;
        let debug = self.options.debug;
        let config_clone = self.config.clone();
        let tx_clone = tx.clone();

        // Get tools from registry
        let tools = self.tool_registry.get_openai_tools();

        // Build messages
        let messages = self.build_api_messages(message, conversation_history)?;

        tokio::spawn(async move {
            // Create tool registry for execution
            let mut execution_registry = create_basic_tool_registry();
            if let Err(e) = initialize_mcp_tools(&mut execution_registry, &config_clone).await {
                if debug {
                    debug_print(&format!("⚠️ Failed to initialize MCP tools: {}", e));
                }
            }

            if let Err(e) = Self::handle_non_streaming(
                api_client,
                messages,
                tools,
                tx,
                auto_execute_tools,
                max_tool_iterations,
                debug,
                &execution_registry,
            )
            .await
            {
                let _ = tx_clone.send(ContentBlock::error(format!("API error: {}", e)));
            }
        });

        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }

    /// Handle non-streaming API calls with tool execution loop
    async fn handle_non_streaming(
        api_client: ApiClient,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
        tx: mpsc::UnboundedSender<ContentBlock>,
        auto_execute_tools: bool,
        max_tool_iterations: u32,
        debug: bool,
        tool_registry: &crate::api::agent::ToolRegistry,
    ) -> Result<()> {
        let mut current_messages = messages;
        let mut iterations = 0;

        loop {
            if iterations >= max_tool_iterations {
                debug_print("Max tool iterations reached, stopping");
                break;
            }

            if debug {
                debug_print(&format!("Non-streaming iteration {}", iterations + 1));
            }

            // Make non-streaming API call using send_message_with_tools_sync
            let response = api_client
                .send_message_with_tools_sync(&current_messages, &tools)
                .await?;

            // Send the complete text response
            if !response.response.is_empty() {
                let _ = tx.send(ContentBlock::Text { text: response.response.clone() });
            }

            // Check for tool calls
            if let Some(ref calls) = response.tool_calls {
                if !calls.is_empty() && auto_execute_tools {
                    // Add assistant message with tool calls
                    current_messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: if response.response.is_empty() { None } else { Some(response.response.clone()) },
                        tool_calls: Some(calls.clone()),
                        tool_call_id: None,
                        tool_name: None,
                    });

                    // Execute each tool call
                    for tool_call in calls {
                        // Send tool call notification
                        let _ = tx.send(ContentBlock::tool_call(
                            tool_call.id.clone(),
                            tool_call.function.name.clone(),
                            tool_call.function.arguments.clone(),
                        ));

                        // Parse arguments and execute
                        let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                            .unwrap_or(json!({}));

                        let tool_result = tool_registry
                            .execute_tool(&tool_call.function.name, args.clone())
                            .await;

                        let result_content = match tool_result {
                            Some(result) => {
                                // Send tool result
                                let _ = tx.send(ContentBlock::tool_result(
                                    tool_call.id.clone(),
                                    result.clone(),
                                ));
                                
                                // Format for message history
                                if result.success {
                                    result.data.to_string()
                                } else {
                                    format!("Error: {}", result.error.unwrap_or_default())
                                }
                            }
                            None => {
                                let error_msg = format!("Tool not found: {}", tool_call.function.name);
                                let _ = tx.send(ContentBlock::tool_result(
                                    tool_call.id.clone(),
                                    crate::api::agent::ToolResult::error(error_msg.clone()),
                                ));
                                error_msg
                            }
                        };

                        // Add tool result to messages
                        current_messages.push(ChatMessage {
                            role: "tool".to_string(),
                            content: Some(result_content),
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                            tool_name: Some(tool_call.function.name.clone()),
                        });
                    }

                    // Continue the loop for another iteration
                    iterations += 1;
                    continue;
                } else {
                    // Tool calls present but auto-execute disabled
                    for tool_call in calls {
                        let _ = tx.send(ContentBlock::tool_call(
                            tool_call.id.clone(),
                            tool_call.function.name.clone(),
                            tool_call.function.arguments.clone(),
                        ));
                    }
                }
            }

            // No more tool calls, we're done
            break;
        }

        Ok(())
    }

    /// Handle true SSE streaming with tool execution loop
    async fn handle_true_streaming(
        api_client: ApiClient,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
        tx: mpsc::UnboundedSender<ContentBlock>,
        auto_execute_tools: bool,
        max_tool_iterations: u32,
        debug: bool,
        tool_registry: &crate::api::agent::ToolRegistry,
    ) -> Result<()> {
        use crate::api::streaming::StreamEvent;
        
        let mut current_messages = messages;
        let mut iterations = 0;

        loop {
            if iterations >= max_tool_iterations {
                debug_print("Max tool iterations reached, stopping");
                break;
            }

            // Collect stream events and the final response
            let mut text_started = false;
            let mut accumulated_text = String::new();
            let mut tool_calls = Vec::new();

            // Use the new streaming API
            let response = api_client
                .send_message_streaming(&current_messages, &tools, |event| {
                    match event {
                        StreamEvent::Start { id: _, model: _ } => {
                            // Stream started
                            let _ = tx.send(ContentBlock::text(""));
                            text_started = true;
                        }
                        StreamEvent::TextDelta(text) => {
                            accumulated_text.push_str(&text);
                            let _ = tx.send(ContentBlock::text(text));
                        }
                        StreamEvent::ToolCallStart { index, id, name } => {
                            if debug {
                                debug_print(&format!("Tool call start: {} ({})", name, id));
                            }
                            // Initialize tool call tracking
                            while tool_calls.len() <= index {
                                tool_calls.push(crate::api::api::ToolCall {
                                    id: String::new(),
                                    r#type: "function".to_string(),
                                    function: crate::api::api::ToolCallFunction {
                                        name: String::new(),
                                        arguments: String::new(),
                                    },
                                });
                            }
                            tool_calls[index].id = id;
                            tool_calls[index].function.name = name;
                        }
                        StreamEvent::ToolCallDelta { index, arguments } => {
                            if index < tool_calls.len() {
                                tool_calls[index].function.arguments.push_str(&arguments);
                            }
                        }
                        StreamEvent::ToolCallComplete(tc) => {
                            // Ensure the tool call is in our list
                            if debug {
                                debug_print(&format!("Tool call complete: {:?}", tc));
                            }
                        }
                        StreamEvent::Finish { reason, usage: _ } => {
                            if debug {
                                debug_print(&format!("Stream finished: {}", reason));
                            }
                        }
                        StreamEvent::Error(err) => {
                            let _ = tx.send(ContentBlock::error(err));
                        }
                        StreamEvent::ThinkingStart => {
                            // Thinking started - can emit reasoning block
                            if debug {
                                debug_print("Thinking started");
                            }
                        }
                        StreamEvent::ThinkingDelta(thinking) => {
                            // Accumulate thinking content
                            let _ = tx.send(ContentBlock::reasoning(thinking));
                        }
                        StreamEvent::ThinkingEnd => {
                            // Thinking finished
                            if debug {
                                debug_print("Thinking ended");
                            }
                        }
                    }
                })
                .await?;

            // Use tool_calls from response if our tracking is empty
            let final_tool_calls = if tool_calls.is_empty() {
                response.tool_calls.clone()
            } else {
                Some(tool_calls)
            };

            // Check if we have tool calls to execute
            if let Some(ref calls) = final_tool_calls {
                if !calls.is_empty() && auto_execute_tools {
                    // Add assistant message with tool calls
                    current_messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: if accumulated_text.is_empty() { None } else { Some(accumulated_text.clone()) },
                        tool_calls: Some(calls.clone()),
                        tool_call_id: None,
                        tool_name: None,
                    });

                    // Execute each tool call
                    for tool_call in calls {
                        // Send tool call notification
                        let _ = tx.send(ContentBlock::tool_call(
                            tool_call.id.clone(),
                            tool_call.function.name.clone(),
                            tool_call.function.arguments.clone(),
                        ));

                        // Parse arguments and execute
                        let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                            .unwrap_or(json!({}));

                        let tool_result = tool_registry
                            .execute_tool(&tool_call.function.name, args.clone())
                            .await;

                        let result_content = match tool_result {
                            Some(result) => {
                                // Send tool result
                                let _ = tx.send(ContentBlock::tool_result(
                                    tool_call.id.clone(),
                                    result.clone(),
                                ));
                                
                                // Format for message history
                                if result.success {
                                    result.data.to_string()
                                } else {
                                    format!("Error: {}", result.error.unwrap_or_default())
                                }
                            }
                            None => {
                                let error_msg = format!("Tool not found: {}", tool_call.function.name);
                                let _ = tx.send(ContentBlock::tool_result(
                                    tool_call.id.clone(),
                                    ToolResult::error(error_msg.clone()),
                                ));
                                error_msg
                            }
                        };

                        // Add tool result to messages
                        current_messages.push(ChatMessage {
                            role: "tool".to_string(),
                            content: Some(result_content),
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                            tool_name: Some(tool_call.function.name.clone()),
                        });
                    }

                    // Continue the loop for another iteration
                    iterations += 1;
                    continue;
                } else {
                    // Tool calls present but auto-execute disabled
                    for tool_call in calls {
                        let _ = tx.send(ContentBlock::tool_call(
                            tool_call.id.clone(),
                            tool_call.function.name.clone(),
                            tool_call.function.arguments.clone(),
                        ));
                    }
                }
            }

            // No more tool calls, we're done
            break;
        }

        Ok(())
    }

    /// Register additional tools
    pub fn register_tool<T: crate::api::agent::Tool + 'static>(&mut self, tool: T) {
        self.tool_registry.register(tool);
    }

    /// Initialize MCP tools lazily (called when needed)
    async fn ensure_mcp_tools_initialized(&mut self) {
        // Check if MCP tools are already initialized by looking for MCP server tools
        let has_mcp_tools = self.tool_registry.get_tools().iter().any(|tool| {
            tool.starts_with("__mcp_") || tool.starts_with("mcp_")
        });

        if !has_mcp_tools {
            if let Err(e) = crate::tools::tools::initialize_mcp_tools(&mut self.tool_registry, &self.config).await {
                eprintln!("⚠️ Failed to initialize MCP tools: {}", e);
            }
        }
    }

    /// Get available tools (with lazy MCP initialization)
    pub async fn get_available_tools(&mut self) -> Vec<String> {
        self.ensure_mcp_tools_initialized().await;
        self.tool_registry.get_tools().into_iter().map(|s| s.to_string()).collect()
    }

    /// Get available tools (sync version without MCP initialization)
    pub fn get_available_tools_sync(&self) -> Vec<&str> {
        self.tool_registry.get_tools()
    }

    /// Build API messages from user message and conversation history
    fn build_api_messages(
        &self,
        message: &str,
        conversation_history: Option<Vec<ChatMessage>>,
    ) -> Result<Vec<ChatMessage>> {
        let mut messages = Vec::new();

        // Check if we have conversation history
        if let Some(history) = conversation_history {
            // Check if the first message is a system message
            let has_system_message = history.first().map_or(false, |msg| msg.role == "system");

            // Add system message only if not already in history
            if !has_system_message {
                messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: Some(self.options.system_prompt.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    tool_name: None,
                });
            }

            // Add all messages from history
            for msg in history {
                if msg.role != "system" || !has_system_message {
                    messages.push(msg);
                }
            }

            // Check if the last message in history is already the current user message
            let history_has_current_message = messages.last().is_some_and(|last| {
                last.role == "user" && last.content.as_deref() == Some(message)
            });

            // Only add current user message if it's not already in history
            if !history_has_current_message {
                messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: Some(message.to_string()),
                    tool_calls: None,
                    tool_call_id: None,
                    tool_name: None,
                });
            }
        } else {
            // No history provided, add system message and user message
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: Some(self.options.system_prompt.clone()),
                tool_calls: None,
                tool_call_id: None,
                tool_name: None,
            });
            
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: Some(message.to_string()),
                tool_calls: None,
                tool_call_id: None,
                tool_name: None,
            });
        }

        Ok(messages)
    }

    /// Handle streaming response with tool calling
    async fn handle_streaming_response(
        api_client: ApiClient,
        messages: Vec<ChatMessage>,
        _tools: Vec<serde_json::Value>,
        tx: mpsc::UnboundedSender<ContentBlock>,
        auto_execute_tools: bool,
        _max_tool_iterations: u32,
        debug: bool,
        tool_registry: &crate::api::agent::ToolRegistry,
    ) -> Result<()> {

        // Use the tools passed in (already filtered in query method)
        let tools = _tools;

        let mut current_messages = messages;

        loop {
            // Send request with tools
            let mut stream_rx = api_client
                .send_message_with_tools(&current_messages, &tools)
                .await?;

            let mut accumulated_text = String::new();
            let mut response_tools = Vec::new();

            // Process streaming response
            while let Some(response) = stream_rx.recv().await {
                match response {
                    StreamingResponse::Start => {
                        let _ = tx.send(ContentBlock::text(""));
                    }
                    StreamingResponse::Chunk(chunk) => {
                        accumulated_text.push_str(&chunk);
                        let _ = tx.send(ContentBlock::text(chunk));
                    }
                    StreamingResponse::End(api_response) => {
                        // Check for tool calls in the response
                        if let Some(tool_calls) = api_response.tool_calls {
                            response_tools.extend(tool_calls);
                        }

                        // Send reasoning content if available (for Z.AI thinking mode)
                        if let Some(reasoning) = &api_response.reasoning_content {
                            if !reasoning.is_empty() {
                                let _ = tx.send(ContentBlock::reasoning(reasoning.clone()));
                            }
                        }
                        break;
                    }
                    StreamingResponse::Error(err) => {
                        let _ = tx.send(ContentBlock::error(err));
                        return Ok(());
                    }
                }
            }

            // If no tools were called, we're done
            if response_tools.is_empty() {
                break;
            }

            // Add assistant message with tool calls
            // Use None for content if it's empty or just whitespace (OpenAI spec: content can be null when tool_calls present)
            let trimmed_text = accumulated_text.trim();
            let assistant_content = if trimmed_text.is_empty() {
                None
            } else {
                Some(accumulated_text)
            };
            current_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: assistant_content,
                tool_calls: Some(response_tools.clone()),
                tool_call_id: None,
                tool_name: None,
            });

            // Execute tools if auto-execute is enabled
            if auto_execute_tools {
                for tool_call in response_tools {
                    let tool_call_id = tool_call.id.clone();
                    let tool_name = tool_call.function.name.clone();

                    let _ = tx.send(ContentBlock::tool_call(
                        tool_call.id.clone(),
                        tool_name.clone(),
                        tool_call.function.arguments.clone(),
                    ));

                    // Parse and execute the tool
                    let raw_args = &tool_call.function.arguments;
                    if debug {
                        debug_print(&format!(
                            "DEBUG: Raw tool args for '{}': {}",
                            tool_name, raw_args
                        ));
                    }
                    match serde_json::from_str::<serde_json::Value>(raw_args) {
                        Ok(args) => {
                            if debug {
                                debug_print(&format!(
                                    "DEBUG: Parsed tool args for '{}': {}",
                                    tool_name,
                                    serde_json::to_string_pretty(&args)
                                        .unwrap_or_else(|_| "Invalid JSON".to_string())
                                ));
                            }
                            if let Some(result) = tool_registry.execute_tool(&tool_name, args).await
                            {
                                if debug {
                                    debug_print(&format!(
                                        "DEBUG: Tool '{}' result: success={}, data={:?}",
                                        tool_name, result.success, result.data
                                    ));
                                }
                                let result_json = if result.success {
                                    json!({
                                        "success": true,
                                        "data": result.data
                                    })
                                } else {
                                    json!({
                                        "success": false,
                                        "error": result.error
                                    })
                                };

                                if debug {
                                    let json_str = serde_json::to_string_pretty(&result_json)
                                        .unwrap_or_else(|_| "Invalid JSON".to_string());
                                    debug_print(&format!(
                                        "DEBUG: Tool result JSON size: {} bytes",
                                        json_str.len()
                                    ));
                                    // Truncate for debug output
                                    if json_str.len() > 500 {
                                        debug_print(&format!(
                                            "DEBUG: Tool result JSON (truncated): {}",
                                            &json_str[..500]
                                        ));
                                    } else {
                                        debug_print(&format!(
                                            "DEBUG: Tool result JSON: {}",
                                            json_str
                                        ));
                                    }
                                }

                                // Send tool result back
                                let _ = tx.send(ContentBlock::tool_result(
                                    tool_call_id.clone(),
                                    result.clone(),
                                ));

                                // Add tool result to conversation
                                current_messages.push(ChatMessage {
                                    role: "tool".to_string(),
                                    content: Some(result_json.to_string()),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_call_id.clone()),
                                    tool_name: Some(tool_name.clone()),
                                });
                            } else {
                                let error_msg = format!("Tool '{}' not found", tool_name);
                                let _ = tx.send(ContentBlock::tool_result(
                                    tool_call_id.clone(),
                                    ToolResult::error(error_msg.clone()),
                                ));

                                current_messages.push(ChatMessage {
                                    role: "tool".to_string(),
                                    content: Some(
                                        json!({
                                            "success": false,
                                            "error": error_msg
                                        })
                                        .to_string(),
                                    ),
                                    tool_calls: None,
                                    tool_call_id: Some(tool_call_id.clone()),
                                    tool_name: Some(tool_name.clone()),
                                });
                            }
                        }
                        Err(err) => {
                            let error_msg = format!("Failed to parse tool arguments: {}", err);
                            let _ = tx.send(ContentBlock::tool_result(
                                tool_call_id.clone(),
                                ToolResult::error(error_msg.clone()),
                            ));

                            current_messages.push(ChatMessage {
                                role: "tool".to_string(),
                                content: Some(
                                    json!({
                                        "success": false,
                                        "error": error_msg
                                    })
                                    .to_string(),
                                ),
                                tool_calls: None,
                                tool_call_id: Some(tool_call_id.clone()),
                                tool_name: Some(tool_name.clone()),
                            });
                        }
                    }
                }

                // Continue conversation to get AI's response to tool results
                if debug {
                    debug_print(&format!(
                        "DEBUG: About to make continuation API call with {} messages",
                        current_messages.len()
                    ));
                    // Check total message size
                    let total_size: usize = current_messages
                        .iter()
                        .map(|msg| serde_json::to_string(msg).unwrap_or_default().len())
                        .sum();
                    debug_print(&format!(
                        "DEBUG: Total message payload size: {} bytes",
                        total_size
                    ));
                }
                continue;
            } else {
                // If not auto-executing, just return the tool calls
                break;
            }
        }

        Ok(())
    }
}
