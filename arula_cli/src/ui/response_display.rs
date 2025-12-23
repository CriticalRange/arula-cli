use crate::ui::tui::InlineRenderer;
use crate::ui::widgets::{
    status::{ToolStatus, ToolStatusWidget},
    thinking::{AnimationState, ThinkingWidget},
};
use anyhow::Result;
use arula_core::api::agent::ToolResult;
use arula_core::app::AiResponse;
use ratatui::layout::{Constraint, Direction, Layout};
use std::io::{self, Write};
use std::time::Instant;

/// Enhanced response display using Ratatui for TUI widgets
pub struct ResponseDisplay {
    renderer: InlineRenderer,

    // State
    is_thinking: bool,
    thinking_content: String,
    thinking_frame: usize,
    thinking_expanded: bool,
    thinking_animation_state: AnimationState,
    thinking_animation_start: Option<Instant>,

    current_tools: Vec<ToolState>,

    // Animation timing
    last_update: Instant,
}

struct ToolState {
    id: String,
    name: String,
    args: String,
    status: ToolStatus,
    result_summary: Option<String>,
    start_time: Instant,
}

impl ResponseDisplay {
    pub fn new() -> Result<Self> {
        // Start with a small height, will grow as needed
        let renderer = InlineRenderer::new(1)?;

        Ok(Self {
            renderer,
            is_thinking: false,
            thinking_content: String::new(),
            thinking_frame: 0,
            thinking_expanded: false,
            thinking_animation_state: AnimationState::Idle,
            thinking_animation_start: None,
            current_tools: Vec::new(),
            last_update: Instant::now(),
        })
    }

    /// Clear the TUI area (useful before printing final output or exiting)
    pub fn clear(&mut self) -> Result<()> {
        self.renderer.clear()
    }

    /// Toggle thinking bubble expansion
    pub fn toggle_thinking_expansion(&mut self) {
        if self.thinking_expanded {
            // Start fade out
            self.thinking_animation_state = AnimationState::FadingOut { progress: 0 };
            self.thinking_animation_start = Some(Instant::now());
            self.thinking_expanded = false;
        } else {
            // Start fade in
            self.thinking_expanded = true;
            self.thinking_animation_state = AnimationState::FadingIn { progress: 0 };
            self.thinking_animation_start = Some(Instant::now());
        }
    }

    /// Update the display based on an AI response event
    pub fn update(&mut self, response: &AiResponse) -> Result<()> {
        match response {
            AiResponse::AgentStreamText(text) => {
                // For text, we clear the TUI, print the text, then let the next draw restore the TUI
                // This ensures text ends up in scrollback
                self.renderer.clear()?;

                print!("{}", text);
                io::stdout().flush()?;
            }
            AiResponse::AgentThinkingStart => {
                if !self.is_thinking {
                    self.is_thinking = true;
                    self.thinking_content.clear();
                    self.thinking_expanded = false;
                    self.thinking_animation_state = AnimationState::Idle;
                }
            }
            AiResponse::AgentThinkingContent(text) => {
                self.thinking_content.push_str(text);
            }
            AiResponse::AgentThinkingEnd => {
                // thinking finished - keep collapsed state
            }

            AiResponse::AgentToolCall {
                id,
                name,
                arguments,
            } => {
                if !self.current_tools.iter().any(|t| t.id == *id) {
                    self.current_tools.push(ToolState {
                        id: id.clone(),
                        name: name.clone(),
                        args: arguments.clone(),
                        status: ToolStatus::Running,
                        result_summary: None,
                        start_time: Instant::now(),
                    });
                }
            }
            AiResponse::AgentToolResult {
                tool_call_id,
                success,
                result,
            } => {
                // Handle result logic if needed or just wait for `handle_tool_result` call
                // Actually `update` receives `response`.
                // If I handle it here, I don't need `handle_tool_result`.
                // Let's implement logic here.

                let summary = self.summarize_result(result);
                // Look for tool by ID first
                if let Some(tool) = self
                    .current_tools
                    .iter_mut()
                    .find(|t| t.id == *tool_call_id)
                {
                    tool.status = if *success {
                        ToolStatus::Success
                    } else {
                        ToolStatus::Error
                    };
                    tool.result_summary = Some(summary);
                }
                // Fallback to name match for legacy/openrouter if ID is missing?
            }
            AiResponse::AgentStreamEnd => {
                self.is_thinking = false;
                // Maybe we keep tools visible? Or clear them?
                // Usually for a chat CLI, once turn is done, we clear status bars.
                self.clear()?;
                self.current_tools.clear();
            }
            _ => {}
        }
        Ok(())
    }

