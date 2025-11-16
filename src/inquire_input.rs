use anyhow::Result;
use crossterm::style::Color;
use inquire::{
    ui::{Attributes, Color as InquireColor, IndexPrefix, RenderConfig, StyleSheet, Styled},
    validator::{StringValidator, Validation},
    Text,
};

/// Modern input handler using inquire with custom styling
pub struct InquireInputHandler<'a> {
    prompt: String,
    render_config: RenderConfig<'a>,
}

impl<'a> InquireInputHandler<'a> {
    /// Create a new inquire input handler with custom styling
    pub fn new(prompt: &str) -> Self {
        let render_config = Self::create_custom_render_config();

        Self {
            prompt: prompt.to_string(),
            render_config,
        }
    }

    /// Create custom render config with modern styling (enhanced version)
    fn create_custom_render_config() -> RenderConfig<'static> {
        let mut config = RenderConfig::default();

        // Customize prompt styling
        config.prompt = StyleSheet::new()
            .with_fg(InquireColor::LightCyan)
            .with_attr(Attributes::BOLD);

        // Custom prompt prefix with icon
        config.prompt_prefix = Styled::new("⚡").with_fg(InquireColor::LightCyan);

        // Customize answer/input text styling
        config.answer = StyleSheet::new()
            .with_fg(InquireColor::White)
            .with_attr(Attributes::ITALIC);

        // Customize default value styling
        config.default_value = StyleSheet::new()
            .with_fg(InquireColor::DarkGrey);

        // Customize placeholder styling
        config.placeholder = StyleSheet::new()
            .with_fg(InquireColor::DarkGrey);

        // Customize help message styling
        config.help_message = StyleSheet::new()
            .with_fg(InquireColor::DarkCyan);

        // Enhanced error message styling
        config.error_message.message.fg = Some(InquireColor::LightRed);
        config.error_message.prefix = Styled::new("✗").with_fg(InquireColor::LightRed);

        // Better selection indicators
        config.highlighted_option_prefix = Styled::new("➤").with_fg(InquireColor::LightGreen);
        config.option_index_prefix = IndexPrefix::None;

        // Modern checkbox icons
        config.selected_checkbox = Styled::new("☑").with_fg(InquireColor::LightGreen);
        config.unselected_checkbox = Styled::new("☐").with_fg(InquireColor::DarkGrey);

        // Scroll indicators
        config.scroll_up_prefix = Styled::new("⇞").with_fg(InquireColor::LightCyan);
        config.scroll_down_prefix = Styled::new("⇟").with_fg(InquireColor::LightCyan);

        config
    }

    /// Get input from user with custom styling
    pub fn get_input(&self) -> Result<String> {
        let input = Text::new(&self.prompt)
            .with_render_config(self.render_config)
            .prompt()?;

        Ok(input)
    }

    /// Get input with a default value
    pub fn get_input_with_default(&self, default: &str) -> Result<String> {
        let input = Text::new(&self.prompt)
            .with_default(default)
            .with_render_config(self.render_config)
            .prompt()?;

        Ok(input)
    }

    /// Get input with a placeholder
    pub fn get_input_with_placeholder(&self, placeholder: &str) -> Result<String> {
        let input = Text::new(&self.prompt)
            .with_placeholder(placeholder)
            .with_render_config(self.render_config)
            .prompt()?;

        Ok(input)
    }

    /// Get input with help message
    pub fn get_input_with_help(&self, help: &str) -> Result<String> {
        let input = Text::new(&self.prompt)
            .with_help_message(help)
            .with_render_config(self.render_config)
            .prompt()?;

        Ok(input)
    }

    /// Get input with validation
    pub fn get_input_with_validation<F>(&self, validator: F) -> Result<String>
    where
        F: Fn(&str) -> Result<Validation, String> + Clone + 'static,
    {
        let input = Text::new(&self.prompt)
            .with_validator(move |input: &str| {
                validator(input).map_err(|e| e.into())
            })
            .with_render_config(self.render_config)
            .prompt()?;

        Ok(input)
    }

    /// Get input with autocomplete suggestions
    pub fn get_input_with_suggestions(&self, suggestions: Vec<String>) -> Result<String> {
        let input = Text::new(&self.prompt)
            .with_autocomplete(move |input: &str| {
                let input_lower = input.to_lowercase();
                Ok(suggestions
                    .iter()
                    .filter(|s| s.to_lowercase().contains(&input_lower))
                    .map(|s| s.clone())
                    .collect())
            })
            .with_render_config(self.render_config)
            .prompt()?;

        Ok(input)
    }

    /// Set global render config for all inquire prompts
    pub fn set_global_config() {
        let config = Self::create_custom_render_config();
        inquire::set_global_render_config(config);
    }
}

