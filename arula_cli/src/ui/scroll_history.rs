//! Insert history lines into native terminal scrollback
//! Uses ANSI scroll regions to push text ABOVE the TUI viewport.


use crossterm::{
    style::Color,
};




/// A styled line segment
pub struct HistorySpan {
    pub text: String,
    pub fg: Option<Color>,
    pub bold: bool,
    pub dim: bool,
}

impl HistorySpan {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fg: None,
            bold: false,
            dim: false,
        }
    }
    
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }
    
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }
}

/// A line to insert into history
pub struct HistoryLine {
    pub spans: Vec<HistorySpan>,
}

impl HistoryLine {
    pub fn new(spans: Vec<HistorySpan>) -> Self {
        Self { spans }
    }
    
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            spans: vec![HistorySpan::new(text)],
        }
    }
}

/// Insert lines into terminal scrollback ABOVE the viewport.
/// 
/// This uses ANSI scroll regions to achieve the effect:
/// 1. Set scroll region from row 1 to viewport_top
/// 2. Move cursor to bottom of that region
/// 3. Print lines with newlines (they scroll up into history)
use ratatui::{
    text::{Line, Span as RSpan},
    style::{Style, Color as RColor, Modifier},
};

fn to_ratatui_color(c: crossterm::style::Color) -> RColor {
    use crossterm::style::Color as C;
    match c {
        C::Reset => RColor::Reset,
        C::Black => RColor::Black,
        C::DarkGrey => RColor::DarkGray,
        C::Red => RColor::Red,
        C::DarkRed => RColor::Red,
        C::Green => RColor::Green,
        C::DarkGreen => RColor::Green,
        C::Yellow => RColor::Yellow,
        C::DarkYellow => RColor::Yellow,
        C::Blue => RColor::Blue,
        C::DarkBlue => RColor::Blue,
        C::Magenta => RColor::Magenta,
        C::DarkMagenta => RColor::Magenta,
        C::Cyan => RColor::Cyan,
        C::DarkCyan => RColor::Cyan,
        C::White => RColor::White,
        C::Grey => RColor::Gray,
        C::AnsiValue(v) => RColor::Indexed(v),
        C::Rgb { r, g, b } => RColor::Rgb(r, g, b),
    }
}

impl HistoryLine {
    pub fn to_line(&self) -> Line<'_> {
        let spans: Vec<RSpan> = self.spans.iter().map(|span| {
            let mut style = Style::default();
            if let Some(fg) = span.fg {
                style = style.fg(to_ratatui_color(fg));
            }
            if span.bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if span.dim {
                style = style.add_modifier(Modifier::DIM);
            }
            RSpan::styled(span.text.clone(), style)
        }).collect();
        Line::from(spans)
    }
}
