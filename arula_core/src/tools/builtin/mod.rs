//! Built-in tools for the ARULA CLI
//!
//! This module contains all built-in tools that are available by default:
//!
//! - `bash` - Execute shell commands
//! - `file_read` - Read file contents
//! - `file_write` - Write/create files
//! - `file_edit` - Edit existing files
//! - `list_dir` - List directory contents
//! - `search` - Search files for patterns
//! - `web_search` - Search the web
//! - `visioneer` - Vision/screenshot capabilities
//! - `question` - Ask clarifying questions
//!
//! # Architecture
//!
//! Each tool implements the `Tool` trait from `crate::api::agent` and provides:
//! - Parameter and result types using serde
//! - Tool schema for AI function calling
//! - Async execution logic
//!
//! # Adding New Tools
//!
//! 1. Create a new module file (e.g., `my_tool.rs`)
//! 2. Define parameter and result structs with `#[derive(Deserialize)]` and `#[derive(Serialize)]`
//! 3. Implement `Tool` trait for your tool struct
//! 4. Export from this module and add to `create_basic_tool_registry()`

pub mod bash;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod list_dir;
pub mod question;
pub mod search;
pub mod web_search;

// Re-export all tools for public API
// These are intentionally unused internally but exported for library users
#[allow(unused_imports)]
pub use bash::{BashParams, BashResult, BashTool};
#[allow(unused_imports)]
pub use file_edit::{FileEditParams, FileEditResult, FileEditTool};
#[allow(unused_imports)]
pub use file_read::{FileReadParams, FileReadResult, FileReadTool};
#[allow(unused_imports)]
pub use file_write::{WriteFileParams, WriteFileResult, WriteFileTool};
#[allow(unused_imports)]
pub use list_dir::{DirectoryEntry, ListDirParams, ListDirResult, ListDirectoryTool};
#[allow(unused_imports)]
pub use question::{QuestionParams, QuestionResult, QuestionTool};
#[allow(unused_imports)]
pub use search::{FileMatch, SearchMatch, SearchParams, SearchResult, SearchTool};
#[allow(unused_imports)]
pub use web_search::{WebSearchParams, WebSearchResult, WebSearchResultItem, WebSearchTool};
