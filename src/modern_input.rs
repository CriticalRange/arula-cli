/// Modern input handler using inquire - fully integrated with async event loop
use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, ClearType},
};
use inquire::{
    ui::{Attributes, Color as InquireColor, IndexPrefix, RenderConfig, StyleSheet, Styled},
    validator::Validation,
    Text,
};
use std::collections::VecDeque;
use std::io::{self, Write};

/// Modern input handler with inquire styling but custom event handling
pub struct ModernInputHandler {
    buffer: String,
    cursor_pos: usize,
    history: VecDeque<String>,
    history_index: Option<usize>,
    temp_buffer: Option<String>,
    prompt: String,
    max_history: usize,
    esc_pressed_once: bool,
}

impl ModernInputHandler {
    pub fn new(prompt: &str) -> Self {
        // Set global inquire theme
        Self::set_global_theme();

        Self {
            buffer: String::new(),
            cursor_pos: 0,
            history: VecDeque::new(),
            history_index: None,
            temp_buffer: None,
            prompt: prompt.to_string(),
            max_history: 1000,
            esc_pressed_once: false,
        }
    }

    /// Set global inquire theme for consistency with modern ARULA styling
    fn set_global_theme() {
        let mut config = RenderConfig::default();

        // Modern cyan/white theme matching ARULA
        config.prompt = StyleSheet::new()
            .with_fg(InquireColor::LightCyan)
            .with_attr(Attributes::BOLD);

        // Custom prompt prefix with icon
        config.prompt_prefix = Styled::new("⚡").with_fg(InquireColor::LightCyan);

        config.answer = StyleSheet::new()
            .with_fg(InquireColor::White)
            .with_attr(Attributes::ITALIC);

        config.default_value = StyleSheet::new().with_fg(InquireColor::DarkGrey);

        config.placeholder = StyleSheet::new().with_fg(InquireColor::DarkGrey);

        config.help_message = StyleSheet::new().with_fg(InquireColor::DarkCyan);

        // Enhanced error message styling
        config.error_message.message.fg = Some(InquireColor::LightRed);
        config.error_message.prefix = Styled::new("✗").with_fg(InquireColor::LightRed);

        // Better selection indicators
        config.highlighted_option_prefix =
            Styled::new("➤").with_fg(InquireColor::LightGreen);
        config.option_index_prefix = IndexPrefix::None;

        // Modern checkbox icons
        config.selected_checkbox = Styled::new("☑").with_fg(InquireColor::LightGreen);
        config.unselected_checkbox = Styled::new("☐").with_fg(InquireColor::DarkGrey);

        // Scroll indicators
        config.scroll_up_prefix = Styled::new("⇞").with_fg(InquireColor::LightCyan);
        config.scroll_down_prefix = Styled::new("⇟").with_fg(InquireColor::LightCyan);

        inquire::set_global_render_config(config);
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
    }

    pub fn add_to_history(&mut self, entry: String) {
        if entry.trim().is_empty() {
            return;
        }

        if self.history.back() == Some(&entry) {
            return;
        }

        self.history.push_back(entry);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }

    pub fn load_history(&mut self, lines: Vec<String>) {
        for line in lines {
            if !line.trim().is_empty() {
                self.history.push_back(line);
            }
        }
        if self.history.len() > self.max_history {
            self.history.drain(0..self.history.len() - self.max_history);
        }
    }

    pub fn get_history(&self) -> Vec<String> {
        self.history.iter().cloned().collect()
    }

    /// Draw the modern styled input prompt with enhanced visuals
    pub fn draw(&self) -> io::Result<()> {
        // Clear current line
        execute!(
            io::stdout(),
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::CurrentLine)
        )?;

        // Modern styled prompt with icon and gradient-like effect
        let icon = console::style("⚡").cyan().bold();
        let prompt = console::style(&self.prompt).cyan().bold();
        let text = console::style(&self.buffer).white();

        print!("{} {}{}", icon, prompt, text);

        // Position cursor (accounting for icon + space)
        let icon_len = "⚡ ".chars().count();
        let cursor_col = (icon_len + self.prompt.chars().count() + self.cursor_pos) as u16;
        execute!(io::stdout(), cursor::MoveToColumn(cursor_col))?;

