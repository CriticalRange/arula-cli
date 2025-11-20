//! Testable version of the App with dependency injection

use crate::agent::{AgentOptionsBuilder, ContentBlock};
use crate::agent_client::AgentClient;
use crate::chat::{EnhancedChatMessage, ChatRole};
use crate::config::Config;
use crate::tool_call::{ToolCall, ToolCallResult};
use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

// Trait definitions for dependency injection
#[async_trait]
pub trait OutputHandler: Send + Sync {
    async fn print_message(&mut self, role: ChatRole, content: &str) -> std::io::Result<()>;
    async fn print_error(&mut self, error: &str) -> std::io::Result<()>;
    async fn start_ai_message(&mut self) -> std::io::Result<()>;
    async fn end_ai_message(&mut self) -> std::io::Result<()>;
    async fn print_streaming_chunk(&mut self, chunk: &str) -> std::io::Result<()>;
}

#[async_trait]
pub trait ConfigManager: Send + Sync {
    async fn load_config(&self) -> Result<crate::config::Config, anyhow::Error>;
    async fn save_config(&self, config: &crate::config::Config) -> Result<(), anyhow::Error>;
    async fn get_default_endpoint(&self) -> Result<String, anyhow::Error>;
}

type StreamResult = Result<Box<dyn futures::Stream<Item = Result<String, anyhow::Error>> + Send + Unpin>, anyhow::Error>;

#[async_trait]
pub trait AiClient: Send + Sync {
    async fn send_message(&self, message: &str, history: &[EnhancedChatMessage]) -> Result<String, anyhow::Error>;
    async fn send_message_stream(&self, message: &str, history: &[EnhancedChatMessage]) -> StreamResult;
}

#[async_trait]
pub trait FileSystem: Send + Sync {
    async fn read_file(&self, path: &std::path::PathBuf) -> Result<Vec<u8>, anyhow::Error>;
    async fn write_file(&self, path: &std::path::PathBuf, contents: &[u8]) -> Result<(), anyhow::Error>;
    async fn exists(&self, path: &std::path::PathBuf) -> bool;
    async fn create_dir_all(&self, path: &std::path::PathBuf) -> Result<(), anyhow::Error>;
}

#[async_trait]
pub trait ProcessExecutor: Send + Sync {
    async fn execute_command(&self, command: &str, args: &[&str]) -> Result<std::process::Output, anyhow::Error>;
}

pub trait TimeProvider: Send + Sync {
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
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

/// Trait for the core application logic
#[async_trait]
pub trait Application: Send + Sync {
    async fn initialize(&mut self) -> Result<()>;
    async fn send_message(&mut self, message: &str) -> Result<()>;
    async fn check_ai_response(&mut self) -> Option<AiResponse>;
    fn get_message_history(&self) -> &[EnhancedChatMessage];
    fn clear_conversation(&mut self);
    async fn cancel_request(&mut self);
    fn is_waiting_for_response(&self) -> bool;
    fn has_pending_tool_calls(&self) -> bool;
    fn get_pending_tool_calls(&mut self) -> Option<Vec<ToolCall>>;
    fn get_pending_tool_results(&mut self) -> Option<Vec<ToolCallResult>>;
}

/// Testable application implementation with dependency injection
pub struct TestableApp {
    config: Arc<Config>,
    agent_client: Option<AgentClient>,
    messages: Vec<EnhancedChatMessage>,
    ai_response_rx: Option<mpsc::UnboundedReceiver<AiResponse>>,
    current_streaming_message: Option<String>,
    pending_tool_calls: Option<Vec<ToolCall>>,
    pending_tool_results: Option<Vec<ToolCallResult>>,
    cancellation_token: CancellationToken,
    current_task_handle: Option<tokio::task::JoinHandle<()>>,
    debug: bool,

