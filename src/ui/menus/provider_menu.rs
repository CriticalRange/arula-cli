//! Provider selection menu for ARULA CLI (1:1 from overlay_menu.rs)

use crate::app::App;
use crate::ui::output::OutputHandler;
use anyhow::Result;
use console::style;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
    cursor::MoveTo,
    style::{SetForegroundColor, ResetColor, Print},
    ExecutableCommand, QueueableCommand,
};
use std::io::{stdout, Write};
use std::time::Duration;

/// Provider menu handler
pub struct ProviderMenu {
    providers: Vec<String>,
}

impl ProviderMenu {
    pub fn new() -> Self {
        Self {
            providers: vec![
                "openai".to_string(),
                "anthropic".to_string(),
                "ollama".to_string(),
                "z.ai coding plan".to_string(),
                "openrouter".to_string(),
                "custom".to_string(),
            ],
        }
    }

    /// Display and handle the provider selection menu (1:1 from overlay_menu.rs)
    pub fn show(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let current_config = app.get_config();
        let current_idx = self.providers
            .iter()
            .position(|p| p == &current_config.active_provider)
            .unwrap_or(0);

        // Clear screen once when entering submenu to avoid artifacts (like original overlay_menu.rs)
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;

        // Comprehensive event clearing before provider selector (like original)
        std::thread::sleep(Duration::from_millis(20));
        for _ in 0..3 {
            while crossterm::event::poll(Duration::from_millis(0))? {
                let _ = crossterm::event::read()?;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        // Create a temporary selection for provider (like original)
        let mut selected_idx = current_idx;
        loop {
            self.render_provider_selector(selected_idx)?;

            if crossterm::event::poll(Duration::from_millis(100))? {
                match crossterm::event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        // Only handle valid navigation keys (like original)
                        match key_event.code {
                            KeyCode::Up => {
                                if selected_idx > 0 {
                                    selected_idx -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if selected_idx < self.providers.len() - 1 {
                                    selected_idx += 1;
                                }
                            }
                            KeyCode::Enter => {
                                let new_provider = self.providers[selected_idx].clone();

                                // Switch to the new provider
                                let _ = app.config.switch_provider(&new_provider);

                                // Show what changed (like original)
                                output.print_system(&format!(
                                    "🔄 Model automatically set to: {}",
                                    app.config.get_model()
                                ))?;
                                output.print_system(&format!(
                                    "🌐 API URL automatically set to: {}",
                                    app.config.get_api_url()
                                ))?;

                                let _ = app.config.save();
                                match app.initialize_agent_client() {
                                    Ok(()) => {
                                        output.print_system(&format!(
                                            "✅ Provider set to: {} (AI client initialized)",
                                            self.providers[selected_idx]
                                        ))?;
                                    }
                                    Err(_) => {
                                        output.print_system(&format!(
                                            "✅ Provider set to: {} (AI client will initialize when configuration is complete)",
                                            self.providers[selected_idx]
                                        ))?;
                                    }
                                }
                                break; // Selection made
                            }
                            KeyCode::Esc => {
                                break; // Cancel selection
                            }
                            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                                // Ctrl+C - close provider menu (will show exit confirmation)
                                break;
                            }
                            _ => {}
                        }
                    }
                    Event::Resize(_, _) => {
                        // Continue loop to re-render
                    }
                    _ => {
                        // Ignore all other event types
                        continue;
                    }
                }
            }
        }
        Ok(())
    }

    /// Render the provider selector with original styling (1:1 from overlay_menu.rs)
    fn render_provider_selector(&self, selected_idx: usize) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;

        // Don't clear entire screen - causes flicker (like original)
        let menu_width = 50.min(cols.saturating_sub(4));
        let menu_height = self.providers.len() + 6; // Added space for header and footer
        let menu_height_u16 = menu_height as u16;

        // Ensure menu fits in terminal
        let menu_width = menu_width.min(cols.saturating_sub(4));
        let menu_height = if menu_height_u16 > rows.saturating_sub(4) {
            rows.saturating_sub(4) as usize
        } else {
            menu_height
        };

        let start_x = if cols > menu_width { cols.saturating_sub(menu_width) / 2 } else { 0 };
        let start_y = if rows > menu_height as u16 { rows.saturating_sub(menu_height as u16) / 2 } else { 0 };

        // Draw modern box using original function
        self.draw_modern_box(start_x, start_y, menu_width, menu_height as u16, "AI PROVIDER")?;

        // Draw title/header (like original)
        let title_y = start_y + 1;
        let title = "Select AI Provider";
        let title_x = if menu_width > title.len() as u16 {
            start_x + (menu_width - title.len() as u16) / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(title_x, title_y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
              .queue(Print(style(title).bold()))?
              .queue(ResetColor)?;

        // Draw provider options (like original)
        let items_start_y = start_y + 3;
        for (idx, provider) in self.providers.iter().enumerate() {
            let y = items_start_y + idx as u16;
            if idx == selected_idx {
                // Selected item with golden color (NO background)
                self.draw_selected_item(start_x + 2, y, menu_width - 4, provider)?;
            } else {
                // Unselected item with normal color
                stdout().queue(MoveTo(start_x + 4, y))?
                      .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
                      .queue(Print(provider))?
                      .queue(ResetColor)?;
            }
        }

        // Draw help text (intercepting box border - like original)
        let help_y = start_y + menu_height as u16 - 1;
        let help_text = "↑↓ Navigate • Enter Select • ESC Cancel";
        let help_len = help_text.len() as u16;
        let help_x = if menu_width > help_len + 2 {
            start_x + menu_width / 2 - help_len / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(help_x, help_y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
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
        stdout().queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?;

        // Draw vertical borders
        for i in 0..height {
            stdout().queue(MoveTo(x, y + i))?.queue(Print(vertical))?;
            stdout().queue(MoveTo(x + width.saturating_sub(1), y + i))?.queue(Print(vertical))?;
        }

        // Top border
        stdout().queue(MoveTo(x, y))?.queue(Print(top_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(top_right))?;

        // Bottom border
        stdout().queue(MoveTo(x, y + height.saturating_sub(1)))?.queue(Print(bottom_left))?;
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
            let char_end = text.char_indices().nth(safe_len)
                .map(|(idx, _)| idx)
                .unwrap_or(text.len());
            format!("▶ {}...", &text[..char_end])
        } else {
            display_text
        };

        stdout().queue(MoveTo(x + 2, y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::PRIMARY_ANSI)))?
              .queue(Print(safe_text))?
              .queue(ResetColor)?;

        Ok(())
    }
}

impl Default for ProviderMenu {
    fn default() -> Self {
        Self::new()
    }
}