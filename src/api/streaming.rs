//! True streaming implementation for AI API responses
//!
//! This module provides proper Server-Sent Events (SSE) streaming support
//! for OpenAI-compatible APIs, including:
//!
//! - Real-time text streaming
//! - Tool call delta accumulation
//! - Usage statistics tracking
//!
//! Based on OpenAI's streaming specification:
//! - Each chunk contains a `delta` object with partial content
//! - Tool calls arrive as multiple delta chunks that must be accumulated
//! - Stream ends with `finish_reason: "stop"` or `finish_reason: "tool_calls"`

use crate::api::api::{ApiResponse, ToolCall, ToolCallFunction, Usage};
use crate::utils::debug::debug_print;
use anyhow::{anyhow, Result};
use reqwest::Response;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

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
    /// Thinking/reasoning content chunk (for models that support extended thinking)
    ThinkingDelta(String),
    /// Thinking started
    ThinkingStart,
    /// Thinking completed
    ThinkingEnd,
    /// Tool call started (first delta with id and name)
    ToolCallStart {
        index: usize,
        id: String,
        name: String,
    },
    /// Tool call arguments chunk
    ToolCallDelta {
        index: usize,
        arguments: String,
    },
    /// Tool call completed (all deltas accumulated)
    ToolCallComplete(ToolCall),
    /// Stream finished with reason
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

