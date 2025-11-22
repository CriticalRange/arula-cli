//! Test utilities and helper functions for the ARULA CLI project
//!
//! This module provides common utilities for testing, including:
//! - Test data factories
//! - Helper functions for setup/teardown
//! - Common test patterns

#[cfg(test)]
pub mod factories {
    use crate::config::Config;
    use tempfile::TempDir;

    /// Test factory for creating test configurations
    pub struct ConfigFactory;

    impl ConfigFactory {
        /// Create a basic test configuration
        pub fn create_basic() -> Config {
            Config::new_for_test("test-provider", "test-model", "https://test.api.com", "test-key")
        }

        /// Create a configuration with custom values
        pub fn create_custom(provider: &str, model: &str, api_url: &str, api_key: &str) -> Config {
            Config::new_for_test(provider, model, api_url, api_key)
        }

        /// Create a test configuration and save it to a temporary directory
        pub fn create_with_file() -> (Config, TempDir) {
            let temp_dir = TempDir::new().unwrap();
            let config_path = temp_dir.path().join("config.yaml");
            let config = Self::create_basic();
            config.save_to_file(&config_path).unwrap();
            (config, temp_dir)
        }
    }

    /// Test factory for creating chat messages
    pub struct ChatFactory;

    impl ChatFactory {
        /// Create a basic user message
        pub fn create_user_message(content: &str) -> crate::chat::ChatMessage {
            crate::chat::ChatMessage::new_user_message(content)
        }

        /// Create a basic AI message
        pub fn create_ai_message(content: &str) -> crate::chat::ChatMessage {
            crate::chat::ChatMessage::new_arula_message(content)
        }

        /// Create a system message
        pub fn create_system_message(content: &str) -> crate::chat::ChatMessage {
            crate::chat::ChatMessage::new_system_message(content)
        }

        /// Create a tool call message
        pub fn create_tool_call(tool_name: &str, arguments: &str) -> crate::chat::ChatMessage {
            crate::chat::ChatMessage::new_tool_call(
                format!("Execute {}", tool_name),
                format!("{{\"name\": \"{}\", \"arguments\": \"{}\"}}", tool_name, arguments),
            )
        }
    }
}

#[cfg(test)]
pub mod file_utils {
    use std::fs;
    use tempfile::{TempDir, NamedTempFile};

    /// Test file utilities
    pub struct FileTestUtils;

    impl FileTestUtils {
        /// Create a temporary file with specified content
        pub fn create_temp_file_with_content(content: &str) -> (NamedTempFile, String) {
            let temp_file = NamedTempFile::new().unwrap();
            let path = temp_file.path().to_string_lossy().to_string();
            fs::write(&temp_file, content).unwrap();
            (temp_file, path)
        }

        /// Create a temporary directory with test files
        pub fn create_temp_dir_with_files(files: &[(&str, &str)]) -> TempDir {
            let temp_dir = TempDir::new().unwrap();

            for (filename, content) in files {
                let file_path = temp_dir.path().join(filename);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                fs::write(file_path, content).unwrap();
            }

            temp_dir
        }
    }
}

#[cfg(test)]
pub mod assertions {
    use crate::chat::ChatMessage;

    /// Assert that a message is a user message with specific content
    pub fn assert_user_message(message: &ChatMessage, expected_content: &str) {
        assert_eq!(message.message_type, crate::chat::MessageType::User);
        assert_eq!(message.content, expected_content);
    }

    /// Assert that a message is an AI message
    pub fn assert_ai_message(message: &ChatMessage, expected_content: &str) {
        assert_eq!(message.message_type, crate::chat::MessageType::Arula);
        assert_eq!(message.content, expected_content);
    }

    /// Assert that a message is a tool call
    pub fn assert_tool_call(message: &ChatMessage, expected_tool: &str) {
        assert_eq!(message.message_type, crate::chat::MessageType::ToolCall);
        assert!(message.tool_call_json.as_ref().unwrap().contains(expected_tool));
    }
}

#[cfg(test)]
pub mod performance {
    use std::time::{Duration, Instant};

    /// Measure execution time of a function
    pub fn measure_time<F, R>(f: F) -> (R, Duration)
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }

    /// Assert that execution time is within expected bounds
    pub fn assert_execution_time_within<F, R>(f: F, max_duration: Duration) -> R
    where
        F: FnOnce() -> R,
    {
        let (result, duration) = measure_time(f);
        assert!(
            duration <= max_duration,
            "Execution time {:?} exceeds expected maximum {:?}",
            duration,
            max_duration
        );
        result
    }
}

#[cfg(test)]
pub mod async_utils {
    use tokio::time::{timeout, Duration};

    /// Assert that an async operation completes within specified duration
    pub async fn assert_async_completes_within<F, T>(
        duration: Duration,
        future: F,
    ) -> Result<T, &'static str>
    where
        F: std::future::Future<Output = T>,
    {
        timeout(duration, future)
            .await
            .map_err(|_| "Operation timed out")
    }
}

#[cfg(test)]
pub mod generators {
    use fastrand;

    /// Generate a random string of specified length
    pub fn random_string(length: usize) -> String {
        (0..length).map(|_| fastrand::alphabetic()).collect()
    }

    /// Generate a random numeric string
    pub fn random_numeric_string(length: usize) -> String {
        (0..length).map(|_| fastrand::digit(10).to_string()).collect()
    }

    /// Generate random email
    pub fn random_email() -> String {
        format!("{}@{}.com", random_string(8), random_string(6))
    }

    /// Generate random URL
    pub fn random_url() -> String {
        format!("https://{}.com", random_string(10))
    }
}

#[cfg(test)]
mod test_utilities_tests {
    use super::factories::*;

    #[test]
    fn test_config_factory_basic() {
        let config = ConfigFactory::create_basic();
        assert_eq!(config.active_provider, "test-provider");
        assert_eq!(config.get_model(), "test-model");
        assert_eq!(config.get_api_url(), "https://test.api.com");
        assert_eq!(config.get_api_key(), "test-key");
    }

    #[test]
    fn test_config_factory_custom() {
        let config = ConfigFactory::create_custom("custom", "model-v2", "https://api.test.com", "custom-key");
        assert_eq!(config.active_provider, "custom");
        assert_eq!(config.get_model(), "model-v2");
        assert_eq!(config.get_api_url(), "https://api.test.com");
        assert_eq!(config.get_api_key(), "custom-key");
    }

    #[test]
    fn test_file_utils() {
        use super::file_utils::*;
        let (temp_file, path) = FileTestUtils::create_temp_file_with_content("test content");
        assert!(std::path::Path::new(&path).exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_generators() {
        use super::generators::*;
        let random_str = random_string(10);
        assert_eq!(random_str.len(), 10);

        let email = random_email();
        assert!(email.contains('@'));
        assert!(email.contains(".com"));
    }
}