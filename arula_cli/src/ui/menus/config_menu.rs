//! Configuration menu functionality for ARULA CLI

use crate::app::App;
use crate::ui::menus::api_key_selector::ApiKeySelector;
use crate::ui::menus::common::{MenuAction, MenuResult, MenuState, MenuUtils};
use crate::ui::menus::dialogs::Dialogs;
use crate::ui::menus::model_selector::ModelSelector;
use crate::ui::menus::provider_menu::ProviderMenu;
use crate::ui::output::OutputHandler;
use anyhow::Result;
use console::style;
use crossterm::{
    event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Print, ResetColor, SetForegroundColor},
    terminal, ExecutableCommand, QueueableCommand,
};
use std::io::{stdout, Write};
use std::time::Duration;

/// Configuration menu options
#[derive(Debug, Clone)]
pub enum ConfigMenuItem {
    AIProvider,
    AIModel,
    APIUrl,
    APIKey,
    ThinkingMode,
    WebSearch,
    OllamaTools,
}

impl ConfigMenuItem {
    pub fn all() -> Vec<Self> {
        vec![
            ConfigMenuItem::AIProvider,
            ConfigMenuItem::AIModel,
            ConfigMenuItem::APIUrl,
            ConfigMenuItem::APIKey,
            ConfigMenuItem::ThinkingMode,
            ConfigMenuItem::WebSearch,
            ConfigMenuItem::OllamaTools,
        ]
    }

    /// Get items based on current provider (some items only show for specific providers)
    pub fn for_provider(provider: &str) -> Vec<Self> {
        let mut items = vec![
            ConfigMenuItem::AIProvider,
            ConfigMenuItem::AIModel,
            ConfigMenuItem::APIUrl,
            ConfigMenuItem::APIKey,
            ConfigMenuItem::ThinkingMode,
            ConfigMenuItem::WebSearch,
        ];

        // Only show OllamaTools for Ollama provider
        if provider.to_lowercase() == "ollama" {
            items.push(ConfigMenuItem::OllamaTools);
        }

        items
    }

    pub fn label(&self) -> &str {
        match self {
            ConfigMenuItem::AIProvider => "AI Provider",
            ConfigMenuItem::AIModel => "AI Model",
            ConfigMenuItem::APIUrl => "API URL",
            ConfigMenuItem::APIKey => "API Key",
            ConfigMenuItem::ThinkingMode => "Thinking Mode",
            ConfigMenuItem::WebSearch => "Web Search",
            ConfigMenuItem::OllamaTools => "Ollama Tools",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            ConfigMenuItem::AIProvider => "Select AI provider (OpenAI, Anthropic, etc)",
            ConfigMenuItem::AIModel => "Choose AI model to use",
            ConfigMenuItem::APIUrl => "Set custom API endpoint URL",
            ConfigMenuItem::APIKey => "Configure API authentication key",
            ConfigMenuItem::ThinkingMode => "Toggle thinking mode (show AI reasoning)",
            ConfigMenuItem::WebSearch => "Toggle web search provider (DuckDuckGo/Z.AI)",
            ConfigMenuItem::OllamaTools => "Enable/disable tool calling for Ollama models",
        }
    }
}

/// Configuration menu handler
pub struct ConfigMenu {
    state: MenuState,
    items: Vec<ConfigMenuItem>,
    provider_menu: ProviderMenu,
    model_selector: ModelSelector,
    api_key_selector: ApiKeySelector,
    dialogs: Dialogs,
}

impl Default for ConfigMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigMenu {
    pub fn new() -> Self {
        Self {
            state: MenuState::new(),
            items: ConfigMenuItem::all(),
            provider_menu: ProviderMenu::new(),
            model_selector: ModelSelector::new(),
            api_key_selector: ApiKeySelector::new(),
            dialogs: Dialogs::new(),
        }
    }

