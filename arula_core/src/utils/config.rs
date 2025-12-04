use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde_json;
use serde_yaml; // Only for migration

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Currently active provider
    pub active_provider: String,

    /// Provider-specific configurations
    pub providers: HashMap<String, ProviderConfig>,

    /// MCP server configurations
    #[serde(skip_serializing_if = "HashMap::is_empty", default = "HashMap::new")]
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Legacy field for backward compatibility (deprecated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    pub api_key: String,

    // Z.AI specific options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_usage_tracking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_enabled: Option<bool>,
    
    /// Enable streaming mode for API responses (default: true)
    /// When enabled, responses are displayed as they arrive
    /// When disabled, waits for complete response before displaying
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,
    
    /// Enable tools/function calling for Ollama (default: false)
    /// Some Ollama models support tool calling, but it may cause issues with others
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub url: String,
    #[serde(skip_serializing_if = "HashMap::is_empty", default = "HashMap::new")]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,
}

/// Legacy config structure for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
    pub api_url: String,
    pub api_key: String,
}

impl AiConfig {
    /// Get the default configuration for a specific provider
    pub fn get_provider_defaults(provider: &str) -> AiConfig {
        match provider.to_lowercase().as_str() {
            "z.ai coding plan" | "z.ai" | "zai" => AiConfig {
                provider: "z.ai coding plan".to_string(),
                model: "GLM-4.6".to_string(),
                api_url: "https://api.z.ai/api/coding/paas/v4".to_string(),
                api_key: std::env::var("ZAI_API_KEY").unwrap_or_default(),
            },
            "openai" => AiConfig {
                provider: "openai".to_string(),
                model: "gpt-3.5-turbo".to_string(),
                api_url: "https://api.openai.com/v1".to_string(),
                api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            },
            "anthropic" => AiConfig {
                provider: "anthropic".to_string(),
                model: "claude-3-sonnet-20240229".to_string(),
                api_url: "https://api.anthropic.com".to_string(),
                api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            },
            "ollama" => AiConfig {
                provider: "ollama".to_string(),
                model: "llama2".to_string(),
                api_url: "http://localhost:11434".to_string(),
                api_key: std::env::var("OLLAMA_API_KEY").unwrap_or_default(),
            },
            "openrouter" => AiConfig {
                provider: "openrouter".to_string(),
                model: "openai/gpt-4o".to_string(), // Popular default model
                api_url: "https://openrouter.ai/api/v1".to_string(),
                api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            },
            _ => AiConfig {
                provider: "custom".to_string(),
                model: "default".to_string(),
                api_url: "http://localhost:8080".to_string(),
                api_key: std::env::var("CUSTOM_API_KEY").unwrap_or_default(),
            },
        }
    }

    /// Apply provider defaults while preserving user customizations where appropriate
    pub fn apply_provider_defaults(&mut self, preserve_api_key: bool) {
        let defaults = Self::get_provider_defaults(&self.provider);

        // Always update provider
        self.provider = defaults.provider;

        // Update model if it was the default from previous provider or empty
        if self.model.is_empty() || self.model == "default" {
            self.model = defaults.model;
        }

        // Always update API URL (not user-editable for predefined providers)
        self.api_url = defaults.api_url;

        // Preserve API key if requested and it exists
        if !preserve_api_key || self.api_key.is_empty() {
            self.api_key = defaults.api_key;
        }
    }

