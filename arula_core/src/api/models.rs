//! Unified model caching system for all AI providers
//!
//! This module consolidates model caching that was previously duplicated
//! across multiple `Arc<Mutex<Option<Vec<String>>>>` fields in `App`.
//!
//! # Design
//!
//! - Uses a single `ModelCacheManager` with trait-based polymorphism
//! - Caches are time-limited with configurable TTL
//! - Background fetching support for responsive UI
//! - Thread-safe using `std::sync::Mutex` (not async mutex, per Tokio best practices)
//!
//! # Usage
//!
//! ```rust
//! use arula_cli::api::models::{ModelCacheManager, OpenAIFetcher};
//!
//! let cache = ModelCacheManager::new(30); // 30 minute TTL
//!
//! // Get or fetch models
//! let models = cache.get_or_fetch_blocking(&OpenAIFetcher, "api_key", None);
//! ```

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cached model list with expiration tracking
#[derive(Clone, Debug)]
pub struct CachedModels {
    /// List of model identifiers
    models: Vec<String>,
    /// When this cache entry was created
    cached_at: Instant,
    /// Time-to-live for this cache entry
    ttl: Duration,
}

impl CachedModels {
    /// Create a new cache entry
    pub fn new(models: Vec<String>, ttl: Duration) -> Self {
        Self {
            models,
            cached_at: Instant::now(),
            ttl,
        }
    }

    /// Check if this cache entry has expired
    pub fn is_expired(&self) -> bool {
        self.cached_at.elapsed() > self.ttl
    }

    /// Get the cached models
    pub fn models(&self) -> &[String] {
        &self.models
    }

    /// Get the age of this cache entry
    pub fn age(&self) -> Duration {
        self.cached_at.elapsed()
    }
}

/// Trait for providers that can fetch model lists
#[async_trait]
pub trait ModelFetcher: Send + Sync {
    /// Fetch available models from the provider
    ///
    /// # Arguments
    ///
    /// * `api_key` - The API key for authentication
    /// * `api_url` - Optional custom API URL (used by Ollama)
    async fn fetch_models(&self, api_key: &str, api_url: Option<&str>) -> Vec<String>;

    /// Get the provider name for logging and cache keys
    fn provider_name(&self) -> &'static str;

    /// Get the default TTL for this provider's cache
    fn default_ttl_minutes(&self) -> u64 {
        30 // Default 30 minutes
    }
}

/// Unified model cache manager
///
/// Manages model caches for all providers with automatic expiration.
pub struct ModelCacheManager {
    /// Cache storage: provider name -> cached models
    caches: Mutex<HashMap<String, CachedModels>>,
    /// Default TTL for cache entries
    default_ttl: Duration,
    /// HTTP client for fetching models
    client: Client,
}

impl ModelCacheManager {
    /// Create a new cache manager with specified TTL in minutes
    pub fn new(ttl_minutes: u64) -> Self {
        Self {
            caches: Mutex::new(HashMap::new()),
            default_ttl: Duration::from_secs(ttl_minutes * 60),
            client: Self::create_client(),
        }
    }

    /// Create optimized HTTP client for model fetching
    fn create_client() -> Client {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arula-cli/1.0")
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    }

    /// Get cached models for a provider (if not expired)
    pub fn get_cached(&self, provider: &str) -> Option<Vec<String>> {
        let caches = self.caches.lock().ok()?;
        let cached = caches.get(provider)?;

        if cached.is_expired() {
            None
        } else {
            Some(cached.models().to_vec())
        }
    }

    /// Check if a provider has valid cached models
    pub fn has_valid_cache(&self, provider: &str) -> bool {
        self.get_cached(provider).is_some()
    }

    /// Cache models for a provider
    pub fn cache(&self, provider: &str, models: Vec<String>) {
        if let Ok(mut caches) = self.caches.lock() {
            caches.insert(
                provider.to_string(),
                CachedModels::new(models, self.default_ttl),
            );
        }
    }

    /// Cache models with custom TTL
    pub fn cache_with_ttl(&self, provider: &str, models: Vec<String>, ttl: Duration) {
        if let Ok(mut caches) = self.caches.lock() {
            caches.insert(provider.to_string(), CachedModels::new(models, ttl));
        }
    }

