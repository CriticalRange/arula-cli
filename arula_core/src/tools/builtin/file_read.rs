//! File reading tool
//!
//! This tool reads file contents with optional line range selection.
//! Uses memory mapping for large files for better performance.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use memmap2::MmapOptions;
use serde::{Deserialize, Serialize};

/// Parameters for the file read tool
#[derive(Debug, Deserialize)]
pub struct FileReadParams {
    /// The path to the file to read
    pub path: String,
    /// Optional starting line number (1-indexed)
    pub start_line: Option<usize>,
    /// Optional ending line number (1-indexed)
    pub end_line: Option<usize>,
}

/// Result from file reading
#[derive(Debug, Serialize)]
pub struct FileReadResult {
    /// The file content (or portion if line range specified)
    pub content: String,
    /// Number of lines returned
    pub lines: usize,
    /// Whether the read was successful
    pub success: bool,
}

/// File reading tool with memory-mapped file support
///
/// # Features
///
/// - Supports partial reads with line range selection
/// - Uses memory mapping for efficient reading of large files
/// - Falls back to buffered reading when memory mapping fails
///
/// # Example
///
/// ```rust,ignore
/// let tool = FileReadTool::new();
/// let result = tool.execute(FileReadParams {
///     path: "README.md".to_string(),
///     start_line: Some(1),
///     end_line: Some(10),
/// }).await?;
/// ```
pub struct FileReadTool;

impl FileReadTool {
    /// Create a new FileReadTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileReadTool {
    type Params = FileReadParams;
    type Result = FileReadResult;

    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Supports line range selection for partial reads."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new("read_file", "Read the contents of a file")
            .param("path", "string")
            .description("path", "The path to the file to read")
            .required("path")
            .param("start_line", "integer")
            .description(
                "start_line",
                "The starting line number (1-indexed, optional)",
            )
            .param("end_line", "integer")
            .description("end_line", "The ending line number (1-indexed, optional)")
            .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let FileReadParams {
            path,
            start_line,
            end_line,
        } = params;

        // Basic security check
        if path.trim().is_empty() {
            return Err("File path cannot be empty".to_string());
        }

        let file =
            File::open(&path).map_err(|e| format!("Failed to open file '{}': {}", path, e))?;

        // Try to use memory mapping for large files first
        if let Ok(mmap) = unsafe { MmapOptions::new().map(&file) } {
            // Use memmap2 for efficient reading
            let content = if let (Some(start), Some(end)) = (start_line, end_line) {
                // For line range with memmap, we need to count lines
                let lines: Vec<&str> = std::str::from_utf8(&mmap)
                    .map_err(|e| format!("Invalid UTF-8 in file: {}", e))?
                    .lines()
                    .collect();

                if start <= lines.len() {
                    let start_idx = start - 1;
                    let end_idx = std::cmp::min(end, lines.len());
                    lines[start_idx..end_idx].join("\n")
                } else {
                    String::new()
                }
            } else if let Some(start) = start_line {
                // Single start line - read from that line to end
                let lines: Vec<&str> = std::str::from_utf8(&mmap)
                    .map_err(|e| format!("Invalid UTF-8 in file: {}", e))?
                    .lines()
                    .collect();

                if start <= lines.len() {
                    lines[start - 1..].join("\n")
                } else {
                    String::new()
                }
            } else {
                // Read entire file with memmap
                std::str::from_utf8(&mmap)
                    .map_err(|e| format!("Invalid UTF-8 in file: {}", e))?
                    .to_string()
            };

            let line_count = content.lines().count();

            Ok(FileReadResult {
                content,
                lines: line_count,
                success: true,
            })
        } else {
            // Fallback to buffered reading for small files or when memmap fails
            let reader = BufReader::new(file);
            let mut lines: Vec<String> = Vec::new();

            for (line_num, line) in reader.lines().enumerate() {
                let line = line.map_err(|e| format!("Error reading file: {}", e))?;
                let current_line = line_num + 1; // Convert to 1-indexed

                // Apply line range filters if specified
                if let Some(start) = start_line {
                    if current_line < start {
                        continue;
                    }
                }

                if let Some(end) = end_line {
                    if current_line > end {
                        break;
                    }
                }

                lines.push(line);
            }

            let content = lines.join("\n");
            let line_count = lines.len();

            Ok(FileReadResult {
                content,
                lines: line_count,
                success: true,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_entire_file() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "line 1").unwrap();
        writeln!(temp, "line 2").unwrap();
        writeln!(temp, "line 3").unwrap();

        let tool = FileReadTool::new();
        let result = tool.execute(FileReadParams {
            path: temp.path().to_string_lossy().to_string(),
            start_line: None,
            end_line: None,
        }).await.unwrap();

        assert!(result.success);
        assert!(result.content.contains("line 1"));
        assert!(result.content.contains("line 2"));
        assert!(result.content.contains("line 3"));
    }

    #[tokio::test]
    async fn test_read_line_range() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "line 1").unwrap();
        writeln!(temp, "line 2").unwrap();
        writeln!(temp, "line 3").unwrap();
        writeln!(temp, "line 4").unwrap();

        let tool = FileReadTool::new();
        let result = tool.execute(FileReadParams {
            path: temp.path().to_string_lossy().to_string(),
            start_line: Some(2),
            end_line: Some(3),
        }).await.unwrap();

        assert!(result.success);
        assert!(!result.content.contains("line 1"));
        assert!(result.content.contains("line 2"));
        assert!(result.content.contains("line 3"));
        assert!(!result.content.contains("line 4"));
    }

    #[tokio::test]
    async fn test_file_not_found() {
        let tool = FileReadTool::new();
        let result = tool.execute(FileReadParams {
            path: "/nonexistent/path/file.txt".to_string(),
            start_line: None,
            end_line: None,
        }).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_empty_path() {
        let tool = FileReadTool::new();
        let result = tool.execute(FileReadParams {
            path: "".to_string(),
            start_line: None,
            end_line: None,
        }).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }
}

