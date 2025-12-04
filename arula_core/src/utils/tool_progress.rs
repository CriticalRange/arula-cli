//! Multi-tool progress display with ANSI cursor control
//!
//! This module provides inline progress display for multiple concurrent tools
//! while preserving native terminal scrollback.

use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Instant;

/// Status of a tool execution
#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    Pending,
    Running,
    Completed { success: bool },
}

/// Information about a single tool's progress
#[derive(Debug, Clone)]
pub struct ToolProgress {
    pub name: String,
    pub status: ToolStatus,
    pub start_time: Option<Instant>,
    pub result_preview: Option<String>,
}

/// Manages multi-tool progress display with ANSI cursor control
pub struct ToolProgressManager {
    tools: HashMap<String, ToolProgress>,
    tool_order: Vec<String>, // Maintain insertion order
    lines_reserved: usize,
    is_active: bool,
    input_line: String, // Current user input to display
}

impl ToolProgressManager {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            tool_order: Vec::new(),
            lines_reserved: 0,
            is_active: false,
            input_line: String::new(),
        }
    }

    /// Start tracking multiple tools
    pub fn start_tools(&mut self, tool_names: Vec<String>) -> io::Result<()> {
        self.tools.clear();
        self.tool_order.clear();

        for name in tool_names {
            self.tools.insert(
                name.clone(),
                ToolProgress {
                    name: name.clone(),
                    status: ToolStatus::Pending,
                    start_time: None,
                    result_preview: None,
                },
            );
            self.tool_order.push(name);
        }

        // Reserve lines: one per tool + one for input + one separator
        self.lines_reserved = self.tool_order.len() + 2;
        self.is_active = true;

        // Print initial state
        self.render_all()?;

        Ok(())
    }

    /// Update a specific tool's status
    pub fn update_tool(&mut self, tool_id: &str, status: ToolStatus) -> io::Result<()> {
        if let Some(tool) = self.tools.get_mut(tool_id) {
            if matches!(status, ToolStatus::Running) && tool.start_time.is_none() {
                tool.start_time = Some(Instant::now());
            }
            tool.status = status;
        }

        if self.is_active {
            self.render_all()?;
        }

        Ok(())
    }

    /// Set result preview for a tool
    pub fn set_result(&mut self, tool_id: &str, result: &str, success: bool) -> io::Result<()> {
        if let Some(tool) = self.tools.get_mut(tool_id) {
            // Truncate result for preview
            let preview = if result.len() > 50 {
                format!("{}...", &result[..47])
            } else {
                result.to_string()
            };
            tool.result_preview = Some(preview);
            tool.status = ToolStatus::Completed { success };
        }

        if self.is_active {
            self.render_all()?;
        }

        Ok(())
    }

    /// Update the input line display
    pub fn update_input(&mut self, input: &str) -> io::Result<()> {
        self.input_line = input.to_string();
        if self.is_active {
            self.render_all()?;
        }
        Ok(())
    }

    /// Check if all tools are completed
    pub fn all_completed(&self) -> bool {
        self.tools
            .values()
            .all(|t| matches!(t.status, ToolStatus::Completed { .. }))
    }

    /// Finalize and commit results to scrollback
    pub fn finalize(&mut self) -> io::Result<()> {
        if !self.is_active {
            return Ok(());
        }

        // Clear the reserved lines
        self.clear_reserved_lines()?;

        self.is_active = false;
        self.lines_reserved = 0;

        Ok(())
    }

    /// Render all tool progress lines
    fn render_all(&self) -> io::Result<()> {
        let mut stdout = io::stdout();

        // Move to start of reserved area (up from current position)
        // Use \r to go to column 0 first
        write!(stdout, "\r")?;
        if self.lines_reserved > 0 {
            write!(stdout, "\x1b[{}A", self.lines_reserved)?;
        }

        // Render separator - clear entire line first
        write!(stdout, "\x1b[2K")?; // Clear entire line
        writeln!(stdout, "{}", console::style("─".repeat(50)).dim())?;

        // Render each tool
        for tool_id in &self.tool_order {
            if let Some(tool) = self.tools.get(tool_id) {
                write!(stdout, "\x1b[2K")?; // Clear entire line
                self.render_tool_line(&mut stdout, tool)?;
                writeln!(stdout)?;
            }
        }

        // Render input line - clear and show
        write!(stdout, "\x1b[2K")?; // Clear entire line
        let prompt = console::style("▶ ").cyan();
        write!(stdout, "{}{}", prompt, self.input_line)?;

        stdout.flush()?;
        Ok(())
    }

    /// Render a single tool's status line
    fn render_tool_line(&self, stdout: &mut io::Stdout, tool: &ToolProgress) -> io::Result<()> {
        let (icon, status_text) = match &tool.status {
            ToolStatus::Pending => ("○", console::style("Pending").dim()),
            ToolStatus::Running => ("◐", console::style("Running...").yellow()),
            ToolStatus::Completed { success: true } => ("✓", console::style("Done").green()),
            ToolStatus::Completed { success: false } => ("✗", console::style("Failed").red()),
        };

        let name = console::style(&tool.name).bold();

        // Calculate elapsed time if running or completed
        let elapsed = if let Some(start) = tool.start_time {
            let secs = start.elapsed().as_secs_f32();
            format!(" ({:.1}s)", secs)
        } else {
            String::new()
        };

        write!(stdout, " {} {} {}{}", icon, name, status_text, elapsed)?;

        // Show result preview if available
        if let Some(preview) = &tool.result_preview {
            write!(stdout, " → {}", console::style(preview).dim())?;
        }

        Ok(())
    }

    /// Clear the reserved lines
    fn clear_reserved_lines(&self) -> io::Result<()> {
        let mut stdout = io::stdout();

        // Move to column 0 first
        write!(stdout, "\r")?;

        // Move up to start of reserved area
        if self.lines_reserved > 0 {
            write!(stdout, "\x1b[{}A", self.lines_reserved)?;
        }

        // Clear each line completely
        for _ in 0..self.lines_reserved {
            writeln!(stdout, "\x1b[2K")?;
        }

        // Move back up to where we started
        if self.lines_reserved > 0 {
            write!(stdout, "\x1b[{}A", self.lines_reserved)?;
        }

        stdout.flush()?;
        Ok(())
    }

    /// Check if manager is currently active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Get number of tools being tracked
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }
}

