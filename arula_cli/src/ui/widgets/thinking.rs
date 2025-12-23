use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Widget, Wrap},
};

/// Animation state for fade in/out effects
#[derive(Clone, Copy, Debug)]
pub enum AnimationState {
    Idle,
    FadingIn { progress: u8 },
    FadingOut { progress: u8 },
    FullyVisible,
}

impl AnimationState {
    pub fn opacity(&self) -> u8 {
        match self {
            AnimationState::Idle => 255,
            AnimationState::FadingIn { progress } => *progress,
            AnimationState::FadingOut { progress } => 255 - progress,
            AnimationState::FullyVisible => 255,
        }
    }
}

/// A Ratatui widget for displaying AI thinking content with animation
pub struct ThinkingWidget<'a> {
    content: &'a str,
    frame: usize,
    is_active: bool,
    expanded: bool,
    animation_state: AnimationState,
}

impl<'a> ThinkingWidget<'a> {
    pub fn new(content: &'a str, frame: usize, is_active: bool) -> Self {
        Self {
            content,
            frame,
            is_active,
            expanded: false,
            animation_state: AnimationState::Idle,
        }
    }

    pub fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn with_animation(mut self, state: AnimationState) -> Self {
        self.animation_state = state;
        self
    }

    fn truncate_with_dots(text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            return text.to_string();
        }
        let mut truncated = text.chars().take(max_len.saturating_sub(3)).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

impl Widget for ThinkingWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Animation frames for the spinner
        let frames = ["◐", "◓", "◑", "◒"];
        let frame_char = frames[self.frame % frames.len()];

        // Define styles
        let primary_color = Color::Cyan;
        let border_color = Color::DarkGray;
        let text_color = Color::Gray;

        // Apply fade animation using dim modifier for simplicity
        let base_style = match self.animation_state {
            AnimationState::FadingIn { progress } | AnimationState::FadingOut { progress } => {
                if progress < 128 {
                    Style::default().fg(primary_color).dim()
                } else {
                    Style::default().fg(primary_color)
                }
            }
            _ => Style::default().fg(primary_color),
        };

        let title_style = if self.is_active {
            // Pulse effect
            let is_bright = (self.frame / 2) % 2 == 0;
            if is_bright {
                base_style.bold()
            } else {
                base_style.dim()
            }
        } else {
            base_style
        };

        // Prepare title - show only "Thought" when expanded, or with preview when collapsed
        let title_text = if self.expanded {
            "Thought".to_string()
        } else if !self.content.is_empty() {
            // Show preview with "..." if content doesn't fit
            let max_preview_len = 40;
            let preview_content = self.content.lines().next().unwrap_or(self.content);
            Self::truncate_with_dots(preview_content, max_preview_len)
        } else {
            "Thought".to_string()
        };

        // Render the block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(vec![
                Span::styled(
                    format!("{} ", frame_char),
                    base_style,
                ),
                Span::styled(title_text, title_style),
            ]))
            .padding(Padding::new(1, 1, 0, 0));

        // Render content inside (only when expanded)
        let inner_area = block.inner(area);
        block.render(area, buf);

        if self.expanded && !self.content.is_empty() {
            let paragraph = ratatui::widgets::Paragraph::new(self.content)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(text_color));

            paragraph.render(inner_area, buf);
        }
    }
}
