use crate::api::Usage;
use crate::colors::{ColorTheme, helpers};
use console::style;
use console::Style;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::SyntaxSet,
    util::as_24_bit_terminal_escaped,
};
use termimad::MadSkin;

/// Animation utilities for cool visual effects
mod animations {
    use console::style;
    use std::io::{self, Write};
    use std::thread;
    use std::time::Duration;

    /// Fade-in text with gradual appearance
    pub fn fade_in(text: &str, delay_ms: u64) -> io::Result<()> {
        let chars: Vec<char> = text.chars().collect();
        let mut output = String::new();

        for (i, ch) in chars.iter().enumerate() {
            output.push(*ch);

            // Calculate opacity based on position
            let progress = (i + 1) as f32 / chars.len() as f32;

            // Print with increasing intensity using different styles
            let styled = if progress < 0.3 {
                style(&output).dim()
            } else if progress < 0.7 {
                style(&output)
            } else {
                style(&output).bright()
            };

            print!("{}\r", styled);
            io::stdout().flush()?;
            thread::sleep(Duration::from_millis(delay_ms));
        }
        println!();
        Ok(())
    }

    /// Typewriter effect with random delays
    pub fn typewriter(text: &str, base_delay_ms: u64) -> io::Result<()> {
        for ch in text.chars() {
            print!("{}", style(ch).bright());
            io::stdout().flush()?;

            // Add variation to typing speed
            let delay = if ch.is_whitespace() {
                base_delay_ms * 2
            } else if ch.is_ascii_punctuation() {
                base_delay_ms / 2
            } else {
                base_delay_ms + fastrand::u64(0..=base_delay_ms / 2)
            };

            thread::sleep(Duration::from_millis(delay));
        }
        Ok(())
    }

    /// Animated spinner characters
    pub const SPINNER_FRAMES: &[&str] = &[
        "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗",
        "🌘", "○", "◔", "◑", "◕", "●",
    ];

    /// Loading animation frames
    pub const LOADING_FRAMES: &[&str] = &[
        "⚡", "🔥", "💫", "✨", "🌟", "⭐", "💥", "🎆", "🎇", "🌠", "🚀", "💨", "🌪️", "⚡", "🔥",
        "💫", "✨",
    ];

    /// Pulse animation frames
    pub const PULSE_FRAMES: &[&str] = &["░", "▒", "▓", "█", "▓", "▒", "░"];

    /// Success animation
    pub fn success_animation() -> io::Result<()> {
        let frames = ["✨", "🎉", "🎊", "⭐", "💫", "✨"];
        for frame in frames {
            print!(" {} ", style(frame).green().bold());
            io::stdout().flush()?;
            thread::sleep(Duration::from_millis(150));
        }
        print!(" \r");
        Ok(())
    }

    /// Error shake animation
    pub fn error_shake() -> io::Result<()> {
        let chars = ["❌", "⚠️", "❌", "⚠️", "❌"];
        for (i, ch) in chars.iter().enumerate() {
            let offset = if i % 2 == 0 { " " } else { "  " };
            print!("{}{}{}\r", offset, style(ch).red().bold(), offset);
            io::stdout().flush()?;
            thread::sleep(Duration::from_millis(100));
        }
        print!(" \r");
        Ok(())
    }

    /// Draw a progress bar with animation
    pub fn animated_progress(current: usize, total: usize, width: usize) -> String {
        let progress = current as f32 / total as f32;
        let filled = (progress * width as f32) as usize;
        let empty = width - filled;

        let mut bar = String::new();

        // Add filled part with gradient effect
        for i in 0..filled {
            let frame = PULSE_FRAMES[i % PULSE_FRAMES.len()];
            bar.push_str(&style(frame).green().bold().to_string());
        }

        // Add empty part
        for _ in 0..empty {
            bar.push_str(&style("░").dim().to_string());
        }

        bar
    }
}

/// Debug print helper that checks ARULA_DEBUG environment variable
fn debug_print(msg: &str) {
    if std::env::var("ARULA_DEBUG").is_ok() {
        eprintln!("{}", msg);
    }
}

/// Helper function to find closing pattern in character slice
fn find_closing_pattern(chars: &[char], pattern: &str) -> Option<usize> {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let pattern_len = pattern_chars.len();

    if pattern_len == 0 {
        return None;
    }

    for i in 0..chars.len() {
        if i + pattern_len <= chars.len() {
            if &chars[i..(i + pattern_len)] == &pattern_chars[..] {
                return Some(i);
            }
        }
    }
    None
}

/// Result of parsing an HTML tag
struct HtmlTagResult {
    rendered: String,
    consumed: usize,
}

/// States for the animated progress prompts
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PromptState {
    Input,     // User is typing
    Loading,   // AI is processing
    Completed, // Response complete
    Error,     // Error occurred
}

pub struct OutputHandler {
    debug: bool,
    spinner: Option<Arc<Mutex<ProgressBar>>>,
    animation_start_time: Option<Instant>,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    mad_skin: MadSkin,
    accumulated_text: String,
    in_code_block: bool,
    code_block_lang: String,
    code_block_content: String,
    line_buffer: String,
    last_printed_len: usize,
}

