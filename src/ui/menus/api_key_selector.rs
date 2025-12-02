//! API key configuration menu for ARULA CLI
//! Matches modern menu pattern from provider_menu.rs and model_selector.rs

use crate::app::App;
use crate::ui::output::OutputHandler;
use anyhow::Result;
use console::style;
use crossterm::{
    event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
    cursor::MoveTo,
    style::{SetForegroundColor, ResetColor, Print},
    ExecutableCommand, QueueableCommand,
};
use std::io::{stdout, Write};
use std::time::Duration;

/// API key configuration menu handler
pub struct ApiKeySelector;

impl ApiKeySelector {
    pub fn new() -> Self {
        Self
    }

    /// Display and handle the API key configuration menu (matching modern pattern)
    pub fn show(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let has_key = !app.config.get_api_key().is_empty();

        // Clear screen once when entering submenu to avoid artifacts (like original overlay_menu.rs)
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;

        // Comprehensive event clearing before selector (like original)
        std::thread::sleep(Duration::from_millis(20));
        for _ in 0..3 {
            while crossterm::event::poll(Duration::from_millis(0))? {
                let _ = crossterm::event::read()?;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        // Create options menu
        let options = if has_key {
            vec![
                "Update API Key".to_string(),
                "Clear API Key".to_string(),
            ]
        } else {
            vec![
                "Set API Key".to_string(),
            ]
        };

        let mut selected_idx = 0;
        let mut needs_clear = false; // Track when to clear screen

        loop {
            // Clear screen once if needed (after navigation)
            if needs_clear {
                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                stdout().flush()?;
                needs_clear = false;
            }

            self.render_api_key_selector(&options, selected_idx, has_key)?;

            if crossterm::event::poll(Duration::from_millis(100))? {
                match crossterm::event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                if selected_idx > 0 {
                                    selected_idx -= 1;
                                    needs_clear = true; // Clear once when scrolling
                                }
                            }
                            KeyCode::Down => {
                                if selected_idx < options.len() - 1 {
                                    selected_idx += 1;
                                    needs_clear = true; // Clear once when scrolling
                                }
                            }
                            KeyCode::Enter => {
                                // Handle selection
                                match selected_idx {
                                    0 => {
                                        // Set or Update API Key
                                        if let Some(new_key) = self.input_api_key(has_key, output)? {
                                            if !new_key.trim().is_empty() {
                                                app.config.set_api_key(&new_key);
                                                if let Err(e) = app.config.save() {
                                                    output.print_error(&format!("Failed to save config: {}", e))?;
                                                } else {
                                                    output.print_system("✅ API key updated successfully")?;
                                                }
                                            } else if !has_key {
                                                output.print_error("⚠️ API key cannot be empty")?;
                                            }
                                        }
                                    }
                                    1 => {
                                        // Clear API Key (only available if has_key is true)
                                        if has_key {
                                            app.config.set_api_key("");
                                            if let Err(e) = app.config.save() {
                                                output.print_error(&format!("Failed to save config: {}", e))?;
                                            } else {
                                                output.print_system("✅ API key cleared")?;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                                // Clear screen before exiting
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                break; // Selection made
                            }
                            KeyCode::Esc => {
                                // Clear screen before exiting
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                break; // Cancel selection
                            }
                            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                                // Clear screen before exiting
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                // Ctrl+C - close API key menu (will show exit confirmation)
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

    /// Render the API key selector with original styling (1:1 from overlay_menu.rs pattern)
    fn render_api_key_selector(&self, options: &[String], selected_idx: usize, has_key: bool) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;

        // Don't clear entire screen - causes flicker (like original)
        let menu_width = 50.min(cols.saturating_sub(4));
        let menu_height = options.len() + 8; // Added space for header, status, and footer
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
        self.draw_modern_box(start_x, start_y, menu_width, menu_height as u16, "API KEY")?;

        // Draw title/header (like original)
        let title_y = start_y + 1;
        let title = "🔑 API Key Configuration";
        let title_x = if menu_width > title.len() as u16 {
            start_x + (menu_width - title.len() as u16) / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(title_x, title_y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
              .queue(Print(style(title).bold()))?
              .queue(ResetColor)?;

        // Draw status line
        let status_y = start_y + 3;
        let status = if has_key {
            "Status: ✓ API key is set (••••••••)"
        } else {
            "Status: ✗ No API key configured"
        };
        let status_x = start_x + 2;
        let status_color = if has_key {
            crossterm::style::Color::Green
        } else {
            crossterm::style::Color::Red
        };
        stdout().queue(MoveTo(status_x, status_y))?
              .queue(SetForegroundColor(status_color))?
              .queue(Print(status))?
              .queue(ResetColor)?;

        // Draw options (like original)
        let items_start_y = start_y + 5;
        for (idx, option) in options.iter().enumerate() {
            let y = items_start_y + idx as u16;
            if idx == selected_idx {
                // Selected item with golden color (NO background)
                self.draw_selected_item(start_x + 2, y, menu_width - 4, option)?;
            } else {
                // Unselected item with normal color
                stdout().queue(MoveTo(start_x + 4, y))?
                      .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
                      .queue(Print(option))?
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

    /// Input API key with inline rendering (modern pattern)
    fn input_api_key(&self, has_existing: bool, _output: &mut OutputHandler) -> Result<Option<String>> {
        let prompt = if has_existing {
            "Enter new API key (leave empty to keep current):"
        } else {
            "Enter API key:"
        };

        // Clear screen for input
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;

        let (cols, rows) = crossterm::terminal::size()?;
        let dialog_width = 60.min(cols.saturating_sub(4));
        let dialog_height = 8;

        let start_x = if cols > dialog_width { (cols - dialog_width) / 2 } else { 0 };
        let start_y = if rows > dialog_height { (rows - dialog_height) / 2 } else { 0 };

        let mut input = String::new();
        let mut cursor_pos = 0;

        loop {
            // Render input dialog
            self.draw_modern_box(start_x, start_y, dialog_width, dialog_height, "API KEY INPUT")?;

            // Draw prompt
            let prompt_y = start_y + 2;
            stdout().queue(MoveTo(start_x + 2, prompt_y))?
                  .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
                  .queue(Print(prompt))?
                  .queue(ResetColor)?;

            // Draw input field with masked characters
            let input_y = start_y + 4;
            let masked_input = "•".repeat(input.len());
            stdout().queue(MoveTo(start_x + 2, input_y))?
                  .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::PRIMARY_ANSI)))?
                  .queue(Print(&format!("{}_", masked_input)))?
                  .queue(ResetColor)?;

            // Draw help
            let help_y = start_y + dialog_height - 1;
            let help_text = "Enter to confirm • ESC to cancel";
            let help_x = if dialog_width > help_text.len() as u16 {
                start_x + dialog_width / 2 - help_text.len() as u16 / 2
            } else {
                start_x + 1
            };
            stdout().queue(MoveTo(help_x, help_y))?
                  .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
                  .queue(Print(help_text))?
                  .queue(ResetColor)?;

            stdout().flush()?;

            // Handle input
            if crossterm::event::poll(Duration::from_millis(100))? {
                if let Event::Key(key_event) = crossterm::event::read()? {
                    if key_event.kind != KeyEventKind::Press {
                        continue;
                    }

                    match key_event.code {
                        KeyCode::Enter => {
                            return if input.trim().is_empty() && !has_existing {
                                Ok(None)
                            } else {
                                Ok(Some(input))
                            };
                        }
                        KeyCode::Esc => {
                            return Ok(None);
                        }
                        KeyCode::Backspace => {
                            if cursor_pos > 0 {
                                input.remove(cursor_pos - 1);
                                cursor_pos -= 1;
                            }
                        }
                        KeyCode::Left => {
                            cursor_pos = cursor_pos.saturating_sub(1);
                        }
                        KeyCode::Right => {
                            if cursor_pos < input.len() {
                                cursor_pos += 1;
                            }
                        }
                        KeyCode::Home => {
                            cursor_pos = 0;
                        }
                        KeyCode::End => {
                            cursor_pos = input.len();
                        }
                        KeyCode::Char(c) => {
                            input.insert(cursor_pos, c);
                            cursor_pos += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
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

impl Default for ApiKeySelector {
    fn default() -> Self {
        Self::new()
    }
}
