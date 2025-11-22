//! Mock implementations for testing

use super::*;
use async_trait::async_trait;
use mockall::mock;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::sync::Mutex;
use std::sync::Arc;

mock! {
    pub OutputHandler {}

    #[async_trait]
    impl super::OutputHandler for OutputHandler {
        async fn print_message(&mut self, role: ChatRole, content: &str) -> std::io::Result<()>;
        async fn print_error(&mut self, error: &str) -> std::io::Result<()>;
        async fn start_ai_message(&mut self) -> std::io::Result<()>;
        async fn end_ai_message(&mut self) -> std::io::Result<()>;
        async fn print_streaming_chunk(&mut self, chunk: &str) -> std::io::Result<()>;
    }
}

mock! {
    pub ConfigManager {}

    #[async_trait]
    impl super::ConfigManager for ConfigManager {
        async fn load_config(&self) -> Result<crate::config::Config, anyhow::Error>;
        async fn save_config(&self, config: &crate::config::Config) -> Result<(), anyhow::Error>;
        async fn get_default_endpoint(&self) -> Result<String, anyhow::Error>;
    }
}

mock! {
    pub AiClient {}

    #[async_trait]
    impl super::AiClient for AiClient {
        async fn send_message(&self, message: &str, history: &[EnhancedChatMessage]) -> Result<String, anyhow::Error>;
        async fn send_message_stream(&self, message: &str, history: &[EnhancedChatMessage]) -> Result<Box<dyn tokio::stream::Item = Result<String, anyhow::Error>> + Send + Unpin, anyhow::Error>;
    }
}

mock! {
    pub FileSystem {}

    #[async_trait]
    impl super::FileSystem for FileSystem {
        async fn read_file(&self, path: &PathBuf) -> Result<Vec<u8>, anyhow::Error>;
        async fn write_file(&self, path: &PathBuf, contents: &[u8]) -> Result<(), anyhow::Error>;
        async fn exists(&self, path: &PathBuf) -> bool;
        async fn create_dir_all(&self, path: &PathBuf) -> Result<(), anyhow::Error>;
        async fn list_directory(&self, path: &PathBuf) -> Result<Vec<PathBuf>, anyhow::Error>;
    }
}

mock! {
    pub ProcessExecutor {}

    #[async_trait]
    impl super::ProcessExecutor for ProcessExecutor {
        async fn execute_command(&self, command: &str, args: &[&str]) -> Result<std::process::Output, anyhow::Error>;
        async fn execute_command_with_input(&self, command: &str, args: &[&str], input: &[u8]) -> Result<std::process::Output, anyhow::Error>;
    }
}

mock! {
    pub HttpClient {}

    #[async_trait]
    impl super::HttpClient for HttpClient {
        async fn post_json(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value, anyhow::Error>;
        async fn post_json_stream(&self, url: &str, body: &serde_json::Value) -> Result<Box<dyn tokio::stream::Item = Result<String, anyhow::Error>> + Send + Unpin, anyhow::Error>;
        async fn get(&self, url: &str) -> Result<serde_json::Value, anyhow::Error>;
    }
}

mock! {
    pub TimeProvider {}

    impl super::TimeProvider for TimeProvider {
        fn now(&self) -> chrono::DateTime<chrono::Utc>;
        fn sleep(&self, duration: std::time::Duration) -> tokio::time::Sleep;
    }
}

mock! {
    pub InputHandler {}

    #[async_trait]
    impl super::InputHandler for InputHandler {
        async fn read_line(&mut self) -> Result<String, anyhow::Error>;
        async fn read_password(&mut self) -> Result<String, anyhow::Error>;
        async fn confirm(&mut self, message: &str) -> Result<bool, anyhow::Error>;
        async fn select_option(&mut self, message: &str, options: &[String]) -> Result<usize, anyhow::Error>;
    }
}

mock! {
    pub MenuHandler {}

    #[async_trait]
    impl super::MenuHandler for MenuHandler {
        async fn show_main_menu(&mut self) -> Result<MenuSelection, anyhow::Error>;
        async fn show_confirm_dialog(&mut self, message: &str) -> Result<bool, anyhow::Error>;
        async fn show_input_dialog(&mut self, message: &str) -> Result<String, anyhow::Error>;
    }
}

/// Simple in-memory filesystem mock for testing
pub struct InMemoryFileSystem {
    files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
}

impl InMemoryFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_file(&self, path: PathBuf, contents: Vec<u8>) {
        let mut files = self.files.lock().await;
        files.insert(path, contents);
    }

    pub async fn clear(&self) {
        let mut files = self.files.lock().await;
        files.clear();
    }
}

