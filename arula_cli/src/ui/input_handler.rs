//! Async-first input handler with tokio integration
//!
//! Provides non-blocking input handling that works seamlessly with async code.

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    terminal::{self, ClearType},
};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// Shared state for managing input during AI responses
/// Now supports queuing messages instead of blocking
#[derive(Clone)]
pub struct InputBlocker {
    is_blocked: Arc<Mutex<bool>>,
    queued_input: Arc<Mutex<Option<String>>>,
}

impl Default for InputBlocker {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBlocker {
    pub fn new() -> Self {
        Self {
            is_blocked: Arc::new(Mutex::new(false)),
            queued_input: Arc::new(Mutex::new(None)),
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

    /// Queue input to be processed after AI response completes
    pub fn queue_input(&self, input: String) {
        if let Ok(mut queued) = self.queued_input.lock() {
            *queued = Some(input);
        }
    }

    /// Take queued input (returns and clears it)
    pub fn take_queued_input(&self) -> Option<String> {
        if let Ok(mut queued) = self.queued_input.lock() {
            queued.take()
        } else {
            None
        }
    }

    /// Check if there's queued input
    pub fn has_queued_input(&self) -> bool {
        self.queued_input
            .lock()
            .map(|q| q.is_some())
            .unwrap_or(false)
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
        // Don't draw input line here - main.rs will print the initial prompt
        Ok(())
    }

    /// Calculate how many lines the input will take
    fn calculate_input_lines(&self, width: usize) -> usize {
        let prompt_len = self.prompt.chars().count();
        let buffer_len = self.buffer.chars().count();
        let total_len = prompt_len + buffer_len;

        if total_len == 0 {
            return 1;
        }

        // First line has prompt, subsequent lines are full width
        let first_line_chars = width.saturating_sub(prompt_len).min(buffer_len);
        let remaining_chars = buffer_len.saturating_sub(first_line_chars);

        if remaining_chars == 0 {
            1
        } else {
            1 + remaining_chars.div_ceil(width)
        }
    }

    /// Draw the input area at the bottom in full-duplex mode (multi-line support)
    pub fn draw_input_line(&self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let (width, height) = terminal::size()?;
        let width = width as usize;
        let prompt_len = self.prompt.chars().count();
        let buffer_chars: Vec<char> = self.buffer.chars().collect();
        let buffer_len = buffer_chars.len();

        // Calculate how many lines we need
        let num_lines = self.calculate_input_lines(width);
        let max_lines = 5.min(height as usize / 4); // Limit to 5 lines or 1/4 of screen
        let display_lines = num_lines.min(max_lines);

        // Calculate starting row for input area
        let start_row = height.saturating_sub(display_lines as u16);

        // Clear the input area
        for i in 0..display_lines {
            queue!(
                io::stdout(),
                cursor::MoveTo(0, start_row + i as u16),
                terminal::Clear(ClearType::CurrentLine),
            )?;
        }

        // Move to start of input area
        queue!(io::stdout(), cursor::MoveTo(0, start_row))?;

        // Print prompt
        queue!(io::stdout(), crossterm::style::Print(&self.prompt))?;

        // Print buffer content with wrapping
        let mut current_col = prompt_len;
        let mut current_row = start_row;
        let mut char_index = 0;

        // If buffer is too long, scroll to show cursor
        let chars_before_cursor = self.cursor_pos;
        let max_visible_chars = display_lines * width - prompt_len;

        let scroll_offset =
            if buffer_len > max_visible_chars && chars_before_cursor > max_visible_chars / 2 {
                // Scroll to keep cursor roughly in the middle
                (chars_before_cursor - max_visible_chars / 2)
                    .min(buffer_len.saturating_sub(max_visible_chars))
            } else {
                0
            };

        for (i, &ch) in buffer_chars.iter().enumerate().skip(scroll_offset) {
            if current_row >= height {
                break;
            }

            // Check if we need to wrap
            if current_col >= width {
                current_row += 1;
                current_col = 0;
                if current_row >= height {
                    break;
                }
                queue!(io::stdout(), cursor::MoveTo(0, current_row))?;
            }

            queue!(io::stdout(), crossterm::style::Print(ch))?;
            current_col += 1;
            char_index = i + 1;
        }

        // Show indicator if there's more content
        if scroll_offset > 0 || char_index < buffer_len {
            // There's hidden content
        }

        // Calculate cursor position
        let cursor_pos_in_view = self.cursor_pos.saturating_sub(scroll_offset);
        let cursor_row;
        let cursor_col;

        if cursor_pos_in_view == 0 {
            cursor_row = start_row;
            cursor_col = prompt_len as u16;
        } else {
            // First line can hold (width - prompt_len) chars
            let first_line_capacity = width.saturating_sub(prompt_len);

            if cursor_pos_in_view <= first_line_capacity {
                cursor_row = start_row;
                cursor_col = (prompt_len + cursor_pos_in_view) as u16;
            } else {
                // Calculate which line and column
                let remaining = cursor_pos_in_view - first_line_capacity;
                let extra_lines = remaining / width;
                let col_in_line = remaining % width;
                cursor_row = start_row + 1 + extra_lines as u16;
                cursor_col = col_in_line as u16;
            }
        }

        // Position cursor
        if cursor_row < height {
            queue!(io::stdout(), cursor::MoveTo(cursor_col, cursor_row))?;
        }

        io::stdout().flush()?;
        Ok(())
    }

    /// Move cursor to position for output (above input line)
    pub fn prepare_for_output(&self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let (_, _height) = terminal::size()?;

        // Move cursor up one line to make room for output
        queue!(io::stdout(), cursor::MoveToColumn(0), cursor::MoveUp(1),)?;

        io::stdout().flush()?;
        Ok(())
    }

    /// Scroll terminal up to make room for new output lines
    pub fn scroll_for_output(&self, lines: u16) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let (_, _height) = terminal::size()?;

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
                let visible_end = self
                    .buffer
                    .chars()
                    .take(available_width)
                    .collect::<String>();
                format!("{}{}", self.prompt, visible_end)
            } else {
                // Cursor is beyond the first screen - scroll to keep cursor visible
                let scroll_start = self.cursor_pos - available_width + 1;
                let visible_chars: String = self
                    .buffer
                    .chars()
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
                // If buffer is empty, don't do anything
                if self.buffer.trim().is_empty() {
                    return Ok(None);
                }

                // Check if input is blocked (AI is responding)
                if self.use_full_duplex && self.input_blocker.is_blocked() {
                    // Queue the input for later processing instead of blocking
                    let input = self.buffer.clone();
                    self.input_blocker.queue_input(input.clone());
                    self.buffer.clear();
                    self.cursor_pos = 0;
                    self.history_index = None;
                    self.temp_buffer = None;

                    // Show queued feedback
                    self.show_queued_feedback()?;
                    return Ok(None);
                }

                // Submit input immediately
                let input = self.buffer.clone();
                self.buffer.clear();
                self.cursor_pos = 0;
                self.history_index = None;
                self.temp_buffer = None;

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

    /// Show visual feedback when input is blocked (legacy - now we queue instead)
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

    /// Show visual feedback when input is queued
    fn show_queued_feedback(&mut self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let (_, height) = terminal::size()?;
        let bottom_line = height.saturating_sub(1);

        queue!(
            io::stdout(),
            cursor::MoveTo(0, bottom_line),
            terminal::Clear(ClearType::CurrentLine),
            crossterm::style::Print("📝 Message queued - will send after AI finishes"),
        )?;
        io::stdout().flush()?;

        std::thread::sleep(Duration::from_millis(800));
        self.draw_input_line()?;
        Ok(())
    }

    /// Poll for a single key event (non-blocking)
    /// Returns Some(input) if Enter was pressed, None otherwise
    pub fn poll_key_event(&mut self) -> io::Result<Option<String>> {
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(input) = self.handle_key(key)? {
                        // Check for special signals
                        if input == "__ESC__" {
                            return Ok(Some("__MENU__".to_string()));
                        }
                        if input == "__CTRL_C__" || input == "__CTRL_D__" {
                            return Ok(Some(input));
                        }
                        // Check for menu trigger
                        if input == "m" {
                            return Ok(Some("__MENU__".to_string()));
                        }
                        return Ok(Some(input));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Async read input - yields to tokio runtime between polls
    /// This is the PRIMARY method for reading input in async context
    pub async fn read_input_async(&mut self) -> io::Result<Option<String>> {
        // Draw initial input line
        self.draw_input_line()?;

        loop {
            // Poll for key events (non-blocking)
            match self.poll_key_event() {
                Ok(Some(input)) => {
                    // Input received
                    if !input.is_empty() {
                        self.add_to_history(input.clone());
                    }
                    return Ok(Some(input));
                }
                Ok(None) => {
                    // No input yet, yield to tokio runtime
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Legacy blocking read (for non-async contexts)
    pub fn read_input(&mut self) -> io::Result<Option<String>> {
        if self.use_full_duplex {
            // Use polling loop for full-duplex mode
            self.draw_input_line()?;
            loop {
                if event::poll(Duration::from_millis(10))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            if let Some(input) = self.handle_key(key)? {
                                if input == "__ESC__" || input == "m" {
                                    return Ok(Some("__MENU__".to_string()));
                                }
                                if !input.is_empty() {
                                    self.add_to_history(input.clone());
                                }
                                return Ok(Some(input));
                            }
                        }
                    }
                }
            }
        } else {
            // Simple stdin read for non-full-duplex mode
            print!("{} ", self.prompt);
            io::stdout().flush()?;

            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(0) => Ok(Some(String::new())),
                Ok(_) => {
                    let input = input.trim();
                    if input == "m" || input == "esc" || input == "escape" {
                        return Ok(Some("__MENU__".to_string()));
                    }
                    if !input.is_empty() {
                        self.add_to_history(input.to_string());
                    }
                    Ok(Some(input.to_string()))
                }
                Err(e) => Err(e),
            }
        }
    }

    /// Deprecated: Use read_input_async() instead
    #[deprecated(note = "Use read_input_async() for async contexts or read_input() for sync")]
    pub fn read_input_with_menu_detection(&mut self) -> io::Result<Option<String>> {
        self.read_input()
    }
}

/// Input event types for the async channel
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// User submitted input (pressed Enter)
    Input(String),
    /// Menu requested (ESC or 'm')
    Menu,
    /// Ctrl+C pressed
    Interrupt,
    /// Ctrl+D pressed (EOF)
    Eof,
}

/// Shared input state for coordinating between input task and main thread
#[derive(Clone)]
pub struct SharedInputState {
    buffer: Arc<Mutex<String>>,
    cursor_pos: Arc<Mutex<usize>>,
    output_active: Arc<Mutex<bool>>, // True when AI output is streaming
    prompt: String,
    use_full_duplex: bool,
}

impl SharedInputState {
    pub fn new(prompt: &str) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(String::new())),
            cursor_pos: Arc::new(Mutex::new(0)),
            output_active: Arc::new(Mutex::new(false)),
            prompt: prompt.to_string(),
            use_full_duplex: true,
        }
    }

