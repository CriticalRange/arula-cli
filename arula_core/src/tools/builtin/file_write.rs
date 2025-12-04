//! File writing tool
//!
//! This tool creates or overwrites files with new content.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Parameters for the write file tool
#[derive(Debug, Deserialize)]
pub struct WriteFileParams {
    /// The path to the file to write
    pub path: String,
    /// The content to write to the file
    pub content: String,
}

/// Result from file writing
#[derive(Debug, Serialize)]
pub struct WriteFileResult {
    /// Whether the write was successful
    pub success: bool,
    /// Status message
    pub message: String,
    /// Number of bytes written
    pub bytes_written: usize,
}

/// File writing tool
///
/// Creates new files or overwrites existing files with content.
/// Automatically creates parent directories if they don't exist.
pub struct WriteFileTool;

impl WriteFileTool {
    /// Create a new WriteFileTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    type Params = WriteFileParams;
    type Result = WriteFileResult;

    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file, creating it if it doesn't exist or overwriting if it does."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new(
            "write_file",
            "Write content to a file, creating or overwriting as needed",
        )
        .param("path", "string")
        .description("path", "The path to the file to write")
        .required("path")
        .param("content", "string")
        .description("content", "The content to write to the file")
        .required("content")
        .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let WriteFileParams { path, content } = params;

        // Validate path
        if path.trim().is_empty() {
            return Err("File path cannot be empty".to_string());
        }

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directories: {}", e))?;
            }
        }

        // Write the file
        let bytes_written = content.len();
        fs::write(&path, &content)
            .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

        Ok(WriteFileResult {
            success: true,
            message: format!("Successfully wrote {} bytes to '{}'", bytes_written, path),
            bytes_written,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");

        let tool = WriteFileTool::new();
        let result = tool.execute(WriteFileParams {
            path: file_path.to_string_lossy().to_string(),
            content: "Hello, World!".to_string(),
        }).await.unwrap();

        assert!(result.success);
        assert_eq!(result.bytes_written, 13);
        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[tokio::test]
    async fn test_overwrite_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("existing.txt");
        fs::write(&file_path, "old content").unwrap();

        let tool = WriteFileTool::new();
        let result = tool.execute(WriteFileParams {
            path: file_path.to_string_lossy().to_string(),
            content: "new content".to_string(),
        }).await.unwrap();

        assert!(result.success);
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn test_create_with_directories() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("a/b/c/deep.txt");

        let tool = WriteFileTool::new();
        let result = tool.execute(WriteFileParams {
            path: file_path.to_string_lossy().to_string(),
            content: "deep content".to_string(),
        }).await.unwrap();

        assert!(result.success);
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_empty_path_error() {
        let tool = WriteFileTool::new();
        let result = tool.execute(WriteFileParams {
            path: "".to_string(),
            content: "content".to_string(),
        }).await;

        assert!(result.is_err());
    }
}

