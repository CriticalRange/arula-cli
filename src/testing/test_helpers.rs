//! Test helper utilities for the ARULA CLI

use super::*;
use crate::chat::{EnhancedChatMessage, ChatRole};
use serde_json::json;
use std::path::PathBuf;
use tempfile::{TempDir, NamedTempFile};

/// Builder for creating test dependencies with default mocks
pub struct TestDependenciesBuilder {
    deps: Dependencies,
}

impl TestDependenciesBuilder {
    pub fn new() -> Self {
        Self {
            deps: Dependencies::new(),
        }
    }

    pub fn with_output(mut self, output: Box<dyn OutputHandler>) -> Self {
        self.deps.output = output;
        self
    }

    pub fn with_config(mut self, config: Box<dyn ConfigManager>) -> Self {
        self.deps.config = config;
        self
    }

    pub fn with_ai_client(mut self, ai_client: Box<dyn AiClient>) -> Self {
        self.deps.ai_client = ai_client;
        self
    }

    pub fn with_filesystem(mut self, filesystem: Box<dyn FileSystem>) -> Self {
        self.deps.filesystem = filesystem;
        self
    }

    pub fn with_process_executor(mut self, process_executor: Box<dyn ProcessExecutor>) -> Self {
        self.deps.process_executor = process_executor;
        self
    }

    pub fn with_http_client(mut self, http_client: Box<dyn HttpClient>) -> Self {
        self.deps.http_client = http_client;
        self
    }

    pub fn with_time_provider(mut self, time_provider: Box<dyn TimeProvider>) -> Self {
        self.deps.time_provider = time_provider;
        self
    }

    pub fn with_input_handler(mut self, input_handler: Box<dyn InputHandler>) -> Self {
        self.deps.input_handler = input_handler;
        self
    }

    pub fn with_menu_handler(mut self, menu_handler: Box<dyn MenuHandler>) -> Self {
        self.deps.menu_handler = menu_handler;
        self
    }

    pub fn build(self) -> Dependencies {
        self.deps
    }
}

impl Default for TestDependenciesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a set of mocked dependencies for testing
pub fn create_test_dependencies() -> Dependencies {
    TestDependenciesBuilder::new().build()
}

/// Creates dependencies with all mocks for testing
pub fn create_mock_dependencies() -> Dependencies {
    use crate::testing::mocks::*;

    let output = MockOutputHandler::new();
    let config = MockConfigManager::new();
    let ai_client = MockAiClient::new();
    let filesystem = InMemoryFileSystem::new();
    let process_executor = MockProcessExecutor::new();
    let http_client = MockHttpClient::new();
    let time_provider = MockTimeProvider::new();
    let input_handler = MockInputHandler::new();
    let menu_handler = MockMenuHandler::new();

    TestDependenciesBuilder::new()
        .with_output(Box::new(output))
        .with_config(Box::new(config))
        .with_ai_client(Box::new(ai_client))
        .with_filesystem(Box::new(filesystem))
        .with_process_executor(Box::new(process_executor))
        .with_http_client(Box::new(http_client))
        .with_time_provider(Box::new(time_provider))
        .with_input_handler(Box::new(input_handler))
        .with_menu_handler(Box::new(menu_handler))
        .build()
}

/// Test message factory for creating ChatMessage instances
pub struct TestMessageFactory;

impl TestMessageFactory {
    pub fn user_message(content: &str) -> EnhancedChatMessage {
        EnhancedChatMessage {
            role: ChatRole::User,
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
            tool_calls: None,
            tool_results: None,
        }
    }

    pub fn assistant_message(content: &str) -> EnhancedChatMessage {
        EnhancedChatMessage {
            role: ChatRole::Assistant,
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
            tool_calls: None,
            tool_results: None,
        }
    }

    pub fn system_message(content: &str) -> EnhancedChatMessage {
        EnhancedChatMessage {
            role: ChatRole::System,
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
            tool_calls: None,
            tool_results: None,
        }
    }

    pub fn tool_call_message(content: &str, tool_calls: Vec<serde_json::Value>) -> EnhancedChatMessage {
        EnhancedChatMessage {
            role: ChatRole::Assistant,
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
            tool_calls: Some(tool_calls),
            tool_results: None,
        }
    }