    /// Check if a field is editable for the current provider
    pub fn is_field_editable(&self, field: ProviderField) -> bool {
        match self.provider.to_lowercase().as_str() {
            "custom" | "ollama" => true, // All fields editable for custom and ollama
            _ => match field {
                ProviderField::Model => true,  // Model always editable
                ProviderField::ApiKey => true, // API key always editable
                ProviderField::ApiUrl => false, // URL not editable for predefined providers
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderField {
    Model,
    ApiUrl,
    ApiKey,
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut config: Config = serde_json::from_str(&content)?;

        // Migrate legacy config if present
        config.migrate_legacy_config();

        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_config_path() -> String {
        // Use cross-platform home directory detection
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))  // Windows
            .unwrap_or_else(|_| ".".to_string());
        format!("{}/.arula/config.json", home)
    }

    pub fn load_or_default() -> Result<Self> {
        let config_path = Self::get_config_path();
        let config_file = Path::new(&config_path);
        // Use cross-platform home directory detection
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))  // Windows
            .unwrap_or_else(|_| ".".to_string());
        let old_yaml_path = format!("{}/.arula/config.yaml", home);

        // Try to load JSON config first
        if config_file.exists() {
            if let Ok(config) = Self::load_from_file(config_file) {
                return Ok(config);
            }
        }

        // Check for old YAML config and migrate it
        let old_yaml_file = Path::new(&old_yaml_path);
        if old_yaml_file.exists() {
            println!("🔄 Migrating config from YAML to JSON...");
            if let Ok(yaml_content) = fs::read_to_string(old_yaml_file) {
                // Try to parse as YAML and convert to JSON
                match serde_yaml::from_str::<Config>(&yaml_content) {
                    Ok(config) => {
                        // Save as JSON
                        config.save_to_file(&config_path)?;
                        println!("✅ Config migrated to JSON: {}", config_path);

                        // Remove old YAML file
                        let _ = fs::remove_file(old_yaml_file);
                        return Ok(config);
                    }
                    Err(e) => {
                        println!("❌ Failed to migrate YAML config: {}", e);
                    }
                }
            }
        }

        // Return default config if loading/migration fails
        Ok(Self::default())
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path();
        self.save_to_file(config_path)
    }

    /// Migrate legacy ai config to new providers structure
    fn migrate_legacy_config(&mut self) {
        if let Some(legacy) = self.ai.take() {
            // Add the legacy provider config to providers map
            let provider_config = ProviderConfig {
                model: legacy.model.clone(),
                api_url: Some(legacy.api_url.clone()),
                api_key: legacy.api_key.clone(),
                thinking_enabled: None,
                max_retries: None,
                timeout_seconds: None,
                enable_usage_tracking: None,
                web_search_enabled: None,
                streaming: None,
                tools_enabled: None,
            };

            self.providers.insert(legacy.provider.clone(), provider_config);
            self.active_provider = legacy.provider;
        }
    }

    /// Get the currently active provider configuration
    pub fn get_active_provider_config(&self) -> Option<&ProviderConfig> {
        self.providers.get(&self.active_provider)
    }

    /// Get mutable reference to active provider configuration
    pub fn get_active_provider_config_mut(&mut self) -> Option<&mut ProviderConfig> {
        self.providers.get_mut(&self.active_provider)
    }

    /// Switch to a different provider
    pub fn switch_provider(&mut self, provider_name: &str) -> Result<()> {
        // Create provider config if it doesn't exist
        if !self.providers.contains_key(provider_name) {
            let defaults = AiConfig::get_provider_defaults(provider_name);
            self.providers.insert(
                provider_name.to_string(),
                ProviderConfig {
                    model: defaults.model,
                    api_url: Some(defaults.api_url),
                    api_key: defaults.api_key,
                    thinking_enabled: None,
                    max_retries: Some(3),
                    timeout_seconds: Some(300),
                    enable_usage_tracking: Some(true),
                    web_search_enabled: Some(false),
                    streaming: None,
                    tools_enabled: None,
                },
            );
        }

        self.active_provider = provider_name.to_string();
        Ok(())
    }

    /// Get thinking mode setting for the active provider
    pub fn get_thinking_enabled(&self) -> Option<bool> {
        if let Some(config) = self.get_active_provider_config() {
            config.thinking_enabled
        } else {
            None
        }
    }

    /// Alias for backward compatibility
    pub fn get_zai_thinking_enabled(&self) -> Option<bool> {
        self.get_thinking_enabled()
    }