    /// Get current buffer content
    pub fn get_buffer(&self) -> String {
        self.buffer.lock().map(|b| b.clone()).unwrap_or_default()
    }

    /// Get current cursor position
    pub fn get_cursor_pos(&self) -> usize {
        self.cursor_pos.lock().map(|p| *p).unwrap_or(0)
    }

    /// Check if output is currently active (suppress input drawing)
    pub fn is_output_active(&self) -> bool {
        self.output_active.lock().map(|a| *a).unwrap_or(false)
    }

    /// Set output active state
    pub fn set_output_active(&self, active: bool) {
        if let Ok(mut a) = self.output_active.lock() {
            *a = active;
        }
    }

    /// Clear the input line from screen (call before printing output)
    pub fn clear_line(&self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }
        // Move to start of line and clear it
        print!("\r\x1b[K");
        io::stdout().flush()?;
        Ok(())
    }

    /// Redraw the input line (call after printing output)
    /// This prints a newline first to move to a fresh line
    pub fn redraw(&self) -> io::Result<()> {
        if !self.use_full_duplex {
            return Ok(());
        }

        let buffer = self.get_buffer();
        let cursor_pos = self.get_cursor_pos();

        // Move to new line and print prompt with any buffered content
        print!("\n{}{}", self.prompt, buffer);

        // Position cursor correctly (only if not at end)
        let buffer_len = buffer.chars().count();
        if cursor_pos < buffer_len {
            let move_back = buffer_len - cursor_pos;
            print!("\x1b[{}D", move_back);
        }

        io::stdout().flush()?;
        Ok(())
    }
}

