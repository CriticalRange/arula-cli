//! Spinner and progress bar utilities using indicatif
//!
//! Provides pre-configured spinners and progress bars for various operations.

use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Pre-defined spinner styles for different use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerStyle {
    /// Default AI thinking spinner (dots)
    Thinking,
    /// Tool execution spinner (gear)
    ToolExecution,
    /// File operation spinner (folder)
    FileOperation,
    /// Network request spinner (arrows)
    Network,
    /// Search operation spinner (magnifying glass)
    Search,
    /// Custom orbital spinner (for main AI responses)
    Orbital,
}

impl SpinnerStyle {
    /// Get the tick characters for this spinner style
    pub fn tick_chars(&self) -> &'static str {
        match self {
            Self::Thinking => "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏",
            Self::ToolExecution => "⣾⣽⣻⢿⡿⣟⣯⣷",
            Self::FileOperation => "📂📁📂📁📂📁",
            Self::Network => "◐◓◑◒",
            Self::Search => "🔍🔎🔍🔎",
            Self::Orbital => "◜◠◝◞◡◟",
        }
    }

    /// Get the icon for this spinner style
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Thinking => "🤔",
            Self::ToolExecution => "⚙️",
            Self::FileOperation => "📂",
            Self::Network => "🌐",
            Self::Search => "🔍",
            Self::Orbital => "✨",
        }
    }

    /// Get the default message prefix
    pub fn default_message(&self) -> &'static str {
        match self {
            Self::Thinking => "Thinking",
            Self::ToolExecution => "Executing",
            Self::FileOperation => "Processing",
            Self::Network => "Connecting",
            Self::Search => "Searching",
            Self::Orbital => "Processing",
        }
    }
}

/// Create a spinner with the specified style and message
///
/// # Example
///
/// ```rust,ignore
/// use arula_cli::ui::output::spinners::{create_spinner, SpinnerStyle};
///
/// let spinner = create_spinner(SpinnerStyle::Thinking, "Processing your request...");
/// // ... do work ...
/// spinner.finish_with_message("Done!");
/// ```
pub fn create_spinner(style: SpinnerStyle, message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();

    let template = format!(
        "{{spinner:.cyan}} {} {{msg}}",
        style.icon()
    );

    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars(style.tick_chars())
            .template(&template)
            .expect("Invalid spinner template")
    );

    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(Duration::from_millis(80));

    spinner
}

/// Create a simple spinner with just a message
pub fn create_simple_spinner(message: &str) -> ProgressBar {
    create_spinner(SpinnerStyle::Thinking, message)
}

/// Create a progress bar for operations with known length
///
/// # Example
///
/// ```rust,ignore
/// let progress = create_progress_bar(100, "Processing files");
/// for i in 0..100 {
///     // do work
///     progress.inc(1);
/// }
/// progress.finish_with_message("Complete!");
/// ```
pub fn create_progress_bar(total: u64, message: &str) -> ProgressBar {
    let bar = ProgressBar::new(total);

    bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .expect("Invalid progress bar template")
            .progress_chars("█▓░")
    );

    bar.set_message(message.to_string());
    bar.enable_steady_tick(Duration::from_millis(100));

    bar
}

/// Create a download-style progress bar with ETA
pub fn create_download_bar(total: u64, message: &str) -> ProgressBar {
    let bar = ProgressBar::new(total);

    bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} {msg}\n{wide_bar:.cyan/blue} {bytes}/{total_bytes} ({eta})")
            .expect("Invalid download bar template")
            .progress_chars("━━╸")
    );

    bar.set_message(message.to_string());
    bar.enable_steady_tick(Duration::from_millis(100));

    bar
}

/// Create an indeterminate progress bar (for unknown length operations)
pub fn create_indeterminate_bar(message: &str) -> ProgressBar {
    let bar = ProgressBar::new_spinner();

    bar.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("▰▰▰▰▰▱▱▱▱▱")
            .template("{spinner:.cyan} {msg}")
            .expect("Invalid indeterminate bar template")
    );

    bar.set_message(message.to_string());
    bar.enable_steady_tick(Duration::from_millis(100));

    bar
}

/// Spinner manager for handling multiple spinners
pub struct SpinnerManager {
    current: Option<ProgressBar>,
}

impl SpinnerManager {
    /// Create a new spinner manager
    pub fn new() -> Self {
        Self { current: None }
    }

    /// Start a new spinner, stopping any existing one
    pub fn start(&mut self, style: SpinnerStyle, message: &str) {
        self.stop();
        self.current = Some(create_spinner(style, message));
    }

    /// Update the current spinner's message
    pub fn set_message(&mut self, message: &str) {
        if let Some(ref spinner) = self.current {
            spinner.set_message(message.to_string());
        }
    }

    /// Stop the current spinner with a completion message
    pub fn finish(&mut self, message: &str) {
        if let Some(spinner) = self.current.take() {
            spinner.finish_with_message(message.to_string());
        }
    }

    /// Stop the current spinner and clear its output
    pub fn stop(&mut self) {
        if let Some(spinner) = self.current.take() {
            spinner.finish_and_clear();
        }
    }

    /// Check if a spinner is currently running
    pub fn is_running(&self) -> bool {
        self.current.is_some()
    }
}

impl Default for SpinnerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SpinnerManager {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_styles() {
        for style in [
            SpinnerStyle::Thinking,
            SpinnerStyle::ToolExecution,
            SpinnerStyle::FileOperation,
            SpinnerStyle::Network,
            SpinnerStyle::Search,
            SpinnerStyle::Orbital,
        ] {
            assert!(!style.tick_chars().is_empty());
            assert!(!style.icon().is_empty());
            assert!(!style.default_message().is_empty());
        }
    }

    #[test]
    fn test_create_spinner() {
        let spinner = create_spinner(SpinnerStyle::Thinking, "Test message");
        spinner.finish_and_clear();
    }

    #[test]
    fn test_spinner_manager() {
        let mut manager = SpinnerManager::new();
        assert!(!manager.is_running());

        manager.start(SpinnerStyle::Thinking, "Starting");
        assert!(manager.is_running());

        manager.set_message("Updated");
        assert!(manager.is_running());

        manager.stop();
        assert!(!manager.is_running());
    }
}

