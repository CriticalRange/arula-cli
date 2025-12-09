//! Unified streaming implementation for AI API responses
//!
//! This module provides a consolidated approach to handling streaming responses
//! from various AI providers, with built-in support for:
//! - Server-Sent Events (SSE) and NDJSON streams
//! - Automatic tool execution loops
//! - Provider-specific request formatting (Z.AI, OpenAI, Ollama)

use crate::api::agent::ToolResult;
use crate::api::api::{
    AIProvider, ApiClient, ApiResponse, ChatMessage, ToolCall, ToolCallFunction, Usage,
};
use crate::api::xml_toolcall::extract_tool_call_from_xml;
use crate::tools::builtin::execute_bash_streaming;
use crate::utils::error_utils::{stream_error, ErrorContext};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use reqwest::Response;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ============================================================================
//  Types
// ============================================================================

/// Represents a streaming chunk from OpenAI-compatible APIs
#[derive(Debug, Clone, Deserialize)]
pub struct StreamChunk {
    pub id: Option<String>,
    pub object: Option<String>,
    pub created: Option<u64>,
    pub model: Option<String>,
    pub choices: Vec<StreamChoice>,
    #[serde(default)]
    pub usage: Option<StreamUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamChoice {
    pub index: usize,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<StreamToolCallDelta>>,
    /// OpenAI o1/o3 reasoning content
    pub reasoning_content: Option<String>,
    /// Ollama thinking content
    pub thinking: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub r#type: Option<String>,
    pub function: Option<StreamFunctionDelta>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StreamFunctionDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Events emitted during streaming
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Start of stream
    Start { id: String, model: String },
    /// Text content chunk
    TextDelta(String),
    /// Thinking/reasoning content chunk
    ThinkingDelta(String),
    /// Thinking started
    ThinkingStart,
    /// Thinking completed
    ThinkingEnd,
    /// Tool call started
    ToolCallStart {
        index: usize,
        id: String,
        name: String,
    },
    /// Tool call arguments chunk
    ToolCallDelta { index: usize, arguments: String },
    /// Tool call completed
    ToolCallComplete(ToolCall),
    /// Tool result (after execution)
    ToolResult {
        tool_call_id: String,
        result: ToolResult,
    },
    /// Bash output line (streamed during execution)
    BashOutputLine {
        tool_call_id: String,
        line: String,
        is_stderr: bool,
    },
    /// Stream finished
    Finish {
        reason: String,
        usage: Option<Usage>,
    },
    /// Error occurred
    Error(String),
}

/// Accumulator for tool call deltas
#[derive(Debug, Default)]
struct ToolCallAccumulator {
    id: String,
    name: String,
    arguments: String,
}

impl ToolCallAccumulator {
    fn to_tool_call(&self) -> ToolCall {
        ToolCall {
            id: self.id.clone(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: self.name.clone(),
                arguments: self.arguments.clone(),
            },
        }
    }
}

// ============================================================================
//  Request Building
// ============================================================================

/// Check if the endpoint URL is an Anthropic-compatible z.ai endpoint
pub fn is_anthropic_compatible_endpoint(endpoint: &str) -> bool {
    endpoint.contains("/api/anthropic")
}

/// Convert OpenAI-format tools to Anthropic format
fn convert_tools_to_anthropic(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .filter_map(|tool| {
            // OpenAI format: { "type": "function", "function": { "name", "description", "parameters" } }
            // Anthropic format: { "name", "description", "input_schema" }
            let func = tool.get("function")?;
            let name = func.get("name")?.as_str()?;
            let description = func.get("description")?.as_str().unwrap_or("");
            let parameters = func.get("parameters").cloned().unwrap_or(json!({"type": "object", "properties": {}}));
            
            Some(json!({
                "name": name,
                "description": description,
                "input_schema": parameters
            }))
        })
        .collect()
}

/// Build an Anthropic Messages API compatible request body
pub fn build_anthropic_request(
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[Value]>,
    max_tokens: u32,
) -> Value {
    // Extract system message (first message with role "system")
    let system_content: Option<String> = messages
        .iter()
        .find(|m| m.role == "system")
        .and_then(|m| m.content.clone());

    // Build messages array, excluding system messages and converting tool messages
    let anthropic_messages: Vec<Value> = messages
        .iter()
        .filter(|msg| msg.role != "system") // System goes in separate param
        .filter_map(|msg| {
            match msg.role.as_str() {
                "user" => {
                    Some(json!({
                        "role": "user",
                        "content": msg.content.clone().unwrap_or_default()
                    }))
                }
                "assistant" => {
                    let mut content_blocks: Vec<Value> = Vec::new();
                    
                    // Add text content if present
                    if let Some(text) = &msg.content {
                        if !text.is_empty() {
                            content_blocks.push(json!({
                                "type": "text",
                                "text": text
                            }));
                        }
                    }
                    
                    // Add tool_use blocks if present
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            let input: Value = serde_json::from_str(&tc.function.arguments)
                                .unwrap_or(json!({}));
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.function.name,
                                "input": input
                            }));
                        }
                    }
                    
