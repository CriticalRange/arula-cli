use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::Color,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color as RColor, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
    Frame, Terminal, TerminalOptions, Viewport,
};
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use termimad::MadSkin;
use arula_core::app::AiResponse;
use arula_core::App;

use crate::ui::scroll_history::{HistoryLine, HistorySpan};
use crate::ui::menus::main_menu::MainMenu;
use crate::ui::menus::common::MenuResult;
use crate::ui::output::OutputHandler;
use arula_core::utils::chat::MessageType;

/// Tool execution status
#[derive(Clone)]
pub struct ToolExecution {
    pub id: String,
    pub name: String,
    pub status: ToolState,
}

#[derive(Clone, PartialEq)]
pub enum ToolState {
    Running,
    Success(String),
    Error(String),
}

/// The TUI viewport height (status + input area only)
const VIEWPORT_HEIGHT: u16 = 3;

/// Application state (separate from terminal for borrow checker)
struct AppState {
    input: String,
    input_cursor: usize,
    is_waiting: bool,
    thinking_content: String,
    active_tools: Vec<ToolExecution>,
    current_response: String,
    pending_history: Vec<HistoryLine>,
    frame: usize,
    last_tick: Instant,
    screen_height: u16,
    screen_width: u16,
    app: App,
}

impl AppState {
    fn new(app: App, width: u16, height: u16) -> Self {
        Self {
            input: String::new(),
            input_cursor: 0,
            is_waiting: false,
            thinking_content: String::new(),
            active_tools: Vec::new(),
            current_response: String::new(),
            pending_history: Vec::new(),
            frame: 0,
            last_tick: Instant::now(),
            screen_height: height,
            screen_width: width,
            app,
        }
    }

    fn add_user_message(&mut self, message: &str) {
        self.pending_history.push(HistoryLine::new(vec![
            HistorySpan::new("▶ You: ").fg(Color::Cyan).bold(),
            HistorySpan::new(message),
        ]));
    }

    fn add_ai_message(&mut self, message: &str) {
        let message = message.trim();
        if message.is_empty() {
            return;
        }

        let width = (self.screen_width as usize).saturating_sub(8); // -8 for padding/safety
        let skin = MadSkin::default();
        let text = skin.text(message, Some(width)).to_string();

        let mut lines = text.lines();

        if let Some(first) = lines.next() {
            self.pending_history.push(HistoryLine::new(vec![
                HistorySpan::new("◆ AI: ").fg(Color::Yellow).bold(),
                HistorySpan::new(first.to_string()),
            ]));
        }

        for line in lines {
            self.pending_history.push(HistoryLine::new(vec![
                HistorySpan::new("      "), // Indentation to align with text
                HistorySpan::new(line.to_string()),
            ]));
        }
    }



    fn add_tool_message(&mut self, name: &str, _args: &str) {
         self.pending_history.push(HistoryLine::new(vec![
            HistorySpan::new("🔧 Tool: ").fg(Color::Magenta).bold(),
            HistorySpan::new(name).bold(),
            HistorySpan::new("...").dim(),
        ]));
    }

    fn tick(&mut self) {
        if self.last_tick.elapsed() >= Duration::from_millis(100) {
            self.frame = self.frame.wrapping_add(1);
            self.last_tick = Instant::now();
        }
    }

