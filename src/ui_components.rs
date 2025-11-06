use ratatui::{
    layout::{Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Padding, Paragraph, Sparkline, Wrap,
    },
    Frame,
};
use std::time::{SystemTime, UNIX_EPOCH};
use strum::Display;

#[derive(Debug, Clone, Copy, Display)]
pub enum Theme {
    #[strum(to_string = "Cyberpunk")]
    Cyberpunk,
    #[allow(dead_code)]
    #[strum(to_string = "Matrix")]
    Matrix,
    #[allow(dead_code)]
    #[strum(to_string = "Ocean")]
    Ocean,
    #[allow(dead_code)]
    #[strum(to_string = "Sunset")]
    Sunset,
    #[allow(dead_code)]
    #[strum(to_string = "Monochrome")]
    Monochrome,
}

impl Theme {
    pub fn get_colors(&self) -> ThemeColors {
        match self {
            Theme::Cyberpunk => ThemeColors {
                primary: Color::Magenta,
                secondary: Color::Cyan,
                success: Color::Green,
                error: Color::Red,
                info: Color::LightCyan,
                background: Color::Black,
                text: Color::White,
                border: Color::Magenta,
                gradient: vec![Color::Magenta, Color::Blue, Color::Cyan],
            },
            Theme::Matrix => ThemeColors {
                primary: Color::Green,
                secondary: Color::LightGreen,
                success: Color::LightGreen,
                error: Color::LightRed,
                info: Color::LightCyan,
                background: Color::Black,
                text: Color::LightGreen,
                border: Color::Green,
                gradient: vec![Color::Green, Color::LightGreen, Color::White],
            },
            Theme::Ocean => ThemeColors {
                primary: Color::Blue,
                secondary: Color::Cyan,
                success: Color::LightGreen,
                error: Color::Red,
                info: Color::LightCyan,
                background: Color::Rgb(10, 20, 30),
                text: Color::White,
                border: Color::Blue,
                gradient: vec![Color::DarkGray, Color::Blue, Color::Cyan, Color::LightBlue],
            },
            Theme::Sunset => ThemeColors {
                primary: Color::Rgb(255, 94, 77),
                secondary: Color::Rgb(255, 206, 84),
                success: Color::LightGreen,
                error: Color::LightRed,
                info: Color::LightCyan,
                background: Color::Rgb(25, 25, 35),
                text: Color::White,
                border: Color::Rgb(255, 94, 77),
                gradient: vec![
                    Color::Rgb(255, 94, 77),
                    Color::Rgb(255, 157, 77),
                    Color::Rgb(255, 206, 84),
                ],
            },
            Theme::Monochrome => ThemeColors {
                primary: Color::Gray,
                secondary: Color::Rgb(200, 200, 200),
                success: Color::LightGreen,
                error: Color::LightRed,
                info: Color::LightBlue,
                background: Color::Black,
                text: Color::White,
                border: Color::Gray,
                gradient: vec![Color::Black, Color::Gray, Color::White],
            },
        }
    }
}

pub struct ThemeColors {
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub error: Color,
    pub info: Color,
    #[allow(dead_code)]
    pub background: Color,
    pub text: Color,
    pub border: Color,
    pub gradient: Vec<Color>,
}

#[allow(dead_code)]
pub struct Button {
    pub label: String,
    pub style: Style,
    pub hover_style: Style,
    pub is_hovered: bool,
}

#[allow(dead_code)]
impl Button {
    pub fn new(label: &str) -> Self {
        let base_style = Style::default()
            .fg(Color::Cyan)
            .bg(Color::Black)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED);

        Self {
            label: label.to_string(),
            style: base_style,
            hover_style: base_style.fg(Color::Yellow).bg(Color::DarkGray),
            is_hovered: false,
        }
    }

    pub fn render(&self, area: Rect, f: &mut Frame) {
        let style = if self.is_hovered {
            self.hover_style
        } else {
            self.style
        };

        let button = Paragraph::new(self.label.as_str())
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(style)
                    .padding(Padding::horizontal(1)),
            )
            .alignment(ratatui::layout::Alignment::Center);

        f.render_widget(button, area);
    }
}

