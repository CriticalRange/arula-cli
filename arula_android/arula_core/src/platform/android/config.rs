//! Android configuration management using SharedPreferences

use crate::platform::android::AndroidContext;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Android configuration backend using SharedPreferences
pub struct AndroidConfig {
    ctx: AndroidContext,
    cache: Arc<RwLock<HashMap<String, String>>>,
}

impl AndroidConfig {
    pub fn new(ctx: AndroidContext) -> Self {
        Self {
            ctx,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load configuration from SharedPreferences
    pub async fn load(&self) -> Result<Config> {
        let env = self.ctx.get_env()?;
        let mut config = Config::default();

        // In a real implementation, this would use SharedPreferences
        // For now, we'll simulate with environment variables and defaults

        // Active provider
        if let Ok(provider) = std::env::var("ARULA_PROVIDER") {
            config.active_provider = provider;
        }

        // API keys from environment
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            config.providers.insert("openai".to_string(), ProviderConfig {
                api_key: Some(key),
                api_url: Some("https://api.openai.com/v1".to_string()),
                model: Some("gpt-4".to_string()),
                max_tokens: Some(4096),
                temperature: Some(0.7),
                ..Default::default()
            });
        }

        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            config.providers.insert("anthropic".to_string(), ProviderConfig {
                api_key: Some(key),
                api_url: Some("https://api.anthropic.com".to_string()),
                model: Some("claude-3-opus-20240229".to_string()),
                max_tokens: Some(4096),
                temperature: Some(0.7),
                ..Default::default()
            });
        }

        if let Ok(key) = std::env::var("ZAI_API_KEY") {
            config.providers.insert("zai".to_string(), ProviderConfig {
                api_key: Some(key),
                api_url: Some("https://z.ai/api".to_string()),
                model: Some("glm-4".to_string()),
                max_tokens: Some(4096),
                temperature: Some(0.7),
                ..Default::default()
            });
        }

        // Cache the configuration
        let json = serde_json::to_string(&config)?;
        let mut cache = self.cache.write().await;
        cache.insert("config".to_string(), json);

        Ok(config)
    }

    /// Save configuration to SharedPreferences
    pub async fn save(&self, config: &Config) -> Result<()> {
        let json = serde_json::to_string(config)?;

        // In a real implementation, this would save to SharedPreferences
        // For now, we'll just cache it
        let mut cache = self.cache.write().await;
        cache.insert("config".to_string(), json);

        log::info!("Configuration saved to Android SharedPreferences");
        Ok(())
    }

    /// Get a configuration value
    pub async fn get(&self, key: &str) -> Option<String> {
        let cache = self.cache.read().await;
        cache.get(key).cloned()
    }

    /// Set a configuration value
    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.insert(key.to_string(), value.to_string());

        // In a real implementation, persist to SharedPreferences
        log::debug!("Config set: {} = {}", key, value);
        Ok(())
    }

    /// Get API key for provider
    pub async fn get_api_key(&self, provider: &str) -> Option<String> {
        let env_key = format!("{}_API_KEY", provider.to_uppercase());
        std::env::var(&env_key).ok()
            .or_else(|| self.get(&format!("{}.api_key", provider)).await)
    }

    /// Get model for provider
    pub async fn get_model(&self, provider: &str) -> String {
        self.get(&format!("{}.model", provider)).await
            .unwrap_or_else(|| self.default_model(provider))
    }

    /// Get API URL for provider
    pub async fn get_api_url(&self, provider: &str) -> String {
        self.get(&format!("{}.api_url", provider)).await
            .unwrap_or_else(|| self.default_url(provider))
    }

    /// Get maximum tokens for provider
    pub async fn get_max_tokens(&self, provider: &str) -> u32 {
        self.get(&format!("{}.max_tokens", provider)).await
            .and_then(|s| s.parse().ok())
            .unwrap_or(4096)
    }

    /// Get temperature for provider
    pub async fn get_temperature(&self, provider: &str) -> f32 {
        self.get(&format!("{}.temperature", provider)).await
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.7)
    }

    /// Default model for provider
    fn default_model(&self, provider: &str) -> String {
        match provider {
            "openai" => "gpt-4",
            "anthropic" => "claude-3-opus-20240229",
            "zai" => "glm-4",
            "ollama" => "llama2",
            _ => "gpt-3.5-turbo",
        }.to_string()
    }

    /// Default URL for provider
    fn default_url(&self, provider: &str) -> String {
        match provider {
            "openai" => "https://api.openai.com/v1",
            "anthropic" => "https://api.anthropic.com",
            "zai" => "https://z.ai/api",
            "ollama" => "http://localhost:11434",
            _ => "https://api.openai.com/v1",
        }.to_string()
    }

    /// Clear all configuration
    pub async fn clear(&self) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.clear();

        // In a real implementation, clear SharedPreferences
        log::info!("Configuration cleared");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub active_provider: String,
    pub providers: HashMap<String, ProviderConfig>,
    pub ui: UiConfig,
    pub system: SystemConfig,
}

impl Default for Config {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert("openai".to_string(), ProviderConfig::default());
        providers.insert("anthropic".to_string(), ProviderConfig::default());

        Self {
            active_provider: "openai".to_string(),
            providers,
            ui: UiConfig::default(),
            system: SystemConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub api_url: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub timeout_seconds: Option<u64>,
    pub enable_streaming: Option<bool>,
    pub auto_execute_tools: Option<bool>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_url: None,
            model: None,
            max_tokens: Some(4096),
            temperature: Some(0.7),
            timeout_seconds: Some(30),
            enable_streaming: Some(true),
            auto_execute_tools: Some(true),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub font_size: u16,
    pub show_timestamps: bool,
    pub auto_scroll: bool,
    pub enable_notifications: bool,
    pub vibrate_on_tool: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            font_size: 14,
            show_timestamps: true,
            auto_scroll: true,
            enable_notifications: true,
            vibrate_on_tool: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub log_level: String,
    pub max_history: usize,
    pub auto_save: bool,
    pub export_format: String,
    pub enable_termux_api: bool,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            max_history: 1000,
            auto_save: true,
            export_format: "json".to_string(),
            enable_termux_api: true,
        }
    }
}