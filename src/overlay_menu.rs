use crate::app::App;
use crate::output::OutputHandler;
use anyhow::Result;
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

pub struct OverlayMenu;

impl OverlayMenu {
    pub fn new() -> Self {
        Self
    }

    pub fn show_main_menu(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<bool> {
        println!();
        println!(
            "{}",
            style("╔══════════════════════════════════════╗").cyan()
        );
        println!(
            "{}",
            style("║          ARULA MAIN MENU            ║").cyan()
        );
        println!(
            "{}",
            style("╚══════════════════════════════════════╝").cyan()
        );
        println!();

        let options = vec![
            "💬 Continue Chat",
            "🔧 Configuration",
            "📊 Session Info",
            "🗑️  Clear Chat",
            "❓ Help",
            "🚪 Exit",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select an option")
            .items(&options)
            .default(0)
            .interact_opt()?;

        println!();

        match selection {
            Some(0) => Ok(false), // Continue chat
            Some(1) => {
                // Configuration
                self.show_configuration_menu(app, output)?;
                Ok(false)
            }
            Some(2) => {
                // Session info
                self.show_session_info(app)?;
                Ok(false)
            }
            Some(3) => {
                // Clear chat
                if self.show_confirm_dialog("Clear chat history?")? {
                    app.clear_conversation();
                    output.print_system("✅ Chat history cleared")?;
                }
                Ok(false)
            }
            Some(4) => {
                // Help
                self.show_help()?;
                Ok(false)
            }
            Some(5) => {
                // Exit
                if self.show_confirm_dialog("Exit ARULA?")? {
                    output.print_system("Goodbye! 👋")?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            None => Ok(false), // ESC pressed
            _ => Ok(false),
        }
    }

    pub fn show_exit_confirmation(&mut self, output: &mut OutputHandler) -> Result<bool> {
        use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

        println!();
        // Temporarily disable raw mode for dialoguer
        disable_raw_mode()?;

        let result = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Exit ARULA?")
            .default(false)
            .interact()?;

        // Re-enable raw mode
        enable_raw_mode()?;

        if result {
            output.print_system("Goodbye! 👋")?;
        }
        Ok(result)
    }

    fn show_configuration_menu(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        loop {
            println!();
            let config = app.get_config();

            let options = vec![
                format!("Provider: {}", config.ai.provider),
                format!("Model: {}", config.ai.model),
                format!("API URL: {}", config.ai.api_url),
                format!(
                    "API Key: {}",
                    if config.ai.api_key.is_empty() {
                        "Not set"
                    } else {
                        "********"
                    }
                ),
                "← Back to Main Menu".to_string(),
            ];

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Configuration")
                .items(&options)
                .default(0)
                .interact_opt()?;

            match selection {
                Some(0) => {
                    // Edit provider
                    let providers = vec!["openai", "claude", "anthropic", "ollama", "custom"];
                    let current_idx = providers
                        .iter()
                        .position(|&p| p == config.ai.provider)
                        .unwrap_or(0);

                    let provider_idx = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("Select AI Provider")
                        .items(&providers)
                        .default(current_idx)
                        .interact()?;

                    app.config.ai.provider = providers[provider_idx].to_string();
                    let _ = app.config.save();
                    let _ = app.initialize_agent_client();
                    output.print_system(&format!(
                        "✅ Provider set to: {}",
                        providers[provider_idx]
                    ))?;
                }
                Some(1) => {
                    // Edit model
                    let model: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Enter model name")
                        .default(config.ai.model.clone())
                        .interact_text()?;

                    app.set_model(&model);
                    output.print_system(&format!("✅ Model set to: {}", model))?;
                }
                Some(2) => {
                    // Edit API URL
                    let url: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Enter API URL")
                        .default(config.ai.api_url.clone())
                        .interact_text()?;

                    app.config.ai.api_url = url.clone();
                    let _ = app.config.save();
                    let _ = app.initialize_agent_client();
                    output.print_system(&format!("✅ API URL set to: {}", url))?;
                }
                Some(3) => {
                    // Edit API Key
                    let key: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Enter API Key (or leave empty to use environment variable)")
                        .allow_empty(true)
                        .interact_text()?;

                    if !key.is_empty() {
                        app.config.ai.api_key = key;
                        let _ = app.config.save();
                        let _ = app.initialize_agent_client();
                        output.print_system("✅ API Key updated")?;
                    }
                }
                Some(4) | None => {
                    // Back or ESC
                    break;
                }
                _ => break,
            }
        }

        Ok(())
    }

    fn show_session_info(&mut self, app: &App) -> Result<()> {
        println!();
        println!("{}", style("📊 Session Information").cyan().bold());
        println!("{}", style("━".repeat(40)).cyan());

        let config = app.get_config();
        println!("{}: {}", style("Provider").bold(), config.ai.provider);
        println!("{}: {}", style("Model").bold(), config.ai.model);
        println!("{}: {}", style("API URL").bold(), config.ai.api_url);
        println!("{}: {}", style("Messages").bold(), app.messages.len());

        println!("{}", style("━".repeat(40)).cyan());
        println!();
        println!("{}", style("Press Enter to continue...").dim());

        let _ = std::io::stdin().read_line(&mut String::new());
        Ok(())
    }

    fn show_help(&mut self) -> Result<()> {
        println!();
        println!("{}", style("❓ ARULA Help").cyan().bold());
        println!("{}", style("━".repeat(50)).cyan());
        println!();
        println!("{}", style("🔧 Commands:").yellow().bold());
        println!("  {}  - Show this help", style("/help").green());
        println!("  {}  - Open interactive menu", style("/menu").green());
        println!(
            "  {}  - Clear conversation history",
            style("/clear").green()
        );
        println!(
            "  {}  - Show current configuration",
            style("/config").green()
        );
        println!("  {}  - Change AI model", style("/model <name>").green());
        println!("  {}  - Exit ARULA", style("exit or quit").green());
        println!();
        println!("{}", style("⌨️  Keyboard Shortcuts:").yellow().bold());
        println!("  {}  - Open menu", style("Ctrl+C").green());
        println!("  {}  - Exit", style("Ctrl+D").green());
        println!(
            "  {}  - Navigate command history",
            style("Up/Down Arrow").green()
        );
        println!();
        println!("{}", style("💡 Tips:").yellow().bold());
        println!("  • End line with \\ to continue on next line");
        println!("  • Ask ARULA to execute bash commands");
        println!("  • Use natural language");
        println!("  • Native terminal scrollback works!");
        println!();
        println!("{}", style("━".repeat(50)).cyan());
        println!();
        println!("{}", style("Press Enter to continue...").dim());

        let _ = std::io::stdin().read_line(&mut String::new());
        Ok(())
    }

    fn show_confirm_dialog(&mut self, message: &str) -> Result<bool> {
        let result = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .default(false)
            .interact()?;
        Ok(result)
    }
}

impl Default for OverlayMenu {
    fn default() -> Self {
        Self::new()
    }
}
