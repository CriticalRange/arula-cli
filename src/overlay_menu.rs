use crate::app::App;
use crate::output::OutputHandler;
use anyhow::Result;
use std::io::{stdout, Write};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, KeyEventKind},
    terminal::{self, size, EnterAlternateScreen, LeaveAlternateScreen},
    cursor::{MoveTo, Show, Hide, SetCursorStyle},
    style::{Color, Print, SetForegroundColor, SetBackgroundColor, ResetColor},
    ExecutableCommand, QueueableCommand,
};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum MenuResult {
    Continue,
    Exit,
    ClearChat,
    BackToMain,
    ConfigurationUpdated,
}

#[derive(Debug, PartialEq)]
enum MenuAction {
    Continue,     // Stay in menu
    CloseMenu,    // Exit menu, continue app
    ExitApp,      // Exit menu AND exit app
}

pub struct OverlayMenu {
    selected_index: usize,
    main_options: Vec<String>,
    config_options: Vec<String>,
    is_in_config: bool,
    animation_offset: u16,
    max_animation_offset: u16,
}

impl OverlayMenu {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            main_options: vec![
                "▶ Chat".to_string(),
                "⚙️  Settings".to_string(),
                "📈 Session".to_string(),
                "🧹 Clear".to_string(),
                "💡 Help".to_string(),
                "✕ Exit".to_string(),
            ],
            config_options: vec![
                "Provider".to_string(),
                "Model".to_string(),
                "API URL".to_string(),
                "API Key".to_string(),
                "← Back".to_string(),
            ],
            is_in_config: false,
            animation_offset: 0,
            max_animation_offset: 10,
        }
    }

    pub fn show_main_menu(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<bool> {
        self.show_menu(app, output, false)
    }

    pub fn show_config_menu(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<bool> {
        self.show_menu(app, output, true)
    }

    pub fn show_exit_confirmation(&mut self, _output: &mut OutputHandler) -> Result<bool> {
        let (_original_cols, _original_rows) = size()?;

        // Enter alternate screen and hide cursor (raw mode is already handled by main app)
        stdout().execute(EnterAlternateScreen)?;
        stdout().execute(Hide)?;

        // Animation loop - slide down effect
        self.animation_offset = self.max_animation_offset;
        while self.animation_offset > 0 {
            self.render_exit_confirmation("Exit ARULA?")?;
            self.animation_offset -= 1;
            std::thread::sleep(Duration::from_millis(20));
        }

        // Show confirmation dialog
        let result = self.show_confirm_dialog("Exit ARULA?")?;

        // Exit animation - slide up effect
        while self.animation_offset < self.max_animation_offset {
            self.animation_offset += 1;
            self.render_exit_confirmation("Exit ARULA?")?;
            std::thread::sleep(Duration::from_millis(20));
        }

        // Cleanup and restore terminal (with proper cursor restoration)
        self.cleanup_terminal()?;

        Ok(result)
    }

    fn render_exit_confirmation(&self, message: &str) -> Result<()> {
        let (cols, rows) = size()?;

        stdout().queue(crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;

        let menu_width = 40.min(cols - 4);
        let menu_height = 6u16;
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2 + self.animation_offset;

        self.draw_box(start_x, start_y, menu_width, menu_height, "Confirm")?;

        // Message
        stdout().queue(MoveTo(start_x + 2, start_y + 2))?
              .queue(Print(message))?;

        stdout().flush()?;
        Ok(())
    }

    fn show_menu(&mut self, app: &mut App, output: &mut OutputHandler, start_in_config: bool) -> Result<bool> {
        self.is_in_config = start_in_config;
        self.selected_index = 0;

        // Save terminal state and cursor style
        let (_original_cols, _original_rows) = size()?;
        // Note: We'll need to save/restore cursor style - but crossterm doesn't have a GetCursorStyle function
        // We'll restore to a known good state instead

        // Enter alternate screen and hide cursor (raw mode is already handled by main app)
        stdout().execute(EnterAlternateScreen)?;
        stdout().execute(Hide)?;

        // Animation loop - slide down effect
        self.animation_offset = self.max_animation_offset;
        while self.animation_offset > 0 {
            self.render_frame(app, output)?;
            self.animation_offset -= 1;
            std::thread::sleep(Duration::from_millis(20));
        }

        // Main event loop
        let result = self.run_menu_loop(app, output)?;

        // Exit animation - slide up effect
        while self.animation_offset < self.max_animation_offset {
            self.animation_offset += 1;
            self.render_frame(app, output)?;
            std::thread::sleep(Duration::from_millis(20));
        }

        // Cleanup and restore terminal
        self.cleanup_terminal()?;

        Ok(result)
    }

    fn run_menu_loop(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<bool> {
        let mut should_exit_app = false;

        // Comprehensive event clearing to prevent submenu issues
        std::thread::sleep(Duration::from_millis(50));
        for _ in 0..5 { // Multiple passes to ensure all events are cleared
            while event::poll(Duration::from_millis(0))? {
                let _ = event::read()?;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        loop {
            self.render_frame(app, output)?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        // Ignore any unexpected key events that might be spurious
                        match key_event.code {
                            KeyCode::Esc | KeyCode::Char('q') => {
                                // If in a submenu, go back to main menu. Otherwise, exit menu.
                                if self.is_in_config {
                                    self.is_in_config = false;
                                    self.selected_index = 0;
                                    // Clear any pending events when returning to main menu to prevent immediate issues
                                    while event::poll(Duration::from_millis(0))? {
                                        let _ = event::read()?;
                                    }
                                } else {
                                    break; // Exit menu, continue app
                                }
                            }
                            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                                break; // Exit menu, continue app
                            }
                            // Only process navigation and selection keys
                            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right |
                            KeyCode::Enter | KeyCode::Char('j') | KeyCode::Char('k') |
                            KeyCode::Char('h') | KeyCode::Char('l') | KeyCode::Tab |
                            KeyCode::Backspace | KeyCode::Delete => {
                                // Valid menu keys - process them
                                let result = self.handle_key_event(key_event, app, output)?;
                                match result {
                                    MenuAction::ExitApp => {
                                        should_exit_app = true;
                                        break;    // Exit menu AND exit app
                                    }
                                    MenuAction::CloseMenu => break,  // Exit menu, continue app
                                    MenuAction::Continue => {},      // Stay in menu
                                }
                            }
                            _ => {
                                // Ignore any other key events that might be spurious
                                continue;
                            }
                        }
                    }
                    Event::Resize(_, _) => {
                        // Redraw on resize
                        self.render_frame(app, output)?;
                    }
                    // Ignore all other event types (mouse, focus, etc.) that might cause issues on Windows
                    _ => {
                        continue;
                    }
                }
            }
        }

        Ok(should_exit_app)
    }

    fn handle_key_event(&mut self, key_event: KeyEvent, app: &mut App, output: &mut OutputHandler) -> Result<MenuAction> {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                self.move_selection(-1);
                Ok(MenuAction::Continue)
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.move_selection(1);
                Ok(MenuAction::Continue)
            }
            KeyCode::Enter => {
                if self.is_in_config {
                    if self.handle_config_selection(app, output)? {
                        Ok(MenuAction::ExitApp)
                    } else {
                        Ok(MenuAction::CloseMenu)
                    }
                } else {
                    if self.handle_main_selection(app, output)? {
                        Ok(MenuAction::ExitApp)
                    } else {
                        Ok(MenuAction::CloseMenu)
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') if self.is_in_config => {
                self.is_in_config = false;
                self.selected_index = 0;
                // More aggressive event clearing when returning to main menu
                std::thread::sleep(Duration::from_millis(20));
                for _ in 0..3 {
                    while event::poll(Duration::from_millis(0))? {
                        let _ = event::read()?;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(MenuAction::Continue)
            }
            _ => Ok(MenuAction::Continue),
        }
    }

    fn handle_main_selection(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<bool> {
        match self.selected_index {
            0 => Ok(false), // Continue chat
            1 => { // Configuration
                self.is_in_config = true;
                self.selected_index = 0;
                // More aggressive event clearing when switching to submenu
                std::thread::sleep(Duration::from_millis(20)); // Small delay
                for _ in 0..3 { // Multiple passes to clear all pending events
                    while event::poll(Duration::from_millis(0))? {
                        let _ = event::read()?;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(false)
            }
            2 => { // Session info
                self.show_session_info(app)?;
                // Clear any pending events that might have been generated during the dialog
                while event::poll(Duration::from_millis(0))? {
                    let _ = event::read()?;
                }
                Ok(false)
            }
            3 => { // Clear chat
                if self.show_confirm_dialog("Clear chat history?")? {
                    app.clear_conversation();
                    output.print_system("✅ Chat history cleared")?;
                }
                // Clear any pending events that might have been generated during the dialog
                while event::poll(Duration::from_millis(0))? {
                    let _ = event::read()?;
                }
                Ok(false)
            }
            4 => { // Help
                self.show_help()?;
                // Clear any pending events that might have been generated during the dialog
                while event::poll(Duration::from_millis(0))? {
                    let _ = event::read()?;
                }
                Ok(false)
            }
            5 => { // Exit
                if self.show_confirm_dialog("Exit ARULA?")? {
                    Ok(true) // Signal to exit application
                } else {
                    Ok(false) // Continue with application
                }
            }
            _ => Ok(false),
        }
    }

    fn handle_config_selection(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<bool> {
        match self.selected_index {
            0 => { // Provider
                self.show_provider_selector(app, output)?;
                // Clear any pending events that might have been generated during the dialog
                while event::poll(Duration::from_millis(0))? {
                    let _ = event::read()?;
                }
                Ok(false)
            }
            1 => { // Model
                if let Some(model) = self.show_text_input("Enter model name", &app.get_config().ai.model)? {
                    app.set_model(&model);
                    output.print_system(&format!("✅ Model set to: {}", model))?;
                }
                // Clear any pending events that might have been generated during the dialog
                while event::poll(Duration::from_millis(0))? {
                    let _ = event::read()?;
                }
                Ok(false)
            }
            2 => { // API URL
                if let Some(url) = self.show_text_input("Enter API URL", &app.get_config().ai.api_url)? {
                    app.config.ai.api_url = url.clone();
                    let _ = app.config.save();
                    match app.initialize_agent_client() {
                        Ok(()) => {
                            output.print_system(&format!("✅ API URL set to: {} (AI client initialized)", url))?;
                        }
                        Err(_) => {
                            output.print_system(&format!("✅ API URL set to: {} (AI client will initialize when configuration is complete)", url))?;
                        }
                    }
                }
                // Clear any pending events that might have been generated during the dialog
                while event::poll(Duration::from_millis(0))? {
                    let _ = event::read()?;
                }
                Ok(false)
            }
            3 => { // API Key
                if let Some(key) = self.show_text_input("Enter API Key (or leave empty to use environment variable)", "")? {
                    if !key.is_empty() {
                        app.config.ai.api_key = key;
                        let _ = app.config.save();
                        match app.initialize_agent_client() {
                            Ok(()) => {
                                output.print_system("✅ API Key updated (AI client initialized)")?;
                            }
                            Err(_) => {
                                output.print_system("✅ API Key updated (AI client will initialize when other settings are complete)")?;
                            }
                        }
                    }
                }
                // Clear any pending events that might have been generated during the dialog
                while event::poll(Duration::from_millis(0))? {
                    let _ = event::read()?;
                }
                Ok(false)
            }
            4 | _ => { // Back
                self.is_in_config = false;
                self.selected_index = 0;
                // More aggressive event clearing when returning to main menu
                std::thread::sleep(Duration::from_millis(20));
                for _ in 0..3 {
                    while event::poll(Duration::from_millis(0))? {
                        let _ = event::read()?;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(false)
            }
        }
    }

    fn show_provider_selector(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        let providers = vec!["openai", "claude", "anthropic", "ollama", "custom"];
        let current_config = app.get_config();
        let current_idx = providers
            .iter()
            .position(|&p| p == current_config.ai.provider)
            .unwrap_or(0);

        // Comprehensive event clearing before provider selector
        std::thread::sleep(Duration::from_millis(20));
        for _ in 0..3 {
            while event::poll(Duration::from_millis(0))? {
                let _ = event::read()?;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        // Create a temporary selection for provider
        let mut selected_idx = current_idx;
        loop {
            self.render_provider_selector(&providers, selected_idx)?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        // Only handle valid navigation keys
                        match key_event.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                if selected_idx > 0 {
                                    selected_idx -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if selected_idx < providers.len() - 1 {
                                    selected_idx += 1;
                                }
                            }
                            KeyCode::Enter => {
                                app.config.ai.provider = providers[selected_idx].to_string();
                                let _ = app.config.save();
                                match app.initialize_agent_client() {
                                    Ok(()) => {
                                        output.print_system(&format!(
                                            "✅ Provider set to: {} (AI client initialized)",
                                            providers[selected_idx]
                                        ))?;
                                    }
                                    Err(_) => {
                                        output.print_system(&format!(
                                            "✅ Provider set to: {} (AI client will initialize when configuration is complete)",
                                            providers[selected_idx]
                                        ))?;
                                    }
                                }
                                break;
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                break;
                            }
                            _ => {
                                // Ignore all other keys
                                continue;
                            }
                        }
                    }
                    _ => {
                        // Ignore all other event types
                        continue;
                    }
                }
            }
        }

        Ok(())
    }

    fn render_provider_selector(&self, providers: &[&str], selected_idx: usize) -> Result<()> {
        let (cols, rows) = size()?;

        stdout().queue(terminal::Clear(terminal::ClearType::All))?;

        let menu_width = 40.min(cols - 4);
        let menu_height = providers.len() + 4;
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height as u16) / 2 + self.animation_offset;

        self.draw_box(start_x, start_y, menu_width, menu_height as u16, "Select AI Provider")?;

        for (i, provider) in providers.iter().enumerate() {
            let y = start_y + 2 + i as u16;
            let prefix = if i == selected_idx { "▶ " } else { "  " };
            let text = if i == selected_idx {
                format!("{}{}", prefix, provider).yellow_bold()
            } else {
                format!("{}{}", prefix, provider)
            };

            stdout().queue(MoveTo(start_x + 2, y))?
                  .queue(Print(text))?;
        }

        stdout().flush()?;
        Ok(())
    }

    fn show_text_input(&mut self, prompt: &str, default: &str) -> Result<Option<String>> {
        let mut input = default.to_string();
        let mut cursor_pos = input.len();

        // Clear any pending events in the buffer
        while event::poll(Duration::from_millis(0))? {
            let _ = event::read()?;
        }

        loop {
            self.render_text_input(prompt, &input, cursor_pos)?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        // Only handle valid input keys
                        match key_event.code {
                            KeyCode::Enter => {
                                return Ok(Some(input));
                            }
                            KeyCode::Esc => {
                                return Ok(None);
                            }
                            KeyCode::Char(c) => {
                                input.insert(cursor_pos, c);
                                cursor_pos += 1;
                            }
                            KeyCode::Backspace => {
                                if cursor_pos > 0 {
                                    input.remove(cursor_pos - 1);
                                    cursor_pos -= 1;
                                }
                            }
                            KeyCode::Delete => {
                                if cursor_pos < input.len() {
                                    input.remove(cursor_pos);
                                }
                            }
                            KeyCode::Left => {
                                if cursor_pos > 0 {
                                    cursor_pos -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if cursor_pos < input.len() {
                                    cursor_pos += 1;
                                }
                            }
                            _ => {
                                // Ignore all other keys
                                continue;
                            }
                        }
                    }
                    _ => {
                        // Ignore all other event types
                        continue;
                    }
                }
            }
        }
    }

    fn render_text_input(&self, prompt: &str, input: &str, cursor_pos: usize) -> Result<()> {
        let (cols, rows) = size()?;

        stdout().queue(terminal::Clear(terminal::ClearType::All))?;

        let menu_width = 60.min(cols - 4);
        let menu_height = 6u16;
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2 + self.animation_offset;

        self.draw_box(start_x, start_y, menu_width, menu_height, prompt)?;

        // Draw input field
        let input_y = start_y + 2;
        let input_text = if input.is_empty() {
            "← Type here...".dim()
        } else {
            input.to_string()
        };

        stdout().queue(MoveTo(start_x + 2, input_y))?
              .queue(Print(input_text))?;

        // Draw cursor
        let display_cursor_pos = if input.is_empty() { 0 } else { cursor_pos };
        stdout().queue(MoveTo(start_x + 2 + display_cursor_pos as u16, input_y))?
              .queue(Print("█".yellow()))?;

        stdout().flush()?;
        Ok(())
    }

    fn show_session_info(&mut self, app: &App) -> Result<()> {
        // Clear any pending events in the buffer
        while event::poll(Duration::from_millis(0))? {
            let _ = event::read()?;
        }

        loop {
            self.render_session_info(app)?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        if matches!(key_event.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q')) {
                            break;
                        }
                        // Ignore all other keys
                        continue;
                    }
                    _ => {
                        // Ignore all other event types
                        continue;
                    }
                }
            }
        }
        Ok(())
    }

    fn render_session_info(&self, app: &App) -> Result<()> {
        let (cols, rows) = size()?;

        stdout().queue(terminal::Clear(terminal::ClearType::All))?;

        let menu_width = 50.min(cols - 4);
        let menu_height = 10u16;
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2 + self.animation_offset;

        self.draw_box(start_x, start_y, menu_width, menu_height, "📊 Session Information")?;

        let config = app.get_config();
        let info_lines = vec![
            format!("Provider: {}", config.ai.provider),
            format!("Model: {}", config.ai.model),
            format!("API URL: {}", config.ai.api_url),
            format!("Messages: {}", app.messages.len()),
        ];

        for (i, line) in info_lines.iter().enumerate() {
            let y = start_y + 2 + i as u16;
            stdout().queue(MoveTo(start_x + 2, y))?
                  .queue(Print(line))?;
        }

        // Instructions
        let instruction_y = start_y + menu_height - 2;
        stdout().queue(MoveTo(start_x + 2, instruction_y))?
              .queue(Print("Press Enter to continue...".dim()))?;

        stdout().flush()?;
        Ok(())
    }

    fn show_help(&mut self) -> Result<()> {
        // Clear any pending events in the buffer
        while event::poll(Duration::from_millis(0))? {
            let _ = event::read()?;
        }

        loop {
            self.render_help()?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        if matches!(key_event.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q')) {
                            break;
                        }
                        // Ignore all other keys
                        continue;
                    }
                    _ => {
                        // Ignore all other event types
                        continue;
                    }
                }
            }
        }
        Ok(())
    }

    fn render_help(&self) -> Result<()> {
        let (cols, rows) = size()?;

        stdout().queue(terminal::Clear(terminal::ClearType::All))?;

        let menu_width = 70.min(cols - 4);
        let menu_height = 20u16;
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2 + self.animation_offset;

        self.draw_box(start_x, start_y, menu_width, menu_height, "❓ ARULA Help")?;

        let help_lines = vec![
            "🔧 Commands:",
            "  /help     - Show this help",
            "  /menu     - Open interactive menu",
            "  /clear    - Clear conversation history",
            "  /config   - Show current configuration",
            "  /model <name> - Change AI model",
            "  exit or quit - Exit ARULA",
            "",
            "⌨️  Keyboard Shortcuts:",
            "  Ctrl+C    - Open menu",
            "  m         - Open menu",
            "  Ctrl+D    - Exit",
            "  Up/Down   - Navigate command history",
            "",
            "💡 Tips:",
            "  • End line with \\ to continue on next line",
            "  • Ask ARULA to execute bash commands",
            "  • Use natural language",
            "  • Native terminal scrollback works!",
        ];

        for (i, line) in help_lines.iter().enumerate() {
            if i < help_lines.len() {
                let y = start_y + 2 + i as u16;
                stdout().queue(MoveTo(start_x + 2, y))?
                      .queue(Print(*line))?;
            }
        }

        stdout().flush()?;
        Ok(())
    }

    fn show_confirm_dialog(&mut self, message: &str) -> Result<bool> {
        let mut selected = false; // false for No, true for Yes

        // Clear any pending events in the buffer
        while event::poll(Duration::from_millis(0))? {
            let _ = event::read()?;
        }

        loop {
            self.render_confirm_dialog(message, selected)?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        match key_event.code {
                            KeyCode::Enter => {
                                return Ok(selected);
                            }
                            KeyCode::Esc => {
                                return Ok(false);
                            }
                            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                                // Ctrl+C should exit the app (same as selecting "Yes" on exit confirmation)
                                return Ok(true);
                            }
                            KeyCode::Left | KeyCode::Right | KeyCode::Tab |
                            KeyCode::Char('h') | KeyCode::Char('l') => {
                                selected = !selected;
                            }
                            _ => {
                                // Ignore all other keys
                                continue;
                            }
                        }
                    }
                    _ => {
                        // Ignore all other event types
                        continue;
                    }
                }
            }
        }
    }

    fn render_confirm_dialog(&self, message: &str, selected: bool) -> Result<()> {
        let (cols, rows) = size()?;

        stdout().queue(terminal::Clear(terminal::ClearType::All))?;

        let menu_width = 50.min(cols - 8);
        let menu_height = 8u16;
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2 + self.animation_offset;

        // Draw modern box for confirmation
        self.draw_modern_box(start_x, start_y, menu_width, menu_height, "CONFIRM")?;

        // Draw title
        let title_y = start_y + 1;
        let title = "?";
        stdout().queue(MoveTo(start_x + menu_width / 2 - 1, title_y))?
              .queue(SetForegroundColor(Color::Yellow))?
              .queue(Print(title.bold()))?
              .queue(ResetColor)?;

        // Message
        stdout().queue(MoveTo(start_x + 2, start_y + 3))?
              .queue(SetForegroundColor(Color::White))?
              .queue(Print(message))?
              .queue(ResetColor)?;

        // Modern styled options
        let no_text = "NO";
        let yes_text = "YES";

        let options_y = start_y + 5;
        let no_x = start_x + menu_width / 2 - 10;
        let yes_x = start_x + menu_width / 2 + 2;

        // Draw NO option
        if !selected {
            // Unselected
            stdout().queue(MoveTo(no_x, options_y))?
                  .queue(SetBackgroundColor(Color::DarkGrey))?
                  .queue(SetForegroundColor(Color::White))?
                  .queue(Print(format!(" {} ", no_text)))?
                  .queue(ResetColor)?;
        } else {
            // Selected
            stdout().queue(MoveTo(no_x, options_y))?
                  .queue(SetBackgroundColor(Color::Red))?
                  .queue(SetForegroundColor(Color::White))?
                  .queue(Print(format!(" {} ", no_text.bold())))?
                  .queue(ResetColor)?;
        }

        // Draw YES option
        if selected {
            // Selected
            stdout().queue(MoveTo(yes_x, options_y))?
                  .queue(SetBackgroundColor(Color::Green))?
                  .queue(SetForegroundColor(Color::White))?
                  .queue(Print(format!(" {} ", yes_text.bold())))?
                  .queue(ResetColor)?;
        } else {
            // Unselected
            stdout().queue(MoveTo(yes_x, options_y))?
                  .queue(SetBackgroundColor(Color::DarkGrey))?
                  .queue(SetForegroundColor(Color::White))?
                  .queue(Print(format!(" {} ", yes_text)))?
                  .queue(ResetColor)?;
        }

        stdout().flush()?;
        Ok(())
    }

    fn render_frame(&self, app: &App, _output: &OutputHandler) -> Result<()> {
        let (_cols, _rows) = size()?;

        stdout().queue(crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;

        if self.is_in_config {
            self.render_config_menu(app)?;
        } else {
            self.render_main_menu()?;
        }

        stdout().flush()?;
        Ok(())
    }

    fn render_main_menu(&self) -> Result<()> {
        let (cols, rows) = size()?;

        let menu_width = 50.min(cols - 8);
        let menu_height = 12; // Fixed height for better layout
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2 + self.animation_offset;

        // Draw modern box with gradient effect
        self.draw_modern_box(start_x, start_y, menu_width, menu_height, "ARULA")?;

        // Draw title with modern styling
        let title_y = start_y + 2;
        let title = "● MENU";
        let title_len = title.len() as u16;
        let title_x = if menu_width > title_len + 2 {
            start_x + menu_width / 2 - title_len / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(title_x, title_y))?
              .queue(SetForegroundColor(Color::Cyan))?
              .queue(Print(title))?
              .queue(ResetColor)?;

        // Draw menu items with modern styling
        let items_start_y = start_y + 4;
        for (i, option) in self.main_options.iter().enumerate() {
            let y = items_start_y + i as u16;

            if i == self.selected_index {
                // Selected item with modern highlight
                self.draw_selected_item(start_x + 2, y, menu_width - 4, option)?;
            } else {
                // Unselected item with subtle styling
                stdout().queue(MoveTo(start_x + 4, y))?
                      .queue(SetForegroundColor(Color::DarkGrey))?
                      .queue(Print(option))?
                      .queue(ResetColor)?;
            }
        }

        // Draw modern help text
        let help_y = start_y + menu_height - 2;
        let help_text = "↑↓ Navigate • Enter Select • ESC Exit";
        let help_len = help_text.len() as u16;
        let help_x = if menu_width > help_len + 2 {
            start_x + menu_width / 2 - help_len / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(help_x, help_y))?
              .queue(SetForegroundColor(Color::DarkGrey))?
              .queue(Print(help_text))?
              .queue(ResetColor)?;

        Ok(())
    }

    fn render_config_menu(&self, app: &App) -> Result<()> {
        let (cols, rows) = size()?;

        let config = app.get_config();
        let mut display_options = self.config_options.clone();

        // Update display values with modern styling
        display_options[0] = format!("○ Provider: {}", config.ai.provider);
        display_options[1] = format!("○ Model: {}", config.ai.model);
        display_options[2] = format!("○ API URL: {}", config.ai.api_url);
        display_options[3] = format!(
            "○ API Key: {}",
            if config.ai.api_key.is_empty() {
                "Not set"
            } else {
                "••••••••"
            }
        );

        let menu_width = 60.min(cols - 8);
        let menu_height = 12; // Fixed height for consistency
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2 + self.animation_offset;

        // Draw modern box
        self.draw_modern_box(start_x, start_y, menu_width, menu_height, "SETTINGS")?;

        // Draw title with modern styling
        let title_y = start_y + 2;
        let title = "⚙️ SETTINGS";
        let title_len = title.len() as u16;
        let title_x = if menu_width > title_len + 2 {
            start_x + menu_width / 2 - title_len / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(title_x, title_y))?
              .queue(SetForegroundColor(Color::Cyan))?
              .queue(Print(title))?
              .queue(ResetColor)?;

        // Draw config items with modern styling
        let items_start_y = start_y + 4;
        for (i, option) in display_options.iter().enumerate() {
            let y = items_start_y + i as u16;

            if i == self.selected_index {
                // Selected item with modern highlight
                self.draw_selected_item(start_x + 2, y, menu_width - 4, option)?;
            } else {
                // Unselected item with subtle styling
                stdout().queue(MoveTo(start_x + 4, y))?
                      .queue(SetForegroundColor(Color::DarkGrey))?
                      .queue(Print(option))?
                      .queue(ResetColor)?;
            }
        }

        // Draw modern help text
        let help_y = start_y + menu_height - 2;
        let help_text = "↑↓ Edit • Enter Select • ← Back • ESC Exit";
        let help_len = help_text.len() as u16;
        let help_x = if menu_width > help_len + 2 {
            start_x + menu_width / 2 - help_len / 2
        } else {
            start_x + 1
        };
        stdout().queue(MoveTo(help_x, help_y))?
              .queue(SetForegroundColor(Color::DarkGrey))?
              .queue(Print(help_text))?
              .queue(ResetColor)?;

        Ok(())
    }

    fn draw_modern_box(&self, x: u16, y: u16, width: u16, height: u16, _title: &str) -> Result<()> {
        // Modern box with rounded corners - simplified to avoid overflow issues
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

        // Clear the area first with bounds checking
        for row in y..std::cmp::min(y + height, u16::MAX) {
            stdout().queue(MoveTo(x, row))?;
            for _col in x..std::cmp::min(x + width, u16::MAX) {
                stdout().queue(Print(" "))?;
            }
        }

        // Draw simple gradient border (cyan to blue)
        let height_f = height as f32;
        for i in 0..height {
            if height_f == 0.0 { break; }
            let progress = i as f32 / height_f;

            // Use closest terminal color (simplified to Cyan/Blue)
            let border_color = if progress < 0.5 { Color::Cyan } else { Color::Blue };

            stdout().queue(SetForegroundColor(border_color))?;
            stdout().queue(MoveTo(x, y + i))?.queue(Print(vertical))?;
            stdout().queue(MoveTo(x + width.saturating_sub(1), y + i))?.queue(Print(vertical))?;
        }

        // Top border with gradient
        stdout().queue(SetForegroundColor(Color::Cyan))?;
        stdout().queue(MoveTo(x, y))?.queue(Print(top_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(top_right))?;

        // Bottom border with gradient
        stdout().queue(SetForegroundColor(Color::Blue))?;
        stdout().queue(MoveTo(x, y + height.saturating_sub(1)))?.queue(Print(bottom_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(bottom_right))?;

        stdout().queue(ResetColor)?;
        Ok(())
    }

    fn draw_selected_item(&self, x: u16, y: u16, width: u16, text: &str) -> Result<()> {
        // Validate dimensions
        if width < 3 {
            return Ok(());
        }

        // Draw selection background with modern style
        stdout().queue(MoveTo(x, y))?;

        // Background fill with bounds checking
        for _i in 0..width {
            stdout().queue(SetBackgroundColor(Color::DarkBlue))?;
            stdout().queue(Print(" "))?;
        }

        // Reset background for text
        stdout().queue(ResetColor)?;

        // Draw text with proper spacing and color - safely format
        let display_text = format!("▶ {}", text);
        let safe_text = if display_text.len() > width.saturating_sub(4) as usize {
            // Truncate if too long
            let safe_len = width.saturating_sub(7) as usize;
            format!("▶ {}...", &text[..safe_len.min(text.len())])
        } else {
            display_text
        };

        stdout().queue(MoveTo(x + 2, y))?
              .queue(SetBackgroundColor(Color::DarkBlue))?
              .queue(SetForegroundColor(Color::White))?
              .queue(Print(safe_text))?
              .queue(ResetColor)?;

        Ok(())
    }

    fn draw_box(&self, x: u16, y: u16, width: u16, height: u16, title: &str) -> Result<()> {
        // Keep the old method for compatibility
        let top_left = "╔";
        let top_right = "╗";
        let bottom_left = "╚";
        let bottom_right = "╝";
        let horizontal = "═";
        let vertical = "║";

        // Set purple color for borders
        stdout().queue(SetForegroundColor(Color::DarkMagenta))?;

        // Top border
        stdout().queue(MoveTo(x, y))?.queue(Print(top_left))?;
        for _i in 1..width-1 {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(top_right))?;

        // Title in top border
        if !title.is_empty() && title.len() < width as usize - 4 {
            let title_start = x + 2;
            stdout().queue(MoveTo(title_start, y))?;
            stdout().queue(SetBackgroundColor(Color::DarkMagenta))?
                  .queue(SetForegroundColor(Color::Yellow))?
                  .queue(Print(format!(" {} ", title)))?
                  .queue(ResetColor)?;
        }

        // Vertical borders
        for _i in 1..height-1 {
            stdout().queue(MoveTo(x, y + _i))?.queue(Print(vertical))?;
            stdout().queue(MoveTo(x + width - 1, y + _i))?.queue(Print(vertical))?;
        }

        // Bottom border
        stdout().queue(MoveTo(x, y + height - 1))?.queue(Print(bottom_left))?;
        for _i in 1..width-1 {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(bottom_right))?;

        stdout().queue(ResetColor)?;
        Ok(())
    }

    fn move_selection(&mut self, direction: isize) {
        let options = if self.is_in_config {
            &self.config_options
        } else {
            &self.main_options
        };

        let new_index = self.selected_index as isize + direction;
        self.selected_index = new_index.clamp(0, (options.len() - 1) as isize) as usize;
    }

    fn cleanup_terminal(&self) -> Result<()> {
        let mut stdout = stdout();

        // Reset terminal colors and attributes first
        stdout.execute(crossterm::style::ResetColor)?;

        // Restore cursor visibility and style to match main app
        stdout.execute(Show)?;
        stdout.execute(SetCursorStyle::BlinkingBlock)?;

        // Leave alternate screen to return to main terminal
        stdout.execute(LeaveAlternateScreen)?;

        // Ensure all commands are sent to terminal
        stdout.flush()?;

        Ok(())
    }
}

// Simple color formatting functions
fn format_colored(text: &str, color_code: &str) -> String {
    format!("\x1b[{}m{}\x1b[0m", color_code, text)
}

fn format_colored_bold(text: &str, color_code: &str) -> String {
    format!("\x1b[1;{}m{}\x1b[0m", color_code, text)
}

trait ColoredText {
    fn red(&self) -> String;
    fn green(&self) -> String;
    fn yellow(&self) -> String;
    fn blue(&self) -> String;
    fn purple(&self) -> String;
    fn dim(&self) -> String;
    fn bold(&self) -> String;
    fn yellow_bold(&self) -> String;
}

impl ColoredText for str {
    fn red(&self) -> String {
        format_colored(self, "31")
    }

    fn green(&self) -> String {
        format_colored(self, "32")
    }

    fn yellow(&self) -> String {
        format_colored(self, "33")
    }

    fn blue(&self) -> String {
        format_colored(self, "34")
    }

    fn purple(&self) -> String {
        format_colored(self, "35")
    }

    fn dim(&self) -> String {
        format_colored(self, "2")
    }

    fn bold(&self) -> String {
        format_colored_bold(self, "1")
    }

    fn yellow_bold(&self) -> String {
        format_colored_bold(self, "33")
    }
}

impl ColoredText for String {
    fn red(&self) -> String {
        self.as_str().red()
    }

    fn green(&self) -> String {
        self.as_str().green()
    }

    fn yellow(&self) -> String {
        self.as_str().yellow()
    }

    fn blue(&self) -> String {
        self.as_str().blue()
    }

    fn purple(&self) -> String {
        self.as_str().purple()
    }

    fn dim(&self) -> String {
        self.as_str().dim()
    }

    fn bold(&self) -> String {
        self.as_str().bold()
    }

    fn yellow_bold(&self) -> String {
        self.as_str().yellow_bold()
    }
}

impl Default for OverlayMenu {
    fn default() -> Self {
        Self::new()
    }
}