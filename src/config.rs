use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::app::{AiConfig, ArtConfig, Config, GitConfig, LoggingConfig, WorkspaceConfig};

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

    pub fn load() -> Self {
        let config_path = Self::get_config_path();
        let config_file = Path::new(&config_path);

        // Try to load existing config
        if config_file.exists() {
            if let Ok(config) = Self::load_from_file(config_file) {
                return config;
            }
        }

        // Return default config if loading fails
        Self::default()
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
                models: vec!["gpt-3.5-turbo".to_string()], // Initialize with default model
                api_url: "https://api.openai.com".to_string(),
                api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            },
            git: GitConfig {
                auto_commit: false,
                create_branch: false,
            },
            logging: LoggingConfig {
                level: "INFO".to_string(),
            },
            art: ArtConfig {
                default_style: "minimal".to_string(),
            },
            workspace: WorkspaceConfig {
                path: "./arula_workspace".to_string(),
            },
        }
    }

    pub fn get_provider_defaults() -> Vec<(String, String, String)> {
        vec![
            ("openai".to_string(), "gpt-3.5-turbo".to_string(), "https://api.openai.com".to_string()),
            ("openai".to_string(), "gpt-4".to_string(), "https://api.openai.com".to_string()),
            ("claude".to_string(), "claude-3-sonnet-20240229".to_string(), "https://api.anthropic.com".to_string()),
            ("claude".to_string(), "claude-3-haiku-20240307".to_string(), "https://api.anthropic.com".to_string()),
            ("ollama".to_string(), "llama2".to_string(), "http://localhost:11434".to_string()),
            ("ollama".to_string(), "codellama".to_string(), "http://localhost:11434".to_string()),
            ("ollama".to_string(), "mistral".to_string(), "http://localhost:11434".to_string()),
            ("Z.AI Coding Plan".to_string(), "glm-4.6".to_string(), "https://api.z.ai/api/paas/v4".to_string()),
            ("Z.AI Coding Plan".to_string(), "glm-4.5".to_string(), "https://api.z.ai/api/paas/v4".to_string()),
            ("custom".to_string(), "custom-model".to_string(), "http://localhost:8080".to_string()),
        ]
    }

    pub fn validate_ai_config(&self) -> Result<()> {
        if self.ai.provider.is_empty() {
            return Err(anyhow::anyhow!("AI provider cannot be empty"));
        }

        if self.ai.provider.to_lowercase() == "custom" {
            // For custom provider, check models array
            if self.ai.models.is_empty() {
                return Err(anyhow::anyhow!("At least one model must be configured for custom provider"));
            }
            if !self.ai.models.contains(&self.ai.model) {
                return Err(anyhow::anyhow!("Selected model must be in the models list"));
            }
        } else {
            // For other providers, check single model
            if self.ai.model.is_empty() {
                return Err(anyhow::anyhow!("AI model cannot be empty"));
            }
        }

        if self.ai.api_url.is_empty() {
            return Err(anyhow::anyhow!("API URL cannot be empty"));
        }

        // Check if API key is required and provided
        match self.ai.provider.to_lowercase().as_str() {
            "openai" if self.ai.api_key.is_empty() => {
                return Err(anyhow::anyhow!("OpenAI API key is required. Set OPENAI_API_KEY environment variable or configure in settings."));
            }
            "claude" | "anthropic" if self.ai.api_key.is_empty() => {
                return Err(anyhow::anyhow!("Claude API key is required. Set ANTHROPIC_API_KEY environment variable or configure in settings."));
            }
            "z.ai coding plan" | "z.ai" | "zai" if self.ai.api_key.is_empty() => {
                return Err(anyhow::anyhow!("Z.AI API key is required. Configure in settings."));
            }
            _ => {}
        }

        Ok(())
    }
}