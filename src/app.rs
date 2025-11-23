use crate::api::agent::{AgentOptionsBuilder, ContentBlock};
use crate::api::agent_client::AgentClient;
use crate::utils::chat::{ChatMessage, MessageType};
use crate::utils::config::Config;
use crate::utils::tool_call::{execute_bash_tool, ToolCall, ToolCallResult};
use anyhow::Result;
use futures::StreamExt;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Debug print helper that checks ARULA_DEBUG environment variable
fn debug_print(msg: &str) {
    if std::env::var("ARULA_DEBUG").is_ok() {
        println!("🔧 DEBUG: {}", msg);
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
    // Task handle for aborting in-flight requests
    pub current_task_handle: Option<tokio::task::JoinHandle<()>>,
    // Model caches for all providers
    pub openrouter_models: Arc<Mutex<Option<Vec<String>>>>,
    pub openai_models: Arc<Mutex<Option<Vec<String>>>>,
    pub anthropic_models: Arc<Mutex<Option<Vec<String>>>>,
    pub ollama_models: Arc<Mutex<Option<Vec<String>>>>,
    pub zai_models: Arc<Mutex<Option<Vec<String>>>>,
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
            current_task_handle: None,
            openrouter_models: Arc::new(Mutex::new(None)),
            openai_models: Arc::new(Mutex::new(None)),
            anthropic_models: Arc::new(Mutex::new(None)),
            ollama_models: Arc::new(Mutex::new(None)),
            zai_models: Arc::new(Mutex::new(None)),
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

        // Send message using modern agent in background
        let msg = message.to_string();
        let cancel_token = self.cancellation_token.clone();
        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    // Request was cancelled
                    let _ = tx.send(AiResponse::AgentStreamEnd);
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
                                                let _ = tx.send(AiResponse::AgentStreamText(text));
                                            }
                                            Some(ContentBlock::ToolCall {
                                                id,
                                                name,
                                                arguments,
                                            }) => {
                                                let _ = tx.send(AiResponse::AgentToolCall {
                                                    id,
                                                    name,
                                                    arguments,
                                                });
                                            }
                                            Some(ContentBlock::ToolResult {
                                                tool_call_id,
                                                result,
                                            }) => {
                                                let _ = tx.send(AiResponse::AgentToolResult {
                                                    tool_call_id,
                                                    success: result.success,
                                                    result: result.data,
                                                });
                                            }
                                            Some(ContentBlock::Error { error }) => {
                                                // Convert error to AgentStreamText to maintain compatibility
                                                let _ = tx.send(AiResponse::AgentStreamText(format!(
                                                    "[Error] {}",
                                                    error
                                                )));
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
                } => {}
            }
        });

        // Store the task handle so we can abort it on cancellation
        self.current_task_handle = Some(handle);

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