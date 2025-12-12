//! Insert history lines into native terminal scrollback
//! Uses ANSI scroll regions to push text ABOVE the TUI viewport.

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Color, Colors, Print, SetBackgroundColor, SetColors, SetForegroundColor},
    terminal::{Clear, ClearType},
    Command,
};
use ratatui::{
    style::{Color as RColor, Modifier, Style},
    text::{Line, Span as RSpan},
    Terminal,
};
use std::io::Write;
use unicode_width::UnicodeWidthChar;

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

    pub fn to_line(&self) -> Line<'_> {
        let spans: Vec<RSpan> = self
            .spans
            .iter()
            .map(|span| {
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
            })
            .collect();
        Line::from(spans)
    }
}

/// Insert history lines above the inline viewport using scroll regions.
///
/// Inspired by Codex' insert_history: wraps lines to viewport width, limits scroll region above
/// the viewport, and restores the cursor afterward to avoid desync.
pub fn insert_history_lines(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    screen_width: u16,
    screen_height: u16,
    viewport_height: u16,
    lines: Vec<Line<'_>>,
) -> std::io::Result<()> {
    let viewport_top = screen_height.saturating_sub(viewport_height);
    let writer = terminal.backend_mut();

    // Cache cursor position to restore later.
    let cursor_pos = crossterm::cursor::position().unwrap_or((0, viewport_top));

    // Limit scroll region to everything above the inline viewport (1-based coordinates).
    if viewport_top > 0 {
        queue!(writer, SetScrollRegion(1..viewport_top))?;
    }
    // Place cursor at the line above the viewport to start inserting.
    queue!(writer, MoveTo(0, viewport_top.saturating_sub(1)))?;

        let width = screen_width.max(1) as usize;
        for line in lines.into_iter() {
            let owned = line_to_static(&line);
            let wrapped = wrap_line(&owned, width);
            for wrapped_line in wrapped {
                queue!(writer, Print("\r\n"))?;
                queue!(
                    writer,
                    Clear(ClearType::UntilNewLine),
                    SetColors(Colors::new(
                        wrapped_line
                            .style
                            .fg
                            .map(to_crossterm_color)
                            .unwrap_or(crossterm::style::Color::Reset),
                        wrapped_line
                            .style
                            .bg
                            .map(to_crossterm_color)
                            .unwrap_or(crossterm::style::Color::Reset)
                    ))
                )?;

                let merged_spans: Vec<RSpan> = wrapped_line
                    .spans
                    .iter()
                    .map(|s| RSpan {
                        content: s.content.clone(),
                        style: s.style.patch(wrapped_line.style),
                    })
                    .collect();

                for span in merged_spans {
                    queue!(
                        writer,
                        SetForegroundColor(
                            span.style
                                .fg
                                .map(to_crossterm_color)
                                .unwrap_or(crossterm::style::Color::Reset)
                        ),
                        SetBackgroundColor(
                            span.style
                                .bg
                                .map(to_crossterm_color)
                                .unwrap_or(crossterm::style::Color::Reset)
                        ),
                        Print(span.content)
                    )?;
                }
            }
        }

    // Reset region/cursor.
    queue!(writer, ResetScrollRegion)?;
    queue!(writer, MoveTo(cursor_pos.0, cursor_pos.1))?;
    writer.flush()?;
    Ok(())
}

/// Wrap a line to the given width using word boundaries where possible.
fn wrap_line(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![line.clone()];
    }

    let mut out = Vec::new();
    let mut current = Line::default();
    let mut current_width = 0;

    for span in line.spans.iter() {
        let content = span.content.clone();
        // Split span into whitespace and non-whitespace tokens to prefer word boundaries.
        let mut tokens: Vec<(String, bool)> = Vec::new();
        let mut buf = String::new();
        let mut buf_is_space: Option<bool> = None;
        for ch in content.chars() {
            if ch == '\n' {
                if let Some(is_space) = buf_is_space.take() {
                    tokens.push((buf.clone(), is_space));
                }
                buf.clear();
                tokens.push((String::new(), false)); // newline marker
                continue;
            }
            let is_space = ch.is_whitespace();
            if let Some(prev) = buf_is_space {
                if prev == is_space {
                    buf.push(ch);
                } else {
                    tokens.push((buf.clone(), prev));
                    buf.clear();
                    buf.push(ch);
                    buf_is_space = Some(is_space);
                }
            } else {
                buf.push(ch);
                buf_is_space = Some(is_space);
            }
        }
        if let Some(is_space) = buf_is_space {
            tokens.push((buf, is_space));
        }

        for (token, is_space) in tokens {
            if token.is_empty() {
                // newline marker
                out.push(current.to_owned());
                current = Line::default();
                current_width = 0;
                continue;
            }

            let token_width: usize = token.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum();

            // Long word that exceeds width on its own: fall back to character wrapping.
            if token_width > width && !is_space {
                for ch in token.chars() {
                    let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
                    if current_width + ch_width > width && current_width > 0 {
                        out.push(current.to_owned());
                        current = Line::default();
                        current_width = 0;
                    }
                    let mut s = span.clone();
                    s.content = ch.to_string().into();
                    current.spans.push(s);
                    current_width += ch_width;
                }
                continue;
            }

            if current_width + token_width > width && current_width > 0 && !is_space {
                out.push(current.to_owned());
                current = Line::default();
                current_width = 0;
            }

            let mut s = span.clone();
            s.content = token.into();
            current_width += token_width;
            current.spans.push(s);
        }
    }

    out.push(current);
    out
}

fn line_to_static(line: &Line<'_>) -> Line<'static> {
    let spans: Vec<_> = line
        .spans
        .iter()
        .map(|s| RSpan::styled(s.content.to_string(), s.style))
        .collect();
    let mut out = Line::from(spans);
    out.style = line.style;
    out
}

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

fn to_crossterm_color(c: RColor) -> crossterm::style::Color {
    use crossterm::style::Color as C;
    match c {
        RColor::Reset => C::Reset,
        RColor::Black => C::Black,
        RColor::Red => C::Red,
        RColor::Green => C::Green,
        RColor::Yellow => C::Yellow,
        RColor::Blue => C::Blue,
        RColor::Magenta => C::Magenta,
        RColor::Cyan => C::Cyan,
        RColor::Gray => C::Grey,
        RColor::DarkGray => C::DarkGrey,
        RColor::LightRed => C::Red,
        RColor::LightGreen => C::Green,
        RColor::LightYellow => C::Yellow,
        RColor::LightBlue => C::Blue,
        RColor::LightMagenta => C::Magenta,
        RColor::LightCyan => C::Cyan,
        RColor::White => C::White,
        RColor::Indexed(v) => C::AnsiValue(v),
        RColor::Rgb(r, g, b) => C::Rgb { r, g, b },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetScrollRegion(pub std::ops::Range<u16>);

impl Command for SetScrollRegion {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b[{};{}r", self.0.start, self.0.end)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("SetScrollRegion not supported via WinAPI; use ANSI");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetScrollRegion;

impl Command for ResetScrollRegion {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b[r")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("ResetScrollRegion not supported via WinAPI; use ANSI");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}
