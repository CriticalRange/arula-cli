//! Conversation history management menu

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
    ExecutableCommand, QueueableCommand,
};
use std::io::{stdout, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use console::style;
use chrono::{DateTime, Utc};

use crate::app::App;
use crate::ui::output::OutputHandler;
use crate::utils::conversation::{Conversation, ConversationSummary};
use super::common::MenuResult;

pub struct ConversationMenu {
    selected_index: usize,
    conversations: Vec<ConversationSummary>,
    scroll_offset: usize,
    is_loading: bool,
    spinner_counter: usize,
}

impl ConversationMenu {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            conversations: Vec::new(),
            scroll_offset: 0,
            is_loading: true,
            spinner_counter: 0,
        }
    }

    /// Format a timestamp as relative time (e.g., "2 hours ago", "just now")
    fn format_relative_time(timestamp: DateTime<Utc>) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(timestamp);

        if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            let minutes = duration.num_minutes();
            if minutes == 1 {
                "1 minute ago".to_string()
            } else {
                format!("{} minutes ago", minutes)
            }
        } else if duration.num_hours() < 24 {
            let hours = duration.num_hours();
            if hours == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", hours)
            }
        } else if duration.num_days() < 7 {
            let days = duration.num_days();
            if days == 1 {
                "yesterday".to_string()
            } else {
                format!("{} days ago", days)
            }
        } else {
            // For older conversations, show the date
            timestamp.format("%b %d, %Y").to_string()
        }
    }

    /// Show the conversation selector menu
    pub fn show(&mut self, _app: &mut App, _output: &mut OutputHandler) -> Result<MenuResult> {
        // Clear screen once when entering
        stdout().execute(terminal::Clear(ClearType::All))?;

        // Start loading conversations in background
        self.is_loading = true;
        self.spinner_counter = 0;
        let current_dir = std::env::current_dir()?;

        // Shared state for background loading
        let conversations_shared = Arc::new(Mutex::new(Vec::new()));
        let conversations_clone = conversations_shared.clone();
        let is_loading_shared = Arc::new(Mutex::new(true));
        let is_loading_clone = is_loading_shared.clone();

        // Launch background task to load conversations
        let dir_for_task = current_dir.clone();
        std::thread::spawn(move || {
            match Conversation::list_all(&dir_for_task) {
                Ok(convs) => {
                    if let Ok(mut conversations) = conversations_clone.lock() {
                        *conversations = convs;
                    }
                }
                Err(_) => {
                    // Failed to load, mark as done anyway
                }
            }
            // Mark as done
            if let Ok(mut loading) = is_loading_clone.lock() {
                *loading = false;
            }
        });

        let result = self.run_selector(conversations_shared, is_loading_shared)?;

        // Clear screen before exiting
        stdout().execute(terminal::Clear(ClearType::All))?;
        stdout().flush()?;

        Ok(result)
    }

    fn run_selector(
        &mut self,
        conversations_shared: Arc<Mutex<Vec<ConversationSummary>>>,
        is_loading_shared: Arc<Mutex<bool>>,
    ) -> Result<MenuResult> {
        let visible_rows = 10;  // Visible conversation items
        let mut needs_clear = false;

        // Comprehensive event clearing to prevent issues
        std::thread::sleep(Duration::from_millis(50));
        for _ in 0..5 {
            while event::poll(Duration::from_millis(0))? {
                let _ = event::read()?;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // Track state to avoid unnecessary renders
        let mut last_loading_state = self.is_loading;
        let mut last_conversation_count = 0;

        loop {
            let mut state_changed = false;

            // Update loading state and conversations from background task
            if let Ok(loading) = is_loading_shared.lock() {
                if self.is_loading && !*loading {
                    // Just finished loading - major state change
                    self.is_loading = false;
                    state_changed = true;
                }
                self.is_loading = *loading;
            }

            // Update conversations list from shared state
            if let Ok(conversations) = conversations_shared.lock() {
                if self.conversations.len() != conversations.len() {
                    self.conversations = conversations.clone();
                    state_changed = true;
                }
            }

            // Update spinner animation (doesn't require clear)
            if self.is_loading {
                self.spinner_counter += 1;
            }

            // Only clear screen for major state changes (not for spinner animation)
            let should_clear = needs_clear || state_changed ||
                              (self.is_loading != last_loading_state) ||
                              (self.conversations.len() != last_conversation_count);

            if should_clear {
                stdout().execute(terminal::Clear(ClearType::All))?;
                stdout().flush()?;
                needs_clear = false;
                last_loading_state = self.is_loading;
                last_conversation_count = self.conversations.len();
            }

            self.render(visible_rows, !should_clear)?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        match key_event.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                if !self.conversations.is_empty() && self.selected_index > 0 {
                                    self.selected_index -= 1;
                                    if self.selected_index < self.scroll_offset {
                                        self.scroll_offset = self.selected_index;
                                    }
                                    needs_clear = true;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if !self.conversations.is_empty() && self.selected_index < self.conversations.len() - 1 {
                                    self.selected_index += 1;
                                    if self.selected_index >= self.scroll_offset + visible_rows {
                                        self.scroll_offset = self.selected_index - visible_rows + 1;
                                    }
                                    needs_clear = true;
                                }
                            }
                            KeyCode::PageUp => {
                                if !self.conversations.is_empty() {
                                    self.selected_index = self.selected_index.saturating_sub(visible_rows);
                                    self.scroll_offset = self.scroll_offset.saturating_sub(visible_rows);
                                    needs_clear = true;
                                }
                            }
                            KeyCode::PageDown => {
                                if !self.conversations.is_empty() {
                                    self.selected_index = (self.selected_index + visible_rows).min(self.conversations.len() - 1);
                                    if self.selected_index >= self.scroll_offset + visible_rows {
                                        self.scroll_offset = self.selected_index - visible_rows + 1;
                                    }
                                    needs_clear = true;
                                }
                            }
                            KeyCode::Home => {
                                if !self.conversations.is_empty() {
                                    self.selected_index = 0;
                                    self.scroll_offset = 0;
                                    needs_clear = true;
                                }
                            }
                            KeyCode::End => {
                                if !self.conversations.is_empty() {
                                    self.selected_index = self.conversations.len() - 1;
                                    self.scroll_offset = if self.conversations.len() > visible_rows {
                                        self.conversations.len() - visible_rows
                                    } else {
                                        0
                                    };
                                    needs_clear = true;
                                }
                            }
                            KeyCode::Enter => {
                                if !self.conversations.is_empty() {
                                    if let Some(summary) = self.conversations.get(self.selected_index) {
                                        return Ok(MenuResult::LoadConversation(summary.conversation_id.clone()));
                                    }
                                } else {
                                    // No conversations, treat Enter as back
                                    return Ok(MenuResult::BackToMain);
                                }
                            }
                            KeyCode::Char('d') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                                if !self.conversations.is_empty() {
                                    if let Some(summary) = self.conversations.get(self.selected_index) {
                                        let conversation_id = summary.conversation_id.clone();
                                        self.delete_conversation(&conversation_id)?;
                                        needs_clear = true;
                                    }
                                }
                            }
                            KeyCode::Char('n') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                                // Create new conversation
                                return Ok(MenuResult::NewConversation);
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                return Ok(MenuResult::BackToMain);
                            }
                            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(MenuResult::BackToMain);
                            }
                            _ => {}
                        }
                    }
                    Event::Resize(_, _) => {
                        // Continue loop to re-render
                        needs_clear = true;
                    }
                    _ => {}
                }
            }
        }
    }

    fn render(&self, visible_rows: usize, partial_update: bool) -> Result<()> {
        let (cols, rows) = terminal::size()?;

        let menu_width = 80.min(cols.saturating_sub(4));
        let menu_height = if self.conversations.is_empty() || self.is_loading {
            8  // Smaller for empty/loading state
        } else {
            visible_rows + 8  // Header + items + footer
        };
        let menu_height_u16 = menu_height as u16;

        // Center the menu
        let start_x = if cols > menu_width { (cols - menu_width) / 2 } else { 0 };
        let start_y = if rows > menu_height_u16 { (rows - menu_height_u16) / 2 } else { 0 };

        // Only draw box and title on full render
        if !partial_update {
            // Draw box
            self.draw_box(start_x, start_y, menu_width, menu_height_u16)?;

            // Draw title
            let title_y = start_y + 1;
            let title = "📚 Conversation History";
            let title_x = if menu_width > title.len() as u16 {
                start_x + (menu_width - title.len() as u16) / 2
            } else {
                start_x + 1
            };
            stdout()
                .queue(MoveTo(title_x, title_y))?
                .queue(SetForegroundColor(Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
                .queue(Print(style(title).bold()))?
                .queue(ResetColor)?;
        }

        if self.is_loading {
            // Show loading indicator with spinner - only update the spinner line
            let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let spinner = spinner_chars[(self.spinner_counter / 2) % spinner_chars.len()];

            let loading_y = start_y + 3;
            let loading_msg = format!("{} Loading conversations...", spinner);
            let loading_x = if menu_width > loading_msg.len() as u16 {
                start_x + (menu_width - loading_msg.len() as u16) / 2
            } else {
                start_x + 2
            };

            // Clear the line first with spaces to prevent artifacts
            let clear_line = " ".repeat(menu_width as usize - 4);
            stdout()
                .queue(MoveTo(start_x + 2, loading_y))?
                .queue(Print(&clear_line))?;

            stdout()
                .queue(MoveTo(loading_x, loading_y))?
                .queue(SetForegroundColor(Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
                .queue(Print(&loading_msg))?
                .queue(ResetColor)?;
        } else if self.conversations.is_empty() {
            // Empty state
            let empty_y = start_y + 3;
            let empty_msg = "No saved conversations found.";
            let empty_x = if menu_width > empty_msg.len() as u16 {
                start_x + (menu_width - empty_msg.len() as u16) / 2
            } else {
                start_x + 2
            };
            stdout()
                .queue(MoveTo(empty_x, empty_y))?
                .queue(SetForegroundColor(Color::Grey))?
                .queue(Print(empty_msg))?
                .queue(ResetColor)?;

            let hint_y = empty_y + 2;
            let hint_msg = "Start chatting to create your first conversation!";
            let hint_x = if menu_width > hint_msg.len() as u16 {
                start_x + (menu_width - hint_msg.len() as u16) / 2
            } else {
                start_x + 2
            };
            stdout()
                .queue(MoveTo(hint_x, hint_y))?
                .queue(SetForegroundColor(Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
                .queue(Print(hint_msg))?
                .queue(ResetColor)?;
        } else {
            // Draw status line
            let status_y = start_y + 3;
            let status = format!("Total: {} conversation{}",
                self.conversations.len(),
                if self.conversations.len() == 1 { "" } else { "s" }
            );
            let status_x = start_x + 2;
            stdout()
                .queue(MoveTo(status_x, status_y))?
                .queue(SetForegroundColor(Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
                .queue(Print(&status))?
                .queue(ResetColor)?;

            // Draw conversations
            let items_start_y = start_y + 5;
            let end_index = (self.scroll_offset + visible_rows).min(self.conversations.len());

            for (i, conv) in self.conversations[self.scroll_offset..end_index].iter().enumerate() {
                let actual_index = self.scroll_offset + i;
                let y = items_start_y + i as u16;
                let is_selected = actual_index == self.selected_index;

                // Format: [Relative Time] Title (N msgs, model)
                let relative_time = Self::format_relative_time(conv.updated_at);
                let line = format!(
                    "[{}] {} ({} msgs, {})",
                    relative_time, conv.title, conv.message_count, conv.model
                );

                // Truncate if too long
                let max_width = (menu_width as usize).saturating_sub(6);
                let display = if line.len() > max_width {
                    format!("{}...", &line[..max_width.saturating_sub(3)])
                } else {
                    line
                };

                if is_selected {
                    // Selected item with golden color
                    self.draw_selected_item(start_x + 2, y, menu_width - 4, &display)?;
                } else {
                    // Unselected item
                    stdout()
                        .queue(MoveTo(start_x + 4, y))?
                        .queue(SetForegroundColor(Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
                        .queue(Print(&display))?
                        .queue(ResetColor)?;
                }
            }

            // Show scroll indicator if needed
            if self.conversations.len() > visible_rows {
                let scroll_y = items_start_y + visible_rows as u16;
                let scroll_text = format!("Showing {}-{} of {}",
                    self.scroll_offset + 1,
                    end_index,
                    self.conversations.len()
                );
                let scroll_x = start_x + 2;
                stdout()
                    .queue(MoveTo(scroll_x, scroll_y))?
                    .queue(SetForegroundColor(Color::Grey))?
                    .queue(Print(&scroll_text))?
                    .queue(ResetColor)?;
            }
        }

        // Draw help text
        let help_y = start_y + menu_height_u16 - 1;
        let help_text = if self.is_loading {
            "Loading... • ESC Back"
        } else if self.conversations.is_empty() {
            "ESC/Enter Back"
        } else {
            "↑↓ Navigate • Enter Load • Ctrl+D Delete • Ctrl+N New • ESC Back"
        };
        let help_len = help_text.len() as u16;
        let help_x = if menu_width > help_len + 2 {
            start_x + menu_width / 2 - help_len / 2
        } else {
            start_x + 1
        };
        stdout()
            .queue(MoveTo(help_x, help_y))?
            .queue(SetForegroundColor(Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
            .queue(Print(help_text))?
            .queue(ResetColor)?;

        stdout().flush()?;
        Ok(())
    }

    fn draw_box(&self, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
        let border_color = Color::AnsiValue(crate::utils::colors::MISC_ANSI);

        // Top border
        stdout()
            .queue(MoveTo(x, y))?
            .queue(SetForegroundColor(border_color))?
            .queue(Print("╭"))?;
        for _ in 0..(width - 2) {
            stdout().queue(Print("─"))?;
        }
        stdout().queue(Print("╮"))?;

        // Side borders
        for i in 1..height - 1 {
            stdout()
                .queue(MoveTo(x, y + i))?
                .queue(Print("│"))?
                .queue(MoveTo(x + width - 1, y + i))?
                .queue(Print("│"))?;
        }

        // Bottom border
        stdout()
            .queue(MoveTo(x, y + height - 1))?
            .queue(Print("╰"))?;
        for _ in 0..(width - 2) {
            stdout().queue(Print("─"))?;
        }
        stdout()
            .queue(Print("╯"))?
            .queue(ResetColor)?;

        Ok(())
    }

    fn draw_selected_item(&self, x: u16, y: u16, _width: u16, text: &str) -> Result<()> {
        stdout()
            .queue(MoveTo(x, y))?
            .queue(SetForegroundColor(Color::AnsiValue(crate::utils::colors::PRIMARY_ANSI)))?
            .queue(Print("▶ "))?
            .queue(Print(text))?
            .queue(ResetColor)?;
        Ok(())
    }

    fn delete_conversation(&mut self, conversation_id: &str) -> Result<()> {
        // Show confirmation at bottom of menu
        let (_cols, rows) = terminal::size()?;
        let confirm_y = rows - 2;
        let confirm_x = 2;

        stdout()
            .queue(MoveTo(confirm_x, confirm_y))?
            .queue(SetForegroundColor(Color::Red))?
            .queue(Print("⚠️  Delete this conversation? (y/N): "))?
            .queue(ResetColor)?;
        stdout().flush()?;

        // Wait for confirmation
        if let Event::Key(key) = event::read()? {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
                let current_dir = std::env::current_dir()?;
                Conversation::delete(&current_dir, conversation_id)?;

                // Reload conversation list
                self.conversations = Conversation::list_all(&current_dir)?;

                // Adjust selection if needed
                if self.selected_index >= self.conversations.len() && self.selected_index > 0 {
                    self.selected_index = self.conversations.len().saturating_sub(1);
                }
                if self.scroll_offset > self.selected_index {
                    self.scroll_offset = self.selected_index;
                }
            }
        }

        Ok(())
    }
}

impl Default for ConversationMenu {
    fn default() -> Self {
        Self::new()
    }
}
