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
pub mod file_read;
pub mod file_write;
pub mod file_edit;
pub mod list_dir;
pub mod search;
pub mod web_search;
pub mod question;

// Re-export all tools for public API
// These are intentionally unused internally but exported for library users
#[allow(unused_imports)]
pub use bash::{BashTool, BashParams, BashResult};
#[allow(unused_imports)]
pub use file_read::{FileReadTool, FileReadParams, FileReadResult};
#[allow(unused_imports)]
pub use file_write::{WriteFileTool, WriteFileParams, WriteFileResult};
#[allow(unused_imports)]
pub use file_edit::{FileEditTool, FileEditParams, FileEditResult};
#[allow(unused_imports)]
pub use list_dir::{ListDirectoryTool, ListDirParams, DirectoryEntry, ListDirResult};
#[allow(unused_imports)]
pub use search::{SearchTool, SearchParams, SearchMatch, FileMatch, SearchResult};
#[allow(unused_imports)]
pub use web_search::{WebSearchTool, WebSearchParams, WebSearchResultItem, WebSearchResult};
#[allow(unused_imports)]
pub use question::{QuestionTool, QuestionParams, QuestionResult};

