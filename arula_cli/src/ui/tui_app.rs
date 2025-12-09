use anyhow::Result;
use console::strip_ansi_codes;
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
    widgets::Paragraph,
    Frame, Terminal, TerminalOptions, Viewport,
};
use serde_json::Value;
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use arula_core::app::AiResponse;
use arula_core::App;
use regex::Regex;
use std::sync::OnceLock;
use termimad::MadSkin;

use crate::ui::menus::common::MenuResult;
use crate::ui::menus::main_menu::MainMenu;
use crate::ui::output::OutputHandler;
use crate::ui::scroll_history::{insert_history_lines, HistoryLine, HistorySpan};
use arula_core::utils::chat::MessageType;

/// Tool execution status
#[derive(Clone)]
pub struct ToolExecution {
    pub id: String,
    pub name: String,
    pub args: String,
    pub status: ToolState,
    pub started_at: Instant,
    pub finished_at: Option<Instant>,
    pub summary: Option<String>,
}

#[derive(Clone, PartialEq)]
pub enum ToolState {
    Running,
    Success,
    Error,
}

/// The TUI viewport height (status + input area only)
const VIEWPORT_HEIGHT: u16 = 3;

/// Application state (separate from terminal for borrow checker)
struct AppState {
    input: String,
    input_cursor: usize,
    is_waiting: bool,
    thinking_content: String,
    stream_collector: StreamCollector,
    active_tools: Vec<ToolExecution>,
    current_response: String,
    pending_history: Vec<HistoryLine>,
    frame: usize,
    last_tick: Instant,
    screen_height: u16,
    screen_width: u16,
    last_ai_message: Option<String>,
    app: App,
}

impl AppState {
    fn new(app: App, width: u16, height: u16) -> Self {
        Self {
            input: String::new(),
            input_cursor: 0,
            is_waiting: false,
            thinking_content: String::new(),
            stream_collector: StreamCollector::new(),
            active_tools: Vec::new(),
            current_response: String::new(),
            pending_history: Vec::new(),
            frame: 0,
            last_tick: Instant::now(),
            screen_height: height,
            screen_width: width,
            last_ai_message: None,
            app,
        }
    }

    fn add_user_message(&mut self, message: &str) {
        let clean = clean_text(message);
        self.pending_history.push(HistoryLine::new(vec![
            HistorySpan::new("▶ You: ").fg(Color::Cyan).bold(),
            HistorySpan::new(clean),
        ]));
        self.last_ai_message = None;
    }

    fn add_ai_message(&mut self, message: &str) {
        let message = clean_text(message).trim().to_string();
        if message.is_empty() {
            return;
        }
        if self.last_ai_message.as_deref() == Some(&message) {
            return;
        }

        let width = (self.screen_width as usize).saturating_sub(8); // -8 for padding/safety
        let skin = MadSkin::default();
        let text = skin.text(&message, Some(width)).to_string();

        let mut lines = text.lines();

        if let Some(first) = lines.next() {
            self.pending_history
                .push(HistoryLine::new(vec![HistorySpan::new(first.to_string())]));
        }

        for line in lines {
            self.pending_history.push(HistoryLine::new(vec![
                HistorySpan::new("      "), // Indentation to align with text
                HistorySpan::new(line.to_string()),
            ]));
        }

        self.last_ai_message = Some(message);
    }

    fn add_tool_message(&mut self, name: &str, args: &str) {
        let clean_args = clean_text(args);
        self.pending_history.push(HistoryLine::new(vec![
            HistorySpan::new("🔧 Tool: ").fg(Color::Magenta).bold(),
            HistorySpan::new(name).bold(),
            HistorySpan::new(format!(" {}", clean_args)).dim(),
        ]));
    }

    fn tick(&mut self) -> bool {
        if self.last_tick.elapsed() >= Duration::from_millis(100) {
            self.frame = self.frame.wrapping_add(1);
            self.last_tick = Instant::now();
            return true;
        }
        false
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
                    format!("{} ", spinner),
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
                ToolState::Success => ("✓".to_string(), RColor::Green),
                ToolState::Error => ("✗".to_string(), RColor::Red),
            };

            let mut spans = Vec::new();
            spans.push(Span::styled(
                format!("{} ", icon),
                Style::default().fg(color),
            ));
            spans.push(Span::styled(
                TuiApp::display_tool_name(&tool.name),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));

            let args_preview = TuiApp::format_args_preview(&tool.args);
            if !args_preview.is_empty() {
                spans.push(Span::styled(
                    format!(" • {}", args_preview),
                    Style::default().fg(RColor::Gray),
                ));
            }

