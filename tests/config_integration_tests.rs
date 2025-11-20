//! Integration tests for the configuration module

use arula_cli::config::{Config, AiConfig};
use std::fs;
use tempfile::TempDir;
use std::env;

#[test]
fn test_config_full_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test_config.yaml");

    // Create a custom config
    let original_config = Config {
        ai: AiConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-sonnet".to_string(),
            api_url: "https://api.anthropic.com".to_string(),
            api_key: "test-key-123".to_string(),
        },
    };

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
    assert_eq!(loaded_config.ai.provider, original_config.ai.provider);
    assert_eq!(loaded_config.ai.model, original_config.ai.model);
    assert_eq!(loaded_config.ai.api_url, original_config.ai.api_url);
    assert_eq!(loaded_config.ai.api_key, original_config.ai.api_key);

    Ok(())
}

#[test]
fn test_config_environment_variables() -> Result<(), Box<dyn std::error::Error>> {
    // Set environment variable
    env::set_var("OPENAI_API_KEY", "env-test-key-456");

    // Create default config (should use environment variable)
    let config = Config::default();

    assert_eq!(config.ai.provider, "openai");
    assert_eq!(config.ai.model, "gpt-3.5-turbo");
    assert_eq!(config.ai.api_url, "https://api.openai.com/v1");
    assert_eq!(config.ai.api_key, "env-test-key-456");

    // Clean up
    env::remove_var("OPENAI_API_KEY");

    Ok(())
}

#[test]
fn test_config_load_or_default_flow() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;

    // Override HOME environment variable
    let original_home = env::var("HOME").ok();
    env::set_var("HOME", temp_dir.path());

    // Test load_or_default when no config file exists
    env::remove_var("OPENAI_API_KEY");
    let config = Config::load_or_default()?;
    assert_eq!(config.ai.provider, "openai");
    assert_eq!(config.ai.api_key, "");

    // Create a config file in the correct location (.arula/config.yaml)
    let arula_dir = temp_dir.path().join(".arula");
    std::fs::create_dir_all(&arula_dir)?;
    let config_path = arula_dir.join("config.yaml");

    let custom_config = Config {
        ai: AiConfig {
            provider: "custom".to_string(),
            model: "custom-model".to_string(),
            api_url: "https://custom.api.com".to_string(),
            api_key: "custom-key".to_string(),
        },
    };
    custom_config.save_to_file(&config_path)?;

    let loaded_config = Config::load_or_default()?;
    assert_eq!(loaded_config.ai.provider, "custom");
    assert_eq!(loaded_config.ai.model, "custom-model");

    // Restore original HOME
    match original_home {
        Some(home) => env::set_var("HOME", home),
        None => env::remove_var("HOME"),
    }

    Ok(())
}

#[test]
fn test_config_serialization_formats() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config {
        ai: AiConfig {
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            api_url: "https://test.api.com".to_string(),
            api_key: "test-key".to_string(),
        },
    };

    // Test JSON serialization
    let json_str = serde_json::to_string(&config)?;
    let json_config: Config = serde_json::from_str(&json_str)?;
    assert_eq!(config.ai.provider, json_config.ai.provider);

    // Test YAML serialization
    let yaml_str = serde_yaml::to_string(&config)?;
    let yaml_config: Config = serde_yaml::from_str(&yaml_str)?;
    assert_eq!(config.ai.provider, yaml_config.ai.provider);

    Ok(())
}

#[test]
fn test_config_validation() -> Result<(), Box<dyn std::error::Error>> {
    // Test config with missing required fields (though current Config doesn't validate)
    let incomplete_config = Config {
        ai: AiConfig {
            provider: "".to_string(), // Empty provider
            model: "test-model".to_string(),
            api_url: "https://test.api.com".to_string(),
            api_key: "test-key".to_string(),
        },
    };

    // Should still serialize/deserialize
    let serialized = serde_json::to_string(&incomplete_config)?;
    let deserialized: Config = serde_json::from_str(&serialized)?;
    assert_eq!(deserialized.ai.provider, "");

    Ok(())
}

#[test]
fn test_config_concurrent_access() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("concurrent_config.yaml");

    let config = Config {
        ai: AiConfig {
            provider: "concurrent".to_string(),
            model: "test-model".to_string(),
            api_url: "https://test.api.com".to_string(),
            api_key: "test-key".to_string(),
        },
    };

    // Test multiple save operations
    config.save_to_file(&config_path)?;
    config.save_to_file(&config_path)?;

    // Test multiple load operations
    let loaded1 = Config::load_from_file(&config_path)?;
    let loaded2 = Config::load_from_file(&config_path)?;

    assert_eq!(loaded1.ai.provider, loaded2.ai.provider);

    Ok(())
}

#[test]
fn test_config_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("edge_case_config.yaml");

    // Test with very long strings
    let long_string = "x".repeat(1000);
    let long_config = Config {
        ai: AiConfig {
            provider: long_string.clone(),
            model: long_string.clone(),
            api_url: long_string.clone(),
            api_key: long_string.clone(),
        },
    };

    long_config.save_to_file(&config_path)?;
    let loaded_config = Config::load_from_file(&config_path)?;

    assert_eq!(loaded_config.ai.provider.len(), 1000);
    assert_eq!(loaded_config.ai.model.len(), 1000);

    // Test with special characters
    let special_config = Config {
        ai: AiConfig {
            provider: "test-🚀-provider".to_string(),
            model: "model@v1.2.3".to_string(),
            api_url: "https://api.test.com/v1/path?query=value".to_string(),
            api_key: "sk-1234567890abcdef!@#$%^&*()".to_string(),
        },
    };

    let special_path = temp_dir.path().join("special_config.yaml");
    special_config.save_to_file(&special_path)?;
    let loaded_special = Config::load_from_file(&special_path)?;

    assert_eq!(loaded_special.ai.provider, "test-🚀-provider");
    assert_eq!(loaded_special.ai.model, "model@v1.2.3");

    Ok(())
}

#[test]
fn test_config_error_handling() {
    // Test loading from non-existent file
    let result = Config::load_from_file("/path/that/does/not/exist/config.yaml");
    assert!(result.is_err());

    // Test loading from invalid YAML
    let temp_dir = TempDir::new().unwrap();
    let invalid_path = temp_dir.path().join("invalid.yaml");
    fs::write(&invalid_path, "invalid: yaml: content: [").unwrap();

    let result = Config::load_from_file(&invalid_path);
    assert!(result.is_err());

    // Test saving to read-only directory (if possible)
    let readonly_dir = TempDir::new().unwrap();
    let readonly_path = readonly_dir.path().join("readonly_config.yaml");

    // Make directory read-only (unix-like systems only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(readonly_dir.path()).unwrap().permissions();
        perms.set_mode(0o444); // read-only
        fs::set_permissions(readonly_dir.path(), perms).unwrap();

        let config = Config::default();
        let result = config.save_to_file(&readonly_path);
        assert!(result.is_err());

        // Restore permissions for cleanup
        let mut perms = fs::metadata(readonly_dir.path()).unwrap().permissions();
        perms.set_mode(0o755); // restore normal permissions
        fs::set_permissions(readonly_dir.path(), perms).unwrap();
    }
}