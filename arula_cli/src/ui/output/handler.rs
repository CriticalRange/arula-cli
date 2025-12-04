//! Main output handler - facade for all output operations
//!
//! This is the primary interface for terminal output in ARULA CLI.
//! It provides a unified API for all output operations.

use super::code_blocks::CodeHighlighter;
use super::markdown::MarkdownStreamer;
use super::spinners::{SpinnerManager, SpinnerStyle};
use super::tool_display;
use crate::api::api::Usage;
use console::style;
use crossterm::terminal;
use std::io::{self, Write};

/// Main output handler for ARULA CLI
///
/// Provides a unified interface for:
/// - AI response streaming with markdown rendering
/// - Code block syntax highlighting
/// - Tool call/result display
/// - Progress indicators and spinners
/// - Banner and status messages
///
/// # Example
///
/// ```rust,ignore
/// let mut output = OutputHandler::new().with_debug(true);
/// output.print_banner()?;
/// output.start_ai_stream()?;
/// output.stream_chunk("Hello, world!")?;
/// output.finalize_stream()?;
/// ```
pub struct OutputHandler {
    /// Debug mode flag
    debug: bool,
    /// Markdown streamer for AI responses
    markdown_streamer: MarkdownStreamer,
    /// Code highlighter
    code_highlighter: CodeHighlighter,
    /// Spinner manager
    spinner_manager: SpinnerManager,
    /// Whether we're currently streaming AI output
    streaming: bool,
    /// Current stream content buffer
    stream_buffer: String,
}

impl OutputHandler {
    /// Create a new output handler with default settings
    pub fn new() -> Self {
        Self {
            debug: false,
            markdown_streamer: MarkdownStreamer::new(),
            code_highlighter: CodeHighlighter::default_theme(),
            spinner_manager: SpinnerManager::new(),
            streaming: false,
            stream_buffer: String::new(),
        }
    }

    /// Builder method to set debug mode
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Get terminal width
    pub fn terminal_width(&self) -> usize {
        terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(super::DEFAULT_TERMINAL_WIDTH)
    }

    // ========================================================================
    // Banner and Status
    // ========================================================================

    /// Print the ARULA banner
    pub fn print_banner(&self) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        writeln!(handle, "\n{}", style("╔══════════════════════════════════════════════╗").cyan())?;
        writeln!(handle, "{}", style("║                                              ║").cyan())?;
        writeln!(handle, "{}{}{}",
            style("║").cyan(),
            style("   █████╗ ██████╗ ██╗   ██╗██╗      █████╗   ").bright().cyan(),
            style("║").cyan()
        )?;
        writeln!(handle, "{}{}{}",
            style("║").cyan(),
            style("  ██╔══██╗██╔══██╗██║   ██║██║     ██╔══██╗  ").bright().cyan(),
            style("║").cyan()
        )?;
        writeln!(handle, "{}{}{}",
            style("║").cyan(),
            style("  ███████║██████╔╝██║   ██║██║     ███████║  ").bright().cyan(),
            style("║").cyan()
        )?;
        writeln!(handle, "{}{}{}",
            style("║").cyan(),
            style("  ██╔══██║██╔══██╗██║   ██║██║     ██╔══██║  ").bright().cyan(),
            style("║").cyan()
        )?;
        writeln!(handle, "{}{}{}",
            style("║").cyan(),
            style("  ██║  ██║██║  ██║╚██████╔╝███████╗██║  ██║  ").bright().cyan(),
            style("║").cyan()
        )?;
        writeln!(handle, "{}{}{}",
            style("║").cyan(),
            style("  ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝  ").bright().cyan(),
            style("║").cyan()
        )?;
        writeln!(handle, "{}", style("║                                              ║").cyan())?;
        writeln!(handle, "{}{}{}",
            style("║").cyan(),
            style("        Autonomous AI CLI Assistant          ").dim(),
            style("║").cyan()
        )?;
        writeln!(handle, "{}", style("╚══════════════════════════════════════════════╝").cyan())?;
        writeln!(handle)?;

