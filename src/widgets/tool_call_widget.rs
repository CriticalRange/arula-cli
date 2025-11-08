use ratatui::{
    buffer::Buffer,
    layout::{Rect, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Padding},
};
use crate::tool_call::{ToolCall, ToolCallResult};

/// A beautiful widget to display tool calls with syntax highlighting
pub struct ToolCallWidget<'a> {
    tool_call: &'a ToolCall,
    result: Option<&'a ToolCallResult>,
}

impl<'a> ToolCallWidget<'a> {
    pub fn new(tool_call: &'a ToolCall) -> Self {
        Self {
            tool_call,
            result: None,
        }
    }

    pub fn with_result(mut self, result: &'a ToolCallResult) -> Self {
        self.result = Some(result);
        self
    }

    /// Render the tool call as formatted lines
    fn render_tool_call(&self) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // Tool name header with icon
        let tool_icon = match self.tool_call.tool.as_str() {
            "bash" | "execute" => "⚡",
            "read_file" => "📄",
            "write_file" => "✏️ ",
            "list_dir" => "📁",
            _ => "🔧",
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("{} Tool: ", tool_icon),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &self.tool_call.tool,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]));

        // Arguments section
        if !self.tool_call.arguments.is_null() {
            lines.push(Line::from(""));
            lines.push(Line::from(
                Span::styled(
                    "Arguments:",
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                )
            ));

            // Pretty print JSON arguments
            if let Ok(formatted) = serde_json::to_string_pretty(&self.tool_call.arguments) {
                for arg_line in formatted.lines() {
                    lines.push(Line::from(
                        Span::styled(
                            format!("  {}", arg_line),
                            Style::default().fg(Color::White),
                        )
                    ));
                }
            }
        }

        // Result section if available
        if let Some(result) = self.result {
            lines.push(Line::from(""));

            let (status_icon, status_text, status_color) = if result.success {
                ("✓", "Success", Color::Green)
            } else {
                ("✗", "Failed", Color::Red)
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} Status: ", status_icon),
                    Style::default().fg(status_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    status_text,
                    Style::default().fg(status_color).add_modifier(Modifier::BOLD),
                ),
            ]));

            if !result.output.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(
                    Span::styled(
                        "Output:",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )
                ));

                // Show output lines (limit to prevent overflow)
                for (i, output_line) in result.output.lines().take(20).enumerate() {
                    if i < 19 {
                        lines.push(Line::from(
                            Span::styled(
                                format!("  {}", output_line),
                                Style::default().fg(Color::Gray),
                            )
                        ));
                    } else {
                        lines.push(Line::from(
                            Span::styled(
                                "  ... (output truncated)",
                                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                            )
                        ));
                        break;
                    }
                }
            }
        }

        lines
    }
}

impl<'a> Widget for ToolCallWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a styled block for the tool call
        let border_color = if let Some(result) = self.result {
            if result.success {
                Color::Green
            } else {
                Color::Red
            }
        } else {
            Color::Cyan
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                " Tool Call ",
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            ))
            .padding(Padding::uniform(1));

        let inner = block.inner(area);
        block.render(area, buf);

        // Render the content
        let lines = self.render_tool_call();
        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}