    // Injected dependencies
    output_handler: Arc<dyn OutputHandler>,
    config_manager: Arc<dyn ConfigManager>,
    ai_client: Arc<dyn AiClient>,
    filesystem: Arc<dyn FileSystem>,
    process_executor: Arc<dyn ProcessExecutor>,
    time_provider: Arc<dyn TimeProvider>,
}

impl TestableApp {
    pub fn new(
        config: Config,
        output_handler: Arc<dyn OutputHandler>,
        config_manager: Arc<dyn ConfigManager>,
        ai_client: Arc<dyn AiClient>,
        filesystem: Arc<dyn FileSystem>,
        process_executor: Arc<dyn ProcessExecutor>,
        time_provider: Arc<dyn TimeProvider>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            agent_client: None,
            messages: Vec::new(),
            ai_response_rx: None,
            current_streaming_message: None,
            pending_tool_calls: None,
            pending_tool_results: None,
            cancellation_token: CancellationToken::new(),
            current_task_handle: None,
            debug: false,
            output_handler,
            config_manager,
            ai_client,
            filesystem,
            process_executor,
            time_provider,
        }
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Build comprehensive system prompt from ARULA.md files
    async fn build_system_prompt(&self) -> Result<String> {
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

Always format your responses with proper code blocks, markdown, and clear explanations.
"#.to_string());

        // Load additional context from ARULA.md files if available
        if self.load_arula_context().await.is_ok() {
            prompt_parts.push(self.load_arula_context().await?);
        }

        Ok(prompt_parts.join("\n\n"))
    }

    async fn load_arula_context(&self) -> Result<String> {
        // Look for ARULA.md in various directories
        let search_paths = vec![
            std::env::current_dir().unwrap().join("ARULA.md"),
            std::env::current_dir().unwrap().join(".claude/CLAUDE.md"),
        ];

        for path in search_paths {
            if self.filesystem.exists(&path).await {
                let content = self.filesystem.read_file(&path).await?;
                if let Ok(content_str) = String::from_utf8(content) {
                    return Ok(content_str);
                }
            }
        }

        Ok("No ARULA.md context found".to_string())
    }

    async fn initialize_agent_client(&mut self) -> Result<()> {
        let agent_options = AgentOptionsBuilder::new()
            .with_endpoint(&self.config.endpoint)
            .with_model(&self.config.model)
            .with_api_key(&self.config.api_key)
            .with_temperature(self.config.temperature)
            .with_max_tokens(self.config.max_tokens)
            .build();

        self.agent_client = Some(AgentClient::new(agent_options));
        Ok(())
    }

