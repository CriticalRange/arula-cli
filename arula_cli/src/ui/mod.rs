//! UI modules for ARULA CLI
//!
//! Contains user interface components including input handling, output formatting,
//! menu systems, terminal interactions, and beautiful animation effects.
//!
//! # Module Structure
//!
//! - `output` - Modular output system with lazy-loaded resources
//! - `response_display` - AI response streaming display
//! - `thinking_widget` - AI thinking/reasoning display with pulsing animation
//! - `menus` - Interactive menu system
//! - `custom_spinner` - Custom spinner animations
//! - `effects` - Terminal animation effects
//! - `input_handler` - User input handling
//!
//! # Output System Architecture
//!
//! The `output` module provides:
//! - `OutputHandler` - Main facade for all output operations
//! - `CodeHighlighter` - Lazy-loaded syntax highlighting
//! - `MarkdownStreamer` - Real-time markdown streaming
//! - Spinners and progress bars via `indicatif`

pub mod custom_spinner;
pub mod effects;
pub mod input_handler;
pub mod menus;
pub mod output;
pub mod response_display;
pub mod thinking_widget;

// Re-export thinking widget for convenience
pub use thinking_widget::ThinkingWidget;

// Output module components available via:
// output::{OutputHandler, CodeHighlighter, MarkdownStreamer, SpinnerStyle, etc.}