use anyhow::Result;
use console::strip_ansi_codes;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::Color,
    terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType},
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
use arula_core::prelude::detect_project;
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

/// The TUI viewport height (input + info line)
const VIEWPORT_HEIGHT: u16 = 2;

/// Application state (separate from terminal for borrow checker)
struct AppState {
    input: String,
    input_cursor: usize,
    is_waiting: bool,
    thinking_content: String,
    thinking_expanded: bool,
    stream_collector: StreamCollector,
    active_tools: Vec<ToolExecution>,
    current_response: String,
    pending_history: Vec<HistoryLine>,
    frame: usize,
    last_tick: Instant,
    screen_height: u16,
    screen_width: u16,
    last_ai_message: Option<String>,
    last_history_kind: Option<HistoryKind>,
    app: App,
    /// Conversation starters from AI
    conversation_starters: Vec<String>,
    /// Whether starters are being fetched
    fetching_starters: bool,
    /// Currently selected starter index (for keyboard navigation)
    selected_starter: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryKind {
    User,
    Ai,
    Tool,
}

impl AppState {
    fn new(app: App, width: u16, height: u16) -> Self {
        Self {
            input: String::new(),
            input_cursor: 0,
            is_waiting: false,
            thinking_content: String::new(),
            thinking_expanded: false,
            stream_collector: StreamCollector::new(),
            active_tools: Vec::new(),
            current_response: String::new(),
            pending_history: Vec::new(),
            frame: 0,
            last_tick: Instant::now(),
            screen_height: height,
            screen_width: width,
            last_ai_message: None,
            last_history_kind: None,
            app,
            conversation_starters: Vec::new(),
            fetching_starters: false,
            selected_starter: None,
        }
    }

    fn add_user_message(&mut self, message: &str) {
        let clean = clean_text(message);
        self.push_history(
            HistoryKind::User,
            HistoryLine::new(vec![
                HistorySpan::new("▶ You: ").fg(Color::Cyan).bold(),
                HistorySpan::new(clean),
            ]),
        );
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
            self.push_history(
                HistoryKind::Ai,
                HistoryLine::new(vec![HistorySpan::new(first.to_string())]),
            );
        }

        for line in lines {
            self.push_history(
                HistoryKind::Ai,
                HistoryLine::new(vec![
                    HistorySpan::new("      "), // Indentation to align with text
                    HistorySpan::new(line.to_string()),
                ]),
            );
        }

        self.last_ai_message = Some(message);
    }

    fn add_tool_message(&mut self, name: &str, args: &str) {
        let clean_args = clean_text(args);
        self.push_history(
            HistoryKind::Tool,
            HistoryLine::new(vec![
                HistorySpan::new("🔧 Tool: ").fg(Color::Magenta).bold(),
                HistorySpan::new(name).bold(),
                HistorySpan::new(format!(" {}", clean_args)).dim(),
            ]),
        );
    }

    fn push_history(&mut self, kind: HistoryKind, line: HistoryLine) {
        if let Some(last) = self.last_history_kind {
            if last != kind {
                self.pending_history.push(HistoryLine::plain(""));
            }
        }
        self.pending_history.push(line);
        self.last_history_kind = Some(kind);
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

        // Always reserve space for input and info at the bottom
        let input_height = 1;
        let info_height = 1;

        // Calculate available space for status (above input and info)
        let bottom_reserved = input_height + info_height;
        let status_max_height = area.height.saturating_sub(bottom_reserved);

        // Get actual status height, but clamp it to available space
        let status_height = self.status_height().min(status_max_height);

        // Build layout with input ALWAYS at the bottom
        let mut constraints = Vec::new();

        // Status takes whatever space it needs (or is available)
        if status_height > 0 {
            constraints.push(Constraint::Length(status_height));
        } else {
            // If no status, add empty space to push input down
            constraints.push(Constraint::Min(0));
        }

        // Fill any remaining space (this ensures input stays at bottom)
        if status_height + bottom_reserved < area.height {
            constraints.push(Constraint::Min(0));
        }

        // Input row (always present)
        constraints.push(Constraint::Length(input_height));

        // Info row (if we have space)
        if area.height > 1 {
            constraints.push(Constraint::Length(info_height));
        }

        // Create the layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        // Render status if present
        if status_height > 0 && !chunks.is_empty() {
            self.render_status_box(f, chunks[0]);
        }

        // Render input (always at bottom of its allocated space)
        let input_idx = chunks.len().saturating_sub(2);
        if input_idx < chunks.len() {
            self.render_input(f, chunks[input_idx]);
        }

        // Render info at very bottom
        if !chunks.is_empty() {
            let info_idx = chunks.len() - 1;
            self.render_info(f, chunks[info_idx]);
        }
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        // Ensure area is valid
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Clear the input area to prevent artifacts
        f.render_widget(ratatui::widgets::Clear, area);

        // Create input text with styled prompt
        let prompt_color = if self.is_waiting {
            RColor::Yellow
        } else {
            RColor::Cyan
        };

        let input_text = Line::from(vec![
            Span::styled("▶ ", Style::default().fg(prompt_color).add_modifier(Modifier::BOLD)),
            Span::styled(&self.input, Style::default().fg(RColor::White)),
        ]);

        let input = Paragraph::new(input_text)
            .style(Style::default().fg(RColor::White).bg(RColor::Rgb(12, 12, 16)))
            .block(
                ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::TOP)
                    .border_style(Style::default().fg(RColor::Rgb(80, 80, 80)))
            );

