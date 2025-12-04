//! Refactored output system for ARULA CLI
//!
//! This module provides a modular, high-performance terminal output system with:
//!
//! - **Lazy-loaded resources**: Syntax highlighter and themes loaded once on first use
//! - **Streaming markdown**: Real-time markdown rendering for AI responses
//! - **Syntax highlighting**: Code block highlighting using syntect
//! - **Progress indicators**: Spinners and progress bars using indicatif
//! - **Tool display**: Formatted tool call and result output
//!
//! # Architecture
//!
//! ```text
//! OutputHandler (facade)
//!     ├── CodeHighlighter (OnceLock)
//!     ├── MarkdownStreamer
//!     ├── SpinnerManager
//!     └── tool_display
//! ```
//!
//! # Performance Optimizations
//!
//! - `SyntaxSet` and `ThemeSet` are lazy-loaded using `OnceLock`
//! - Connection pooling for HTTP requests (see `api/http_client.rs`)
//! - Efficient string building with pre-allocated buffers
//! - Minimal terminal writes with batched updates
//!
//! # Example
//!
//! ```rust,ignore
//! use arula_cli::ui::output::OutputHandler;
//!
//! let mut output = OutputHandler::new().with_debug(true);
//! output.print_banner()?;
//! output.stream_ai_response("Hello!")?;
//! ```

pub mod handler;
pub mod code_blocks;
pub mod markdown;
pub mod spinners;
pub mod tool_display;

// Re-export main handler
pub use handler::OutputHandler;

// Additional exports available via submodules:
// code_blocks::{CodeHighlighter, get_syntax_set, get_theme_set, format_code_box}
// markdown::{MarkdownStreamer, render_markdown, render_markdown_inline}
// spinners::{SpinnerStyle, SpinnerManager, create_spinner, create_progress_bar}
// tool_display::{format_tool_call_box, format_tool_result_box, get_tool_icon}

/// Terminal width constant (can be made dynamic)
pub const DEFAULT_TERMINAL_WIDTH: usize = 80;

/// Maximum code block preview lines
pub const MAX_CODE_PREVIEW_LINES: usize = 50;

