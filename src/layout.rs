use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout as RatatuiLayout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use super::ui_components::{Gauge, Theme};

pub struct Layout {
    pub theme: Theme,
    pub status_gauge: Gauge,
    pub activity_gauge: Gauge,
    pub settings_mode: bool, // Special mode for settings access
}

impl Layout {
    pub fn new(theme: Theme) -> Self {
        let colors = theme.get_colors();

        Self {
            status_gauge: Gauge::new("AI Processing", colors.gradient.clone()),
            activity_gauge: Gauge::new("Network Activity", vec![
                Color::Green,
                Color::Yellow,
                Color::Red,
            ]),
            theme,
            settings_mode: false,
        }
    }

    pub fn render(&mut self, f: &mut Frame, app: &crate::app::App, messages: &[crate::chat::ChatMessage], input: &str, input_mode: bool) {
        // Clear the entire frame with background color
        f.render_widget(
            ratatui::widgets::Clear,
            f.area()
        );

        // Main layout - NO HEADER, NO TABS
        let main_chunks = RatatuiLayout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),     // Content (full screen)
                Constraint::Length(4),  // Enhanced input
                Constraint::Length(2),  // Status
            ])
            .split(f.area());

        // Render content based on mode
        if self.settings_mode {
            self.settings_area(f, main_chunks[0]);
        } else {
            self.chat_area(f, main_chunks[0], messages); // Always show chat
        }

        self.input_area(f, main_chunks[1], input, input_mode, app.cursor_position);
        self.status_bar(f, main_chunks[2]);

        // Update animations
        self.update();
    }

    fn header(&self, f: &mut Frame, area: Rect) {
        let colors = self.theme.get_colors();
        let timestamp = chrono::Local::now().format("%H:%M:%S");

        let header_text = Line::from(vec![
            Span::styled("🚀 ARULA", Style::default().fg(colors.primary).add_modifier(Modifier::BOLD)),
            Span::styled(" • ", Style::default().fg(colors.secondary)),
            Span::styled(timestamp.to_string(), Style::default().fg(colors.info)),
            Span::styled(" • ", Style::default().fg(colors.secondary)),
            Span::styled(
                self.theme.to_string(),
                Style::default().fg(colors.accent).add_modifier(Modifier::BOLD),
            ),
        ]);

        let header = Paragraph::new(header_text)
            .style(Style::default().fg(colors.text))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors.primary))
                    .padding(Padding::horizontal(1)),
            )
            .alignment(Alignment::Center);

        f.render_widget(header, area);
    }

    fn chat_area(&self, f: &mut Frame, area: Rect, messages: &[crate::chat::ChatMessage]) {
        let colors = self.theme.get_colors();

        // Messages area with proper alignment
        let message_items: Vec<ListItem> = messages
            .iter()
            .rev()
            .take(area.height as usize - 1)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|msg| {
                let timestamp = msg.timestamp.format("%H:%M:%S").to_string();
                let (icon, color) = match msg.message_type {
                    crate::chat::MessageType::User => ("👤", colors.success),
                    crate::chat::MessageType::Arula => ("🤖", colors.primary),
                    crate::chat::MessageType::System => ("🔧", colors.text),
                    crate::chat::MessageType::Success => ("✅", colors.success),
                    crate::chat::MessageType::Error => ("❌", colors.error),
                    crate::chat::MessageType::Info => ("ℹ️", colors.info),
                };

                // Better alignment with proper spacing
                let content = Line::from(vec![
                    Span::styled(
                        format!("[{}] {} ", timestamp, icon),
                        Style::default()
                            .fg(color)
                            .add_modifier(Modifier::BOLD)
                            .bg(colors.background),
                    ),
                    Span::styled(
                        &msg.content,
                        Style::default()
                            .fg(colors.text)
                            .bg(colors.background)
                    ),
                ]);

                ListItem::new(content)
            })
            .collect();

        let messages_list = List::new(message_items)
            .style(Style::default()
                .fg(colors.text)
                .bg(colors.background));

        f.render_widget(messages_list, area);
    }

    
    fn settings_area(&self, f: &mut Frame, area: Rect) {
        let colors = self.theme.get_colors();

        let settings_text = vec![
            Line::from(vec![
                Span::styled("⚙️ ", Style::default().fg(colors.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Settings", Style::default().fg(colors.primary).add_modifier(Modifier::BOLD).add_modifier(Modifier::REVERSED)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("🎨 Theme: ", Style::default().fg(colors.text).add_modifier(Modifier::BOLD)),
                Span::styled(
                    self.theme.to_string(),
                    Style::default()
                        .fg(colors.accent)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::REVERSED),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Keyboard shortcuts:", Style::default().fg(colors.primary).add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("• ", Style::default().fg(colors.secondary)),
                Span::styled("Tab", Style::default().fg(colors.info).add_modifier(Modifier::BOLD)),
                Span::styled(": Switch tabs", Style::default().fg(colors.text)),
            ]),
            Line::from(vec![
                Span::styled("• ", Style::default().fg(colors.secondary)),
                Span::styled("t", Style::default().fg(colors.info).add_modifier(Modifier::BOLD)),
                Span::styled(": Change theme", Style::default().fg(colors.text)),
            ]),
            Line::from(vec![
                Span::styled("• ", Style::default().fg(colors.secondary)),
                Span::styled("i", Style::default().fg(colors.info).add_modifier(Modifier::BOLD)),
                Span::styled(": Start typing", Style::default().fg(colors.text)),
            ]),
            Line::from(vec![
                Span::styled("• ", Style::default().fg(colors.secondary)),
                Span::styled("q", Style::default().fg(colors.info).add_modifier(Modifier::BOLD)),
                Span::styled(": Quit", Style::default().fg(colors.text)),
            ]),
            Line::from(vec![
                Span::styled("• ", Style::default().fg(colors.secondary)),
                Span::styled("Ctrl+L", Style::default().fg(colors.info).add_modifier(Modifier::BOLD)),
                Span::styled(": Clear chat", Style::default().fg(colors.text)),
            ]),
        ];

        let settings = Paragraph::new(settings_text)
            .style(Style::default().fg(colors.text).bg(colors.background))
            .block(
                Block::default()
                    .title("Configuration")
                    .title_style(Style::default().fg(colors.warning).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors.warning).bg(colors.background))
                    .padding(Padding::uniform(1)),
            )
            .wrap(Wrap { trim: true });

        f.render_widget(settings, area);
    }

    fn input_area(&self, f: &mut Frame, area: Rect, input: &str, input_mode: bool, cursor_position: usize) {
        let colors = self.theme.get_colors();

        // Split input area into prompt and input box
        let input_chunks = RatatuiLayout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // Prompt indicator with better contrast
        let prompt = Paragraph::new("❯")
            .style(
                Style::default()
                    .fg(if input_mode { colors.accent } else { colors.primary })
                    .add_modifier(Modifier::BOLD)
                    .bg(colors.background),
            )
            .alignment(Alignment::Right);

        f.render_widget(prompt, input_chunks[0]);

        // Input box with cursor display
        let input_text = if input_mode {
            // Show input with visual cursor
            let before_cursor = &input[..cursor_position];
            let after_cursor = &input[cursor_position..];
            format!("{}█{}", before_cursor, after_cursor)
        } else {
            "Press any key or click to start typing...".to_string()
        };

        let input_style = if input_mode {
            Style::default()
                .fg(colors.text)
                .bg(colors.background)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(colors.secondary)
                .bg(colors.background)
                .add_modifier(Modifier::DIM)
        };

        let input_box = Paragraph::new(input_text)
            .style(input_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if input_mode {
                        Style::default()
                            .fg(colors.accent)
                            .bg(colors.background)
                    } else {
                        Style::default()
                            .fg(colors.border)
                            .bg(colors.background)
                    })
                    .title("Input")
                    .title_style(
                        Style::default()
                            .fg(colors.primary)
                            .add_modifier(Modifier::BOLD)
                    )
                    .padding(Padding::horizontal(1)),
            );

        f.render_widget(input_box, input_chunks[1]);

        // Set terminal cursor position to match our visual cursor
        if input_mode {
            f.set_cursor_position((
                input_chunks[1].x + 2 + cursor_position as u16, // +2 for padding
                input_chunks[1].y + 1,
            ));
        }
    }

    fn status_bar(&self, f: &mut Frame, area: Rect) {
        let colors = self.theme.get_colors();

        let current_section = if self.settings_mode {
            "Settings"
        } else {
            "Chat"
        };

        let status_text = vec![
            Span::styled("● ", Style::default().fg(colors.success).add_modifier(Modifier::BOLD)),
            Span::styled("Connected", Style::default().fg(colors.text).add_modifier(Modifier::BOLD)),
            Span::styled(" • ", Style::default().fg(colors.secondary)),
            Span::styled(
                current_section,
                Style::default()
                    .fg(colors.primary)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED),
            ),
            Span::styled(" • ", Style::default().fg(colors.secondary)),
            if self.settings_mode {
                Span::styled("Esc: exit settings", Style::default().fg(colors.warning).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("Esc: settings", Style::default().fg(colors.info).add_modifier(Modifier::BOLD))
            },
        ];

        let status = Paragraph::new(Line::from(status_text))
            .style(Style::default().fg(colors.text).bg(colors.background))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors.border).bg(colors.background)),
            );

        f.render_widget(status, area);
    }

    fn update(&mut self) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Update gauges with smooth animation
        let phase = (secs % 10) as f32 / 10.0;
        self.status_gauge.update(phase * 2.0);
        self.activity_gauge.update((phase * 3.0).sin().abs() * 50.0 + 25.0);
    }

    
    pub fn toggle_settings_mode(&mut self) {
        self.settings_mode = !self.settings_mode;
    }

    #[allow(dead_code)]
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        // Reinitialize components with new theme
        let colors = self.theme.get_colors();
        self.status_gauge.colors = colors.gradient.clone();
    }
}