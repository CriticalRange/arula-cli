//! Exit confirmation menu for ARULA CLI

use crate::ui::menus::common::{draw_modern_box, draw_selected_item, MenuUtils};
use crate::ui::output::OutputHandler;
use crate::utils::colors::{ColorTheme, AI_HIGHLIGHT_ANSI, MISC_ANSI};
use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Print, ResetColor, SetForegroundColor},
    terminal, ExecutableCommand, QueueableCommand,
};
use std::io::{stdout, Write};

/// Exit confirmation menu handler
pub struct ExitMenu {
    options: Vec<String>,
}

impl ExitMenu {
    pub fn new() -> Self {
        Self {
            options: vec![
                "Stay in ARULA CLI".to_string(),
                "Exit ARULA CLI".to_string(),
            ],
        }
    }

    /// Show exit confirmation menu and return true if user wants to exit
    pub fn show(&mut self, output: &mut OutputHandler) -> Result<bool> {
        // Check terminal size
        if !MenuUtils::check_terminal_size(30, 8)? {
            output.print_system("Terminal too small for exit menu")?;
            return Ok(false);
        }

        // Setup terminal
        MenuUtils::setup_terminal()?;

        let result = self.run_menu_loop();

        // Restore terminal
        MenuUtils::restore_terminal()?;

        result
    }

    /// Exit menu event loop
    fn run_menu_loop(&mut self) -> Result<bool> {
        let mut selected_index = 0; // 0 = Stay, 1 = Exit

        loop {
            // Render menu
            self.render(selected_index)?;

            // Handle input
            match event::read()? {
                Event::Key(key_event) => {
                    if key_event.kind != KeyEventKind::Press {
                        continue;
                    }

                    match key_event.code {
                        KeyCode::Up => {
                            selected_index = if selected_index == 0 {
                                self.options.len() - 1
                            } else {
                                selected_index - 1
                            };
                        }
                        KeyCode::Down => {
                            selected_index = (selected_index + 1) % self.options.len();
                        }
                        KeyCode::Left => {
                            selected_index = if selected_index == 0 {
                                self.options.len() - 1
                            } else {
                                selected_index - 1
                            };
                        }
                        KeyCode::Right => {
                            selected_index = (selected_index + 1) % self.options.len();
                        }
                        KeyCode::Enter => {
                            return Ok(selected_index == 1); // true if Exit selected
                        }
                        KeyCode::Esc => {
                            return Ok(false); // Stay
                        }
                        KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                            return Ok(true); // Exit immediately
                        }
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {
                    // Continue loop to re-render
                }
                _ => {}
            }
        }
    }

    /// Render the exit confirmation menu
    fn render(&self, selected_index: usize) -> Result<()> {
        let (cols, rows) = terminal::size()?;

        // Clear screen
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;

        let menu_width = 40.min(cols.saturating_sub(4));
        let menu_height = 8;
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2;

        // Draw modern box using shared function
        draw_modern_box(start_x, start_y, menu_width, menu_height)?;

        // Draw title
        let title = " Exit Confirmation ";
        let title_x = start_x + (menu_width.saturating_sub(title.len() as u16)) / 2;
        stdout()
            .execute(MoveTo(title_x, start_y + 1))?
            .queue(Print(ColorTheme::primary().bold().apply_to(title)))?;

        // Draw options
        for (i, option) in self.options.iter().enumerate() {
            let y = start_y + 3 + i as u16;

            if i == selected_index {
                draw_selected_item(start_x, y, menu_width, option)?;
            } else {
                // Unselected item
                stdout()
                    .execute(MoveTo(start_x + 3, y))?
                    .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
                        MISC_ANSI,
                    )))?
                    .queue(Print(option))?
                    .queue(ResetColor)?;
            }
        }

        // Help text (left aligned)
        let help_text = "↑↓ Navigate • Enter Select • ESC Cancel";
        let help_x = start_x + 2; // Left aligned with padding
        stdout()
            .execute(MoveTo(help_x, start_y + 6))?
            .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
                AI_HIGHLIGHT_ANSI,
            )))?
            .queue(Print(help_text))?
            .queue(ResetColor)?;

        stdout().flush()?;
        Ok(())
    }

    // NOTE: draw_modern_box and draw_selected_item are now in common.rs
}

impl Default for ExitMenu {
    fn default() -> Self {
        Self::new()
    }
}