    // Explicit handler for tool results since they might come from a different channel
    pub fn handle_tool_result(&mut self, _id: &str, name: &str, result: &ToolResult) -> Result<()> {
        let summary = self.summarize_result(&result.data);
        if let Some(tool) = self
            .current_tools
            .iter_mut()
            .find(|t| t.name == name && t.status == ToolStatus::Running)
        {
            tool.status = if result.success {
                ToolStatus::Success
            } else {
                ToolStatus::Error
            };
            tool.result_summary = Some(summary);
        }
        Ok(())
    }

    /// Summarize tool result for display
    fn summarize_result(&self, data: &serde_json::Value) -> String {
        // Simplified logic from old code
        if let Some(s) = data.as_str() {
            if s.len() > 40 {
                format!("{}...", &s[..37])
            } else {
                s.to_string()
            }
        } else {
            "Done".to_string()
        }
    }

    /// Render the current frame
    pub fn draw(&mut self) -> Result<()> {
        // Update animation frames
        if self.last_update.elapsed().as_millis() > 100 {
            self.thinking_frame += 1;
            self.last_update = Instant::now();
        }

        // Update fade animation
        if let Some(start) = self.thinking_animation_start {
            const FADE_DURATION_MS: u64 = 300; // 300ms fade duration
            let elapsed = start.elapsed().as_millis() as u64;

            match self.thinking_animation_state {
                AnimationState::FadingIn { ref mut progress } => {
                    if elapsed >= FADE_DURATION_MS {
                        *progress = 255;
                        self.thinking_animation_state = AnimationState::FullyVisible;
                        self.thinking_animation_start = None;
                    } else {
                        *progress = ((elapsed as f64 / FADE_DURATION_MS as f64) * 255.0) as u8;
                    }
                }
                AnimationState::FadingOut { ref mut progress } => {
                    if elapsed >= FADE_DURATION_MS {
                        *progress = 255;
                        self.thinking_animation_state = AnimationState::Idle;
                        self.thinking_animation_start = None;
                    } else {
                        *progress = ((elapsed as f64 / FADE_DURATION_MS as f64) * 255.0) as u8;
                    }
                }
                _ => {
                    self.thinking_animation_start = None;
                }
            }
        }

        // Calculate functionality needed
        let show_thinking = self.is_thinking && !self.thinking_content.is_empty();
        let show_tools = !self.current_tools.is_empty();

        if !show_thinking && !show_tools {
            // Nothing to draw, maybe just finish?
            // If we cleared, we are good.
            return Ok(());
        }

        // We need 1 line per tool + few for thinking?
        // Thinking widget height: borders + content.
        // Let's dynamic size.

        let thinking_height = if show_thinking {
            // When collapsed, minimal height (3 lines: top, content, bottom)
            // When expanded, show full content
            if self.thinking_expanded {
                let lines = self.thinking_content.lines().count().min(5).max(1);
                (lines + 2) as u16
            } else {
                3 // Always 3 lines when collapsed
            }
        } else {
            0
        };

        let tools_height = self.current_tools.len() as u16;

        let total_height = thinking_height + tools_height;

        if total_height == 0 {
            return Ok(());
        }

        self.renderer.resize(total_height)?;

        let thinking_content = &self.thinking_content;
        let thinking_frame = self.thinking_frame;
        let is_thinking = self.is_thinking;
        let thinking_expanded = self.thinking_expanded;
        let thinking_animation_state = self.thinking_animation_state;
        let tools = &self.current_tools;

        self.renderer.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(thinking_height),
                    Constraint::Length(tools_height),
                ])
                .split(f.area());

            if show_thinking {
                let widget = ThinkingWidget::new(thinking_content, thinking_frame, is_thinking)
                    .with_expanded(thinking_expanded)
                    .with_animation(thinking_animation_state);
                f.render_widget(widget, chunks[0]);
            }

            if show_tools {
                let area = if show_thinking { chunks[1] } else { chunks[0] };
                let tool_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(vec![Constraint::Length(1); tools.len()])
                    .split(area);

                for (i, tool) in tools.iter().enumerate() {
                    let widget = ToolStatusWidget::new(&tool.name, &tool.args, tool.status.clone())
                        .with_frame(thinking_frame); // Reuse frame counter

                    let widget = if let Some(summary) = &tool.result_summary {
                        widget.with_result(summary)
                    } else {
                        widget
                    };

                    if i < tool_chunks.len() {
                        f.render_widget(widget, tool_chunks[i]);
                    }
                }
            }
        })?;

        Ok(())
    }
}
