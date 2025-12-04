use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

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
            401 => Self::AuthenticationError { message: message.to_string() },
            429 => Self::RateLimitError { message: message.to_string() },
            500..=599 => Self::InternalError { message: message.to_string() },
            _ => Self::StatusError { message: message.to_string(), status_code: status },
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
fn log_http_request(method: &str, url: &str, headers: &reqwest::header::HeaderMap, body: Option<&str>) {
    let mut log_msg = format!("=== HTTP REQUEST ===\n{} {}\n", method, url);

    // Log headers
    log_msg.push_str("HEADERS:\n");
    for (name, value) in headers {
        log_msg.push_str(&format!("  {}: {}\n", name, value.to_str().unwrap_or("<binary>")));
    }

    // Log body if present
    if let Some(body_content) = body {
        log_msg.push_str(&format!("BODY ({} bytes):\n{}\n", body_content.len(), body_content));
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
        log_msg.push_str(&format!("  {}: {}\n", name, value.to_str().unwrap_or("<binary>")));
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        eprintln!("🔧 Z.AI Usage [{}]: {} prompt + {} completion = {} total tokens",
                 model, self.prompt_tokens, self.completion_tokens, self.total_tokens);
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
    endpoint: String,
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
        let normalized_endpoint = endpoint
            .trim_end_matches('/')
            .trim_end_matches("/api/chat")
            .trim_end_matches("/api/generate")
            .trim_end_matches("/v1/chat/completions")
            .trim_end_matches("/v1")
            .trim_end_matches("/chat/completions")
            .to_string();

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

        match self.provider {
            AIProvider::OpenAI => self.send_openai_request(messages).await,
            AIProvider::Claude => self.send_claude_request(messages).await,
            AIProvider::Ollama => self.send_ollama_request(messages).await,
            AIProvider::ZAiCoding => self.send_zai_request(messages).await,
            AIProvider::OpenRouter => self.send_openrouter_request(messages).await,
            AIProvider::Custom => self.send_custom_request(messages).await,
        }
    }

    pub async fn send_message_stream(
        &self,
        message: &str,
        conversation_history: Option<Vec<ChatMessage>>,
    ) -> Result<mpsc::UnboundedReceiver<StreamingResponse>> {
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

        let (tx, rx) = mpsc::unbounded_channel();

        match self.provider {
            AIProvider::OpenAI => {
                debug_print("DEBUG: Using OpenAI provider in send_message_stream");
                // Use regular OpenAI request for now to support tool calls
                let client = self.clone();
                tokio::spawn(async move {
                    match client.send_openai_request(messages).await {
                        Ok(response) => {
                            debug_print(&format!(
                                "DEBUG: OpenAI response with tool_calls: {:?}",
                                response.tool_calls.is_some()
                            ));
                            let _ = tx.send(StreamingResponse::Start);
                            let _ = tx.send(StreamingResponse::Chunk(response.response.clone()));
                            let _ = tx.send(StreamingResponse::End(response));
                        }
                        Err(e) => {
                            debug_print(&format!("DEBUG: OpenAI request error: {}", e));
                            let _ = tx.send(StreamingResponse::Error(format!(
                                "OpenAI request error: {}",
                                e
                            )));
                        }
                    }
                });
            }
            AIProvider::OpenRouter => {
                debug_print("DEBUG: Using OpenRouter provider in send_message_stream");
                // Use OpenAI-compatible format for OpenRouter
                let client = self.clone();
                tokio::spawn(async move {
                    match client.send_openai_request(messages).await {
                        Ok(response) => {
                            debug_print(&format!(
                                "DEBUG: OpenRouter response with tool_calls: {:?}",
                                response.tool_calls.is_some()
                            ));
                            let _ = tx.send(StreamingResponse::Start);
                            let _ = tx.send(StreamingResponse::Chunk(response.response.clone()));
                            let _ = tx.send(StreamingResponse::End(response));
                        }
                        Err(e) => {
                            debug_print(&format!("DEBUG: OpenRouter request error: {}", e));
                            let _ = tx.send(StreamingResponse::Error(format!(
                                "OpenRouter request error: {}",
                                e
                            )));
                        }
                    }
                });
            }
            _ => {
                // Fallback to non-streaming for other providers
                let client = self.clone();
                tokio::spawn(async move {
                    // Use the provider-specific methods directly with the complete message array
                    let result = match client.provider {
                        AIProvider::Claude => client.send_claude_request(messages).await,
                        AIProvider::Ollama => client.send_ollama_request(messages).await,
                        AIProvider::ZAiCoding => client.send_zai_request(messages).await,
                        AIProvider::Custom => client.send_custom_request(messages).await,
                        AIProvider::OpenRouter => client.send_openai_request(messages).await, // OpenRouter uses OpenAI-compatible format
                        _ => Err(anyhow::anyhow!("Unsupported provider")),
                    };

                    match result {
                        Ok(response) => {
                            let _ = tx.send(StreamingResponse::Start);

                            // Check if this response contains tool calls
                            if let Some(_tool_calls) = &response.tool_calls {
                                // Return tool calls for the app layer to handle
                                // Don't execute here - let the app manage the conversation flow
                                let _ = tx.send(StreamingResponse::Chunk(
                                    "Let me help you with that...".to_string(),
                                ));
                                let _ = tx.send(StreamingResponse::End(response));
                            } else {
                                // Regular text response
                                let _ =
                                    tx.send(StreamingResponse::Chunk(response.response.clone()));
                                let _ = tx.send(StreamingResponse::End(response));
                            }
                        }
                        Err(e) => {
                            let _ =
                                tx.send(StreamingResponse::Error(format!("Request failed: {}", e)));
                        }
                    }
                });
            }
        }

        Ok(rx)
    }

    pub async fn continue_conversation_with_tool_results(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<mpsc::UnboundedReceiver<StreamingResponse>> {
        let (tx, rx) = mpsc::unbounded_channel();

        match self.provider {
            AIProvider::OpenAI => {
                debug_print("DEBUG: Using OpenAI provider in send_message_stream");
                // Use regular OpenAI request for now to support tool calls
                let client = self.clone();
                tokio::spawn(async move {
                    match client.send_openai_request(messages).await {
                        Ok(response) => {
                            debug_print(&format!(
                                "DEBUG: OpenAI response with tool_calls: {:?}",
                                response.tool_calls.is_some()
                            ));
                            let _ = tx.send(StreamingResponse::Start);
                            let _ = tx.send(StreamingResponse::Chunk(response.response.clone()));
                            let _ = tx.send(StreamingResponse::End(response));
                        }
                        Err(e) => {
                            debug_print(&format!("DEBUG: OpenAI request error: {}", e));
                            let _ = tx.send(StreamingResponse::Error(format!(
                                "OpenAI request error: {}",
                                e
                            )));
                        }
                    }
                });
            }
            AIProvider::OpenRouter => {
                debug_print("DEBUG: Using OpenRouter provider in send_message_stream");
                // Use OpenAI-compatible format for OpenRouter
                let client = self.clone();
                tokio::spawn(async move {
                    match client.send_openai_request(messages).await {
                        Ok(response) => {
                            debug_print(&format!(
                                "DEBUG: OpenRouter response with tool_calls: {:?}",
                                response.tool_calls.is_some()
                            ));
                            let _ = tx.send(StreamingResponse::Start);
                            let _ = tx.send(StreamingResponse::Chunk(response.response.clone()));
                            let _ = tx.send(StreamingResponse::End(response));
                        }
                        Err(e) => {
                            debug_print(&format!("DEBUG: OpenRouter request error: {}", e));
                            let _ = tx.send(StreamingResponse::Error(format!(
                                "OpenRouter request error: {}",
                                e
                            )));
                        }
                    }
                });
            }
            _ => {
                // Fallback to non-streaming for other providers
                let client = self.clone();
                tokio::spawn(async move {
                    let result = match client.provider {
                        AIProvider::Claude => client.send_claude_request(messages).await,
                        AIProvider::Ollama => client.send_ollama_request(messages).await,
                        AIProvider::ZAiCoding => client.send_zai_request(messages).await,
                        AIProvider::OpenRouter => client.send_openai_request(messages).await, // OpenRouter uses OpenAI-compatible format
                        AIProvider::Custom => client.send_custom_request(messages).await,
                        _ => Err(anyhow::anyhow!("Unsupported provider")),
                    };

                    match result {
                        Ok(response) => {
                            let _ = tx.send(StreamingResponse::Start);
                            let _ = tx.send(StreamingResponse::Chunk(response.response.clone()));
                            let _ = tx.send(StreamingResponse::End(response));
                        }
                        Err(e) => {
                            let _ =
                                tx.send(StreamingResponse::Error(format!("Request failed: {}", e)));
                        }
                    }
                });
            }
        }

        Ok(rx)
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
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };
        let mut request_builder = self
            .client
            .post(&request_url)
            .json(&request_body);

        // Add authorization header if API key is provided
        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        // Log the outgoing request
        let mut request_headers = reqwest::header::HeaderMap::new();
        if !self.api_key.is_empty() {
            request_headers.insert("Authorization", format!("Bearer {}", self.api_key).parse().unwrap());
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
                    let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| calls
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
                                .collect::<Vec<_>>());
                    
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
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                    created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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

        // Use the newer /api/chat endpoint which is OpenAI-compatible
        let request_url = format!("{}/api/chat", self.endpoint);
        let request_builder = self
            .client
            .post(&request_url)
            .json(&request);

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
                .or_else(|| ollama_response["message"]["thinking"].as_str().map(|s| s.to_string()));

            if let Some(message) = ollama_response["message"].as_object() {
                if let Some(response_text) = message["content"].as_str() {
                    Ok(ApiResponse {
                        response: response_text.to_string(),
                        success: true,
                        error: None,
                        usage: None,
                        tool_calls: None,
                        model: Some(self.model.clone()),
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                    created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
            "GLM-4.5" | "GLM-4.5-AIR" | "GLM-4.5-X" | "GLM-4.5-AIRX" | "GLM-4.5-FLASH" | "GLM-4.5V" => 65536, // Official default for GLM-4.5 series
            "GLM-4-32B-0414-128K" => 16384, // Official default for older model
            _ => 2048, // Fallback for other models
        };
        
        // Log the model being used for debugging
        debug_print(&format!("Using model: {} with max_tokens: {}", self.model, max_tokens));
        
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
        if thinking_enabled && (self.model.starts_with("GLM-4.5") || self.model.starts_with("GLM-4.6")) {
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
                _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
            };
            
            // Store a reference to the endpoint for logging
            let endpoint_str = endpoint.as_str();
            
            let mut request_builder = self
                .client
                .post(&endpoint)  // Borrow endpoint here
                .timeout(timeout)
                .json(&request);

            // Add Z.AI recommended headers
            request_builder = request_builder
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Accept-Language", "en-US,en");

            // Log the outgoing request for this attempt
            let mut request_headers = reqwest::header::HeaderMap::new();
            request_headers.insert("Authorization", format!("Bearer {}", self.api_key).parse().unwrap());
            request_headers.insert("Accept-Language", "en-US,en".parse().unwrap());
            
            // Add Content-Type header explicitly
            request_headers.insert("Content-Type", "application/json".parse().unwrap());
            
            let body_str = serde_json::to_string_pretty(&request).unwrap_or_default();
            
            // Log the full request for debugging
            debug_print(&format!("Sending request to {}: {}", endpoint_str, body_str));
            
            // Use provider-specific endpoint for logging
            let log_url = match self.provider {
                AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
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
                                let prompt_tokens = usage.get("prompt_tokens")
                                    .and_then(|v| v.as_u64()).unwrap_or(0);
                                let completion_tokens = usage.get("completion_tokens")
                                    .and_then(|v| v.as_u64()).unwrap_or(0);
                                let total_tokens = usage.get("total_tokens")
                                    .and_then(|v| v.as_u64()).unwrap_or(0);

                                ZAIUsage {
                                    prompt_tokens,
                                    completion_tokens,
                                    total_tokens,
                                    cost_estimate: self.calculate_zai_cost(&self.model, total_tokens),
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
                                let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| calls
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
                                            .collect::<Vec<_>>());

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
                                    reasoning_content: response_json["choices"][0]["message"]["reasoning_content"].as_str().map(|s| s.to_string()),
                                });
                            }
                        }

                        return Err(anyhow!("No choices in Z.AI response"));
                    } else {
                        // Handle HTTP errors with Z.AI-specific mapping
                        let error_body = resp.text().await.unwrap_or_default();
                        let api_error = ZAIApiError::from_status_code(
                            status.as_u16(),
                            &error_body
                        );
                        
                        // Log detailed error information
                        debug_print(&format!("Z.AI API error ({}): {}", status, error_body));

                        // Don't retry on client errors (4xx)
                        if status.is_client_error() {
                            return Err(anyhow!("Z.AI API error ({}): {}", status, api_error));
                        }

                        // Log retry attempt
                        if attempt < max_retries {
                            eprintln!("🔄 Z.AI request failed (attempt {}/{}), retrying...: {}",
                                     attempt + 1, max_retries + 1, api_error);
                            tokio::time::sleep(Duration::from_millis((1000 * (attempt + 1)) as u64)).await;
                            continue;
                        } else {
                            return Err(anyhow!("Z.AI API request failed after {} retries: {}", max_retries, api_error));
                        }
                    }
                }
                Err(e) => {
                    // Handle network errors
                    if attempt < max_retries {
                        eprintln!("🔄 Z.AI network error (attempt {}/{}) retrying...: {}",
                                 attempt + 1, max_retries + 1, e);
                        tokio::time::sleep(Duration::from_millis((1000 * (attempt + 1)) as u64)).await;
                        continue;
                    } else {
                        return Err(anyhow!("Z.AI network error after {} retries: {}", max_retries, e));
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
            "GLM-4" | "GLM-4.6" => 0.0025, // $2.50 per 1M tokens
            "GLM-4.5" => 0.0015, // $1.50 per 1M tokens
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
        let mut request_builder = self
            .client
            .post(&request_url)
            .json(&request_body);

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
            request_headers.insert("Authorization", format!("Bearer {}", self.api_key).parse().unwrap());
        }
        request_headers.insert("HTTP-Referer", "https://github.com/arula-cli/arula-cli".parse().unwrap());
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
                    let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| calls
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
                                .collect::<Vec<_>>());

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage: None, // TODO: Parse usage from response if needed
                        tool_calls,
                        model: Some(self.model.clone()),
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                    created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
                    reasoning_content: None,
                })
            }
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("OpenRouter API request failed: {}", error_text))
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

        // Z.AI uses OpenAI-compatible format with specific endpoint
        let mut request = json!({
            "model": self.model,
            "messages": zai_messages,
            "temperature": 0.7f32,  // Use f32 to ensure consistent precision
            "max_tokens": 2048,
            "stream": false
        });

        // Define bash tool
        request["tools"] = json!([
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
        request["tool_choice"] = json!("auto");  // "required" is not supported by Z.AI

        // Use provider-specific endpoint
        let endpoint = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };
        let mut request_builder = self
            .client
            .post(endpoint)
            .json(&request);

        // Add authorization header if API key is provided
        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

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
                    let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| calls
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
                                .collect::<Vec<_>>());

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
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
            created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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

    /// Send message with true SSE streaming (OpenAI-compatible)
    ///
    /// This method enables real Server-Sent Events streaming for:
    /// - Real-time text output as it's generated
    /// - Proper tool call delta accumulation
    /// - Usage statistics via stream_options
    ///
    /// # Arguments
    ///
    /// * `messages` - Conversation messages
    /// * `tools` - Available tool definitions
    /// * `callback` - Function called for each stream event
    ///
    /// # Returns
    ///
    /// The final accumulated response with all content and tool calls
    pub async fn send_message_streaming<F>(
        &self,
        messages: &[ChatMessage],
        tools: &[serde_json::Value],
        callback: F,
    ) -> Result<ApiResponse>
    where
        F: FnMut(crate::api::streaming::StreamEvent) + Send,
    {
        use crate::api::streaming::{build_streaming_request_full, process_stream};

        // Check if this is Z.AI - both by provider type AND by endpoint URL
        // This ensures proper handling even if provider detection failed
        let is_zai = matches!(self.provider, AIProvider::ZAiCoding) 
            || self.endpoint.contains("api.z.ai");
        let is_ollama = matches!(self.provider, AIProvider::Ollama);

        // Convert ChatMessage to JSON format
        // Z.AI has strict requirements - only include role and content
        let json_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter_map(|msg| {
                // For Z.AI streaming, skip tool-related messages entirely
                if is_zai {
                    // Skip tool role messages for Z.AI streaming
                    if msg.role == "tool" {
                        return None;
                    }
                    // For assistant messages with only tool_calls (no content), skip
                    if msg.role == "assistant" && msg.content.is_none() && msg.tool_calls.is_some() {
                        return None;
                    }
                }
                
                let mut obj = serde_json::json!({
                    "role": msg.role,
                });
                
                if let Some(content) = &msg.content {
                    obj["content"] = serde_json::json!(content);
                } else if is_zai {
                    // Z.AI requires content, use empty string if none
                    obj["content"] = serde_json::json!("");
                } else if msg.tool_calls.is_some() {
                    // Assistant message with tool_calls can have null content (non-Z.AI)
                    obj["content"] = serde_json::json!(null);
                }
                
                // Only include tool-related fields for non-Z.AI providers
                if !is_zai {
                    if let Some(tool_calls) = &msg.tool_calls {
                        // For Ollama, convert arguments from string to object
                        if is_ollama {
                            let converted_calls: Vec<serde_json::Value> = tool_calls.iter().map(|tc| {
                                // Parse arguments string to JSON object
                                let args_obj = serde_json::from_str::<serde_json::Value>(&tc.function.arguments)
                                    .unwrap_or_else(|_| serde_json::json!({}));
                                serde_json::json!({
                                    "function": {
                                        "name": tc.function.name,
                                        "arguments": args_obj  // Object, not string
                                    }
                                })
                            }).collect();
                            obj["tool_calls"] = serde_json::json!(converted_calls);
                        } else {
                            obj["tool_calls"] = serde_json::json!(tool_calls);
                        }
                    }
                    
                    if is_ollama {
                        // Ollama uses tool_name instead of tool_call_id for tool responses
                        if let Some(tool_name) = &msg.tool_name {
                            obj["tool_name"] = serde_json::json!(tool_name);
                        }
                    } else {
                        // OpenAI-compatible APIs use tool_call_id
                        if let Some(tool_call_id) = &msg.tool_call_id {
                            obj["tool_call_id"] = serde_json::json!(tool_call_id);
                        }
                    }
                }
                
                Some(obj)
            })
            .collect();

        // Build streaming request
        // Z.AI has specific requirements (from docs.z.ai):
        // - Does NOT support stream_options parameter (returns error 1210)
        // - Does NOT support tool_choice with streaming
        // Ollama also doesn't support stream_options or tool_choice
        let include_stream_options = !is_zai && !is_ollama;
        let include_tool_choice = !is_zai && !is_ollama;
        
        // For Z.AI, we need special handling for tools with streaming
        // Based on Z.AI docs: all streaming + tool examples only use primitive types (string, number, boolean)
        // Complex types (object, array) in tool parameters may cause error 1210
        // 
        // For Z.AI, filter out tools with complex parameter types to avoid error 1210
        let tools_ref = if is_zai {
            // Filter to only tools with simple parameter types
            let simple_tools: Vec<&serde_json::Value> = tools.iter()
                .filter(|tool| {
                    if let Some(params) = tool.get("function")
                        .and_then(|f| f.get("parameters"))
                        .and_then(|p| p.get("properties"))
                        .and_then(|props| props.as_object()) 
                    {
                        // Check all parameters - reject if any has object/array type
                        for (param_name, param) in params {
                            if let Some(param_type) = param.get("type").and_then(|t| t.as_str()) {
                                if param_type == "object" || param_type == "array" {
                                    debug_print(&format!(
                                        "DEBUG: Z.AI - filtering out tool '{}' due to param '{}' with type '{}'",
                                        tool.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("unknown"),
                                        param_name,
                                        param_type
                                    ));
                                    return false;
                                }
                            }
                        }
                    }
                    true
                })
                .collect();
            
            debug_print(&format!("DEBUG: Z.AI - {} of {} tools have simple params", simple_tools.len(), tools.len()));
            
            // For Z.AI, we don't include tools in streaming requests to avoid error 1210
            // Tool calls will be handled via non-streaming fallback
            if !simple_tools.is_empty() {
                debug_print("DEBUG: Z.AI streaming - excluding tools to avoid error 1210");
            }
            None
        } else if !tools.is_empty() {
            Some(tools)
        } else {
            None
        };
        
        // Use model-specific parameters for streaming (matching non-streaming logic)
        let max_tokens = match self.model.as_str() {
            "GLM-4.6" => 65536, // Official default for GLM-4.6
            "GLM-4.5" | "GLM-4.5-AIR" | "GLM-4.5-X" | "GLM-4.5-AIRX" | "GLM-4.5-FLASH" | "GLM-4.5V" => 65536, // Official default for GLM-4.5 series
            "GLM-4-32B-0414-128K" => 16384, // Official default for older model
            _ => 2048, // Fallback for other models
        };

        let mut request_body = build_streaming_request_full(
            &self.model,
            &json_messages,
            tools_ref,
            0.7, // Use default temperature for GLM models
            max_tokens,
            include_stream_options,
            include_tool_choice,
        );

        // Ollama-specific request formatting
        if is_ollama {
            // Convert max_tokens to options.num_predict for Ollama
            if let Some(obj) = request_body.as_object_mut() {
                let max_tokens = obj.remove("max_tokens").and_then(|v| v.as_u64()).unwrap_or(2048);
                let temperature = obj.remove("temperature").and_then(|v| v.as_f64()).unwrap_or(0.7);
                obj.insert("options".to_string(), serde_json::json!({
                    "num_predict": max_tokens,
                    "temperature": temperature
                }));
            }
        }

        // Z.AI-specific request formatting
        // Rebuild request with ONLY supported parameters to avoid error 1210
        // Z.AI docs explicitly support: model, messages, stream, temperature, top_p, max_tokens, stop, user_id
        // Note: Z.AI Coding API (api.z.ai/api/coding/...) may have different supported params
        if is_zai {
            let model = request_body.get("model").cloned().unwrap_or(serde_json::json!(self.model.clone()));
            let messages = request_body.get("messages").cloned().unwrap_or_else(|| serde_json::json!([]));
            let temperature = request_body.get("temperature").cloned().unwrap_or(serde_json::json!(0.7));
            let max_tokens = request_body.get("max_tokens").cloned().unwrap_or(serde_json::json!(2048));
            
            // For Z.AI, we need to be very specific about which fields we include
            // to avoid error 1210 (Invalid API parameter)
            request_body = serde_json::json!({
                "model": model,
                "messages": messages,
                "stream": true,
                "temperature": temperature,
                "max_tokens": max_tokens
            });
            
            // Note: We're not including tools in streaming requests for Z.AI
            // to prevent error 1210
            debug_print(&format!("DEBUG: Z.AI cleaned request body: {}", serde_json::to_string_pretty(&request_body).unwrap_or_default()));
        }

        // Use provider-specific endpoint
        let request_url = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };

        debug_print(&format!("DEBUG: Streaming request to {}", request_url));
        debug_print(&format!("DEBUG: Provider type: {:?}, is_zai: {}, is_ollama: {}", self.provider, is_zai, is_ollama));
        debug_print(&format!("DEBUG: include_stream_options: {}, include_tool_choice: {}", include_stream_options, include_tool_choice));
        debug_print(&format!("DEBUG: Streaming request body: {}", serde_json::to_string_pretty(&request_body).unwrap_or_default()));
        
        let mut request_builder = self
            .client
            .post(&request_url)
            .json(&request_body);

        // Add authorization
        if !self.api_key.is_empty() {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        // Add provider-specific headers
        match self.provider {
            AIProvider::OpenRouter => {
                request_builder = request_builder
                    .header("HTTP-Referer", "https://github.com/arula-cli/arula-cli")
                    .header("X-Title", "ARULA CLI");
            }
            AIProvider::ZAiCoding => {
                request_builder = request_builder
                    .header("Accept-Language", "en-US,en");
            }
            _ => {}
        }

        // Send request
        let response = request_builder.send().await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            // Truncate HTML error pages to show just the first meaningful part
            let error_display = if error_text.contains("<!DOCTYPE") || error_text.contains("<html") {
                format!("{} (HTML error page received - check if the endpoint URL is correct)", status)
            } else {
                error_text
            };
            return Err(anyhow::anyhow!("API request to {} failed: {}", request_url, error_display));
        }

        // Process the streaming response
        process_stream(response, callback).await
    }

    /// Send message with custom tools (used by modern agent client)
    pub async fn send_message_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[serde_json::Value],
    ) -> Result<mpsc::UnboundedReceiver<StreamingResponse>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let messages = messages.to_vec();
        let tools = tools.to_vec();

        let client = self.clone();
        tokio::spawn(async move {
            match client.provider {
                AIProvider::OpenAI | AIProvider::OpenRouter => {
                    // Use custom tool-aware OpenAI-compatible implementation
                    match client.send_openai_request_with_tools(messages, tools).await {
                        Ok(response) => {
                            debug_print(&format!("DEBUG: {:?} response with tools", client.provider));
                            let _ = tx.send(StreamingResponse::Start);
                            let _ = tx.send(StreamingResponse::Chunk(response.response.clone()));
                            let _ = tx.send(StreamingResponse::End(response));
                        }
                        Err(e) => {
                            debug_print(&format!("DEBUG: {:?} request error: {}", client.provider, e));
                            let _ = tx.send(StreamingResponse::Error(format!(
                                "Request error: {}",
                                e
                            )));
                        }
                    }
                }
                AIProvider::ZAiCoding | AIProvider::Custom => {
                    // For Z.AI, use OpenAI-compatible format with tools
                    match client.send_zai_request_with_tools(messages, tools).await {
                        Ok(response) => {
                            debug_print("DEBUG: Z.AI response with tools");
                            let _ = tx.send(StreamingResponse::Start);
                            let _ = tx.send(StreamingResponse::Chunk(response.response.clone()));
                            let _ = tx.send(StreamingResponse::End(response));
                        }
                        Err(e) => {
                            debug_print(&format!("DEBUG: Z.AI request error: {}", e));
                            let _ = tx.send(StreamingResponse::Error(format!(
                                "Z.AI request error: {}",
                                e
                            )));
                        }
                    }
                }
                _ => {
                    // Fallback for other providers
                    let result = match client.provider {
                        AIProvider::Claude => client.send_claude_request(messages).await,
                        AIProvider::Ollama => client.send_ollama_request(messages).await,
                        AIProvider::ZAiCoding => client.send_zai_request(messages).await,
                        _ => Err(anyhow::anyhow!("Unsupported provider for tools")),
                    };

                    match result {
                        Ok(response) => {
                            let _ = tx.send(StreamingResponse::Start);
                            let _ = tx.send(StreamingResponse::Chunk(response.response.clone()));
                            let _ = tx.send(StreamingResponse::End(response));
                        }
                        Err(e) => {
                            let _ =
                                tx.send(StreamingResponse::Error(format!("Request failed: {}", e)));
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Send message with custom tools (synchronous version - waits for complete response)
    /// 
    /// Unlike `send_message_with_tools`, this method directly returns the API response
    /// instead of using a channel. Used for non-streaming mode.
    pub async fn send_message_with_tools_sync(
        &self,
        messages: &[ChatMessage],
        tools: &[serde_json::Value],
    ) -> Result<ApiResponse> {
        let messages = messages.to_vec();
        let tools = tools.to_vec();

        match self.provider {
            AIProvider::OpenAI | AIProvider::OpenRouter => {
                self.send_openai_request_with_tools(messages, tools).await
            }
            AIProvider::ZAiCoding | AIProvider::Custom => {
                self.send_zai_request_with_tools(messages, tools).await
            }
            AIProvider::Claude => {
                self.send_claude_request(messages).await
            }
            AIProvider::Ollama => {
                self.send_ollama_request(messages).await
            }
        }
    }

    /// Send OpenAI request with custom tools (also used for OpenRouter)
    async fn send_openai_request_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
    ) -> Result<ApiResponse> {
        // Check if thinking/reasoning is enabled
        let config = crate::utils::config::Config::load_or_default()?;
        let thinking_enabled = config.get_thinking_enabled().unwrap_or(false);
        
        // Create request with custom tools
        let mut request_body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_tokens": 2048,
            "tools": tools,
            "tool_choice": "auto"
        });
        
        // Add reasoning effort when thinking is enabled
        // Works with GPT-5.1 and reasoning models; ignored by unsupported models
        if thinking_enabled {
            request_body["reasoning_effort"] = serde_json::json!("medium");
        }

        // Use provider-specific endpoint
        let request_url = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };
        let mut request_builder = self
            .client
            .post(&request_url)
            .json(&request_body);

        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        // Add OpenRouter-specific headers if using OpenRouter
        if self.provider == AIProvider::OpenRouter {
            request_builder = request_builder
                .header("HTTP-Referer", "https://github.com/arula-cli/arula-cli")
                .header("X-Title", "ARULA CLI");
        }

        // Log the outgoing request
        let mut request_headers = reqwest::header::HeaderMap::new();
        if !self.api_key.is_empty() {
            request_headers.insert("Authorization", format!("Bearer {}", self.api_key).parse().unwrap());
        }
        if self.provider == AIProvider::OpenRouter {
            request_headers.insert("HTTP-Referer", "https://github.com/arula-cli/arula-cli".parse().unwrap());
            request_headers.insert("X-Title", "ARULA CLI".parse().unwrap());
        }
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

                    let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| calls
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
                                .collect::<Vec<_>>());

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage: None,
                        tool_calls,
                        model: Some(self.model.clone()),
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                    created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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

    /// Send Z.AI request with custom tools (with retry logic)
    async fn send_zai_request_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
    ) -> Result<ApiResponse> {
        debug_print(&format!(
            "DEBUG: Z.AI Formatted Request with Tools - API key length: {}",
            self.api_key.len()
        ));

        // Get Z.AI configuration from the config file
        let config = crate::utils::config::Config::load_or_default()?;
        let thinking_enabled = config.get_thinking_enabled().unwrap_or(false);

        let max_retries = 3;
        let mut retry_count = 0;

        loop {
            match self
                .send_zai_request_with_tools_once(messages.clone(), tools.clone(), thinking_enabled)
                .await
            {
                Ok(response) => return Ok(response),
                Err(e) if retry_count < max_retries && self.should_retry(&e) => {
                    retry_count += 1;
                    debug_print(&format!(
                        "DEBUG: Z.AI request failed (attempt {}), retrying in {} seconds: {}",
                        retry_count,
                        2 * retry_count,
                        e
                    ));
                    tokio::time::sleep(tokio::time::Duration::from_secs(2 * retry_count)).await;
                    continue;
                }
                Err(e) => {
                    debug_print(&format!(
                        "DEBUG: Z.AI request failed permanently after {} attempts: {}",
                        retry_count, e
                    ));
                    return Err(e);
                }
            }
        }
    }

    /// Send Z.AI request with custom tools (single attempt)
    async fn send_zai_request_with_tools_once(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
        thinking_enabled: bool,
    ) -> Result<ApiResponse> {
        // Filter out tools with unsupported parameter types (object, array)
        // Based on Z.AI docs: all function calling examples only use primitive types (string, number, boolean)
        // Complex types (object, array) in tool parameters may cause error 1210
        let filtered_tools: Vec<serde_json::Value> = tools.iter()
            .filter(|tool| {
                if let Some(params) = tool.get("function")
                    .and_then(|f| f.get("parameters"))
                    .and_then(|p| p.get("properties"))
                    .and_then(|props| props.as_object()) 
                {
                    for (param_name, param) in params {
                        if let Some(param_type) = param.get("type").and_then(|t| t.as_str()) {
                            if param_type == "object" || param_type == "array" {
                                debug_print(&format!(
                                    "DEBUG: Z.AI non-streaming - filtering out tool '{}' due to param '{}' with type '{}'", 
                                    tool.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("unknown"),
                                    param_name,
                                    param_type
                                ));
                                return false;
                            }
                        }
                    }
                }
                true
            })
            .cloned()
            .collect();
        
        debug_print(&format!("DEBUG: Z.AI non-streaming - {} of {} tools have simple params", filtered_tools.len(), tools.len()));
        
        // Track if we've already added a system message
        let mut has_system_message = false;

        // For tool-enabled requests, we need to include tool_calls and tool results properly
        // But filter out problematic message patterns
        let zai_messages: Vec<serde_json::Value> = messages
            .into_iter()
            .filter_map(|msg| {
                // Handle system messages first
                if msg.role == "system" {
                    if has_system_message {
                        // Skip duplicate system messages
                        return None;
                    }
                    has_system_message = true;
                    return Some(serde_json::json!({
                        "role": "system",
                        "content": msg.content.unwrap_or_default()
                    }));
                }

                // Handle other message types
                match msg.role.as_str() {
                    "tool" => {
                        // Tool result message - Z.AI expects this format
                        Some(serde_json::json!({
                            "role": "tool",
                            "content": msg.content.unwrap_or_default(),
                            "tool_call_id": msg.tool_call_id.unwrap_or_default()
                        }))
                    }
                    "assistant" => {
                        if let Some(tool_calls) = msg.tool_calls {
                            // Assistant with tool calls
                            Some(serde_json::json!({
                                "role": "assistant",
                                "content": msg.content.unwrap_or_default(), // Ensure content is not None
                                "tool_calls": tool_calls
                            }))
                        } else {
                            // Regular assistant message
                            Some(serde_json::json!({
                                "role": "assistant",
                                "content": msg.content.unwrap_or_default()
                            }))
                        }
                    }
                    _ => {
                        // User and other message types
                        Some(serde_json::json!({
                            "role": msg.role,
                            "content": msg.content.unwrap_or_default()
                        }))
                    }
                }
            })
            .collect();

        let mut request = serde_json::json!({
            "model": self.model,
            "messages": zai_messages,
            "temperature": 0.7f32,  // Use f32 to ensure consistent precision
            "max_tokens": 2048,
            "stream": false,
            "tools": filtered_tools,
            "tool_choice": "auto"
        });

        // Add thinking mode if enabled
        if thinking_enabled {
            request["thinking"] = serde_json::json!({
                "type": "enabled"
            });
        }

        // Debug: Log the Z.AI request when debug mode is enabled
        if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
            println!("🔧 DEBUG: Z.AI Tools Request: {}", serde_json::to_string_pretty(&request).unwrap_or_else(|_| "Failed to serialize request".to_string()));
            println!("🔧 DEBUG: Thinking enabled: {}", thinking_enabled);
        }

        // Create a new client specifically for Z.AI to force HTTP/1.1 for better compatibility
        let zai_client = Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("arula-cli/1.0")
            .http1_only() // Force HTTP/1.1 for Z.AI compatibility
            .tcp_nodelay(true)
            .connection_verbose(std::env::var("ARULA_DEBUG").unwrap_or_default() == "1")
            .build()
            .expect("Failed to create Z.AI HTTP client");

        // Use provider-specific endpoint
        let endpoint = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };
        let mut request_builder = zai_client
            .post(endpoint)
            .json(&request);

        // Log the outgoing request
        let request_headers = reqwest::header::HeaderMap::new();
        let body_str = serde_json::to_string_pretty(&request).unwrap_or_default();
        // Use provider-specific endpoint for logging
        let log_url = match self.provider {
            AIProvider::Ollama => format!("{}/api/chat", self.endpoint), // Ollama uses /api/chat
            _ => format!("{}/chat/completions", self.endpoint), // OpenAI-compatible endpoints
        };
        log_http_request("POST", &log_url, &request_headers, Some(&body_str));

        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
            debug_print(&format!(
                "DEBUG: Sending Z.AI request to: {}/chat/completions",
                self.endpoint
            ));
            debug_print(&format!(
                "DEBUG: Request body size: {} bytes",
                serde_json::to_string(&request).unwrap_or_default().len()
            ));
        }

        let response = request_builder
            .timeout(std::time::Duration::from_secs(45))
            .send()
            .await?;
        let status = response.status();

        // Log the incoming response
        log_http_response(&response);

        if status.is_success() {
            let response_json: serde_json::Value = response.json().await?;

            // Debug: Log the full Z.AI response when debug mode is enabled
            if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                println!("🔧 DEBUG: Z.AI Tools Response: {}", serde_json::to_string_pretty(&response_json).unwrap_or_else(|_| "Failed to serialize response".to_string()));
            }

            if let Some(choices) = response_json["choices"].as_array() {
                if let Some(choice) = choices.first() {
                    let content = choice["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    // Extract reasoning content if available
                    let reasoning_content = choice["message"]["reasoning_content"]
                        .as_str()
                        .map(|s| s.to_string());

                    // Debug: Log reasoning content if present
                    if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
                        if let Some(ref reasoning) = reasoning_content {
                            println!("🧠 DEBUG: Z.AI Tools Reasoning Content Found: {}", reasoning);
                        } else {
                            println!("🔧 DEBUG: Z.AI Tools No Reasoning Content in response");
                        }
                    }

                    let tool_calls = choice["message"]["tool_calls"].as_array().map(|calls| calls
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
                                .collect::<Vec<_>>());

                    let usage = response_json.get("usage").map(|usage_info| Usage {
                            prompt_tokens: usage_info["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                            completion_tokens: usage_info["completion_tokens"].as_u64().unwrap_or(0)
                                as u32,
                            total_tokens: usage_info["total_tokens"].as_u64().unwrap_or(0) as u32,
                        });

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage,
                        tool_calls,
                        model: Some(self.model.clone()),
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
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
                        created: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()),
                        reasoning_content: None,
                    })
                }
            } else {
                Err(anyhow::anyhow!("Invalid response format from Z.AI API"))
            }
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("Z.AI API request failed: {}", error_text))
        }
    }

    /// Determine if an error should trigger a retry
    fn should_retry(&self, error: &anyhow::Error) -> bool {
        let error_str = error.to_string().to_lowercase();

        // Retry on network-related errors
        error_str.contains("bad gateway")
            || error_str.contains("timeout")
            || error_str.contains("connection refused")
            || error_str.contains("connection reset")
            || error_str.contains("connection aborted")
            || error_str.contains("connection timed out")
            || error_str.contains("connection failed")
            || error_str.contains("error sending request")
            || error_str.contains("dns resolution failed")
            || error_str.contains("no route to host")
            || error_str.contains("network is unreachable")
            || error_str.contains("temporary failure")
            || error_str.contains("broken pipe")
            || error_str.contains("unexpected eof")
            || error_str.contains("http error")
            || error_str.contains("hyper error")
            || error_str.contains("reqwest error")
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
        assert_eq!(deserialized.function.arguments, "{\"command\": \"echo hello\"}");
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