/// Builder for creating custom styled input prompts
pub struct StyledInputBuilder {
    prompt: String,
    default: Option<String>,
    placeholder: Option<String>,
    help: Option<String>,
    // Remove validator since it requires Clone which closures don't have
    suggestions: Option<Vec<String>>,
}

impl StyledInputBuilder {
    pub fn new(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            default: None,
            placeholder: None,
            help: None,
            suggestions: None,
        }
    }

    pub fn with_default(mut self, default: &str) -> Self {
        self.default = Some(default.to_string());
        self
    }

    pub fn with_placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = Some(placeholder.to_string());
        self
    }

    pub fn with_help(mut self, help: &str) -> Self {
        self.help = Some(help.to_string());
        self
    }

    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = Some(suggestions);
        self
    }

    pub fn prompt(self) -> Result<String> {
        let render_config = InquireInputHandler::create_custom_render_config();

        let mut text_prompt = Text::new(&self.prompt).with_render_config(render_config);

        if let Some(ref default) = self.default {
            text_prompt = text_prompt.with_default(default);
        }

        if let Some(ref placeholder) = self.placeholder {
            text_prompt = text_prompt.with_placeholder(placeholder);
        }

        if let Some(ref help) = self.help {
            text_prompt = text_prompt.with_help_message(help);
        }

        if let Some(suggestions) = self.suggestions {
            text_prompt = text_prompt.with_autocomplete(move |input: &str| {
                let input_lower = input.to_lowercase();
                Ok(suggestions
                    .iter()
                    .filter(|s| s.to_lowercase().contains(&input_lower))
                    .map(|s| s.clone())
                    .collect())
            });
        }

        Ok(text_prompt.prompt()?)
    }
}

/// Custom theme presets for different contexts
pub mod themes {
    use super::*;

    /// Default ARULA theme - cyan and white
    pub fn arula_default() -> RenderConfig<'static> {
        InquireInputHandler::create_custom_render_config()
    }

    /// Error/warning theme - red and yellow
    pub fn error_theme() -> RenderConfig<'static> {
        let mut config = RenderConfig::default();
        config.prompt = StyleSheet::new()
            .with_fg(InquireColor::LightRed)
            .with_attr(Attributes::BOLD);
        config.answer = StyleSheet::new()
            .with_fg(InquireColor::White);
        config
    }

    /// Success theme - green
    pub fn success_theme() -> RenderConfig<'static> {
        let mut config = RenderConfig::default();
        config.prompt = StyleSheet::new()
            .with_fg(InquireColor::LightGreen)
            .with_attr(Attributes::BOLD);
        config.answer = StyleSheet::new()
            .with_fg(InquireColor::White);
        config
    }

    /// Info theme - blue
    pub fn info_theme() -> RenderConfig<'static> {
        let mut config = RenderConfig::default();
        config.prompt = StyleSheet::new()
            .with_fg(InquireColor::LightBlue)
            .with_attr(Attributes::BOLD);
        config.answer = StyleSheet::new()
            .with_fg(InquireColor::White);
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creation() {
        let builder = StyledInputBuilder::new("Test prompt")
            .with_placeholder("Enter something...")
            .with_help("This is a test");

        assert_eq!(builder.prompt, "Test prompt");
        assert_eq!(builder.placeholder, Some("Enter something...".to_string()));
        assert_eq!(builder.help, Some("This is a test".to_string()));
    }

    #[test]
    fn test_inquire_handler_creation() {
        let handler = InquireInputHandler::new("▶ ");
        assert_eq!(handler.prompt, "▶ ");
    }
}
