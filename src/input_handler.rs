use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, ClearType},
};
use std::collections::VecDeque;
use std::io::{self, Write};

/// Custom input handler that manages input line independently
pub struct InputHandler {
    buffer: String,
    cursor_pos: usize,
    history: VecDeque<String>,
    history_index: Option<usize>,
    temp_buffer: Option<String>, // Temporary storage when navigating history
    prompt: String,
    max_history: usize,
}

impl InputHandler {
    pub fn new(prompt: &str) -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
            history: VecDeque::new(),
            history_index: None,
            temp_buffer: None,
            prompt: prompt.to_string(),
            max_history: 1000,
        }
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
    }

    /// Add entry to history
    pub fn add_to_history(&mut self, entry: String) {
        if entry.trim().is_empty() {
            return;
        }

        // Don't add duplicates of the last entry
        if self.history.back() == Some(&entry) {
            return;
        }

        self.history.push_back(entry);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }

    /// Load history from lines
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

    /// Get history entries
    pub fn get_history(&self) -> Vec<String> {
        self.history.iter().cloned().collect()
    }

    /// Draw the input prompt and buffer at current cursor position
    pub fn draw(&self) -> io::Result<()> {
        // Get terminal width for single-line scrolling
        let (width, _) = terminal::size()?;
        let width = width as usize;

        let prompt_len = self.prompt.chars().count();
        let buffer_len = self.buffer.chars().count();

        // Build the display content first
        let display_content = if prompt_len + buffer_len <= width {
            // Buffer fits entirely on screen
            format!("{}{}", self.prompt, self.buffer)
        } else {
            // Buffer is longer than screen - implement horizontal scrolling
            let available_width = width.saturating_sub(prompt_len);
            if available_width == 0 {
                // Screen too small, just show prompt
                self.prompt.clone()
            } else if self.cursor_pos < available_width {
                // Cursor is in the first screen position
                let visible_end = self.buffer.chars().take(available_width).collect::<String>();
                format!("{}{}", self.prompt, visible_end)
            } else {
                // Cursor is beyond the first screen - scroll to keep cursor visible
                let scroll_start = self.cursor_pos - available_width + 1;
                let visible_chars: String = self.buffer.chars()
                    .skip(scroll_start)
                    .take(available_width)
                    .collect();
                format!("{}{}", self.prompt, visible_chars)
            }
        };

        // Clear and print content in one atomic operation to reduce cursor flash
        execute!(
            io::stdout(),
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::CurrentLine),
            cursor::Show,
            crossterm::style::Print(display_content)
        )?;

        // Position cursor correctly after content is displayed
        let cursor_col = if prompt_len + buffer_len <= width {
            (prompt_len + self.cursor_pos) as u16
        } else {
            let available_width = width.saturating_sub(prompt_len);
            if available_width == 0 {
                prompt_len as u16
            } else if self.cursor_pos < available_width {
                (prompt_len + self.cursor_pos) as u16
            } else {
                (prompt_len + available_width - 1) as u16
            }
        };

        execute!(io::stdout(), cursor::MoveToColumn(cursor_col))?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Handle a key event, returns Some(input) if user submitted
    pub fn handle_key(&mut self, key: KeyEvent) -> io::Result<Option<String>> {
        match key.code {
            KeyCode::Enter => {
                // Submit input
                let input = self.buffer.clone();
                self.buffer.clear();
                self.cursor_pos = 0;
                self.history_index = None;
                self.temp_buffer = None;

                // Don't add newline here - main.rs will handle layout for AI response
                // This prevents extra blank lines after user submits input

                Ok(Some(input))
            }
            KeyCode::Char(c) => {
                // Insert character at cursor position
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Handle Ctrl+C, Ctrl+D etc
                    match c {
                        'c' | 'C' => {
                            // Ctrl+C - return special signal
                            self.buffer.clear();
                            self.cursor_pos = 0;
                            println!();
                            return Ok(Some("__CTRL_C__".to_string()));
                        }
                        'd' | 'D' => {
                            // Ctrl+D - EOF
                            if self.buffer.is_empty() {
                                println!();
                                return Ok(Some("__CTRL_D__".to_string()));
                            }
                        }
                        'u' | 'U' => {
                            // Ctrl+U - clear line
                            self.buffer.clear();
                            self.cursor_pos = 0;
                        }
                        'a' | 'A' => {
                            // Ctrl+A - move to start
                            self.cursor_pos = 0;
                        }
                        'e' | 'E' => {
                            // Ctrl+E - move to end
                            self.cursor_pos = self.buffer.chars().count();
                        }
                        'w' | 'W' => {
                            // Ctrl+W - delete word backwards (character-aware)
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

                                // Rebuild buffer from characters
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
                    // Insert character at cursor position (UTF-8 safe)
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
                    // Remove character before cursor (UTF-8 safe)
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
                    // Remove character at cursor (UTF-8 safe)
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
                // Navigate history backwards
                if self.history.is_empty() {
                    return Ok(None);
                }

                if self.history_index.is_none() {
                    // Save current buffer
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
                // Navigate history forwards
                if let Some(idx) = self.history_index {
                    if idx < self.history.len() - 1 {
                        self.history_index = Some(idx + 1);
                        self.buffer = self.history[idx + 1].clone();
                    } else {
                        // Restore temp buffer
                        self.history_index = None;
                        self.buffer = self.temp_buffer.take().unwrap_or_default();
                    }
                    self.cursor_pos = self.buffer.chars().count();
                    self.draw()?;
                }
                Ok(None)
            }
            KeyCode::Tab => {
                // Could implement tab completion here
                Ok(None)
            }
            KeyCode::Esc => {
                // ESC - return special signal for cancellation
                Ok(Some("__ESC__".to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Clear the current input
    pub fn clear(&mut self) -> io::Result<()> {
        self.buffer.clear();
        self.cursor_pos = 0;
        self.draw()
    }

    /// Set the input buffer content
    pub fn set_input(&mut self, input: &str) {
        self.buffer = input.to_string();
        self.cursor_pos = self.buffer.chars().count();
    }
}
