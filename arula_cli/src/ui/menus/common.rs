//! Common utilities and types for the ARULA menu system
//!
//! This module provides shared utilities for all ARULA CLI menus including:
//! - Common result/action types
//! - Terminal setup/restore helpers
//! - Shared drawing functions (box, item rendering)
//! - Menu state management

use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyEvent, KeyEventKind},
    style::{Print, ResetColor, SetForegroundColor},
    terminal::{self, size},
    ExecutableCommand, QueueableCommand,
};
use std::io::{stdout, Write};
use std::time::Duration;

use crate::utils::colors::{AI_HIGHLIGHT_ANSI, MISC_ANSI, PRIMARY_ANSI};

/// Common result types for menu operations
#[derive(Debug, Clone, PartialEq)]
pub enum MenuResult {
    Continue,
    Settings,
    Exit,
    ClearChat,
    BackToMain,
    ConfigurationUpdated,
    LoadConversation(String),
    NewConversation,
}

/// Internal menu action for flow control
#[derive(Debug, PartialEq)]
pub enum MenuAction {
    Continue,  // Stay in menu
    CloseMenu, // Exit menu, continue app
    ExitApp,   // Exit menu AND exit app
    CtrlC,     // Ctrl+C pressed (close menu, show exit confirmation)
}

/// Common menu utilities
pub struct MenuUtils;

impl MenuUtils {
    /// Truncate text to fit within max_width, adding "..." if truncated
    pub fn truncate_text(text: &str, max_width: usize) -> String {
        if text.len() <= max_width {
            text.to_string()
        } else {
            format!("{}...", &text[..max_width.saturating_sub(3)])
        }
    }

    /// Check if terminal has enough space for menu
    pub fn check_terminal_size(min_cols: u16, min_rows: u16) -> Result<bool> {
        let (cols, rows) = size()?;
        Ok(cols >= min_cols && rows >= min_rows)
    }

    /// Setup terminal for menu display (uses alternate screen to prevent scrollback pollution)
    pub fn setup_terminal() -> Result<()> {
        terminal::enable_raw_mode()?;
        stdout().execute(terminal::EnterAlternateScreen)?;
        stdout().execute(Hide)?;
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;
        stdout().execute(crossterm::cursor::MoveTo(0, 0))?;
        stdout().flush()?;
        Ok(())
    }

    /// Restore terminal state after menu (leaves alternate screen to return to conversation)
    pub fn restore_terminal() -> Result<()> {
        terminal::disable_raw_mode()?;
        stdout().execute(terminal::LeaveAlternateScreen)?;
        stdout().execute(Show)?;
        stdout().flush()?;
        Ok(())
    }

    /// Wait for key event with timeout
    pub fn wait_for_key(timeout_ms: u64) -> Result<Option<KeyEvent>> {
        if event::poll(Duration::from_millis(timeout_ms))? {
            if let Event::Key(key_event) = event::read()? {
                return Ok(Some(key_event));
            }
        }
        Ok(None)
    }

