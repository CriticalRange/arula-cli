//! Thinking Widget - Displays AI reasoning/thinking with pulsing animation
//!
//! This module provides a visual display for AI model thinking/reasoning content
//! with a pulsing animation effect to indicate active thinking.
//!
//! # Supported Providers
//!
//! - **OpenAI**: Reasoning models with `reasoning.effort` parameter
//! - **Anthropic/Claude**: Extended thinking with `thinking.budget_tokens`
//! - **Ollama**: Models like DeepSeek-R1, Qwen3 with `think: true`
//! - **Z.AI**: Thinking mode with `thinking.type: "enabled"`

use console::style;
use crossterm::{cursor, execute, terminal};
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crate::utils::colors::PRIMARY_ANSI;

/// Thinking widget that displays AI reasoning with pulsing animation
pub struct ThinkingWidget {
    /// Whether the widget is currently active
    is_active: bool,
    /// Accumulated thinking content
    content: String,
    /// Animation frame counter
    frame: usize,
    /// Last animation update time
    last_update: Instant,
    /// Whether we've printed the header
    header_printed: bool,
}

impl ThinkingWidget {
    /// Create a new thinking widget
    pub fn new() -> Self {
        Self {
            is_active: false,
            content: String::new(),
            frame: 0,
            last_update: Instant::now(),
            header_printed: false,
        }
    }

    /// Start the thinking display
    pub fn start(&mut self) -> io::Result<()> {
        self.is_active = true;
        self.content.clear();
        self.frame = 0;
        self.header_printed = false;
        self.last_update = Instant::now();

        // Print initial thinking header with animation
        self.print_header()?;

        Ok(())
    }

    /// Print the pulsing "Thinking" header
    fn print_header(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();

        // Add spacing before thinking block
        println!();

        // Print the thinking indicator with pulse effect
        let frames = ["◐", "◓", "◑", "◒"];
        let frame_char = frames[self.frame % frames.len()];

        print!(
            "{} {}",
            style(frame_char).color256(PRIMARY_ANSI),
            style("Thinking").color256(PRIMARY_ANSI).bold()
        );

        stdout.flush()?;
        self.header_printed = true;

        Ok(())
    }

    /// Update the pulsing animation
    pub fn pulse(&mut self) -> io::Result<()> {
        if !self.is_active || !self.header_printed {
            return Ok(());
        }

        // Only update animation every 150ms
        if self.last_update.elapsed() < Duration::from_millis(150) {
            return Ok(());
        }

        let mut stdout = io::stdout();

        // Move cursor back to beginning of "Thinking" line
        execute!(stdout, cursor::MoveToColumn(0))?;

        // Clear the line
        print!("\x1b[K");

        // Update frame
        self.frame += 1;
        let frames = ["◐", "◓", "◑", "◒"];
        let frame_char = frames[self.frame % frames.len()];

        // Pulse effect: alternate between bright and dim
        let is_bright = (self.frame / 2).is_multiple_of(2);

        if is_bright {
            print!(
                "{} {}",
                style(frame_char).color256(PRIMARY_ANSI),
                style("Thinking").color256(PRIMARY_ANSI).bold()
            );
        } else {
            print!(
                "{} {}",
                style(frame_char).color256(PRIMARY_ANSI).dim(),
                style("Thinking").color256(PRIMARY_ANSI)
            );
        }

        stdout.flush()?;
        self.last_update = Instant::now();

        Ok(())
    }

    /// Add thinking content (streamed chunk)
    pub fn add_content(&mut self, chunk: &str) -> io::Result<()> {
        if !self.is_active {
            return Ok(());
        }

        self.content.push_str(chunk);

        Ok(())
    }

    /// Finish thinking and display the full thought
    pub fn finish(&mut self) -> io::Result<()> {
        if !self.is_active {
            return Ok(());
        }

        let mut stdout = io::stdout();

        // Move to new line after the pulsing header
        println!();

        // Display the thinking content in a styled box
        if !self.content.is_empty() {
            self.render_thinking_box(&self.content.clone())?;
        }

        stdout.flush()?;

        self.is_active = false;
        self.header_printed = false;

        Ok(())
    }

    /// Render the thinking content in a styled box
    fn render_thinking_box(&self, content: &str) -> io::Result<()> {
        let width = terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(80)
            .saturating_sub(4)
            .min(100);

        let border_color = 242; // Gray
        let text_color = 245; // Light gray

        // Top border
        println!(
            "{}",
            style(format!("┌{}┐", "─".repeat(width))).color256(border_color)
        );

        // Content lines with word wrapping
        for line in self.wrap_text(content, width - 4).lines() {
            let padding = width.saturating_sub(line.chars().count() + 2);
            println!(
                "{} {}{} {}",
                style("│").color256(border_color),
                style(line).color256(text_color).dim(),
                " ".repeat(padding),
                style("│").color256(border_color)
            );
        }

        // Bottom border
        println!(
            "{}",
            style(format!("└{}┘", "─".repeat(width))).color256(border_color)
        );

        Ok(())
    }

    /// Wrap text to fit within a given width
    fn wrap_text(&self, text: &str, max_width: usize) -> String {
        let mut result = String::new();
        let mut current_line = String::new();

        for word in text.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.chars().count() + 1 + word.chars().count() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&current_line);
        }

        result
    }

    /// Check if widget is currently active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Cancel thinking display without showing content
    pub fn cancel(&mut self) -> io::Result<()> {
        if !self.is_active {
            return Ok(());
        }

        let mut stdout = io::stdout();

        // Clear the thinking line
        execute!(stdout, cursor::MoveToColumn(0))?;
        print!("\x1b[K");
        stdout.flush()?;

        self.is_active = false;
        self.header_printed = false;
        self.content.clear();

        Ok(())
    }
}

impl Default for ThinkingWidget {
    fn default() -> Self {
        Self::new()
    }
}

/// Display thinking content with a simple static format (non-animated)
pub fn display_thinking_static(content: &str) -> io::Result<()> {
    let mut widget = ThinkingWidget::new();
    widget.content = content.to_string();
    widget.render_thinking_box(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_widget_creation() {
        let widget = ThinkingWidget::new();
        assert!(!widget.is_active());
    }

    #[test]
    fn test_wrap_text() {
        let widget = ThinkingWidget::new();
        let text = "This is a test of the word wrapping functionality";
        let wrapped = widget.wrap_text(text, 20);

        // Each line should be at most 20 chars
        for line in wrapped.lines() {
            assert!(line.chars().count() <= 20);
        }
    }

    #[test]
    fn test_add_content() {
        let mut widget = ThinkingWidget::new();
        widget.is_active = true;

        widget.add_content("Hello ").unwrap();
        widget.add_content("World").unwrap();

        assert_eq!(widget.content, "Hello World");
    }
}