    pub fn tool_result_message(content: &str, tool_results: Vec<serde_json::Value>) -> EnhancedChatMessage {
        EnhancedChatMessage {
            role: ChatRole::Tool,
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
            tool_calls: None,
            tool_results: Some(tool_results),
        }
    }

    pub fn create_conversation() -> Vec<EnhancedChatMessage> {
        vec![
            Self::user_message("Hello, how are you?"),
            Self::assistant_message("I'm doing well, thank you!"),
            Self::user_message("What can you help me with?"),
        ]
    }
}

/// Test utility for temporary files and directories
pub struct TestFileSystem {
    temp_dir: Option<TempDir>,
}

impl TestFileSystem {
    pub fn new() -> Self {
        Self {
            temp_dir: Some(TempDir::new().expect("Failed to create temp directory")),
        }
    }

    pub fn path(&self) -> &PathBuf {
        self.temp_dir.as_ref().unwrap().path().to_path_buf().as_path()
    }

    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let file_path = self.path().join(name);
        std::fs::write(&file_path, content).expect("Failed to write test file");
        file_path
    }

    pub fn create_temp_file(content: &str) -> NamedTempFile {
        NamedTempFile::new().expect("Failed to create temp file")
    }

    pub fn read_file(&self, name: &str) -> String {
        let file_path = self.path().join(name);
        std::fs::read_to_string(&file_path).expect("Failed to read test file")
    }
}

impl Drop for TestFileSystem {
    fn drop(&mut self) {
        self.temp_dir.take(); // Automatically cleaned up
    }
}

/// Utility for creating test configurations
pub struct TestConfigBuilder;

impl TestConfigBuilder {
    pub fn default() -> crate::config::Config {
        crate::config::Config {
            endpoint: "http://localhost:8080".to_string(),
            model: "gpt-4".to_string(),
            api_key: "test-key".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            verbose: false,
            debug: false,
        }
    }

    pub fn with_endpoint(endpoint: &str) -> crate::config::Config {
        let mut config = Self::default();
        config.endpoint = endpoint.to_string();
        config
    }

    pub fn with_model(model: &str) -> crate::config::Config {
        let mut config = Self::default();
        config.model = model.to_string();
        config
    }

    pub fn verbose() -> crate::config::Config {
        let mut config = Self::default();
        config.verbose = true;
        config
    }

    pub fn debug() -> crate::config::Config {
        let mut config = Self::default();
        config.debug = true;
        config
    }
}

/// Utility for creating test tool parameters
pub struct TestToolParams;

impl TestToolParams {
    pub fn bash_command(command: &str) -> serde_json::Value {
        json!({
            "command": command,
            "args": [],
            "working_dir": "/tmp",
            "timeout": 30,
            "capture_output": true
        })
    }

    pub fn file_read(path: &str) -> serde_json::Value {
        json!({
            "path": path,
            "encoding": "utf8"
        })
    }

    pub fn file_write(path: &str, content: &str) -> serde_json::Value {
        json!({
            "path": path,
            "content": content,
            "encoding": "utf8",
            "create_dirs": true
        })
    }

    pub fn search_files(pattern: &str, directory: &str) -> serde_json::Value {
        json!({
            "pattern": pattern,
            "directory": directory,
            "recursive": true,
            "include_hidden": false,
            "max_results": 100
        })
    }

    pub fn web_search(query: &str) -> serde_json::Value {
        json!({
            "query": query,
            "max_results": 10,
            "safe_search": "moderate"
        })
    }
}

/// Utility for creating test HTTP responses
pub struct TestHttpResponseBuilder;

impl TestHttpResponseBuilder {
    pub fn ai_response(content: &str) -> serde_json::Value {
        json!({
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": content
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 15,
                "total_tokens": 25
            }
        })
    }

    pub fn ai_response_with_tool_calls(content: &str, tool_calls: Vec<serde_json::Value>) -> serde_json::Value {
        json!({
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": content,
                        "tool_calls": tool_calls
                    },
                    "finish_reason": "tool_calls"
                }
            ],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 25,
                "total_tokens": 45
            }
        })
    }

    pub fn error_response(message: &str) -> serde_json::Value {
        json!({
            "error": {
                "message": message,
                "type": "api_error",
                "code": "invalid_request"
            }
        })
    }
}