    /// Set thinking mode for the active provider
    pub fn set_thinking_enabled(&mut self, enabled: bool) -> Result<()> {
        if let Some(config) = self.get_active_provider_config_mut() {
            config.thinking_enabled = Some(enabled);
        }
        self.save_to_file(Self::get_config_path())?;
        Ok(())
    }

    /// Alias for backward compatibility
    pub fn set_zai_thinking_enabled(&mut self, enabled: bool) -> Result<()> {
        self.set_thinking_enabled(enabled)
    }

    /// Get Z.AI web search enabled setting
    pub fn get_zai_web_search_enabled(&self) -> Option<bool> {
        if let Some(config) = self.get_active_provider_config() {
            config.web_search_enabled
        } else {
            None
        }
    }

    /// Set Z.AI web search enabled
    pub fn set_zai_web_search_enabled(&mut self, enabled: bool) -> Result<()> {
        if let Some(config) = self.get_active_provider_config_mut() {
            config.web_search_enabled = Some(enabled);
        }
        self.save_to_file(Self::get_config_path())?;
        Ok(())
    }

    /// Get streaming mode setting for the active provider
    /// Returns true by default if not explicitly set (streaming is the default)
    pub fn get_streaming_enabled(&self) -> bool {
        if let Some(config) = self.get_active_provider_config() {
            config.streaming.unwrap_or(true)
        } else {
            true // Default to streaming enabled
        }
    }

    /// Set streaming mode for the active provider
    pub fn set_streaming_enabled(&mut self, enabled: bool) -> Result<()> {
        if let Some(config) = self.get_active_provider_config_mut() {
            config.streaming = Some(enabled);
        }
        self.save_to_file(Self::get_config_path())?;
        Ok(())
    }

    /// Get tools enabled setting for the active provider (primarily for Ollama)
    /// Returns false by default - tools are opt-in for Ollama
    pub fn get_tools_enabled(&self) -> bool {
        if let Some(config) = self.get_active_provider_config() {
            config.tools_enabled.unwrap_or(false)
        } else {
            false
        }
    }

    /// Set tools enabled for the active provider
    pub fn set_tools_enabled(&mut self, enabled: bool) -> Result<()> {
        if let Some(config) = self.get_active_provider_config_mut() {
            config.tools_enabled = Some(enabled);
        }
        self.save_to_file(Self::get_config_path())?;
        Ok(())
    }

    /// Get Z.AI usage tracking enabled setting
    pub fn get_zai_usage_tracking_enabled(&self) -> Option<bool> {
        if let Some(config) = self.get_active_provider_config() {
            config.enable_usage_tracking
        } else {
            None
        }
    }

    /// Get Z.AI max retries setting
    pub fn get_zai_max_retries(&self) -> u32 {
        if let Some(config) = self.get_active_provider_config() {
            config.max_retries.unwrap_or(3)
        } else {
            3
        }
    }

    /// Get Z.AI timeout seconds setting
    pub fn get_zai_timeout_seconds(&self) -> u64 {
        if let Some(config) = self.get_active_provider_config() {
            config.timeout_seconds.unwrap_or(300)
        } else {
            300
        }
    }