    /// Display and handle the configuration menu
    pub fn show(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<MenuResult> {
        // Check terminal size
        if !MenuUtils::check_terminal_size(30, 8)? {
            output.print_system("Terminal too small for config menu")?;
            return Ok(MenuResult::Continue);
        }

        // Setup terminal
        MenuUtils::setup_terminal()?;

        let result = self.run_menu_loop(app, output);

        // Restore terminal
        MenuUtils::restore_terminal()?;

        result
    }

    /// Configuration menu event loop (fixed pattern)
    fn run_menu_loop(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<MenuResult> {
        // Clear pending events to prevent immediate exit
        std::thread::sleep(Duration::from_millis(50));
        for _ in 0..5 {
            while crossterm::event::poll(Duration::from_millis(0))? {
                let _ = crossterm::event::read()?;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // Track state for selective rendering
        let mut last_selected_index = self.state.selected_index;
        let mut needs_render = true; // Render first time

        loop {
            // Update items based on current provider (OllamaTools only for Ollama)
            self.items = ConfigMenuItem::for_provider(&app.config.active_provider);
            // Ensure we don't start on non-editable API URL (index 2)
            if self.state.selected_index == 2
                && !app
                    .config
                    .is_field_editable(crate::utils::config::ProviderField::ApiUrl)
            {
                self.state.selected_index = if self.state.selected_index + 1 < self.items.len() {
                    self.state.selected_index + 1
                } else {
                    0
                };
            }

            // Only render if state changed
            if needs_render || last_selected_index != self.state.selected_index {
                self.render(app, output)?;
                last_selected_index = self.state.selected_index;
                needs_render = false;
            }

            // Wait for input event with timeout
            if crossterm::event::poll(Duration::from_millis(100))? {
                match crossterm::event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                // Custom navigation logic to skip non-editable API URL (index 2)
                                let max_index = self.items.len().saturating_sub(1);
                                let mut new_index = self.state.selected_index as isize - 1;
                                new_index = if new_index < 0 {
                                    max_index as isize
                                } else {
                                    new_index
                                };

                                // If trying to land on API URL (index 2) and it's not editable, skip it
                                if new_index == 2
                                    && !app.config.is_field_editable(
                                        crate::utils::config::ProviderField::ApiUrl,
                                    )
                                {
                                    new_index -= 1;
                                    if new_index < 0 {
                                        new_index = max_index as isize;
                                    }
                                }
                                self.state.selected_index = new_index as usize;
                                needs_render = true;
                            }
                            KeyCode::Down => {
                                // Custom navigation logic to skip non-editable API URL (index 2)
                                let max_index = self.items.len().saturating_sub(1);
                                let mut new_index = self.state.selected_index as isize + 1;
                                new_index = if new_index > max_index as isize {
                                    0
                                } else {
                                    new_index
                                };

                                // If trying to land on API URL (index 2) and it's not editable, skip it
                                if new_index == 2
                                    && !app.config.is_field_editable(
                                        crate::utils::config::ProviderField::ApiUrl,
                                    )
                                {
                                    new_index += 1;
                                    if new_index > max_index as isize {
                                        new_index = 0;
                                    }
                                }
                                self.state.selected_index = new_index as usize;
                                needs_render = true;
                            }
                            KeyCode::Enter => {
                                match self.handle_selection(app, output)? {
                                    MenuAction::Continue => {
                                        // Submenu exited, re-render config menu
                                        needs_render = true;
                                    }
                                    MenuAction::CloseMenu => {
                                        return Ok(MenuResult::BackToMain);
                                    }
                                    MenuAction::ExitApp => {
                                        return Ok(MenuResult::Exit);
                                    }
                                    MenuAction::CtrlC => {
                                        return Ok(MenuResult::Exit); // Ctrl+C - close menu, show exit confirmation
                                    }
                                }
                            }
                            KeyCode::Esc => {
                                // Clear screen before exiting to remove menu display
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                return Ok(MenuResult::BackToMain);
                            }
                            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                                // Clear screen before exiting to remove menu display
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                return Ok(MenuResult::Exit); // Ctrl+C - exit immediately (will show exit confirmation)
                            }
                            _ => {}
                        }
                    }
                    Event::Resize(_, _) => {
                        // Re-render on resize
                        needs_render = true;
                    }
                    _ => {
                        // Ignore all other event types
                        continue;
                    }
                }
            }
        }
    }

    /// Render the configuration menu with original styling (1:1 from overlay_menu.rs)
    fn render(&self, app: &App, _output: &mut OutputHandler) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;

        // Ensure we have enough space for the menu, prevent underflow
        if cols < 25 || rows < 8 {
            return Ok(());
        }

        let config = app.get_config();
        let menu_width = 60.min(cols.saturating_sub(4));

        // Calculate max width for menu items (menu_width - 6 for padding and marker)
        let max_item_width = menu_width.saturating_sub(6) as usize;

        // Update display values with original styling and overflow protection
        let thinking_enabled = config
            .get_active_provider_config()
            .and_then(|c| c.thinking_enabled)
            .unwrap_or(false);
        let web_search_enabled = config
            .get_active_provider_config()
            .and_then(|c| c.web_search_enabled)
            .unwrap_or(false);
        let web_search_provider = if web_search_enabled && config.active_provider.contains("z.ai") {
            "Z.AI"
        } else {
            "DuckDuckGo"
        };

        let tools_enabled = config.get_tools_enabled();
        let is_ollama = config.active_provider.to_lowercase() == "ollama";

        let mut display_options = vec![
            format!(
                "Provider: {}",
                MenuUtils::truncate_text(
                    &config.active_provider,
                    max_item_width.saturating_sub(11)
                )
            ),
            format!(
                "Model: {}",
                MenuUtils::truncate_text(&config.get_model(), max_item_width.saturating_sub(9))
            ),
            format!(
                "API URL: {}",
                MenuUtils::truncate_text(
                    &config
                        .get_active_provider_config()
                        .and_then(|c| c.api_url.clone())
                        .unwrap_or_default(),
                    max_item_width.saturating_sub(11)
                )
            ),
            format!(
                "API Key: {}",
                if config.get_api_key().is_empty() {
                    "Not set"
                } else {
                    "••••••••"
                }
            ),
            format!(
                "Thinking: {}",
                if thinking_enabled {
                    "Enabled"
                } else {
                    "Disabled"
                }
            ),
            format!(
                "Web Search: {} ({})",
                if web_search_enabled {
                    "Enabled"
                } else {
                    "Disabled"
                },
                web_search_provider
            ),
        ];

        // Add Ollama Tools option only for Ollama provider
        if is_ollama {
            display_options.push(format!(
                "Ollama Tools: {}",
                if tools_enabled { "Enabled" } else { "Disabled" }
            ));
        }

        let menu_height = 14; // Increased height to accommodate new menu items
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2;

        // Clear screen before rendering to remove submenu remnants
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;
        stdout().execute(crossterm::cursor::MoveTo(0, 0))?;

        // Draw modern box using original draw_modern_box implementation
        self.draw_modern_box(start_x, start_y, menu_width, menu_height, "SETTINGS")?;

        // Draw title with modern styling
        let title_y = start_y + 1;
        let title = "⚙ SETTINGS";
        let title_len = title.len() as u16;
        let title_x = if menu_width > title_len + 2 {
            start_x + menu_width / 2 - title_len / 2
        } else {
            start_x + 1
        };
        stdout()
            .queue(crossterm::cursor::MoveTo(title_x, title_y))?
            .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
                crate::utils::colors::MISC_ANSI,
            )))?
            .queue(Print(style(title).bold()))?
            .queue(ResetColor)?;

        // Draw config items with modern styling
        let items_start_y = start_y + 3;
        for (i, option) in display_options.iter().enumerate() {
            let y = items_start_y + i as u16;

            // Check if this item is editable (API URL is index 2)
            let is_editable = if i == 2 {
                app.config
                    .is_field_editable(crate::utils::config::ProviderField::ApiUrl)
            } else {
                true
            };

            if i == self.state.selected_index {
                // Selected item with modern highlight using original draw_selected_item
                self.draw_selected_item(start_x + 2, y, menu_width - 4, option)?;
            } else {
                // Unselected item - clear the line first to remove any previous selection background
                stdout().queue(crossterm::cursor::MoveTo(start_x + 2, y))?;
                for _ in 0..(menu_width.saturating_sub(4)) {
                    stdout().queue(Print(" "))?;
                }
                // Then draw the text with gray color if not editable
                let color = if is_editable {
                    crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI)
                } else {
                    crossterm::style::Color::DarkGrey
                };
                stdout()
                    .queue(crossterm::cursor::MoveTo(start_x + 4, y))?
                    .queue(SetForegroundColor(color))?
                    .queue(Print(option))?
                    .queue(ResetColor)?;
            }
        }

        // Draw modern help text (intercepting box border - left aligned)
        let help_y = start_y + menu_height - 1;
        let help_text = "↑↓ Edit • Enter Select • ESC Exit";
        let help_x = start_x + 2; // Left aligned with padding
        stdout()
            .queue(crossterm::cursor::MoveTo(help_x, help_y))?
            .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
                crate::utils::colors::AI_HIGHLIGHT_ANSI,
            )))?
            .queue(Print(help_text))?
            .queue(ResetColor)?;

        stdout().flush()?;
        Ok(())
    }

    /// Draw modern box (1:1 from overlay_menu.rs)
    fn draw_modern_box(&self, x: u16, y: u16, width: u16, height: u16, _title: &str) -> Result<()> {
        // Modern box with rounded corners using our color theme
        let top_left = "╭";
        let top_right = "╮";
        let bottom_left = "╰";
        let bottom_right = "╯";
        let horizontal = "─";
        let vertical = "│";

        // Validate dimensions to prevent overflow
        if width < 2 || height < 2 {
            return Ok(());
        }

        // Draw borders using our AI highlight color (steel blue)
        stdout().queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
            crate::utils::colors::AI_HIGHLIGHT_ANSI,
        )))?;

        // Draw vertical borders
        for i in 0..height {
            stdout()
                .queue(crossterm::cursor::MoveTo(x, y + i))?
                .queue(Print(vertical))?;
            stdout()
                .queue(crossterm::cursor::MoveTo(
                    x + width.saturating_sub(1),
                    y + i,
                ))?
                .queue(Print(vertical))?;
        }

        // Top border
        stdout()
            .queue(crossterm::cursor::MoveTo(x, y))?
            .queue(Print(top_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(top_right))?;

        // Bottom border
        stdout()
            .queue(crossterm::cursor::MoveTo(x, y + height.saturating_sub(1)))?
            .queue(Print(bottom_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(bottom_right))?;

        stdout().queue(ResetColor)?;
        Ok(())
    }

    /// Draw selected item (1:1 from overlay_menu.rs) - NO BACKGROUND
    fn draw_selected_item(&self, x: u16, y: u16, width: u16, text: &str) -> Result<()> {
        // Validate dimensions
        if width < 3 {
            return Ok(());
        }

        // Draw text with proper spacing and primary color (NO background)
        let display_text = format!("▶ {}", text);
        let safe_text = if display_text.len() > width.saturating_sub(4) as usize {
            // Truncate if too long - use character boundaries, not byte boundaries
            let safe_len = width.saturating_sub(7) as usize;
            // Use char_indices to get safe character boundaries
            let char_end = text
                .char_indices()
                .nth(safe_len)
                .map(|(idx, _)| idx)
                .unwrap_or(text.len());
            format!("▶ {}...", &text[..char_end])
        } else {
            display_text
        };

        stdout()
            .queue(crossterm::cursor::MoveTo(x + 2, y))?
            .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
                crate::utils::colors::PRIMARY_ANSI,
            )))?
            .queue(Print(safe_text))?
            .queue(ResetColor)?;

        Ok(())
    }

    /// Get current value and description for menu items
    fn get_item_value_and_description(
        &self,
        item: &ConfigMenuItem,
        app: &App,
    ) -> (Option<String>, String) {
        match item {
            ConfigMenuItem::AIProvider => (
                Some(app.config.active_provider.clone()),
                item.description().to_string(),
            ),
            ConfigMenuItem::AIModel => {
                (Some(app.config.get_model()), item.description().to_string())
            }
            ConfigMenuItem::APIUrl => {
                let url = app
                    .config
                    .get_active_provider_config()
                    .and_then(|c| c.api_url.clone())
                    .unwrap_or_default();
                if url.is_empty() {
                    (None, item.description().to_string())
                } else {
                    (
                        Some(MenuUtils::truncate_text(&url, 30)),
                        item.description().to_string(),
                    )
                }
            }
            ConfigMenuItem::APIKey => {
                let has_key = !app.config.get_api_key().is_empty();
                if has_key {
                    (Some("••••••••".to_string()), item.description().to_string())
                } else {
                    (Some("Not set".to_string()), item.description().to_string())
                }
            }
            ConfigMenuItem::ThinkingMode => {
                let enabled = app
                    .config
                    .get_active_provider_config()
                    .and_then(|c| c.thinking_enabled)
                    .unwrap_or(false);
                (
                    Some(if enabled { "Enabled" } else { "Disabled" }.to_string()),
                    item.description().to_string(),
                )
            }
            ConfigMenuItem::WebSearch => {
                let enabled = app
                    .config
                    .get_active_provider_config()
                    .and_then(|c| c.web_search_enabled)
                    .unwrap_or(false);
                let provider = if enabled && app.config.active_provider.contains("z.ai") {
                    "Z.AI"
                } else {
                    "DuckDuckGo"
                };
                (
                    Some(format!(
                        "{} ({})",
                        if enabled { "Enabled" } else { "Disabled" },
                        provider
                    )),
                    item.description().to_string(),
                )
            }
            ConfigMenuItem::OllamaTools => {
                let enabled = app.config.get_tools_enabled();
                (
                    Some(if enabled { "Enabled" } else { "Disabled" }.to_string()),
                    item.description().to_string(),
                )
            }
        }
    }

    /// Handle keyboard input
    fn handle_input(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<MenuAction> {
        while let Some(key_event) = MenuUtils::read_key_event()? {
            match key_event.code {
                KeyCode::Up => {
                    self.state.move_up(self.items.len());
                }
                KeyCode::Down => {
                    self.state.move_down(self.items.len());
                }
                KeyCode::Enter => {
                    return self.handle_selection(app, output);
                }
                KeyCode::Esc => {
                    return Ok(MenuAction::CloseMenu);
                }
                _ => {}
            }
        }
        Ok(MenuAction::Continue)
    }

    /// Handle selection from configuration menu
    fn handle_selection(
        &mut self,
        app: &mut App,
        output: &mut OutputHandler,
    ) -> Result<MenuAction> {
        if let Some(selected_item) = self.items.get(self.state.selected_index) {
            match selected_item {
                ConfigMenuItem::AIProvider => {
                    self.provider_menu.show(app, output)?;
                    Ok(MenuAction::Continue)
                }
                ConfigMenuItem::AIModel => {
                    self.configure_model(app, output)?;
                    while crossterm::event::poll(Duration::from_millis(0))? {
                        let _ = crossterm::event::read()?;
                    }
                    Ok(MenuAction::Continue)
                }
                ConfigMenuItem::APIUrl => {
                    self.configure_api_url(app, output)?;
                    Ok(MenuAction::Continue)
                }
                ConfigMenuItem::APIKey => {
                    self.api_key_selector.show(app, output)?;
                    while crossterm::event::poll(Duration::from_millis(0))? {
                        let _ = crossterm::event::read()?;
                    }
                    Ok(MenuAction::Continue)
                }
                ConfigMenuItem::ThinkingMode => {
                    self.toggle_thinking_mode(app, output)?;
                    Ok(MenuAction::Continue)
                }
                ConfigMenuItem::WebSearch => {
                    self.toggle_web_search(app, output)?;
                    Ok(MenuAction::Continue)
                }
                ConfigMenuItem::OllamaTools => {
                    self.toggle_ollama_tools(app, output)?;
                    Ok(MenuAction::Continue)
                }
            }
        } else {
            Ok(MenuAction::Continue)
        }
    }

    fn configure_model(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        self.model_selector.show_model_selector(app, output)
    }

    fn configure_api_url(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let current_url = app
            .config
            .get_active_provider_config()
            .and_then(|c| c.api_url.clone())
            .unwrap_or_default();
        let prompt = if current_url.is_empty() {
            "Enter API URL:".to_string()
        } else {
            format!("Enter API URL (current: {}):", current_url)
        };
        if let Some(new_url) = self
            .dialogs
            .input_dialog(&prompt, Some(&current_url), output)?
        {
            if !new_url.trim().is_empty() {
                if let Some(config) = app.config.get_active_provider_config_mut() {
                    config.api_url = Some(new_url.to_string());
                }
                // Save config to disk and reinitialize client
                if let Err(e) = app.config.save() {
                    output.print_error(&format!("Failed to save configuration: {}", e))?;
                } else {
                    output.print_system(&format!("API URL updated to: {}", new_url))?;
                    // Reinitialize agent client with new URL
                    let _ = app.initialize_agent_client();
                }
            }
        }
        Ok(())
    }

    fn toggle_thinking_mode(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let current_enabled = app
            .config
            .get_active_provider_config()
            .and_then(|c| c.thinking_enabled)
            .unwrap_or(false);
        let new_enabled = !current_enabled;
        if let Some(config) = app.config.get_active_provider_config_mut() {
            config.thinking_enabled = Some(new_enabled);
        }
        if let Err(e) = app.config.save() {
            output.print_error(&format!("Failed to save configuration: {}", e))?;
        }
        let provider_name = &app.config.active_provider;
        if new_enabled {
            output.print_system(&format!(
                "💭 Thinking mode enabled for {} - AI will show reasoning",
                provider_name
            ))?;
        } else {
            output.print_system(&format!(
                "💭 Thinking mode disabled for {} - AI will give direct answers",
                provider_name
            ))?;
        }
        Ok(())
    }

    fn toggle_web_search(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let current_enabled = app
            .config
            .get_active_provider_config()
            .and_then(|c| c.web_search_enabled)
            .unwrap_or(false);
        let new_enabled = !current_enabled;
        if let Some(config) = app.config.get_active_provider_config_mut() {
            config.web_search_enabled = Some(new_enabled);
        }
        if let Err(e) = app.config.save() {
            output.print_error(&format!("Failed to save configuration: {}", e))?;
        }
        if new_enabled {
            let provider = if app.config.active_provider.contains("z.ai") {
                output.print_system("🔍 Web search enabled using Z.AI search")?;
                "Z.AI"
            } else {
                output.print_system("🔍 Web search enabled using DuckDuckGo")?;
                "DuckDuckGo"
            };
            output.print_system(&format!("ℹ Web search provider: {}", provider))?;
        } else {
            output.print_system("🔍 Web search disabled")?;
        }
        Ok(())
    }

    fn toggle_ollama_tools(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let current_enabled = app.config.get_tools_enabled();
        let new_enabled = !current_enabled;
        if let Err(e) = app.config.set_tools_enabled(new_enabled) {
            output.print_error(&format!("Failed to save configuration: {}", e))?;
            return Ok(());
        }
        // Reinitialize agent client with new setting
        let _ = app.initialize_agent_client();
        if new_enabled {
            output.print_system(
                "🔧 Ollama tools enabled - function calling will be sent to Ollama models",
            )?;
            output.print_system(
                "⚠️  Note: Not all Ollama models support tools. If you get errors, disable this.",
            )?;
        } else {
            output.print_system("🔧 Ollama tools disabled - function calling will not be used")?;
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.state.reset();
    }
}
