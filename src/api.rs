use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;

/// Debug print helper that checks ARULA_DEBUG environment variable
fn debug_print(msg: &str) {
    if std::env::var("ARULA_DEBUG").is_ok() {
        eprintln!("{}", msg);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
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
}

#[derive(Debug, Clone)]
pub enum StreamingResponse {
    Start,
    Chunk(String),
    End(ApiResponse),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum AIProvider {
    OpenAI,
    Claude,
    Ollama,
    ZAiCoding,
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
        let provider_type = match provider.to_lowercase().as_str() {
            "openai" => AIProvider::OpenAI,
            "claude" | "anthropic" => AIProvider::Claude,
            "ollama" => AIProvider::Ollama,
            "z.ai coding plan" | "z.ai" | "zai" => AIProvider::ZAiCoding,
            _ => AIProvider::Custom,
        };

        if std::env::var("ARULA_DEBUG").is_ok() {
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
            .connection_verbose(std::env::var("ARULA_DEBUG").is_ok())
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .expect("Failed to create HTTP client");

        // Initialize OpenAI client for streaming support
        Self {
            client,
            provider: provider_type,
            endpoint,
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

        match self.provider {
            AIProvider::OpenAI => self.send_openai_request(messages).await,
            AIProvider::Claude => self.send_claude_request(messages).await,
            AIProvider::Ollama => self.send_ollama_request(messages).await,
            AIProvider::ZAiCoding => self.send_zai_request(messages).await,
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
            _ => {
                // Fallback to non-streaming for other providers
                let client = self.clone();
                tokio::spawn(async move {
                    let result = match client.provider {
                        AIProvider::Claude => client.send_claude_request(messages).await,
                        AIProvider::Ollama => client.send_ollama_request(messages).await,
                        AIProvider::ZAiCoding => client.send_zai_request(messages).await,
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
        // Create request with tools as JSON
        let request_body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_tokens": 2048,
            "tools": [
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
            ],
            "tool_choice": "auto"
        });

        let mut request_builder = self
            .client
            .post(format!("{}/chat/completions", self.endpoint))
            .json(&request_body);

        // Add authorization header if API key is provided
        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = request_builder.send().await?;

        if response.status().is_success() {
            let response_json: serde_json::Value = response.json().await?;

            if let Some(choices) = response_json["choices"].as_array() {
                if let Some(choice) = choices.first() {
                    let content = choice["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    // Handle tool calls
                    let tool_calls = if let Some(calls) = choice["message"]["tool_calls"].as_array()
                    {
                        Some(
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
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        None
                    };

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage: None, // TODO: Parse usage from response if needed
                        tool_calls,
                    })
                } else {
                    Ok(ApiResponse {
                        response: "No response received".to_string(),
                        success: false,
                        error: Some("No choices in response".to_string()),
                        usage: None,
                        tool_calls: None,
                    })
                }
            } else {
                Ok(ApiResponse {
                    response: "No response received".to_string(),
                    success: false,
                    error: Some("No choices in response".to_string()),
                    usage: None,
                    tool_calls: None,
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
        let claude_messages: Vec<Value> = messages
            .into_iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content.unwrap_or_default()
                })
            })
            .collect();

        let request = json!({
            "model": self.model,
            "messages": claude_messages,
            "max_tokens": 2048,
            "temperature": 0.7
        });

        let mut request_builder = self
            .client
            .post(format!("{}/v1/messages", self.endpoint))
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&request);

        // Add authorization header if API key is provided
        if !self.api_key.is_empty() {
            request_builder = request_builder.header("x-api-key", &self.api_key);
        }

        let response = request_builder.send().await?;

        if response.status().is_success() {
            let claude_response: Value = response.json().await?;

            if let Some(content) = claude_response["content"].as_array() {
                if let Some(text_block) = content.first() {
                    if let Some(text) = text_block["text"].as_str() {
                        return Ok(ApiResponse {
                            response: text.to_string(),
                            success: true,
                            error: None,
                            usage: None, // Claude has different usage format
                            tool_calls: None,
                        });
                    }
                }
            }

            Ok(ApiResponse {
                response: "Invalid Claude response format".to_string(),
                success: false,
                error: Some("Could not parse Claude response".to_string()),
                usage: None,
                tool_calls: None,
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
        // Convert messages to Ollama format
        let prompt = messages
            .iter()
            .map(|msg| {
                format!(
                    "{}: {}",
                    msg.role.to_uppercase(),
                    msg.content.as_ref().unwrap_or(&String::new())
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let request = json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": 0.7,
                "num_predict": 2048
            }
        });

        let response = self
            .client
            .post(format!("{}/api/generate", self.endpoint))
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let ollama_response: Value = response.json().await?;

            if let Some(response_text) = ollama_response["response"].as_str() {
                Ok(ApiResponse {
                    response: response_text.to_string(),
                    success: true,
                    error: None,
                    usage: None,
                    tool_calls: None,
                })
            } else {
                Ok(ApiResponse {
                    response: "Invalid Ollama response format".to_string(),
                    success: false,
                    error: Some("Could not parse Ollama response".to_string()),
                    usage: None,
                    tool_calls: None,
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
        // Convert ChatMessage format to plain objects for Z.AI
        let zai_messages: Vec<Value> = messages
            .into_iter()
            .map(|msg| {
                let mut msg_obj = json!({
                    "role": msg.role,
                });

                if let Some(content) = msg.content {
                    msg_obj["content"] = json!(content);
                }

                if let Some(tool_calls) = msg.tool_calls {
                    msg_obj["tool_calls"] = json!(tool_calls);
                }

                if let Some(tool_call_id) = msg.tool_call_id {
                    msg_obj["tool_call_id"] = json!(tool_call_id);
                }

                msg_obj
            })
            .collect();

        // Z.AI uses OpenAI-compatible format with specific endpoint
        let mut request = json!({
            "model": self.model,
            "messages": zai_messages,
            "temperature": 0.7,
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
        request["tool_choice"] = json!("required");

        let mut request_builder = self
            .client
            .post(format!("{}/chat/completions", self.endpoint)) // Z.AI uses this exact path
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
                    let tool_calls = if let Some(calls) = choice["message"]["tool_calls"].as_array()
                    {
                        Some(
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
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        None
                    };

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage: None,
                        tool_calls,
                    })
                } else {
                    Ok(ApiResponse {
                        response: "No response received".to_string(),
                        success: false,
                        error: Some("No choices in response".to_string()),
                        usage: None,
                        tool_calls: None,
                    })
                }
            } else {
                Ok(ApiResponse {
                    response: "No response received".to_string(),
                    success: false,
                    error: Some("No choices in response".to_string()),
                    usage: None,
                    tool_calls: None,
                })
            }
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("Z.AI API request failed: {}", error_text))
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
        let zai_messages: Vec<Value> = messages
            .into_iter()
            .map(|msg| {
                let mut msg_obj = json!({
                    "role": msg.role,
                });

                if let Some(content) = msg.content {
                    msg_obj["content"] = json!(content);
                }

                if let Some(tool_calls) = msg.tool_calls {
                    msg_obj["tool_calls"] = json!(tool_calls);
                }

                if let Some(tool_call_id) = msg.tool_call_id {
                    msg_obj["tool_call_id"] = json!(tool_call_id);
                }

                msg_obj
            })
            .collect();

        // Z.AI uses OpenAI-compatible format with specific endpoint
        let mut request = json!({
            "model": self.model,
            "messages": zai_messages,
            "temperature": 0.7,
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
        request["tool_choice"] = json!("required");

        let mut request_builder = self
            .client
            .post(format!("{}/chat/completions", self.endpoint)) // Z.AI uses this exact path
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
                    let tool_calls = if let Some(calls) = choice["message"]["tool_calls"].as_array()
                    {
                        Some(
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
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        None
                    };

                    let usage = if let Some(usage_info) = response_json.get("usage") {
                        Some(Usage {
                            prompt_tokens: usage_info["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                            completion_tokens: usage_info["completion_tokens"].as_u64().unwrap_or(0)
                                as u32,
                            total_tokens: usage_info["total_tokens"].as_u64().unwrap_or(0) as u32,
                        })
                    } else {
                        None
                    };

                    return Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage,
                        tool_calls,
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
                AIProvider::OpenAI => {
                    // Use custom tool-aware OpenAI implementation
                    match client.send_openai_request_with_tools(messages, tools).await {
                        Ok(response) => {
                            debug_print("DEBUG: OpenAI response with tools");
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
                }
                AIProvider::Custom => {
                    // For Z.AI, use the existing zai_formatted_request with tools
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

    /// Send OpenAI request with custom tools
    async fn send_openai_request_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
    ) -> Result<ApiResponse> {
        // Create request with custom tools
        let request_body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "max_tokens": 2048,
            "tools": tools,
            "tool_choice": "auto"
        });

        let mut request_builder = self
            .client
            .post(format!("{}/chat/completions", self.endpoint))
            .json(&request_body);

        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = request_builder.send().await?;

        if response.status().is_success() {
            let response_json: serde_json::Value = response.json().await?;

            if let Some(choices) = response_json["choices"].as_array() {
                if let Some(choice) = choices.first() {
                    let content = choice["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    let tool_calls = if let Some(calls) = choice["message"]["tool_calls"].as_array()
                    {
                        Some(
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
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        None
                    };

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage: None,
                        tool_calls,
                    })
                } else {
                    Ok(ApiResponse {
                        response: "No response received".to_string(),
                        success: false,
                        error: Some("No choices in response".to_string()),
                        usage: None,
                        tool_calls: None,
                    })
                }
            } else {
                Ok(ApiResponse {
                    response: "No response received".to_string(),
                    success: false,
                    error: Some("No choices in response".to_string()),
                    usage: None,
                    tool_calls: None,
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

        let max_retries = 3;
        let mut retry_count = 0;

        loop {
            match self
                .send_zai_request_with_tools_once(messages.clone(), tools.clone())
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
    ) -> Result<ApiResponse> {
        let zai_messages: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|msg| {
                let mut msg_obj = serde_json::json!({
                    "role": msg.role,
                });

                if let Some(content) = msg.content {
                    msg_obj["content"] = serde_json::json!(content);
                }

                if let Some(tool_calls) = msg.tool_calls {
                    msg_obj["tool_calls"] = serde_json::json!(tool_calls);
                }

                if let Some(tool_call_id) = msg.tool_call_id {
                    msg_obj["tool_call_id"] = serde_json::json!(tool_call_id);
                }

                msg_obj
            })
            .collect();

        let request = serde_json::json!({
            "model": self.model,
            "messages": zai_messages,
            "temperature": 0.7,
            "max_tokens": 2048,
            "stream": false,
            "tools": tools,
            "tool_choice": "auto"
        });

        // Create a new client specifically for Z.AI to force HTTP/1.1 for better compatibility
        let zai_client = Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("arula-cli/1.0")
            .http1_only() // Force HTTP/1.1 for Z.AI compatibility
            .tcp_nodelay(true)
            .connection_verbose(std::env::var("ARULA_DEBUG").is_ok())
            .build()
            .expect("Failed to create Z.AI HTTP client");

        let mut request_builder = zai_client
            .post(format!("{}/chat/completions", self.endpoint))
            .json(&request);

        if !self.api_key.is_empty() {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        if std::env::var("ARULA_DEBUG").is_ok() {
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

        if status.is_success() {
            let response_json: serde_json::Value = response.json().await?;

            if let Some(choices) = response_json["choices"].as_array() {
                if let Some(choice) = choices.first() {
                    let content = choice["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    let tool_calls = if let Some(calls) = choice["message"]["tool_calls"].as_array()
                    {
                        Some(
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
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        None
                    };

                    let usage = if let Some(usage_info) = response_json.get("usage") {
                        Some(Usage {
                            prompt_tokens: usage_info["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                            completion_tokens: usage_info["completion_tokens"].as_u64().unwrap_or(0)
                                as u32,
                            total_tokens: usage_info["total_tokens"].as_u64().unwrap_or(0) as u32,
                        })
                    } else {
                        None
                    };

                    Ok(ApiResponse {
                        response: content,
                        success: true,
                        error: None,
                        usage,
                        tool_calls,
                    })
                } else {
                    Ok(ApiResponse {
                        response: "No response received".to_string(),
                        success: false,
                        error: Some("No choices in response".to_string()),
                        usage: None,
                        tool_calls: None,
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