    /// Load configuration from environment variables
    pub fn load_from_env() -> Result<Self> {
        let api_key = std::env::var("ZAI_API_KEY")
            .or_else(|_| std::env::var("ZAI_CODING_PLAN_API_KEY"))
            .unwrap_or_default();

        let endpoint = std::env::var("ZAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.z.ai/api/paas/v4/".to_string());

        let model = std::env::var("ZAI_MODEL")
            .unwrap_or_else(|_| "GLM-4.6".to_string());

        let mut config = Self::default();
        config.active_provider = "z.ai coding plan".to_string();
        config.providers.insert("z.ai coding plan".to_string(), ProviderConfig {
            model,
            api_url: Some(endpoint),
            api_key,
            thinking_enabled: std::env::var("ZAI_THINKING_ENABLED")
                .ok()
                .and_then(|v| v.parse().ok()),
            max_retries: std::env::var("ZAI_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok()),
            timeout_seconds: std::env::var("ZAI_TIMEOUT_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok()),
            enable_usage_tracking: Some(std::env::var("ZAI_ENABLE_USAGE_TRACKING")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true)),
            web_search_enabled: Some(std::env::var("ZAI_ENABLE_WEB_SEARCH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false)),
            streaming: std::env::var("ARULA_STREAMING")
                .ok()
                .and_then(|v| v.parse().ok()),
            tools_enabled: None,
        });

        Ok(config)
    }

    /// Get the API URL for the current provider
    pub fn get_api_url(&self) -> String {
        if let Some(config) = self.get_active_provider_config() {
            if let Some(url) = &config.api_url {
                return url.clone();
            }
        }

        // Fallback to defaults
        AiConfig::get_provider_defaults(&self.active_provider).api_url
    }

    /// Get current model
    pub fn get_model(&self) -> String {
        self.get_active_provider_config()
            .map(|c| c.model.clone())
            .unwrap_or_else(|| "default".to_string())
    }

    /// Set model for current provider
    pub fn set_model(&mut self, model: &str) {
        if let Some(config) = self.get_active_provider_config_mut() {
            config.model = model.to_string();
        }
    }

    /// Get current API key
    pub fn get_api_key(&self) -> String {
        self.get_active_provider_config()
            .map(|c| c.api_key.clone())
            .unwrap_or_default()
    }

    /// Set API key for current provider
    pub fn set_api_key(&mut self, api_key: &str) {
        if let Some(config) = self.get_active_provider_config_mut() {
            config.api_key = api_key.to_string();
        }
    }

    /// Get list of all configured providers
    pub fn get_provider_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.providers.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get all configured MCP servers
    pub fn get_mcp_servers(&self) -> &HashMap<String, McpServerConfig> {
        &self.mcp_servers
    }

    /// Get specific MCP server configuration
    pub fn get_mcp_server(&self, server_id: &str) -> Option<&McpServerConfig> {
        self.mcp_servers.get(server_id)
    }

    /// Add or update an MCP server configuration
    pub fn set_mcp_server(&mut self, server_id: &str, config: McpServerConfig) {
        self.mcp_servers.insert(server_id.to_string(), config);
    }

    /// Remove an MCP server configuration
    pub fn remove_mcp_server(&mut self, server_id: &str) -> Option<McpServerConfig> {
        self.mcp_servers.remove(server_id)
    }

    /// Get list of all MCP server IDs
    pub fn get_mcp_server_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.mcp_servers.keys().cloned().collect();
        names.sort();
        names
    }

    /// Check if a field is editable for the current provider
    pub fn is_field_editable(&self, field: ProviderField) -> bool {
        match self.active_provider.to_lowercase().as_str() {
            "custom" | "ollama" => true, // All fields editable for custom and ollama
            _ => match field {
                ProviderField::Model => true,  // Model always editable
                ProviderField::ApiKey => true, // API key always editable
                ProviderField::ApiUrl => false, // URL not editable for predefined providers
            },
        }
    }

    /// Set API URL for current provider (only works for custom providers)
    pub fn set_api_url(&mut self, api_url: &str) {
        if let Some(config) = self.get_active_provider_config_mut() {
            config.api_url = Some(api_url.to_string());
        }
    }

    /// Add or update a custom provider
    pub fn add_custom_provider(&mut self, name: &str, model: &str, api_url: &str, api_key: &str) -> Result<()> {
        self.providers.insert(
            name.to_string(),
            ProviderConfig {
                model: model.to_string(),
                api_url: Some(api_url.to_string()),
                api_key: api_key.to_string(),
                thinking_enabled: None,
                max_retries: None,
                timeout_seconds: None,
                enable_usage_tracking: None,
                web_search_enabled: None,
                streaming: None,
                tools_enabled: None,
            },
        );
        Ok(())
    }

    pub fn default() -> Self {
        let mut providers = HashMap::new();

        // Initialize with OpenAI defaults
        let openai_defaults = AiConfig::get_provider_defaults("openai");
        providers.insert(
            "openai".to_string(),
            ProviderConfig {
                model: openai_defaults.model,
                api_url: Some(openai_defaults.api_url),
                api_key: openai_defaults.api_key,
                thinking_enabled: None,
                max_retries: None,
                timeout_seconds: None,
                enable_usage_tracking: None,
                web_search_enabled: None,
                streaming: None, // Defaults to true when not set
                tools_enabled: None,
            },
        );

        Self {
            active_provider: "openai".to_string(),
            providers,
            mcp_servers: HashMap::new(),
            ai: None,
        }
    }

    pub fn zai_default() -> Self {
        let mut providers = HashMap::new();

        // Initialize with Z.AI defaults
        let zai_defaults = AiConfig::get_provider_defaults("z.ai coding plan");
        providers.insert(
            "z.ai coding plan".to_string(),
            ProviderConfig {
                model: zai_defaults.model,
                api_url: Some(zai_defaults.api_url),
                api_key: zai_defaults.api_key,
                thinking_enabled: None,
                max_retries: None,
                timeout_seconds: None,
                enable_usage_tracking: None,
                web_search_enabled: None,
                streaming: None, // Defaults to true when not set
                tools_enabled: None,
            },
        );

        Self {
            active_provider: "z.ai coding plan".to_string(),
            providers,
            mcp_servers: HashMap::new(),
            ai: None,
        }
    }

    // Helper methods for testing
    pub fn new_for_test(provider: &str, model: &str, api_url: &str, api_key: &str) -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            provider.to_string(),
            ProviderConfig {
                model: model.to_string(),
                api_url: Some(api_url.to_string()),
                api_key: api_key.to_string(),
                thinking_enabled: None,
                max_retries: None,
                timeout_seconds: None,
                enable_usage_tracking: None,
                web_search_enabled: None,
                streaming: None,
                tools_enabled: None,
            },
        );

        Self {
            active_provider: provider.to_string(),
            providers,
            mcp_servers: HashMap::new(),
            ai: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{TempDir, NamedTempFile};

    #[test]
    fn test_config_default() {
        std::env::remove_var("OPENAI_API_KEY");
        let config = Config::default();

        assert_eq!(config.active_provider, "openai");
        assert_eq!(config.get_model(), "gpt-3.5-turbo");
        assert_eq!(config.get_api_url(), "https://api.openai.com/v1");
        assert_eq!(config.get_api_key(), "");
    }

    #[test]
    fn test_config_with_env_api_key() {
        std::env::set_var("OPENAI_API_KEY", "test-key-123");
        let config = Config::default();

        assert_eq!(config.get_api_key(), "test-key-123");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn test_config_new_for_test() {
        let config = Config::new_for_test("anthropic", "claude-3", "https://api.anthropic.com", "test-key");

        assert_eq!(config.active_provider, "anthropic");
        assert_eq!(config.get_model(), "claude-3");
        assert_eq!(config.get_api_url(), "https://api.anthropic.com");
        assert_eq!(config.get_api_key(), "test-key");
    }

    #[test]
    fn test_save_and_load_config() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("test_config.yaml");

        // Create a test config
        let original_config = Config::new_for_test(
            "test-provider",
            "test-model",
            "https://test.api.com",
            "test-api-key"
        );

        // Save the config
        original_config.save_to_file(&config_path)?;

        // Verify the file was created
        assert!(config_path.exists());

        // Load the config
        let loaded_config = Config::load_from_file(&config_path)?;

        // Verify the loaded config matches the original
        assert_eq!(loaded_config.active_provider, original_config.active_provider);
        assert_eq!(loaded_config.get_model(), original_config.get_model());
        assert_eq!(loaded_config.get_api_url(), original_config.get_api_url());
        assert_eq!(loaded_config.get_api_key(), original_config.get_api_key());

        Ok(())
    }

    #[test]
    fn test_save_creates_parent_directories() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let nested_path = temp_dir.path().join("nested").join("dir").join("config.yaml");

        // Ensure the nested directory doesn't exist
        assert!(!nested_path.parent().unwrap().exists());

        let config = Config::new_for_test("test", "test", "test", "test");
        config.save_to_file(&nested_path)?;

        // Verify the file and parent directories were created
        assert!(nested_path.exists());
        assert!(nested_path.parent().unwrap().exists());

        Ok(())
    }

    #[test]
    fn test_load_invalid_yaml() {
        let temp_file = NamedTempFile::new().unwrap();

        // Write invalid YAML
        fs::write(temp_file.path(), "invalid: yaml: content: [").unwrap();

        let result = Config::load_from_file(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let nonexistent_path = "/path/that/does/not/exist/config.yaml";
        let result = Config::load_from_file(nonexistent_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_config_path() {
        // Set a known HOME directory for testing
        std::env::set_var("HOME", "/test/home");
        let config_path = Config::get_config_path();

        assert_eq!(config_path, "/test/home/.arula/config.json");
        std::env::remove_var("HOME");
    }

    #[test]
    fn test_get_config_path_no_home() {
        // Remove HOME environment variable
        std::env::remove_var("HOME");
        let config_path = Config::get_config_path();

        // Should fall back to current directory
        assert_eq!(config_path, "./.arula/config.json");
    }

    #[test]
    fn test_load_or_default_existing_file() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create the .arula directory and config file as expected by get_config_path()
        let arula_dir = temp_dir.path().join(".arula");
        std::fs::create_dir_all(&arula_dir)?;
        let config_path = arula_dir.join("config.yaml");

        // Create a custom config file
        let test_config = Config::new_for_test("custom", "custom-model", "custom-url", "custom-key");
        test_config.save_to_file(&config_path)?;

        // Temporarily override HOME to point to our test directory
        std::env::set_var("HOME", temp_dir.path());

        // Load using load_or_default
        let loaded_config = Config::load_or_default()?;

        assert_eq!(loaded_config.active_provider, "custom");
        assert_eq!(loaded_config.get_model(), "custom-model");
        assert_eq!(loaded_config.get_api_url(), "custom-url");
        assert_eq!(loaded_config.get_api_key(), "custom-key");

        std::env::remove_var("HOME");
        Ok(())
    }

    #[test]
    fn test_load_or_default_no_file() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Set HOME to a directory without config file
        std::env::set_var("HOME", temp_dir.path());
        std::env::remove_var("OPENAI_API_KEY");

        let config = Config::load_or_default()?;

        // Should return default config
        assert_eq!(config.active_provider, "openai");
        assert_eq!(config.get_model(), "gpt-3.5-turbo");
        assert_eq!(config.get_api_url(), "https://api.openai.com/v1");
        assert_eq!(config.get_api_key(), "");

        std::env::remove_var("HOME");
        Ok(())
    }

    #[test]
    fn test_config_serialization_roundtrip() -> Result<()> {
        let original = Config::new_for_test(
            "provider-test",
            "model-test",
            "https://api-test.com",
            "key-test-123"
        );

        // Serialize to JSON
        let json = serde_json::to_string(&original)?;

        // Deserialize from JSON
        let deserialized: Config = serde_json::from_str(&json)?;

        assert_eq!(original.active_provider, deserialized.active_provider);
        assert_eq!(original.get_model(), deserialized.get_model());
        assert_eq!(original.get_api_url(), deserialized.get_api_url());
        assert_eq!(original.get_api_key(), deserialized.get_api_key());

        Ok(())
    }

    #[test]
    fn test_config_json_serialization() -> Result<()> {
        let config = Config::new_for_test(
            "json-provider",
            "json-model",
            "https://json.api.com",
            "json-key"
        );

        // Serialize to JSON
        let json = serde_json::to_string(&config)?;

        // Deserialize from JSON
        let deserialized: Config = serde_json::from_str(&json)?;

        assert_eq!(config.active_provider, deserialized.active_provider);
        assert_eq!(config.get_model(), deserialized.get_model());
        assert_eq!(config.get_api_url(), deserialized.get_api_url());
        assert_eq!(config.get_api_key(), deserialized.get_api_key());

        Ok(())
    }

    #[test]
    fn test_ai_config_methods() {
        let ai_config = AiConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-sonnet".to_string(),
            api_url: "https://api.anthropic.com".to_string(),
            api_key: "anthropic-key".to_string(),
        };

        assert_eq!(ai_config.provider, "anthropic");
        assert_eq!(ai_config.model, "claude-3-sonnet");
        assert_eq!(ai_config.api_url, "https://api.anthropic.com");
        assert_eq!(ai_config.api_key, "anthropic-key");
    }

    #[test]
    fn test_config_clone() {
        let config1 = Config::new_for_test("clone-test", "clone-model", "clone-url", "clone-key");
        let config2 = config1.clone();

        assert_eq!(config1.active_provider, config2.active_provider);
        assert_eq!(config1.get_model(), config2.get_model());
        assert_eq!(config1.get_api_url(), config2.get_api_url());
        assert_eq!(config1.get_api_key(), config2.get_api_key());

        // Ensure they're independent
        let config3 = Config::new_for_test("different", "different", "different", "different");
        assert_ne!(config1.active_provider, config3.active_provider);
    }

    #[test]
    fn test_provider_switching() -> Result<()> {
        let mut config = Config::default();

        // Initially on OpenAI
        assert_eq!(config.active_provider, "openai");

        // Switch to Anthropic
        config.switch_provider("anthropic")?;
        assert_eq!(config.active_provider, "anthropic");
        assert_eq!(config.get_model(), "claude-3-sonnet-20240229");

        // OpenAI config should be preserved
        config.switch_provider("openai")?;
        assert_eq!(config.active_provider, "openai");
        assert_eq!(config.get_model(), "gpt-3.5-turbo");

        Ok(())
    }

    #[test]
    fn test_provider_config_persistence() -> Result<()> {
        let mut config = Config::default();

        // Configure OpenAI
        config.set_model("gpt-4");
        config.set_api_key("openai-key-123");

        // Switch to Anthropic and configure
        config.switch_provider("anthropic")?;
        config.set_model("claude-3-opus");
        config.set_api_key("anthropic-key-456");

        // Switch back to OpenAI - config should be preserved
        config.switch_provider("openai")?;
        assert_eq!(config.get_model(), "gpt-4");
        assert_eq!(config.get_api_key(), "openai-key-123");

        // Switch back to Anthropic - config should be preserved
        config.switch_provider("anthropic")?;
        assert_eq!(config.get_model(), "claude-3-opus");
        assert_eq!(config.get_api_key(), "anthropic-key-456");

        Ok(())
    }

    #[test]
    fn test_legacy_config_migration() -> Result<()> {
        use tempfile::NamedTempFile;

        // Create a legacy config with ai field in JSON format
        let legacy_json = r#"{
    "active_provider": "openai",
    "providers": {},
    "ai": {
        "provider": "openai",
        "model": "gpt-4",
        "api_url": "https://api.openai.com/v1",
        "api_key": "legacy-key-123"
    }
}"#;

        let temp_file = NamedTempFile::new()?;
        fs::write(temp_file.path(), legacy_json)?;

        // Load the legacy config
        let config = Config::load_from_file(temp_file.path())?;

        // Verify migration worked - legacy ai field should be migrated to active provider
        assert_eq!(config.active_provider, "openai");

        // After migration, the active provider should have the migrated settings
        // Note: migrate_legacy_config() creates a new provider entry
        if let Some(provider) = config.providers.get("openai") {
            assert_eq!(provider.model, "gpt-4");
            assert_eq!(provider.api_key, "legacy-key-123");
        }

        // Verify the ai field is now None (migrated)
        assert!(config.ai.is_none());

        Ok(())
    }

