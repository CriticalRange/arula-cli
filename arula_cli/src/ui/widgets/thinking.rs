use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Widget, Wrap},
};

/// A Ratatui widget for displaying AI thinking content with animation
pub struct ThinkingWidget<'a> {
    content: &'a str,
    frame: usize,
    is_active: bool,
}

impl<'a> ThinkingWidget<'a> {
    pub fn new(content: &'a str, frame: usize, is_active: bool) -> Self {
        Self {
            content,
            frame,
            is_active,
        }
    }
}

impl Widget for ThinkingWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Animation frames for the spinner
        let frames = ["◐", "◓", "◑", "◒"];
        let frame_char = frames[self.frame % frames.len()];
        
        // Define styles
        let primary_color = Color::Cyan; // Adjust to match PRIMARY_ANSI if possible
        let border_color = Color::DarkGray;
        let text_color = Color::Gray;
        
        let title_style = if self.is_active {
            // Pulse effect
            let is_bright = (self.frame / 2) % 2 == 0;
            if is_bright {
                Style::default().fg(primary_color).bold()
            } else {
                Style::default().fg(primary_color).dim()
            }
        } else {
            Style::default().fg(primary_color)
        };

        // Render the block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(vec![
                Span::styled(format!("{} ", frame_char), Style::default().fg(primary_color)),
                Span::styled("Thinking", title_style),
            ]))
            .padding(Padding::new(1, 1, 0, 0));

        // Render content inside
        let inner_area = block.inner(area);
        block.render(area, buf);
        
        if !self.content.is_empty() {
            let paragraph = ratatui::widgets::Paragraph::new(self.content)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(text_color));
            
            paragraph.render(inner_area, buf);
        }
    }
}
