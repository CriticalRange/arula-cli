//! Main menu functionality for ARULA CLI

use crate::app::App;
use crate::ui::output::OutputHandler;
use crate::utils::colors::ColorTheme;
use crate::ui::menus::common::{MenuResult, MenuUtils, MenuState};
use anyhow::Result;
use crossterm::{
    event::{Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
    cursor::MoveTo,
    style::{SetForegroundColor, ResetColor, Print},
    ExecutableCommand, QueueableCommand,
};
use std::io::{stdout, Write};
use std::time::Duration;
use serde_json::Value;
use nu_ansi_term::Color;

/// Tool call delay to prevent API rate limiting (in seconds) - made more reasonable
const TOOL_CALL_DELAY_SECS: u64 = 2;

/// Maximum lines to read from a file to prevent API failures
const MAX_FILE_LINES: u32 = 100;

/// Maximum file size to attempt reading (in characters)
const MAX_FILE_SIZE_CHARS: u32 = 50000; // ~50KB

/// Maximum time to wait for AI to respond after an error (in seconds)
const ERROR_RECOVERY_TIMEOUT_SECS: u64 = 30;

/// Format tool call with icon and human-readable description (copied from app.rs)
fn format_tool_call(tool_name: &str, arguments: &str) -> String {
    // Parse arguments to extract key information
    let args: Result<Value, _> = serde_json::from_str(arguments);

    let (icon, description) = match tool_name {
        "list_directory" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            ("📂", format!("Listing directory: {}", path))
        },
        "read_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");

            let max_lines = args.as_ref()
                .ok()
                .and_then(|v| v.get("max_lines"))
                .and_then(|m| m.as_u64())
                .unwrap_or(u64::MAX);

            if max_lines < u64::MAX {
                ("📖", format!("Reading file: {} (limited to {} lines)", path, max_lines))
            } else {
                ("📖", format!("Reading file: {}", path))
            }
        },
        "write_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            ("✍️", format!("Writing file: {}", path))
        },
        "edit_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            ("✏️", format!("Editing file: {}", path))
        },
        "execute_bash" => {
            let command = args.as_ref()
                .ok()
                .and_then(|v| v.get("command"))
                .and_then(|c| c.as_str())
                .unwrap_or("unknown");
            // Truncate long commands
            let display_cmd = if command.len() > 50 {
                format!("{}...", &command[..47])
            } else {
                command.to_string()
            };
            ("⚙️", format!("Running: {}", display_cmd))
        },
        "search_files" => {
            let query = args.as_ref()
                .ok()
                .and_then(|v| v.get("query"))
                .and_then(|q| q.as_str())
                .unwrap_or("unknown");
            ("🔍", format!("Searching for: {}", query))
        },
        "web_search" => {
            let query = args.as_ref()
                .ok()
                .and_then(|v| v.get("query"))
                .and_then(|q| q.as_str())
                .unwrap_or("unknown");
            ("🌐", format!("Web search: {}", query))
        },
        _ => ("🔧", format!("Running tool: {}", tool_name))
    };

    // Format with loading spinner and colored description
    format!("{} {}",
        Color::Cyan.paint(icon),
        Color::White.dimmed().paint(description)
    )
}

/// Summarize tool result in a human-readable format (copied from app.rs)
fn summarize_tool_result(result_value: &Value) -> String {
    // Check for error in Err wrapper first (e.g., {"Err": "error message"})
    if let Some(err_value) = result_value.get("Err") {
        if let Some(err_str) = err_value.as_str() {
            return format!("Error: {}", err_str);
        }
        // If Err value is not a string, show it as JSON
        return format!("Error: {}", serde_json::to_string_pretty(err_value).unwrap_or_else(|_| err_value.to_string()));
    }

    // Try to parse as our standard tool result format
    if let Some(ok_result) = result_value.get("Ok") {
        // list_directory results
        if let Some(entries) = ok_result.get("entries") {
            if let Some(arr) = entries.as_array() {
                let files = arr.iter().filter(|e| e.get("file_type").and_then(|t| t.as_str()) == Some("file")).count();
                let dirs = arr.iter().filter(|e| e.get("file_type").and_then(|t| t.as_str()) == Some("directory")).count();
                return format!("Found {} files and {} directories", files, dirs);
            }
        }

        // execute_bash results
        if let Some(stdout) = ok_result.get("stdout") {
            if let Some(stdout_str) = stdout.as_str() {
                let stderr = ok_result.get("stderr").and_then(|s| s.as_str()).unwrap_or("");
                let exit_code = ok_result.get("exit_code").and_then(|c| c.as_i64()).unwrap_or(0);

                if exit_code == 0 {
                    if !stdout_str.trim().is_empty() {
                        return format!("Command succeeded:\n{}", stdout_str.trim());
                    } else {
                        return "Command succeeded (no output)".to_string();
                    }
                } else {
                    return format!("Command failed (exit code {}):\n{}", exit_code, stderr);
                }
            }
        }

        // read_file results
        if let Some(lines) = ok_result.get("lines") {
            return format!("Read {} lines", lines);
        }

        // write_file/edit_file results
        if let Some(message) = ok_result.get("message") {
            if let Some(msg_str) = message.as_str() {
                return msg_str.to_string();
            }
        }

        // search_files results
        if let Some(total_matches) = ok_result.get("total_matches") {
            let files_searched = ok_result.get("files_searched").and_then(|f| f.as_i64()).unwrap_or(0);
            return format!("Found {} matches in {} files", total_matches, files_searched);
        }

        // Generic success with success flag
        if ok_result.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            return "Success".to_string();
        }
    }

    // Fallback: show compact JSON
    serde_json::to_string_pretty(result_value).unwrap_or_else(|_| result_value.to_string())
}

/// Main menu options
#[derive(Debug, Clone)]
pub enum MainMenuItem {
    ContinueChat,
    Conversations,
    ContinuousMode,
    Settings,
    InfoHelp,
    ClearChat,
}