    #[test]
    fn test_get_provider_names() {
        let mut config = Config::default();

        // Add multiple providers
        let _ = config.switch_provider("openai");
        let _ = config.switch_provider("anthropic");
        let _ = config.switch_provider("ollama");

        let providers = config.get_provider_names();

        // Should have all three providers
        assert!(providers.contains(&"openai".to_string()));
        assert!(providers.contains(&"anthropic".to_string()));
        assert!(providers.contains(&"ollama".to_string()));
        assert_eq!(providers.len(), 3);
    }

    #[test]
    fn test_multi_provider_save_load() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("multi_config.yaml");

        // Create config with multiple providers
        let mut config = Config::default();
        config.switch_provider("openai")?;
        config.set_model("gpt-4");
        config.set_api_key("openai-key");

        config.switch_provider("anthropic")?;
        config.set_model("claude-3-opus");
        config.set_api_key("anthropic-key");

        config.switch_provider("ollama")?;
        config.set_model("llama3");
        config.set_api_key("");

        // Set active provider back to OpenAI
        config.switch_provider("openai")?;

        // Save
        config.save_to_file(&config_path)?;

        // Load
        let loaded = Config::load_from_file(&config_path)?;

        // Verify active provider
        assert_eq!(loaded.active_provider, "openai");
        assert_eq!(loaded.get_model(), "gpt-4");