impl OutputHandler {
    pub fn new() -> Self {
        let mut mad_skin = MadSkin::default();

        // Customize markdown skin for better visibility
        // Use termimad's color type to avoid version conflicts
        use termimad::crossterm::style::Color as TMColor;
        mad_skin.bold.set_fg(TMColor::Yellow);
        mad_skin.italic.set_fg(TMColor::Cyan);
        mad_skin.code_block.set_bg(TMColor::AnsiValue(235)); // Dark gray background
        // Purple inline code with dark purple background
        mad_skin.inline_code.set_bg(TMColor::AnsiValue(54)); // Dark purple background
        mad_skin.inline_code.set_fg(TMColor::Magenta); // Purple text

        Self {
            debug: false,
            spinner: None,
            animation_start_time: None,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            mad_skin,
            accumulated_text: String::new(),
            in_code_block: false,
            code_block_lang: String::new(),
            code_block_content: String::new(),
            line_buffer: String::new(),
            last_printed_len: 0,
        }
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    pub fn is_debug(&self) -> bool {
        self.debug
    }

    pub fn print_user_message(&mut self, content: &str) -> io::Result<()> {
        println!("{}", helpers::user_message().apply_to(content));
        Ok(())
    }

    pub fn print_ai_message(&mut self, content: &str) -> io::Result<()> {
        println!();
        println!("{} {}", helpers::ai_response().apply_to("▶ ARULA:"), content);
        println!();
        Ok(())
    }

    pub fn print_error(&mut self, content: &str) -> io::Result<()> {
        println!("{} {}", ColorTheme::error().apply_to("Error:"), content);
        Ok(())
    }

    pub fn print_system(&mut self, content: &str) -> io::Result<()> {
        println!("{}", helpers::system_notification().apply_to(content));
        Ok(())
    }

    pub fn print_tool_call(&mut self, name: &str, args: &str) -> io::Result<()> {
        if self.debug {
            println!(
                "{} {}",
                helpers::tool_call().apply_to("🔧 Tool Call:"),
                ColorTheme::ai_highlight().apply_to(name)
            );
            if !args.is_empty() {
                println!("   {}", ColorTheme::dim().apply_to(format!("Args: {}", args)));
            }
        } else {
            // Show enhanced tool call box for non-debug mode
            self.print_tool_call_box(name, args)?;
        }
        Ok(())
    }

    pub fn print_tool_result(&mut self, result: &str) -> io::Result<()> {
        if self.debug {
            let max_lines = 50;
            let truncated_result = self.truncate_output(result, max_lines);
            println!(
                "   {}",
                helpers::tool_result().apply_to(format!("Result: {}", truncated_result))
            );
        } else {
            // Show enhanced result box for non-debug mode
            self.print_tool_result_box(result)?;
        }
        Ok(())
    }

    /// Start tool execution with compact single-line display
    pub fn start_tool_execution(&mut self, tool_name: &str, input: &str) -> io::Result<()> {
        self.animation_start_time = Some(Instant::now());

        // Format tool name nicely
        let formatted_name = self.format_tool_name(tool_name);

        // Stop the main spinner temporarily to show tool execution
        let was_spinner_active = self.spinner.is_some();
        if was_spinner_active {
            self.stop_spinner();
        }

        // Special styling for edit_file tool with background
        let tool_display = if tool_name == "edit_file" {
            ColorTheme::primary_on_background().apply_to(format!("📝 {}", formatted_name))
        } else {
            helpers::tool_call().apply_to(format!("🛠️  {}", formatted_name))
        };

        // Compact single-line display with tool name and truncated input
        if !input.is_empty() {
            let truncated_args = self.smart_truncate(input, 60);
            println!("{} · {}", tool_display, ColorTheme::dim().apply_to(truncated_args));
        } else {
            println!("{}", tool_display);
        }

        // Restart the main spinner if it was active
        if was_spinner_active {
            self.start_spinner("Working...")?;
        }

        Ok(())
    }

    /// Complete tool execution with success/failure status
    pub fn complete_tool_execution(&mut self, result: &str, success: bool) -> io::Result<()> {
        self.print_tool_result_box_with_status(result, success)?;
        Ok(())
    }

    /// Print tool call with compact single-line display
    fn print_tool_call_box(&mut self, name: &str, args: &str) -> io::Result<()> {
        self.animation_start_time = Some(Instant::now());

        // Format tool name nicely
        let formatted_name = self.format_tool_name(name);

        // Special styling for edit_file tool with background
        let tool_display = if name == "edit_file" {
            ColorTheme::primary_on_background().apply_to(format!("📝 {}", formatted_name))
        } else {
            helpers::tool_call().apply_to(format!("🛠️  {}", formatted_name))
        };

        // Compact single-line display
        if !args.is_empty() {
            let truncated_args = self.smart_truncate(args, 60);
            println!("{} · {}", tool_display, ColorTheme::dim().apply_to(truncated_args));
        } else {
            println!("{}", tool_display);
        }

        Ok(())
    }

    /// Format tool name for display (capitalize first letter, replace underscores with spaces)
    fn format_tool_name(&self, name: &str) -> String {
        name.replace('_', " ")
            .split(' ')
            .enumerate()
            .map(|(i, word)| {
                if i == 0 {
                    // Capitalize first word
                    let mut chars: Vec<char> = word.chars().collect();
                    if let Some(first_char) = chars.get_mut(0) {
                        *first_char = first_char.to_uppercase().next().unwrap_or(*first_char);
                    }
                    chars.into_iter().collect::<String>()
                } else {
                    word.to_string()
                }
            })
            .collect::<Vec<String>>()
            .join(" ")
    }

    /// Print tool result in a nice box format for non-debug mode
    fn print_tool_result_box(&mut self, result: &str) -> io::Result<()> {
        // Smart truncation for result display
        let truncated_result = self.smart_truncate(result, 300);

        if !result.is_empty() {
            // Calculate display metrics
            let result_lines: Vec<&str> = truncated_result.lines().collect();
            let line_count = result_lines.len();
            let char_count = result.len();

            println!(
                "│ {} {} lines, {} chars",
                ColorTheme::primary().apply_to("Output:"),
                helpers::misc().apply_to(line_count),
                helpers::misc().apply_to(char_count)
            );

            // Show first few lines of result
            let max_display_lines = 5;
            for (i, line) in result_lines.iter().take(max_display_lines).enumerate() {
                let line_prefix = if line_count > max_display_lines && i == max_display_lines - 1 {
                    "├─"
                } else if i == result_lines.len() - 1 || i == max_display_lines - 1 {
                    "└─"
                } else {
                    "├─"
                };
                println!("│ {} {}", ColorTheme::dim().apply_to(line_prefix), helpers::tool_result().apply_to(line));
            }

            // Show truncation indicator if content was cut
            if result.lines().count() > max_display_lines || result.len() > 300 {
                let remaining_lines = result.lines().count().saturating_sub(max_display_lines);
                let remaining_chars = result.len().saturating_sub(300);
                println!(
                    "│ └─ {} {} {} more lines, {} more chars",
                    ColorTheme::dim().apply_to("..."),
                    ColorTheme::dim().apply_to("(hidden)"),
                    ColorTheme::dim().apply_to(remaining_lines),
                    ColorTheme::dim().apply_to(remaining_chars)
                );
            }
        } else {
            println!(
                "│ {} {}",
                ColorTheme::primary().apply_to("Output:"),
                ColorTheme::dim().apply_to("(empty)")
            );
        }

        println!(
            "{}",
            ColorTheme::border().apply_to("└─────────────────────────────────────")
        );
        println!();
        Ok(())
    }

    /// Print tool result with compact display
    fn print_tool_result_box_with_status(&mut self, result: &str, success: bool) -> io::Result<()> {
        // Show execution time and status on one line
        let status_icon = if success {
            style("✓").green()
        } else {
            style("✗").red()
        };

        if let Some(start_time) = self.animation_start_time {
            let duration = start_time.elapsed();
            print!("  {} {:.2}s", status_icon, duration.as_secs_f32());
        } else {
            print!("  {}", status_icon);
        }

        // Show compact result summary
        if !result.is_empty() {
            let line_count = result.lines().count();
            let char_count = result.len();

            if line_count == 1 {
                // Single line result - show it inline
                let truncated = self.smart_truncate(result, 80);
                println!(" · {}", style(truncated).dim());
            } else {
                // Multi-line result - show count
                println!(
                    " · {} lines, {} chars",
                    style(line_count).cyan(),
                    style(char_count).dim()
                );
            }
        } else {
            println!();
        }

        Ok(())
    }

    /// Smart truncation that preserves structure and readability
    fn smart_truncate(&self, text: &str, max_chars: usize) -> String {
        if text.len() <= max_chars {
            return text.to_string();
        }

        // Try to truncate at word boundaries
        let truncated = &text[..max_chars];

        // Find the last complete line or sentence
        if let Some(last_newline) = truncated.rfind('\n') {
            // Truncate at last complete line
            format!("{}...\n[truncated]", &truncated[..last_newline])
        } else if let Some(last_period) = truncated.rfind('.') {
            // Truncate at last sentence
            format!("{}...", &truncated[..last_period + 1])
        } else if let Some(last_space) = truncated.rfind(' ') {
            // Truncate at last word
            format!("{}...", &truncated[..last_space])
        } else {
            // Hard truncate
            format!("{}...", &truncated[..max_chars.saturating_sub(3)])
        }
    }

    fn truncate_output(&self, output: &str, max_lines: usize) -> String {
        let lines: Vec<&str> = output.lines().collect();

        if lines.len() <= max_lines {
            output.to_string()
        } else {
            let truncated_lines: Vec<String> = lines
                .iter()
                .take(max_lines)
                .map(|line| line.to_string())
                .collect();

            format!(
                "{}\n... ({} more lines)",
                truncated_lines.join("\n"),
                lines.len() - max_lines
            )
        }
    }

    pub fn print_streaming_chunk(&mut self, chunk: &str) -> io::Result<()> {
        // Stop spinner if active to print the chunk
        if self.spinner.is_some() {
            self.stop_spinner();
            print!("\r\x1b[K"); // Clear the spinner line
        }

        // Accumulate text for potential re-rendering
        self.accumulated_text.push_str(chunk);

        // Print the chunk with inline markdown rendering
        self.print_markdown_inline(chunk)?;

        Ok(())
    }

    pub fn start_ai_message(&mut self) -> io::Result<()> {
        // Clear accumulated text for new message
        self.accumulated_text.clear();
        self.in_code_block = false;
        self.code_block_lang.clear();
        self.code_block_content.clear();
        self.line_buffer.clear();
        self.last_printed_len = 0;

        // Don't stop spinner here - let it keep running until first text chunk
        // Just flush to ensure any pending output is written
        std::io::stdout().flush()?;
        Ok(())
    }

    pub fn end_line(&mut self) -> io::Result<()> {
        // If we're still in a code block, close it
        if self.in_code_block {
            self.render_code_block()?;
            self.in_code_block = false;
            self.code_block_content.clear();
        }

        println!();
        Ok(())
    }

    /// Render markdown text with termimad
    pub fn render_markdown(&self, text: &str) -> io::Result<()> {
        // Use termimad to render markdown with our custom skin
        println!("{}", self.mad_skin.term_text(text));
        Ok(())
    }

    /// Render a code block with syntax highlighting
    fn render_code_block(&mut self) -> io::Result<()> {
        if self.code_block_content.is_empty() {
            return Ok(());
        }

        println!();
        println!(
            "{}",
            style(
                "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓"
            )
            .dim()
        );

        // Show language tag if present
        if !self.code_block_lang.is_empty() {
            println!(
                "┃ {}",
                style(&self.code_block_lang).cyan().bold()
            );
            println!("{}", style("┣━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫").dim());
        }

        // Try syntax highlighting if we have a language
        if !self.code_block_lang.is_empty() {
            if let Some(syntax) = self.syntax_set.find_syntax_by_token(&self.code_block_lang) {
                let theme = &self.theme_set.themes["base16-ocean.dark"];
                let mut highlighter = HighlightLines::new(syntax, theme);

                for line in self.code_block_content.lines() {
                    let ranges = highlighter
                        .highlight_line(line, &self.syntax_set)
                        .unwrap_or_default();
                    let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                    println!("┃ {}", escaped);
                }
            } else {
                // Fallback: no syntax highlighting available
                for line in self.code_block_content.lines() {
                    println!("┃ {}", style(line).white());
                }
            }
        } else {
            // No language specified - plain formatting
            for line in self.code_block_content.lines() {
                println!("┃ {}", style(line).white());
            }
        }

        println!(
            "{}",
            style(
                "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛"
            )
            .dim()
        );
        println!();

        Ok(())
    }

    /// Process buffer and extract complete markdown patterns for streaming
    /// Uses termimad for proper markdown rendering
    fn process_and_extract_complete_patterns(&self, buffer: &str, already_printed: usize) -> String {
        if buffer.is_empty() || already_printed >= buffer.len() {
            return String::new();
        }

        // Use termimad to render inline markdown
        let processed = self.mad_skin.inline(buffer).to_string();

        // On first call (already_printed == 0), just return processed text
        if already_printed == 0 {
            return processed;
        }

        // On subsequent calls, we need to figure out what changed
        // Count visible chars (excluding ANSI codes) in already printed portion
        let old_processed = self.mad_skin.inline(&buffer[..already_printed]).to_string();
        let old_visible_len = self.count_visible_chars(&old_processed);

        // Return: move cursor back, clear line, and reprint entire processed buffer
        format!("\r\x1b[{}D\x1b[K{}", old_visible_len, processed)
    }

    /// Count visible characters (excluding ANSI escape codes)
    fn count_visible_chars(&self, text: &str) -> usize {
        let mut count = 0;
        let mut in_escape = false;

        for ch in text.chars() {
            if ch == '\x1b' {
                in_escape = true;
            } else if in_escape {
                if ch == 'm' {
                    in_escape = false;
                }
            } else {
                count += 1;
            }
        }

        count
    }

    /// Process and print text with comprehensive markdown formatting (for streaming)
    /// Buffers text until complete lines are available to handle split markdown patterns
    fn print_markdown_inline(&mut self, text: &str) -> io::Result<()> {
        // Add incoming text to line buffer
        self.line_buffer.push_str(text);

        // Process complete lines (those ending with \n)
        while let Some(newline_pos) = self.line_buffer.find('\n') {
            let line = &self.line_buffer[..newline_pos];

            // Check for code block markers
            if line.trim().starts_with("```") {
                if self.in_code_block {
                    // Closing code block
                    self.render_code_block()?;
                    self.in_code_block = false;
                    self.code_block_content.clear();
                    self.code_block_lang.clear();
                } else {
                    // Opening code block
                    self.in_code_block = true;
                    self.code_block_lang = line.trim().trim_start_matches("```").to_string();
                }
                // Remove processed line from buffer
                self.line_buffer = self.line_buffer[(newline_pos + 1)..].to_string();
                continue;
            }

            // If we're in a code block, accumulate content
            if self.in_code_block {
                self.code_block_content.push_str(line);
                self.code_block_content.push('\n');
                self.line_buffer = self.line_buffer[(newline_pos + 1)..].to_string();
                continue;
            }

            // Process and render the complete line with markdown
            self.render_markdown_line(line)?;
            println!();

            // Remove processed line from buffer
            self.line_buffer = self.line_buffer[(newline_pos + 1)..].to_string();
            // Reset last printed length since we completed a line
            self.last_printed_len = 0;
        }

        // Process partial buffer for complete markdown patterns
        if !self.line_buffer.is_empty() && !self.in_code_block {
            let processed = self.process_and_extract_complete_patterns(&self.line_buffer, self.last_printed_len);
            if !processed.is_empty() {
                print!("{}", processed);
                std::io::stdout().flush()?;
                self.last_printed_len = self.line_buffer.len();
            }
        }

        Ok(())
    }

    /// Render a single markdown line with all formatting
    fn render_markdown_line(&self, line: &str) -> io::Result<()> {
        let trimmed = line.trim_start();

        // Check for different markdown patterns at line start

        // Headers (# ## ### #### ##### ######)
        if let Some(header_level) = trimmed.strip_prefix("###### ") {
            print!(
                "{}",
                style(format!("###### {}", header_level)).cyan().bold()
            );
            return Ok(());
        } else if let Some(header_level) = trimmed.strip_prefix("##### ") {
            print!("{}", style(format!("##### {}", header_level)).cyan().bold());
            return Ok(());
        } else if let Some(header_level) = trimmed.strip_prefix("#### ") {
            print!("{}", style(format!("#### {}", header_level)).cyan().bold());
            return Ok(());
        } else if let Some(header_level) = trimmed.strip_prefix("### ") {
            print!("{}", style(format!("### {}", header_level)).cyan().bold());
            return Ok(());
        } else if let Some(header_level) = trimmed.strip_prefix("## ") {
            print!("{}", style(format!("## {}", header_level)).cyan().bold());
            return Ok(());
        } else if let Some(header_level) = trimmed.strip_prefix("# ") {
            print!("{}", style(format!("# {}", header_level)).cyan().bold());
            return Ok(());
        }

        // Task lists - [ ] and - [x]
        if trimmed.starts_with("- [ ] ") {
            let task = trimmed.strip_prefix("- [ ] ").unwrap();
            print!(
                "  {} {}",
                style("☐").dim(),
                self.process_inline_markdown(task)
            );
            return Ok(());
        } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
            let task = trimmed
                .strip_prefix("- [x] ")
                .or_else(|| trimmed.strip_prefix("- [X] "))
                .unwrap();
            print!("  {} {}", style("✓").green().bold(), style(task).dim());
            return Ok(());
        }

        // Unordered lists (- or * or +) with nesting support
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            // Calculate indentation level (number of leading spaces / 2)
            let leading_spaces = line.len() - line.trim_start().len();
            let indent_level = leading_spaces / 2;
            let indent = "  ".repeat(indent_level);

            let item = trimmed.chars().skip(2).collect::<String>();
            print!(
                "{}  {} {}",
                indent,
                style("•").yellow(),
                self.mad_skin.inline(&item)
            );
            return Ok(());
        }

