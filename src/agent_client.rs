//! Modern AI Agent Client that integrates with existing API infrastructure
//!
//! This module provides a high-level agent interface that uses the modern tool calling
//! patterns while integrating with the existing reqwest-based API client.

use crate::agent::{AgentOptions, ContentBlock, ToolRegistry, ToolResult};
use crate::api::{ApiClient, ChatMessage, StreamingResponse};
use crate::tools::create_default_tool_registry;
use anyhow::Result;
use futures::Stream;
use serde_json::json;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

/// Debug print helper that checks ARULA_DEBUG environment variable
fn debug_print(msg: &str) {
    if std::env::var("ARULA_DEBUG").is_ok() {
        eprintln!("{}", msg);
    }
}

/// Modern AI Agent Client
pub struct AgentClient {
    api_client: ApiClient,
    tool_registry: ToolRegistry,
    options: AgentOptions,
}

impl Clone for AgentClient {
    fn clone(&self) -> Self {
        Self {
            api_client: self.api_client.clone(),
            tool_registry: create_default_tool_registry(), // Recreate registry
            options: self.options.clone(),
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
    ) -> Self {
        let api_client = ApiClient::new(provider, endpoint, api_key, model);
        let tool_registry = create_default_tool_registry();

        Self {
            api_client,
            tool_registry,
            options,
        }
    }

    /// Create an agent client from existing config
    pub fn from_config(provider: String, endpoint: String, api_key: String, model: String) -> Self {
        let options = AgentOptions::default();
        Self::new(provider, endpoint, api_key, model, options)
    }

    /// Send a message and get a streaming response
    pub async fn query(
        &self,
        message: &str,
        conversation_history: Option<Vec<ChatMessage>>,
    ) -> Result<Pin<Box<dyn Stream<Item = ContentBlock> + Send>>> {
        // Convert conversation history to the format expected by our existing API
        let api_messages = self.build_api_messages(message, conversation_history)?;

        // Start streaming request with tools
        let (tx, rx) = mpsc::unbounded_channel();
        let api_client = self.api_client.clone();
        let tools = self.tool_registry.get_openai_tools();
        let auto_execute_tools = self.options.auto_execute_tools;
        let max_tool_iterations = self.options.max_tool_iterations;

        let debug = self.options.debug;
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::handle_streaming_response(
                api_client,
                api_messages,
                tools,
                tx,
                auto_execute_tools,
                max_tool_iterations,
                debug,
            )
            .await
            {
                let _ = tx_clone.send(ContentBlock::error(format!("Stream error: {}", e)));
            }
        });

        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }

    /// Register additional tools
    pub fn register_tool<T: crate::agent::Tool + 'static>(&mut self, tool: T) {
        self.tool_registry.register(tool);
    }

    /// Get available tools
    pub fn get_available_tools(&self) -> Vec<&str> {
        self.tool_registry.get_tools()
    }

    /// Build API messages from user message and conversation history
    fn build_api_messages(
        &self,
        message: &str,
        conversation_history: Option<Vec<ChatMessage>>,
    ) -> Result<Vec<ChatMessage>> {
        let mut messages = Vec::new();

        // Add system message
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(self.options.system_prompt.clone()),
            tool_calls: None,
            tool_call_id: None,
        });

        // Add conversation history if provided
        if let Some(history) = conversation_history {
            for msg in history {
                if msg.role != "system" {
                    messages.push(msg);
                }
            }
        }

        // Add current user message
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(message.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });

        Ok(messages)
    }

    /// Handle streaming response with tool calling
    async fn handle_streaming_response(
        api_client: ApiClient,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
        tx: mpsc::UnboundedSender<ContentBlock>,
        auto_execute_tools: bool,
        _max_tool_iterations: u32,
        debug: bool,
    ) -> Result<()> {
        let tool_registry = create_default_tool_registry();
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
            current_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: Some(accumulated_text),
                tool_calls: Some(response_tools.clone()),
                tool_call_id: None,
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