/// Process a streaming response from an OpenAI-compatible API
///
/// # Arguments
///
/// * `response` - The HTTP response with streaming body
/// * `callback` - Function called for each stream event
///
/// # Returns
///
/// The final accumulated response with all content and tool calls
pub async fn process_stream<F>(
    response: Response,
    mut callback: F,
) -> Result<ApiResponse>
where
    F: FnMut(StreamEvent),
{
    let mut accumulated_content = String::new();
    let mut tool_accumulators: HashMap<usize, ToolCallAccumulator> = HashMap::new();
    let mut finish_reason = String::new();
    let mut usage: Option<Usage> = None;
    let mut model = String::new();
    let mut stream_id = String::new();

    // Read the response as text chunks
    let body = response.text().await?;

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Determine the data to parse based on format:
        // - SSE format: lines start with "data: "
        // - Ollama/NDJSON format: plain JSON objects
        let data = if line.starts_with("data: ") {
            let data = &line[6..]; // Skip "data: " prefix
            // Stream end marker (SSE)
            if data == "[DONE]" {
                break;
            }
            data
        } else if line.starts_with("{") {
            // Plain JSON line (Ollama format)
            line
        } else {
            // Skip unknown lines
            continue;
        };

        // Check for Ollama's done marker
        if let Ok(ollama_check) = serde_json::from_str::<serde_json::Value>(data) {
            if ollama_check.get("done").and_then(|v| v.as_bool()) == Some(true) {
                // Ollama stream complete - extract final message if present
                if let Some(message) = ollama_check.get("message") {
                    if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                        if !content.is_empty() && !accumulated_content.contains(content) {
                            accumulated_content.push_str(content);
                            callback(StreamEvent::TextDelta(content.to_string()));
                        }
                    }
                    
                    // Extract tool calls from final Ollama response
                    if let Some(tool_calls) = message.get("tool_calls").and_then(|tc| tc.as_array()) {
                        for (index, tc) in tool_calls.iter().enumerate() {
                            if let Some(function) = tc.get("function") {
                                let name = function.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                let arguments = if let Some(args) = function.get("arguments") {
                                    if args.is_string() {
                                        args.as_str().unwrap_or("{}").to_string()
                                    } else {
                                        serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string())
                                    }
                                } else {
                                    "{}".to_string()
                                };
                                
                                let id = format!("ollama_call_{}", index);
                                
                                // Only add if not already tracked
                                if !tool_accumulators.contains_key(&index) {
                                    callback(StreamEvent::ToolCallStart {
                                        index,
                                        id: id.clone(),
                                        name: name.clone(),
                                    });
                                    callback(StreamEvent::ToolCallDelta {
                                        index,
                                        arguments: arguments.clone(),
                                    });
                                    tool_accumulators.insert(index, ToolCallAccumulator {
                                        id,
                                        name,
                                        arguments,
                                    });
                                }
                            }
                        }
                        finish_reason = "tool_calls".to_string();
                    } else {
                        finish_reason = "stop".to_string();
                    }
                } else {
                    finish_reason = "stop".to_string();
                }
                break;
            }
        }

        // Parse the JSON chunk (try OpenAI format first)
        let chunk: StreamChunk = match serde_json::from_str(data) {
            Ok(c) => c,
            Err(_) => {
                // Try Ollama format
                if let Ok(ollama) = serde_json::from_str::<serde_json::Value>(data) {
                    // Extract content from Ollama response
                    if let Some(message) = ollama.get("message") {
                        // Extract text content
                        if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                            if !content.is_empty() {
                                accumulated_content.push_str(content);
                                callback(StreamEvent::TextDelta(content.to_string()));
                            }
                        }
                        
                        // Extract tool calls from Ollama response
                        // Ollama format: { "message": { "tool_calls": [{ "function": { "name": "...", "arguments": {...} } }] } }
                        if let Some(tool_calls) = message.get("tool_calls").and_then(|tc| tc.as_array()) {
                            for (index, tc) in tool_calls.iter().enumerate() {
                                if let Some(function) = tc.get("function") {
                                    let name = function.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                    // Ollama returns arguments as object, convert to string
                                    let arguments = if let Some(args) = function.get("arguments") {
                                        if args.is_string() {
                                            args.as_str().unwrap_or("{}").to_string()
                                        } else {
                                            serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string())
                                        }
                                    } else {
                                        "{}".to_string()
                                    };
                                    
                                    // Generate a unique ID for the tool call
                                    let id = format!("ollama_call_{}", index);
                                    
                                    callback(StreamEvent::ToolCallStart {
                                        index,
                                        id: id.clone(),
                                        name: name.clone(),
                                    });
                                    callback(StreamEvent::ToolCallDelta {
                                        index,
                                        arguments: arguments.clone(),
                                    });
                                    
                                    // Store in accumulator
                                    tool_accumulators.insert(index, ToolCallAccumulator {
                                        id,
                                        name,
                                        arguments,
                                    });
                                }
                            }
                        }
                    }
                    continue;
                }
                debug_print(&format!("Failed to parse stream chunk: {}", data));
                continue;
            }
        };

        // Extract stream metadata
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

        // Track usage if provided
        if let Some(u) = chunk.usage {
            usage = Some(Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            });
        }

        // Process each choice
        for choice in chunk.choices {
            // Check finish reason
            if let Some(reason) = &choice.finish_reason {
                finish_reason = reason.clone();
            }

            let delta = choice.delta;

            // Handle thinking/reasoning content (OpenAI o1/o3)
            if let Some(reasoning) = delta.reasoning_content {
                if !reasoning.is_empty() {
                    callback(StreamEvent::ThinkingDelta(reasoning));
                }
            }
            
            // Handle thinking content (Ollama deepseek-r1, qwq, etc.)
            if let Some(thinking) = delta.thinking {
                if !thinking.is_empty() {
                    callback(StreamEvent::ThinkingDelta(thinking));
                }
            }
            
            // Handle text content
            if let Some(content) = delta.content {
                if !content.is_empty() {
                    accumulated_content.push_str(&content);
                    callback(StreamEvent::TextDelta(content));
                }
            }

            // Handle tool calls
            if let Some(tool_calls) = delta.tool_calls {
                for tc_delta in tool_calls {
                    let idx = tc_delta.index;

                    // Get or create accumulator for this tool call
                    let accumulator = tool_accumulators.entry(idx).or_default();

                    // First delta contains id and name
                    if let Some(id) = tc_delta.id {
                        accumulator.id = id.clone();
                        if let Some(func) = &tc_delta.function {
                            if let Some(name) = &func.name {
                                accumulator.name = name.clone();
                                callback(StreamEvent::ToolCallStart {
                                    index: idx,
                                    id: accumulator.id.clone(),
                                    name: accumulator.name.clone(),
                                });
                            }
                        }
                    }

                    // Accumulate arguments
                    if let Some(func) = tc_delta.function {
                        if let Some(args) = func.arguments {
                            accumulator.arguments.push_str(&args);
                            callback(StreamEvent::ToolCallDelta {
                                index: idx,
                                arguments: args,
                            });
                        }
                    }
                }
            }
        }
    }

    // Finalize tool calls
    let tool_calls: Option<Vec<ToolCall>> = if tool_accumulators.is_empty() {
        None
    } else {
        let mut calls: Vec<(usize, ToolCall)> = tool_accumulators
            .into_iter()
            .map(|(idx, acc)| {
                let tc = acc.to_tool_call();
                callback(StreamEvent::ToolCallComplete(tc.clone()));
                (idx, tc)
            })
            .collect();
        // Sort by index to maintain order
        calls.sort_by_key(|(idx, _)| *idx);
        Some(calls.into_iter().map(|(_, tc)| tc).collect())
    };

    // Send finish event
    callback(StreamEvent::Finish {
        reason: finish_reason.clone(),
        usage: usage.clone(),
    });

    Ok(ApiResponse {
        response: accumulated_content,
        success: true,
        error: None,
        usage,
        tool_calls,
        model: Some(model),
        created: None,
        reasoning_content: None,
    })
}

/// Build a streaming request body for OpenAI-compatible APIs
/// 
/// # Arguments
/// * `model` - The model name
/// * `messages` - The messages array
/// * `tools` - Optional tools array
/// * `temperature` - Temperature setting
/// * `max_tokens` - Max tokens limit
/// * `include_stream_options` - Whether to include stream_options (not supported by Z.AI)
pub fn build_streaming_request(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    temperature: f32,
    max_tokens: u32,
) -> Value {
    build_streaming_request_with_options(model, messages, tools, temperature, max_tokens, true)
}