    fn render_viewport(&self, f: &mut Frame) {
        let area = f.area();
        // Reserve 1 line for input, rest for status
        let input_height = 1;
        let max_status_height = area.height.saturating_sub(input_height);
        
        // Calculate needed status height but clamp to available space
        let mut status_height = self.calculate_status_height();
        if status_height > max_status_height {
            status_height = max_status_height;
        }
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(status_height),
                Constraint::Length(input_height),
            ])
            .split(area);

        self.render_status(f, chunks[0]);
        self.render_input(f, chunks[1]);
    }

    fn calculate_status_height(&self) -> u16 {
        let mut height = 0u16;
        if self.is_waiting && self.current_response.is_empty() && self.active_tools.is_empty() {
            height += 1;
        }
        if !self.thinking_content.is_empty() {
            height += 1;
        }
        height += self.active_tools.len() as u16;
        if !self.current_response.is_empty() {
            height += 1;
        }
        // Min 1 (at least a blank line or status), Max infinite (clamped by viewport)
        height.max(1)
    }

    fn render_status(&self, f: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        if self.is_waiting
            && self.thinking_content.is_empty()
            && self.current_response.is_empty()
            && self.active_tools.is_empty()
        {
            let spinner = ["◐", "◓", "◑", "◒"][self.frame % 4];
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", spinner), Style::default().fg(RColor::Cyan)),
                Span::styled("Processing...", Style::default().fg(RColor::Cyan)),
            ]));
        }

        if !self.thinking_content.is_empty() {
            let spinner = ["◐", "◓", "◑", "◒"][self.frame % 4];
            // Only show last line of thinking to save space
            let preview: String = self
                .thinking_content
                .lines()
                .last()
                .unwrap_or("")
                .chars()
                .take(50)
                .collect();
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} Thinking: ", spinner),
                    Style::default().fg(RColor::Magenta),
                ),
                Span::styled(preview, Style::default().fg(RColor::DarkGray)),
            ]));
        }

        for tool in &self.active_tools {
            let (icon, color) = match &tool.status {
                ToolState::Running => {
                    let s = ["◐", "◓", "◑", "◒"][self.frame % 4];
                    (s.to_string(), RColor::Yellow)
                }
                ToolState::Success(_) => ("✓".to_string(), RColor::Green),
                ToolState::Error(_) => ("✗".to_string(), RColor::Red),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(
                    &tool.name,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        if !self.current_response.is_empty() {
            // ... preview current response ...
             let preview: String = self
                .current_response
                .lines()
                .last()
                .unwrap_or("")
                .chars()
                .take(60)
                .collect();
            lines.push(Line::from(vec![
                Span::styled("◆ ", Style::default().fg(RColor::Yellow)),
                Span::styled(preview, Style::default().fg(RColor::White)),
            ]));
        }

        // If we have more lines than area, take the LAST N lines (most recent status)
        let total_lines = lines.len();
        let display_lines = if total_lines > area.height as usize {
            lines.into_iter().rev().take(area.height as usize).rev().collect()
        } else {
            lines
        };

        let status = Paragraph::new(display_lines);
        f.render_widget(status, area);
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let input_text = format!("▶ {}", self.input);
        let input = Paragraph::new(input_text.as_str())
            .style(Style::default().fg(RColor::White));
            // Removed Block::borders(TOP) to save space

        f.render_widget(input, area);
        
        // Calculate cursor X position based on char count
        let cursor_x = area.x + 2 + self.input.chars().take(self.input_cursor).count() as u16;
        // Cursor Y is just area.y because no border
        f.set_cursor_position((cursor_x, area.y));
    }
}

/// Main TUI Application
pub struct TuiApp {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    state: AppState,
}

impl TuiApp {
    pub fn new(app: App) -> Result<Self> {
        enable_raw_mode()?;

        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let (width, height) = terminal::size()?;

        let viewport = Viewport::Inline(VIEWPORT_HEIGHT);
        let terminal = Terminal::with_options(backend, TerminalOptions { viewport })?;

        Ok(Self {
            terminal,
            state: AppState::new(app, width, height),
        })
    }



    pub async fn run(&mut self) -> Result<()> {
        loop {
            // Update screen size
            if let Ok((w, h)) = terminal::size() {
                self.state.screen_width = w;
                self.state.screen_height = h;
            }

            // Flush pending history BEFORE drawing the viewport
            if !self.state.pending_history.is_empty() {
                let height = self.state.pending_history.len() as u16;
                let lines: Vec<_> = self.state.pending_history.iter().map(|h| h.to_line()).collect();
                self.terminal.insert_before(height, |buf| {
                    let area = buf.area; // Trying field access first as per some versions
                    let p = Paragraph::new(lines);
                    p.render(area, buf);
                })?;
                self.state.pending_history.clear();
            }

            // Draw viewport
            self.terminal.draw(|f| self.state.render_viewport(f))?;

            // Handle events - only Press events (not Release or Repeat)
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    // Filter for Press events only to avoid double-typing
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(());
                        }
                        KeyCode::Enter => {
                            if !self.state.input.is_empty() && !self.state.is_waiting {
                                self.submit_message().await?;
                            }
                        }
                        KeyCode::Char(c) => {
                            // Insert at byte position corresponding to char position
                            let byte_pos = self.state.input
                                .char_indices()
                                .nth(self.state.input_cursor)
                                .map(|(i, _)| i)
                                .unwrap_or(self.state.input.len());
                            self.state.input.insert(byte_pos, c);
                            self.state.input_cursor += 1;
                        }
                        KeyCode::Backspace => {
                            if self.state.input_cursor > 0 {
                                self.state.input_cursor -= 1;
                                // Remove char at cursor position
                                if let Some((byte_pos, _ch)) = self.state.input
                                    .char_indices()
                                    .nth(self.state.input_cursor)
                                {
                                    self.state.input.remove(byte_pos);
                                }
                            }
                        }
                        KeyCode::Delete => {
                            let char_count = self.state.input.chars().count();
                            if self.state.input_cursor < char_count {
                                if let Some((byte_pos, _)) = self.state.input
                                    .char_indices()
                                    .nth(self.state.input_cursor)
                                {
                                    self.state.input.remove(byte_pos);
                                }
                            }
                        }
                        KeyCode::Left => {
                            self.state.input_cursor = self.state.input_cursor.saturating_sub(1);
                        }
                        KeyCode::Right => {
                            let char_count = self.state.input.chars().count();
                            if self.state.input_cursor < char_count {
                                self.state.input_cursor += 1;
                            }
                        }
                        KeyCode::Esc => {
                            if !self.state.input.is_empty() {
                                self.state.input.clear();
                                self.state.input_cursor = 0;
                            }
                        }
                        KeyCode::BackTab => {
                            let mut menu = MainMenu::new();
                            let mut output = OutputHandler::new();
                            // Temporarily disable raw mode if needed, but menu handles it.
                            let result = menu.show(&mut self.state.app, &mut output)?;
                            self.handle_menu_result(result)?;
                        }
                        _ => {}
                    }
                }
            }

            // Poll AI
            if self.state.is_waiting {
                self.poll_ai_response()?;
            }

            self.state.tick();
        }
    }

    async fn submit_message(&mut self) -> Result<()> {
        let message = self.state.input.clone();
        self.state.input.clear();
        self.state.input_cursor = 0;

        self.state.add_user_message(&message);

        self.state.is_waiting = true;
        self.state.current_response.clear();
        self.state.thinking_content.clear();
        self.state.active_tools.clear();

        self.state.app.send_to_ai(&message).await?;
        Ok(())
    }

    fn poll_ai_response(&mut self) -> Result<()> {
        while let Some(response) = self.state.app.check_ai_response_nonblocking() {
            match response {
                AiResponse::AgentStreamStart => {}
                AiResponse::AgentStreamText(text) => {
                    self.state.current_response.push_str(&text);
                }
                AiResponse::AgentThinkingStart => {
                    self.state.thinking_content.clear();
                }
                AiResponse::AgentThinkingContent(content) => {
                    self.state.thinking_content.push_str(&content);
                }
                AiResponse::AgentThinkingEnd => {}
                AiResponse::AgentToolCall { id, name, arguments } => {
                    // Log tool call to history so it scrolls up
                    self.state.add_tool_message(&name, &arguments);
                    
                    self.state.active_tools.push(ToolExecution {
                        id,
                        name,
                        status: ToolState::Running,
                    });
                }
                AiResponse::AgentToolResult {
                    tool_call_id,
                    success,
                    result,
                } => {
                    if let Some(tool) = self
                        .state
                        .active_tools
                        .iter_mut()
                        .find(|t| t.id == tool_call_id)
                    {
                        tool.status = if success {
                            ToolState::Success(result.to_string().chars().take(50).collect())
                        } else {
                            ToolState::Error(result.to_string().chars().take(50).collect())
                        };
                    }
                }
                AiResponse::AgentStreamEnd => {
                    if !self.state.current_response.is_empty() {
                        let response = self.state.current_response.clone();
                        self.state.add_ai_message(&response);

                    }
                    self.state.current_response.clear();
                    self.state.active_tools.clear();
                    self.state.thinking_content.clear();
                    self.state.is_waiting = false;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_menu_result(&mut self, result: MenuResult) -> Result<()> {
        match result {
            MenuResult::LoadConversation(id) => {
                // Clear state
                self.state.input.clear();
                self.state.input_cursor = 0;
                
                // Load conversation
                self.state.app.load_conversation(&id)?;
                
                // Clear screen and reprint history
                let output = OutputHandler::new();
                
                // Clear terminal (we need to bypass ratatui for a moment or rely on it cleaning up)
                execute!(io::stdout(), terminal::Clear(terminal::ClearType::All), crossterm::cursor::MoveTo(0, 0))?;
                output.print_banner()?;
                
                for msg in self.state.app.get_message_history() {
                    match msg.message_type {
                        MessageType::User => output.print_user_message(&msg.content)?,
                        MessageType::Arula => output.print_ai_message(&msg.content)?,
                        MessageType::ToolCall => {
                             // Parse tool call if possible or just print info
                             // The content is "🔧 Tool call: name(args)"
                             // We might want to parse it back or just print as system/debug
                             // output.print_system(&msg.content)?;
                        }
                        MessageType::ToolResult => {
                             // output.print_system(&msg.content)?;
                        }
                        _ => {}
                    }
                }
                println!(); // Extra space
            }
            MenuResult::ClearChat => {
                 self.state.app.clear_conversation();
                 // Clear screen
                 execute!(io::stdout(), terminal::Clear(terminal::ClearType::All), crossterm::cursor::MoveTo(0, 0))?;
                 let output = OutputHandler::new();
                 output.print_banner()?;
                 println!();
            }
            MenuResult::NewConversation => {
                // Clear state
                self.state.input.clear();
                self.state.input_cursor = 0;
                
                // New conversation
                self.state.app.new_conversation();
                self.state.app.clear_conversation();
                
                // Clear screen
                execute!(io::stdout(), terminal::Clear(terminal::ClearType::All), crossterm::cursor::MoveTo(0, 0))?;
                let output = OutputHandler::new();
                output.print_banner()?;
                println!();
            }
            _ => {}
        }
        Ok(())
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        let _ = self.terminal.clear();
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show);
    }
}