#[async_trait]
impl super::FileSystem for InMemoryFileSystem {
    async fn read_file(&self, path: &PathBuf) -> Result<Vec<u8>, anyhow::Error> {
        let files = self.files.lock().await;
        files.get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
    }

    async fn write_file(&self, path: &PathBuf, contents: &[u8]) -> Result<(), anyhow::Error> {
        let mut files = self.files.lock().await;
        files.insert(path.clone(), contents.to_vec());
        Ok(())
    }

    async fn exists(&self, path: &PathBuf) -> bool {
        let files = self.files.lock().await;
        files.contains_key(path)
    }

    async fn create_dir_all(&self, _path: &PathBuf) -> Result<(), anyhow::Error> {
        // For in-memory filesystem, directories are implicit
        Ok(())
    }

    async fn list_directory(&self, path: &PathBuf) -> Result<Vec<PathBuf>, anyhow::Error> {
        let files = self.files.lock().await;
        let mut result = Vec::new();

        for file_path in files.keys() {
            if let Some(parent) = file_path.parent() {
                if parent == path {
                    result.push(file_path.clone());
                }
            }
        }

        Ok(result)
    }
}

/// Mock HTTP client that simulates responses
pub struct MockHttpClient {
    responses: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn set_response(&self, url: String, response: serde_json::Value) {
        let mut responses = self.responses.lock().await;
        responses.insert(url, response);
    }

    pub async fn clear(&self) {
        let mut responses = self.responses.lock().await;
        responses.clear();
    }
}

#[async_trait]
impl super::HttpClient for MockHttpClient {
    async fn post_json(&self, url: &str, _body: &serde_json::Value) -> Result<serde_json::Value, anyhow::Error> {
        let responses = self.responses.lock().await;
        responses.get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No mock response configured for URL: {}", url))
    }

    async fn post_json_stream(&self, url: &str, _body: &serde_json::Value) -> Result<Box<dyn tokio::stream::Item = Result<String, anyhow::Error>> + Send + Unpin, anyhow::Error> {
        let responses = self.responses.lock().await;
        if let Some(response) = responses.get(url) {
            let response_str = response.to_string();
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let _ = tx.send(Ok(response_str));
            Ok(Box::new(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)))
        } else {
            Err(anyhow::anyhow!("No mock response configured for URL: {}", url))
        }
    }

    async fn get(&self, url: &str) -> Result<serde_json::Value, anyhow::Error> {
        let responses = self.responses.lock().await;
        responses.get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No mock response configured for URL: {}", url))
    }
}

/// Mock process executor that captures commands for testing
pub struct MockProcessExecutor {
    commands: Arc<Mutex<Vec<(String, Vec<String>)>>>,
    outputs: Arc<Mutex<HashMap<String, std::process::Output>>>,
}

impl MockProcessExecutor {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            outputs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_executed_commands(&self) -> Vec<(String, Vec<String>)> {
        let commands = self.commands.lock().await;
        commands.clone()
    }

    pub async fn set_output(&self, key: String, output: std::process::Output) {
        let mut outputs = self.outputs.lock().await;
        outputs.insert(key, output);
    }

    pub async fn clear(&self) {
        let mut commands = self.commands.lock().await;
        commands.clear();
        let mut outputs = self.outputs.lock().await;
        outputs.clear();
    }
}

#[async_trait]
impl super::ProcessExecutor for MockProcessExecutor {
    async fn execute_command(&self, command: &str, args: &[&str]) -> Result<std::process::Output, anyhow::Error> {
        let mut commands = self.commands.lock().await;
        commands.push((command.to_string(), args.iter().map(|s| s.to_string()).collect()));

        let outputs = self.outputs.lock().await;
        let key = format!("{} {}", command, args.join(" "));

        if let Some(output) = outputs.get(&key) {
            Ok(output.clone())
        } else {
            // Default successful output
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: b"".to_vec(),
                stderr: b"".to_vec(),
            })
        }
    }

    async fn execute_command_with_input(&self, command: &str, args: &[&str], _input: &[u8]) -> Result<std::process::Output, anyhow::Error> {
        self.execute_command(command, args).await
    }
}

/// Mock time provider for deterministic time-based tests
pub struct MockTimeProvider {
    current_time: Arc<Mutex<chrono::DateTime<chrono::Utc>>>,
}

impl MockTimeProvider {
    pub fn new() -> Self {
        Self {
            current_time: Arc::new(Mutex::new(chrono::Utc::now())),
        }
    }

    pub async fn set_time(&self, time: chrono::DateTime<chrono::Utc>) {
        let mut current_time = self.current_time.lock().await;
        *current_time = time;
    }