        io::stdout().flush()?;
        Ok(())
    }

    fn reset_esc_flag(&mut self) {
        self.esc_pressed_once = false;
    }

    /// Handle key events with modern UX
    pub fn handle_key(&mut self, key: KeyEvent) -> io::Result<Option<String>> {
        if key.code != KeyCode::Esc {
            self.reset_esc_flag();
        }

        match key.code {
            KeyCode::Enter => {
                let input = self.buffer.clone();
                self.buffer.clear();
                self.cursor_pos = 0;
                self.history_index = None;
                self.temp_buffer = None;
                println!();
                Ok(Some(input))
            }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match c {
                        'c' | 'C' => {
                            self.buffer.clear();
                            self.cursor_pos = 0;
                            println!();
                            return Ok(Some("__CTRL_C__".to_string()));
                        }
                        'd' | 'D' => {
                            if self.buffer.is_empty() {
                                println!();
                                return Ok(Some("__CTRL_D__".to_string()));
                            }
                        }
                        'u' | 'U' => {
                            self.buffer.clear();
                            self.cursor_pos = 0;
                        }
                        'a' | 'A' => {
                            self.cursor_pos = 0;
                        }
                        'e' | 'E' => {
                            self.cursor_pos = self.buffer.chars().count();
                        }
                        'w' | 'W' => {
                            if self.cursor_pos > 0 {
                                let chars: Vec<char> = self.buffer.chars().collect();
                                let before_cursor: String =
                                    chars[..self.cursor_pos].iter().collect();
                                let trimmed = before_cursor.trim_end();
                                let last_space = trimmed
                                    .chars()
                                    .rev()
                                    .position(|c| c == ' ')
                                    .map(|p| trimmed.chars().count() - p)
                                    .unwrap_or(0);

                                let new_buffer: String = chars[..last_space]
                                    .iter()
                                    .chain(chars[self.cursor_pos..].iter())
                                    .collect();
                                self.buffer = new_buffer;
                                self.cursor_pos = last_space;
                            }
                        }
                        _ => {}
                    }
                } else {
                    let chars: Vec<char> = self.buffer.chars().collect();
                    let mut new_buffer = String::new();
                    new_buffer.extend(chars[..self.cursor_pos].iter());
                    new_buffer.push(c);
                    new_buffer.extend(chars[self.cursor_pos..].iter());
                    self.buffer = new_buffer;
                    self.cursor_pos += 1;
                    self.history_index = None;
                }
                self.draw()?;
                Ok(None)
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    let chars: Vec<char> = self.buffer.chars().collect();
                    let mut new_buffer = String::new();
                    new_buffer.extend(chars[..(self.cursor_pos - 1)].iter());
                    new_buffer.extend(chars[self.cursor_pos..].iter());
                    self.buffer = new_buffer;
                    self.cursor_pos -= 1;
                    self.history_index = None;
                }
                self.draw()?;
                Ok(None)
            }
            KeyCode::Delete => {
                let char_count = self.buffer.chars().count();
                if self.cursor_pos < char_count {
                    let chars: Vec<char> = self.buffer.chars().collect();
                    let mut new_buffer = String::new();
                    new_buffer.extend(chars[..self.cursor_pos].iter());
                    new_buffer.extend(chars[(self.cursor_pos + 1)..].iter());
                    self.buffer = new_buffer;
                    self.history_index = None;
                }
                self.draw()?;
                Ok(None)
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                self.draw()?;
                Ok(None)
            }
            KeyCode::Right => {
                let char_count = self.buffer.chars().count();
                if self.cursor_pos < char_count {
                    self.cursor_pos += 1;
                }
                self.draw()?;
                Ok(None)
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
                self.draw()?;
                Ok(None)
            }
            KeyCode::End => {
                self.cursor_pos = self.buffer.chars().count();
                self.draw()?;
                Ok(None)
            }
            KeyCode::Up => {
                if self.history.is_empty() {
                    return Ok(None);
                }

                if self.history_index.is_none() {
                    self.temp_buffer = Some(self.buffer.clone());
                    self.history_index = Some(self.history.len() - 1);
                } else if let Some(idx) = self.history_index {
                    if idx > 0 {
                        self.history_index = Some(idx - 1);
                    }
                }

                if let Some(idx) = self.history_index {
                    self.buffer = self.history[idx].clone();
                    self.cursor_pos = self.buffer.chars().count();
                }

                self.draw()?;
                Ok(None)
            }
            KeyCode::Down => {
                if let Some(idx) = self.history_index {
                    if idx < self.history.len() - 1 {
                        self.history_index = Some(idx + 1);
                        self.buffer = self.history[idx + 1].clone();
                    } else {
                        self.history_index = None;
                        self.buffer = self.temp_buffer.take().unwrap_or_default();
                    }
                    self.cursor_pos = self.buffer.chars().count();
                    self.draw()?;
                }
                Ok(None)
            }
            KeyCode::Tab => {
                // Could add autocomplete here using inquire's autocomplete
                Ok(None)
            }
            KeyCode::Esc => {
                if !self.buffer.is_empty() {
                    if self.esc_pressed_once {
                        self.buffer.clear();
                        self.cursor_pos = 0;
                        self.esc_pressed_once = false;
                        Ok(Some("__ESC_CLEARED__".to_string()))
                    } else {
                        self.esc_pressed_once = true;
                        Ok(Some("__ESC_WARN__".to_string()))
                    }
                } else {
                    Ok(Some("__ESC__".to_string()))
                }
            }
            _ => Ok(None),
        }
    }

    pub fn clear(&mut self) -> io::Result<()> {
        self.buffer.clear();
        self.cursor_pos = 0;
        self.draw()
    }
}

/// Helper functions for using inquire prompts in dialogs
pub mod dialogs {
    use super::*;

    /// Get input with validation using inquire
    pub fn get_validated_input<F>(prompt: &str, validator: F) -> Result<String>
    where
        F: Fn(&str) -> Result<Validation, String> + Clone + 'static,
    {
        let input = Text::new(prompt)
            .with_validator(move |input: &str| validator(input).map_err(|e| e.into()))
            .prompt()?;
        Ok(input)
    }

    /// Get input with placeholder
    pub fn get_input_with_placeholder(prompt: &str, placeholder: &str) -> Result<String> {
        let input = Text::new(prompt).with_placeholder(placeholder).prompt()?;
        Ok(input)
    }

    /// Get input with default value
    pub fn get_input_with_default(prompt: &str, default: &str) -> Result<String> {
        let input = Text::new(prompt).with_default(default).prompt()?;
        Ok(input)
    }

    /// Get input with autocomplete
    pub fn get_input_with_autocomplete(
        prompt: &str,
        suggestions: Vec<String>,
    ) -> Result<String> {
        let input = Text::new(prompt)
            .with_autocomplete(move |input: &str| {
                let input_lower = input.to_lowercase();
                Ok(suggestions
                    .iter()
                    .filter(|s| s.to_lowercase().contains(&input_lower))
                    .map(|s| s.clone())
                    .collect())
            })
            .prompt()?;
        Ok(input)
    }
}