impl MainMenuItem {
    pub fn all() -> Vec<Self> {
        vec![
            MainMenuItem::ContinueChat,
            MainMenuItem::Conversations,
            MainMenuItem::ContinuousMode,
            MainMenuItem::Settings,
            MainMenuItem::InfoHelp,
            MainMenuItem::ClearChat,
        ]
    }

    pub fn label(&self) -> &str {
        match self {
            MainMenuItem::ContinueChat => "⦿ Continue Chat",
            MainMenuItem::Conversations => "📚 Conversations",
            MainMenuItem::ContinuousMode => "🔄 Continuous Mode",
            MainMenuItem::Settings => "⚙ Configuration",
            MainMenuItem::InfoHelp => "ℹ Info & Help",
            MainMenuItem::ClearChat => "Ⓒ Clear Chat",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            MainMenuItem::ContinueChat => "Return to conversation",
            MainMenuItem::Conversations => "View, load, or manage saved conversations",
            MainMenuItem::ContinuousMode => "Start AI-powered continuous project improvement",
            MainMenuItem::Settings => "Configure AI provider and configuration",
            MainMenuItem::InfoHelp => "View help and session information",
            MainMenuItem::ClearChat => "Clear conversation history",
        }
    }
}

/// Main menu handler
pub struct MainMenu {
    state: MenuState,
    items: Vec<MainMenuItem>,
}

impl MainMenu {
    pub fn new() -> Self {
        Self {
            state: MenuState::new(),
            items: MainMenuItem::all(),
        }
    }