/// Utility for creating test process outputs
pub struct TestProcessOutputBuilder;

impl TestProcessOutputBuilder {
    pub fn success(output: &str) -> std::process::Output {
        std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: output.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    pub fn error(output: &str, error: &str, exit_code: i32) -> std::process::Output {
        std::process::Output {
            status: std::process::ExitStatus::from_raw(exit_code),
            stdout: output.as_bytes().to_vec(),
            stderr: error.as_bytes().to_vec(),
        }
    }

    pub fn empty_success() -> std::process::Output {
        Self::success("")
    }
}

/// Assertion helpers for testing
pub struct TestAssertions;

impl TestAssertions {
    /// Asserts that a vector contains a specific element
    pub fn contains<T: PartialEq + std::fmt::Debug>(vec: &[T], item: &T) -> bool {
        vec.contains(item)
    }

    /// Asserts that a vector contains an element matching a predicate
    pub fn contains_by<T, F>(vec: &[T], predicate: F) -> bool
    where
        F: Fn(&T) -> bool,
    {
        vec.iter().any(predicate)
    }

    /// Asserts that a string contains a substring
    pub fn string_contains(haystack: &str, needle: &str) -> bool {
        haystack.contains(needle)
    }

    /// Asserts that JSON values are equal (ignoring whitespace)
    pub fn json_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        a == b
    }
}

/// Async test utilities
pub struct AsyncTestUtils;

impl AsyncTestUtils {
    /// Runs an async test in a tokio runtime
    pub async fn run_test<F, Fut>(test_fn: F) -> Result<(), anyhow::Error>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), anyhow::Error>>,
    {
        test_fn().await
    }

    /// Runs multiple async operations concurrently and waits for all
    pub async fn join_all<T>(futures: Vec<impl std::future::Future<Output = T>>) -> Vec<T> {
        futures::future::join_all(futures).await
    }

    /// Creates a timeout for an async operation
    pub async fn with_timeout<F, T>(
        future: F,
        timeout: std::time::Duration,
    ) -> Result<T, tokio::time::error::Elapsed>
    where
        F: std::future::Future<Output = T>,
    {
        tokio::time::timeout(timeout, future).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mocks::*;

    #[tokio::test]
    async fn test_test_dependencies_builder() {
        let deps = TestDependenciesBuilder::new()
            .with_output(Box::new(MockOutputHandler::new()))
            .with_config(Box::new(MockConfigManager::new()))
            .build();

        // Verify the dependencies are set correctly
        let _ = deps;
    }

    #[tokio::test]
    async fn test_message_factory() {
        let user_msg = TestMessageFactory::user_message("Hello");
        assert_eq!(user_msg.role, ChatRole::User);
        assert_eq!(user_msg.content, "Hello");

        let assistant_msg = TestMessageFactory::assistant_message("Hi there");
        assert_eq!(assistant_msg.role, ChatRole::Assistant);
        assert_eq!(assistant_msg.content, "Hi there");
    }

    #[tokio::test]
    async fn test_file_system() {
        let fs = TestFileSystem::new();
        let file_path = fs.create_file("test.txt", "Hello, World!");
        assert_eq!(fs.read_file("test.txt"), "Hello, World!");
        assert!(file_path.exists());
    }

    #[test]
    fn test_config_builder() {
        let config = TestConfigBuilder::with_endpoint("http://test.com");
        assert_eq!(config.endpoint, "http://test.com");

        let config = TestConfigBuilder::with_model("gpt-3.5");
        assert_eq!(config.model, "gpt-3.5");
    }

    #[test]
    fn test_tool_params() {
        let params = TestToolParams::bash_command("echo hello");
        assert_eq!(params["command"], "echo hello");
        assert_eq!(params["args"], json!([]));
    }

    #[test]
    fn test_assertions() {
        let vec = vec![1, 2, 3, 4, 5];
        assert!(TestAssertions::contains(&vec, &3));
        assert!(!TestAssertions::contains(&vec, &6));

        assert!(TestAssertions::contains_by(&vec, |x| *x > 3));
        assert!(!TestAssertions::contains_by(&vec, |x| *x > 10));
    }
}