            lines.push(Line::from(spans));
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
            lines
                .into_iter()
                .rev()
                .take(area.height as usize)
                .rev()
                .collect()
        } else {
            lines
        };

        let status = Paragraph::new(display_lines);
        f.render_widget(status, area);
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let input_text = format!("▶ {}", self.input);
        let input = Paragraph::new(input_text.as_str()).style(Style::default().fg(RColor::White));
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
    viewport_height: u16,
    state: AppState,
}

/// Simple newline-gated stream collector (Codex-inspired).
struct StreamCollector {
    buffer: String,
}

impl StreamCollector {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    fn push(&mut self, delta: &str) -> Vec<String> {
        if delta.is_empty() {
            return Vec::new();
        }
        let clean = clean_text(delta);
        if clean.is_empty() {
            return Vec::new();
        }
        self.buffer.push_str(&clean);
        let mut out = Vec::new();
        if let Some(idx) = self.buffer.rfind('\n') {
            let complete = self.buffer[..=idx].to_string();
            self.buffer = self.buffer[idx + 1..].to_string();
            out.extend(
                complete
                    .split('\n')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
            );
        }
        out
    }

    fn finalize(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.buffer.is_empty() {
            out.push(self.buffer.clone());
        }
        self.buffer.clear();
        out
    }
}

