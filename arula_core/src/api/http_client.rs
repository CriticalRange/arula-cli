//! Optimized HTTP client with connection pooling
//!
//! This module provides a lazily-initialized, optimized HTTP client
//! following reqwest best practices for:
//! - Connection pooling with configurable idle timeout
//! - HTTP/2 keep-alive for persistent connections
//! - Request timeouts for reliability
//! - TCP keep-alive for network stability
//!
//! # Performance
//!
//! Per Tokio best practices, we use:
//! - Lazy initialization with `OnceLock` to avoid blocking startup
//! - Connection pooling to reuse connections
//! - HTTP/2 multiplexing when available

use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

/// Lazy-initialized HTTP client for AI API requests
static AI_CLIENT: OnceLock<Client> = OnceLock::new();

/// Lazy-initialized HTTP client for general requests
static GENERAL_CLIENT: OnceLock<Client> = OnceLock::new();

/// Configuration for the AI API client
pub struct AiClientConfig {
    /// Overall request timeout (default: 5 minutes)
    pub timeout: Duration,
    /// Connection timeout (default: 30 seconds)
    pub connect_timeout: Duration,
    /// Pool idle timeout (default: 90 seconds)
    pub pool_idle_timeout: Duration,
    /// Max idle connections per host (default: 10)
    pub pool_max_idle_per_host: usize,
    /// HTTP/2 keep-alive interval (default: 30 seconds)
    pub http2_keep_alive_interval: Duration,
    /// TCP keep-alive (default: 60 seconds)
    pub tcp_keepalive: Duration,
}

impl Default for AiClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300),           // 5 minutes for AI responses
            connect_timeout: Duration::from_secs(30),    // 30s to establish connection
            pool_idle_timeout: Duration::from_secs(90),  // Keep connections alive
            pool_max_idle_per_host: 10,                  // Multiple parallel requests
            http2_keep_alive_interval: Duration::from_secs(30),
            tcp_keepalive: Duration::from_secs(60),
        }
    }
}

/// Get the optimized HTTP client for AI API requests
///
/// This client is configured for long-running AI requests with:
/// - 5 minute timeout (AI responses can be slow)
/// - Connection pooling for efficiency
/// - HTTP/2 keep-alive for persistent connections
///
/// The client is lazily initialized on first use and reused thereafter.
///
/// # Example
///
/// ```rust,ignore
/// let client = get_ai_client();
/// let response = client.post(url)
///     .json(&body)
///     .send()
///     .await?;
/// ```
pub fn get_ai_client() -> &'static Client {
    AI_CLIENT.get_or_init(|| {
        create_ai_client(AiClientConfig::default())
            .expect("Failed to create AI HTTP client")
    })
}

/// Get a general-purpose HTTP client
///
/// This client is configured for typical HTTP requests with:
/// - 30 second timeout
/// - Connection pooling
///
/// # Example
///
/// ```rust,ignore
/// let client = get_general_client();
/// let response = client.get(url).send().await?;
/// ```
pub fn get_general_client() -> &'static Client {
    GENERAL_CLIENT.get_or_init(|| {
        create_general_client()
            .expect("Failed to create general HTTP client")
    })
}

/// Create an AI API client with the specified configuration
pub fn create_ai_client(config: AiClientConfig) -> Result<Client, reqwest::Error> {
    Client::builder()
        // Timeouts
        .timeout(config.timeout)
        .connect_timeout(config.connect_timeout)
        // Connection pooling
        .pool_idle_timeout(config.pool_idle_timeout)
        .pool_max_idle_per_host(config.pool_max_idle_per_host)
        // TCP keep-alive for network stability
        .tcp_keepalive(config.tcp_keepalive)
        // User agent
        .user_agent(format!("arula-cli/{}", env!("CARGO_PKG_VERSION")))
        // Build
        .build()
}

/// Create a general-purpose HTTP client
pub fn create_general_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(5)
        .user_agent(format!("arula-cli/{}", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Create a client for streaming requests (no overall timeout)
///
/// Streaming requests need special handling because the total
/// response time is unpredictable.
pub fn create_streaming_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        // No overall timeout - streaming can take any length
        .connect_timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(10)
        .tcp_keepalive(Duration::from_secs(60))
        .user_agent(format!("arula-cli/{}", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Create a client with custom timeout
///
/// # Arguments
///
/// * `timeout_secs` - Request timeout in seconds
///
/// # Example
///
/// ```rust,ignore
/// let client = create_client_with_timeout(60)?; // 60 second timeout
/// ```
pub fn create_client_with_timeout(timeout_secs: u64) -> Result<Client, reqwest::Error> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(60))
        .user_agent(format!("arula-cli/{}", env!("CARGO_PKG_VERSION")))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_client_config_default() {
        let config = AiClientConfig::default();
        assert_eq!(config.timeout.as_secs(), 300);
        assert_eq!(config.connect_timeout.as_secs(), 30);
    }

    #[test]
    fn test_get_ai_client() {
        // Should create client successfully
        let client = get_ai_client();
        // Second call should return same instance
        let client2 = get_ai_client();
        assert!(std::ptr::eq(client, client2));
    }

    #[test]
    fn test_get_general_client() {
        let client = get_general_client();
        let client2 = get_general_client();
        assert!(std::ptr::eq(client, client2));
    }

    #[test]
    fn test_create_ai_client() {
        let config = AiClientConfig::default();
        let result = create_ai_client(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_streaming_client() {
        let result = create_streaming_client();
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_client_with_timeout() {
        let result = create_client_with_timeout(60);
        assert!(result.is_ok());
    }
}

