use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::app::{AiConfig, ArtConfig, Config, GitConfig, LoggingConfig, WorkspaceConfig};

impl Config {
    #[allow(dead_code)]
    pub async fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    #[allow(dead_code)]
    pub async fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn default() -> Self {
        Self {
            ai: AiConfig {
                provider: "local".to_string(),
                model: "default".to_string(),
            },
            git: GitConfig {
                auto_commit: true,
                create_branch: true,
            },
            logging: LoggingConfig {
                level: "INFO".to_string(),
            },
            art: ArtConfig {
                default_style: "fractal".to_string(),
            },
            workspace: WorkspaceConfig {
                path: "./arula_workspace".to_string(),
            },
        }
    }
}