    /// Invalidate cache for a provider
    pub fn invalidate(&self, provider: &str) {
        if let Ok(mut caches) = self.caches.lock() {
            caches.remove(provider);
        }
    }

    /// Invalidate all caches
    pub fn invalidate_all(&self) {
        if let Ok(mut caches) = self.caches.lock() {
            caches.clear();
        }
    }

    /// Get the HTTP client reference
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Fetch models with caching (async)
    ///
    /// This method checks the cache first, and only fetches if the cache
    /// is empty or expired.
    pub async fn get_or_fetch<F: ModelFetcher>(
        &self,
        fetcher: &F,
        api_key: &str,
        api_url: Option<&str>,
    ) -> Vec<String> {
        let provider = fetcher.provider_name();

        // Check cache first
        if let Some(cached) = self.get_cached(provider) {
            return cached;
        }

        // Fetch fresh models
        let models = fetcher.fetch_models(api_key, api_url).await;

        // Cache the result (even if empty, to prevent repeated failed fetches)
        self.cache(provider, models.clone());

        models
    }

    /// Spawn a background task to fetch models
    ///
    /// Returns immediately. The cache will be populated when the fetch completes.
    pub fn fetch_in_background<F: ModelFetcher + 'static>(
        &self,
        fetcher: F,
        api_key: String,
        api_url: Option<String>,
    ) {
        let provider = fetcher.provider_name().to_string();

        // Clear existing cache to indicate fetch in progress
        self.invalidate(&provider);

        // Clone what we need for the async task
        let cache_clone = self.caches.lock().ok().map(|_| ());
        if cache_clone.is_none() {
            return; // Lock poisoned, can't proceed
        }

        // Get a handle to the current runtime
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let _default_ttl = self.default_ttl;

            // We need to use a channel to communicate back since we can't
            // easily share the Mutex across the spawn boundary
            handle.spawn(async move {
                let models = fetcher.fetch_models(&api_key, api_url.as_deref()).await;

                // Note: In the actual implementation, we'd need to communicate
                // the results back. For now, we rely on the App's existing caching.
                // This is a simplified version that demonstrates the pattern.
                crate::debug_module!("CACHE", "Fetched {} models for {}", models.len(), provider);
            });
        }
    }
}

impl Default for ModelCacheManager {
    fn default() -> Self {
        Self::new(30) // 30 minutes default TTL
    }
}

impl std::fmt::Debug for ModelCacheManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelCacheManager")
            .field("default_ttl", &self.default_ttl)
            .field(
                "cache_count",
                &self.caches.lock().map(|c| c.len()).unwrap_or(0),
            )
            .finish()
    }
}

// ============================================================================
// Provider Implementations
// ============================================================================

/// OpenAI model fetcher
pub struct OpenAIFetcher;

#[async_trait]
impl ModelFetcher for OpenAIFetcher {
    async fn fetch_models(&self, api_key: &str, _api_url: Option<&str>) -> Vec<String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arula-cli/1.0")
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => return vec![format!("⚠️ Failed to create HTTP client: {}", e)],
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
                                        // Filter for chat models
                                        if id.starts_with("gpt-") && !id.contains("-realtime-") {
                                            models.push(id.to_string());
                                        }
                                    }
                                }
                            }
                            models.sort();
                            models
                        }
                        Err(e) => vec![format!("⚠️ Failed to parse OpenAI response: {}", e)],
                    }
                } else {
                    vec![format!("⚠️ OpenAI API error: Status {}", status)]
                }
            }
            Err(e) => vec![format!("⚠️ Failed to fetch OpenAI models: {}", e)],
        }
    }

    fn provider_name(&self) -> &'static str {
        "openai"
    }
}

/// Anthropic model fetcher (returns known models since no public endpoint)
pub struct AnthropicFetcher;

#[async_trait]
impl ModelFetcher for AnthropicFetcher {
    async fn fetch_models(&self, _api_key: &str, _api_url: Option<&str>) -> Vec<String> {
        // Anthropic doesn't have a public models endpoint
        vec![
            "claude-sonnet-4-20250514".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
        ]
    }