    async fn send_to_ai(&mut self, message: &str) -> Result<()> {
        let user_message = EnhancedChatMessage {
            role: ChatRole::User,
            content: message.to_string(),
            timestamp: self.time_provider.now(),
            tool_calls: None,
            tool_results: None,
        };

        self.messages.push(user_message);

        let system_prompt = self.build_system_prompt().await?;
        let messages_with_system = vec![
            EnhancedChatMessage {
                role: ChatRole::System,
                content: system_prompt,
                timestamp: self.time_provider.now(),
                tool_calls: None,
                tool_results: None,
            },
            self.messages.clone().into_iter().nth(self.messages.len() - 1).unwrap_or_default(),
        ];

        if let Some(agent_client) = &self.agent_client {
            let message_content = message.to_string();
            let (tx, rx) = mpsc::unbounded_channel();
            self.ai_response_rx = Some(rx);

            let client = agent_client.clone();
            let token = self.cancellation_token.clone();
            let messages_clone = messages_with_system.clone();

            self.current_task_handle = Some(tokio::spawn(async move {
                let options = AgentOptionsBuilder::new()
                    .with_temperature(0.7)
                    .with_max_tokens(4096)
                    .build();

                let mut stream = match client.send_message_stream(&message_content, &messages_clone, &options).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        let _ = tx.send(AiResponse::AgentStreamEnd);
                        if token.is_cancelled() {
                            return;
                        }
                        eprintln!("Error sending message: {}", e);
                        return;
                    }
                };

                let _ = tx.send(AiResponse::AgentStreamStart);

                while let Some(response) = stream.next().await {
                    if token.is_cancelled() {
                        break;
                    }

                    match response {
                        Ok(chunk) => {
                            if let Ok(content_block) = serde_json::from_str::<ContentBlock>(&chunk) {
                                match content_block.block_type.as_str() {
                                    "text" => {
                                        let text = content_block.text.unwrap_or_default();
                                        if !text.is_empty() {
                                            let _ = tx.send(AiResponse::AgentStreamText(text));
                                        }
                                    }
                                    "tool_call" => {
                                        if let Some(tool_call) = content_block.tool_call {
                                            let _ = tx.send(AiResponse::AgentToolCall {
                                                id: tool_call.id.unwrap_or_default(),
                                                name: tool_call.name.unwrap_or_default(),
                                                arguments: tool_call.arguments.unwrap_or_default(),
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Stream error: {}", e);
                            break;
                        }
                    }
                }

                let _ = tx.send(AiResponse::AgentStreamEnd);
            }));

            Ok(())
        } else {
            Err(anyhow::anyhow!("AI client not initialized"))
        }
    }

    async fn process_tool_result(&mut self, tool_call_id: String, tool_name: &str, tool_args: &str) -> Result<()> {
        let result = match tool_name {
            "execute_bash" => {
                let params: serde_json::Value = serde_json::from_str(tool_args)?;
                let command = params["command"].as_str().unwrap_or("");

                let result = match self.process_executor.execute_command(
                    command,
                    &[]
                ).await {
                    Ok(output) => json!({
                        "success": true,
                        "stdout": String::from_utf8_lossy(&output.stdout),
                        "stderr": String::from_utf8_lossy(&output.stderr),
                        "exit_code": output.status.code().unwrap_or(-1)
                    }),
                    Err(e) => json!({
                        "success": false,
                        "error": e.to_string()
                    })
                };

                result
            }
            "read_file" => {
                let params: serde_json::Value = serde_json::from_str(tool_args)?;
                let path = params["path"].as_str().unwrap_or("");
                let path_buf = std::path::PathBuf::from(path);

                match self.filesystem.read_file(&path_buf).await {
                    Ok(content) => {
                        let content_str = String::from_utf8(content)
                            .unwrap_or_else(|_| "Invalid UTF-8 content".to_string());
                        json!({
                            "success": true,
                            "content": content_str
                        })
                    }
                    Err(e) => json!({
                        "success": false,
                        "error": e.to_string()
                    })
                }
            }
            "write_file" => {
                let params: serde_json::Value = serde_json::from_str(tool_args)?;
                let path = params["path"].as_str().unwrap_or("");
                let content = params["content"].as_str().unwrap_or("");
                let path_buf = std::path::PathBuf::from(path);

                match self.filesystem.write_file(&path_buf, content.as_bytes()).await {
                    Ok(_) => json!({
                        "success": true,
                        "message": "File written successfully"
                    }),
                    Err(e) => json!({
                        "success": false,
                        "error": e.to_string()
                    })
                }
            }
            _ => json!({
                "success": false,
                "error": format!("Unknown tool: {}", tool_name)
            })
        };

        let tool_result = AiResponse::AgentToolResult {
            tool_call_id,
            success: result["success"].as_bool().unwrap_or(false),
            result,
        };

        if let Some(tx) = self.ai_response_rx.as_ref() {
            // This is a bit tricky with the current architecture
            // We'll need to modify this to properly handle tool results
        }

        Ok(())
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    pub fn set_model(&mut self, model: &str) {
        let config = Arc::make_mut(&mut self.config);
        config.model = model.to_string();
    }
}

#[async_trait]
impl Application for TestableApp {
    async fn initialize(&mut self) -> Result<()> {
        self.initialize_agent_client().await?;
        Ok(())
    }

    async fn send_message(&mut self, message: &str) -> Result<()> {
        self.send_to_ai(message).await?;
        Ok(())
    }

    async fn check_ai_response(&mut self) -> Option<AiResponse> {
        if let Some(ref mut rx) = self.ai_response_rx {
            match rx.try_recv() {
                Ok(response) => Some(response),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    self.ai_response_rx = None;
                    None
                }
            }
        } else {
            None
        }
    }

    fn get_message_history(&self) -> &[EnhancedChatMessage] {
        &self.messages
    }

    fn clear_conversation(&mut self) {
        self.messages.clear();
        self.current_streaming_message = None;
        self.pending_tool_calls = None;
        self.pending_tool_results = None;
    }

    async fn cancel_request(&mut self) {
        self.cancellation_token.cancel();

        if let Some(handle) = self.current_task_handle.take() {
            handle.abort();
        }

        self.ai_response_rx = None;
        self.current_streaming_message = None;
        self.pending_tool_calls = None;
        self.pending_tool_results = None;

        // Create a new cancellation token for future requests
        self.cancellation_token = CancellationToken::new();
    }

    fn is_waiting_for_response(&self) -> bool {
        self.ai_response_rx.is_some() || self.current_task_handle.is_some()
    }

    fn has_pending_tool_calls(&self) -> bool {
        self.pending_tool_calls.is_some() &&
            self.pending_tool_calls.as_ref().unwrap().len() > 0
    }

    fn get_pending_tool_calls(&mut self) -> Option<Vec<ToolCall>> {
        self.pending_tool_calls.take()
    }

    fn get_pending_tool_results(&mut self) -> Option<Vec<ToolCallResult>> {
        self.pending_tool_results.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{
        mocks::*, test_helpers::*, *,
    };
    use crate::chat::{ChatRole};

    #[tokio::test]
    async fn test_app_initialization() {
        let config = TestConfigBuilder::default();
        let deps = create_mock_dependencies();

        let mut app = TestableApp::new(
            config,
            Arc::new(MockOutputHandler::new()),
            Arc::new(MockConfigManager::new()),
            Arc::new(MockAiClient::new()),
            Arc::new(InMemoryFileSystem::new()),
            Arc::new(MockProcessExecutor::new()),
            Arc::new(MockTimeProvider::new()),
        );

        let result = app.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_message() {
        let config = TestConfigBuilder::default();
        let filesystem = Arc::new(InMemoryFileSystem::new());
        let process_executor = Arc::new(MockProcessExecutor::new());
        let time_provider = Arc::new(MockTimeProvider::new());

        let mut app = TestableApp::new(
            config,
            Arc::new(MockOutputHandler::new()),
            Arc::new(MockConfigManager::new()),
            Arc::new(MockAiClient::new()),
            filesystem.clone(),
            process_executor.clone(),
            time_provider.clone(),
        );

        // Initialize with a mock agent client
        app.initialize().await.unwrap();

        let result = app.send_message("Hello, world!").await;
        assert!(result.is_ok());

        let history = app.get_message_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].role, ChatRole::User);
        assert_eq!(history[0].content, "Hello, world!");
    }

    #[tokio::test]
    async fn test_clear_conversation() {
        let config = TestConfigBuilder::default();
        let deps = create_mock_dependencies();

        let mut app = TestableApp::new(
            config,
            Arc::new(MockOutputHandler::new()),
            Arc::new(MockConfigManager::new()),
            Arc::new(MockAiClient::new()),
            Arc::new(InMemoryFileSystem::new()),
            Arc::new(MockProcessExecutor::new()),
            Arc::new(MockTimeProvider::new()),
        );

        app.initialize().await.unwrap();
        app.send_message("Test message").await.unwrap();

        assert_eq!(app.get_message_history().len(), 1);

        app.clear_conversation();

        assert_eq!(app.get_message_history().len(), 0);
        assert!(!app.has_pending_tool_calls());
        assert!(app.get_pending_tool_results().unwrap_or_default().is_empty());
    }

    #[tokio::test]
    async fn test_cancel_request() {
        let config = TestConfigBuilder::default();
        let deps = create_mock_dependencies();

        let mut app = TestableApp::new(
            config,
            Arc::new(MockOutputHandler::new()),
            Arc::new(MockConfigManager::new()),
            Arc::new(MockAiClient::new()),
            Arc::new(InMemoryFileSystem::new()),
            Arc::new(MockProcessExecutor::new()),
            Arc::new(MockTimeProvider::new()),
        );

        app.initialize().await.unwrap();
        app.send_message("Test message").await.unwrap();

        assert!(app.is_waiting_for_response());

        app.cancel_request().await;

        assert!(!app.is_waiting_for_response());
    }

    #[tokio::test]
    async fn test_build_system_prompt() {
        let config = TestConfigBuilder::default();
        let filesystem = Arc::new(InMemoryFileSystem::new());

        // Add a mock ARULA.md file
        let arula_path = std::path::PathBuf::from("ARULA.md");
        filesystem.add_file(arula_path, b"# Custom ARULA Context\nThis is test content".to_vec()).await;

        let app = TestableApp::new(
            config,
            Arc::new(MockOutputHandler::new()),
            Arc::new(MockConfigManager::new()),
            Arc::new(MockAiClient::new()),
            filesystem.clone(),
            Arc::new(MockProcessExecutor::new()),
            Arc::new(MockTimeProvider::new()),
        );

        let prompt = app.build_system_prompt().await.unwrap();
        assert!(prompt.contains("ARULA, an Autonomous AI Interface assistant"));
        assert!(prompt.contains("Available Tools"));
    }
}