    /// Display and handle the main menu
    pub fn show(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<MenuResult> {
        // Check terminal size
        if !MenuUtils::check_terminal_size(30, 8)? {
            output.print_system("Terminal too small for menu")?;
            return Ok(MenuResult::Continue);
        }

        // Setup terminal
        MenuUtils::setup_terminal()?;

        let result = self.run_menu_loop(app, output);

        // Restore terminal
        MenuUtils::restore_terminal()?;

        result
    }

    /// Main menu event loop (original implementation pattern)
    fn run_menu_loop(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<MenuResult> {
        // Comprehensive event clearing to prevent issues
        std::thread::sleep(Duration::from_millis(50));
        for _ in 0..5 {
            while crossterm::event::poll(Duration::from_millis(0))? {
                let _ = crossterm::event::read()?;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // Track state for selective rendering
        let mut last_selected_index = self.state.selected_index;
        let mut needs_render = true; // Render first time

        loop {
            // Only render if state changed
            if needs_render || last_selected_index != self.state.selected_index {
                self.render(output)?;
                last_selected_index = self.state.selected_index;
                needs_render = false;
            }

            // Wait for input event with timeout
            if crossterm::event::poll(Duration::from_millis(100))? {
                match crossterm::event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing
                        if key_event.kind != crossterm::event::KeyEventKind::Press {
                            continue;
                        }

                        match key_event.code {
                            crossterm::event::KeyCode::Up => {
                                self.state.move_up(self.items.len());
                                needs_render = true;
                            }
                            crossterm::event::KeyCode::Down => {
                                self.state.move_down(self.items.len());
                                needs_render = true;
                            }
                            crossterm::event::KeyCode::Enter => {
                                return self.handle_selection(app, output);
                            }
                            crossterm::event::KeyCode::Esc => {
                                // Clear screen before exiting
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                return Ok(MenuResult::Continue);
                            }
                            crossterm::event::KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                                // Clear screen before exiting
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                // Ctrl+C - close menu
                                return Ok(MenuResult::Continue);
                            }
                            _ => {}
                        }
                    }
                    Event::Resize(_, _) => {
                        // Re-render on resize
                        needs_render = true;
                    }
                    _ => {
                        // Ignore all other event types
                        continue;
                    }
                }
            }
        }
    }

    /// Render the main menu with original styling (1:1 from original overlay_menu.rs)
    fn render(&self, _output: &mut OutputHandler) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;
        let menu_width = 50.min(cols.saturating_sub(4));
        let menu_height = 10;
        let start_x = if cols > menu_width { (cols - menu_width) / 2 } else { 0 };
        let start_y = if rows > menu_height { (rows - menu_height) / 2 } else { 0 };

        // Don't clear screen on every render - we're in alternate screen mode
        // Only position cursor at top
        stdout().execute(crossterm::cursor::MoveTo(0, 0))?;

        // Draw modern box using original styling
        self.draw_modern_box(start_x, start_y, menu_width, menu_height, "ARULA")?;

        // Draw title with modern styling
        let title_y = start_y + 1;
        let title = "● MENU";
        let title_len = title.len() as u16;
        let title_x = if menu_width > title_len + 2 {
            start_x + menu_width / 2 - title_len / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(title_x, title_y))?
              .queue(Print(ColorTheme::primary().bold().apply_to(title)))?;

        // Draw menu items with modern styling
        let items_start_y = start_y + 3;
        for (i, item) in self.items.iter().enumerate() {
            let y = items_start_y + i as u16;

            if i == self.state.selected_index {
                // Selected item with modern highlight
                self.draw_selected_item(start_x + 2, y, menu_width - 4, item.label())?;
            } else {
                // Unselected item - clear the line first to remove any previous selection background
                stdout().queue(MoveTo(start_x + 2, y))?;
                for _ in 0..(menu_width.saturating_sub(4)) {
                    stdout().queue(Print(" "))?;
                }
                // Then draw the text with truncation
                let max_text_width = menu_width.saturating_sub(6) as usize; // padding for margins
                let display_text = MenuUtils::truncate_text(item.label(), max_text_width);
                stdout().queue(MoveTo(start_x + 4, y))?
                      .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
                      .queue(Print(display_text))?
                      .queue(ResetColor)?;
            }
        }

        // Draw modern help text (intercepting box border - left aligned)
        let help_y = start_y + menu_height - 1;
        let help_text = "↑↓ Navigate • Enter Select • ESC Exit";
        let max_help_width = menu_width.saturating_sub(4) as usize;
        let display_help = MenuUtils::truncate_text(help_text, max_help_width);
        let help_x = start_x + 2; // Left aligned with padding
        stdout().queue(MoveTo(help_x, help_y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(display_help))?
              .queue(ResetColor)?;

        stdout().flush()?;
        Ok(())
    }

    /// Draw modern box with rounded corners (original function)
    fn draw_modern_box(&self, x: u16, y: u16, width: u16, height: u16, _title: &str) -> Result<()> {
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
        stdout().queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?;

        // Draw vertical borders
        for i in 0..height {
            stdout().queue(MoveTo(x, y + i))?.queue(Print(vertical))?;
            stdout().queue(MoveTo(x + width.saturating_sub(1), y + i))?.queue(Print(vertical))?;
        }

        // Top border
        stdout().queue(MoveTo(x, y))?.queue(Print(top_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(top_right))?;

        // Bottom border
        stdout().queue(MoveTo(x, y + height.saturating_sub(1)))?.queue(Print(bottom_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(bottom_right))?;

        stdout().queue(ResetColor)?;
        Ok(())
    }

    /// Draw selected item (NO background) - matching other menus
    fn draw_selected_item(&self, x: u16, y: u16, width: u16, text: &str) -> Result<()> {
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
            let char_end = text.char_indices().nth(safe_len)
                .map(|(idx, _)| idx)
                .unwrap_or(text.len());
            format!("▶ {}...", &text[..char_end])
        } else {
            display_text
        };

        stdout().queue(MoveTo(x + 2, y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::PRIMARY_ANSI)))?
              .queue(Print(safe_text))?
              .queue(ResetColor)?;

        Ok(())
    }

    /// Handle keyboard input (legacy - no longer used in main loop)
    #[allow(dead_code)]
    fn handle_input(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<MenuResult> {
        while let Some(key_event) = MenuUtils::read_key_event()? {
            match key_event.code {
                KeyCode::Up => {
                    self.state.move_up(self.items.len());
                }
                KeyCode::Down => {
                    self.state.move_down(self.items.len());
                }
                KeyCode::Enter => {
                    return self.handle_selection(app, output);
                }
                KeyCode::Esc => {
                    return Ok(MenuResult::Continue);
                }
                _ => {}
            }
        }
        Ok(MenuResult::Continue)
    }

    /// Handle selection from main menu
    pub fn handle_selection(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<MenuResult> {
        if let Some(selected_item) = self.items.get(self.state.selected_index) {
            match selected_item {
                MainMenuItem::ContinueChat => {
                    // Clear screen before exiting
                    stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                    stdout().flush()?;
                    Ok(MenuResult::Continue)
                }
                MainMenuItem::Conversations => {
                    // Show conversation selector submenu
                    use crate::ui::menus::ConversationMenu;
                    let mut conversation_menu = ConversationMenu::new();
                    let result = conversation_menu.show(app, output)?;

                    // Return the result from conversation menu (could be LoadConversation, NewConversation, or BackToMain)
                    Ok(result)
                }
                MainMenuItem::ContinuousMode => {
                    // Handle Continuous Mode activation
                    use futures::executor::block_on;

                    // Block on async operation in sync context
                    let result = block_on(async {
                        self.handle_continuous_mode(app, output).await
                    });

                    match result {
                        Ok(_) => {
                            // Clear screen before exiting
                            stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                            stdout().flush()?;
                            Ok(MenuResult::Continue)
                        }
                        Err(e) => {
                            output.print_error(&format!("Continuous Mode error: {}", e))?;
                            // Clear screen before exiting
                            stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                            stdout().flush()?;
                            Ok(MenuResult::Continue)
                        }
                    }
                }
                MainMenuItem::Settings => {
                    // Clear screen before exiting
                    stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                    stdout().flush()?;
                    Ok(MenuResult::Settings)
                }
                MainMenuItem::InfoHelp => {
                    self.show_info_and_help(app, output)?;
                    // Clear screen before exiting
                    stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                    stdout().flush()?;
                    Ok(MenuResult::Continue)
                }
                MainMenuItem::ClearChat => {
                    // Clear screen before exiting
                    stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                    stdout().flush()?;
                    Ok(MenuResult::ClearChat)
                }
            }
        } else {
            // Clear screen before exiting
            stdout().execute(terminal::Clear(terminal::ClearType::All))?;
            stdout().flush()?;
            Ok(MenuResult::Continue)
        }
    }

    /// Show information and help dialog (original implementation)
    fn show_info_and_help(&self, _app: &App, _output: &mut OutputHandler) -> Result<()> {
        // Clear screen once when entering submenu to avoid artifacts
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;

        // Clear any pending events in the buffer
        while crossterm::event::poll(Duration::from_millis(0))? {
            let _ = crossterm::event::read()?;
        }

        let mut scroll_offset = 0;

        loop {
            self.render_help(scroll_offset)?;

            if crossterm::event::poll(Duration::from_millis(100))? {
                match crossterm::event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        match key_event.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                if scroll_offset > 0 {
                                    scroll_offset -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                // Get help content and calculate max scroll
                                let help_lines = self.get_help_content();
                                let menu_height = 22u16;
                                let content_height = (menu_height - 5) as usize; // Space for content display
                                let max_scroll = help_lines.len().saturating_sub(content_height);

                                if scroll_offset < max_scroll {
                                    scroll_offset += 1;
                                }
                            }
                            KeyCode::PageUp => {
                                scroll_offset = scroll_offset.saturating_sub(5);
                            }
                            KeyCode::PageDown => {
                                let help_lines = self.get_help_content();
                                let menu_height = 22u16;
                                let content_height = (menu_height - 5) as usize;
                                let max_scroll = help_lines.len().saturating_sub(content_height);

                                scroll_offset = (scroll_offset + 5).min(max_scroll);
                            }
                            KeyCode::Home => {
                                scroll_offset = 0;
                            }
                            KeyCode::End => {
                                let help_lines = self.get_help_content();
                                let menu_height = 22u16;
                                let content_height = (menu_height - 5) as usize;
                                scroll_offset = help_lines.len().saturating_sub(content_height);
                            }
                            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
                                break;
                            }
                            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                                // Ctrl+C - close help dialog
                                break;
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                    Event::Resize(_, _) => {
                        // Re-render on resize
                        continue;
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }
        Ok(())
    }

    /// Get help content (original implementation)
    fn get_help_content(&self) -> Vec<String> {
        vec![
            "🔧 Commands:",
            "  /help     - Show this help",
            "  /menu     - Open interactive menu",
            "  /clear    - Clear conversation history",
            "  /config   - Show current configuration",
            "  /model <name> - Change AI model",
            "  exit or quit - Exit ARULA",
            "",
            "⌨️  Keyboard Shortcuts:",
            "  Ctrl+C    - Exit menu",
            "  m         - Open menu",
            "  Ctrl+D    - Exit",
            "  Up/Down   - Navigate command history",
            "",
            "💡 Tips:",
            "  • End line with \\ to continue on next line",
            "  • Ask ARULA to execute bash commands",
            "  • Use natural language",
            "  • Native terminal scrollback works!",
            "",
            "🛠️  Available Tools:",
            "  • execute_bash - Run shell commands",
            "  • read_file - Read file contents",
            "  • write_file - Create or overwrite files",
            "  • edit_file - Edit existing files",
            "  • list_directory - Browse directories",
            "  • search_files - Fast parallel search",
            "  • visioneer - Desktop automation",
        ].iter().map(|s| s.to_string()).collect()
    }

    /// Render help dialog (original implementation)
    fn render_help(&self, scroll_offset: usize) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;

        // Don't clear entire screen - causes flicker
        // We're in alternate screen mode, so just draw over existing content

        let menu_width = 70.min(cols.saturating_sub(4));
        let menu_height = 22u16; // Increased for header and footer
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2;

        self.draw_modern_box(start_x, start_y, menu_width, menu_height, "HELP")?;

        // Draw title/header
        let title_y = start_y + 1;
        let title = "ARULA Info & Help";
        let title_x = if menu_width > title.len() as u16 {
            start_x + (menu_width - title.len() as u16) / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(title_x, title_y))?
              .queue(Print(ColorTheme::primary().bold().apply_to(title)))?;

        // Get all help content
        let help_lines = self.get_help_content();

        // Calculate visible area
        let content_height = (menu_height - 5) as usize; // Reserve space for title, border, and footer
        let visible_lines: Vec<&str> = help_lines
            .iter()
            .skip(scroll_offset)
            .take(content_height)
            .map(|s| s.as_str())
            .collect();

        // Draw visible lines
        for (i, line) in visible_lines.iter().enumerate() {
            let y = start_y + 3 + i as u16;

            // Use different colors for different sections
            let color = if line.starts_with("🔧") || line.starts_with("⌨️") || line.starts_with("💡") || line.starts_with("🛠️") || line.starts_with("📊") {
                SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI))
            } else if line.starts_with("  •") {
                SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI))
            } else {
                SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI))
            };

            // Clear the line first to remove any previous content
            stdout().queue(MoveTo(start_x + 2, y))?;
            for _ in 0..(menu_width.saturating_sub(4)) {
                stdout().queue(Print(" "))?;
            }

            // Draw the text
            stdout().queue(MoveTo(start_x + 2, y))?
                  .queue(color)?
                  .queue(Print(*line))?
                  .queue(ResetColor)?;
        }

        // Clear any remaining lines if content is shorter than viewport
        for i in visible_lines.len()..content_height {
            let y = start_y + 3 + i as u16;
            stdout().queue(MoveTo(start_x + 2, y))?;
            for _ in 0..(menu_width.saturating_sub(4)) {
                stdout().queue(Print(" "))?;
            }
        }

        // Draw footer with dynamic scroll indicator (centered, intercepting box border)
        let footer_y = start_y + menu_height - 1;
        let max_scroll = help_lines.len().saturating_sub(content_height);

        // Determine scroll indicator text for footer
        let scroll_part = if max_scroll == 0 {
            "".to_string()
        } else if scroll_offset == 0 {
            "⬇ More".to_string()
        } else if scroll_offset >= max_scroll {
            "⬆ Top".to_string()
        } else {
            format!("↑↓ {}/{}", scroll_offset + 1, max_scroll + 1)
        };

        // Build navigation text with scroll indicator
        let nav_text = if scroll_part.is_empty() {
            "↵ Continue • Esc Back".to_string()
        } else {
            format!("{} • ↵ Continue • Esc Back", scroll_part)
        };

        // Left aligned with padding
        let nav_x = start_x + 2;

        stdout().queue(MoveTo(nav_x, footer_y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(nav_text))?
              .queue(ResetColor)?;

        stdout().flush()?;
        Ok(())
    }

    /// Reset menu state
    pub fn reset(&mut self) {
        self.state.reset();
    }

    /// Get current selected index
    pub fn selected_index(&self) -> usize {
        self.state.selected_index
    }

    /// Handle Continuous Mode activation
    async fn handle_continuous_mode(&self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        use crate::ui::menus::dialogs::Dialogs;

        // Clear screen once when entering continuous mode
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;

        // Clear any pending events in the buffer
        while crossterm::event::poll(Duration::from_millis(0))? {
            let _ = crossterm::event::read()?;
        }

        let dialogs = Dialogs::new();
        let confirmation_result = dialogs.confirm_dialog("Start Continuous Mode?\n\nThis will create a new git branch and analyze your project for improvements.", output)?;

        if confirmation_result {
            // Start Continuous Mode
            output.print_system("🔄 Initializing Continuous Mode...")?;

            // Create git branch
            match self.create_continuous_mode_branch(output) {
                Ok(branch_name) => {
                    output.print_system(&format!("📂 Created and switched to branch: {}", branch_name))?;

                    // Start AI continuous analysis in background
                    output.print_system("🤖 Starting AI project analysis...")?;

                    // Start the continuous improvement loop
                    if let Err(e) = self.start_continuous_improvement_loop(app, output).await {
                        output.print_error(&format!("Continuous Mode error: {}", e))?;
                    } else {
                        output.print_system("🔄 Continuous Mode completed")?;
                    }
                }
                Err(e) => {
                    output.print_error(&format!("Failed to create git branch: {}", e))?;
                }
            }
        } else {
            output.print_system("Continuous Mode cancelled")?;
        }

        Ok(())
    }

    
    /// Create and switch to a new git branch for Continuous Mode
    fn create_continuous_mode_branch(&self, _output: &OutputHandler) -> Result<String> {
        use std::process::Command;

        // Generate branch name with timestamp
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let branch_name = format!("continuous-mode-{}", timestamp);

        // Check if we're in a git repository
        let git_check = Command::new("git")
            .args(&["rev-parse", "--git-dir"])
            .output();

        match git_check {
            Ok(check_output) if check_output.status.success() => {
                // We're in a git repository
                // Create and checkout new branch
                let result = Command::new("git")
                    .args(&["checkout", "-b", &branch_name])
                    .output();

                match result {
                    Ok(branch_output) if branch_output.status.success() => {
                        Ok(branch_name)
                    }
                    Ok(branch_output) => {
                        let error_msg = String::from_utf8_lossy(&branch_output.stderr);
                        Err(anyhow::anyhow!("Failed to create branch: {}", error_msg))
                    }
                    Err(e) => Err(anyhow::anyhow!("Failed to execute git command: {}", e))
                }
            }
            Ok(_) | Err(_) => {
                Err(anyhow::anyhow!("Not in a git repository. Continuous Mode requires git version control."))
            }
        }
    }

    /// Start the continuous improvement loop
    async fn start_continuous_improvement_loop(&self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let mut iteration_count = 0;
        const MAX_ITERATIONS: u32 = 50; // Allow for many incremental improvements

        // Initial analysis prompt
        let initial_prompt = r#"You are now in Continuous Mode with RESEARCH-ENABLED iterative improvement. Your task is to analyze this codebase and create a plan for incremental improvements through online research and best practices validation.

Please perform the following analysis:

1. **Component Mapping**: Identify all major components, modules, and files in the project
2. **Technology Stack Assessment**: What frameworks, libraries, and patterns are used?
3. **Documentation Gaps**: What components lack proper documentation or comments?
4. **Best Practices Research Plan**: For each component, identify what needs online research
5. **Improvement Roadmap**: Create a prioritized list of small, achievable improvements

Focus on creating a roadmap for 20+ small iterations. Each iteration should:
- Research ONE specific aspect online
- Make ONE small, focused improvement
- Build incrementally on previous changes

Provide a comprehensive component-by-component analysis with specific research areas for each part of the codebase."#;

        output.print_system("🔍 Starting initial project analysis...")?;

        // Send initial analysis
        app.track_user_message(initial_prompt);
        app.send_to_ai(initial_prompt).await?;

        // Wait for initial analysis to complete
        self.wait_for_ai_completion(app, output).await?;

        // Start the continuous improvement loop
        loop {
            iteration_count += 1;

            if iteration_count > MAX_ITERATIONS {
                output.print_system("⚠️ Reached maximum iterations for safety. Stopping Continuous Mode.")?;
                break;
            }

            output.print_system(&format!("🔬 Iteration {} - Researching best practices & making incremental improvement...", iteration_count))?;

            // Ask AI what to improve next
            let followup_prompt = r#"Based on your previous analysis, perform ONE research-driven incremental improvement:

**STEP 1 - RESEARCH**: Use web search tools to research current best practices for ONE specific component/technology in this codebase. Examples:
- "Rust async patterns 2024" for async code
- "Error handling best practices Rust" for error management
- "Testing patterns [framework_name]" for test files
- "Documentation standards [language]" for documentation
- "Security best practices [technology]" for security aspects

**STEP 2 - ANALYZE**: Compare current implementation with researched best practices by:
- Reading relevant source files to understand current implementation
- Checking documentation and comments
- Identifying specific areas for improvement

**STEP 3 - IMPROVE**: Make ONE small, focused improvement based on research:
- Add better error handling to ONE function
- Improve documentation for ONE module
- Add ONE missing test case
- Refactor ONE complex function slightly
- Add ONE security improvement
- Update ONE import/dependency

**IMPORTANT**: Continue using tools as needed throughout this process. Research thoroughly, analyze files, then make your improvement. Don't stop after the first tool call - keep going until you complete all three steps.

**RULES**:
- Only change 1-2 lines or add 1 small function per iteration
- Research BEFORE making changes
- Explain what you researched and why it matters
- Build incrementally - don't try to fix everything at once
- Continue using tools throughout the entire process

If after extensive research you believe this codebase follows current best practices well and no small incremental improvements remain, respond with "CODEBASE_OPTIMIZED" and explain your research findings."#;

            app.track_user_message(&followup_prompt);
            app.send_to_ai(&followup_prompt).await?;

            // Wait for AI to complete this iteration
            let completion_result = self.wait_for_ai_completion_with_check(app, output).await?;

            match completion_result {
                AICompletionResult::Optimized => {
                    output.print_system("✅ AI indicates codebase is optimized")?;
                    break;
                }
                AICompletionResult::Continue => {
                    output.print_system(&format!("✅ Iteration {} completed - Incremental improvement applied", iteration_count))?;
                    // Small delay between iterations
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                }
                AICompletionResult::Error(e) => {
                    output.print_error(&format!("AI iteration failed: {}", e))?;
                    break;
                }
            }
        }

        output.print_system(&format!("🏁 Continuous Mode completed after {} iterations", iteration_count))?;
        Ok(())
    }

    /// Wait for AI to complete its response with basic timeout
    async fn wait_for_ai_completion(&self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let mut timeout_counter = 0;
        const MAX_TIMEOUT: u32 = 1200; // 2 minutes max wait for initial analysis
        let mut last_activity = std::time::Instant::now();
        let mut has_seen_activity = false;

        while timeout_counter < MAX_TIMEOUT {
            if let Some(response) = app.check_ai_response_nonblocking() {
                last_activity = std::time::Instant::now();
                has_seen_activity = true;

                match response {
                    crate::app::AiResponse::AgentStreamEnd => {
                        output.print_system("✅ AI response completed")?;
                        return Ok(());
                    }
                    crate::app::AiResponse::AgentStreamText(chunk) => {
                        // Show AI analysis messages (but not too verbose)
                        if chunk.contains("analysis") ||
                           chunk.contains("research") ||
                           chunk.contains("found") ||
                           chunk.contains("improvement") ||
                           chunk.contains("component") ||
                           (chunk.len() > 20 && !chunk.starts_with(' ') && !chunk.starts_with('\n')) {
                            // Show meaningful AI messages
                            if chunk.trim().len() > 0 {
                                output.print_system(&format!("💭 AI: {}", chunk.trim().to_string()))?;
                            }
                        }
                        // Note: We don't track content in the initial analysis function
                    }
                    crate::app::AiResponse::AgentToolCall { id: _, name, arguments } => {
                        // Modify read_file calls to limit lines to prevent API failures
                        let modified_arguments = if name == "read_file" {
                            self.limit_read_file_lines(&arguments)
                        } else {
                            arguments.clone()
                        };

                        // Use the same formatting as the main app
                        let tool_display = format_tool_call(&name, &modified_arguments);
                        output.print_system(&tool_display)?;

                        // Add delay between tool calls to prevent rate limiting with progress indicator
                        output.print_system(&format!("⏳ Waiting {} seconds to prevent API rate limiting...", TOOL_CALL_DELAY_SECS))?;
                        for i in 1..=TOOL_CALL_DELAY_SECS {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            let remaining = TOOL_CALL_DELAY_SECS - i;
                            if remaining > 0 {
                                output.print_system(&format!("⏳ Rate limit delay: {}s remaining", remaining))?;
                            }
                        }
                    }
                    crate::app::AiResponse::AgentToolResult { tool_call_id, success, result } => {
                        // Use the same formatting as the main app
                        let status = if success { "✅" } else { "❌" };
                        let summary = summarize_tool_result(&result);
                        output.print_system(&format!("  {} {} [{}]", status, summary, tool_call_id))?;

                        // If there's an error, start a timeout counter to detect if AI gets stuck
                        if !success {
                            output.print_system("⚠️ Tool failed - watching for AI recovery...")?;
                            match self.wait_for_ai_recovery_after_error(app, output).await {
                                Ok(_) => {
                                    output.print_system("✅ Recovery completed - continuing")?;
                                    return Ok(());
                                }
                                Err(e) => {
                                    output.print_error(&format!("Recovery failed: {}", e))?;
                                    return Ok(());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // If we've seen activity but it's been more than 30 seconds since last activity, show progress
            if has_seen_activity && last_activity.elapsed().as_secs() > 30 {
                output.print_system(&format!("⏳ AI working... ({}s since last activity)", last_activity.elapsed().as_secs()))?;
                last_activity = std::time::Instant::now(); // Reset to avoid spam
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await; // Slightly longer sleep
            timeout_counter += 1;

            // Show progress every 10 seconds if no activity
            if timeout_counter % 50 == 0 && !has_seen_activity {
                output.print_system(&format!("⏳ Waiting for AI response... ({}s)", (timeout_counter * 200) / 1000))?;
            }
        }

        output.print_system("⚠️ AI response timeout - you can press Ctrl+C to stop Continuous Mode")?;

        // Brief pause to allow user interruption if needed
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        Ok(())
    }

    /// Wait for AI completion and check for optimization signal
    async fn wait_for_ai_completion_with_check(&self, app: &mut App, output: &mut OutputHandler) -> Result<AICompletionResult> {
        let mut timeout_counter = 0;
        const MAX_TIMEOUT: u32 = 600; // 1 minute max per iteration to prevent hanging
        let mut last_ai_content = String::new();
        let mut last_activity = std::time::Instant::now();
        let mut tool_count = 0;
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 3; // Stop after 3 consecutive errors

        while timeout_counter < MAX_TIMEOUT {
            if let Some(response) = app.check_ai_response_nonblocking() {
                last_activity = std::time::Instant::now();
                consecutive_errors = 0; // Reset error counter on successful response

                match response {
                    crate::app::AiResponse::AgentStreamEnd => {
                        // Enhanced debug output for stream completion
                        if std::env::var("ARULA_DEBUG").is_ok() {
                            let content_preview = if last_ai_content.len() > 200 {
                                format!("{}...", &last_ai_content[..200])
                            } else {
                                last_ai_content.clone()
                            };
                            output.print_system(&format!("🔧 AI Stream End - Final content: '{}'", content_preview))?;
                        }

                        // Check for EXPLICIT signals to stop continuous mode ONLY
                        let content_lower = last_ai_content.to_lowercase();

                        // Very specific stop signals that explicitly mention stopping continuous mode
                        let explicit_stop_signals = [
                            "stop continuous mode",
                            "continuous mode should stop",
                            "stopping continuous mode",
                            "end continuous mode",
                            "terminate continuous mode",
                            "continuous mode complete",
                        ];

                        // Explicit optimization signal
                        let has_codebase_optimized = last_ai_content.contains("CODEBASE_OPTIMIZED");

                        // Only stop for these very specific signals
                        let has_explicit_stop = explicit_stop_signals.iter().any(|signal| content_lower.contains(signal));

                        // Debug output for decision making
                        if std::env::var("ARULA_DEBUG").is_ok() {
                            output.print_system(&format!("🔧 Completion check - explicit_stop: {}, optimized: {}", has_explicit_stop, has_codebase_optimized))?;
                        }

                        if has_explicit_stop || has_codebase_optimized {
                            return Ok(AICompletionResult::Optimized);
                        }

                        // Otherwise, always continue - normal task completions should NOT stop continuous mode
                        return Ok(AICompletionResult::Continue);
                    }
                    crate::app::AiResponse::AgentStreamText(chunk) => {
                        // Show AI reasoning during research phase
                        if chunk.contains("research") ||
                           chunk.contains("best practice") ||
                           chunk.contains("improvement") ||
                           chunk.contains("according to") ||
                           chunk.contains("documentation") ||
                           (chunk.len() > 30 && !chunk.starts_with(' ') && !chunk.starts_with('\n') && chunk.len() < 200) {
                            if chunk.trim().len() > 0 {
                                output.print_system(&format!("💭 Research: {}", chunk.trim().to_string()))?;
                            }
                        }
                        last_ai_content.push_str(&chunk);
                    }
                    crate::app::AiResponse::AgentToolCall { id: _, name, arguments } => {
                        tool_count += 1;

                        // Modify read_file calls to limit lines to prevent API failures
                        let modified_arguments = if name == "read_file" {
                            self.limit_read_file_lines(&arguments)
                        } else {
                            arguments.clone()
                        };

                        // Use the same formatting as the main app
                        let tool_display = format_tool_call(&name, &modified_arguments);
                        output.print_system(&format!("🔧 Tool {} - {}", tool_count, tool_display))?;

                        // Add delay between tool calls to prevent rate limiting with progress indicator
                        output.print_system(&format!("⏳ Tool {}: Waiting {} seconds to prevent API rate limiting...", tool_count, TOOL_CALL_DELAY_SECS))?;
                        for i in 1..=TOOL_CALL_DELAY_SECS {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            let remaining = TOOL_CALL_DELAY_SECS - i;
                            if remaining > 0 {
                                output.print_system(&format!("⏳ Tool {} rate limit delay: {}s remaining", tool_count, remaining))?;
                            }
                        }
                    }
                    crate::app::AiResponse::AgentToolResult { tool_call_id, success, result } => {
                        // Use the same formatting as the main app
                        let status = if success { "✅" } else { "❌" };
                        let summary = summarize_tool_result(&result);
                        output.print_system(&format!("  {} {} [Research: {}]", status, summary, tool_call_id))?;

                        // Enhanced debug output for tool results
                        if std::env::var("ARULA_DEBUG").is_ok() {
                            let result_json = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Invalid JSON".to_string());
                            if result_json.len() > 300 {
                                output.print_system(&format!("🔧 Tool Result (truncated): {}...", &result_json[..300]))?;
                            } else {
                                output.print_system(&format!("🔧 Tool Result: {}", result_json))?;
                            }
                        }

                        // If there's an error, start a timeout counter to detect if AI gets stuck
                        if !success {
                            output.print_system("⚠️ Tool failed - watching for AI recovery...")?;
                            return Ok(AICompletionResult::Continue); // Skip to next iteration on error
                        }
                    }
                    _ => {}
                }
            }

            // Check for AI hanging (no response for too long)
            let elapsed = last_activity.elapsed();
            if elapsed.as_secs() > 30 { // 30 seconds of inactivity
                if std::env::var("ARULA_DEBUG").is_ok() {
                    output.print_system(&format!("🔧 AI Hanging Detection - No response for {}s, last activity: {:?}", elapsed.as_secs(), last_activity))?;
                }
                output.print_system(&format!("⚠️ AI appears to be hanging (no response for {}s) - forcing continuation...", elapsed.as_secs()))?;
                return Ok(AICompletionResult::Continue);
            }

            // Show progress if it's been a while since last activity
            if elapsed.as_secs() > 60 {
                output.print_system(&format!("⏳ AI working... ({} tools used, {}s since last activity)", tool_count, elapsed.as_secs()))?;
                last_activity = std::time::Instant::now(); // Reset to avoid spam
            }

            // Show periodic progress and check for API errors
            if timeout_counter % 100 == 0 {
                let elapsed_seconds = (timeout_counter * 100) / 1000;

                // Check if we're in the middle of a tool call that might be hanging
                if tool_count > 0 && elapsed.as_secs() > 20 {
                    output.print_system(&format!("⚠️ Tool call appears to be taking too long ({}s) - this might indicate an API error", elapsed.as_secs()))?;
                }

                output.print_system(&format!("⏳ Research in progress... ({} tools used, {}s elapsed)", tool_count, elapsed_seconds))?;
            }

            // If no response for a long time, increment error counter
            if elapsed.as_secs() > 45 {
                consecutive_errors += 1;
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    output.print_system("⚠️ Too many consecutive timeouts - continuing to next iteration...")?;
                    return Ok(AICompletionResult::Continue);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            timeout_counter += 1;
        }

        output.print_system(&format!("⚠️ Research timeout after {} tools used - continuing to next iteration...", tool_count))?;
        Ok(AICompletionResult::Continue)
    }

    /// Limit read_file arguments to prevent reading too many lines or huge files
    fn limit_read_file_lines(&self, arguments: &str) -> String {
        // First check if this is trying to read large files we want to skip
        if let Ok(mut args_value) = serde_json::from_str::<Value>(arguments) {
            if let Some(path) = args_value.get("path").and_then(|p| p.as_str()) {
                // Skip common large files that aren't useful for analysis
                if path.ends_with("Cargo.lock") ||
                   path.ends_with("package-lock.json") ||
                   path.contains("node_modules/") ||
                   path.ends_with(".git/") ||
                   path.contains("target/") {
                    return r#"{"path": "skipped", "reason": "Large/generated file skipped for analysis"}"#.to_string();
                }

                // Try to correct common path mistakes
                let corrected_path = self.correct_file_path(path);
                if corrected_path != path {
                    if let Some(args_obj) = args_value.as_object_mut() {
                        args_obj.insert("path".to_string(), Value::String(corrected_path));
                    }
                }
            }

            if let Some(args_obj) = args_value.as_object_mut() {
                // Add max_lines parameter if not already present, or limit existing value
                if let Some(existing_lines) = args_obj.get_mut("max_lines") {
                    // Limit existing value to our maximum
                    if let Some(num) = existing_lines.as_u64() {
                        if num > MAX_FILE_LINES as u64 {
                            *existing_lines = Value::Number(serde_json::Number::from(MAX_FILE_LINES));
                        }
                    }
                } else {
                    // Add both max_lines and max_size parameters
                    args_obj.insert("max_lines".to_string(), Value::Number(serde_json::Number::from(MAX_FILE_LINES)));
                    args_obj.insert("max_size".to_string(), Value::Number(serde_json::Number::from(MAX_FILE_SIZE_CHARS)));
                }
            }
            serde_json::to_string(&args_value).unwrap_or_else(|_| arguments.to_string())
        } else {
            // If JSON parsing fails, just add max_lines parameter
            format!("{}, \"max_lines\": {}, \"max_size\": {}", arguments.trim_end_matches('}'), MAX_FILE_LINES, MAX_FILE_SIZE_CHARS)
        }
    }

    /// Correct common file path mistakes made by AI
    fn correct_file_path(&self, path: &str) -> String {
        // Common file path corrections based on actual project structure
        let corrections = [
            // agent_client.rs is in src/api/, not src/
            ("src/agent_client.rs", "src/api/agent_client.rs"),
            // agent.rs is in src/api/, not src/
            ("src/agent.rs", "src/api/agent.rs"),
            // Other common patterns
            ("src/tools/", "src/tools/"),  // Already correct
            ("src/ui/", "src/ui/"),        // Already correct
            ("src/utils/", "src/utils/"),  // Already correct
        ];

        for (wrong_path, correct_path) in corrections.iter() {
            if path == *wrong_path {
                return correct_path.to_string();
            }
        }

        // If the path starts with "src/" and doesn't exist, try common subdirectories
        if path.starts_with("src/") && !path.contains("/") {
            let filename = &path[4..]; // Remove "src/" prefix
            let possible_locations = [
                &format!("src/api/{}", filename),
                &format!("src/ui/{}", filename),
                &format!("src/tools/{}", filename),
                &format!("src/utils/{}", filename),
            ];

            // For simplicity, return the first likely candidate
            for possible_path in possible_locations.iter() {
                if filename == "agent_client.rs" || filename == "agent.rs" {
                    return possible_path.to_string();
                }
            }
        }

        path.to_string()
    }

    /// Wait for AI to recover from an error, with timeout to prevent hanging
    async fn wait_for_ai_recovery_after_error(&self, app: &mut App, output: &mut OutputHandler) -> Result<AICompletionResult> {
        output.print_system(&format!("⏳ Giving AI {} seconds to recover from error...", ERROR_RECOVERY_TIMEOUT_SECS))?;

        let mut timeout_counter = 0;
        let mut last_activity = std::time::Instant::now();
        const MAX_RECOVERY_TIMEOUT: u32 = ERROR_RECOVERY_TIMEOUT_SECS as u32 * 10; // 10 checks per second

        while timeout_counter < MAX_RECOVERY_TIMEOUT {
            if let Some(response) = app.check_ai_response_nonblocking() {
                last_activity = std::time::Instant::now();

                match response {
                    crate::app::AiResponse::AgentStreamEnd => {
                        output.print_system("✅ AI recovered from error")?;
                        return Ok(AICompletionResult::Continue);
                    }
                    crate::app::AiResponse::AgentStreamText(chunk) => {
                        // Check if AI is acknowledging the error and continuing
                        if chunk.to_lowercase().contains("error") ||
                           chunk.to_lowercase().contains("failed") ||
                           chunk.to_lowercase().contains("continue") ||
                           chunk.to_lowercase().contains("next") {
                            output.print_system("💭 AI acknowledging error - continuing recovery...")?;
                        }
                    }
                    crate::app::AiResponse::AgentToolCall { id: _, name: _, arguments: _ } => {
                        output.print_system("🔧 AI making new tool call - recovery in progress...")?;
                        return Ok(AICompletionResult::Continue);
                    }
                    _ => {}
                }
            }

            // Show progress every 3 seconds
            if timeout_counter % 30 == 0 {
                let elapsed = (timeout_counter * 100) / 1000;
                let remaining = ERROR_RECOVERY_TIMEOUT_SECS.saturating_sub(elapsed as u64);
                output.print_system(&format!("⏳ Recovery timeout: {}s remaining", remaining))?;
            }

            // If no activity for 10 seconds, assume AI is stuck
            if last_activity.elapsed().as_secs() > 10 {
                output.print_system("⚠️ AI appears stuck on error - forcing continuation...")?;
                return Ok(AICompletionResult::Continue);
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            timeout_counter += 1;
        }

        output.print_system("⚠️ Recovery timeout exceeded - forcing continuation to next iteration...")?;
        Ok(AICompletionResult::Continue)
    }
}

/// Result of AI completion check
#[derive(Debug, PartialEq)]
enum AICompletionResult {
    Continue,   // Continue with next iteration
    Optimized,  // AI says codebase is optimized
    Error(String), // Error occurred
}