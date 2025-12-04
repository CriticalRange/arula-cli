//! Markdown streaming renderer for terminal output
//!
//! Provides real-time markdown rendering as AI responses stream in,
//! using termimad for proper terminal formatting with code block
//! detection and syntax highlighting.
//!
//! # Features
//!
//! - Real-time streaming with proper buffering
//! - Code block detection with syntax highlighting  
//! - Terminal-width aware text wrapping
//! - Customizable color themes
//!
//! # Example
//!
//! ```rust,ignore
//! let mut streamer = MarkdownStreamer::new();
//! streamer.process_chunk("# Hello\n")?;
//! streamer.process_chunk("This is **bold** text\n")?;
//! streamer.finalize()?;
//! ```

use super::code_blocks::CodeHighlighter;
use console::style;
use crossterm::terminal;
use std::io::{self, Write};
use std::sync::OnceLock;
use termimad::crossterm::style::Color as TMColor;
use termimad::MadSkin;

/// Global skin instance for consistent styling across renders
static MARKDOWN_SKIN: OnceLock<MadSkin> = OnceLock::new();

/// Initialize or get the shared markdown skin
fn get_skin() -> &'static MadSkin {
    MARKDOWN_SKIN.get_or_init(|| {
        let mut skin = MadSkin::default();

        // Configure colors for a pleasant terminal experience
        // Bold - Yellow for emphasis
        skin.bold.set_fg(TMColor::Yellow);
        skin.bold.add_attr(termimad::crossterm::style::Attribute::Bold);

        // Italic - Cyan for subtle emphasis  
        skin.italic.set_fg(TMColor::Cyan);
        skin.italic.add_attr(termimad::crossterm::style::Attribute::Italic);

        // Inline code - Cyan background for visibility
        skin.inline_code.set_fg(TMColor::Rgb { r: 230, g: 230, b: 230 });
        skin.inline_code.set_bg(TMColor::Rgb { r: 45, g: 45, b: 45 });

        // Code blocks - Green on dark background
        skin.code_block.set_fg(TMColor::Green);
        skin.code_block.set_bg(TMColor::Rgb { r: 30, g: 30, b: 30 });

        // Headers - Different colors for hierarchy
        skin.headers[0].set_fg(TMColor::Magenta);
        skin.headers[0].add_attr(termimad::crossterm::style::Attribute::Bold);
        skin.headers[1].set_fg(TMColor::Blue);
        skin.headers[1].add_attr(termimad::crossterm::style::Attribute::Bold);
        skin.headers[2].set_fg(TMColor::Cyan);

        // Strikeout
        skin.strikeout.set_fg(TMColor::DarkGrey);
        skin.strikeout.add_attr(termimad::crossterm::style::Attribute::CrossedOut);

        // Bullet points and quotes
        skin.bullet = termimad::StyledChar::from_fg_char(TMColor::Cyan, '•');
        skin.quote_mark = termimad::StyledChar::new(
            termimad::CompoundStyle::with_fg(TMColor::DarkGrey),
            '│',
        );

        skin
    })
}

/// Get terminal width with a fallback
fn terminal_width() -> usize {
    terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
}

/// State machine for tracking code block parsing
#[derive(Debug, Clone, PartialEq)]
enum ParseState {
    /// Normal text (outside code blocks)
    Normal,
    /// Inside a fenced code block
    InCodeBlock {
        language: String,
        content: String,
        fence: String,
    },
}

/// Streaming markdown renderer
///
/// Processes markdown text chunk-by-chunk, rendering inline
/// formatting and collecting code blocks for syntax highlighting.
pub struct MarkdownStreamer {
    /// Current parsing state
    state: ParseState,
    /// Buffer for incomplete lines
    line_buffer: String,
    /// Code highlighter instance
    highlighter: CodeHighlighter,
    /// Whether we've started output (for spacing)
    has_output: bool,
    /// Accumulated text for batch rendering
    text_buffer: String,
}

impl MarkdownStreamer {
    /// Create a new markdown streamer
    pub fn new() -> Self {
        Self {
            state: ParseState::Normal,
            line_buffer: String::new(),
            highlighter: CodeHighlighter::default_theme(),
            has_output: false,
            text_buffer: String::new(),
        }
    }

    /// Process a chunk of markdown text
    ///
    /// Returns the number of bytes processed.
    pub fn process_chunk(&mut self, chunk: &str) -> io::Result<usize> {
        self.line_buffer.push_str(chunk);

        // Process complete lines
        while let Some(newline_pos) = self.line_buffer.find('\n') {
            let line = self.line_buffer[..=newline_pos].to_string();
            self.line_buffer = self.line_buffer[newline_pos + 1..].to_string();
            self.process_line(&line)?;
        }

        Ok(chunk.len())
    }

