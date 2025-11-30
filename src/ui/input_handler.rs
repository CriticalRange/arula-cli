use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    queue,
    terminal::{self, ClearType},
};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Shared state for blocking input during AI responses
#[derive(Clone)]
pub struct InputBlocker {
    is_blocked: Arc<Mutex<bool>>,
}

impl InputBlocker {
    pub fn new() -> Self {
        Self {
            is_blocked: Arc::new(Mutex::new(false)),
        }
    }

    pub fn block(&self) {
        if let Ok(mut blocked) = self.is_blocked.lock() {
            *blocked = true;
        }
    }

    pub fn unblock(&self) {
        if let Ok(mut blocked) = self.is_blocked.lock() {
            *blocked = false;
        }
    }

    pub fn is_blocked(&self) -> bool {
        self.is_blocked.lock().map(|b| *b).unwrap_or(false)
    }
}

/// Custom input handler that manages input line independently
#[derive(Clone)]
pub struct InputHandler {
    buffer: String,
    cursor_pos: usize,
    history: VecDeque<String>,
    history_index: Option<usize>,
    temp_buffer: Option<String>, // Temporary storage when navigating history
    prompt: String,
    max_history: usize,
    input_blocker: InputBlocker,
    bottom_line: u16,
    pub use_full_duplex: bool,
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
            input_blocker: InputBlocker::new(),
            bottom_line: 0,
            use_full_duplex: false,
        }
    }

    pub fn new_with_blocking(prompt: &str, input_blocker: InputBlocker) -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
            history: VecDeque::new(),
            history_index: None,
            temp_buffer: None,
            prompt: prompt.to_string(),
            max_history: 1000,
            input_blocker,
            bottom_line: 0,
            use_full_duplex: true,
        }
    }

    /// Initialize full-duplex mode
    pub fn initialize_full_duplex(&mut self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        // Save current cursor position
        let (_, current_row) = cursor::position()?;
        self.bottom_line = current_row;
        self.draw_input_line()?;
        Ok(())
    }

    /// Draw the input line at the bottom in full-duplex mode
    pub fn draw_input_line(&self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let (width, height) = terminal::size()?;
        let bottom_line = height.saturating_sub(1);

        // Move to bottom line and clear it
        queue!(
            io::stdout(),
            cursor::MoveTo(0, bottom_line),
            terminal::Clear(ClearType::CurrentLine),
        )?;

        // Get terminal width for horizontal scrolling
        let width = width as usize;
        let prompt_len = self.prompt.chars().count();
        let buffer_len = self.buffer.chars().count();

        // Build display content with horizontal scrolling
        let display_content = if prompt_len + buffer_len <= width {
            format!("{}{}", self.prompt, self.buffer)
        } else {
            let available_width = width.saturating_sub(prompt_len);
            if available_width == 0 {
                self.prompt.clone()
            } else if self.cursor_pos < available_width {
                let visible_end = self.buffer.chars().take(available_width).collect::<String>();
                format!("{}{}", self.prompt, visible_end)
            } else {
                let scroll_start = self.cursor_pos - available_width + 1;
                let visible_chars: String = self.buffer.chars()
                    .skip(scroll_start)
                    .take(available_width)
                    .collect();
                format!("{}{}", self.prompt, visible_chars)
            }
        };

        // Don't show any status text when blocked - just the normal prompt
        let final_content = display_content;

        // Print the content
        queue!(io::stdout(), crossterm::style::Print(final_content))?;

        // Position cursor correctly
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

        queue!(io::stdout(), cursor::MoveToColumn(cursor_col))?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Move cursor to position for output (above input line)
    pub fn prepare_for_output(&self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let (_, height) = terminal::size()?;

        // Move cursor up one line to make room for output
        queue!(
            io::stdout(),
            cursor::MoveToColumn(0),
            cursor::MoveUp(1),
        )?;

        io::stdout().flush()?;
        Ok(())
    }

    /// Scroll terminal up to make room for new output lines
    pub fn scroll_for_output(&self, lines: u16) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let (_, height) = terminal::size()?;

        // Print newlines to scroll content up
        for _ in 0..lines {
            println!();
        }

        // Redraw input line at bottom
        self.draw_input_line()?;
        Ok(())
    }

    /// Get the input blocker for external control
    pub fn get_input_blocker(&self) -> InputBlocker {
        self.input_blocker.clone()
    }

    /// Print a line while preserving the input area
    pub fn print_line_preserving_input(&self, content: &str) -> io::Result<()> {
        if self.use_full_duplex {
            // Move cursor up to output area
            self.prepare_for_output()?;
            // Move up another line to make room
            crossterm::execute!(io::stdout(), crossterm::cursor::MoveUp(1))?;
        }

        println!("{}", content);

        if self.use_full_duplex {
            // Redraw input line
            self.draw_input_line()?;
        }

        Ok(())
    }

    /// Print formatted content while preserving the input area
    pub fn print_preserving_input<F>(&self, print_fn: F) -> io::Result<()>
    where
        F: FnOnce() -> io::Result<()>,
    {
        if self.use_full_duplex {
            // Move cursor up to output area
            self.prepare_for_output()?;
            // Move up another line to make room
            crossterm::execute!(io::stdout(), crossterm::cursor::MoveUp(1))?;
        }

        print_fn()?;

        if self.use_full_duplex {
            // Redraw input line
            self.draw_input_line()?;
        }

        Ok(())
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
                // Check if input is blocked (AI is responding)
                if self.use_full_duplex && self.input_blocker.is_blocked() {
                    // Show blocked feedback and don't submit
                    self.show_blocked_feedback()?;
                    return Ok(None);
                }

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
                if self.use_full_duplex {
                    self.draw_input_line()?;
                } else {
                    self.draw()?;
                }
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
                if self.use_full_duplex {
                    self.draw_input_line()?;
                } else {
                    self.draw()?;
                }
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
                if self.use_full_duplex {
                    self.draw_input_line()?;
                } else {
                    self.draw()?;
                }
                Ok(None)
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                if self.use_full_duplex {
                    self.draw_input_line()?;
                } else {
                    self.draw()?;
                }
                Ok(None)
            }
            KeyCode::Right => {
                let char_count = self.buffer.chars().count();
                if self.cursor_pos < char_count {
                    self.cursor_pos += 1;
                }
                if self.use_full_duplex {
                    self.draw_input_line()?;
                } else {
                    self.draw()?;
                }
                Ok(None)
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
                if self.use_full_duplex {
                    self.draw_input_line()?;
                } else {
                    self.draw()?;
                }
                Ok(None)
            }
            KeyCode::End => {
                self.cursor_pos = self.buffer.chars().count();
                if self.use_full_duplex {
                    self.draw_input_line()?;
                } else {
                    self.draw()?;
                }
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

                if self.use_full_duplex {
                    self.draw_input_line()?;
                } else {
                    self.draw()?;
                }
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
                    if self.use_full_duplex {
                        self.draw_input_line()?;
                    } else {
                        self.draw()?;
                    }
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

    /// Show visual feedback when input is blocked
    fn show_blocked_feedback(&mut self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let (_, height) = terminal::size()?;
        let bottom_line = height.saturating_sub(1);

        queue!(
            io::stdout(),
            cursor::MoveTo(0, bottom_line),
            terminal::Clear(ClearType::CurrentLine),
            crossterm::style::Print("⏸️  Please wait for AI response to finish..."),
        )?;
        io::stdout().flush()?;

        std::thread::sleep(Duration::from_millis(500));
        self.draw_input_line()?;
        Ok(())
    }

    /// Read input with full-duplex mode (non-blocking keyboard polling)
    pub fn read_input_full_duplex(&mut self) -> io::Result<Option<String>> {
        if !self.use_full_duplex {
            return self.read_input_with_menu_detection();
        }

        loop {
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if let Some(input) = self.handle_key(key)? {
                        // Check for special signals
                        if input == "__ESC__" {
                            return Ok(None); // Cancel signal
                        }
                        if input == "__CTRL_C__" || input == "__CTRL_D__" {
                            return Ok(Some(input)); // Control signals
                        }
                        return Ok(Some(input));
                    }
                }
            }
        }
    }

    /// Read input with menu detection using simple stdin
    /// Returns: Some(input) for normal input, None for menu triggers
    pub fn read_input_with_menu_detection(&mut self) -> io::Result<Option<String>> {
        if self.use_full_duplex {
            return self.read_input_full_duplex();
        }

        print!("{} ", self.prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF
                return Ok(Some(String::new()));
            }
            Ok(_) => {
                let input = input.trim();

                // Check for menu triggers
                if input == "m" {
                    return Ok(None);
                }
                if input == "esc" || input == "escape" {
                    return Ok(None);
                }

                if !input.is_empty() {
                    self.add_to_history(input.to_string());
                    return Ok(Some(input.to_string()));
                }

                return Ok(Some(String::new()));
            }
            Err(e) => return Err(e),
        }
    }
}
