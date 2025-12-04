//! Common utilities and types for the ARULA menu system

use anyhow::Result;
use crossterm::{
    terminal::{self, size},
    cursor::{Hide, Show},
    ExecutableCommand,
    event::{self, Event, KeyEvent, KeyEventKind},
};
use std::io::{stdout, Write};
use std::time::Duration;

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
    Continue,     // Stay in menu
    CloseMenu,    // Exit menu, continue app
    ExitApp,      // Exit menu AND exit app
    CtrlC,        // Ctrl+C pressed (close menu, show exit confirmation)
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
                    top_border.push(title_with_padding.chars().nth(title_char_index).unwrap_or('─'));
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
            output.push(format!("{}{}{}", vertical, " ".repeat(width as usize - 2), vertical));
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