    /// Process a complete line
    fn process_line(&mut self, line: &str) -> io::Result<()> {
        let trimmed = line.trim_end();

        // Handle based on current state
        match &self.state {
            ParseState::Normal => {
                // Check for code fence start
                if let Some(fence_info) = Self::parse_code_fence_start(trimmed) {
                    // Flush any accumulated text first
                    self.flush_text_buffer()?;
                    
                    self.state = ParseState::InCodeBlock {
                        language: fence_info.0,
                        content: String::new(),
                        fence: fence_info.1,
                    };
                } else {
                    // Accumulate text for batch rendering
                    self.text_buffer.push_str(line);
                    
                    // If buffer is getting large or we have complete paragraphs, flush
                    if self.text_buffer.len() > 500 || self.text_buffer.ends_with("\n\n") {
                        self.flush_text_buffer()?;
                    }
                }
            }
            ParseState::InCodeBlock { fence, language, content } => {
                // Check for matching closing fence
                let fence_len = fence.len();
                let is_closing = trimmed.len() >= fence_len 
                    && trimmed[..fence_len] == *fence 
                    && trimmed.chars().all(|c| c == '`' || c == '~' || c.is_whitespace());

                if is_closing {
                    let lang = language.clone();
                    let code = content.clone();
                    self.render_code_block(&lang, &code)?;
                    self.state = ParseState::Normal;
                } else {
                    // Clone state to avoid borrow issues
                    let mut new_content = String::new();
                    if let ParseState::InCodeBlock { content: existing, .. } = &self.state {
                        new_content = existing.clone();
                    }
                    new_content.push_str(line);
                    
                    if let ParseState::InCodeBlock { content, .. } = &mut self.state {
                        *content = new_content;
                    }
                }
            }
        }

        Ok(())
    }

    /// Flush accumulated text buffer to output
    fn flush_text_buffer(&mut self) -> io::Result<()> {
        if self.text_buffer.is_empty() {
            return Ok(());
        }

        let stdout = io::stdout();
        let mut handle = stdout.lock();

        // Use termimad for proper rendering with terminal width
        let skin = get_skin();
        let width = terminal_width();
        
        // For streaming, use term_text which handles wrapping
        let formatted = skin.text(&self.text_buffer, Some(width.saturating_sub(4)));
        write!(handle, "{}", formatted)?;
        handle.flush()?;

        self.text_buffer.clear();
        self.has_output = true;

        Ok(())
    }

    /// Parse code fence start (e.g., "```rust" or "~~~python")
    fn parse_code_fence_start(line: &str) -> Option<(String, String)> {
        let trimmed = line.trim();

        // Check for backtick fence (minimum 3)
        if trimmed.starts_with("```") {
            let fence_chars: String = trimmed.chars().take_while(|&c| c == '`').collect();
            let fence = fence_chars.clone();
            let language = trimmed[fence_chars.len()..].trim().to_string();
            return Some((language, fence));
        }

        // Check for tilde fence (minimum 3)
        if trimmed.starts_with("~~~") {
            let fence_chars: String = trimmed.chars().take_while(|&c| c == '~').collect();
            let fence = fence_chars.clone();
            let language = trimmed[fence_chars.len()..].trim().to_string();
            return Some((language, fence));
        }

        None
    }

    /// Render a completed code block with syntax highlighting
    fn render_code_block(&mut self, language: &str, content: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        // Determine effective language
        let lang = if language.is_empty() { "text" } else { language };
        let width = terminal_width();
        let box_width = width.saturating_sub(4).min(78);

        // Create box border
        let header = format!("─ {} ", lang);
        let header_padding = "─".repeat(box_width.saturating_sub(header.len() + 2));

        // Print code block header
        writeln!(
            handle,
            "{}",
            style(format!("┌{}{}┐", header, header_padding)).dim()
        )?;

        // Highlight and print code
        let highlighted = self.highlighter.highlight(content.trim_end(), lang);
        for line in highlighted.lines() {
            // Truncate long lines to fit in box (handle UTF-8 safely)
            let display_line = if line.chars().count() > box_width - 4 {
                // Find safe truncation point at character boundary
                let truncate_at = line
                    .char_indices()
                    .take(box_width - 5)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                format!("{}…", &line[..truncate_at])
            } else {
                line.to_string()
            };
            writeln!(handle, "{} {}", 
                style("│").dim(), 
                display_line,
            )?;
        }

        // Print code block footer
        writeln!(
            handle,
            "{}",
            style(format!("└{}┘", "─".repeat(box_width))).dim()
        )?;

        handle.flush()?;
        self.has_output = true;
        Ok(())
    }