        f.render_widget(input, area);

        // Calculate cursor X position with bounds checking
        let prompt_width = 2; // Width of "▶ "
        let input_char_count = self.input.chars().take(self.input_cursor).count() as u16;

        // Ensure cursor stays within the input area (minus border)
        let max_cursor_x = area.width.saturating_sub(1); // Leave 1 char for border
        let cursor_offset = input_char_count.min(max_cursor_x.saturating_sub(prompt_width));
        let cursor_x = area.x + prompt_width + cursor_offset;

        // Cursor Y is at the input line (accounting for top border)
        let cursor_y = area.y;

        // Only set cursor if it's within bounds
        if cursor_x < area.x + area.width && cursor_y <= area.y + area.height {
            f.set_cursor_position((cursor_x, cursor_y));
        }
    }

    fn render_info(&self, f: &mut Frame, area: Rect) {
        // Add a subtle background to the info line
        let info = Paragraph::new(self.info_line())
            .style(Style::default().bg(RColor::Rgb(15, 15, 20)).fg(RColor::Rgb(180, 180, 180)));
        f.render_widget(info, area);
    }

    fn info_line(&self) -> Line<'static> {
        let spinner = ["◐", "◓", "◑", "◒"][self.frame % 4];
        let mut spans = Vec::new();

        if self.is_waiting {
            // Active tools take priority so users see progress.
            if let Some(tool) = self.active_tools.first() {
                let name = TuiApp::display_tool_name(&tool.name);
                let label = if self.active_tools.len() > 1 {
                    format!("{name} (+{})", self.active_tools.len() - 1)
                } else {
                    name.to_string()
                };
                spans.push(Span::styled(
                    format!("{spinner} "),
                    Style::default().fg(RColor::Yellow).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("🔧 ", Style::default().fg(RColor::Yellow)));
                spans.push(Span::styled(label, Style::default().fg(RColor::Rgb(220, 220, 150))));
            } else if !self.thinking_content.is_empty() {
                let preview = TuiApp::thinking_preview(&self.thinking_content, 32)
                    .unwrap_or_else(|| "Thought...".to_string());
                spans.push(Span::styled(
                    format!("{spinner} "),
                    Style::default().fg(RColor::Magenta).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("💭 ", Style::default().fg(RColor::Magenta)));
                spans.push(Span::styled(preview, Style::default().fg(RColor::Rgb(200, 180, 220)).add_modifier(Modifier::DIM)));
            } else if !self.current_response.is_empty() {
                spans.push(Span::styled(
                    format!("{spinner} "),
                    Style::default().fg(RColor::Cyan).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("✨ ", Style::default().fg(RColor::Cyan)));
                spans.push(Span::styled("AI typing...", Style::default().fg(RColor::Rgb(180, 220, 240))));
                let preview = self
                    .current_response
                    .lines()
                    .last()
                    .unwrap_or("")
                    .to_string();
                if !preview.is_empty() {
                    let truncated = if preview.len() > 30 {
                        format!("{}...", &preview[..27])
                    } else {
                        preview
                    };
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(truncated, Style::default().fg(RColor::Rgb(150, 180, 200)).add_modifier(Modifier::DIM)));
                }
            } else {
                spans.push(Span::styled(
                    format!("{spinner} "),
                    Style::default().fg(RColor::Cyan).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("⚡ Working", Style::default().fg(RColor::Cyan)));
            }
        } else {
            spans.push(Span::styled(
                "● ",
                Style::default().fg(RColor::Green).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("Ready", Style::default().fg(RColor::Rgb(150, 255, 150)).add_modifier(Modifier::DIM)));
        }

        // Separator
        spans.push(Span::styled(
            "  │  ",
            Style::default().fg(RColor::Rgb(60, 60, 60)),
        ));

        // Model badge with improved styling
        let model = self.app.config.get_model();
        spans.push(Span::styled(
            model,
            Style::default()
                .fg(RColor::Rgb(100, 140, 180))
                .add_modifier(Modifier::ITALIC)
                .add_modifier(Modifier::DIM),
        ));

        // Separator
        spans.push(Span::styled(
            "  │  ",
            Style::default().fg(RColor::Rgb(60, 60, 60)),
        ));

        spans.push(Span::styled(
            "Shift+Tab",
            Style::default().fg(RColor::Rgb(140, 140, 140)).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            " menu",
            Style::default().fg(RColor::Rgb(100, 100, 100)).add_modifier(Modifier::DIM),
        ));

        Line::from(spans)
    }

    fn status_height(&self) -> u16 {
        let mut height = 0;
        if self.is_waiting && !self.thinking_content.is_empty() {
            if self.thinking_expanded {
                // Expanded mode: title line + up to 5 content lines + bottom border
                let content_lines = self.thinking_content.lines().count().min(5);
                height += 1 + content_lines as u16 + 1;
            } else {
                // Collapsed mode: just 1 line
                height += 1;
            }
        }
        if self.is_waiting && !self.active_tools.is_empty() {
            height += 1;
        }

        // Limit status height to prevent overflow
        // We need at least 2 lines for input and info
        let max_status_height = self.screen_height.saturating_sub(2);
        height.min(max_status_height)
    }

    fn status_lines(&self) -> Vec<Line<'_>> {
        let mut lines = Vec::new();
        let border = Style::default().fg(RColor::Rgb(100, 100, 120));

        if self.is_waiting && !self.active_tools.is_empty() {
            let spinner = ["◐", "◓", "◑", "◒"][self.frame % 4];
            let first = &self.active_tools[0];
            let label = TuiApp::display_tool_name(&first.name);
            let active_count = self.active_tools.len();
            let elapsed_ms = first
                .finished_at
                .unwrap_or_else(Instant::now)
                .saturating_duration_since(first.started_at)
                .as_millis();
            let args_preview = TuiApp::format_args_preview(&first.args);
            let mut spans = Vec::new();
            spans.push(Span::styled("┌", border));
            spans.push(Span::styled(
                format!(" {spinner} Tool 1/{} ", active_count),
                Style::default()
                    .fg(RColor::Rgb(255, 220, 100))
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("┐ ", border));
            spans.push(Span::styled(
                label.to_string(),
                Style::default().fg(RColor::Rgb(255, 220, 100)),
            ));
            spans.push(Span::raw("  "));
            if !args_preview.is_empty() {
                spans.push(Span::styled(
                    args_preview,
                    Style::default().fg(RColor::Rgb(180, 180, 180)),
                ));
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(
                format!("{}ms", elapsed_ms),
                Style::default().fg(RColor::Rgb(120, 120, 120)).add_modifier(Modifier::DIM),
            ));
            lines.push(Line::from(spans));
        }

        if self.is_waiting && !self.thinking_content.is_empty() {
            let spinner = ["◐", "◓", "◑", "◒"][self.frame % 4];

            if self.thinking_expanded {
                // Expanded mode - show full content
                let mut spans = Vec::new();
                spans.push(Span::styled("┌", border));
                spans.push(Span::styled(
                    format!(" {spinner} Thought "),
                    Style::default()
                        .fg(RColor::Rgb(255, 150, 255))
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("┐", border));
                lines.push(Line::from(spans));

                // Show full thought content (up to 5 lines)
                for line in self.thinking_content.lines().take(5) {
                    lines.push(Line::from(vec![
                        Span::styled("│ ", border),
                        Span::styled(line, Style::default().fg(RColor::Rgb(200, 180, 220))),
                    ]));
                }

                // Close the box
                let _content_lines = self.thinking_content.lines().count().min(5);
                let bottom_spans = vec![
                    Span::styled("└", border),
                    Span::styled("──────────────────────────────────────────────────────────", border),
                    Span::styled("┘", border),
                ];
                lines.push(Line::from(bottom_spans));
            } else {
                // Collapsed mode - show preview with "..." if truncated
                let preview = TuiApp::thinking_preview(&self.thinking_content, 48)
                    .unwrap_or_else(|| "Thought...".to_string());

                let mut spans = Vec::new();
                spans.push(Span::styled("┌", border));
                spans.push(Span::styled(
                    format!(" {spinner} Thought "),
                    Style::default()
                        .fg(RColor::Rgb(255, 150, 255))
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled("┐ ", border));

                // Add "..." if content is longer than preview
                let full_preview = if self.thinking_content.len() > 48 {
                    format!("{}...", preview.trim_end().chars().take(45).collect::<String>())
                } else {
                    preview
                };

                spans.push(Span::styled(full_preview, Style::default().fg(RColor::Rgb(200, 180, 220)).add_modifier(Modifier::DIM)));
                lines.push(Line::from(spans));
            }
        }

        lines
    }

    fn render_status_box(&self, f: &mut Frame, area: Rect) {
        let lines = self.status_lines();
        if lines.is_empty() || area.height == 0 {
            return;
        }

        // Add a bottom border to separate status from input with improved styling
        let status = Paragraph::new(lines)
            .block(
                ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::BOTTOM)
                    .border_style(Style::default().fg(RColor::Rgb(100, 100, 120)))
            );

        f.render_widget(status, area);
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
    let re =
        RE.get_or_init(|| Regex::new(r"(\x1b\[[0-9;]*[A-Za-z]|\[\d{1,3}(?:;\d{1,3})*m)").unwrap());
    let stripped = strip_ansi_codes(s);
    re.replace_all(&stripped, "").to_string()
}

impl TuiApp {
    pub fn new(app: App) -> Result<Self> {
        enable_raw_mode()?;

        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let (width, height) = terminal::size()?;

        // Ensure we have a valid terminal size
        if width == 0 || height == 0 {
            disable_raw_mode()?;
            return Err(anyhow::anyhow!("Terminal has zero size"));
        }

        let viewport = Viewport::Inline(VIEWPORT_HEIGHT);
        let terminal = Terminal::with_options(backend, TerminalOptions { viewport })?;
        let viewport_height = VIEWPORT_HEIGHT;

        Ok(Self {
            terminal,
            viewport_height,
            state: AppState::new(app, width, height),
        })
    }

    /// Generate conversation starters based on project context
    /// This is called when the conversation is empty
    fn generate_conversation_starters(&mut self) {
        use std::path::PathBuf;

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        // Check if we have PROJECT.manifest
        let manifest_path = cwd.join("PROJECT.manifest");
        let _has_manifest = manifest_path.exists();

        // Generate context-aware starters
        let starters = if let Some(project) = detect_project(&cwd) {
            match project.project_type {
                arula_core::ProjectType::Rust => vec![
                    "Review and improve code quality".to_string(),
                    "Run tests and fix any issues".to_string(),
                    "Add new feature with proper error handling".to_string(),
                ],
                arula_core::ProjectType::Node => vec![
                    "Review dependencies and update outdated packages".to_string(),
                    "Add tests for critical functions".to_string(),
                    "Improve error handling and logging".to_string(),
                ],
                arula_core::ProjectType::Python => vec![
                    "Review code for PEP 8 compliance".to_string(),
                    "Add type hints to improve code clarity".to_string(),
                    "Write unit tests for core functionality".to_string(),
                ],
                arula_core::ProjectType::Go => vec![
                    "Review code for idiomatic Go patterns".to_string(),
                    "Add comprehensive error handling".to_string(),
                    "Write benchmarks for performance".to_string(),
                ],
                arula_core::ProjectType::Unknown => vec![
                    "Explain the project structure".to_string(),
                    "Suggest improvements to code organization".to_string(),
                    "Add documentation for key components".to_string(),
                ],
            }
        } else {
            // Default starters when no project detected
            vec![
                "Start a new conversation".to_string(),
                "Ask about my capabilities".to_string(),
                "Get help with a task".to_string(),
            ]
        };

        self.state.conversation_starters = starters;
    }

    fn rebuild_terminal(&mut self, viewport_height: u16) -> Result<()> {
        // Get current terminal size
        let (screen_w, screen_h) = terminal::size()?;

        // Ensure viewport_height doesn't exceed screen height
        let viewport_height = viewport_height.min(screen_h);

        // Save current cursor state
        let (cursor_x, cursor_y) = crossterm::cursor::position().unwrap_or((0, 0));

        // Flush any pending history BEFORE rebuilding to prevent scrollback loss
        if !self.state.pending_history.is_empty() {
            // Force a terminal sync before rebuild
            let lines: Vec<_> = self
                .state
                .pending_history
                .iter()
                .map(|h| h.to_line().to_owned())
                .collect();

            // Use the current terminal state for insertion
            if let Err(e) = insert_history_lines(
                &mut self.terminal,
                self.state.screen_width,
                self.state.screen_height,
                self.viewport_height,
                lines,
            ) {
                // Log but continue - better to lose scrollback than crash
                eprintln!("Warning: Failed to flush history before rebuild: {}", e);
            }
            self.state.pending_history.clear();
        }

        // Clear any pending input display
        execute!(
            io::stdout(),
            Hide,
            Clear(ClearType::CurrentLine)
        )?;

        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let options = TerminalOptions {
            viewport: Viewport::Inline(viewport_height),
        };
        self.terminal = Terminal::with_options(backend, options)?;
        self.viewport_height = viewport_height;

        // Restore cursor position (ensure it's within screen bounds)
        let cursor_y = cursor_y.min(screen_h - 1);
        let cursor_x = cursor_x.min(screen_w.saturating_sub(1));
        execute!(
            io::stdout(),
            MoveTo(cursor_x, cursor_y),
            Show
        )?;

        Ok(())
    }

    fn required_viewport_height(&self) -> u16 {
        // Always reserve space for input + info at bottom (2 lines)
        let bottom_reserved = 2;

        // Add status height, but ensure we don't exceed screen
        let status_height = self.state.status_height();
        let screen_height = self.state.screen_height.max(bottom_reserved);

        // Status can take available space above the bottom reserved area
        let max_status_height = screen_height.saturating_sub(bottom_reserved);
        let actual_status_height = status_height.min(max_status_height);

        // Total height = status (if any) + bottom reserved
        actual_status_height + bottom_reserved
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

    fn thinking_preview(content: &str, _max: usize) -> Option<String> {
        let last = content
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())?
            .trim()
            .to_string();
        if last.is_empty() {
            return None;
        }
        let clean = clean_text(&last);
        let stripped = Self::strip_list_prefix(&clean);
        if stripped.is_empty() {
            None
        } else {
            Some(stripped)
        }
    }

    fn strip_list_prefix(s: &str) -> String {
        let mut trimmed = s.trim_start();
        if let Some(rest) = trimmed.strip_prefix(['-', '*', '•']) {
            trimmed = rest.trim_start();
        }
        if let Some((digits, rest)) = trimmed.split_once('.') {
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                return rest.trim_start().to_string();
            }
        }
        trimmed.to_string()
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
                    return format!("{key}: {rendered_val}");
                }
            }
            if let Some(s) = val.as_str() {
                return s.to_string();
            }
        }

        arguments.trim().to_string()
    }

    fn summarize_tool_result(result: &Value, success: bool) -> String {
        // Prefer structured fields
        if let Some(obj) = result.as_object() {
            for key in [
                "summary", "message", "stdout", "output", "path", "result", "content",
            ] {
                if let Some(val) = obj.get(key) {
                    if let Some(s) = val.as_str() {
                        return clean_text(s);
                    }
                    if !val.is_null() {
                        return clean_text(&val.to_string());
                    }
                }
            }
            if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
                return clean_text(err);
            }
        }

        if let Some(s) = result.as_str() {
            return clean_text(s);
        }

        if !success {
            return "Failed".to_string();
        }

        "Done".to_string()
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut needs_redraw = true;

        // Generate conversation starters on startup (if conversation is empty)
        if self.state.app.messages.is_empty() && self.state.conversation_starters.is_empty() {
            self.generate_conversation_starters();
        }

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

            // Flush pending history FIRST before any viewport changes to preserve scrollback
            if !self.state.pending_history.is_empty() {
                // Ensure the scrollback insertion uses the latest terminal size
                self.terminal.autoresize()?;

                let lines: Vec<_> = self
                    .state
                    .pending_history
                    .iter()
                    .map(|h| h.to_line().to_owned())
                    .collect();

                // Only insert if we have space for scrollback
                let area_top = self
                    .state
                    .screen_height
                    .saturating_sub(self.viewport_height)
                    .max(1);

                if self.state.screen_height > self.viewport_height && area_top > 0 {
                    if let Err(e) = insert_history_lines(
                        &mut self.terminal,
                        self.state.screen_width,
                        self.state.screen_height,
                        self.viewport_height,
                        lines,
                    ) {
                        eprintln!("Warning: Failed to insert history: {}", e);
                    }
                }
                self.state.pending_history.clear();
                redraw = true;
            }

            // Now grow/shrink inline viewport to match current status/input needs.
            let needed_height = self.required_viewport_height();

            // Only rebuild terminal if there's a significant change
            if needed_height != self.viewport_height {
                // Use sync update to prevent flicker
                execute!(io::stdout(), crossterm::terminal::BeginSynchronizedUpdate)?;
                self.rebuild_terminal(needed_height)?;
                execute!(io::stdout(), crossterm::terminal::EndSynchronizedUpdate)?;
                redraw = true;
            }
            // Keep buffer in sync with terminal size so scrollback stays intact.
            self.terminal.autoresize()?;

            // Check for pending init message and send it
            if let Some(init_message) = self.state.app.pending_init_message.take() {
                // Add the message as if user typed it
                self.state.add_user_message(&init_message);
                self.state.last_ai_message = None;

                // Clear input BEFORE setting waiting state
                self.state.input.clear();
                self.state.input_cursor = 0;

                // Send to AI
                self.state.is_waiting = true;
                self.state.current_response.clear();
                self.state.thinking_content.clear();
                self.state.active_tools.clear();

                self.state.app.send_to_ai(&init_message).await?;
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
                            // Ctrl+1/2/3: Send conversation starter messages
                            KeyCode::Char('1') | KeyCode::Char('2') | KeyCode::Char('3') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if !self.state.conversation_starters.is_empty() {
                                    let idx = match key.code {
                                        KeyCode::Char('1') => 0,
                                        KeyCode::Char('2') => 1,
                                        KeyCode::Char('3') => 2,
                                        _ => 0,
                                    };
                                    if let Some(starter) = self.state.conversation_starters.get(idx) {
                                        self.state.input = starter.clone();
                                        self.state.input_cursor = self.state.input.chars().count();
                                        // Auto-submit the message
                                        self.submit_message().await?;
                                        redraw = true;
                                    }
                                }
                            }
                            KeyCode::Enter => {
                                if !self.state.input.is_empty() && !self.state.is_waiting {
                                    self.submit_message().await?;
                                    redraw = true;
                                }
                            }
                            KeyCode::Char('t') => {
                                // Toggle thinking bubble expansion
                                if !self.state.thinking_content.is_empty() {
                                    self.state.thinking_expanded = !self.state.thinking_expanded;
                                    redraw = true;
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
                                redraw = true;
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
                                    redraw = true;
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
                                    redraw = true;
                                }
                            }
                            KeyCode::Left => {
                                self.state.input_cursor = self.state.input_cursor.saturating_sub(1);
                                redraw = true;
                            }
                            KeyCode::Right => {
                                let char_count = self.state.input.chars().count();
                                if self.state.input_cursor < char_count {
                                    self.state.input_cursor += 1;
                                    redraw = true;
                                }
                            }
                            KeyCode::Esc => {
                                if !self.state.input.is_empty() {
                                    self.state.input.clear();
                                    self.state.input_cursor = 0;
                                    redraw = true;
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

                        // Flush pending history BEFORE resize to preserve scrollback
                        if !self.state.pending_history.is_empty() {
                            let lines: Vec<_> = self
                                .state
                                .pending_history
                                .iter()
                                .map(|h| h.to_line().to_owned())
                                .collect();

                            if let Err(e) = insert_history_lines(
                                &mut self.terminal,
                                self.state.screen_width,
                                self.state.screen_height,
                                self.viewport_height,
                                lines,
                            ) {
                                eprintln!("Warning: Failed to flush history on resize: {}", e);
                            }
                            self.state.pending_history.clear();
                        }

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
                    self.state.thinking_expanded = false;
                    changed = true;
                }
                AiResponse::AgentThinkingContent(content) => {
                    self.state.thinking_content.push_str(&content);
                    changed = true;
                }
                AiResponse::AgentThinkingEnd => {
                    // Reset expansion state when thinking ends
                    self.state.thinking_expanded = false;
                }
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
                        self.state
                            .push_history(HistoryKind::Tool, HistoryLine::new(spans));

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