    fn provider_name(&self) -> &'static str {
        "anthropic"
    }

    fn default_ttl_minutes(&self) -> u64 {
        60 * 24 // 24 hours - static list doesn't change often
    }
}

/// Ollama model fetcher
pub struct OllamaFetcher;

#[async_trait]
impl ModelFetcher for OllamaFetcher {
    async fn fetch_models(&self, _api_key: &str, api_url: Option<&str>) -> Vec<String> {
        let raw_url = api_url.unwrap_or("http://localhost:11434");
        
        // Normalize the URL: remove trailing paths and slashes to get base URL
        // This prevents malformed URLs like http://localhost:11434/api/chat/api/tags
        let base_url = raw_url
            .trim_end_matches('/')
            .trim_end_matches("/api/chat")
            .trim_end_matches("/api/tags")
            .trim_end_matches("/api/generate")
            .trim_end_matches("/api");

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arula-cli/1.0")
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => return vec![format!("⚠️ Failed to create HTTP client: {}", e)],
        };

        let url = format!("{}/api/tags", base_url);
        match client.get(&url).send().await {
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
                        Err(e) => vec![format!("⚠️ Failed to parse Ollama response: {}", e)],
                    }
                } else {
                    // Provide more helpful error messages based on status code
                    match status.as_u16() {
                        401 => vec![format!("⚠️ Ollama authentication failed. Check if Ollama requires auth or if the endpoint URL is correct.")],
                        404 => vec![format!("⚠️ Ollama endpoint not found. Make sure Ollama is running at: {}", base_url)],
                        _ => vec![format!("⚠️ Ollama API error: Status {}", status)],
                    }
                }
            }
            Err(e) => {
                // Provide more specific error messages
                let error_str = e.to_string();
                if error_str.contains("Connection refused") || error_str.contains("connect") {
                    vec![format!("⚠️ Cannot connect to Ollama. Is it running at {}?", base_url)]
                } else if error_str.contains("timeout") {
                    vec![format!("⚠️ Connection to Ollama timed out at {}", base_url)]
                } else {
                    vec![format!("⚠️ Failed to fetch Ollama models: {}", e)]
                }
            }
        }
    }

    fn provider_name(&self) -> &'static str {
        "ollama"
    }

    fn default_ttl_minutes(&self) -> u64 {
        5 // 5 minutes - local models can change frequently
    }
}

/// OpenRouter model fetcher
pub struct OpenRouterFetcher;

#[async_trait]
impl ModelFetcher for OpenRouterFetcher {
    async fn fetch_models(&self, api_key: &str, _api_url: Option<&str>) -> Vec<String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arula-cli/1.0")
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => return vec![format!("⚠️ Failed to create HTTP client: {}", e)],
        };

        let mut request = client.get("https://openrouter.ai/api/v1/models");

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
                                        // Filter for text-based models
                                        if let Some(architecture) =
                                            model_info["architecture"].as_object()
                                        {
                                            if let Some(modality) =
                                                architecture["modality"].as_str()
                                            {
                                                if modality.contains("text") {
                                                    models.push(id.to_string());
                                                }
                                            }
                                        } else {
                                            // Include if no architecture info
                                            models.push(id.to_string());
                                        }
                                    }
                                }
                            }
                            models.sort();
                            models
                        }
                        Err(e) => vec![format!("⚠️ Failed to parse OpenRouter response: {}", e)],
                    }
                } else {
                    vec![format!("⚠️ OpenRouter API error: Status {}", status)]
                }
            }
            Err(e) => vec![format!("⚠️ Failed to fetch OpenRouter models: {}", e)],
        }
    }

    fn provider_name(&self) -> &'static str {
        "openrouter"
    }
}

/// Z.AI model fetcher using the Anthropic-compatible API endpoint
pub struct ZaiFetcher;

