//! Directory listing tool
//!
//! This tool lists directory contents with support for hidden files
//! and recursive listing.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Parameters for the directory listing tool
#[derive(Debug, Deserialize)]
pub struct ListDirParams {
    /// The directory path to list
    pub path: String,
    /// Whether to show hidden files (default: false)
    pub show_hidden: Option<bool>,
    /// Whether to list recursively (default: false)
    pub recursive: Option<bool>,
}

/// Result from directory listing
#[derive(Debug, Serialize)]
pub struct DirectoryEntry {
    /// The name of the file or directory
    pub name: String,
    /// Full path to the entry
    pub path: String,
    /// Type: "file", "directory", or "symlink"
    pub file_type: String,
    /// File size in bytes (only for files)
    pub size: Option<u64>,
}

/// Result from directory listing
#[derive(Debug, Serialize)]
pub struct ListDirResult {
    /// List of directory entries
    pub entries: Vec<DirectoryEntry>,
    /// The path that was listed
    pub path: String,
    /// Whether the operation was successful
    pub success: bool,
}

/// Directory listing tool with recursive support
///
/// # Example
///
/// ```rust,ignore
/// let tool = ListDirectoryTool::new();
/// let result = tool.execute(ListDirParams {
///     path: ".".to_string(),
///     show_hidden: Some(false),
///     recursive: Some(false),
/// }).await?;
/// ```
pub struct ListDirectoryTool;

impl ListDirectoryTool {
    /// Create a new ListDirectoryTool instance
    pub fn new() -> Self {
        Self
    }

    fn scan_directory(
        &self,
        path: &str,
        show_hidden: bool,
        recursive: bool,
        entries: &mut Vec<DirectoryEntry>,
    ) -> Result<(), String> {
        use std::fs;

        let dir_entries = fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory '{}': {}", path, e))?;

        for entry in dir_entries {
            let entry = entry.map_err(|e| format!("Error reading directory entry: {}", e))?;
            let metadata = entry
                .metadata()
                .map_err(|e| format!("Error reading file metadata: {}", e))?;

            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files unless requested
            if !show_hidden && name.starts_with('.') {
                continue;
            }

            let file_type = if metadata.file_type().is_symlink() {
                "symlink".to_string()
            } else if metadata.file_type().is_dir() {
                "directory".to_string()
            } else {
                "file".to_string()
            };

            let size = if metadata.is_file() {
                Some(metadata.len())
            } else {
                None
            };

            let entry_path = entry.path().to_string_lossy().to_string();

            entries.push(DirectoryEntry {
                name: name.clone(),
                path: entry_path,
                file_type,
                size,
            });

            // Recursively scan subdirectories if requested
            if recursive && metadata.file_type().is_dir() {
                let dir_path = entry.path().to_string_lossy().to_string();
                self.scan_directory(&dir_path, show_hidden, true, entries)?;
            }
        }

        Ok(())
    }
}

impl Default for ListDirectoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListDirectoryTool {
    type Params = ListDirParams;
    type Result = ListDirResult;

    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List the contents of a directory. Can show hidden files and optionally list recursively."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new("list_directory", "List the contents of a directory")
            .param("path", "string")
            .description("path", "The directory path to list")
            .required("path")
            .param("show_hidden", "boolean")
            .description(
                "show_hidden",
                "Whether to show hidden files (default: false)",
            )
            .param("recursive", "boolean")
            .description(
                "recursive",
                "Whether to list directories recursively (default: false)",
            )
            .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let ListDirParams {
            path,
            show_hidden,
            recursive,
        } = params;

        let show_hidden = show_hidden.unwrap_or(false);
        let recursive = recursive.unwrap_or(false);

        let mut entries = Vec::new();
        self.scan_directory(&path, show_hidden, recursive, &mut entries)?;

        Ok(ListDirResult {
            entries,
            path,
            success: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_list_directory() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "content").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();

        let tool = ListDirectoryTool::new();
        let result = tool.execute(ListDirParams {
            path: temp_dir.path().to_string_lossy().to_string(),
            show_hidden: Some(false),
            recursive: Some(false),
        }).await.unwrap();

        assert!(result.success);
        assert_eq!(result.entries.len(), 3);
    }

    #[tokio::test]
    async fn test_list_directory_recursive() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "content").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        fs::write(temp_dir.path().join("subdir").join("nested.txt"), "nested").unwrap();

        let tool = ListDirectoryTool::new();
        let result = tool.execute(ListDirParams {
            path: temp_dir.path().to_string_lossy().to_string(),
            show_hidden: Some(false),
            recursive: Some(true),
        }).await.unwrap();

        assert!(result.success);
        assert!(result.entries.len() >= 3);
    }

    #[tokio::test]
    async fn test_list_nonexistent_directory() {
        let tool = ListDirectoryTool::new();
        let result = tool.execute(ListDirParams {
            path: "/nonexistent/path".to_string(),
            show_hidden: None,
            recursive: None,
        }).await;

        assert!(result.is_err());
    }
}