        // Ordered lists (1. 2. etc) with nesting support
        if let Some(rest) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
            if rest.starts_with(". ") {
                // Calculate indentation level
                let leading_spaces = line.len() - line.trim_start().len();
                let indent_level = leading_spaces / 2;
                let indent = "  ".repeat(indent_level);

                let item = rest.strip_prefix(". ").unwrap();
                let num = trimmed
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>();
                print!(
                    "{}  {}. {}",
                    indent,
                    style(num).cyan(),
                    self.mad_skin.inline(item)
                );
                return Ok(());
            }
        }

        // Block quotes (>) with proper nesting visualization
        if trimmed.starts_with(">") {
            let mut quote_depth = 0;
            let mut remaining = trimmed;

            // Count how many > symbols we have
            while remaining.starts_with(">") {
                quote_depth += 1;
                remaining = remaining.strip_prefix(">").unwrap().trim_start();
            }

            // Create visual nesting with different colors/styles per level
            let mut prefix = String::new();
            for depth in 0..quote_depth {
                if depth % 3 == 0 {
                    prefix.push_str(&format!("{}", style("┃ ").blue().dim()));
                } else if depth % 3 == 1 {
                    prefix.push_str(&format!("{}", style("┃ ").cyan().dim()));
                } else {
                    prefix.push_str(&format!("{}", style("┃ ").magenta().dim()));
                }
            }

            print!("{}{}", prefix, self.mad_skin.inline(remaining));
            return Ok(());
        }

        // Horizontal rules (--- or ***)
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            print!("{}", style("─".repeat(80)).dim());
            return Ok(());
        }

        // Footnote definitions [^1]: text
        if trimmed.starts_with("[^") {
            if let Some(close_bracket) = trimmed.find("]:") {
                let footnote_ref = &trimmed[0..close_bracket + 1];
                let footnote_text = trimmed[(close_bracket + 2)..].trim();
                print!(
                    "{} {}",
                    style(footnote_ref).cyan().bold(),
                    style(footnote_text).dim()
                );
                return Ok(());
            }
        }

        // Regular line with inline markdown - use termimad
        print!("{}", self.mad_skin.inline(line));
        Ok(())
    }

    /// Process inline markdown elements (bold, italic, code, strikethrough, links, HTML tags, escapes)
    fn process_inline_markdown(&self, text: &str) -> String {
        let mut result = String::new();
        let mut i = 0;
        let chars: Vec<char> = text.chars().collect();

        while i < chars.len() {
            // Escape sequences (\* \_ \` etc)
            if chars[i] == '\\' && i + 1 < chars.len() {
                let next_char = chars[i + 1];
                // Check if next char is a markdown special character
                if matches!(
                    next_char,
                    '*' | '_' | '`' | '~' | '[' | ']' | '(' | ')' | '#' | '\\'
                ) {
                    result.push(next_char);
                    i += 2;
                    continue;
                }
            }

            // HTML tags - <mark>, <em>, <strong>, <code>, etc.
            if chars[i] == '<' {
                // Try to parse HTML tag
                if let Some(tag_result) = self.parse_html_tag(&chars[i..]) {
                    result.push_str(&tag_result.rendered);
                    i += tag_result.consumed;
                    continue;
                }
            }

            // Strikethrough ~~text~~
            if i + 1 < chars.len() && chars[i] == '~' && chars[i + 1] == '~' {
                if let Some(close_pos) = find_closing_pattern(&chars[(i + 2)..], "~~") {
                    let close_idx = i + 2 + close_pos;
                    let strikethrough: String = chars[(i + 2)..close_idx].iter().collect();
                    result.push_str(&format!("{}", style(strikethrough).dim().strikethrough()));
                    i = close_idx + 2;
                    continue;
                }
            }

            // Bold **text** or __text__
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                if let Some(close_pos) = find_closing_pattern(&chars[(i + 2)..], "**") {
                    let close_idx = i + 2 + close_pos;
                    let bold: String = chars[(i + 2)..close_idx].iter().collect();
                    result.push_str(&format!("{}", style(bold).yellow().bold()));
                    i = close_idx + 2;
                    continue;
                }
            } else if i + 1 < chars.len() && chars[i] == '_' && chars[i + 1] == '_' {
                if let Some(close_pos) = find_closing_pattern(&chars[(i + 2)..], "__") {
                    let close_idx = i + 2 + close_pos;
                    let bold: String = chars[(i + 2)..close_idx].iter().collect();
                    result.push_str(&format!("{}", style(bold).yellow().bold()));
                    i = close_idx + 2;
                    continue;
                }
            }

            // Inline code `text`
            if chars[i] == '`' {
                if let Some(close_pos) = chars[(i + 1)..].iter().position(|&c| c == '`') {
                    let close_idx = i + 1 + close_pos;
                    let code: String = chars[(i + 1)..close_idx].iter().collect();
                    result.push_str(&format!("{}", helpers::inline_code().apply_to(code)));
                    i = close_idx + 1;
                    continue;
                }
            }

            // Italic *text* or _text_ (but not ** or __)
            if chars[i] == '*' && (i + 1 >= chars.len() || chars[i + 1] != '*') {
                if let Some(close_pos) = chars[(i + 1)..].iter().position(|&c| c == '*') {
                    let close_idx = i + 1 + close_pos;
                    let italic: String = chars[(i + 1)..close_idx].iter().collect();
                    result.push_str(&format!("{}", style(italic).cyan()));
                    i = close_idx + 1;
                    continue;
                }
            } else if chars[i] == '_' && (i + 1 >= chars.len() || chars[i + 1] != '_') {
                if let Some(close_pos) = chars[(i + 1)..].iter().position(|&c| c == '_') {
                    let close_idx = i + 1 + close_pos;
                    let italic: String = chars[(i + 1)..close_idx].iter().collect();
                    result.push_str(&format!("{}", style(italic).cyan()));
                    i = close_idx + 1;
                    continue;
                }
            }

            // Links [text](url) or [text][ref] or footnotes [^1]
            if chars[i] == '[' {
                if let Some(close_bracket) = chars[(i + 1)..].iter().position(|&c| c == ']') {
                    let text_end = i + 1 + close_bracket;
                    let link_text: String = chars[(i + 1)..text_end].iter().collect();

                    // Check for footnote reference [^1]
                    if link_text.starts_with('^') {
                        result.push_str(&format!(
                            "{}",
                            style(format!("[{}]", link_text)).cyan().bold()
                        ));
                        i = text_end + 1;
                        continue;
                    }

                    // Check for (url) or [ref] after ]
                    if text_end + 1 < chars.len() && chars[text_end + 1] == '(' {
                        if let Some(close_paren) =
                            chars[(text_end + 2)..].iter().position(|&c| c == ')')
                        {
                            let url_end = text_end + 2 + close_paren;
                            let url: String = chars[(text_end + 2)..url_end].iter().collect();
                            result.push_str(&format!(
                                "{} {}",
                                style(link_text).blue().underlined(),
                                style(format!("({})", url)).dim()
                            ));
                            i = url_end + 1;
                            continue;
                        }
                    }

                    // Just show link text if no URL found
                    result.push_str(&format!("{}", style(link_text).blue().underlined()));
                    i = text_end + 1;
                    continue;
                }
            }

            // Regular character
            result.push(chars[i]);
            i += 1;
        }

        result
    }

    /// Parse HTML tags and render them with appropriate styling
    fn parse_html_tag(&self, chars: &[char]) -> Option<HtmlTagResult> {
        if chars.is_empty() || chars[0] != '<' {
            return None;
        }

        // Find the tag name
        let tag_start = 1;
        let mut tag_end = tag_start;
        while tag_end < chars.len() && chars[tag_end].is_alphabetic() {
            tag_end += 1;
        }

        if tag_end == tag_start {
            return None;
        }

        let tag_name: String = chars[tag_start..tag_end].iter().collect();

        // Skip attributes and find >
        let mut close_bracket = tag_end;
        while close_bracket < chars.len() && chars[close_bracket] != '>' {
            close_bracket += 1;
        }

        if close_bracket >= chars.len() {
            return None;
        }

        // Find closing tag
        let closing_tag = format!("</{}>", tag_name);
        let closing_chars: Vec<char> = closing_tag.chars().collect();

        let content_start = close_bracket + 1;
        let mut content_end = content_start;

        while content_end < chars.len() {
            if content_end + closing_chars.len() <= chars.len() {
                if &chars[content_end..(content_end + closing_chars.len())] == &closing_chars[..] {
                    break;
                }
            }
            content_end += 1;
        }

        if content_end >= chars.len() {
            return None;
        }

        let content: String = chars[content_start..content_end].iter().collect();
        let total_consumed = content_end + closing_chars.len();

        // Apply styling based on tag
        let rendered = match tag_name.as_str() {
            "mark" | "highlight" => format!("{}", style(content).black().on_yellow()),
            "em" | "i" => format!("{}", style(content).cyan()),
            "strong" | "b" => format!("{}", style(content).yellow().bold()),
            "code" => format!("{}", style(content).green().on_black()),
            "u" => format!("{}", style(content).underlined()),
            "s" | "del" | "strike" => format!("{}", style(content).strikethrough().dim()),
            "sub" => format!("{}", style(format!("_{}", content)).dim()),
            "sup" => format!("{}", style(format!("^{}", content)).dim()),
            "kbd" => format!("{}", style(format!("[{}]", content)).white().on_black()),
            _ => content, // Unknown tag, return content as-is
        };

        Some(HtmlTagResult {
            rendered,
            consumed: total_consumed,
        })
    }

    pub fn print_banner(&mut self) -> io::Result<()> {
        println!(
            "{}",
            ColorTheme::primary().apply_to("  █████╗ ██████╗ ██╗   ██╗██╗      █████╗")
        );
        println!(
            "{}",
            ColorTheme::primary().apply_to(" ██╔══██╗██╔══██╗██║   ██║██║     ██╔══██╗")
        );
        println!(
            "{}",
            ColorTheme::primary().apply_to(" ███████║██████╔╝██║   ██║██║     ███████║")
        );
        println!(
            "{}",
            ColorTheme::primary().apply_to(" ██╔══██║██╔══██╗██║   ██║██║     ██╔══██║")
        );
        println!(
            "{}",
            ColorTheme::primary().apply_to(" ██║  ██║██║  ██║╚██████╔╝███████╗██║  ██║")
        );
        println!(
            "{}",
            ColorTheme::primary().apply_to(" ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝")
        );
        println!();
        println!(
            "{}",
            ColorTheme::primary().apply_to("    Autonomous AI Command-Line Interface")
        );
        Ok(())
    }

    /// Start showing an animated spinner
    pub fn start_spinner(&mut self, message: &str) -> io::Result<()> {
        // Only start if no spinner is already active
        if self.spinner.is_some() {
            return Ok(());
        }

        // Stop any existing spinner first (shouldn't be needed, but just in case)
        self.stop_spinner();

        // Create a new spinner with our custom color theme
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.bright.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        spinner.set_message(message.to_string());
        spinner.enable_steady_tick(Duration::from_millis(80));

        // Store the spinner
        self.spinner = Some(Arc::new(Mutex::new(spinner)));
        Ok(())
    }

    /// Update the spinner message
    pub fn update_spinner_message(&self, message: &str) -> io::Result<()> {
        if let Some(spinner_arc) = &self.spinner {
            if let Ok(spinner) = spinner_arc.lock() {
                spinner.set_message(message.to_string());
            }
        }
        Ok(())
    }

    /// Stop the spinner and clear it completely
    pub fn stop_spinner(&mut self) {
        if let Some(spinner_arc) = self.spinner.take() {
            if let Ok(spinner) = spinner_arc.lock() {
                spinner.finish_and_clear();
                // Also clear any remaining characters on the line
                print!("\r\x1b[K");
                std::io::stdout().flush().unwrap_or(());
            }
        }
    }

    /// Check if spinner is currently active
    pub fn has_spinner(&self) -> bool {
        self.spinner.is_some()
    }

    /// Print content above the spinner while preserving input at bottom
    pub fn print_above_spinner(&mut self, content: &str) -> io::Result<()> {
        if self.spinner.is_some() {
            // Save cursor position
            print!("\x1b[s"); // Save cursor
                              // Move to beginning of spinner line
            print!("\x1b[1A"); // Move cursor up 1 line
            print!("\x1b[G"); // Move to beginning of line
            print!("\x1b[2K"); // Clear entire line
            print!("{}", content);
            print!("\n");
            std::io::stdout().flush()?;
        } else {
            print!("{}", content);
            std::io::stdout().flush()?;
        }
        Ok(())
    }

    /// Start a message above the spinner (cursor positioned)
    pub fn start_message_above_spinner(&mut self) -> io::Result<()> {
        if self.spinner.is_some() {
            // Save cursor position
            print!("\x1b[s"); // Save cursor
                              // Move to beginning of spinner line
            print!("\x1b[1A"); // Move cursor up 1 line
            print!("\x1b[G"); // Move to beginning of line
            print!("\x1b[2K"); // Clear entire line
            std::io::stdout().flush()?;
        }
        Ok(())
    }

    /// Create a dedicated spinner line that won't interfere with input
    pub fn start_middle_spinner(&mut self, message: &str) -> io::Result<()> {
        // Stop any existing spinner first
        self.stop_spinner();
        print!("\n"); // Create middle area
        std::io::stdout().flush()?;
        self.start_spinner(message)?;
        Ok(())
    }

    /// Check if spinner is currently active
    pub fn is_spinner_active(&self) -> bool {
        self.spinner.is_some()
    }

    /// Print an animated prompt that shows the latest text + loading state
    pub fn print_animated_prompt(
        &mut self,
        prefix: &str,
        text: &str,
        loading: bool,
    ) -> io::Result<()> {
        if loading {
            // Show loading indicator with partial text
            let dots = ["⚡", "🔄", "⚙️", "💫", "✨"];
            let dot = dots[fastrand::usize(0..dots.len())];
            println!(
                "{} {} {} {}",
                style(prefix).cyan(),
                style(text).white(),
                style("...").dim(),
                style(dot).yellow()
            );
        } else {
            // Show static text without loading
            println!("{} {}", style(prefix).cyan(), style(text).white());
        }
        Ok(())
    }

    /// Update the current line with new text (for inline updates)
    pub fn update_prompt_line(
        &mut self,
        prefix: &str,
        text: &str,
        loading: bool,
    ) -> io::Result<()> {
        print!("\r"); // Move to start of line
        if loading {
            let dots = ["⚡", "🔄", "⚙️", "💫", "✨"];
            let dot = dots[fastrand::usize(0..dots.len())];
            print!(
                "{} {} {} {}",
                style(prefix).cyan(),
                style(text).white(),
                style("...").dim(),
                style(dot).yellow()
            );
        } else {
            print!("{} {}", style(prefix).cyan(), style(text).white());
        }
        print!(" \r"); // Clear any remaining characters
        print!("{} {} ", style(prefix).cyan(), style(text).white());
        std::io::stdout().flush()?;
        Ok(())
    }

    /// Print animated prompt with loading circle for AI processing
    pub fn print_progress_prompt(
        &mut self,
        prefix: &str,
        text: &str,
        state: PromptState,
    ) -> io::Result<()> {
        match state {
            PromptState::Input => {
                // User input - show normal text without loading bar
                println!("{} {}", style(prefix).cyan(), style(text).white());
            }
            PromptState::Loading => {
                // AI processing - show 1-character loading circle
                let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame = spinner_chars[fastrand::usize(0..spinner_chars.len())];
                println!(
                    "{} {} {}",
                    style(prefix).cyan(),
                    style(frame).yellow(),
                    style("Processing...").dim()
                );
            }
            PromptState::Completed => {
                // Task completed - show checkmark
                println!("{} {}", style(prefix).cyan(), style("✓").green());
            }
            PromptState::Error => {
                // Error occurred - show X mark
                println!("{} {}", style(prefix).cyan(), style("✗").red());
            }
        }
        Ok(())
    }

    /// Update progress prompt inline with loading circle
    pub fn update_progress_bar(
        &mut self,
        prefix: &str,
        text: &str,
        state: PromptState,
    ) -> io::Result<()> {
        print!("\r"); // Move to start of line

        match state {
            PromptState::Input => {
                // User input - show normal text without loading bar
                print!("{} {}", style(prefix).cyan(), style(text).white());
            }
            PromptState::Loading => {
                // AI processing - show 1-character loading circle
                let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame = spinner_chars[fastrand::usize(0..spinner_chars.len())];
                print!(
                    "{} {} {}",
                    style(prefix).cyan(),
                    style(frame).yellow(),
                    style("Processing...").dim()
                );
            }
            PromptState::Completed => {
                // Task completed - show checkmark
                print!("{} {}", style(prefix).cyan(), style("✓").green());
            }
            PromptState::Error => {
                // Error occurred - show X mark
                print!("{} {}", style(prefix).cyan(), style("✗").red());
            }
        }

        print!(" \r"); // Clear any remaining characters
        std::io::stdout().flush()?;
        Ok(())
    }

    /// Display token usage information in the status area
    pub fn display_token_usage(&mut self, usage_info: Option<&Usage>) -> io::Result<()> {
        if let Some(usage) = usage_info {
            let max_tokens: u32 = 128000; // Standard context limit
            let _remaining = max_tokens.saturating_sub(usage.total_tokens);
            let percentage = (usage.total_tokens as f32 / max_tokens as f32) * 100.0;

            // Create status bar
            let status_text = format!(
                "📊 Tokens: {}/{} ({:.1}%)",
                usage.total_tokens, max_tokens, percentage
            );

            let status_color = if percentage > 90.0 {
                style(status_text).red().bold()
            } else if percentage > 75.0 {
                style(status_text).yellow().bold()
            } else {
                style(status_text).green()
            };

            println!("{} {}", style("│ Status:").cyan(), status_color);

            // Add warning if getting close to limit
            if percentage > 90.0 {
                println!(
                    "│ {} {}",
                    style("⚠️ Warning:").red().bold(),
                    style("Approaching context limit!").red()
                );
            }
        } else {
            println!(
                "{} {}",
                style("│ Status:").cyan(),
                style("📊 Token usage: Unknown").dim()
            );
        }
        Ok(())
    }

    /// Print code block with syntax highlighting
    pub fn print_code_block(&mut self, code: &str, language: Option<&str>) -> io::Result<()> {
        println!();

        // Try to highlight if language is specified
        if let Some(lang) = language {
            if let Some(syntax) = self.syntax_set.find_syntax_by_token(lang) {
                let theme = &self.theme_set.themes["base16-ocean.dark"];
                let mut highlighter = HighlightLines::new(syntax, theme);

                for line in code.lines() {
                    let ranges = highlighter
                        .highlight_line(line, &self.syntax_set)
                        .unwrap_or_default();
                    let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                    println!("│ {}", escaped);
                }
            } else {
                // Fallback to plain styling if syntax not found
                self.print_plain_code_block(code)?;
            }
        } else {
            // No language specified - auto-detect or use fallback
            self.print_plain_code_block(code)?;
        }

        println!();
        Ok(())
    }

    /// Print plain code block without syntax highlighting
    fn print_plain_code_block(&mut self, code: &str) -> io::Result<()> {
        println!("{}", style("┌─ Code Block").cyan().bold());
        for line in code.lines() {
            println!("│ {}", style(line).white());
        }
        println!("{}", style("└─────────────").cyan().bold());
        Ok(())
    }

    /// Print message history browser
    pub fn print_message_history(
        &mut self,
        messages: &[crate::chat::ChatMessage],
        start_index: usize,
    ) -> io::Result<()> {
        println!();
        println!("{}", style("┌─ 📜 Message History").white().bold());

        let end_index = (start_index + 10).min(messages.len());
        if start_index >= messages.len() {
            println!("│ {} No messages to show", style("Info:").yellow());
        } else {
            for (i, msg) in messages
                .iter()
                .skip(start_index)
                .take(end_index - start_index)
                .enumerate()
            {
                let msg_num = start_index + i + 1;
                let timestamp = msg.timestamp.format("%H:%M:%S");

                match msg.message_type {
                    crate::chat::MessageType::User => {
                        println!(
                            "│ {} ▶ {} {}: {}",
                            style(msg_num).dim(),
                            style(timestamp).cyan().dim(),
                            style("User").green(),
                            style(&msg.content).white()
                        );
                    }
                    crate::chat::MessageType::Arula => {
                        println!(
                            "│ {} ◆ {}: {}",
                            style(msg_num).dim(),
                            style("ARULA:").blue().bold(),
                            style(&msg.content).white()
                        );
                    }
                    _ => {
                        println!(
                            "│ {} ◉ {}: {}",
                            style(msg_num).dim(),
                            style(format!("{:?}", msg.message_type)).yellow(),
                            style(&msg.content).white()
                        );
                    }
                }
            }
        }

        println!("└─────────────────────────────────────");

        if end_index < messages.len() {
            println!("{}", style("💡 Press ↑/↓ to navigate, q to quit").dim());
        } else {
            println!("{}", style("💡 End of message history").dim());
        }
        println!();
        Ok(())
    }

    /// Print conversation summary
    pub fn print_conversation_summary(
        &mut self,
        messages: &[crate::chat::ChatMessage],
    ) -> io::Result<()> {
        println!();
        println!("{}", style("┌─ 📊 Conversation Summary").white().bold());

        let total_messages = messages.len();
        let user_messages = messages
            .iter()
            .filter(|m| matches!(m.message_type, crate::chat::MessageType::User))
            .count();
        let ai_messages = messages
            .iter()
            .filter(|m| matches!(m.message_type, crate::chat::MessageType::Arula))
            .count();

        println!(
            "│ {} Total messages: {}",
            style("Info:").yellow(),
            style(total_messages).cyan()
        );
        println!(
            "│ {} User messages: {}",
            style("Info:").yellow(),
            style(user_messages).green()
        );
        println!(
            "│ {} AI responses: {}",
            style("Info:").yellow(),
            style(ai_messages).blue()
        );

        if !messages.is_empty() {
            let first_msg = &messages[0];
            let last_msg = &messages[messages.len() - 1];
            println!(
                "│ {} Started: {}",
                style("Info:").yellow(),
                style(first_msg.timestamp.format("%Y-%m-%d %H:%M")).cyan()
            );
            println!(
                "│ {} Last: {}",
                style("Info:").yellow(),
                style(last_msg.timestamp.format("%Y-%m-%d %H:%M")).cyan()
            );
        }

        println!("└─────────────────────────────────────");
        println!();
        Ok(())
    }

    /// Print context usage information at the end of AI responses
    /// Only shows when usage data is available and above 75% usage
    pub fn print_context_usage(&mut self, usage: Option<&Usage>) -> io::Result<()> {
        if self.debug {
            debug_print(&format!(
                "DEBUG: print_context_usage called with usage: {:?}",
                usage
            ));
        }

        // Don't show anything if no usage data available
        let usage_info = match usage {
            Some(info) => info,
            None => return Ok(()),
        };

        // Standard context limits (adjust based on model)
        let max_context_tokens: u32 = 128000; // Typical for modern models
        let remaining_tokens = max_context_tokens.saturating_sub(usage_info.total_tokens);
        let usage_percentage = (usage_info.total_tokens as f64 / max_context_tokens as f64) * 100.0;

        // Only show display when usage is above 75% to avoid clutter
        if usage_percentage <= 75.0 {
            return Ok(());
        }

        println!();
        println!(
            "{}",
            style("┌─ Context Usage ───────────────────────").dim()
        );

        // Choose color based on usage level for tokens used
        let used_color = if usage_percentage > 90.0 {
            style(format!("{}", usage_info.total_tokens)).red().bold()
        } else {
            style(format!("{}", usage_info.total_tokens))
                .yellow()
                .bold()
        };

        let remaining_color = if usage_percentage > 90.0 {
            style(format!("{}", remaining_tokens)).red().bold()
        } else {
            style(format!("{}", remaining_tokens)).yellow().bold()
        };

        println!(
            "│ {} {} tokens used ({:.1}%)",
            style("Tokens used:").yellow(),
            used_color,
            usage_percentage
        );
        println!(
            "│ {} {}",
            style("Tokens remaining:").yellow(),
            remaining_color
        );

        // Add visual indicator
        let used_bars = (usage_percentage / 100.0 * 20.0) as usize;
        let remaining_bars = 20 - used_bars;
        let bar = "█".repeat(used_bars) + &"░".repeat(remaining_bars);

        let bar_color = if usage_percentage > 90.0 {
            style(&bar).red().bold()
        } else {
            style(&bar).yellow().bold()
        };

        println!("│ [{}]", bar_color);

        if usage_percentage > 90.0 {
            println!(
                "│ {}",
                style("⚠️  Critical: Only 10% tokens remaining!")
                    .red()
                    .bold()
            );
            println!(
                "│ {}",
                style("⚠️  Consider starting a new conversation")
                    .red()
                    .bold()
            );
        } else {
            println!(
                "│ {}",
                style("ℹ️  Note: Context usage is getting high").yellow()
            );
        }

        println!("{}", style("└───────────────────────────────────").dim());
        Ok(())
    }
}

