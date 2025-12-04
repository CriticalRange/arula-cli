//! Code block syntax highlighting with lazy-loaded resources
//!
//! Uses `OnceLock` for efficient lazy initialization of expensive resources.
//! Per Tokio best practices, this avoids blocking the async runtime during
//! resource loading.

use std::sync::OnceLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
    util::as_24_bit_terminal_escaped,
};

/// Lazy-initialized syntax set (expensive to create, ~5MB)
static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

/// Lazy-initialized theme set
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

/// Get the global syntax set, loading defaults on first access
///
/// This function is thread-safe and will only initialize once.
///
/// # Example
///
/// ```rust,ignore
/// let syntax_set = get_syntax_set();
/// let rust_syntax = syntax_set.find_syntax_by_extension("rs");
/// ```
#[inline]
pub fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Get the global theme set, loading defaults on first access
///
/// # Available Themes
///
/// - `base16-ocean.dark` (default)
/// - `base16-eighties.dark`
/// - `InspiredGitHub`
/// - `Solarized (dark)`
/// - `Solarized (light)`
#[inline]
pub fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// Default theme name for syntax highlighting
pub const DEFAULT_THEME: &str = "base16-ocean.dark";

/// Code highlighter with caching and efficient rendering
pub struct CodeHighlighter {
    theme_name: String,
}

impl CodeHighlighter {
    /// Create a new code highlighter with the specified theme
    pub fn new(theme_name: &str) -> Self {
        Self {
            theme_name: theme_name.to_string(),
        }
    }

    /// Create a code highlighter with the default theme
    pub fn default_theme() -> Self {
        Self::new(DEFAULT_THEME)
    }

    /// Get the current theme
    pub fn get_theme(&self) -> &Theme {
        let theme_set = get_theme_set();
        theme_set
            .themes
            .get(&self.theme_name)
            .unwrap_or_else(|| theme_set.themes.get(DEFAULT_THEME).unwrap())
    }

    /// Highlight code and return terminal-escaped string
    ///
    /// # Arguments
    ///
    /// * `code` - The source code to highlight
    /// * `language` - Language extension (e.g., "rs", "py", "js")
    ///
    /// # Returns
    ///
    /// Terminal-escaped string with 24-bit ANSI colors
    pub fn highlight(&self, code: &str, language: &str) -> String {
        let syntax_set = get_syntax_set();
        let theme = self.get_theme();

        // Find syntax for language, fallback to plain text
        let syntax = syntax_set
            .find_syntax_by_extension(language)
            .or_else(|| syntax_set.find_syntax_by_name(language))
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut output = String::with_capacity(code.len() * 2);

        for line in code.lines() {
            match highlighter.highlight_line(line, syntax_set) {
                Ok(ranges) => {
                    output.push_str(&as_24_bit_terminal_escaped(&ranges[..], true));
                    output.push('\n');
                }
                Err(_) => {
                    // Fallback to plain text on error
                    output.push_str(line);
                    output.push('\n');
                }
            }
        }

        // Reset terminal colors at end
        output.push_str("\x1b[0m");
        output
    }

    /// Highlight a single line (for streaming)
    pub fn highlight_line(&self, line: &str, language: &str) -> String {
        let syntax_set = get_syntax_set();
        let theme = self.get_theme();

        let syntax = syntax_set
            .find_syntax_by_extension(language)
            .or_else(|| syntax_set.find_syntax_by_name(language))
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, theme);

        match highlighter.highlight_line(line, syntax_set) {
            Ok(ranges) => {
                let mut output = as_24_bit_terminal_escaped(&ranges[..], true);
                output.push_str("\x1b[0m");
                output
            }
            Err(_) => line.to_string(),
        }
    }

    /// Get a list of supported language extensions
    pub fn supported_languages() -> Vec<&'static str> {
        let syntax_set = get_syntax_set();
        syntax_set
            .syntaxes()
            .iter()
            .flat_map(|s| s.file_extensions.iter().map(|e| e.as_str()))
            .collect()
    }

    /// Check if a language is supported
    pub fn is_supported(language: &str) -> bool {
        let syntax_set = get_syntax_set();
        syntax_set.find_syntax_by_extension(language).is_some()
            || syntax_set.find_syntax_by_name(language).is_some()
    }
}

impl Default for CodeHighlighter {
    fn default() -> Self {
        Self::default_theme()
    }
}

/// Format code block with box drawing characters
///
/// Creates a bordered code block suitable for terminal display.
///
/// # Example Output
///
/// ```text
/// ┌─ rust ──────────────────────────────────┐
/// │ fn main() {                              │
/// │     println!("Hello, World!");           │
/// │ }                                        │
/// └──────────────────────────────────────────┘
/// ```
pub fn format_code_box(code: &str, language: &str, width: usize) -> String {
    let highlighter = CodeHighlighter::default_theme();
    let highlighted = highlighter.highlight(code, language);

    let inner_width = width.saturating_sub(4); // Account for borders
    let lang_label = if language.is_empty() {
        "code"
    } else {
        language
    };

    let mut output = String::with_capacity(code.len() * 3);

    // Top border with language label
    output.push_str(&format!(
        "\x1b[90m┌─ {} {}\x1b[0m\n",
        lang_label,
        "─".repeat(inner_width.saturating_sub(lang_label.len() + 3))
    ));

    // Code lines with side borders
    for line in highlighted.lines() {
        output.push_str("\x1b[90m│\x1b[0m ");
        output.push_str(line);
        output.push('\n');
    }

    // Bottom border
    output.push_str(&format!(
        "\x1b[90m└{}\x1b[0m\n",
        "─".repeat(inner_width + 2)
    ));

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_syntax_set_loading() {
        let ss = get_syntax_set();
        assert!(ss.find_syntax_by_extension("rs").is_some());
        assert!(ss.find_syntax_by_extension("py").is_some());
    }

    #[test]
    fn test_lazy_theme_set_loading() {
        let ts = get_theme_set();
        assert!(ts.themes.contains_key("base16-ocean.dark"));
    }

    #[test]
    fn test_highlighter_rust() {
        let highlighter = CodeHighlighter::default_theme();
        let code = "fn main() { println!(\"Hello\"); }";
        let highlighted = highlighter.highlight(code, "rs");

        // Should contain ANSI escape codes
        assert!(highlighted.contains("\x1b["));
        // Should end with reset
        assert!(highlighted.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_highlighter_unknown_language() {
        let highlighter = CodeHighlighter::default_theme();
        let code = "some random text";
        let highlighted = highlighter.highlight(code, "nonexistent");

        // Should still produce output (plain text fallback)
        assert!(!highlighted.is_empty());
    }

    #[test]
    fn test_supported_languages() {
        let langs = CodeHighlighter::supported_languages();
        assert!(langs.contains(&"rs"));
        assert!(langs.contains(&"py"));
        assert!(langs.contains(&"js"));
    }

    #[test]
    fn test_is_supported() {
        // Test by extension
        assert!(CodeHighlighter::is_supported("rs"));
        // Test by name (syntect uses "Rust" not "rust")
        assert!(CodeHighlighter::is_supported("Rust"));
        // Test unsupported
        assert!(!CodeHighlighter::is_supported("definitelynotreal"));
    }
}