impl Default for ToolProgressManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper for persistent input display during AI responses
pub struct PersistentInput {
    current_input: String,
    cursor_pos: usize,
    is_visible: bool,
}

impl PersistentInput {
    pub fn new() -> Self {
        Self {
            current_input: String::new(),
            cursor_pos: 0,
            is_visible: true,
        }
    }

    /// Get byte index from character position
    fn char_to_byte_index(&self, char_pos: usize) -> usize {
        self.current_input
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.current_input.len())
    }

    /// Get character count
    fn char_count(&self) -> usize {
        self.current_input.chars().count()
    }

    /// Add a character at cursor position
    pub fn insert_char(&mut self, ch: char) {
        let byte_idx = self.char_to_byte_index(self.cursor_pos);
        self.current_input.insert(byte_idx, ch);
        self.cursor_pos += 1;
    }

    /// Remove character before cursor
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_idx = self.char_to_byte_index(self.cursor_pos);
            self.current_input.remove(byte_idx);
        }
    }

    /// Remove character at cursor
    pub fn delete(&mut self) {
        if self.cursor_pos < self.char_count() {
            let byte_idx = self.char_to_byte_index(self.cursor_pos);
            self.current_input.remove(byte_idx);
        }
    }

    /// Move cursor left
    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        if self.cursor_pos < self.char_count() {
            self.cursor_pos += 1;
        }
    }

    /// Move cursor to start
    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end
    pub fn move_end(&mut self) {
        self.cursor_pos = self.char_count();
    }

    /// Get current input
    pub fn get_input(&self) -> &str {
        &self.current_input
    }

    /// Clear input
    pub fn clear(&mut self) {
        self.current_input.clear();
        self.cursor_pos = 0;
    }

    /// Take the input (clears and returns)
    pub fn take(&mut self) -> String {
        let input = std::mem::take(&mut self.current_input);
        self.cursor_pos = 0;
        input
    }

    /// Render the input line at current position
    pub fn render(&self) -> io::Result<()> {
        if !self.is_visible {
            return Ok(());
        }

        let mut stdout = io::stdout();

        // Move to start of line and clear entire line, then redraw
        write!(stdout, "\r\x1b[2K")?; // Move to column 0 and clear entire line
        let prompt = console::style("▶ ").cyan();
        write!(stdout, "{}{}", prompt, self.current_input)?;

        // Position cursor correctly (use char count, not byte length)
        let cursor_offset = self.char_count() - self.cursor_pos;
        if cursor_offset > 0 {
            write!(stdout, "\x1b[{}D", cursor_offset)?;
        }

        stdout.flush()?;
        Ok(())
    }

    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
}

impl Default for PersistentInput {
    fn default() -> Self {
        Self::new()
    }
}

/// ANSI escape code helpers
pub mod ansi {
    use std::io::{self, Write};

    /// Move cursor up n lines
    pub fn move_up(n: usize) -> io::Result<()> {
        if n > 0 {
            write!(io::stdout(), "\x1b[{}A", n)?;
            io::stdout().flush()?;
        }
        Ok(())
    }

    /// Move cursor down n lines
    pub fn move_down(n: usize) -> io::Result<()> {
        if n > 0 {
            write!(io::stdout(), "\x1b[{}B", n)?;
            io::stdout().flush()?;
        }
        Ok(())
    }

    /// Move cursor to column
    pub fn move_to_column(col: usize) -> io::Result<()> {
        write!(io::stdout(), "\x1b[{}G", col)?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Clear from cursor to end of line
    pub fn clear_to_eol() -> io::Result<()> {
        write!(io::stdout(), "\x1b[K")?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Clear entire line
    pub fn clear_line() -> io::Result<()> {
        write!(io::stdout(), "\x1b[2K")?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Save cursor position
    pub fn save_cursor() -> io::Result<()> {
        write!(io::stdout(), "\x1b[s")?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Restore cursor position
    pub fn restore_cursor() -> io::Result<()> {
        write!(io::stdout(), "\x1b[u")?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Hide cursor
    pub fn hide_cursor() -> io::Result<()> {
        write!(io::stdout(), "\x1b[?25l")?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Show cursor
    pub fn show_cursor() -> io::Result<()> {
        write!(io::stdout(), "\x1b[?25h")?;
        io::stdout().flush()?;
        Ok(())
    }
}
