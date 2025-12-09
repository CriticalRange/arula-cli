use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Widget,
};

/// Status of a tool call
#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    Running,
    Success,
    Error,
}

/// A Ratatui widget for displaying tool execution status
pub struct ToolStatusWidget<'a> {
    pub name: &'a str,
    pub args: &'a str,
    pub status: ToolStatus,
    pub result_summary: Option<&'a str>,
    pub frame: usize,
}

impl<'a> ToolStatusWidget<'a> {
    pub fn new(name: &'a str, args: &'a str, status: ToolStatus) -> Self {
        Self {
            name,
            args,
            status,
            result_summary: None,
            frame: 0,
        }
    }

    pub fn with_result(mut self, summary: &'a str) -> Self {
        self.result_summary = Some(summary);
        self
    }

    pub fn with_frame(mut self, frame: usize) -> Self {
        self.frame = frame;
        self
    }

    fn get_icon(&self) -> &'static str {
        match self.name.to_lowercase().as_str() {
            "execute_bash" => "○",
            "read_file" => "○",
            "write_file" | "edit_file" => "□",
            "list_directory" => "◇",
            "search_files" => "○",
            "web_search" => "⭕",
            "mcp_call" => "◊",
            "visioneer" => "○",
            "capture_screen" => "●",
            "analyze_ui" => "◉",
            _ => "□",
        }
    }

    fn get_display_name(&self) -> String {
        match self.name.to_lowercase().as_str() {
            "execute_bash" => "Shell".to_string(),
            "read_file" => "Read".to_string(),
            "write_file" => "Write".to_string(),
            "edit_file" => "Edit".to_string(),
            "list_directory" => "List".to_string(),
            "search_files" => "Search".to_string(),
            "web_search" => "Web".to_string(),
            "mcp_call" => "MCP".to_string(),
            _ => self.name.to_string(),
        }
    }
}

impl Widget for ToolStatusWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let icon = self.get_icon();
        let display_name = self.get_display_name();

        let (icon_style, _text_style) = match self.status {
            ToolStatus::Running => {
                // Pulse effect
                let is_bright = (self.frame / 2) % 2 == 0;
                let color = if is_bright {
                    Color::Cyan
                } else {
                    Color::DarkGray
                };
                (Style::default().fg(color), Style::default().fg(color))
            }
            ToolStatus::Success => (
                Style::default().fg(Color::Green),
                Style::default().fg(Color::DarkGray), // Past tense/done is dim
            ),
            ToolStatus::Error => (
                Style::default().fg(Color::Red),
                Style::default().fg(Color::Red),
            ),
        };

        let mut spans = vec![
            Span::styled(format!("{} {}", icon, display_name), icon_style.bold()),
            Span::raw(" "),
        ];

        match self.status {
            ToolStatus::Running => {
                spans.push(Span::styled(self.args, Style::default().fg(Color::Yellow)));
            }
            ToolStatus::Success => {
                if let Some(summary) = self.result_summary {
                    spans.push(Span::styled(summary, Style::default().fg(Color::DarkGray)));
                } else {
                    spans.push(Span::styled(
                        "Completed",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            ToolStatus::Error => {
                if let Some(error) = self.result_summary {
                    spans.push(Span::styled(error, Style::default().fg(Color::Red)));
                } else {
                    spans.push(Span::styled("Failed", Style::default().fg(Color::Red)));
                }
            }
        }

        Line::from(spans).render(area, buf);
    }
}
