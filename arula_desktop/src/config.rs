use arula_core::utils::config::{AiConfig, Config};

/// Form state for the settings configuration panel.
#[derive(Debug, Clone)]
pub struct ConfigForm {
    pub provider: String,
    pub model: String,
    pub api_url: String,
    pub api_key: String,
    pub thinking_enabled: bool,
    pub web_search_enabled: bool,
    pub ollama_tools_enabled: bool,
    pub streaming_enabled: bool,
    pub living_background_enabled: bool,
    pub system_prompt: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub provider_options: Vec<String>,
    pub status: Option<String>,
}

impl ConfigForm {
    /// Creates a form pre-filled with provider-specific settings.
    pub fn with_provider_options(config: &Config, provider: String, provider_options: Vec<String>) -> Self {
        let defaults = AiConfig::get_provider_defaults(&provider);
        let provider_config = config.providers.get(&provider);

        let model = provider_config
            .map(|p| p.model.clone())
            .unwrap_or(defaults.model);
        let api_url = provider_config
            .and_then(|p| p.api_url.clone())
            .unwrap_or(defaults.api_url);
        let api_key = provider_config
            .map(|p| p.api_key.clone())
            .unwrap_or(defaults.api_key);
        let thinking_enabled = provider_config
            .and_then(|p| p.thinking_enabled)
            .unwrap_or(false);
        let web_search_enabled = provider_config
            .and_then(|p| p.web_search_enabled)
            .unwrap_or(false);
        let ollama_tools_enabled = provider_config
            .and_then(|p| p.tools_enabled)
            .unwrap_or(false);
        let streaming_enabled = provider_config
            .and_then(|p| p.streaming)
            .unwrap_or(true); // Default to true
        let living_background_enabled = config.get_living_background_enabled();

        Self {
            provider,
            model,
            api_url,
            api_key,
            thinking_enabled,
            web_search_enabled,
            ollama_tools_enabled,
            streaming_enabled,
            living_background_enabled,
            system_prompt: "You are ARULA, an Autonomous AI Interface assistant. You help users with coding, shell commands, and general software development tasks. Be concise, helpful, and provide practical solutions.".to_string(),
            temperature: 0.7,
            max_tokens: 2048,
            provider_options,
            status: None,
        }
    }

    /// Creates a form from the current config.
    pub fn from_config(config: &Config) -> Self {
        let provider_options = collect_provider_options(config);
        Self::with_provider_options(config, config.active_provider.clone(), provider_options)
    }

    /// Returns true if the API URL field should be editable.
    pub fn api_url_editable(&self) -> bool {
        matches!(
            self.provider.to_lowercase().as_str(),
            "custom" | "ollama"
        )
    }

    /// Sets a success status message.
    pub fn set_success(&mut self, message: &str) {
        self.status = Some(message.to_string());
    }

    /// Sets an error status message.
    pub fn set_error(&mut self, message: &str) {
        self.status = Some(message.to_string());
    }

    /// Clears the status message.
    pub fn clear_status(&mut self) {
        self.status = None;
    }
}

/// Collects all available provider names.
pub fn collect_provider_options(config: &Config) -> Vec<String> {
    let mut providers = vec![
        "openai".to_string(),
        "anthropic".to_string(),
        "z.ai coding plan".to_string(),
        "ollama".to_string(),
        "openrouter".to_string(),
    ];

    for name in config.get_provider_names() {
        if !providers.iter().any(|p| p.eq_ignore_ascii_case(&name)) {
            providers.push(name);
        }
    }

    providers.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    providers
}
