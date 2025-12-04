//! Centralized error handling for ARULA CLI
//!
//! This module provides a unified error handling approach using:
//! - `thiserror` for library-style errors with proper error types
//! - `anyhow` for application-level error handling with context
//!
//! # Usage
//!
//! ```rust
//! use arula_cli::utils::error::{ArulaError, ArulaResult, ResultExt};
//!
//! fn do_something() -> ArulaResult<()> {
//!     // Use .with_context() for adding context to errors
//!     some_operation().with_api_context("fetching models")?;
//!     Ok(())
//! }
//! ```

use thiserror::Error;

/// Core errors that can occur in ARULA
#[derive(Error, Debug)]
pub enum ArulaError {
    /// API-related errors
    #[error("API error: {0}")]
    Api(#[from] ApiError),

    /// Tool execution errors
    #[error("Tool execution failed: {tool_name}")]
    ToolExecution {
        tool_name: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Provider not configured
    #[error("Provider not configured: {0}")]
    ProviderNotConfigured(String),

    /// Network/HTTP errors
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Channel communication errors
    #[error("Channel error: receiver dropped")]
    ChannelClosed,

    /// Cancellation
    #[error("Operation cancelled")]
    Cancelled,

    /// Git state errors
    #[error("Git state error: {0}")]
    GitState(String),

    /// Conversation errors
    #[error("Conversation error: {0}")]
    Conversation(String),
}

/// API-specific errors with detailed information
#[derive(Error, Debug)]
pub enum ApiError {
    /// AI client not initialized
    #[error("AI client not initialized. Please configure AI settings using the /config command or application menu.")]
    NotInitialized,

    /// Rate limited by provider
    #[error("Rate limited: retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    /// Authentication failed
    #[error("Authentication failed: invalid or missing API key")]
    AuthenticationFailed,

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Request timeout
    #[error("Request timed out after {timeout_secs} seconds")]
    Timeout { timeout_secs: u64 },

    /// Server error with status code
    #[error("Server error ({status_code}): {message}")]
    ServerError { status_code: u16, message: String },

    /// Invalid response format
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    /// Streaming error
    #[error("Streaming error: {0}")]
    StreamingError(String),

    /// Provider-specific error
    #[error("{provider} error: {message}")]
    ProviderError { provider: String, message: String },
}

/// Tool-specific errors
#[derive(Error, Debug)]
pub enum ToolError {
    /// Tool not found in registry
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// Invalid parameters
    #[error("Invalid parameters for tool '{tool_name}': {message}")]
    InvalidParams { tool_name: String, message: String },

    /// Execution failed
    #[error("Tool '{tool_name}' execution failed: {message}")]
    ExecutionFailed { tool_name: String, message: String },

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Resource not found (file, directory, etc.)
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    /// MCP server error
    #[error("MCP server '{server}' error: {message}")]
    McpError { server: String, message: String },
}

impl From<ToolError> for ArulaError {
    fn from(err: ToolError) -> Self {
        ArulaError::ToolExecution {
            tool_name: match &err {
                ToolError::NotFound(name) => name.clone(),
                ToolError::InvalidParams { tool_name, .. } => tool_name.clone(),
                ToolError::ExecutionFailed { tool_name, .. } => tool_name.clone(),
                ToolError::PermissionDenied(_) => "unknown".to_string(),
                ToolError::ResourceNotFound(_) => "unknown".to_string(),
                ToolError::McpError { server, .. } => format!("mcp_{}", server),
            },
            source: Box::new(err),
        }
    }
}

/// Result type alias for ARULA operations
pub type ArulaResult<T> = anyhow::Result<T>;

/// Extension trait for adding ARULA-specific context to errors
pub trait ResultExt<T> {
    /// Add tool execution context to an error
    fn with_tool_context(self, tool_name: &str) -> ArulaResult<T>;

    /// Add API operation context to an error
    fn with_api_context(self, operation: &str) -> ArulaResult<T>;

    /// Add file operation context to an error
    fn with_file_context(self, path: &str) -> ArulaResult<T>;

    /// Add configuration context to an error
    fn with_config_context(self, setting: &str) -> ArulaResult<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ResultExt<T> for Result<T, E> {
    fn with_tool_context(self, tool_name: &str) -> ArulaResult<T> {
        use anyhow::Context;
        self.map_err(|e| anyhow::anyhow!(e))
            .with_context(|| format!("Failed executing tool: {}", tool_name))
    }

    fn with_api_context(self, operation: &str) -> ArulaResult<T> {
        use anyhow::Context;
        self.map_err(|e| anyhow::anyhow!(e))
            .with_context(|| format!("API operation failed: {}", operation))
    }

    fn with_file_context(self, path: &str) -> ArulaResult<T> {
        use anyhow::Context;
        self.map_err(|e| anyhow::anyhow!(e))
            .with_context(|| format!("File operation failed: {}", path))
    }

    fn with_config_context(self, setting: &str) -> ArulaResult<T> {
        use anyhow::Context;
        self.map_err(|e| anyhow::anyhow!(e))
            .with_context(|| format!("Configuration error for: {}", setting))
    }
}

/// Extension trait for Option types
pub trait OptionExt<T> {
    /// Convert Option to Result with tool context
    fn ok_or_tool_error(self, tool_name: &str, message: &str) -> ArulaResult<T>;

    /// Convert Option to Result with API context
    fn ok_or_api_error(self, message: &str) -> ArulaResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_tool_error(self, tool_name: &str, message: &str) -> ArulaResult<T> {
        self.ok_or_else(|| {
            anyhow::anyhow!(ToolError::ExecutionFailed {
                tool_name: tool_name.to_string(),
                message: message.to_string(),
            })
        })
    }

    fn ok_or_api_error(self, message: &str) -> ArulaResult<T> {
        self.ok_or_else(|| anyhow::anyhow!(ApiError::InvalidResponse(message.to_string())))
    }
}

/// Helper to create a tool execution error
pub fn tool_error(tool_name: impl Into<String>, message: impl Into<String>) -> ArulaError {
    ArulaError::ToolExecution {
        tool_name: tool_name.into(),
        source: Box::new(ToolError::ExecutionFailed {
            tool_name: String::new(),
            message: message.into(),
        }),
    }
}

/// Helper to create an API error
pub fn api_error(message: impl Into<String>) -> ApiError {
    ApiError::InvalidResponse(message.into())
}

/// Helper to create a provider-specific error
pub fn provider_error(provider: impl Into<String>, message: impl Into<String>) -> ApiError {
    ApiError::ProviderError {
        provider: provider.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arula_error_display() {
        let err = ArulaError::Config("invalid value".to_string());
        assert!(err.to_string().contains("Configuration error"));
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError::NotInitialized;
        assert!(err.to_string().contains("not initialized"));

        let err = ApiError::RateLimited { retry_after_secs: 60 };
        assert!(err.to_string().contains("60 seconds"));
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::NotFound("unknown_tool".to_string());
        assert!(err.to_string().contains("unknown_tool"));
    }

    #[test]
    fn test_result_ext_tool_context() {
        let result: Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));

        let with_context = result.with_tool_context("read_file");
        assert!(with_context.is_err());

        let err_string = format!("{:?}", with_context.unwrap_err());
        assert!(err_string.contains("read_file"));
    }

    #[test]
    fn test_option_ext() {
        let none: Option<i32> = None;
        let result = none.ok_or_tool_error("test_tool", "value was None");
        assert!(result.is_err());

        let some: Option<i32> = Some(42);
        let result = some.ok_or_tool_error("test_tool", "value was None");
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_error_conversion() {
        let tool_err = ToolError::NotFound("my_tool".to_string());
        let arula_err: ArulaError = tool_err.into();
        assert!(matches!(arula_err, ArulaError::ToolExecution { .. }));
    }
}

