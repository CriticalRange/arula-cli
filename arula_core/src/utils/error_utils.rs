//! Utility functions for improved error reporting
//!
//! This module provides helper functions to create more informative error messages
//! that include context about what went wrong, where it happened, and why.

use crate::api::api::AIProvider;
use std::fmt;

/// Enhanced error context for API requests
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub url: Option<String>,
    pub provider: Option<AIProvider>,
    pub status_code: Option<u16>,
    pub response_body: Option<String>,
    pub underlying_error: Option<String>,
}

impl ErrorContext {
    pub fn new(operation: &str) -> Self {
        Self {
            operation: operation.to_string(),
            url: None,
            provider: None,
            status_code: None,
            response_body: None,
            underlying_error: None,
        }
    }

    pub fn with_url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    pub fn with_provider(mut self, provider: AIProvider) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }

    pub fn with_response_body(mut self, body: &str) -> Self {
        self.response_body = Some(body.to_string());
        self
    }

    pub fn with_underlying_error(mut self, error: &dyn std::error::Error) -> Self {
        self.underlying_error = Some(error.to_string());
        self
    }

    pub fn with_anyhow_error(mut self, error: &anyhow::Error) -> Self {
        self.underlying_error = Some(error.to_string());
        self
    }

    pub fn with_underlying_error_str(mut self, error: &str) -> Self {
        self.underlying_error = Some(error.to_string());
        self
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut msg = format!("{} failed", self.operation);

        if let Some(url) = &self.url {
            msg.push_str(&format!("\n  URL: {}", url));
        }

        if let Some(provider) = &self.provider {
            msg.push_str(&format!("\n  Provider: {:?}", provider));
        }

        if let Some(status) = self.status_code {
            msg.push_str(&format!("\n  Status: {}", status));
        }

        if let Some(body) = &self.response_body {
            // Truncate very long responses for readability
            let body_preview = if body.len() > 200 {
                format!("{}...", &body[..200])
            } else {
                body.clone()
            };
            msg.push_str(&format!("\n  Response: {}", body_preview));
        }

        if let Some(error) = &self.underlying_error {
            msg.push_str(&format!("\n  Cause: {}", error));
        }

        write!(f, "{}", msg)
    }
}

/// Create an informative error message for streaming operations
pub fn stream_error(context: ErrorContext) -> String {
    format!("Stream error: {}", context)
}

/// Create an informative error message for API operations
pub fn api_error(context: ErrorContext) -> String {
    format!("API error: {}", context)
}

/// Create an informative error message for network operations
pub fn network_error(context: ErrorContext) -> String {
    format!("Network error: {}", context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context_basic() {
        let ctx = ErrorContext::new("Test operation");
        assert!(ctx.to_string().contains("Test operation failed"));
    }

    #[test]
    fn test_error_context_with_details() {
        let ctx = ErrorContext::new("API request")
            .with_url("https://api.example.com/v1/chat")
            .with_provider(AIProvider::OpenAI)
            .with_status_code(401)
            .with_response_body("Invalid API key")
            .with_underlying_error_str("Connection timeout");

        let msg = ctx.to_string();
        assert!(msg.contains("API request failed"));
        assert!(msg.contains("URL: https://api.example.com/v1/chat"));
        assert!(msg.contains("Provider: OpenAI"));
        assert!(msg.contains("Status: 401"));
        assert!(msg.contains("Response: Invalid API key"));
        assert!(msg.contains("Cause: Connection timeout"));
    }
}
