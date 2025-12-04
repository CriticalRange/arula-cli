//! Integration tests for the configuration module

use arula_cli::config::{Config, ProviderConfig};
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

fn create_test_provider(model: &str, api_url: &str, api_key: &str) -> ProviderConfig {
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
    }
}

fn create_test_config() -> Config {
    let mut providers = HashMap::new();
    providers.insert(
        "anthropic".to_string(),
        create_test_provider(
            "claude-3-sonnet",
            "https://api.anthropic.com",
            "test-key-123",
        ),
    );

    Config {
        active_provider: "anthropic".to_string(),
        providers,
        mcp_servers: HashMap::new(),
        ai: None, // Legacy field, deprecated
    }
}

#[test]
fn test_config_full_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test_config.json");

    // Create a custom config
    let original_config = create_test_config();

    // Save the config
    original_config.save_to_file(&config_path)?;

    // Verify file exists and has content
    assert!(config_path.exists());
    let file_content = fs::read_to_string(&config_path)?;
    assert!(file_content.contains("anthropic"));
    assert!(file_content.contains("claude-3-sonnet"));

    // Load the config
    let loaded_config = Config::load_from_file(&config_path)?;

    // Verify loaded config matches original
    assert_eq!(
        loaded_config.active_provider,
        original_config.active_provider
    );
    let loaded_provider = loaded_config.providers.get("anthropic").unwrap();
    let original_provider = original_config.providers.get("anthropic").unwrap();
    assert_eq!(loaded_provider.model, original_provider.model);
    assert_eq!(loaded_provider.api_key, original_provider.api_key);

    Ok(())
}

#[test]
fn test_config_default() -> Result<(), Box<dyn std::error::Error>> {
    // Create default config
    let config = Config::default();

    // Default should have openai as active provider
    assert_eq!(config.active_provider, "openai");
    assert!(config.providers.contains_key("openai"));

    Ok(())
}

#[test]
fn test_config_serialization_formats() -> Result<(), Box<dyn std::error::Error>> {
    let config = create_test_config();

    // Test JSON serialization
    let json_str = serde_json::to_string(&config)?;
    let json_config: Config = serde_json::from_str(&json_str)?;
    assert_eq!(config.active_provider, json_config.active_provider);

    Ok(())
}

#[test]
fn test_config_multiple_providers() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("multi_provider_config.json");

    let mut providers = HashMap::new();
    providers.insert(
        "openai".to_string(),
        create_test_provider("gpt-4", "https://api.openai.com/v1", "sk-openai-key"),
    );
    providers.insert(
        "anthropic".to_string(),
        create_test_provider(
            "claude-3-opus",
            "https://api.anthropic.com",
            "sk-anthropic-key",
        ),
    );
    providers.insert(
        "ollama".to_string(),
        create_test_provider("llama2", "http://localhost:11434", ""),
    );

    let config = Config {
        active_provider: "openai".to_string(),
        providers,
        mcp_servers: HashMap::new(),
        ai: None,
    };

    config.save_to_file(&config_path)?;
    let loaded = Config::load_from_file(&config_path)?;

    assert_eq!(loaded.providers.len(), 3);
    assert!(loaded.providers.contains_key("openai"));
    assert!(loaded.providers.contains_key("anthropic"));
    assert!(loaded.providers.contains_key("ollama"));

    Ok(())
}

#[test]
fn test_config_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("edge_case_config.json");

    // Test with very long strings
    let long_string = "x".repeat(1000);
    let mut providers = HashMap::new();
    providers.insert(
        "test".to_string(),
        create_test_provider(&long_string, &long_string, &long_string),
    );

    let long_config = Config {
        active_provider: "test".to_string(),
        providers,
        mcp_servers: HashMap::new(),
        ai: None,
    };

    long_config.save_to_file(&config_path)?;
    let loaded_config = Config::load_from_file(&config_path)?;

    let loaded_provider = loaded_config.providers.get("test").unwrap();
    assert_eq!(loaded_provider.model.len(), 1000);

    // Test with special characters
    let mut special_providers = HashMap::new();
    special_providers.insert(
        "special".to_string(),
        create_test_provider(
            "model@v1.2.3",
            "https://api.test.com/v1/path?query=value",
            "sk-1234567890abcdef",
        ),
    );

    let special_config = Config {
        active_provider: "special".to_string(),
        providers: special_providers,
        mcp_servers: HashMap::new(),
        ai: None,
    };

    let special_path = temp_dir.path().join("special_config.json");
    special_config.save_to_file(&special_path)?;
    let loaded_special = Config::load_from_file(&special_path)?;

    let special_provider = loaded_special.providers.get("special").unwrap();
    assert_eq!(special_provider.model, "model@v1.2.3");

    Ok(())
}

#[test]
fn test_config_error_handling() {
    // Test loading from non-existent file
    let result = Config::load_from_file("/path/that/does/not/exist/config.json");
    assert!(result.is_err());

    // Test loading from invalid JSON
    let temp_dir = TempDir::new().unwrap();
    let invalid_path = temp_dir.path().join("invalid.json");
    fs::write(&invalid_path, "{ invalid json content [").unwrap();

    let result = Config::load_from_file(&invalid_path);
    assert!(result.is_err());
}

#[test]
fn test_config_provider_switching() -> Result<(), Box<dyn std::error::Error>> {
    let mut providers = HashMap::new();
    providers.insert(
        "openai".to_string(),
        create_test_provider("gpt-4", "https://api.openai.com/v1", "sk-openai"),
    );
    providers.insert(
        "anthropic".to_string(),
        create_test_provider("claude-3", "https://api.anthropic.com", "sk-anthropic"),
    );

    let mut config = Config {
        active_provider: "openai".to_string(),
        providers,
        mcp_servers: HashMap::new(),
        ai: None,
    };

    // Initially openai is active
    assert_eq!(config.active_provider, "openai");

    // Switch to anthropic
    config.active_provider = "anthropic".to_string();
    assert_eq!(config.active_provider, "anthropic");

    // Verify provider details
    let active = config.providers.get(&config.active_provider).unwrap();
    assert_eq!(active.model, "claude-3");

    Ok(())
}
