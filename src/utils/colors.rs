//! Color constants and styling utilities for ARULA CLI
//! Defines the consistent color palette used throughout the application

use console::Style;

/// Primary color - Golden yellow (#E8C547)
pub const PRIMARY_HEX: &str = "#E8C547";
pub const PRIMARY_ANSI: u8 = 214; // ANSI 256 color approximation

/// Secondary color - Dark gray (#30323D)
pub const SECONDARY_HEX: &str = "#30323D";
pub const SECONDARY_ANSI: u8 = 236; // ANSI 256 color approximation

/// Background color - Medium gray (#4D5061)
pub const BACKGROUND_HEX: &str = "#4D5061";
pub const BACKGROUND_ANSI: u8 = 240; // ANSI 256 color approximation

/// AI message highlights - Steel blue (#5C80BC)
pub const AI_HIGHLIGHT_HEX: &str = "#5C80BC";
pub const AI_HIGHLIGHT_ANSI: u8 = 67; // ANSI 256 color approximation

/// Inline code and miscellaneous text - Light gray (#CDD1C4)
pub const MISC_HEX: &str = "#CDD1C4";
pub const MISC_ANSI: u8 = 251; // ANSI 256 color approximation

/// Color theme struct for consistent styling
pub struct ColorTheme;

impl ColorTheme {
    /// Primary golden yellow style
    pub fn primary() -> Style {
        Style::new().color256(PRIMARY_ANSI).bold()
    }

    /// Secondary dark gray style
    pub fn secondary() -> Style {
        Style::new().color256(SECONDARY_ANSI)
    }

    /// Background medium gray style
    pub fn background() -> Style {
        Style::new().color256(BACKGROUND_ANSI)
    }

    /// AI highlight steel blue style
    pub fn ai_highlight() -> Style {
        Style::new().color256(AI_HIGHLIGHT_ANSI).bold()
    }

    /// Misc light gray style
    pub fn misc() -> Style {
        Style::new().color256(MISC_ANSI)
    }

    /// Primary style with background
    pub fn primary_on_background() -> Style {
        Style::new().color256(PRIMARY_ANSI).on_color256(BACKGROUND_ANSI).bold()
    }

    /// Misc style with background for inline code
    pub fn inline_code() -> Style {
        Style::new().color256(MISC_ANSI).on_color256(SECONDARY_ANSI)
    }

    /// AI message style
    pub fn ai_message() -> Style {
        Style::new().color256(AI_HIGHLIGHT_ANSI).bold()
    }

    /// Success style (green variant)
    pub fn success() -> Style {
        Style::new().color256(46).bold() // Bright green
    }

    /// Error style (red variant)
    pub fn error() -> Style {
        Style::new().color256(196).bold() // Bright red
    }

    /// Warning style (orange variant)
    pub fn warning() -> Style {
        Style::new().color256(208).bold() // Orange
    }

    /// Dim/faded style
    pub fn dim() -> Style {
        Style::new().color256(244).dim() // Very light gray
    }

    /// Border/separator style
    pub fn border() -> Style {
        Style::new().color256(AI_HIGHLIGHT_ANSI).dim()
    }

    /// Cursor/selection style
    pub fn selection() -> Style {
        Style::new().color256(PRIMARY_ANSI).on_color256(SECONDARY_ANSI).bold()
    }
}

/// Color extension trait for console Style
pub trait ColorExt {
    /// Apply primary color
    fn primary(self) -> Style;
    /// Apply secondary color
    fn secondary(self) -> Style;
    /// Apply background color
    fn background(self) -> Style;
    /// Apply AI highlight color
    fn ai_highlight(self) -> Style;
    /// Apply misc color
    fn misc(self) -> Style;
    /// Apply inline code styling
    fn inline_code_style(self) -> Style;
}

impl ColorExt for Style {
    fn primary(self) -> Style {
        self.color256(PRIMARY_ANSI).bold()
    }

    fn secondary(self) -> Style {
        self.color256(SECONDARY_ANSI)
    }

    fn background(self) -> Style {
        self.color256(BACKGROUND_ANSI)
    }

    fn ai_highlight(self) -> Style {
        self.color256(AI_HIGHLIGHT_ANSI).bold()
    }