pub struct Gauge {
    pub label: String,
    #[allow(dead_code)]
    pub progress: f32,
    #[allow(dead_code)]
    pub style: Style,
    #[allow(dead_code)]
    pub colors: Vec<Color>,
}

impl Gauge {
    pub fn new(label: &str, colors: Vec<Color>) -> Self {
        Self {
            label: label.to_string(),
            progress: 0.0,
            style: Style::default().fg(Color::Cyan),
            colors,
        }
    }

    pub fn update(&mut self, delta: f32) {
        self.progress = (self.progress + delta).clamp(0.0, 100.0);
    }

    #[allow(dead_code)]
    pub fn render(&self, area: Rect, f: &mut Frame) {
        let mut gauge_colors = Vec::new();
        for color in &self.colors {
            gauge_colors.push(*color);
        }

        let gauge = ratatui::widgets::Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.label.as_str())
                    .border_style(Style::default().fg(gauge_colors[0])),
            )
            .gauge_style(
                Style::default()
                    .fg(gauge_colors[self.progress as usize % gauge_colors.len()])
                    .add_modifier(Modifier::BOLD),
            )
            .label(format!("{:.1}%", self.progress))
            .ratio((self.progress / 100.0) as f64);

        f.render_widget(gauge, area);
    }
}

#[allow(dead_code)]
pub struct Tabs {
    pub titles: Vec<String>,
    pub selected_index: usize,
    pub theme: Theme,
}

#[allow(dead_code)]
impl Tabs {
    pub fn new(titles: Vec<&str>, theme: Theme) -> Self {
        Self {
            titles: titles.into_iter().map(|s| s.to_string()).collect(),
            selected_index: 0,
            theme,
        }
    }

    pub fn next(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.titles.len();
    }

    pub fn previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.titles.len() - 1;
        }
    }

    pub fn render(&self, area: Rect, f: &mut Frame) {
        let colors = self.theme.get_colors();
        let titles: Vec<&str> = self.titles.iter().map(|s| s.as_str()).collect();

        let tabs = ratatui::widgets::Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(colors.border)),
            )
            .style(Style::default().fg(colors.text))
            .highlight_style(
                Style::default()
                    .fg(colors.background)
                    .bg(colors.primary)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(" │ ")
            .select(self.selected_index);

        f.render_widget(tabs, area);
    }
}

#[allow(dead_code)]
pub fn gradient_box(area: Rect, f: &mut Frame, title: &str, colors: &[Color]) {
    let chunk_width = area.width / colors.len() as u16;

    for (i, color) in colors.iter().enumerate() {
        let chunk = Rect {
            x: area.x + (i as u16 * chunk_width),
            y: area.y,
            width: chunk_width,
            height: area.height,
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(*color))
            .title(if i == 0 { title } else { "" })
            .title_style(Style::default().fg(*color).add_modifier(Modifier::BOLD));

        f.render_widget(block, chunk);
    }
}

#[allow(dead_code)]
pub fn sparkline<'a>(data: &'a [u64], colors: &ThemeColors) -> Sparkline<'a> {
    Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Activity")
                .border_style(Style::default().fg(colors.border)),
        )
        .data(data)
        .style(Style::default().fg(colors.primary))
        .max(data.iter().cloned().max().unwrap_or(100))
}

#[allow(dead_code)]
pub fn paragraph(content: &str, style: Style) -> Paragraph<'_> {
    Paragraph::new(content)
        .style(style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .padding(Padding::uniform(1)),
        )
        .wrap(Wrap { trim: true })
        .alignment(ratatui::layout::Alignment::Left)
}

#[allow(dead_code)]
pub fn time_color() -> Color {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    match secs % 6 {
        0 => Color::Red,
        1 => Color::Yellow,
        2 => Color::Green,
        3 => Color::Cyan,
        4 => Color::Blue,
        _ => Color::Magenta,
    }
}