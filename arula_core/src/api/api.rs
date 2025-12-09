use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Z.AI specific error types
#[derive(Debug, thiserror::Error)]
pub enum ZAIApiError {
    #[error("Authentication failed: {message}")]
    AuthenticationError { message: String },

    #[error("Rate limit exceeded: {message}")]
    RateLimitError { message: String },

    #[error("Request timeout: {message}")]
    TimeoutError { message: String },

    #[error("Invalid request: {message} (Status: {status_code})")]
    RequestError { message: String, status_code: u16 },

    #[error("Internal server error: {message}")]
    InternalError { message: String },

    #[error("Server overloaded: {message}")]
    ServerFlowExceedError { message: String },

    #[error("API error: {message} (Status: {status_code})")]
    StatusError { message: String, status_code: u16 },

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}

impl ZAIApiError {
    pub fn from_status_code(status: u16, message: &str) -> Self {
        match status {
            401 => Self::AuthenticationError {
                message: message.to_string(),
            },
            429 => Self::RateLimitError {
                message: message.to_string(),
            },
            500..=599 => Self::InternalError {
                message: message.to_string(),
            },
            _ => Self::StatusError {
                message: message.to_string(),
                status_code: status,
            },
        }
    }
}

/// Debug print helper that checks ARULA_DEBUG environment variable
fn debug_print(msg: &str) {
    if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
        println!("🔧 DEBUG: {}", msg);
        // Also log to file
        crate::utils::logger::debug(msg);
    }
}

/// Log raw HTTP request details
fn log_http_request(
    method: &str,
    url: &str,
    headers: &reqwest::header::HeaderMap,
    body: Option<&str>,
) {
    let mut log_msg = format!("=== HTTP REQUEST ===\n{} {}\n", method, url);

    // Log headers
    log_msg.push_str("HEADERS:\n");
    for (name, value) in headers {
        log_msg.push_str(&format!(
            "  {}: {}\n",
            name,
            value.to_str().unwrap_or("<binary>")
        ));
    }

    // Log body if present
    if let Some(body_content) = body {
        log_msg.push_str(&format!(
            "BODY ({} bytes):\n{}\n",
            body_content.len(),
            body_content
        ));
    } else {
        log_msg.push_str("BODY: <empty>\n");
    }

    log_msg.push_str("===================\n");

    crate::utils::logger::info(&log_msg);
}