                    if content_blocks.is_empty() {
                        None
                    } else {
                        Some(json!({
                            "role": "assistant",
                            "content": content_blocks
                        }))
                    }
                }
                "tool" => {
                    // Convert tool results to Anthropic format
                    Some(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": msg.tool_call_id.clone().unwrap_or_default(),
                            "content": msg.content.clone().unwrap_or_default()
                        }]
                    }))
                }
                _ => None
            }
        })
        .collect();

    let mut request = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": anthropic_messages,
        "stream": true
    });

    // Add system prompt if present
    if let Some(system) = system_content {
        request["system"] = json!(system);
    }

    // Convert and add tools if present
    if let Some(t) = tools {
        if !t.is_empty() {
            let anthropic_tools = convert_tools_to_anthropic(t);
            if !anthropic_tools.is_empty() {
                request["tools"] = json!(anthropic_tools);
            }
        }
    }

    request
}

/// Build a unified request body for streaming, handling provider specifics
pub fn build_streaming_request(
    provider: &AIProvider,
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[Value]>,
    temperature: f32,
    max_tokens: u32,
) -> Value {
    let is_zai = matches!(provider, AIProvider::ZAiCoding);
    let is_ollama = matches!(provider, AIProvider::Ollama);

    // 1. Process Messages
    let json_messages: Vec<Value> = messages
        .iter()
        .filter_map(|msg| {
            // Z.AI Bug Fix: We DO NOT filter out tool messages anymore!
            // Z.AI supports tool calling and needs history to continue.

            let mut obj = json!({
                "role": msg.role,
            });

            if let Some(content) = &msg.content {
                obj["content"] = json!(content);
            } else if is_zai {
                // Z.AI requires content, use empty string if none
                obj["content"] = json!("");
            } else if msg.tool_calls.is_some() {
                obj["content"] = json!(null);
            }

            // Add tool-related fields
            // Z.AI usage: Uses standard OpenAI format for tool_calls/tool_call_id
            if let Some(tool_calls) = &msg.tool_calls {
                if is_ollama {
                    // Ollama specific tool format
                    let converted: Vec<Value> = tool_calls
                        .iter()
                        .map(|tc| {
                            let args = serde_json::from_str::<Value>(&tc.function.arguments)
                                .unwrap_or_else(|_| json!({}));
                            json!({
                                "function": {
                                    "name": tc.function.name,
                                    "arguments": args
                                }
                            })
                        })
                        .collect();
                    obj["tool_calls"] = json!(converted);
                } else {
                    // Standard OpenAI / Z.AI format
                    obj["tool_calls"] = json!(tool_calls);
                }
            }

            if let Some(tool_call_id) = &msg.tool_call_id {
                if !is_ollama {
                    obj["tool_call_id"] = json!(tool_call_id);
                }
            }

            if is_ollama {
                if let Some(tool_name) = &msg.tool_name {
                    obj["tool_name"] = json!(tool_name);
                }
            }

            Some(obj)
        })
        .collect();

    // 2. Process Tools (Z.AI specific filtering)
    let tools_to_send = if is_zai {
        // Z.AI now supports tools in streaming mode
        if let Some(t) = tools {
            if !t.is_empty() {
                Some(t.to_vec())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        // Convert tools for other providers if needed
        if let Some(t) = tools {
            if !t.is_empty() {
                Some(t.to_vec())
            } else {
                None
            }
        } else {
            None
        }
    };

    // 3. Construct Body
    let mut request = json!({
        "model": model,
        "messages": json_messages,
        "max_tokens": max_tokens,
        "stream": true
    });

    // Add temperature separately to avoid type issues
    if is_zai {
        request["temperature"] = json!("0.7");
        // Add thinking parameter for Z.AI
        request["thinking"] = json!({"type": "enabled"});
    } else {
        request["temperature"] = json!(temperature);
    }

    // Option flags
    let include_stream_options = !is_zai && !is_ollama;
    let include_tool_choice = !is_zai && !is_ollama;

    if include_stream_options {
        request["stream_options"] = json!({ "include_usage": true });
    }

    if let Some(t) = tools_to_send {
        request["tools"] = json!(t);
        if include_tool_choice {
            request["tool_choice"] = json!("auto");
        }
    }

    // Ollama specific
    if is_ollama {
        if let Some(obj) = request.as_object_mut() {
            let _ = obj.remove("max_tokens");
            let _ = obj.remove("temperature");
            obj.insert(
                "options".to_string(),
                json!({
                    "num_predict": max_tokens,
                    "temperature": temperature
                }),
            );
        }
    }

    request
}

// ============================================================================
//  Stream Processing
// ============================================================================

/// Process a raw HTTP response into a stream of events
pub async fn process_response<F>(response: Response, callback: F) -> Result<ApiResponse>
where
    F: FnMut(StreamEvent),
{
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.contains("text/event-stream") {
        process_sse_stream(response, callback).await
    } else {
        process_ndjson_stream(response, callback).await
    }
}

async fn process_sse_stream<F>(response: Response, mut callback: F) -> Result<ApiResponse>
where
    F: FnMut(StreamEvent),
{
    use eventsource_stream::Eventsource;

    let mut stream = response.bytes_stream().eventsource();
    let mut accumulated = String::new();
    let mut tool_acc: HashMap<usize, ToolCallAccumulator> = HashMap::new();
    let mut finish_reason = String::new();
    let mut usage = None;
    let mut model = String::new();
    let mut stream_id = String::new();
    let mut reasoning_buffer = String::new(); // For XML tool call extraction

    while let Some(res) = stream.next().await {
        match res {
            Ok(event) => {
                let data = event.data;
                if data == "[DONE]" {
                    break;
                }

                // Log streaming chunk if debug mode is enabled
                if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                    crate::utils::debug::debug_print(&format!("Stream Chunk: {}", data));
                }

                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&data) {
                    if let Some(id) = &chunk.id {
                        if stream_id.is_empty() {
                            stream_id = id.clone();
                        }
                    }
                    if let Some(m) = &chunk.model {
                        if model.is_empty() {
                            model = m.clone();
                            callback(StreamEvent::Start {
                                id: stream_id.clone(),
                                model: model.clone(),
                            });
                        }
                    }
                    if let Some(u) = chunk.usage {
                        usage = Some(Usage {
                            prompt_tokens: u.prompt_tokens,
                            completion_tokens: u.completion_tokens,
                            total_tokens: u.total_tokens,
                        });
                    }

                    for choice in chunk.choices {
                        if let Some(r) = choice.finish_reason {
                            finish_reason = r;
                        }
                        let delta = choice.delta;

                        if let Some(c) = delta.content {
                            if !c.is_empty() {
                                accumulated.push_str(&c);
                                callback(StreamEvent::TextDelta(c));
                            }
                        }

                        if let Some(think) = delta.reasoning_content.or(delta.thinking) {
                            if !think.is_empty() {
                                // Buffer reasoning content for XML tool call detection
                                reasoning_buffer.push_str(&think);
                                callback(StreamEvent::ThinkingDelta(think));
                            }
                        }

                        if let Some(tcs) = delta.tool_calls {
                            for tc in tcs {
                                let idx = tc.index; // Use explicit index
                                let acc = tool_acc.entry(idx).or_default();

                                if let Some(id) = tc.id {
                                    acc.id = id;
                                }
                                if let Some(func) = tc.function {
                                    if let Some(n) = func.name {
                                        acc.name = n.clone();
                                        callback(StreamEvent::ToolCallStart {
                                            index: idx,
                                            id: acc.id.clone(),
                                            name: n,
                                        });
                                    }
                                    if let Some(a) = func.arguments {
                                        acc.arguments.push_str(&a);
                                        callback(StreamEvent::ToolCallDelta {
                                            index: idx,
                                            arguments: a,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let error_context =
                    ErrorContext::new("Process SSE stream").with_underlying_error(&e);
                let msg = stream_error(error_context);
                callback(StreamEvent::Error(msg.clone()));
                return Ok(ApiResponse {
                    response: accumulated,
                    success: false,
                    error: Some(msg),
                    ..Default::default()
                });
            }
        }
    }

    // Before finalizing, check if reasoning_buffer contains XML tool calls
    // This handles GLM-4.6 style XML tool calls in reasoning content (Coding Plan endpoint only)
    // Note: Anthropic-compatible endpoint uses structured tool_use blocks, not XML
    // The XML check only triggers when tool_acc is empty (no structured tool calls found)
    if !reasoning_buffer.is_empty() && tool_acc.is_empty() {
        if let Some(xml_tool_call) = extract_tool_call_from_xml(&reasoning_buffer) {
            // Convert JSON value to ToolCall
            if let Ok(tool_call) = serde_json::from_value::<ToolCall>(xml_tool_call) {
                // Add to tool_acc as if it came from standard tool_calls
                let idx = 0;
                tool_acc.insert(
                    idx,
                    ToolCallAccumulator {
                        id: tool_call.id.clone(),
                        name: tool_call.function.name.clone(),
                        arguments: tool_call.function.arguments.clone(),
                    },
                );
                // Emit the tool call event
                callback(StreamEvent::ToolCallStart {
                    index: idx,
                    id: tool_call.id.clone(),
                    name: tool_call.function.name.clone(),
                });
                callback(StreamEvent::ToolCallDelta {
                    index: idx,
                    arguments: tool_call.function.arguments.clone(),
                });
            }
        }
    }

    finalize(
        accumulated,
        tool_acc,
        finish_reason,
        usage,
        model,
        &mut callback,
    )
}

async fn process_ndjson_stream<F>(response: Response, mut callback: F) -> Result<ApiResponse>
where
    F: FnMut(StreamEvent),
{
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut accumulated = String::new();
    let mut tool_acc: HashMap<usize, ToolCallAccumulator> = HashMap::new();
    let mut finish_reason = "stop".to_string();
    let mut usage = None;
    let mut model = String::new();

    while let Some(item) = stream.next().await {
        let bytes = item.map_err(|e| {
            let error_context = ErrorContext::new("Read stream chunk").with_underlying_error(&e);
            anyhow!("{}", stream_error(error_context))
        })?;
        if let Ok(s) = std::str::from_utf8(&bytes) {
            buffer.push_str(s);
        }

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer.drain(..pos + 1);
            if line.is_empty() {
                continue;
            }

            // Log streaming chunk if debug mode is enabled
            if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                crate::utils::debug::debug_print(&format!("Stream Chunk (NDJSON): {}", line));
            }

            if let Ok(json) = serde_json::from_str::<Value>(&line) {
                // Ollama 'done' check
                if json.get("done").and_then(|v| v.as_bool()) == Some(true) {
                    if let Some(c) = json.get("eval_count").and_then(|v| v.as_u64()) {
                        usage = Some(Usage {
                            total_tokens: c as u32,
                            ..Default::default()
                        });
                    }
                    finish_reason = "stop".to_string();
                }

                // Content
                let content = json
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .or_else(|| json.get("response"))
                    .and_then(|v| v.as_str());

                if let Some(c) = content {
                    if !c.is_empty() {
                        accumulated.push_str(c);
                        callback(StreamEvent::TextDelta(c.to_string()));
                    }
                }

                // Tools
                if let Some(tcs) = json
                    .get("message")
                    .and_then(|m| m.get("tool_calls"))
                    .and_then(|v| v.as_array())
                {
                    for (i, tc) in tcs.iter().enumerate() {
                        if let Some(func) = tc.get("function") {
                            let name = func
                                .get("name")
                                .and_then(|s| s.as_str())
                                .unwrap_or_default()
                                .to_string();
                            let args = func
                                .get("arguments")
                                .map(|v| v.to_string())
                                .unwrap_or_default();

                            let acc = tool_acc.entry(i).or_default();
                            acc.name = name.clone();
                            acc.arguments = args.clone();
                            acc.id = format!("call_{}", i);

                            callback(StreamEvent::ToolCallStart {
                                index: i,
                                id: acc.id.clone(),
                                name: acc.name.clone(),
                            });
                            callback(StreamEvent::ToolCallDelta {
                                index: i,
                                arguments: acc.arguments.clone(),
                            });
                            finish_reason = "tool_calls".to_string();
                        }
                    }
                }

                if model.is_empty() {
                    if let Some(m) = json.get("model").and_then(|s| s.as_str()) {
                        model = m.to_string();
                        callback(StreamEvent::Start {
                            id: "ndjson".into(),
                            model: model.clone(),
                        });
                    }
                }
            }
        }
    }

    finalize(
        accumulated,
        tool_acc,
        finish_reason,
        usage,
        model,
        &mut callback,
    )
}

fn finalize<F>(
    content: String,
    acc: HashMap<usize, ToolCallAccumulator>,
    reason: String,
    usage: Option<Usage>,
    model: String,
    callback: &mut F,
) -> Result<ApiResponse>
where
    F: FnMut(StreamEvent),
{
    let tool_calls = if acc.is_empty() {
        None
    } else {
        let mut calls: Vec<(usize, ToolCall)> = acc
            .into_iter()
            .map(|(i, a)| (i, a.to_tool_call()))
            .collect();
        calls.sort_by_key(|(i, _)| *i);
        // Clean arguments (sometimes they are double-encoded)
        let cleaned: Vec<ToolCall> = calls
            .into_iter()
            .map(|(_, tc)| {
                // Basic check if args are a string containing json or just json
                // For now assume standard behavior, maybe add cleanup later if needed
                callback(StreamEvent::ToolCallComplete(tc.clone()));
                tc
            })
            .collect();
        Some(cleaned)
    };

    callback(StreamEvent::Finish {
        reason: reason.clone(),
        usage: usage.clone(),
    });

    Ok(ApiResponse {
        response: content,
        success: true,
        error: None,
        usage,
        tool_calls,
        model: Some(model),
        created: None,
        reasoning_content: None,
    })
}

// ============================================================================
//  Main Streaming Loop
// ============================================================================

/// Execute a streaming conversation with automatic tool handling
pub async fn stream_with_tools<F>(
    client: &ApiClient,
    messages: Vec<ChatMessage>,
    tools: &[Value],
    tool_registry: &crate::api::agent::ToolRegistry,
    auto_execute_tools: bool,
    max_tool_iterations: u32,
    mut callback: F,
) -> Result<ApiResponse>
where
    F: FnMut(StreamEvent) + Send,
{
    let mut current_messages = messages;
    let mut iterations = 0;

    loop {
        if iterations >= max_tool_iterations {
            tracing::warn!("Max tool iterations reached");
            break;
        }

        // Build request - check if we're using Anthropic-compatible endpoint
        let request_body = if is_anthropic_compatible_endpoint(&client.endpoint) {
            // Use Anthropic Messages API format
            build_anthropic_request(
                client.model(),
                &current_messages,
                Some(tools),
                4096,
            )
        } else {
            // Use standard OpenAI-compatible format (for Coding Plan endpoint)
            build_streaming_request(
                &client.provider,
                client.model(),
                &current_messages,
                Some(tools),
                0.7,
                4096,
            )
        };

        // Send request
        let response = client.make_streaming_request(request_body).await?;

        // Process stream
        let api_response = process_response(response, &mut callback).await?;

        // Check for tools
        if let Some(calls) = &api_response.tool_calls {
            if !calls.is_empty() && auto_execute_tools {
                // Add assistant response with tool calls to history
                current_messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: if api_response.response.is_empty() {
                        None
                    } else {
                        Some(api_response.response.clone())
                    },
                    tool_calls: Some(calls.clone()),
                    tool_call_id: None,
                    tool_name: None,
                });

                // Execute tools
                for call in calls {
                    let args: Value =
                        serde_json::from_str(&call.function.arguments).unwrap_or(json!({}));

                    // Check if this is a bash command - use streaming execution
                    let (result, content) = if call.function.name == "execute_bash" {
                        // Extract command from args
                        let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                        let timeout = args.get("timeout_seconds").and_then(|v| v.as_u64());

                        // Use Arc<Mutex> to share callback across async boundary
                        let call_id = call.id.clone();
                        let callback_ref = Arc::new(Mutex::new(&mut callback));

                        // Execute with streaming
                        let streaming_result = {
                            let callback_clone = callback_ref.clone();
                            let call_id_clone = call_id.clone();
                            execute_bash_streaming(command, timeout, move |line, is_stderr| {
                                if let Ok(mut cb) = callback_clone.lock() {
                                    (*cb)(StreamEvent::BashOutputLine {
                                        tool_call_id: call_id_clone.clone(),
                                        line,
                                        is_stderr,
                                    });
                                }
                            })
                            .await
                        };

                        match streaming_result {
                            Ok(bash_result) => {
                                let result_data = json!({
                                    "stdout": bash_result.stdout,
                                    "stderr": bash_result.stderr,
                                    "exit_code": bash_result.exit_code,
                                    "success": bash_result.success,
                                });
                                let tool_result = ToolResult::success(result_data.clone());
                                let content = if bash_result.success {
                                    result_data.to_string()
                                } else {
                                    format!("Error: exit code {}", bash_result.exit_code)
                                };
                                (Some(tool_result), content)
                            }
                            Err(e) => {
                                let tool_result = ToolResult::error(e.clone());
                                (Some(tool_result), format!("Error: {}", e))
                            }
                        }
                    } else {
                        // Non-bash tools use standard execution
                        let result = tool_registry.execute_tool(&call.function.name, args).await;
                        let (content, _success) = match &result {
                            Some(res) => (
                                if res.success {
                                    res.data.to_string()
                                } else {
                                    format!("Error: {}", res.error.clone().unwrap_or_default())
                                },
                                res.success,
                            ),
                            None => (format!("Tool not found: {}", call.function.name), false),
                        };
                        (result, content)
                    };

                    // Emit tool result event
                    match &result {
                        Some(res) => {
                            callback(StreamEvent::ToolResult {
                                tool_call_id: call.id.clone(),
                                result: res.clone(),
                            });
                        }
                        None => {
                            callback(StreamEvent::ToolResult {
                                tool_call_id: call.id.clone(),
                                result: ToolResult::error(format!(
                                    "Tool not found: {}",
                                    call.function.name
                                )),
                            });
                        }
                    }

                    // Add tool result to history
                    current_messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content: Some(content),
                        tool_calls: None,
                        tool_call_id: Some(call.id.clone()),
                        tool_name: Some(call.function.name.clone()),
                    });
                }

                iterations += 1;
                continue; // Loop again with new history
            }
        }

        // No tools or auto-execute disabled -> done
        return Ok(api_response);
    }

    // Fallback if loop ends
    Ok(ApiResponse {
        success: false,
        error: Some("Max iterations reached".to_string()),
        ..Default::default()
    })
}