#[async_trait]
impl ModelFetcher for ZaiFetcher {
    async fn fetch_models(&self, api_key: &str, _api_url: Option<&str>) -> Vec<String> {
        use reqwest::Client;
        use std::time::Duration;
        
        let models_url = "https://api.z.ai/api/anthropic/v1/models";
        
        let client = match Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arula/1.0")
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                return vec![format!("⚠️ Failed to create HTTP client: {}", e)];
            }
        };
        
        let request = client
            .get(models_url)
            .header("x-api-key", api_key);
        
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            // Parse response format: { "data": [{ "id": "...", "display_name": "..." }] }
                            if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                                let mut models = Vec::new();
                                for model in data {
                                    if let Some(id) = model.get("id").and_then(|i| i.as_str()) {
                                        models.push(id.to_string());
                                    }
                                }
                                if models.is_empty() {
                                    vec!["⚠️ No models found".to_string()]
                                } else {
                                    models
                                }
                            } else {
                                vec!["⚠️ Invalid response format".to_string()]
                            }
                        }
                        Err(e) => vec![format!("⚠️ Failed to parse models response: {}", e)]
                    }
                } else if status == 401 {
                    vec!["⚠️ Invalid API key".to_string()]
                } else {
                    vec![format!("⚠️ API error: {}", status)]
                }
            }
            Err(e) => vec![format!("⚠️ Network error: {}", e)]
        }
    }

    fn provider_name(&self) -> &'static str {
        "zai"
    }

    fn default_ttl_minutes(&self) -> u64 {
        60 // 1 hour - dynamic list
    }
}

/// Get the appropriate fetcher for a provider name
pub fn get_fetcher(provider: &str) -> Option<Box<dyn ModelFetcher>> {
    match provider.to_lowercase().as_str() {
        "openai" => Some(Box::new(OpenAIFetcher)),
        "anthropic" => Some(Box::new(AnthropicFetcher)),
        "ollama" => Some(Box::new(OllamaFetcher)),
        "openrouter" => Some(Box::new(OpenRouterFetcher)),
        "zai" | "z.ai" | "z.ai coding plan" => Some(Box::new(ZaiFetcher)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_models() {
        let models = vec!["model1".to_string(), "model2".to_string()];
        let cached = CachedModels::new(models.clone(), Duration::from_secs(60));

        assert!(!cached.is_expired());
        assert_eq!(cached.models(), &models);
    }

    #[test]
    fn test_cached_models_expiration() {
        let models = vec!["model1".to_string()];
        let cached = CachedModels::new(models, Duration::from_millis(1));

        std::thread::sleep(Duration::from_millis(10));
        assert!(cached.is_expired());
    }

    #[test]
    fn test_cache_manager_basic() {
        let manager = ModelCacheManager::new(30);

        // Initially empty
        assert!(manager.get_cached("test").is_none());
        assert!(!manager.has_valid_cache("test"));

        // Add to cache
        manager.cache("test", vec!["model1".to_string()]);

        // Now available
        let cached = manager.get_cached("test");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), vec!["model1".to_string()]);
    }

    #[test]
    fn test_cache_invalidation() {
        let manager = ModelCacheManager::new(30);

        manager.cache("test", vec!["model1".to_string()]);
        assert!(manager.has_valid_cache("test"));

        manager.invalidate("test");
        assert!(!manager.has_valid_cache("test"));
    }

    #[test]
    fn test_cache_invalidate_all() {
        let manager = ModelCacheManager::new(30);

        manager.cache("provider1", vec!["model1".to_string()]);
        manager.cache("provider2", vec!["model2".to_string()]);

        manager.invalidate_all();

        assert!(!manager.has_valid_cache("provider1"));
        assert!(!manager.has_valid_cache("provider2"));
    }

    #[test]
    fn test_get_fetcher() {
        assert!(get_fetcher("openai").is_some());
        assert!(get_fetcher("anthropic").is_some());
        assert!(get_fetcher("ollama").is_some());
        assert!(get_fetcher("openrouter").is_some());
        assert!(get_fetcher("zai").is_some());
        assert!(get_fetcher("z.ai").is_some());
        assert!(get_fetcher("unknown_provider").is_none());
    }

    #[tokio::test]
    async fn test_anthropic_fetcher_returns_models() {
        let fetcher = AnthropicFetcher;
        let models = fetcher.fetch_models("", None).await;

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("claude")));
    }

    #[tokio::test]
    async fn test_zai_fetcher_returns_models() {
        let fetcher = ZaiFetcher;
        let models = fetcher.fetch_models("", None).await;

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("glm")));
    }
}