/// Build a streaming request body with configurable options
/// 
/// Z.AI has specific requirements:
/// - Does not support stream_options (pass include_stream_options: false)
/// - Does not support tool_choice with streaming (pass include_tool_choice: false)
pub fn build_streaming_request_with_options(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    temperature: f32,
    max_tokens: u32,
    include_stream_options: bool,
) -> Value {
    build_streaming_request_full(model, messages, tools, temperature, max_tokens, include_stream_options, true)
}

/// Build a streaming request body with full control over all options
pub fn build_streaming_request_full(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    temperature: f32,
    max_tokens: u32,
    include_stream_options: bool,
    include_tool_choice: bool,
) -> Value {
    let mut request = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": true
    });

    // Only include stream_options for providers that support it (OpenAI, OpenRouter)
    // Z.AI returns error 1210 (Invalid parameters) when stream_options is included
    if include_stream_options {
        request["stream_options"] = serde_json::json!({
            "include_usage": true
        });
    }

    if let Some(tools) = tools {
        if !tools.is_empty() {
            request["tools"] = serde_json::json!(tools);
            // Z.AI does NOT support tool_choice with streaming - only add for other providers
            if include_tool_choice {
                request["tool_choice"] = serde_json::json!("auto");
            }
        }
    }

    request
}

/// Build a non-streaming request body for OpenAI-compatible APIs
pub fn build_request(
    model: &str,
    messages: &[Value],
    tools: Option<&[Value]>,
    temperature: f32,
    max_tokens: u32,
) -> Value {
    let mut request = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens
    });

    if let Some(tools) = tools {
        if !tools.is_empty() {
            request["tools"] = serde_json::json!(tools);
            request["tool_choice"] = serde_json::json!("auto");
        }
    }

    request
}

/// Parse a non-streaming response from OpenAI-compatible APIs
pub fn parse_response(response_json: &Value) -> Result<ApiResponse> {
    let choices = response_json["choices"]
        .as_array()
        .ok_or_else(|| anyhow!("No choices in response"))?;

    let choice = choices
        .first()
        .ok_or_else(|| anyhow!("Empty choices array"))?;

    let message = &choice["message"];
    let content = message["content"].as_str().unwrap_or("").to_string();

    // Parse tool calls
    let tool_calls = message["tool_calls"]
        .as_array()
        .map(|calls| {
            calls
                .iter()
                .filter_map(|tc| {
                    Some(ToolCall {
                        id: tc["id"].as_str()?.to_string(),
                        r#type: "function".to_string(),
                        function: ToolCallFunction {
                            name: tc["function"]["name"].as_str()?.to_string(),
                            arguments: tc["function"]["arguments"].as_str()?.to_string(),
                        },
                    })
                })
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty());

    // Parse usage
    let usage = response_json["usage"].as_object().map(|u| Usage {
        prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
    });

    let model = response_json["model"].as_str().map(String::from);
    let created = response_json["created"].as_u64();

    // Check for reasoning content (Claude/Z.AI thinking mode)
    let reasoning_content = message["reasoning_content"]
        .as_str()
        .map(String::from);

    Ok(ApiResponse {
        response: content,
        success: true,
        error: None,
        usage,
        tool_calls,
        model,
        created,
        reasoning_content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stream_chunk() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1694268190,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_parse_tool_call_delta() {
        let json = r#"{"id":"chatcmpl-123","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        let tool_calls = chunk.choices[0].delta.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls[0].id, Some("call_abc".to_string()));
    }

    #[test]
    fn test_build_streaming_request() {
        let messages = vec![serde_json::json!({"role": "user", "content": "Hi"})];
        let request = build_streaming_request("gpt-4", &messages, None, 0.7, 2048);
        assert_eq!(request["stream"], true);
        assert!(request["stream_options"]["include_usage"].as_bool().unwrap());
    }

    #[test]
    fn test_build_request_with_tools() {
        let messages = vec![serde_json::json!({"role": "user", "content": "Hi"})];
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {"name": "test", "parameters": {}}
        })];
        let request = build_request("gpt-4", &messages, Some(&tools), 0.7, 2048);
        assert!(request["tools"].is_array());
        assert_eq!(request["tool_choice"], "auto");
    }

    #[test]
    fn test_tool_call_accumulator() {
        let mut acc = ToolCallAccumulator::default();
        acc.id = "call_123".to_string();
        acc.name = "get_weather".to_string();
        acc.arguments = r#"{"location":"Paris"}"#.to_string();

        let tc = acc.to_tool_call();
        assert_eq!(tc.id, "call_123");
        assert_eq!(tc.function.name, "get_weather");
        assert_eq!(tc.function.arguments, r#"{"location":"Paris"}"#);
    }
}

