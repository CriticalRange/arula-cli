//! Common dialog utilities for ARULA menu system

use crate::ui::output::OutputHandler;
use crate::ui::menus::common::MenuUtils;
use anyhow::Result;
use console::style;
use crossterm::{
    event::KeyCode,
    style::Color,
    ExecutableCommand,
};
use std::io::{stdout, Write};

/// Dialog utilities for common user input patterns
pub struct Dialogs;

impl Dialogs {
    pub fn new() -> Self {
        Self
    }

    /// Show a confirmation dialog with Yes/No options
    pub fn confirm_dialog(
        &self,
        message: &str,
        output: &mut OutputHandler,
    ) -> Result<bool> {
        // Setup terminal
        MenuUtils::setup_terminal()?;

        let mut selected = false; // false = No, true = Yes

        loop {
            // Render confirmation dialog
            self.render_confirm_dialog(message, selected, output)?;

            // Handle input
            if let Some(key_event) = MenuUtils::read_key_event()? {
                match key_event.code {
                    KeyCode::Left | KeyCode::Right => {
                        selected = !selected;
                    }
                    KeyCode::Enter => {
                        MenuUtils::restore_terminal()?;
                        return Ok(selected);
                    }
                    KeyCode::Esc => {
                        MenuUtils::restore_terminal()?;
                        return Ok(false); // Cancel defaults to No
                    }
                    _ => {}
                }
            }
        }
    }