        // Verify all providers are present
        assert_eq!(loaded.get_provider_names().len(), 3);

        // Check each provider's settings
        let mut loaded_config = loaded;
        loaded_config.switch_provider("anthropic")?;
        assert_eq!(loaded_config.get_model(), "claude-3-opus");
        assert_eq!(loaded_config.get_api_key(), "anthropic-key");

        loaded_config.switch_provider("ollama")?;
        assert_eq!(loaded_config.get_model(), "llama3");

        Ok(())
    }

    #[test]
    fn test_config_debug_format() {
        let config = Config::new_for_test("debug", "debug-model", "debug-url", "debug-key");
        let debug_str = format!("{:?}", config);

        // Debug format should contain the struct name and field values
        assert!(debug_str.contains("Config"));
        assert!(debug_str.contains("debug"));
        assert!(debug_str.contains("debug-model"));
    }

    #[test]
    fn test_edge_cases() -> Result<()> {
        // Test with empty strings
        let empty_config = Config::new_for_test("", "", "", "");
        assert_eq!(empty_config.active_provider, "");
        assert_eq!(empty_config.get_model(), "");
        assert_eq!(empty_config.get_api_url(), "");
        assert_eq!(empty_config.get_api_key(), "");

        // Test with very long strings
        let long_string = "a".repeat(1000);
        let long_config = Config::new_for_test(&long_string, &long_string, &long_string, &long_string);
        assert_eq!(long_config.active_provider.len(), 1000);
        assert_eq!(long_config.get_model().len(), 1000);

        // Should be able to save and load the long config
        let temp_file = NamedTempFile::new().unwrap();
        long_config.save_to_file(temp_file.path())?;
        let loaded_config = Config::load_from_file(temp_file.path())?;
        assert_eq!(loaded_config.active_provider.len(), 1000);

        Ok(())
    }

    #[test]
    #[should_panic]
    fn test_load_invalid_yaml_with_expect_panics() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), "invalid: yaml: content: [").unwrap();

        // This should panic due to invalid YAML
        Config::load_from_file(temp_file.path()).expect("This should panic");
    }

    #[test]
    fn test_load_invalid_yaml_returns_error() {
        // Test that invalid YAML returns an error (doesn't panic)
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), "invalid: yaml: content: [").unwrap();

        let result = Config::load_from_file(temp_file.path());
        assert!(result.is_err());
    }
}
