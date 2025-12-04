//! Testing infrastructure and mockable traits for the ARULA CLI
//!
//! This module provides trait abstractions that allow for comprehensive testing
//! by injecting mock implementations of external dependencies.

// Temporarily disabled due to outdated mockall syntax and API changes
// TODO: Update mocks to work with current Rust/mockall versions
// pub mod mocks;
// pub mod test_helpers;
// pub mod test_utils;

use async_trait::async_trait;
use std::path::PathBuf;
use arula_cli::chat::{EnhancedChatMessage, ChatRole};

/// Trait for terminal output operations
#[async_trait]
pub trait OutputHandler: Send + Sync {
    async fn print_message(&mut self, role: ChatRole, content: &str) -> std::io::Result<()>;
    async fn print_error(&mut self, error: &str) -> std::io::Result<()>;
    async fn start_ai_message(&mut self) -> std::io::Result<()>;
    async fn end_ai_message(&mut self) -> std::io::Result<()>;
    async fn print_streaming_chunk(&mut self, chunk: &str) -> std::io::Result<()>;
}

/// Trait for configuration management
#[async_trait]
pub trait ConfigManager: Send + Sync {
    async fn load_config(&self) -> Result<arula_cli::utils::config::Config, anyhow::Error>;
    async fn save_config(&self, config: &arula_cli::utils::config::Config) -> Result<(), anyhow::Error>;
    async fn get_default_endpoint(&self) -> Result<String, anyhow::Error>;
}

/// Trait for AI client operations
#[async_trait]
pub trait AiClient: Send + Sync {
    async fn send_message(&self, message: &str, history: &[EnhancedChatMessage]) -> Result<String, anyhow::Error>;
    async fn send_message_stream(&self, message: &str, history: &[EnhancedChatMessage]) -> Result<Box<dyn futures::Stream<Item = Result<String, anyhow::Error>> + Send + Unpin>, anyhow::Error>;
}

/// Trait for file system operations
#[async_trait]
pub trait FileSystem: Send + Sync {
    async fn read_file(&self, path: &PathBuf) -> Result<Vec<u8>, anyhow::Error>;
    async fn write_file(&self, path: &PathBuf, contents: &[u8]) -> Result<(), anyhow::Error>;
    async fn exists(&self, path: &PathBuf) -> bool;
    async fn create_dir_all(&self, path: &PathBuf) -> Result<(), anyhow::Error>;
    async fn list_directory(&self, path: &PathBuf) -> Result<Vec<PathBuf>, anyhow::Error>;
}

/// Trait for process execution
#[async_trait]
pub trait ProcessExecutor: Send + Sync {
    async fn execute_command(&self, command: &str, args: &[&str]) -> Result<std::process::Output, anyhow::Error>;
    async fn execute_command_with_input(&self, command: &str, args: &[&str], input: &[u8]) -> Result<std::process::Output, anyhow::Error>;
}

/// Trait for HTTP operations
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn post_json(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value, anyhow::Error>;
    async fn post_json_stream(&self, url: &str, body: &serde_json::Value) -> Result<Box<dyn futures::Stream<Item = Result<String, anyhow::Error>> + Send + Unpin>, anyhow::Error>;
    async fn get(&self, url: &str) -> Result<serde_json::Value, anyhow::Error>;
}

/// Trait for time operations
pub trait TimeProvider: Send + Sync {
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
    fn sleep(&self, duration: std::time::Duration) -> tokio::time::Sleep;
}

/// Trait for terminal input operations
#[async_trait]
pub trait InputHandler: Send + Sync {
    async fn read_line(&mut self) -> Result<String, anyhow::Error>;
    async fn read_password(&mut self) -> Result<String, anyhow::Error>;
    async fn confirm(&mut self, message: &str) -> Result<bool, anyhow::Error>;
    async fn select_option(&mut self, message: &str, options: &[String]) -> Result<usize, anyhow::Error>;
}

/// Trait for menu operations
#[async_trait]
pub trait MenuHandler: Send + Sync {
    async fn show_main_menu(&mut self) -> Result<MenuSelection, anyhow::Error>;
    async fn show_confirm_dialog(&mut self, message: &str) -> Result<bool, anyhow::Error>;
    async fn show_input_dialog(&mut self, message: &str) -> Result<String, anyhow::Error>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum MenuSelection {
    Continue,
    SaveHistory,
    ClearHistory,
    Settings,
    Help,
    Exit,
}

/// Collection of all dependencies for dependency injection
/// Note: Clone is not derived as trait objects cannot implement Clone
pub struct Dependencies {
    pub output: Box<dyn OutputHandler>,
    pub config: Box<dyn ConfigManager>,
    pub ai_client: Box<dyn AiClient>,
    pub filesystem: Box<dyn FileSystem>,
    pub process_executor: Box<dyn ProcessExecutor>,
    pub http_client: Box<dyn HttpClient>,
    pub time_provider: Box<dyn TimeProvider>,
    pub input_handler: Box<dyn InputHandler>,
    pub menu_handler: Box<dyn MenuHandler>,
}

// Temporarily disabled due to mock module being disabled
// impl Dependencies {
//     pub fn new() -> Self {
//         Self::default()
//     }
// }

// Temporarily disabled due to mock module being disabled
// impl Default for Dependencies {
//     fn default() -> Self {
//         use crate::output::OutputHandler as RealOutputHandler;
//         use crate::config::Config as RealConfigManager;
//         use crate::agent_client::AgentClient as RealAiClient;
//         use std::sync::Arc;

//         Self {
//             output: Box::new(RealOutputHandler::new()),
//             config: Box::new(RealConfigManager::default()),
//             ai_client: Box::new(RealAiClient::default()),
//             filesystem: Box::new(crate::testing::mocks::MockFileSystem::new()),
//             process_executor: Box::new(crate::testing::mocks::MockProcessExecutor::new()),
//             http_client: Box::new(crate::testing::mocks::MockHttpClient::new()),
//             time_provider: Box::new(crate::testing::mocks::MockTimeProvider::new()),
//             input_handler: Box::new(crate::testing::mocks::MockInputHandler::new()),
//             menu_handler: Box::new(crate::testing::mocks::MockMenuHandler::new()),
//         }
//     }
// }