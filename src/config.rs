use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ai: AiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
    pub api_url: String,
    pub api_key: String,
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_config_path() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/.arula/config.yaml", home)
    }

    pub fn load_or_default() -> Result<Self> {
        let config_path = Self::get_config_path();
        let config_file = Path::new(&config_path);

        // Try to load existing config
        if config_file.exists() {
            if let Ok(config) = Self::load_from_file(config_file) {
                return Ok(config);
            }
        }

        // Return default config if loading fails
        Ok(Self::default())
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path();
        self.save_to_file(config_path)
    }

    pub fn default() -> Self {
        Self {
            ai: AiConfig {
                provider: "openai".to_string(),
                model: "gpt-3.5-turbo".to_string(),
                api_url: "https://api.openai.com/v1".to_string(),
                api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            },
        }
    }

    // Helper methods for testing
    pub fn new_for_test(provider: &str, model: &str, api_url: &str, api_key: &str) -> Self {
        Self {
            ai: AiConfig {
                provider: provider.to_string(),
                model: model.to_string(),
                api_url: api_url.to_string(),
                api_key: api_key.to_string(),
            },
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

        assert_eq!(config.ai.provider, "openai");
        assert_eq!(config.ai.model, "gpt-3.5-turbo");
        assert_eq!(config.ai.api_url, "https://api.openai.com/v1");
        assert_eq!(config.ai.api_key, "");
    }

    #[test]
    fn test_config_with_env_api_key() {
        std::env::set_var("OPENAI_API_KEY", "test-key-123");
        let config = Config::default();

        assert_eq!(config.ai.api_key, "test-key-123");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn test_config_new_for_test() {
        let config = Config::new_for_test("anthropic", "claude-3", "https://api.anthropic.com", "test-key");

        assert_eq!(config.ai.provider, "anthropic");
        assert_eq!(config.ai.model, "claude-3");
        assert_eq!(config.ai.api_url, "https://api.anthropic.com");
        assert_eq!(config.ai.api_key, "test-key");
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
        assert_eq!(loaded_config.ai.provider, original_config.ai.provider);
        assert_eq!(loaded_config.ai.model, original_config.ai.model);
        assert_eq!(loaded_config.ai.api_url, original_config.ai.api_url);
        assert_eq!(loaded_config.ai.api_key, original_config.ai.api_key);

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

        assert_eq!(config_path, "/test/home/.arula/config.yaml");
        std::env::remove_var("HOME");
    }

    #[test]
    fn test_get_config_path_no_home() {
        // Remove HOME environment variable
        std::env::remove_var("HOME");
        let config_path = Config::get_config_path();

        // Should fall back to current directory
        assert_eq!(config_path, "./.arula/config.yaml");
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

        assert_eq!(loaded_config.ai.provider, "custom");
        assert_eq!(loaded_config.ai.model, "custom-model");
        assert_eq!(loaded_config.ai.api_url, "custom-url");
        assert_eq!(loaded_config.ai.api_key, "custom-key");

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
        assert_eq!(config.ai.provider, "openai");
        assert_eq!(config.ai.model, "gpt-3.5-turbo");
        assert_eq!(config.ai.api_url, "https://api.openai.com/v1");
        assert_eq!(config.ai.api_key, "");

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

        // Serialize to YAML
        let yaml = serde_yaml::to_string(&original)?;

        // Deserialize from YAML
        let deserialized: Config = serde_yaml::from_str(&yaml)?;

        assert_eq!(original.ai.provider, deserialized.ai.provider);
        assert_eq!(original.ai.model, deserialized.ai.model);
        assert_eq!(original.ai.api_url, deserialized.ai.api_url);
        assert_eq!(original.ai.api_key, deserialized.ai.api_key);

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

        assert_eq!(config.ai.provider, deserialized.ai.provider);
        assert_eq!(config.ai.model, deserialized.ai.model);
        assert_eq!(config.ai.api_url, deserialized.ai.api_url);
        assert_eq!(config.ai.api_key, deserialized.ai.api_key);

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

        assert_eq!(config1.ai.provider, config2.ai.provider);
        assert_eq!(config1.ai.model, config2.ai.model);
        assert_eq!(config1.ai.api_url, config2.ai.api_url);
        assert_eq!(config1.ai.api_key, config2.ai.api_key);

        // Ensure they're independent
        let config3 = Config::new_for_test("different", "different", "different", "different");
        assert_ne!(config1.ai.provider, config3.ai.provider);
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
        assert_eq!(empty_config.ai.provider, "");
        assert_eq!(empty_config.ai.model, "");
        assert_eq!(empty_config.ai.api_url, "");
        assert_eq!(empty_config.ai.api_key, "");

        // Test with very long strings
        let long_string = "a".repeat(1000);
        let long_config = Config::new_for_test(&long_string, &long_string, &long_string, &long_string);
        assert_eq!(long_config.ai.provider.len(), 1000);
        assert_eq!(long_config.ai.model.len(), 1000);

        // Should be able to save and load the long config
        let temp_file = NamedTempFile::new().unwrap();
        long_config.save_to_file(temp_file.path())?;
        let loaded_config = Config::load_from_file(temp_file.path())?;
        assert_eq!(loaded_config.ai.provider.len(), 1000);

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