/// Log raw HTTP response details (without consuming the body)
fn log_http_response(response: &reqwest::Response) {
    let status = response.status();
    let url = response.url();
    let mut log_msg = format!("=== HTTP RESPONSE ===\n{} {}\n", status, url);

    // Log response headers
    log_msg.push_str("HEADERS:\n");
    for (name, value) in response.headers() {
        log_msg.push_str(&format!(
            "  {}: {}\n",
            name,
            value.to_str().unwrap_or("<binary>")
        ));
    }

    // Note: Response body not logged here to avoid consuming it
    log_msg.push_str("BODY: <not logged to avoid consumption>\n");
    log_msg.push_str("===================\n");

    crate::utils::logger::info(&log_msg);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool name for Ollama tool responses (Ollama uses tool_name instead of tool_call_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiResponse {
    pub response: String,
    pub success: bool,
    pub error: Option<String>,
    pub usage: Option<Usage>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub model: Option<String>,
    pub created: Option<u64>,
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZAIUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost_estimate: Option<f64>, // in USD for some models
}

impl ZAIUsage {
    pub fn log_usage(&self, model: &str) {
        eprintln!(
            "🔧 Z.AI Usage [{}]: {} prompt + {} completion = {} total tokens",
            model, self.prompt_tokens, self.completion_tokens, self.total_tokens
        );
        if let Some(cost) = self.cost_estimate {
            eprintln!("💰 Estimated cost: ${:.6}", cost);
        }
    }
}

#[derive(Debug, Clone)]
pub enum StreamingResponse {
    Start,
    Chunk(String),
    End(ApiResponse),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AIProvider {
    OpenAI,
    Claude,
    Ollama,
    ZAiCoding,
    OpenRouter,
    Custom,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    pub provider: AIProvider,
    pub endpoint: String,
    api_key: String,
    model: String,
}

impl ApiClient {
    pub fn new(provider: String, endpoint: String, api_key: String, model: String) -> Self {
        // First try to detect provider by name
        let mut provider_type = match provider.to_lowercase().as_str() {
            "openai" => AIProvider::OpenAI,
            "claude" | "anthropic" => AIProvider::Claude,
            "ollama" => AIProvider::Ollama,
            "z.ai coding plan" | "z.ai" | "zai" => AIProvider::ZAiCoding,
            "openrouter" => AIProvider::OpenRouter,
            _ => AIProvider::Custom,
        };

        // Fallback: Also check endpoint URL to detect Z.AI even if provider name doesn't match
        // This ensures proper handling for Z.AI-specific features like stream_options exclusion
        if matches!(provider_type, AIProvider::Custom) && endpoint.contains("api.z.ai") {
            provider_type = AIProvider::ZAiCoding;
        }

        // Normalize endpoint URL - remove trailing slashes and common API paths
        // This prevents double paths like /api/chat/api/chat
        let normalized_endpoint = if endpoint.contains("api.z.ai") && endpoint.contains("/v4") {
            // Special handling for Z.AI v4 endpoints - don't trim the /v4 part
            endpoint.trim_end_matches('/').to_string()
        } else {
            endpoint
                .trim_end_matches('/')
                .trim_end_matches("/api/chat")
                .trim_end_matches("/api/generate")
                .trim_end_matches("/v1/chat/completions")
                .trim_end_matches("/v1")
                .trim_end_matches("/chat/completions")
                .to_string()
        };

        if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
            debug_print(&format!(
                "DEBUG: Provider = {:?}, Input = {}",
                provider_type, provider
            ));
            debug_print(&format!(
                "DEBUG: API Key length = {}, endpoint = {}",
                api_key.len(),
                endpoint
            ));
            debug_print(&format!("DEBUG: Model = {}", model));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("arula-cli/1.0")
            .http1_title_case_headers()
            .tcp_nodelay(true)
            .connection_verbose(std::env::var("ARULA_DEBUG").unwrap_or_default() == "1")
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .expect("Failed to create HTTP client");

        // Initialize OpenAI client for streaming support
        Self {
            client,
            provider: provider_type,
            endpoint: normalized_endpoint,
            api_key,
            model,
        }
    }

    /// Get the current model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Send a raw streaming request and return the HTTP response
    /// Used by the unified stream.rs module
    pub async fn make_streaming_request(
        &self,
        request_body: serde_json::Value,
    ) -> Result<reqwest::Response> {
        // Align streaming endpoints with provider-specific REST paths
        let request_url = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint),
            AIProvider::Claude => format!("{}/v1/messages", self.endpoint),
            AIProvider::OpenAI | AIProvider::OpenRouter => {
                format!("{}/chat/completions", self.endpoint)
            }
            AIProvider::ZAiCoding => {
                // Z.AI uses the endpoint with /chat/completions appended
                if self.endpoint.ends_with("/v4") {
                    format!("{}/chat/completions", self.endpoint)
                } else {
                    self.endpoint.clone()
                }
            }
            AIProvider::Custom => self.endpoint.clone(),
        };

        let mut request_builder = self
            .client
            .post(&request_url)
            .header("Content-Type", "application/json");

        match self.provider {
            AIProvider::Claude => {
                request_builder = request_builder
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01");
            }
            AIProvider::OpenAI | AIProvider::OpenRouter => {
                request_builder =
                    request_builder.header("Authorization", format!("Bearer {}", self.api_key));
            }
            AIProvider::ZAiCoding => {
                // Check if using Anthropic-compatible endpoint
                if self.endpoint.contains("/api/anthropic") {
                    // Use Anthropic-style headers for the Anthropic-compatible endpoint
                    request_builder = request_builder
                        .header("x-api-key", &self.api_key)
                        .header("anthropic-version", "2023-06-01");
                } else {
                    // Use Bearer token for Coding Plan endpoint
                    request_builder = request_builder
                        .header("Authorization", format!("Bearer {}", self.api_key))
                        .header("Accept-Language", "en-US,en");
                }
            }
            // Ollama usually doesn't need auth, but Custom might
            AIProvider::Custom => {
                if !self.api_key.is_empty() {
                    request_builder =
                        request_builder.header("Authorization", format!("Bearer {}", self.api_key));
                }
            }
            _ => {}
        }

        // Log the request if debug mode is enabled
        if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
            let body_str = serde_json::to_string_pretty(&request_body).unwrap_or_default();
            println!(
                "🔧 DEBUG: Streaming request to {}: {}",
                request_url, body_str
            );
        }

        let response = request_builder.json(&request_body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();

            // Check for specific Z.AI errors
            if self.provider == AIProvider::ZAiCoding {
                return Err(ZAIApiError::from_status_code(status.as_u16(), &text).into());
            }

            return Err(anyhow!("API Error {}: {}", status, text));
        }

        Ok(response)
    }

    pub async fn send_message(
        &self,
        message: &str,
        conversation_history: Option<Vec<ChatMessage>>,
    ) -> Result<ApiResponse> {
        let mut messages = Vec::new();

        // Add system message
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some("You are ARULA, an Autonomous AI Interface assistant. You help users with coding, shell commands, and general software development tasks. Be concise, helpful, and provide practical solutions.".to_string()),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
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
            tool_name: None,
        });

        // Use the unified send_request method without tools
        self.send_request(messages, None).await
    }

    /// Unified request method that handles all providers dynamically
    async fn send_request(
        &self,
        messages: Vec<ChatMessage>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> Result<ApiResponse> {
        // Load configuration
        let config = crate::utils::config::Config::load_or_default()?;
        let thinking_enabled = config.get_thinking_enabled().unwrap_or(false);

        // Build request body based on provider
        let request_body = match self.provider {
            AIProvider::Claude => {
                // Claude-specific request format
                let mut request = json!({
                    "model": self.model,
                    "max_tokens": 4096,
                    "messages": messages.iter().map(|msg| {
                        let mut msg_obj = json!({
                            "role": msg.role,
                        });
                        if let Some(content) = &msg.content {
                            msg_obj["content"] = json!(content);
                        }
                        msg_obj
                    }).collect::<Vec<_>>()
                });

                if let Some(tools) = tools {
                    request["tools"] = json!(tools);
                }

                request
            }
            AIProvider::Ollama => {
                // Ollama-specific request format
                let mut request = json!({
                    "model": self.model,
                    "messages": messages.iter().map(|msg| {
                        let mut msg_obj = json!({
                            "role": msg.role,
                        });
                        if let Some(content) = &msg.content {
                            msg_obj["content"] = json!(content);
                        }

                        // Add tool-related fields for Ollama
                        if let Some(tool_calls) = &msg.tool_calls {
                            let converted: Vec<Value> = tool_calls.iter().map(|tc| {
                                let args = serde_json::from_str::<Value>(&tc.function.arguments)
                                    .unwrap_or_else(|_| json!({}));
                                json!({
                                    "function": {
                                        "name": tc.function.name,
                                        "arguments": args
                                    }
                                })
                            }).collect();
                            msg_obj["tool_calls"] = json!(converted);
                        }

                        msg_obj
                    }).collect::<Vec<_>>(),
                    "stream": false
                });

                if let Some(tools) = tools {
                    request["tools"] = json!(tools);
                }

                // Add Ollama-specific options
                request["options"] = json!({
                    "temperature": 0.7,
                    "num_predict": 4096
                });

                request
            }
            AIProvider::ZAiCoding => {
                // Check if using Anthropic-compatible endpoint
                let is_anthropic_endpoint = self.endpoint.contains("/api/anthropic");
                
                if is_anthropic_endpoint {
                    // Use Anthropic Messages API format
                    // Extract system message
                    let system_content: Option<String> = messages
                        .iter()
                        .find(|m| m.role == "system")
                        .and_then(|m| m.content.clone());

                    // Build messages array, excluding system messages
                    let anthropic_messages: Vec<Value> = messages
                        .into_iter()
                        .filter(|msg| msg.role != "system")
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
                                    
                                    if let Some(text) = &msg.content {
                                        if !text.is_empty() {
                                            content_blocks.push(json!({
                                                "type": "text",
                                                "text": text
                                            }));
                                        }
                                    }
                                    
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
                        "model": self.model,
                        "max_tokens": 4096,
                        "messages": anthropic_messages,
                        "stream": false
                    });

                    if let Some(system) = system_content {
                        request["system"] = json!(system);
                    }

                    // Convert tools to Anthropic format
                    if let Some(t) = tools {
                        if !t.is_empty() {
                            let anthropic_tools: Vec<Value> = t.iter().filter_map(|tool| {
                                let func = tool.get("function")?;
                                let name = func.get("name")?.as_str()?;
                                let description = func.get("description")?.as_str().unwrap_or("");
                                let parameters = func.get("parameters").cloned()
                                    .unwrap_or(json!({"type": "object", "properties": {}}));
                                
                                Some(json!({
                                    "name": name,
                                    "description": description,
                                    "input_schema": parameters
                                }))
                            }).collect();
                            
                            if !anthropic_tools.is_empty() {
                                request["tools"] = json!(anthropic_tools);
                            }
                        }
                    }

                    request
                } else {
                    // Original Z.AI Coding Plan format
                    let is_zai_endpoint = self.endpoint.contains("api.z.ai");

                    // Convert ChatMessage format to plain objects for Z.AI
                    let zai_messages: Vec<Value> = messages
                        .into_iter()
                        .filter_map(|msg| {
                            // Skip assistant messages that only have tool_calls (no content)
                            if msg.role == "assistant"
                                && msg.content.is_none()
                                && msg.tool_calls.is_some()
                            {
                                return None;
                            }

                            // Convert tool role messages to user messages (Z.AI doesn't support tool role)
                            if msg.role == "tool" {
                                // Format tool result as a user message
                                let tool_name = msg.tool_name.as_deref().unwrap_or("unknown_tool");
                                let content = msg.content.as_deref().unwrap_or("");
                                return Some(json!({
                                    "role": "user",
                                    "content": format!("Tool {} returned: {}", tool_name, content)
                                }));
                            }

                            // Regular messages
                            Some(json!({
                                "role": msg.role,
                                "content": msg.content.unwrap_or_default()
                            }))
                        })
                        .collect();

                    // Set up model-specific parameters based on official GLM specs
                    let max_tokens = match self.model.as_str() {
                        "GLM-4.6" => 65536,
                        "GLM-4.5" | "GLM-4.5-AIR" | "GLM-4.5-X" | "GLM-4.5-AIRX" | "GLM-4.5-FLASH"
                        | "GLM-4.5V" => 65536,
                        "GLM-4-32B-0414-128K" => 16384,
                        _ => 2048,
                    };

                    let mut request = json!({
                        "model": self.model,
                        "messages": zai_messages,
                        "max_tokens": max_tokens,
                        "stream": false
                    });

                    // Add thinking mode if enabled
                    if thinking_enabled {
                        request["thinking"] = serde_json::json!({
                            "type": "enabled"
                        });
                    }

                    // Only add tools for non-coding endpoints and if tools are provided
                    if !is_zai_endpoint {
                        if let Some(t) = tools {
                            if !t.is_empty() {
                                request["tools"] = json!(t);
                            }
                        }
                    } else if let Some(t) = tools {
                        if !t.is_empty() {
                            request["tools"] = json!(t);
                        }
                    }

                    request
                }
            }
            AIProvider::OpenAI | AIProvider::OpenRouter | AIProvider::Custom => {
                // OpenAI-compatible request format
                let mut request = json!({
                    "model": self.model,
                    "messages": messages.iter().map(|msg| {
                        let mut msg_obj = json!({
                            "role": msg.role,
                        });
                        if let Some(content) = &msg.content {
                            msg_obj["content"] = json!(content);
                        } else if msg.tool_calls.is_some() {
                            msg_obj["content"] = json!(null);
                        }

                        // Add tool-related fields
                        if let Some(tool_calls) = &msg.tool_calls {
                            msg_obj["tool_calls"] = json!(tool_calls);
                        }

                        if let Some(tool_call_id) = &msg.tool_call_id {
                            msg_obj["tool_call_id"] = json!(tool_call_id);
                        }

                        msg_obj
                    }).collect::<Vec<_>>(),
                    "temperature": 0.7,
                    "max_tokens": 4096,
                    "stream": false
                });

                // Add tools if provided
                if let Some(t) = tools {
                    if !t.is_empty() {
                        request["tools"] = json!(t);
                        request["tool_choice"] = json!("auto");
                    }
                }

                // Add reasoning effort when thinking is enabled
                if thinking_enabled {
                    request["reasoning_effort"] = serde_json::json!("medium");
                }

                request
            }
        };

        // Determine the endpoint URL
        let endpoint_url = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint),
            AIProvider::Claude => format!("{}/v1/messages", self.endpoint),
            AIProvider::ZAiCoding => {
                // Check if Anthropic-compatible endpoint (already has full path)
                if self.endpoint.contains("/api/anthropic") {
                    self.endpoint.clone()
                } else if self.endpoint.ends_with("/v4") {
                    format!("{}/chat/completions", self.endpoint)
                } else {
                    self.endpoint.clone()
                }
            }
            AIProvider::OpenAI | AIProvider::OpenRouter | AIProvider::Custom => {
                format!("{}/chat/completions", self.endpoint)
            }
        };

        // Create HTTP client
        let client = if matches!(self.provider, AIProvider::ZAiCoding) {
            // Create a new client specifically for Z.AI to force HTTP/1.1
            Client::builder()
                .timeout(Duration::from_secs(60))
                .user_agent("arula-cli/1.0")
                .http1_only() // Force HTTP/1.1 for Z.AI compatibility
                .tcp_nodelay(true)
                .connection_verbose(std::env::var("ARULA_DEBUG").unwrap_or_default() == "1")
                .build()
                .expect("Failed to create Z.AI HTTP client")
        } else {
            self.client.clone()
        };

        // Build request with appropriate headers
        let mut request_builder = client
            .post(&endpoint_url)
            .header("Content-Type", "application/json");

        // Add authorization headers based on provider
        match self.provider {
            AIProvider::Claude => {
                request_builder = request_builder
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01");
            }
            AIProvider::OpenAI | AIProvider::OpenRouter => {
                if !self.api_key.is_empty() {
                    request_builder =
                        request_builder.header("Authorization", format!("Bearer {}", self.api_key));
                }
            }
            AIProvider::ZAiCoding => {
                // Check if using Anthropic-compatible endpoint
                if self.endpoint.contains("/api/anthropic") {
                    request_builder = request_builder
                        .header("x-api-key", &self.api_key)
                        .header("anthropic-version", "2023-06-01");
                } else if !self.api_key.is_empty() {
                    request_builder =
                        request_builder.header("Authorization", format!("Bearer {}", self.api_key));
                }
            }
            AIProvider::Custom => {
                if !self.api_key.is_empty() {
                    request_builder =
                        request_builder.header("Authorization", format!("Bearer {}", self.api_key));
                }
            }
            _ => {} // Ollama usually doesn't need auth
        }

        // Log the request if debug mode is enabled
        if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
            let body_str = serde_json::to_string_pretty(&request_body).unwrap_or_default();
            println!(
                "🔧 DEBUG: Sending request to {}: {}",
                endpoint_url, body_str
            );
        }

        // Send the request
        let response = request_builder.json(&request_body).send().await?;

        // Handle the response
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();

            // Log the response for debugging
            if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                println!("🔧 DEBUG: API Response ({}): {}", status, text);
            }

            return Err(anyhow::anyhow!(
                "API request failed with status {}: {}",
                status,
                text
            ));
        }

        // Parse response based on provider
        match self.provider {
            AIProvider::Claude => {
                let response_text = response.text().await?;

                // Log the successful response if debug mode is enabled
                if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                    println!("🔧 DEBUG: API Response (200 OK): {}", response_text);
                }

                let response_json: serde_json::Value = serde_json::from_str(&response_text)?;

                let content = response_json
                    .get("content")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();

                Ok(ApiResponse {
                    response: content,
                    success: true,
                    error: None,
                    usage: None,
                    tool_calls: None,
                    model: Some(self.model.clone()),
                    created: None,
                    reasoning_content: None,
                })
            }
            AIProvider::Ollama => {
                let response_text = response.text().await?;

                // Log the successful response if debug mode is enabled
                if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                    println!("🔧 DEBUG: API Response (200 OK): {}", response_text);
                }

                let response_json: serde_json::Value = serde_json::from_str(&response_text)?;

                let content = response_json
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                Ok(ApiResponse {
                    response: content,
                    success: true,
                    error: None,
                    usage: None,
                    tool_calls: None,
                    model: Some(self.model.clone()),
                    created: None,
                    reasoning_content: None,
                })
            }
            AIProvider::ZAiCoding => {
                let response_text = response.text().await?;

                // Log the successful response if debug mode is enabled
                if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                    println!("🔧 DEBUG: API Response (200 OK): {}", response_text);
                }

                let response_json: serde_json::Value = serde_json::from_str(&response_text)?;

                // Check if this is an Anthropic-format response
                // Anthropic format has "content" array at top level, OpenAI has "choices"
                if response_json.get("content").is_some() && response_json.get("type").map(|t| t.as_str()) == Some(Some("message")) {
                    // Parse Anthropic Messages API response format
                    let content_array = response_json.get("content").and_then(|c| c.as_array());
                    
                    let mut text_content = String::new();
                    let mut tool_calls: Vec<ToolCall> = Vec::new();
                    
                    if let Some(blocks) = content_array {
                        for block in blocks {
                            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            
                            match block_type {
                                "text" => {
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        text_content.push_str(text);
                                    }
                                }
                                "tool_use" => {
                                    // Convert Anthropic tool_use to OpenAI-style ToolCall
                                    let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                    let input = block.get("input").cloned().unwrap_or(json!({}));
                                    
                                    tool_calls.push(ToolCall {
                                        id,
                                        r#type: "function".to_string(),
                                        function: ToolCallFunction {
                                            name,
                                            arguments: input.to_string(),
                                        },
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    
                    Ok(ApiResponse {
                        response: text_content,
                        success: true,
                        error: None,
                        usage: None,
                        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                        model: Some(self.model.clone()),
                        created: None,
                        reasoning_content: None,
                    })
                } else {
                    // Parse OpenAI-compatible format (for Coding Plan endpoint)
                    let content = response_json
                        .get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|c| c.get("message"))
                        .and_then(|m| m.get("content"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();

                    let tool_calls = response_json
                        .get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|c| c.get("message"))
                        .and_then(|m| m.get("tool_calls"))
                        .and_then(|tc| serde_json::from_value(tc.clone()).ok());

                    let reasoning_content = response_json
                        .get("choices")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|c| c.get("message"))
                        .and_then(|m| m.get("reasoning_content"))
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_string());

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage: None,
                        tool_calls,
                        model: Some(self.model.clone()),
                        created: None,
                        reasoning_content,
                    })
                }
            }
            AIProvider::OpenAI
            | AIProvider::OpenRouter
            | AIProvider::Custom => {
                // OpenAI-compatible response format
                let response_text = response.text().await?;

                // Log the successful response if debug mode is enabled
                if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                    println!("🔧 DEBUG: API Response (200 OK): {}", response_text);
                }

                let response_json: serde_json::Value = serde_json::from_str(&response_text)?;

                let content = response_json
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();

                // Extract tool calls if present
                let tool_calls = response_json
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("tool_calls"))
                    .and_then(|tc| serde_json::from_value(tc.clone()).ok());

                // Extract reasoning_content if present (for Z.AI and other reasoning models)
                let reasoning_content = response_json
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("reasoning_content"))
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string());

                Ok(ApiResponse {
                    response: content,
                    success: true,
                    error: None,
                    usage: None,
                    tool_calls,
                    model: Some(self.model.clone()),
                    created: None,
                    reasoning_content,
                })
            }
        }
    }

    pub async fn send_message_with_tools_sync(
        &self,
        messages: &[ChatMessage],
        tools: &[serde_json::Value],
    ) -> Result<ApiResponse> {
        let messages = messages.to_vec();
        let tools = if tools.is_empty() {
            None
        } else {
            Some(tools.to_vec())
        };

        // Use the unified send_request method with tools
        self.send_request(messages, tools).await
    }

    async fn send_openai_request(&self, messages: Vec<ChatMessage>) -> Result<ApiResponse> {
        // NOTE: Tools are intentionally NOT included here to allow normal conversation
        // Tools are only added when explicitly needed via send_message_with_tools

        // Check if thinking/reasoning is enabled
        let config = crate::utils::config::Config::load_or_default()?;
        let thinking_enabled = config.get_thinking_enabled().unwrap_or(false);

        let mut request_body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_tokens": 2048
        });

        // Add reasoning effort when thinking is enabled
        // OpenAI's reasoning_effort parameter works with GPT-5.1 and reasoning models
        // Note: Not supported for o3/o4-mini (they always reason), but adding it won't hurt
        if thinking_enabled {
            request_body["reasoning_effort"] = serde_json::json!("medium");
        }

        // Use provider-specific endpoint
        let request_url = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
            AIProvider::ZAiCoding => self.endpoint.clone(), // Z.AI uses the endpoint directly
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };
        let mut request_builder = self.client.post(&request_url).json(&request_body);

        // Add authorization header if API key is provided
        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        // Log the outgoing request
        let mut request_headers = reqwest::header::HeaderMap::new();
        if !self.api_key.is_empty() {
            request_headers.insert(
                "Authorization",
                format!("Bearer {}", self.api_key).parse().unwrap(),
            );
        }
        request_headers.insert("Content-Type", "application/json".parse().unwrap());
        let body_str = serde_json::to_string_pretty(&request_body).unwrap_or_default();
        log_http_request("POST", &request_url, &request_headers, Some(&body_str));

        let response = request_builder.send().await?;

        // Log the incoming response
        log_http_response(&response);

        if response.status().is_success() {
            let response_json: serde_json::Value = response.json().await?;

            if let Some(choices) = response_json["choices"].as_array() {
                if let Some(choice) = choices.first() {
                    let content = choice["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    // Handle tool calls
                    let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| {
                        calls
                            .iter()
                            .map(|tool_call| ToolCall {
                                id: tool_call["id"].as_str().unwrap_or_default().to_string(),
                                r#type: "function".to_string(),
                                function: ToolCallFunction {
                                    name: tool_call["function"]["name"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                    arguments: tool_call["function"]["arguments"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                },
                            })
                            .collect::<Vec<_>>()
                    });

                    // Extract reasoning content if present (for reasoning models)
                    let reasoning_content = choice["message"]["reasoning_content"]
                        .as_str()
                        .map(|s| s.to_string())
                        .or_else(|| {
                            // Also check response-level reasoning
                            response_json["reasoning"]["summary"]
                                .as_str()
                                .map(|s| s.to_string())
                        });

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage: None, // TODO: Parse usage from response if needed
                        tool_calls,
                        model: Some(self.model.clone()),
                        created: Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        reasoning_content,
                    })
                } else {
                    Ok(ApiResponse {
                        response: "No response received".to_string(),
                        success: false,
                        error: Some("No choices in response".to_string()),
                        usage: None,
                        tool_calls: None,
                        model: Some(self.model.clone()),
                        created: Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        reasoning_content: None,
                    })
                }
            } else {
                Ok(ApiResponse {
                    response: "No response received".to_string(),
                    success: false,
                    error: Some("No choices in response".to_string()),
                    usage: None,
                    tool_calls: None,
                    model: Some(self.model.clone()),
                    created: Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    ),
                    reasoning_content: None,
                })
            }
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("OpenAI API request failed: {}", error_text))
        }
    }

    async fn send_claude_request(&self, messages: Vec<ChatMessage>) -> Result<ApiResponse> {
        // Check if thinking is enabled
        let config = crate::utils::config::Config::load_or_default()?;
        let thinking_enabled = config.get_thinking_enabled().unwrap_or(false);

        let claude_messages: Vec<Value> = messages
            .into_iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content.unwrap_or_default()
                })
            })
            .collect();

        let mut request = json!({
            "model": self.model,
            "messages": claude_messages,
            "max_tokens": 2048,
            "temperature": 0.7
        });

        // Add extended thinking for Claude when enabled
        // Claude uses "thinking" block with budget_tokens
        if thinking_enabled {
            request["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": 10000
            });
            // Extended thinking requires higher max_tokens
            request["max_tokens"] = json!(16000);
        }

        let request_url = format!("{}/v1/messages", self.endpoint);
        let mut request_builder = self
            .client
            .post(&request_url)
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&request);

        // Add authorization header if API key is provided
        if !self.api_key.is_empty() {
            request_builder = request_builder.header("x-api-key", &self.api_key);
        }

        // Log the outgoing request
        let mut request_headers = reqwest::header::HeaderMap::new();
        request_headers.insert("content-type", "application/json".parse().unwrap());
        request_headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
        if !self.api_key.is_empty() {
            request_headers.insert("x-api-key", self.api_key.parse().unwrap());
        }
        let body_str = serde_json::to_string_pretty(&request).unwrap_or_default();
        log_http_request("POST", &request_url, &request_headers, Some(&body_str));

        let response = request_builder.send().await?;

        // Log the incoming response
        log_http_response(&response);

        if response.status().is_success() {
            let claude_response: Value = response.json().await?;

            if let Some(content) = claude_response["content"].as_array() {
                let mut response_text = String::new();
                let mut thinking_text: Option<String> = None;

                // Parse content blocks - Claude can return thinking and text blocks
                for block in content {
                    match block["type"].as_str() {
                        Some("thinking") => {
                            // Capture thinking content
                            if let Some(thinking) = block["thinking"].as_str() {
                                thinking_text = Some(thinking.to_string());
                            }
                        }
                        Some("text") => {
                            if let Some(text) = block["text"].as_str() {
                                response_text.push_str(text);
                            }
                        }
                        _ => {}
                    }
                }

                if !response_text.is_empty() || thinking_text.is_some() {
                    return Ok(ApiResponse {
                        response: response_text,
                        success: true,
                        error: None,
                        usage: None, // Claude has different usage format
                        tool_calls: None,
                        model: Some(self.model.clone()),
                        created: Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        reasoning_content: thinking_text,
                    });
                }
            }

            Ok(ApiResponse {
                response: "Invalid Claude response format".to_string(),
                success: false,
                error: Some("Could not parse Claude response".to_string()),
                usage: None,
                tool_calls: None,
                model: Some(self.model.clone()),
                created: Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                ),
                reasoning_content: None,
            })
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("Claude API request failed: {}", error_text))
        }
    }

    async fn send_ollama_request(&self, messages: Vec<ChatMessage>) -> Result<ApiResponse> {
        // Check if thinking is enabled
        let config = crate::utils::config::Config::load_or_default()?;
        let thinking_enabled = config.get_thinking_enabled().unwrap_or(false);

        // Convert messages to Ollama format (compatible with OpenAI format)
        let ollama_messages: Vec<Value> = messages
            .iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content.as_ref().unwrap_or(&String::new())
                })
            })
            .collect();

        let mut request = json!({
            "model": self.model,
            "messages": ollama_messages,
            "stream": false,
            "options": {
                "temperature": 0.7,
                "num_predict": 2048
            }
        });

        // Add think option for Ollama when enabled
        // Works with models like deepseek-r1, qwq, etc.
        if thinking_enabled {
            request["options"]["think"] = json!(true);
        }

        // Use provider-specific endpoint
        let request_url = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
            AIProvider::ZAiCoding => self.endpoint.clone(), // Z.AI uses the endpoint directly
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };
        let request_builder = self.client.post(&request_url).json(&request);

        // Log the outgoing request
        let request_headers = reqwest::header::HeaderMap::new();
        let body_str = serde_json::to_string_pretty(&request).unwrap_or_default();
        log_http_request("POST", &request_url, &request_headers, Some(&body_str));

        let response = request_builder.send().await?;

        // Log the incoming response
        log_http_response(&response);

        if response.status().is_success() {
            let ollama_response: Value = response.json().await?;

            // Extract thinking content if present (for models like deepseek-r1)
            let thinking_content = ollama_response["message"]["reasoning_content"]
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| {
                    ollama_response["message"]["thinking"]
                        .as_str()
                        .map(|s| s.to_string())
                });

            if let Some(message) = ollama_response["message"].as_object() {
                if let Some(response_text) = message["content"].as_str() {
                    Ok(ApiResponse {
                        response: response_text.to_string(),
                        success: true,
                        error: None,
                        usage: None,
                        tool_calls: None,
                        model: Some(self.model.clone()),
                        created: Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        reasoning_content: thinking_content,
                    })
                } else {
                    Ok(ApiResponse {
                        response: "Invalid Ollama response format: missing content".to_string(),
                        success: false,
                        error: Some("Could not parse Ollama response: missing content".to_string()),
                        usage: None,
                        tool_calls: None,
                        model: Some(self.model.clone()),
                        created: Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        reasoning_content: thinking_content,
                    })
                }
            } else {
                Ok(ApiResponse {
                    response: "Invalid Ollama response format: missing message".to_string(),
                    success: false,
                    error: Some("Could not parse Ollama response: missing message".to_string()),
                    usage: None,
                    tool_calls: None,
                    model: Some(self.model.clone()),
                    created: Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    ),
                    reasoning_content: thinking_content,
                })
            }
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("Ollama API request failed: {}", error_text))
        }
    }

    async fn send_zai_request(&self, messages: Vec<ChatMessage>) -> Result<ApiResponse> {
        // Get Z.AI configuration from the config file
        let config = crate::utils::config::Config::load_or_default()?;
        let max_retries = config.get_zai_max_retries();
        let timeout = Duration::from_secs(config.get_zai_timeout_seconds());
        let thinking_enabled = config.get_thinking_enabled().unwrap_or(false);
        let usage_tracking = config.get_zai_usage_tracking_enabled().unwrap_or(true);

        // Convert ChatMessage format to plain objects for Z.AI
        // Filter out tool-related messages to avoid error 1210
        let zai_messages: Vec<Value> = messages
            .into_iter()
            .filter(|msg| {
                // Skip tool role messages
                if msg.role == "tool" {
                    return false;
                }
                // Skip assistant messages that only have tool_calls (no content)
                if msg.role == "assistant" && msg.content.is_none() && msg.tool_calls.is_some() {
                    return false;
                }
                true
            })
            .map(|msg| {
                // Build simple message with only role and content
                json!({
                    "role": msg.role,
                    "content": msg.content.unwrap_or_default()
                })
            })
            .collect();

        // Set up model-specific parameters based on official GLM specs
        let max_tokens = match self.model.as_str() {
            "GLM-4.6" => 65536, // Official default for GLM-4.6
            "GLM-4.5" | "GLM-4.5-AIR" | "GLM-4.5-X" | "GLM-4.5-AIRX" | "GLM-4.5-FLASH"
            | "GLM-4.5V" => 65536, // Official default for GLM-4.5 series
            "GLM-4-32B-0414-128K" => 16384, // Official default for older model
            _ => 2048,          // Fallback for other models
        };

        // Log the model being used for debugging
        debug_print(&format!(
            "Using model: {} with max_tokens: {}",
            self.model, max_tokens
        ));

        // Set up the base request with model-appropriate defaults
        let mut request = json!({
            "model": &self.model,
            "messages": zai_messages,
            "temperature": 0.7,   // Use default temperature for GLM models
            "max_tokens": max_tokens,
            "stream": false
        });

        // Add optional GLM parameters for better control
        // Note: Temperature and top_p should be mutually exclusive per GLM docs
        // We're using temperature=0.7 for balanced output
        request["do_sample"] = json!(true); // Enable sampling for diversity

        // Add thinking parameter for GLM-4.5 and above models
        if thinking_enabled
            && (self.model.starts_with("GLM-4.5") || self.model.starts_with("GLM-4.6"))
        {
            request["thinking"] = json!({
                "type": "enabled"
            });
        }

        // Log the final request payload
        let request_str = serde_json::to_string_pretty(&request).unwrap_or_default();
        debug_print(&format!("Final request payload: {}", request_str));

        // Implement retry logic
        for attempt in 0..=max_retries {
            // Use provider-specific endpoint
            let endpoint = match self.provider {
                AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
                AIProvider::ZAiCoding => self.endpoint.clone(), // Z.AI uses the endpoint directly
                _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
            };

            // Store a reference to the endpoint for logging
            let endpoint_str = endpoint.as_str();

            let mut request_builder = self
                .client
                .post(&endpoint) // Borrow endpoint here
                .timeout(timeout)
                .json(&request);

            // Add Z.AI recommended headers
            request_builder = request_builder
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Accept-Language", "en-US,en");

            // Log the outgoing request for this attempt
            let mut request_headers = reqwest::header::HeaderMap::new();
            request_headers.insert(
                "Authorization",
                format!("Bearer {}", self.api_key).parse().unwrap(),
            );
            request_headers.insert("Accept-Language", "en-US,en".parse().unwrap());

            // Add Content-Type header explicitly
            request_headers.insert("Content-Type", "application/json".parse().unwrap());

            let body_str = serde_json::to_string_pretty(&request).unwrap_or_default();

            // Log the full request for debugging
            debug_print(&format!(
                "Sending request to {}: {}",
                endpoint_str, body_str
            ));

            // Use provider-specific endpoint for logging
            let log_url = match self.provider {
                AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
                AIProvider::ZAiCoding => self.endpoint.clone(), // Z.AI uses the endpoint directly
                _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
            };
            log_http_request("POST", &log_url, &request_headers, Some(&body_str));

            let response = request_builder.send().await;
            match response {
                Ok(resp) => {
                    let status = resp.status();

                    // Log the incoming response
                    log_http_response(&resp);

                    if status.is_success() {
                        let response_json: serde_json::Value = resp.json().await?;

                        // Extract usage information
                        let zai_usage = if usage_tracking {
                            response_json["usage"].as_object().map(|usage| {
                                let prompt_tokens = usage
                                    .get("prompt_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let completion_tokens = usage
                                    .get("completion_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let total_tokens = usage
                                    .get("total_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);

                                ZAIUsage {
                                    prompt_tokens,
                                    completion_tokens,
                                    total_tokens,
                                    cost_estimate: self
                                        .calculate_zai_cost(&self.model, total_tokens),
                                }
                            })
                        } else {
                            None
                        };

                        // Log usage if tracking is enabled
                        if let Some(ref usage) = zai_usage {
                            usage.log_usage(&self.model);
                        }

                        if let Some(choices) = response_json["choices"].as_array() {
                            if let Some(choice) = choices.first() {
                                let content = choice["message"]["content"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();

                                // Handle tool calls
                                let tool_calls =
                                    choice["message"]["tool_calls"].as_array().map(|calls| {
                                        calls
                                            .iter()
                                            .map(|tool_call| ToolCall {
                                                id: tool_call["id"]
                                                    .as_str()
                                                    .unwrap_or_default()
                                                    .to_string(),
                                                r#type: "function".to_string(),
                                                function: ToolCallFunction {
                                                    name: tool_call["function"]["name"]
                                                        .as_str()
                                                        .unwrap_or_default()
                                                        .to_string(),
                                                    arguments: tool_call["function"]["arguments"]
                                                        .as_str()
                                                        .unwrap_or_default()
                                                        .to_string(),
                                                },
                                            })
                                            .collect::<Vec<_>>()
                                    });

                                // Convert Z.AI usage to our Usage struct
                                let usage = zai_usage.map(|z_usage| Usage {
                                    prompt_tokens: z_usage.prompt_tokens as u32,
                                    completion_tokens: z_usage.completion_tokens as u32,
                                    total_tokens: z_usage.total_tokens as u32,
                                });

                                return Ok(ApiResponse {
                                    response: content,
                                    success: true,
                                    error: None,
                                    usage,
                                    tool_calls,
                                    model: Some(self.model.clone()),
                                    created: response_json["created"].as_u64(),
                                    reasoning_content: response_json["choices"][0]["message"]
                                        ["reasoning_content"]
                                        .as_str()
                                        .map(|s| s.to_string()),
                                });
                            }
                        }

                        return Err(anyhow!("No choices in Z.AI response"));
                    } else {
                        // Handle HTTP errors with Z.AI-specific mapping
                        let error_body = resp.text().await.unwrap_or_default();
                        let api_error = ZAIApiError::from_status_code(status.as_u16(), &error_body);

                        // Log detailed error information
                        debug_print(&format!("Z.AI API error ({}): {}", status, error_body));

                        // Don't retry on client errors (4xx)
                        if status.is_client_error() {
                            return Err(anyhow!("Z.AI API error ({}): {}", status, api_error));
                        }

                        // Log retry attempt
                        if attempt < max_retries {
                            eprintln!(
                                "🔄 Z.AI request failed (attempt {}/{}), retrying...: {}",
                                attempt + 1,
                                max_retries + 1,
                                api_error
                            );
                            tokio::time::sleep(Duration::from_millis(
                                (1000 * (attempt + 1)) as u64,
                            ))
                            .await;
                            continue;
                        } else {
                            return Err(anyhow!(
                                "Z.AI API request failed after {} retries: {}",
                                max_retries,
                                api_error
                            ));
                        }
                    }
                }
                Err(e) => {
                    // Handle network errors
                    if attempt < max_retries {
                        eprintln!(
                            "🔄 Z.AI network error (attempt {}/{}) retrying...: {}",
                            attempt + 1,
                            max_retries + 1,
                            e
                        );
                        tokio::time::sleep(Duration::from_millis((1000 * (attempt + 1)) as u64))
                            .await;
                        continue;
                    } else {
                        return Err(anyhow!(
                            "Z.AI network error after {} retries: {}",
                            max_retries,
                            e
                        ));
                    }
                }
            }
        }

        unreachable!("Loop should have returned")
    }

    // Calculate estimated cost for Z.AI models
    fn calculate_zai_cost(&self, model: &str, total_tokens: u64) -> Option<f64> {
        // Rough cost estimates (per 1M tokens)
        let cost_per_million = match model {
            "GLM-4" | "GLM-4.6" => 0.0025,  // $2.50 per 1M tokens
            "GLM-4.5" => 0.0015,            // $1.50 per 1M tokens
            "claude-instant-1.2" => 0.0008, // $0.80 per 1M tokens
            _ => return None,
        };

        Some((total_tokens as f64 / 1_000_000.0) * cost_per_million)
    }

    async fn send_openrouter_request(&self, messages: Vec<ChatMessage>) -> Result<ApiResponse> {
        // OpenRouter uses OpenAI-compatible format
        // NOTE: Tools are intentionally NOT included here to allow normal conversation
        // Tools are only added when explicitly needed via send_message_with_tools
        let request_body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_tokens": 2048
        });

        // Use provider-specific endpoint
        let request_url = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };
        let mut request_builder = self.client.post(&request_url).json(&request_body);

        // Add authorization header if API key is provided
        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        // Add OpenRouter-specific headers
        request_builder = request_builder
            .header("HTTP-Referer", "https://github.com/arula-cli/arula-cli")
            .header("X-Title", "ARULA CLI");

        // Log the outgoing request
        let mut request_headers = reqwest::header::HeaderMap::new();
        if !self.api_key.is_empty() {
            request_headers.insert(
                "Authorization",
                format!("Bearer {}", self.api_key).parse().unwrap(),
            );
        }
        request_headers.insert(
            "HTTP-Referer",
            "https://github.com/arula-cli/arula-cli".parse().unwrap(),
        );
        request_headers.insert("X-Title", "ARULA CLI".parse().unwrap());
        let body_str = serde_json::to_string_pretty(&request_body).unwrap_or_default();
        log_http_request("POST", &request_url, &request_headers, Some(&body_str));

        let response = request_builder.send().await?;

        // Log the incoming response
        log_http_response(&response);

        if response.status().is_success() {
            let response_json: serde_json::Value = response.json().await?;

            if let Some(choices) = response_json["choices"].as_array() {
                if let Some(choice) = choices.first() {
                    let content = choice["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    // Handle tool calls
                    let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| {
                        calls
                            .iter()
                            .map(|tool_call| ToolCall {
                                id: tool_call["id"].as_str().unwrap_or_default().to_string(),
                                r#type: "function".to_string(),
                                function: ToolCallFunction {
                                    name: tool_call["function"]["name"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                    arguments: tool_call["function"]["arguments"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                },
                            })
                            .collect::<Vec<_>>()
                    });

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage: None, // TODO: Parse usage from response if needed
                        tool_calls,
                        model: Some(self.model.clone()),
                        created: Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        reasoning_content: None,
                    })
                } else {
                    Ok(ApiResponse {
                        response: "No response received".to_string(),
                        success: false,
                        error: Some("No choices in response".to_string()),
                        usage: None,
                        tool_calls: None,
                        model: Some(self.model.clone()),
                        created: Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        reasoning_content: None,
                    })
                }
            } else {
                Ok(ApiResponse {
                    response: "No response received".to_string(),
                    success: false,
                    error: Some("No choices in response".to_string()),
                    usage: None,
                    tool_calls: None,
                    model: Some(self.model.clone()),
                    created: Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    ),
                    reasoning_content: None,
                })
            }
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!(
                "OpenRouter API request failed: {}",
                error_text
            ))
        }
    }

    async fn send_custom_request(&self, messages: Vec<ChatMessage>) -> Result<ApiResponse> {
        // Check if this is a Z.AI endpoint by URL pattern
        let is_zai_endpoint = self.endpoint.contains("api.z.ai");

        if is_zai_endpoint {
            // Use Z.AI-specific format for custom provider with Z.AI endpoint
            self.send_zai_formatted_request(messages).await
        } else {
            // Generic custom provider format
            let request_body = serde_json::json!({
                "model": self.model,
                "messages": messages,
                "temperature": 0.7,
                "max_tokens": 2048
            });

            let mut request_builder = self
                .client
                .post(format!("{}/api/chat", self.endpoint))
                .json(&request_body);

            // Add authorization header if API key is provided
            if !self.api_key.is_empty() {
                request_builder =
                    request_builder.header("Authorization", format!("Bearer {}", self.api_key));
            }

            let response = request_builder.send().await?;

            if response.status().is_success() {
                let api_response: ApiResponse = response.json().await?;
                Ok(api_response)
            } else {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(anyhow::anyhow!("Custom API request failed: {}", error_text))
            }
        }
    }

    async fn send_zai_formatted_request(&self, messages: Vec<ChatMessage>) -> Result<ApiResponse> {
        debug_print(&format!(
            "DEBUG: Z.AI Formatted Request - API key empty? {}, length: {}",
            self.api_key.is_empty(),
            self.api_key.len()
        ));
        // Convert ChatMessage format to plain objects for Z.AI
        // Filter out tool-related messages to avoid error 1210
        let zai_messages: Vec<Value> = messages
            .into_iter()
            .filter(|msg| {
                // Skip tool role messages
                if msg.role == "tool" {
                    return false;
                }
                // Skip assistant messages that only have tool_calls (no content)
                if msg.role == "assistant" && msg.content.is_none() && msg.tool_calls.is_some() {
                    return false;
                }
                true
            })
            .map(|msg| {
                // Build simple message with only role and content
                json!({
                    "role": msg.role,
                    "content": msg.content.unwrap_or_default()
                })
            })
            .collect();

        // Determine the final request URL first (needed for conditional payload)
        let final_endpoint = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama
            AIProvider::ZAiCoding => self.endpoint.clone(), // Z.AI uses the endpoint directly
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible
        };

        // Build request payload – minimal for the Coding-Plan endpoint, full (with tools) for all other endpoints
        let request = if final_endpoint.contains("coding/paas/v4") {
            // Ultra-minimal test payload for debugging
            json!({
                "messages": [
                    {
                        "role": "user",
                        "content": "hi"
                    }
                ]
            })
        } else {
            // Full payload for generic OpenAI-compatible endpoints
            let mut req = json!({
                "model": self.model,
                "messages": zai_messages,
                "temperature": 0.7f32,
                "max_tokens": 2048,
                "stream": false
            });

            // Define bash tool (only for non-coding endpoints)
            req["tools"] = json!([
                {
                    "type": "function",
                    "function": {
                        "name": "execute_bash",
                        "description": "Execute bash shell commands. Use this when you need to run shell commands, check files, navigate directories, install packages, etc.",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "command": {
                                    "type": "string",
                                    "description": "The bash command to execute"
                                }
                            },
                            "required": ["command"]
                        }
                    }
                }
            ]);
            req["tool_choice"] = json!("auto");
            req
        };

        // Debug logging
        debug_print(&format!("DEBUG: Final endpoint URL: {}", final_endpoint));
        debug_print(&format!(
            "DEBUG: Request payload: {}",
            serde_json::to_string_pretty(&request)
                .unwrap_or_else(|_| "Failed to serialize".to_string())
        ));

        // Send the request
        let mut request_builder = self.client.post(final_endpoint).json(&request);

        // Add authorization and language headers
        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }
        // Add Accept-Language header to encourage English responses from Chinese models
        request_builder = request_builder.header("Accept-Language", "en-US,en");

        let response = request_builder.send().await?;
        let status = response.status();

        if status.is_success() {
            let response_json: serde_json::Value = response.json().await?;

            if let Some(choices) = response_json["choices"].as_array() {
                if let Some(choice) = choices.first() {
                    let content = choice["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    // Handle tool calls
                    let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| {
                        calls
                            .iter()
                            .map(|tool_call| ToolCall {
                                id: tool_call["id"].as_str().unwrap_or_default().to_string(),
                                r#type: "function".to_string(),
                                function: ToolCallFunction {
                                    name: tool_call["function"]["name"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                    arguments: tool_call["function"]["arguments"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                },
                            })
                            .collect::<Vec<_>>()
                    });

                    let usage = response_json.get("usage").map(|usage_info| Usage {
                        prompt_tokens: usage_info["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                        completion_tokens: usage_info["completion_tokens"].as_u64().unwrap_or(0)
                            as u32,
                        total_tokens: usage_info["total_tokens"].as_u64().unwrap_or(0) as u32,
                    });

                    return Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage,
                        tool_calls,
                        model: Some(self.model.clone()),
                        created: Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        reasoning_content: None,
                    });
                }
            }

            Err(anyhow::anyhow!("Invalid response format from Z.AI API"))
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("Z.AI API request failed: {}", error_text))
        }
    }

    // Fallback for non-streaming providers
    async fn fallback_non_streaming(messages: Vec<ChatMessage>) -> Result<ApiResponse> {
        // This is a simple fallback - in a real implementation, you'd want to reuse existing non-streaming logic
        let _system_content = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone().unwrap_or_default())
            .unwrap_or_else(|| "You are ARULA, an AI assistant.".to_string());

        let user_content = messages
            .iter()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone().unwrap_or_default())
            .unwrap_or_else(|| "Hello".to_string());

        // For now, return a simple response
        Ok(ApiResponse {
            response: format!("Fallback response to: {}", user_content),
            success: true,
            error: None,
            usage: None,
            tool_calls: None,
            model: Some("fallback".to_string()),
            created: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
            reasoning_content: None,
        })
    }

    #[allow(dead_code)]
    pub async fn test_connection(&self) -> Result<bool> {
        let test_message = "Hello! This is a connection test. Please respond briefly.";
        match self.send_message(test_message, None).await {
            Ok(response) => Ok(response.success),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> ApiClient {
        ApiClient::new(
            "openai".to_string(),
            "http://localhost:8080".to_string(),
            "test-key".to_string(),
            "test-model".to_string(),
        )
    }

    fn create_test_chat_message(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    fn create_test_tool_call() -> ToolCall {
        ToolCall {
            id: "call_1".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "bash_tool".to_string(),
                arguments: "{\"command\": \"echo hello\"}".to_string(),
            },
        }
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
    fn test_chat_message_serialization() {
        let message = create_test_chat_message("user", "Hello, world!");

        // Test serialization
        let json_str = serde_json::to_string(&message).unwrap();
        assert!(json_str.contains("user"));
        assert!(json_str.contains("Hello, world!"));

        // Test deserialization
        let deserialized: ChatMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.role, "user");
        assert_eq!(deserialized.content, Some("Hello, world!".to_string()));
        assert!(deserialized.tool_calls.is_none());
        assert!(deserialized.tool_call_id.is_none());
    }

    #[test]
    fn test_chat_message_with_tool_calls() {
        let tool_call = create_test_tool_call();
        let message = ChatMessage {
            role: "assistant".to_string(),
            content: Some("I'll run a command".to_string()),
            tool_calls: Some(vec![tool_call.clone()]),
            tool_call_id: None,
            tool_name: None,
        };

        // Test serialization
        let json_str = serde_json::to_string(&message).unwrap();
        assert!(json_str.contains("assistant"));
        assert!(json_str.contains("bash_tool"));
        assert!(json_str.contains("echo hello"));

        // Test deserialization
        let deserialized: ChatMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.role, "assistant");
        let tool_calls = deserialized.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].function.name, "bash_tool");
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = create_test_tool_call();

        let json_str = serde_json::to_string(&tool_call).unwrap();
        assert!(json_str.contains("call_1"));
        assert!(json_str.contains("function"));
        assert!(json_str.contains("bash_tool"));

        let deserialized: ToolCall = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.id, "call_1");
        assert_eq!(deserialized.r#type, "function");
        assert_eq!(deserialized.function.name, "bash_tool");
        assert_eq!(
            deserialized.function.arguments,
            "{\"command\": \"echo hello\"}"
        );
    }

    #[test]
    fn test_usage_serialization() {
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };

        let json_str = serde_json::to_string(&usage).unwrap();
        assert!(json_str.contains("10"));
        assert!(json_str.contains("20"));
        assert!(json_str.contains("30"));

        let deserialized: Usage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.prompt_tokens, 10);
        assert_eq!(deserialized.completion_tokens, 20);
        assert_eq!(deserialized.total_tokens, 30);
    }

    #[test]
    fn test_api_response_serialization() {
        let usage = Usage {
            prompt_tokens: 15,
            completion_tokens: 25,
            total_tokens: 40,
        };

        let response = ApiResponse {
            response: "Hello, world!".to_string(),
            success: true,
            error: None,
            usage: Some(usage.clone()),
            tool_calls: None,
            model: Some("test-model".to_string()),
            created: Some(1234567890),
            reasoning_content: None,
        };

        let json_str = serde_json::to_string(&response).unwrap();
        assert!(json_str.contains("Hello, world!"));
        assert!(json_str.contains("true"));

        let deserialized: ApiResponse = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.response, "Hello, world!");
        assert!(deserialized.success);
        assert!(deserialized.error.is_none());
        let deserialized_usage = deserialized.usage.unwrap();
        assert_eq!(deserialized_usage.total_tokens, 40);
    }

    #[test]
    fn test_api_response_with_error() {
        let response = ApiResponse {
            response: "Error occurred".to_string(),
            success: false,
            error: Some("Network error".to_string()),
            usage: None,
            tool_calls: None,
            model: None,
            created: None,
            reasoning_content: None,
        };

        let json_str = serde_json::to_string(&response).unwrap();
        let deserialized: ApiResponse = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.response, "Error occurred");
        assert!(!deserialized.success);
        assert_eq!(deserialized.error, Some("Network error".to_string()));
        assert!(deserialized.usage.is_none());
    }

    #[test]
    fn test_chat_message_with_tool_call_id() {
        let message = ChatMessage {
            role: "tool".to_string(),
            content: Some("Command executed successfully".to_string()),
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
        };

        let json_str = serde_json::to_string(&message).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.role, "tool");
        assert_eq!(deserialized.tool_call_id, Some("call_1".to_string()));
        assert!(deserialized.tool_calls.is_none());
    }

    #[test]
    fn test_streaming_response_variants() {
        // Test that we can create StreamingResponse variants
        let chunk = StreamingResponse::Chunk("Hello".to_string());
        let start = StreamingResponse::Start;

        // Test debug formatting
        assert!(format!("{:?}", chunk).contains("Chunk"));
        assert!(format!("{:?}", start).contains("Start"));

        // End variant needs an ApiResponse, so just test creation
        let api_response = ApiResponse {
            response: "Done".to_string(),
            success: true,
            error: None,
            usage: None,
            tool_calls: None,
            model: None,
            created: None,
            reasoning_content: None,
        };
        let _end = StreamingResponse::End(api_response);
    }

    #[test]
    fn test_api_client_creation() {
        let client = create_test_client();
        assert_eq!(client.model, "test-model");
        assert_eq!(client.provider, AIProvider::OpenAI);
    }

    #[test]
    fn test_ai_provider_enum() {
        // Test all AIProvider variants can be created and compared
        let openai = AIProvider::OpenAI;
        let claude = AIProvider::Claude;
        let _ollama = AIProvider::Ollama;
        let _zai = AIProvider::ZAiCoding;
        let _custom = AIProvider::Custom;

        assert_eq!(openai, AIProvider::OpenAI);
        assert_ne!(openai, claude);

        // Test debug formatting
        let debug_str = format!("{:?}", openai);
        assert!(debug_str.contains("OpenAI"));
    }

    #[test]
    fn test_edge_cases() {
        // Test empty chat message
        let empty_message = ChatMessage {
            role: "".to_string(),
            content: None,
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        };

        let json_str = serde_json::to_string(&empty_message).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json_str).unwrap();
        assert!(deserialized.role.is_empty());
        assert!(deserialized.content.is_none());

        // Test message with only tool calls
        let tool_only_message = ChatMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![create_test_tool_call()]),
            tool_call_id: None,
            tool_name: None,
        };

        let json_str = serde_json::to_string(&tool_only_message).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json_str).unwrap();
        assert!(deserialized.content.is_none());
        assert!(deserialized.tool_calls.is_some());
    }

    #[tokio::test]
    async fn test_async_operations() {
        let client = create_test_client();
        // Test that async operations work (placeholder test)
        assert_eq!(client.model, "test-model");
    }

    #[test]
    fn test_struct_debug_formats() {
        let message = create_test_chat_message("user", "Hello");
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("ChatMessage"));
        assert!(debug_str.contains("user"));

        let tool_call = create_test_tool_call();
        let debug_str = format!("{:?}", tool_call);
        assert!(debug_str.contains("ToolCall"));
        assert!(debug_str.contains("call_1"));

        let usage = Usage {
            prompt_tokens: 5,
            completion_tokens: 10,
            total_tokens: 15,
        };
        let debug_str = format!("{:?}", usage);
        assert!(debug_str.contains("Usage"));
        assert!(debug_str.contains("15"));
    }

    #[test]
    fn test_json_parsing_edge_cases() {
        // Test with special characters in content
        let special_message = ChatMessage {
            role: "user".to_string(),
            content: Some("Special chars: \"quotes\" and \n newlines \t tabs".to_string()),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        };

        let json_str = serde_json::to_string(&special_message).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json_str).unwrap();
        assert!(deserialized.content.unwrap().contains("quotes"));

        // Test with Unicode characters
        let unicode_message = ChatMessage {
            role: "user".to_string(),
            content: Some("Unicode: 🚀🎉中文字符".to_string()),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        };

        let json_str = serde_json::to_string(&unicode_message).unwrap();
        let deserialized: ChatMessage = serde_json::from_str(&json_str).unwrap();
        assert!(deserialized.content.unwrap().contains("🚀"));
    }
}