/// Async input stream that runs input handling in a separate task
pub struct AsyncInputReader {
    rx: mpsc::UnboundedReceiver<InputEvent>,
    shared_state: SharedInputState,
    _handle: tokio::task::JoinHandle<()>,
}

impl AsyncInputReader {
    /// Create a new async input reader
    /// Spawns a background task to handle keyboard input
    pub fn new(handler: InputHandler) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let shared_state = SharedInputState::new(&handler.prompt);
        let state_clone = shared_state.clone();

        // Clone handler fields we need
        let prompt = handler.prompt.clone();
        let mut buffer = handler.buffer.clone();
        let mut cursor_pos = handler.cursor_pos;
        let mut history = handler.history.clone();
        let mut history_index: Option<usize> = None;
        let mut temp_buffer: Option<String> = None;
        let input_blocker = handler.input_blocker.clone();

        let handle = tokio::spawn(async move {
            // Don't print initial prompt - main.rs already printed startup messages
            // The prompt will be shown after the first redraw

            loop {
                // Poll for key events (non-blocking)
                if event::poll(Duration::from_millis(0)).unwrap_or(false) {
                    if let Ok(Event::Key(key)) = event::read() {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }

                        match key.code {
                            KeyCode::Enter => {
                                if buffer.trim().is_empty() {
                                    continue;
                                }

                                // Check if blocked - queue input instead
                                if input_blocker.is_blocked() {
                                    input_blocker.queue_input(buffer.clone());
                                    buffer.clear();
                                    cursor_pos = 0;

                                    // Update shared state
                                    if let Ok(mut b) = state_clone.buffer.lock() {
                                        *b = buffer.clone();
                                    }
                                    if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                        *p = cursor_pos;
                                    }

                                    // Show queued feedback briefly
                                    print!("\r\x1b[K📝 Message queued");
                                    let _ = io::stdout().flush();
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    print!("\r\x1b[K{}", prompt);
                                    let _ = io::stdout().flush();
                                    continue;
                                }

                                let input = buffer.clone();

                                // Add to history
                                if !input.trim().is_empty() && history.back() != Some(&input) {
                                    history.push_back(input.clone());
                                    if history.len() > 1000 {
                                        history.pop_front();
                                    }
                                }

                                buffer.clear();
                                cursor_pos = 0;
                                history_index = None;
                                temp_buffer = None;

                                // Update shared state
                                if let Ok(mut b) = state_clone.buffer.lock() {
                                    *b = buffer.clone();
                                }
                                if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                    *p = cursor_pos;
                                }

                                // Send input event
                                let event = InputEvent::Input(input);
                                if tx.send(event).is_err() {
                                    break;
                                }
                            }
                            KeyCode::Char(c) => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    match c {
                                        'c' | 'C' => {
                                            buffer.clear();
                                            cursor_pos = 0;
                                            let _ = tx.send(InputEvent::Interrupt);
                                        }
                                        'd' | 'D' => {
                                            if buffer.is_empty() {
                                                let _ = tx.send(InputEvent::Eof);
                                            }
                                        }
                                        'u' | 'U' => {
                                            buffer.clear();
                                            cursor_pos = 0;
                                        }
                                        'a' | 'A' => cursor_pos = 0,
                                        'e' | 'E' => cursor_pos = buffer.chars().count(),
                                        _ => {}
                                    }
                                } else {
                                    // Insert character
                                    let chars: Vec<char> = buffer.chars().collect();
                                    let mut new_buffer = String::new();
                                    new_buffer.extend(chars[..cursor_pos].iter());
                                    new_buffer.push(c);
                                    new_buffer.extend(chars[cursor_pos..].iter());
                                    buffer = new_buffer;
                                    cursor_pos += 1;
                                }

                                // Update shared state
                                if let Ok(mut b) = state_clone.buffer.lock() {
                                    *b = buffer.clone();
                                }
                                if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                    *p = cursor_pos;
                                }

                                // Always redraw - use carriage return to overwrite current line
                                print!("\r\x1b[K{}{}", prompt, buffer);
                                let cursor_col = prompt.chars().count() + cursor_pos;
                                print!("\r\x1b[{}C", cursor_col);
                                let _ = io::stdout().flush();
                            }
                            KeyCode::Backspace => {
                                if cursor_pos > 0 {
                                    let chars: Vec<char> = buffer.chars().collect();
                                    let mut new_buffer = String::new();
                                    new_buffer.extend(chars[..(cursor_pos - 1)].iter());
                                    new_buffer.extend(chars[cursor_pos..].iter());
                                    buffer = new_buffer;
                                    cursor_pos -= 1;

                                    if let Ok(mut b) = state_clone.buffer.lock() {
                                        *b = buffer.clone();
                                    }
                                    if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                        *p = cursor_pos;
                                    }

                                    print!("\r\x1b[K{}{}", prompt, buffer);
                                    let cursor_col = prompt.chars().count() + cursor_pos;
                                    print!("\r\x1b[{}C", cursor_col);
                                    let _ = io::stdout().flush();
                                }
                            }
                            KeyCode::Left => {
                                if cursor_pos > 0 {
                                    cursor_pos -= 1;
                                    if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                        *p = cursor_pos;
                                    }
                                    print!("\x1b[D");
                                    let _ = io::stdout().flush();
                                }
                            }
                            KeyCode::Right => {
                                if cursor_pos < buffer.chars().count() {
                                    cursor_pos += 1;
                                    if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                        *p = cursor_pos;
                                    }
                                    print!("\x1b[C");
                                    let _ = io::stdout().flush();
                                }
                            }
                            KeyCode::Up => {
                                if !history.is_empty() {
                                    if history_index.is_none() {
                                        temp_buffer = Some(buffer.clone());
                                        history_index = Some(history.len() - 1);
                                    } else if let Some(idx) = history_index {
                                        if idx > 0 {
                                            history_index = Some(idx - 1);
                                        }
                                    }
                                    if let Some(idx) = history_index {
                                        buffer = history[idx].clone();
                                        cursor_pos = buffer.chars().count();

                                        if let Ok(mut b) = state_clone.buffer.lock() {
                                            *b = buffer.clone();
                                        }
                                        if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                            *p = cursor_pos;
                                        }

                                        print!("\r\x1b[K{}{}", prompt, buffer);
                                        let _ = io::stdout().flush();
                                    }
                                }
                            }
                            KeyCode::Down => {
                                if let Some(idx) = history_index {
                                    if idx < history.len() - 1 {
                                        history_index = Some(idx + 1);
                                        buffer = history[idx + 1].clone();
                                    } else {
                                        history_index = None;
                                        buffer = temp_buffer.take().unwrap_or_default();
                                    }
                                    cursor_pos = buffer.chars().count();

                                    if let Ok(mut b) = state_clone.buffer.lock() {
                                        *b = buffer.clone();
                                    }
                                    if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                        *p = cursor_pos;
                                    }

                                    print!("\r\x1b[K{}{}", prompt, buffer);
                                    let _ = io::stdout().flush();
                                }
                            }
                            KeyCode::Esc => {
                                let _ = tx.send(InputEvent::Menu);
                            }
                            KeyCode::Home => {
                                cursor_pos = 0;
                                if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                    *p = cursor_pos;
                                }
                                let cursor_col = prompt.chars().count();
                                print!("\r\x1b[{}C", cursor_col);
                                let _ = io::stdout().flush();
                            }
                            KeyCode::End => {
                                cursor_pos = buffer.chars().count();
                                if let Ok(mut p) = state_clone.cursor_pos.lock() {
                                    *p = cursor_pos;
                                }
                                let cursor_col = prompt.chars().count() + cursor_pos;
                                print!("\r\x1b[{}C", cursor_col);
                                let _ = io::stdout().flush();
                            }
                            _ => {}
                        }
                    }
                } else {
                    // No input, yield to runtime
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        });

        Self {
            rx,
            shared_state,
            _handle: handle,
        }
    }

    /// Get shared state for clearing/redrawing input line
    pub fn shared_state(&self) -> &SharedInputState {
        &self.shared_state
    }

    /// Receive the next input event
    pub async fn recv(&mut self) -> Option<InputEvent> {
        self.rx.recv().await
    }

    /// Try to receive an input event without blocking
    pub fn try_recv(&mut self) -> Option<InputEvent> {
        self.rx.try_recv().ok()
    }
}
