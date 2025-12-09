//! Provider selection menu for ARULA CLI (1:1 from overlay_menu.rs)

use crate::app::App;
use crate::ui::menus::common::{draw_modern_box, draw_selected_item};
use crate::ui::output::OutputHandler;
use anyhow::Result;
use console::style;
use crossterm::{
    cursor::MoveTo,
    event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Print, ResetColor, SetForegroundColor},
    terminal, ExecutableCommand, QueueableCommand,
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
    pub fn show(&mut self, app: &mut App, _output: &mut OutputHandler) -> Result<()> {
        let current_config = app.get_config();
        let current_idx = self
            .providers
            .iter()
            .position(|p| p == &current_config.active_provider)
            .unwrap_or(0);

        // Clear visible screen once when entering submenu to avoid artifacts
        stdout().execute(crossterm::cursor::MoveTo(0, 0))?;
        stdout().execute(terminal::Clear(terminal::ClearType::FromCursorDown))?;

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
        let mut last_selected_idx = selected_idx;
        let mut needs_render = true; // Render first time

        loop {
            // Only render if state changed
            if needs_render || last_selected_idx != selected_idx {
                self.render_provider_selector(selected_idx)?;
                last_selected_idx = selected_idx;
                needs_render = false;
            }

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
                                    needs_render = true;
                                }
                            }
                            KeyCode::Down => {
                                if selected_idx < self.providers.len() - 1 {
                                    selected_idx += 1;
                                    needs_render = true;
                                }
                            }
                            KeyCode::Enter => {
                                let new_provider = self.providers[selected_idx].clone();

                                // Switch to the new provider
                                let _ = app.config.switch_provider(&new_provider);
                                let _ = app.config.save();
                                let _ = app.initialize_agent_client();

                                // Don't print messages here - menu overlays the main UI
                                // Just exit the menu and return to config menu
                                break; // Selection made
                            }
                            KeyCode::Esc => {
                                // Exit without clearing; main loop will redraw
                                break; // Cancel selection
                            }
                            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                                // Exit without clearing; main loop will redraw
                                break;
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

        let start_x = if cols > menu_width {
            cols.saturating_sub(menu_width) / 2
        } else {
            0
        };
        let start_y = if rows > menu_height as u16 {
            rows.saturating_sub(menu_height as u16) / 2
        } else {
            0
        };

        // Draw modern box using shared function
        draw_modern_box(start_x, start_y, menu_width, menu_height as u16)?;

        // Draw title/header (like original)
        let title_y = start_y + 1;
        let title = "Select AI Provider";
        let title_x = if menu_width > title.len() as u16 {
            start_x + (menu_width - title.len() as u16) / 2
        } else {
            start_x + 1
        };
        stdout()
            .queue(MoveTo(title_x, title_y))?
            .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
                crate::utils::colors::MISC_ANSI,
            )))?
            .queue(Print(style(title).bold()))?
            .queue(ResetColor)?;

        // Draw provider options (like original)
        let items_start_y = start_y + 3;
        for (idx, provider) in self.providers.iter().enumerate() {
            let y = items_start_y + idx as u16;

            // Clear the line first to remove any previous content
            stdout().queue(MoveTo(start_x + 2, y))?;
            for _ in 0..(menu_width.saturating_sub(4)) {
                stdout().queue(Print(" "))?;
            }

            if idx == selected_idx {
                // Selected item with golden color (NO background)
                draw_selected_item(start_x, y, menu_width, provider)?;
            } else {
                // Unselected item with normal color
                stdout()
                    .queue(MoveTo(start_x + 4, y))?
                    .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
                        crate::utils::colors::MISC_ANSI,
                    )))?
                    .queue(Print(provider))?
                    .queue(ResetColor)?;
            }
        }

        // Draw help text (intercepting box border - left aligned)
        let help_y = start_y + menu_height as u16 - 1;
        let help_text = "↑↓ Navigate • Enter Select • ESC Cancel";
        let help_x = start_x + 2; // Left aligned with padding
        stdout()
            .queue(MoveTo(help_x, help_y))?
            .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
                crate::utils::colors::AI_HIGHLIGHT_ANSI,
            )))?
            .queue(Print(help_text))?
            .queue(ResetColor)?;

        stdout().flush()?;
        Ok(())
    }

    // NOTE: draw_modern_box and draw_selected_item are now in common.rs
}

impl Default for ProviderMenu {
    fn default() -> Self {
        Self::new()
    }
}