    /// Show an input dialog for text entry
    pub fn input_dialog(
        &self,
        prompt: &str,
        default_value: Option<&str>,
        output: &mut OutputHandler,
    ) -> Result<Option<String>> {
        // Setup terminal
        MenuUtils::setup_terminal()?;

        let mut input = default_value.unwrap_or("").to_string();
        let mut cursor_pos = input.len();

        loop {
            // Render input dialog
            self.render_input_dialog(prompt, &input, cursor_pos, output)?;

            // Handle input
            if let Some(key_event) = MenuUtils::read_key_event()? {
                match key_event.code {
                    KeyCode::Enter => {
                        MenuUtils::restore_terminal()?;
                        return if input.trim().is_empty() && default_value.is_none() {
                            Ok(None)
                        } else {
                            Ok(Some(input.trim().to_string()))
                        };
                    }
                    KeyCode::Esc => {
                        MenuUtils::restore_terminal()?;
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

    /// Show a password input dialog (characters hidden)
    pub fn password_dialog(
        &self,
        prompt: &str,
        output: &mut OutputHandler,
    ) -> Result<Option<String>> {
        // Setup terminal
        MenuUtils::setup_terminal()?;

        let mut input = String::new();

        loop {
            // Render password dialog
            self.render_password_dialog(prompt, input.len(), output)?;

            // Handle input
            if let Some(key_event) = MenuUtils::read_key_event()? {
                match key_event.code {
                    KeyCode::Enter => {
                        MenuUtils::restore_terminal()?;
                        return if input.trim().is_empty() {
                            Ok(None)
                        } else {
                            Ok(Some(input))
                        };
                    }
                    KeyCode::Esc => {
                        MenuUtils::restore_terminal()?;
                        return Ok(None);
                    }
                    KeyCode::Backspace => {
                        if !input.is_empty() {
                            input.pop();
                        }
                    }
                    KeyCode::Char(c) => {
                        input.push(c);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Show an alert/message dialog
    pub fn alert_dialog(
        &self,
        title: &str,
        message: &str,
        output: &mut OutputHandler,
    ) -> Result<()> {
        // Setup terminal
        MenuUtils::setup_terminal()?;

        // Render alert dialog
        self.render_alert_dialog(title, message, output)?;

        // Wait for any key
        while MenuUtils::read_key_event()?.is_none() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Restore terminal
        MenuUtils::restore_terminal()?;
        Ok(())
    }

    /// Render confirmation dialog
    fn render_confirm_dialog(
        &self,
        message: &str,
        selected_yes: bool,
        _output: &mut OutputHandler,
    ) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;
        let dialog_width = 40.min(cols);
        let dialog_height = 8;

        // Clear screen
        stdout().execute(crossterm::cursor::MoveTo(0, 0))?;

        // Calculate center position
        let start_col = (cols - dialog_width) / 2;
        let start_row = (rows - dialog_height) / 2;

        // Render dialog box
        let frame = MenuUtils::render_box("Confirm", dialog_width, dialog_height);
        for (i, line) in frame.iter().enumerate() {
            stdout().execute(crossterm::cursor::MoveTo(start_col, start_row + i as u16))?;
            println!("{}", line);
        }

        // Render message (wrap if needed)
        let message_lines = self.wrap_text(message, (dialog_width - 4) as usize);
        for (i, msg_line) in message_lines.iter().enumerate() {
            if i >= 3 {
                break; // Max 3 lines for message
            }
            stdout().execute(crossterm::cursor::MoveTo(start_col + 2, start_row + 2 + i as u16))?;
            println!("{}", msg_line);
        }

        // Render Yes/No options
        let options_row = start_row + dialog_height - 3;
        let no_text = if selected_yes { " No " } else { "[No]" };
        let yes_text = if selected_yes { "[Yes]" } else { " Yes " };

        // No option
        stdout().execute(crossterm::cursor::MoveTo(start_col + dialog_width - 20, options_row))?;
        if !selected_yes {
            println!("{}", style(no_text).cyan());
        } else {
            println!("{}", no_text);
        }

        // Yes option
        stdout().execute(crossterm::cursor::MoveTo(start_col + dialog_width - 10, options_row))?;
        if selected_yes {
            println!("{}", style(yes_text).cyan());
        } else {
            println!("{}", yes_text);
        }

        stdout().flush()?;
        Ok(())
    }

    /// Render input dialog
    fn render_input_dialog(
        &self,
        prompt: &str,
        input: &str,
        cursor_pos: usize,
        _output: &mut OutputHandler,
    ) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;
        let dialog_width = 60.min(cols);
        let dialog_height = 6;

        // Clear screen
        stdout().execute(crossterm::cursor::MoveTo(0, 0))?;

        // Calculate center position
        let start_col = (cols - dialog_width) / 2;
        let start_row = (rows - dialog_height) / 2;

        // Render dialog box
        let frame = MenuUtils::render_box("Input", dialog_width, dialog_height);
        for (i, line) in frame.iter().enumerate() {
            stdout().execute(crossterm::cursor::MoveTo(start_col, start_row + i as u16))?;
            println!("{}", line);
        }

        // Render prompt
        stdout().execute(crossterm::cursor::MoveTo(start_col + 2, start_row + 2))?;
        println!("{}", style(prompt).yellow());

        // Render input field
        let input_row = start_row + 3;
        let input_col = start_col + 2;
        stdout().execute(crossterm::cursor::MoveTo(input_col, input_row))?;

        // Input field background
        let field_width = dialog_width - 4;
        stdout().execute(crossterm::style::SetBackgroundColor(Color::DarkGrey))?;
        for _ in 0..field_width {
            println!(" ");
        }
        stdout().execute(crossterm::style::ResetColor)?;

        // Input text
        stdout().execute(crossterm::cursor::MoveTo(input_col, input_row))?;
        println!("{}", style(&input).white());

        // Cursor
        stdout().execute(crossterm::cursor::MoveTo(input_col + cursor_pos as u16, input_row))?;
        stdout().execute(crossterm::cursor::Show)?;

        stdout().flush()?;
        Ok(())
    }

    /// Render password dialog
    fn render_password_dialog(
        &self,
        prompt: &str,
        password_len: usize,
        _output: &mut OutputHandler,
    ) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;
        let dialog_width = 60.min(cols);
        let dialog_height = 6;

        // Clear screen
        stdout().execute(crossterm::cursor::MoveTo(0, 0))?;

        // Calculate center position
        let start_col = (cols - dialog_width) / 2;
        let start_row = (rows - dialog_height) / 2;

        // Render dialog box
        let frame = MenuUtils::render_box("Password", dialog_width, dialog_height);
        for (i, line) in frame.iter().enumerate() {
            stdout().execute(crossterm::cursor::MoveTo(start_col, start_row + i as u16))?;
            println!("{}", line);
        }

        // Render prompt
        stdout().execute(crossterm::cursor::MoveTo(start_col + 2, start_row + 2))?;
        println!("{}", style(prompt).yellow());

        // Render password field (show bullets instead of actual characters)
        let password_row = start_row + 3;
        let password_col = start_col + 2;
        stdout().execute(crossterm::cursor::MoveTo(password_col, password_row))?;

        // Password field background
        let field_width = dialog_width - 4;
        stdout().execute(crossterm::style::SetBackgroundColor(Color::DarkGrey))?;
        for _ in 0..field_width {
            println!(" ");
        }
        stdout().execute(crossterm::style::ResetColor)?;

        // Password bullets
        stdout().execute(crossterm::cursor::MoveTo(password_col, password_row))?;
        for _ in 0..password_len {
            println!("{}", style("•").white());
        }

        // Cursor at end
        stdout().execute(crossterm::cursor::MoveTo(password_col + password_len as u16, password_row))?;
        stdout().execute(crossterm::cursor::Show)?;

        stdout().flush()?;
        Ok(())
    }

    /// Render alert dialog
    fn render_alert_dialog(
        &self,
        title: &str,
        message: &str,
        _output: &mut OutputHandler,
    ) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;
        let dialog_width = 50.min(cols);
        let dialog_height = 8;

        // Clear screen
        stdout().execute(crossterm::cursor::MoveTo(0, 0))?;

        // Calculate center position
        let start_col = (cols - dialog_width) / 2;
        let start_row = (rows - dialog_height) / 2;

        // Render dialog box
        let frame = MenuUtils::render_box(title, dialog_width, dialog_height);
        for (i, line) in frame.iter().enumerate() {
            stdout().execute(crossterm::cursor::MoveTo(start_col, start_row + i as u16))?;
            println!("{}", line);
        }

        // Render message (wrap if needed)
        let message_lines = self.wrap_text(message, (dialog_width - 4) as usize);
        for (i, msg_line) in message_lines.iter().enumerate() {
            if i >= 4 {
                break; // Max 4 lines for message
            }
            stdout().execute(crossterm::cursor::MoveTo(start_col + 2, start_row + 2 + i as u16))?;
            println!("{}", msg_line);
        }

        // Render "Press any key" text
        stdout().execute(crossterm::cursor::MoveTo(start_col + dialog_width/2 - 7, start_row + dialog_height - 2))?;
        println!("{}", style("Press any key").dim());

        stdout().flush()?;
        Ok(())
    }

    /// Wrap text to fit within specified width
    fn wrap_text(&self, text: &str, max_width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut current_length = 0;

        for word in text.split_whitespace() {
            if current_length == 0 {
                current_line.push_str(word);
                current_length = word.len();
            } else if current_length + 1 + word.len() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
                current_length += 1 + word.len();
            } else {
                lines.push(current_line);
                current_line = word.to_string();
                current_length = word.len();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        lines
    }
}

impl Default for Dialogs {
    fn default() -> Self {
        Self
    }
}