    /// Read key event with press/release filtering
    pub fn read_key_event() -> Result<Option<KeyEvent>> {
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => Ok(Some(key)),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Render a modern box frame with rounded corners (original style)
    pub fn render_box(title: &str, width: u16, height: u16) -> Vec<String> {
        let mut output = Vec::new();

        // Original modern rounded box styling
        let top_left = "╭";
        let top_right = "╮";
        let bottom_left = "╰";
        let bottom_right = "╯";
        let horizontal = "─";
        let vertical = "│";

        // Title with padding for centering
        let title_with_padding = format!(" {} ", title);
        let title_start = (width as usize / 2).saturating_sub(title_with_padding.len() / 2);
        let title_end = title_start + title_with_padding.len();

        // Top border with title
        let mut top_border = top_left.to_string();
        for i in 1..(width - 1) {
            let i_usize = i as usize;
            if i_usize >= title_start && i_usize < title_end && title_end <= width as usize {
                let title_char_index = i_usize - title_start;
                if title_char_index < title_with_padding.len() {
                    top_border.push(
                        title_with_padding
                            .chars()
                            .nth(title_char_index)
                            .unwrap_or('─'),
                    );
                } else {
                    top_border.push('─');
                }
            } else {
                top_border.push_str(horizontal);
            }
        }
        top_border.push_str(top_right);
        output.push(top_border);

        // Side borders with empty content
        for _ in 1..(height - 1) {
            output.push(format!(
                "{}{}{}",
                vertical,
                " ".repeat(width as usize - 2),
                vertical
            ));
        }

        // Bottom border
        let mut bottom_border = bottom_left.to_string();
        for _ in 1..(width - 1) {
            bottom_border.push_str(horizontal);
        }
        bottom_border.push_str(bottom_right);
        output.push(bottom_border);

        output
    }

    /// Format menu item with original selection indicator
    pub fn format_menu_item(item: &str, selected: bool) -> String {
        if selected {
            format!("▶ {}", item)
        } else {
            format!("  {}", item)
        }
    }
}

/// Common menu state management
pub struct MenuState {
    pub selected_index: usize,
    pub is_in_submenu: bool,
}

impl Default for MenuState {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            is_in_submenu: false,
        }
    }

    pub fn move_up(&mut self, max_index: usize) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = max_index.saturating_sub(1);
        }
    }

    pub fn move_down(&mut self, max_index: usize) {
        if self.selected_index < max_index.saturating_sub(1) {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    pub fn reset(&mut self) {
        self.selected_index = 0;
        self.is_in_submenu = false;
    }
}

// =============================================================================
// Shared Drawing Functions
// =============================================================================
// These were previously duplicated across all menu modules.
// Now consolidated here for maintainability.

/// Draw a modern box with rounded corners using the AI highlight color.
///
/// This is the shared implementation previously duplicated in:
/// - main_menu.rs
/// - config_menu.rs
/// - provider_menu.rs
/// - model_selector.rs
/// - api_key_selector.rs
/// - exit_menu.rs
pub fn draw_modern_box(x: u16, y: u16, width: u16, height: u16) -> Result<()> {
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
    stdout().queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
        AI_HIGHLIGHT_ANSI,
    )))?;

    // Draw vertical borders
    for i in 0..height {
        stdout().queue(MoveTo(x, y + i))?.queue(Print(vertical))?;
        stdout()
            .queue(MoveTo(x + width.saturating_sub(1), y + i))?
            .queue(Print(vertical))?;
    }

    // Top border
    stdout().queue(MoveTo(x, y))?.queue(Print(top_left))?;
    for _ in 1..width.saturating_sub(1) {
        stdout().queue(Print(horizontal))?;
    }
    stdout().queue(Print(top_right))?;

    // Bottom border
    stdout()
        .queue(MoveTo(x, y + height.saturating_sub(1)))?
        .queue(Print(bottom_left))?;
    for _ in 1..width.saturating_sub(1) {
        stdout().queue(Print(horizontal))?;
    }
    stdout().queue(Print(bottom_right))?;

    stdout().queue(ResetColor)?;
    Ok(())
}

/// Draw a selected menu item with the selection indicator and primary color.
///
/// This is the shared implementation previously duplicated in:
/// - main_menu.rs
/// - config_menu.rs
/// - provider_menu.rs
/// - model_selector.rs
/// - api_key_selector.rs
/// - exit_menu.rs
pub fn draw_selected_item(x: u16, y: u16, width: u16, text: &str) -> Result<()> {
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
        let char_end = text
            .char_indices()
            .nth(safe_len)
            .map(|(idx, _)| idx)
            .unwrap_or(text.len());
        format!("▶ {}...", &text[..char_end])
    } else {
        display_text
    };

    stdout()
        .queue(MoveTo(x + 2, y))?
        .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
            PRIMARY_ANSI,
        )))?
        .queue(Print(safe_text))?
        .queue(ResetColor)?;

    Ok(())
}

/// Draw an unselected menu item with the muted color.
pub fn draw_unselected_item(x: u16, y: u16, width: u16, text: &str) -> Result<()> {
    // Validate dimensions
    if width < 3 {
        return Ok(());
    }

    // Draw text with proper spacing and MISC color
    let display_text = format!("  {}", text);
    let safe_text = if display_text.len() > width.saturating_sub(4) as usize {
        let safe_len = width.saturating_sub(7) as usize;
        let char_end = text
            .char_indices()
            .nth(safe_len)
            .map(|(idx, _)| idx)
            .unwrap_or(text.len());
        format!("  {}...", &text[..char_end])
    } else {
        display_text
    };

    stdout()
        .queue(MoveTo(x + 2, y))?
        .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(
            MISC_ANSI,
        )))?
        .queue(Print(safe_text))?
        .queue(ResetColor)?;

    Ok(())
}

/// Draw a menu item (selected or not) - convenience wrapper
pub fn draw_menu_item(x: u16, y: u16, width: u16, text: &str, selected: bool) -> Result<()> {
    if selected {
        draw_selected_item(x, y, width, text)
    } else {
        draw_unselected_item(x, y, width, text)
    }
}

/// Clear a rectangular area of the terminal
pub fn clear_menu_area(x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    let blank = " ".repeat(width as usize);
    for i in 0..height {
        stdout()
            .queue(MoveTo(x, y + i))?
            .queue(Print(&blank))?;
    }
    Ok(())
}