fn clean_text(s: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\[\d{1,3}(?:;\d{1,3})*m").unwrap());
    let stripped = strip_ansi_codes(s);
    re.replace_all(&stripped, "").to_string()
}

impl TuiApp {
    pub fn new(app: App) -> Result<Self> {
        enable_raw_mode()?;

        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let (width, height) = terminal::size()?;

        let viewport = Viewport::Inline(VIEWPORT_HEIGHT);
        let terminal = Terminal::with_options(backend, TerminalOptions { viewport })?;
        let viewport_height = VIEWPORT_HEIGHT;

        Ok(Self {
            terminal,
            viewport_height,
            state: AppState::new(app, width, height),
        })
    }

    fn rebuild_terminal(&mut self, viewport_height: u16) -> Result<()> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let options = TerminalOptions {
            viewport: Viewport::Inline(viewport_height),
        };
        self.terminal = Terminal::with_options(backend, options)?;
        self.viewport_height = viewport_height;
        Ok(())
    }

    fn required_viewport_height(&self) -> u16 {
        // Status + input line; clamp to available screen height to avoid overdraw or scrollback overwrites.
        let status_height = self.state.calculate_status_height();
        let total = status_height.saturating_add(1);
        let max_height = self.state.screen_height.max(1);
        total.min(max_height).max(1)
    }

    fn display_tool_name(name: &str) -> &str {
        match name.to_lowercase().as_str() {
            "execute_bash" => "Shell",
            "read_file" => "Read",
            "write_file" => "Write",
            "edit_file" => "Edit",
            "list_directory" => "List",
            "search_files" => "Search",
            "web_search" => "Web",
            "mcp_call" => "MCP",
            "visioneer" => "Vision",
            _ => name,
        }
    }

    fn shorten(text: &str, max: usize) -> String {
        if text.len() <= max {
            text.to_string()
        } else if max <= 3 {
            "...".to_string()
        } else {
            format!("{}...", &text[..max.saturating_sub(3)])
        }
    }

    fn format_args_preview(arguments: &str) -> String {
        let arguments = clean_text(arguments);
        if arguments.trim().is_empty() {
            return String::new();
        }

        // Try to parse JSON and show the first key/value succinctly
        if let Ok(val) = serde_json::from_str::<Value>(&arguments) {
            if let Some(obj) = val.as_object() {
                if let Some((key, value)) = obj.iter().next() {
                    let rendered_val = if value.is_string() {
                        value.as_str().unwrap_or_default().to_string()
                    } else {
                        value.to_string()
                    };
                    return format!("{}: {}", key, Self::shorten(&rendered_val, 30));
                }
            }
            if let Some(s) = val.as_str() {
                return Self::shorten(s, 30);
            }
        }

        Self::shorten(arguments.trim(), 30)
    }

    fn summarize_tool_result(result: &Value, success: bool) -> String {
        // Prefer structured fields
        if let Some(obj) = result.as_object() {
            for key in [
                "summary", "message", "stdout", "output", "path", "result", "content",
            ] {
                if let Some(val) = obj.get(key) {
                    if let Some(s) = val.as_str() {
                        return Self::shorten(&clean_text(s), 60);
                    }
                    if !val.is_null() {
                        return Self::shorten(&clean_text(&val.to_string()), 60);
                    }
                }
            }
            if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
                return Self::shorten(&clean_text(err), 60);
            }
        }

        if let Some(s) = result.as_str() {
            return Self::shorten(&clean_text(s), 60);
        }

        if !success {
            return "Failed".to_string();
        }

        "Done".to_string()
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut needs_redraw = true;

        loop {
            let mut redraw = needs_redraw;

            // Update screen size
            if let Ok((w, h)) = terminal::size() {
                // If the terminal is momentarily zero-sized during rotation, skip work this frame.
                if w == 0 || h == 0 {
                    continue;
                }
                if self.state.screen_width != w || self.state.screen_height != h {
                    self.state.screen_width = w;
                    self.state.screen_height = h;
                    redraw = true;
                }
            }

            // Grow/shrink inline viewport to match current status/input needs.
            let needed_height = self.required_viewport_height();
            if needed_height != self.viewport_height {
                self.rebuild_terminal(needed_height)?;
                redraw = true;
            }
            // Keep buffer in sync with terminal size so scrollback stays intact.
            self.terminal.autoresize()?;

            // Flush pending history BEFORE drawing the viewport
            if !self.state.pending_history.is_empty() {
                // Ensure the scrollback insertion uses the latest terminal size after any resize.
                self.terminal.autoresize()?;

                let lines: Vec<_> = self
                    .state
                    .pending_history
                    .iter()
                    .map(|h| h.to_line().to_owned())
                    .collect();

                insert_history_lines(
                    &mut self.terminal,
                    self.state.screen_width,
                    self.state.screen_height,
                    self.viewport_height,
                    lines,
                )?;
                self.state.pending_history.clear();
                redraw = true;
            }

            // Handle events - only Press events (not Release or Repeat)
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => {
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
                                let byte_pos = self
                                    .state
                                    .input
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
                                    if let Some((byte_pos, _ch)) =
                                        self.state.input.char_indices().nth(self.state.input_cursor)
                                    {
                                        self.state.input.remove(byte_pos);
                                    }
                                }
                            }
                            KeyCode::Delete => {
                                let char_count = self.state.input.chars().count();
                                if self.state.input_cursor < char_count {
                                    if let Some((byte_pos, _)) =
                                        self.state.input.char_indices().nth(self.state.input_cursor)
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
                                redraw = true;
                            }
                            _ => {}
                        }
                    }
                    Event::Resize(w, h) => {
                        // Ignore transient zero-size events that happen during orientation changes.
                        if w == 0 || h == 0 {
                            continue;
                        }
                        self.state.screen_width = w;
                        self.state.screen_height = h;
                        // Rebuild inline viewport to the current needs after resize.
                        let needed_height = self.required_viewport_height();
                        if needed_height != self.viewport_height {
                            self.rebuild_terminal(needed_height)?;
                        } else {
                            self.terminal.autoresize()?;
                        }
                        // Realign ratatui buffers to the new terminal size so scrollback isn't wiped.
                        // Skip the rest of this frame; next loop will redraw with the new dimensions.
                        needs_redraw = true;
                        continue;
                    }
                    _ => {}
                }
            }

            // Poll AI
            if self.state.is_waiting {
                if self.poll_ai_response()? {
                    redraw = true;
                }
            }

            // Animate while waiting or when active tools/thinking are visible
            if self.state.tick()
                && (self.state.is_waiting
                    || !self.state.active_tools.is_empty()
                    || !self.state.thinking_content.is_empty()
                    || !self.state.current_response.is_empty())
            {
                redraw = true;
            }

            if redraw {
                self.terminal.draw(|f| self.state.render_viewport(f))?;
                redraw = false;
            }

            needs_redraw = redraw;
        }
    }

    async fn submit_message(&mut self) -> Result<()> {
        let message = self.state.input.clone();
        self.state.input.clear();
        self.state.input_cursor = 0;

        self.state.add_user_message(&message);
        self.state.last_ai_message = None;

        self.state.is_waiting = true;
        self.state.current_response.clear();
        self.state.thinking_content.clear();
        self.state.active_tools.clear();

        self.state.app.send_to_ai(&message).await?;
        Ok(())
    }

    fn poll_ai_response(&mut self) -> Result<bool> {
        let mut changed = false;
        while let Some(response) = self.state.app.check_ai_response_nonblocking() {
            match response {
                AiResponse::AgentStreamStart => {}
                AiResponse::AgentStreamText(text) => {
                    let clean = clean_text(&text);
                    self.state.current_response.push_str(&clean);
                    let completed = self.state.stream_collector.push(&clean);
                    if !completed.is_empty() {
                        let joined = completed.join("\n");
                        self.state.add_ai_message(&joined);
                    }
                    changed = true;
                }
                AiResponse::AgentThinkingStart => {
                    self.state.thinking_content.clear();
                    changed = true;
                }
                AiResponse::AgentThinkingContent(content) => {
                    self.state.thinking_content.push_str(&content);
                    changed = true;
                }
                AiResponse::AgentThinkingEnd => {}
                AiResponse::AgentToolCall {
                    id,
                    name,
                    arguments,
                } => {
                    // Drop fully completed tools so the status area doesn't grow indefinitely.
                    self.state
                        .active_tools
                        .retain(|t| t.status == ToolState::Running || t.id == id);

                    // Log tool call to history so it scrolls up
                    self.state.add_tool_message(&name, &arguments);

                    // Update existing entry or push new
                    if let Some(existing) = self.state.active_tools.iter_mut().find(|t| t.id == id)
                    {
                        existing.name = name.clone();
                        existing.args = arguments.clone();
                        existing.status = ToolState::Running;
                        existing.started_at = Instant::now();
                        existing.finished_at = None;
                        existing.summary = None;
                    } else {
                        self.state.active_tools.push(ToolExecution {
                            id,
                            name,
                            args: arguments,
                            status: ToolState::Running,
                            started_at: Instant::now(),
                            finished_at: None,
                            summary: None,
                        });
                    }
                    changed = true;
                }
                AiResponse::AgentToolResult {
                    tool_call_id,
                    success,
                    result,
                } => {
                    if let Some(pos) = self
                        .state
                        .active_tools
                        .iter()
                        .position(|t| t.id == tool_call_id)
                    {
                        let mut tool = self.state.active_tools.remove(pos);
                        tool.status = if success {
                            ToolState::Success
                        } else {
                            ToolState::Error
                        };
                        tool.finished_at = Some(Instant::now());
                        tool.summary = Some(Self::summarize_tool_result(&result, success));

                        // Push a concise result line into history with duration and summary.
                        let mut spans = vec![
                            HistorySpan::new("🔧 ").fg(Color::Magenta).bold(),
                            HistorySpan::new(Self::display_tool_name(&tool.name)).bold(),
                        ];
                        let args_preview = Self::format_args_preview(&tool.args);
                        if !args_preview.is_empty() {
                            spans.push(HistorySpan::new(" • ").dim());
                            spans.push(HistorySpan::new(args_preview));
                        }
                        if let Some(summary) = &tool.summary {
                            spans.push(HistorySpan::new(" — ").dim());
                            spans.push(HistorySpan::new(summary.clone()).fg(if success {
                                Color::Green
                            } else {
                                Color::Red
                            }));
                        }
                        if let Some(done_at) = tool.finished_at {
                            let duration_ms = done_at
                                .saturating_duration_since(tool.started_at)
                                .as_millis();
                            spans.push(HistorySpan::new(format!(" • {}ms", duration_ms)).dim());
                        }
                        self.state.pending_history.push(HistoryLine::new(spans));

                        // Keep only running tools visible in the status list to avoid duplication.
                        self.state
                            .active_tools
                            .retain(|t| t.status == ToolState::Running);
                    }
                    changed = true;
                }
                AiResponse::AgentStreamEnd => {
                    let remaining = self.state.stream_collector.finalize();
                    if !remaining.is_empty() {
                        for line in remaining {
                            self.state.add_ai_message(&line);
                        }
                    } else {
                        let first_line = self
                            .state
                            .current_response
                            .lines()
                            .find(|l| !l.trim().is_empty())
                            .map(|s| clean_text(s));
                        if let Some(line) = first_line {
                            self.state.add_ai_message(&line);
                        }
                    }
                    self.state.current_response.clear();
                    self.state.stream_collector.buffer.clear();
                    self.state.active_tools.clear();
                    self.state.thinking_content.clear();
                    self.state.is_waiting = false;
                    changed = true;
                }
                _ => {}
            }
        }
        Ok(changed)
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
                execute!(
                    io::stdout(),
                    terminal::Clear(terminal::ClearType::All),
                    crossterm::cursor::MoveTo(0, 0)
                )?;
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
                execute!(
                    io::stdout(),
                    terminal::Clear(terminal::ClearType::All),
                    crossterm::cursor::MoveTo(0, 0)
                )?;
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
                execute!(
                    io::stdout(),
                    terminal::Clear(terminal::ClearType::All),
                    crossterm::cursor::MoveTo(0, 0)
                )?;
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