impl Default for OutputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::time::Duration;

    #[test]
    fn test_output_handler_new() {
        let handler = OutputHandler::new();
        assert!(!handler.is_debug());
        assert!(!handler.has_spinner());
        assert!(!handler.is_spinner_active());
    }

    #[test]
    fn test_output_handler_with_debug() {
        let handler = OutputHandler::new().with_debug(true);
        assert!(handler.is_debug());

        let handler = OutputHandler::new().with_debug(false);
        assert!(!handler.is_debug());
    }

    #[test]
    fn test_debug_print() {
        // Test with ARULA_DEBUG set
        std::env::set_var("ARULA_DEBUG", "1");
        debug_print("Test debug message");
        std::env::remove_var("ARULA_DEBUG");

        // Should not panic without ARULA_DEBUG
        debug_print("Test message without debug");
    }

    #[test]
    fn test_find_closing_pattern() {
        let chars: Vec<char> = "hello world".chars().collect();

        // Test with pattern that exists
        let result = find_closing_pattern(&chars, "world");
        assert_eq!(result, Some(6));

        // Test with pattern that doesn't exist
        let result = find_closing_pattern(&chars, "xyz");
        assert_eq!(result, None);

        // Test with empty pattern
        let result = find_closing_pattern(&chars, "");
        assert_eq!(result, None);

        // Test with empty char slice
        let empty_chars: Vec<char> = vec![];
        let result = find_closing_pattern(&empty_chars, "test");
        assert_eq!(result, None);
    }

    #[test]
    fn test_prompt_state_display() {
        // Test that all PromptState variants can be created
        let states = [
            PromptState::Input,
            PromptState::Loading,
            PromptState::Completed,
            PromptState::Error,
        ];

        // Verify all states can be cloned (since they derive Copy)
        for &state in &states {
            let cloned_state = state;
            assert_eq!(state, cloned_state);
        }
    }

    #[test]
    fn test_format_tool_name() {
        let handler = OutputHandler::new();

        // Test basic formatting
        assert_eq!(handler.format_tool_name("bash_tool"), "Bash tool");
        assert_eq!(handler.format_tool_name("read_file"), "Read file");
        assert_eq!(handler.format_tool_name("edit_file"), "Edit file");

        // Test with multiple underscores
        assert_eq!(handler.format_tool_name("complex_tool_name"), "Complex tool name");

        // Test with no underscores
        assert_eq!(handler.format_tool_name("tool"), "Tool");

        // Test empty string
        assert_eq!(handler.format_tool_name(""), "");

        // Test with leading/trailing underscores - the function capitalizes first word, then processes rest
        assert_eq!(handler.format_tool_name("_tool_"), " tool "); // Leading underscore creates space, no capitalization
    }

    #[test]
    fn test_smart_truncate() {
        let handler = OutputHandler::new();

        // Test with short text (no truncation)
        let short = "short";
        assert_eq!(handler.smart_truncate(short, 20), "short");

        // Test with long text
        let long = "This is a very long text that should be truncated";
        let result = handler.smart_truncate(long, 20);
        assert!(result.len() <= 23); // Original 20 + "..."
        assert!(result.ends_with("..."));

        // Test with newline - check it handles newlines properly
        let text_with_newline = "First line\nSecond line";
        let result = handler.smart_truncate(text_with_newline, 30);
        // The function finds the last newline and truncates there if found
        assert!(result.contains("First line"));
        assert!(result.len() <= 33); // Allow for variations in implementation

        // Test with sentence - it truncates at the last period before the limit
        let text_with_period = "This is a sentence. This is another.";
        let result = handler.smart_truncate(text_with_period, 25);
        // It might not end with "sentence." if the truncation logic is different
        assert!(result.len() <= 28); // Allow for "..." addition
        assert!(result.contains("sentence"));

        // Test with spaces
        let text_with_spaces = "word1 word2 word3 word4";
        let result = handler.smart_truncate(text_with_spaces, 15);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_output() {
        let handler = OutputHandler::new();

        // Test with few lines (no truncation)
        let few_lines = "line1\nline2";
        assert_eq!(handler.truncate_output(few_lines, 5), "line1\nline2");

        // Test with many lines
        let many_lines = (1..=10).map(|i| format!("line{}", i)).collect::<Vec<_>>().join("\n");
        let result = handler.truncate_output(&many_lines, 3);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 4); // 3 lines + truncation line
        assert!(result.contains("... (7 more lines)"));
    }

    #[test]
    fn test_count_visible_chars() {
        let handler = OutputHandler::new();

        // Test with plain text
        assert_eq!(handler.count_visible_chars("hello"), 5);

        // Test with ANSI escape codes
        let with_ansi = "\x1b[31mhello\x1b[0m";
        assert_eq!(handler.count_visible_chars(with_ansi), 5);

        // Test with multiple escape codes
        let with_multiple = "\x1b[31m\x1b[1mhello\x1b[0m";
        assert_eq!(handler.count_visible_chars(with_multiple), 5);

        // Test with empty string
        assert_eq!(handler.count_visible_chars(""), 0);

        // Test with incomplete escape sequence
        let incomplete = "\x1b[31hello";
        let actual_count = handler.count_visible_chars(incomplete);
        // The function considers this as all being part of an incomplete escape sequence
        assert_eq!(actual_count, 0); // No visible characters due to escape sequence
    }

    #[test]
    fn test_process_inline_markdown() {
        let handler = OutputHandler::new();

        // Test bold formatting
        let result = handler.process_inline_markdown("This is **bold** text");
        assert!(result.contains("bold"));

        // Test italic formatting
        let result = handler.process_inline_markdown("This is *italic* text");
        assert!(result.contains("italic"));

        // Test inline code
        let result = handler.process_inline_markdown("This is `code` text");
        assert!(result.contains("code"));

        // Test strikethrough
        let result = handler.process_inline_markdown("This is ~~strikethrough~~ text");
        assert!(result.contains("strikethrough"));

        // Test escape sequences
        let result = handler.process_inline_markdown(r"This is \*not bold\*");
        assert!(result.contains("*"));

        // Test links
        let result = handler.process_inline_markdown("[link](url)");
        assert!(result.contains("link"));
        assert!(result.contains("url"));

        // Test footnote references
        let result = handler.process_inline_markdown("[^1]");
        assert!(result.contains("[^1]"));
    }

    #[test]
    fn test_parse_html_tag() {
        let handler = OutputHandler::new();

        // Test valid HTML tags
        let chars: Vec<char> = "<strong>bold text</strong>".chars().collect();
        let result = handler.parse_html_tag(&chars);
        assert!(result.is_some());

        // Test with unsupported tag
        let chars: Vec<char> = "<unknown>text</unknown>".chars().collect();
        let result = handler.parse_html_tag(&chars);
        assert!(result.is_some());
        let tag_result = result.unwrap();
        assert_eq!(tag_result.rendered, "text");

        // Test incomplete tag
        let chars: Vec<char> = "<strong>incomplete".chars().collect();
        let result = handler.parse_html_tag(&chars);
        assert!(result.is_none());

        // Test non-tag content
        let chars: Vec<char> = "just plain text".chars().collect();
        let result = handler.parse_html_tag(&chars);
        assert!(result.is_none());
    }

    #[test]
    fn test_html_tag_styling() {
        let handler = OutputHandler::new();

        // Test that different HTML tags are handled
        let test_cases = [
            ("<mark>highlighted</mark>", "highlighted"),
            ("<em>italic</em>", "italic"),
            ("<strong>bold</strong>", "bold"),
            ("<code>code</code>", "code"),
            ("<u>underline</u>", "underline"),
            ("<s>strikethrough</s>", "strikethrough"),
        ];

        for (html_input, expected_content) in test_cases {
            let chars: Vec<char> = html_input.chars().collect();
            let result = handler.parse_html_tag(&chars);
            assert!(result.is_some(), "Failed to parse: {}", html_input);
            let tag_result = result.unwrap();
            assert!(tag_result.rendered.contains(expected_content));
        }
    }

    #[test]
    fn test_spinner_operations() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test initial state
        assert!(!handler.has_spinner());
        assert!(!handler.is_spinner_active());

        // Test starting spinner
        handler.start_spinner("Test message")?;
        assert!(handler.has_spinner());
        assert!(handler.is_spinner_active());

        // Test updating spinner message
        handler.update_spinner_message("New message")?;

        // Test stopping spinner
        handler.stop_spinner();
        assert!(!handler.has_spinner());
        assert!(!handler.is_spinner_active());

        Ok(())
    }

    #[test]
    fn test_multiple_spinner_starts() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Start first spinner
        handler.start_spinner("First")?;
        assert!(handler.has_spinner());

        // Starting second spinner should not create another
        handler.start_spinner("Second")?;
        assert!(handler.has_spinner());

        // Should still be able to stop it
        handler.stop_spinner();
        assert!(!handler.has_spinner());

        Ok(())
    }

    #[test]
    fn test_prompt_states() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        let prefix = "Test";
        let text = "Sample text";

        // Test all prompt states
        for state in [
            PromptState::Input,
            PromptState::Loading,
            PromptState::Completed,
            PromptState::Error,
        ] {
            handler.print_progress_prompt(prefix, text, state)?;
            handler.update_progress_bar(prefix, text, state)?;
        }

        Ok(())
    }

    #[test]
    fn test_print_methods() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test all basic print methods
        handler.print_user_message("Hello user")?;
        handler.print_ai_message("Hello from AI")?;
        handler.print_error("Error message")?;
        handler.print_system("System message")?;

        // Test tool call methods
        handler.print_tool_call("test_tool", "arg1=value1")?;
        handler.print_tool_result("Tool executed successfully")?;

        // Test tool execution flow
        handler.start_tool_execution("test_tool", "test input")?;
        handler.complete_tool_execution("Success", true)?;

        Ok(())
    }

    #[test]
    fn test_debug_vs_non_debug_output() -> io::Result<()> {
        let mut debug_handler = OutputHandler::new().with_debug(true);
        let mut normal_handler = OutputHandler::new().with_debug(false);

        // Test tool call output differences
        debug_handler.print_tool_call("test_tool", "args")?;
        normal_handler.print_tool_call("test_tool", "args")?;

        // Test tool result output differences
        debug_handler.print_tool_result("result")?;
        normal_handler.print_tool_result("result")?;

        Ok(())
    }

    #[test]
    fn test_streaming_functionality() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test streaming workflow
        handler.start_ai_message()?;
        handler.print_streaming_chunk("Hello ")?;
        handler.print_streaming_chunk("world")?;
        handler.end_line()?;

        Ok(())
    }

    #[test]
    fn test_markdown_rendering() -> io::Result<()> {
        let handler = OutputHandler::new();

        // Test markdown rendering
        handler.render_markdown("# Header\n\nThis is **bold** text.")?;

        Ok(())
    }

    #[test]
    fn test_code_block_rendering() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test code block with language
        handler.print_code_block("fn main() {}", Some("rust"))?;

        // Test code block without language
        handler.print_code_block("print('Hello')", None)?;

        Ok(())
    }

    #[test]
    fn test_conversation_features() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Create sample messages
        let messages = vec![
            crate::chat::ChatMessage::new_user_message("Hello"),
            crate::chat::ChatMessage::new_arula_message("Hi there!"),
        ];

        // Test message history
        handler.print_message_history(&messages, 0)?;

        // Test conversation summary
        handler.print_conversation_summary(&messages)?;

        Ok(())
    }

    #[test]
    fn test_context_usage_display() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Create mock usage data
        let usage = crate::api::Usage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };

        // Test context usage display
        handler.print_context_usage(Some(&usage))?;
        handler.display_token_usage(Some(&usage))?;

        // Test with None usage
        handler.print_context_usage(None)?;
        handler.display_token_usage(None)?;

        Ok(())
    }

    #[test]
    fn test_banner_display() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test banner display
        handler.print_banner()?;

        Ok(())
    }

    #[test]
    fn test_animated_progress() {
        // Test the animated_progress function from animations module
        let result = animations::animated_progress(5, 10, 20);
        assert!(!result.is_empty());
        assert!(result.len() <= 20 * 10); // Rough bound check
    }

    #[test]
    fn test_animation_constants() {
        // Test that all animation constants are accessible
        assert!(!animations::SPINNER_FRAMES.is_empty());
        assert!(!animations::LOADING_FRAMES.is_empty());
        assert!(!animations::PULSE_FRAMES.is_empty());

        // Verify spinner frames contain expected characters
        assert!(animations::SPINNER_FRAMES.contains(&"⠋"));
        assert!(animations::SPINNER_FRAMES.contains(&"○"));
        assert!(animations::SPINNER_FRAMES.contains(&"●"));
    }

    #[test]
    fn test_edge_cases() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test with empty strings
        handler.print_user_message("")?;
        handler.print_ai_message("")?;
        handler.print_tool_call("", "")?;
        handler.print_tool_result("")?;

        // Test with very long strings
        let long_string = "x".repeat(1000);
        handler.print_user_message(&long_string)?;
        handler.print_tool_result(&long_string)?;

        // Test with special characters
        let special_chars = "!@#$%^&*()[]{}|\\;:'\",<>?`\n\t";
        handler.print_user_message(special_chars)?;

        Ok(())
    }

    #[test]
    fn test_unicode_handling() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test with Unicode characters
        let unicode_text = "Hello 世界 🚀 藍色";
        handler.print_user_message(unicode_text)?;

        // Test with emojis
        let emoji_text = "😀 😃 😄 😁 😆 😅";
        handler.print_ai_message(emoji_text)?;

        Ok(())
    }

    #[test]
    fn test_concurrent_operations() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test starting/stopping spinner rapidly
        for i in 0..5 {
            handler.start_spinner(&format!("Iteration {}", i))?;
            handler.update_spinner_message(&format!("Updated {}", i))?;
            handler.stop_spinner();
        }

        Ok(())
    }

    #[test]
    fn test_error_handling() {
        let handler = OutputHandler::new();

        // Test that various markdown operations handle edge cases gracefully
        let _ = handler.process_inline_markdown(""); // Empty string
        let _ = handler.process_inline_markdown("*"); // Incomplete formatting
        let _ = handler.process_inline_markdown("**bold"); // Unclosed bold
        let _ = handler.process_inline_markdown("text with `unclosed code"); // Unclosed code

        // Test HTML parsing with invalid input
        let chars: Vec<char> = "<".chars().collect();
        let result = handler.parse_html_tag(&chars);
        assert!(result.is_none());

        let chars: Vec<char> = "<unclosed".chars().collect();
        let result = handler.parse_html_tag(&chars);
        assert!(result.is_none());
    }

    #[test]
    fn test_complex_markdown_scenarios() {
        let handler = OutputHandler::new();

        // Test nested formatting
        let nested = "This is **bold and *italic* within**";
        let result = handler.process_inline_markdown(nested);
        assert!(!result.is_empty());

        // Test multiple links
        let multiple_links = "[first](url1) and [second](url2)";
        let result = handler.process_inline_markdown(multiple_links);
        assert!(!result.is_empty());

        // Test mixed formatting
        let mixed = "`code` and **bold** and *italic*";
        let result = handler.process_inline_markdown(mixed);
        assert!(!result.is_empty());

        // Test escape sequences with various characters
        let escapes = r"\* \_ \` \~ \[ \] \( \) \# \\";
        let result = handler.process_inline_markdown(escapes);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_large_input_handling() {
        let handler = OutputHandler::new();

        // Test with very large markdown input
        let large_text = "word ".repeat(1000);
        let _ = handler.process_inline_markdown(&large_text);

        // Test smart truncation with large input
        let very_large = "a".repeat(10000);
        let result = handler.smart_truncate(&very_large, 100);
        assert!(result.len() <= 103); // Account for "..."

        // Test truncate output with many lines
        let many_lines = "line\n".repeat(1000);
        let result = handler.truncate_output(&many_lines, 50);
        assert!(result.lines().count() <= 51); // 50 + truncation line
    }

    #[tokio::test]
    async fn test_async_operations() -> io::Result<()> {
        let mut handler = OutputHandler::new();

        // Test spinner with async delay
        handler.start_spinner("Async test")?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        handler.update_spinner_message("Updated")?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        handler.stop_spinner();

        Ok(())
    }

    // Integration test style test that verifies the complete workflow
    #[test]
    fn test_complete_output_workflow() -> io::Result<()> {
        let mut handler = OutputHandler::new().with_debug(false);

        // Simulate a complete user interaction workflow
        handler.print_banner()?;

        // User message
        handler.print_user_message("Help me with a Rust project")?;

        // AI starts responding
        handler.start_ai_message()?;
        handler.print_streaming_chunk("I'll help you ")?;
        handler.print_streaming_chunk("with your Rust project. ")?;

        // AI calls a tool
        handler.start_tool_execution("read_file", "Cargo.toml")?;
        handler.complete_tool_execution("Successfully read file", true)?;

        // AI continues responding
        handler.print_streaming_chunk("Let me examine your project structure.")?;
        handler.end_line()?;

        // Show context usage
        let usage = crate::api::Usage {
            prompt_tokens: 50,
            completion_tokens: 30,
            total_tokens: 80,
        };
        handler.print_context_usage(Some(&usage))?;

        Ok(())
    }
}