    pub async fn advance_time(&self, duration: chrono::Duration) {
        let mut current_time = self.current_time.lock().await;
        *current_time = *current_time + duration;
    }
}

impl super::TimeProvider for MockTimeProvider {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        // Note: This is a synchronous method but uses async internally
        // In practice, tests will call this within tokio context
        let current_time = self.current_time.clone();
        let handle = tokio::runtime::Handle::current();
        handle.block_on(async move {
            *current_time.lock().await
        })
    }

    fn sleep(&self, duration: std::time::Duration) -> tokio::time::Sleep {
        tokio::time::sleep(duration)
    }
}

/// Simple mock input handler for testing
pub struct MockInputHandler {
    inputs: Arc<Mutex<Vec<String>>>,
    confirms: Arc<Mutex<Vec<bool>>>,
    selections: Arc<Mutex<Vec<usize>>>,
}

impl MockInputHandler {
    pub fn new() -> Self {
        Self {
            inputs: Arc::new(Mutex::new(Vec::new())),
            confirms: Arc::new(Mutex::new(Vec::new())),
            selections: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_input(&self, input: String) {
        let mut inputs = self.inputs.lock().await;
        inputs.push(input);
    }

    pub async fn add_confirm(&self, confirm: bool) {
        let mut confirms = self.confirms.lock().await;
        confirms.push(confirm);
    }

    pub async fn add_selection(&self, selection: usize) {
        let mut selections = self.selections.lock().await;
        selections.push(selection);
    }

    pub async fn clear(&self) {
        self.inputs.lock().await.clear();
        self.confirms.lock().await.clear();
        self.selections.lock().await.clear();
    }
}

#[async_trait]
impl super::InputHandler for MockInputHandler {
    async fn read_line(&mut self) -> Result<String, anyhow::Error> {
        let mut inputs = self.inputs.lock().await;
        inputs.pop()
            .ok_or_else(|| anyhow::anyhow!("No input queued"))
    }

    async fn read_password(&mut self) -> Result<String, anyhow::Error> {
        let mut inputs = self.inputs.lock().await;
        inputs.pop()
            .ok_or_else(|| anyhow::anyhow!("No password input queued"))
    }

    async fn confirm(&mut self, _message: &str) -> Result<bool, anyhow::Error> {
        let mut confirms = self.confirms.lock().await;
        confirms.pop()
            .ok_or_else(|| anyhow::anyhow!("No confirmation queued"))
    }

    async fn select_option(&mut self, _message: &str, _options: &[String]) -> Result<usize, anyhow::Error> {
        let mut selections = self.selections.lock().await;
        selections.pop()
            .ok_or_else(|| anyhow::anyhow!("No selection queued"))
    }
}

/// Simple mock menu handler for testing
pub struct MockMenuHandler {
    selections: Arc<Mutex<Vec<MenuSelection>>>,
    dialogs: Arc<Mutex<Vec<Option<bool>>>>,
    inputs: Arc<Mutex<Vec<String>>>,
}

impl MockMenuHandler {
    pub fn new() -> Self {
        Self {
            selections: Arc::new(Mutex::new(Vec::new())),
            dialogs: Arc::new(Mutex::new(Vec::new())),
            inputs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_selection(&self, selection: MenuSelection) {
        let mut selections = self.selections.lock().await;
        selections.push(selection);
    }

    pub async fn add_dialog_result(&self, result: Option<bool>) {
        let mut dialogs = self.dialogs.lock().await;
        dialogs.push(result);
    }

    pub async fn add_input(&self, input: String) {
        let mut inputs = self.inputs.lock().await;
        inputs.push(input);
    }

    pub async fn clear(&self) {
        self.selections.lock().await.clear();
        self.dialogs.lock().await.clear();
        self.inputs.lock().await.clear();
    }
}

#[async_trait]
impl super::MenuHandler for MockMenuHandler {
    async fn show_main_menu(&mut self) -> Result<MenuSelection, anyhow::Error> {
        let mut selections = self.selections.lock().await;
        selections.pop()
            .ok_or_else(|| anyhow::anyhow!("No menu selection queued"))
    }

    async fn show_confirm_dialog(&mut self, _message: &str) -> Result<bool, anyhow::Error> {
        let mut dialogs = self.dialogs.lock().await;
        dialogs.pop()
            .flatten()
            .ok_or_else(|| anyhow::anyhow!("No dialog result queued"))
    }

    async fn show_input_dialog(&mut self, _message: &str) -> Result<String, anyhow::Error> {
        let mut inputs = self.inputs.lock().await;
        inputs.pop()
            .ok_or_else(|| anyhow::anyhow!("No dialog input queued"))
    }
}