    fn misc(self) -> Style {
        self.color256(MISC_ANSI)
    }

    fn inline_code_style(self) -> Style {
        self.color256(MISC_ANSI).on_color256(SECONDARY_ANSI)
    }
}

/// Helper functions for common color patterns
pub mod helpers {
    use super::*;

    /// Style for user messages
    pub fn user_message() -> Style {
        ColorTheme::primary()
    }

    /// Style for AI responses
    pub fn ai_response() -> Style {
        ColorTheme::ai_highlight()
    }

    /// Style for system notifications
    pub fn system_notification() -> Style {
        ColorTheme::primary().dim()
    }

    /// Style for tool calls
    pub fn tool_call() -> Style {
        ColorTheme::ai_highlight()
    }

    /// Style for tool results
    pub fn tool_result() -> Style {
        ColorTheme::misc()
    }

    /// Style for headers and titles
    pub fn header() -> Style {
        ColorTheme::primary().bold()
    }

    /// Style for menu selections
    pub fn menu_selected() -> Style {
        ColorTheme::selection()
    }

    /// Style for menu unselected items
    pub fn menu_unselected() -> Style {
        ColorTheme::secondary()
    }

    /// Style for code blocks
    pub fn code_block() -> Style {
        ColorTheme::misc()
    }

    /// Style for inline code
    pub fn inline_code() -> Style {
        ColorTheme::inline_code()
    }

    /// Style for progress indicators
    pub fn progress() -> Style {
        ColorTheme::primary()
    }

    /// Style for spinner animations
    pub fn spinner() -> Style {
        ColorTheme::ai_highlight()
    }

    /// Style for miscellaneous text
    pub fn misc() -> Style {
        ColorTheme::misc()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_constants() {
        // Verify that all constants are defined
        assert!(!PRIMARY_HEX.is_empty());
        assert!(!SECONDARY_HEX.is_empty());
        assert!(!BACKGROUND_HEX.is_empty());
        assert!(!AI_HIGHLIGHT_HEX.is_empty());
        assert!(!MISC_HEX.is_empty());

        // Verify ANSI color codes are within valid range
        assert!(PRIMARY_ANSI <= 255);
        assert!(SECONDARY_ANSI <= 255);
        assert!(BACKGROUND_ANSI <= 255);
        assert!(AI_HIGHLIGHT_ANSI <= 255);
        assert!(MISC_ANSI <= 255);
    }

    #[test]
    fn test_color_theme_methods() {
        // Test that all theme methods return Style objects
        let _primary = ColorTheme::primary();
        let _secondary = ColorTheme::secondary();
        let _background = ColorTheme::background();
        let _ai_highlight = ColorTheme::ai_highlight();
        let _misc = ColorTheme::misc();
        let _inline_code = ColorTheme::inline_code();
        let _ai_message = ColorTheme::ai_message();
        let _success = ColorTheme::success();
        let _error = ColorTheme::error();
        let _warning = ColorTheme::warning();
        let _dim = ColorTheme::dim();
        let _border = ColorTheme::border();
        let _selection = ColorTheme::selection();
    }

    #[test]
    fn test_color_ext_trait() {
        use console::Style;

        // Test that the extension trait methods work (each consumes the style)
        let _primary_style = Style::new().primary();
        let _secondary_style = Style::new().secondary();
        let _background_style = Style::new().background();
        let _ai_highlight_style = Style::new().ai_highlight();
        let _misc_style = Style::new().misc();
        let _inline_code_style = Style::new().inline_code_style();
    }

    #[test]
    fn test_helper_functions() {
        // Test that all helper functions return Style objects
        let _user_message = helpers::user_message();
        let _ai_response = helpers::ai_response();
        let _system_notification = helpers::system_notification();
        let _tool_call = helpers::tool_call();
        let _tool_result = helpers::tool_result();
        let _header = helpers::header();
        let _menu_selected = helpers::menu_selected();
        let _menu_unselected = helpers::menu_unselected();
        let _code_block = helpers::code_block();
        let _inline_code = helpers::inline_code();
        let _progress = helpers::progress();
        let _spinner = helpers::spinner();
    }
}