    /// Finalize streaming, rendering any remaining content
    pub fn finalize(&mut self) -> io::Result<()> {
        // Flush any remaining text
        self.flush_text_buffer()?;

        // Process any remaining buffered line content
        if !self.line_buffer.is_empty() {
            let remaining = std::mem::take(&mut self.line_buffer);
            self.text_buffer.push_str(&remaining);
            self.flush_text_buffer()?;
        }

        // Handle unclosed code blocks
        if let ParseState::InCodeBlock { language, content, .. } = &self.state {
            let lang = language.clone();
            let code = content.clone();
            self.render_code_block(&lang, &code)?;
        }
        
        self.state = ParseState::Normal;
        Ok(())
    }

    /// Reset the streamer state for a new message
    pub fn reset(&mut self) {
        self.state = ParseState::Normal;
        self.line_buffer.clear();
        self.text_buffer.clear();
        self.has_output = false;
    }

    /// Check if we're currently inside a code block
    pub fn in_code_block(&self) -> bool {
        matches!(self.state, ParseState::InCodeBlock { .. })
    }

    /// Get the current code block language (if any)
    pub fn current_code_language(&self) -> Option<&str> {
        match &self.state {
            ParseState::InCodeBlock { language, .. } => Some(language),
            _ => None,
        }
    }
}

impl Default for MarkdownStreamer {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a complete markdown string (non-streaming)
///
/// Uses terminal-aware text wrapping for proper display.
pub fn render_markdown(text: &str) -> String {
    let skin = get_skin();
    let width = terminal_width();
    skin.text(text, Some(width.saturating_sub(4))).to_string()
}

/// Render markdown inline (single line, no block formatting)
///
/// Best for short text segments that shouldn't be wrapped.
pub fn render_markdown_inline(text: &str) -> String {
    let skin = get_skin();
    skin.inline(text).to_string()
}

/// Render markdown with explicit width control
pub fn render_markdown_width(text: &str, width: usize) -> String {
    let skin = get_skin();
    skin.text(text, Some(width)).to_string()
}

/// Print markdown directly to stdout (convenience function)
pub fn print_markdown(text: &str) -> io::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let rendered = render_markdown(text);
    write!(handle, "{}", rendered)?;
    handle.flush()
}

/// Print inline markdown directly to stdout
pub fn print_markdown_inline(text: &str) -> io::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let rendered = render_markdown_inline(text);
    write!(handle, "{}", rendered)?;
    handle.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_code_fence_backticks() {
        let result = MarkdownStreamer::parse_code_fence_start("```rust");
        assert_eq!(result, Some(("rust".to_string(), "```".to_string())));
    }

    #[test]
    fn test_parse_code_fence_tildes() {
        let result = MarkdownStreamer::parse_code_fence_start("~~~python");
        assert_eq!(result, Some(("python".to_string(), "~~~".to_string())));
    }

    #[test]
    fn test_parse_code_fence_empty_language() {
        let result = MarkdownStreamer::parse_code_fence_start("```");
        assert_eq!(result, Some(("".to_string(), "```".to_string())));
    }

    #[test]
    fn test_parse_code_fence_long() {
        let result = MarkdownStreamer::parse_code_fence_start("````rust");
        assert_eq!(result, Some(("rust".to_string(), "````".to_string())));
    }

    #[test]
    fn test_parse_not_code_fence() {
        let result = MarkdownStreamer::parse_code_fence_start("regular text");
        assert_eq!(result, None);
    }

    #[test]
    fn test_streamer_code_block_detection() {
        let mut streamer = MarkdownStreamer::new();

        assert!(!streamer.in_code_block());

        // Simulate processing (without actual output)
        streamer.state = ParseState::InCodeBlock {
            language: "rust".to_string(),
            content: String::new(),
            fence: "```".to_string(),
        };

        assert!(streamer.in_code_block());
        assert_eq!(streamer.current_code_language(), Some("rust"));
    }

    #[test]
    fn test_render_markdown_inline() {
        let result = render_markdown_inline("**bold** text");
        // Should contain the text (styling may vary)
        assert!(result.contains("bold"));
        assert!(result.contains("text"));
    }

    #[test]
    fn test_skin_is_initialized_once() {
        let skin1 = get_skin();
        let skin2 = get_skin();
        // Should be the same instance
        assert!(std::ptr::eq(skin1, skin2));
    }
}
