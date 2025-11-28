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

/// Main menu options
#[derive(Debug, Clone)]
pub enum MainMenuItem {
    ContinueChat,
    Conversations,
    Settings,
    InfoHelp,
    ClearChat,
}

impl MainMenuItem {
    pub fn all() -> Vec<Self> {
        vec![
            MainMenuItem::ContinueChat,
            MainMenuItem::Conversations,
            MainMenuItem::Settings,
            MainMenuItem::InfoHelp,
            MainMenuItem::ClearChat,
        ]
    }

    pub fn label(&self) -> &str {
        match self {
            MainMenuItem::ContinueChat => "⦿ Continue Chat",
            MainMenuItem::Conversations => "📚 Conversations",
            MainMenuItem::Settings => "⚙ Configuration",
            MainMenuItem::InfoHelp => "ℹ Info & Help",
            MainMenuItem::ClearChat => "Ⓒ Clear Chat",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            MainMenuItem::ContinueChat => "Return to conversation",
            MainMenuItem::Conversations => "View, load, or manage saved conversations",
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
}