        handle.flush()
    }

    /// Print a system notification
    pub fn print_system(&self, message: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        writeln!(handle, "{} {}", style("ℹ").blue(), style(message).dim())?;
        handle.flush()
    }

    /// Print a success message
    pub fn print_success(&self, message: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        writeln!(handle, "{} {}", style("✓").green(), style(message).green())?;
        handle.flush()
    }

    /// Print an error message
    pub fn print_error(&self, message: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        writeln!(handle, "{} {}", style("✗").red(), style(message).red())?;
        handle.flush()
    }

    /// Print a warning message
    pub fn print_warning(&self, message: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        writeln!(handle, "{} {}", style("⚠").yellow(), style(message).yellow())?;
        handle.flush()
    }

    // ========================================================================
    // User/AI Message Display
    // ========================================================================

    /// Print a user message
    pub fn print_user_message(&self, message: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        writeln!(handle, "\n{} {}", style("You:").bold().green(), message)?;
        handle.flush()
    }

    /// Start AI response streaming
    pub fn start_ai_stream(&mut self) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        // Just add a newline to separate from previous content
        writeln!(handle)?;
        handle.flush()?;

        self.streaming = true;
        self.stream_buffer.clear();
        self.markdown_streamer.reset();

        Ok(())
    }

    /// Stream a chunk of AI response
    pub fn stream_chunk(&mut self, chunk: &str) -> io::Result<()> {
        if !self.streaming {
            self.start_ai_stream()?;
        }

        self.stream_buffer.push_str(chunk);
        self.markdown_streamer.process_chunk(chunk)?;

        Ok(())
    }

    /// Finalize AI response streaming
    pub fn finalize_stream(&mut self) -> io::Result<()> {
        self.markdown_streamer.finalize()?;
        self.streaming = false;

        let stdout = io::stdout();
        let mut handle = stdout.lock();
        writeln!(handle)?;
        handle.flush()
    }

    /// Stream AI response with markdown processing
    ///
    /// This method is an alias for `stream_chunk()` for backward compatibility.
    /// Processes streaming chunks with proper markdown rendering.
    pub fn stream_ai_response(&mut self, chunk: &str) -> io::Result<()> {
        self.stream_chunk(chunk)
    }

    /// Print a complete AI message (non-streaming)
    pub fn print_ai_message(&self, message: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        writeln!(handle, "\n{} {}", style("AI:").bold().cyan(), message)?;
        handle.flush()
    }

    // ========================================================================
    // Tool Call Display
    // ========================================================================

    /// Print a tool call notification
    pub fn print_tool_call(&self, tool_name: &str, arguments: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        let formatted = tool_display::format_tool_call_box(tool_name, arguments);
        // Add extra spacing before tool calls for better readability
        writeln!(handle, "\n\n{}", formatted)?;
        handle.flush()
    }

    /// Print a tool result
    pub fn print_tool_result(&self, tool_name: &str, result: &serde_json::Value, success: bool) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        let formatted = tool_display::format_tool_result_box(tool_name, result, success);
        writeln!(handle, "{}", formatted)?;
        handle.flush()
    }

    /// Print detailed tool result (verbose mode)
    pub fn print_tool_result_detailed(&self, tool_name: &str, result: &serde_json::Value, success: bool) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        let formatted = tool_display::format_detailed_result(tool_name, result, success);
        writeln!(handle, "{}", formatted)?;
        handle.flush()
    }

    // ========================================================================
    // Code Display
    // ========================================================================

    /// Print a code block with syntax highlighting
    pub fn print_code(&self, code: &str, language: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        let highlighted = self.code_highlighter.highlight(code, language);
        writeln!(handle, "{}", highlighted)?;
        handle.flush()
    }

    /// Print a code block with a box border
    pub fn print_code_box(&self, code: &str, language: &str) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        let formatted = super::code_blocks::format_code_box(code, language, self.terminal_width());
        write!(handle, "{}", formatted)?;
        handle.flush()
    }

    // ========================================================================
    // Spinners and Progress
    // ========================================================================

    /// Start a spinner with a message
    pub fn start_spinner(&mut self, style: SpinnerStyle, message: &str) {
        self.spinner_manager.start(style, message);
    }

    /// Start a simple thinking spinner
    pub fn start_thinking(&mut self, message: &str) {
        self.spinner_manager.start(SpinnerStyle::Thinking, message);
    }

    /// Start a tool execution spinner
    pub fn start_tool_spinner(&mut self, message: &str) {
        self.spinner_manager.start(SpinnerStyle::ToolExecution, message);
    }

    /// Update spinner message
    pub fn update_spinner(&mut self, message: &str) {
        self.spinner_manager.set_message(message);
    }

    /// Stop spinner with completion message
    pub fn finish_spinner(&mut self, message: &str) {
        self.spinner_manager.finish(message);
    }

    /// Stop and clear spinner
    pub fn clear_spinner(&mut self) {
        self.spinner_manager.stop();
    }

    // ========================================================================
    // Usage Statistics
    // ========================================================================

    /// Print API usage statistics
    pub fn print_usage(&self, usage: &Usage) -> io::Result<()> {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        writeln!(handle, "\n{}", style("─".repeat(40)).dim())?;
        writeln!(handle, "{}", style("Usage Statistics:").dim())?;
        writeln!(handle, "  {} Prompt tokens: {}",
            style("•").dim(),
            style(usage.prompt_tokens).cyan()
        )?;
        writeln!(handle, "  {} Completion tokens: {}",
            style("•").dim(),
            style(usage.completion_tokens).cyan()
        )?;
        writeln!(handle, "  {} Total tokens: {}",
            style("•").dim(),
            style(usage.total_tokens).bold().cyan()
        )?;
        writeln!(handle, "{}", style("─".repeat(40)).dim())?;

        handle.flush()
    }

    // ========================================================================
    // Debug Output
    // ========================================================================

    /// Print debug message (only if debug mode enabled)
    pub fn debug(&self, message: &str) {
        if self.debug {
            println!("{} {}", style("🔧 DEBUG:").dim(), message);
        }
    }

    /// Check if debug mode is enabled
    pub fn is_debug(&self) -> bool {
        self.debug
    }

    /// Set debug mode
    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
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

    #[test]
    fn test_output_handler_creation() {
        let handler = OutputHandler::new();
        assert!(!handler.is_debug());

        let handler = OutputHandler::new().with_debug(true);
        assert!(handler.is_debug());
    }

    #[test]
    fn test_terminal_width() {
        let handler = OutputHandler::new();
        let width = handler.terminal_width();
        assert!(width > 0);
    }
}

