//! File editing tool
//!
//! This tool provides various file editing operations including:
//! - Text replacement
//! - Line insertion
//! - Line deletion
//! - Content append/prepend
//!
//! Note: The full implementation is in the main tools.rs file.
//! This module re-exports the types for organization.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Parameters for the file edit tool
#[derive(Debug, Deserialize)]
pub struct FileEditParams {
    /// The path to the file to edit
    pub path: String,
    /// The type of edit operation
    #[serde(rename = "type")]
    pub edit_type: Option<String>,
    /// Old text to replace (for replace operations)
    pub old_text: Option<String>,
    /// New text (for replace operations)
    pub new_text: Option<String>,
    /// Content for various operations
    pub content: Option<String>,
    /// Line number for insert operations
    pub line: Option<usize>,
    /// Start line for range operations
    pub start_line: Option<usize>,
    /// End line for range operations
    pub end_line: Option<usize>,
}

/// Result from file editing
#[derive(Debug, Serialize)]
pub struct FileEditResult {
    /// Whether the edit was successful
    pub success: bool,
    /// Status message
    pub message: String,
    /// Number of lines changed
    pub lines_changed: Option<usize>,
    /// Path to backup file if created
    pub backup_path: Option<String>,
    /// Diff showing changes
    pub diff: Option<String>,
}

/// File editing tool
///
/// Supports multiple edit operations:
/// - replace: Find and replace text
/// - insert: Insert content at a line
/// - delete: Delete a range of lines
/// - append: Add content to end
/// - prepend: Add content to beginning
pub struct FileEditTool;

impl FileEditTool {
    /// Create a new FileEditTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileEditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileEditTool {
    type Params = FileEditParams;
    type Result = FileEditResult;

    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit file contents with various operations: replace, insert, delete, append, prepend."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new("edit_file", "Edit file contents")
            .param("path", "string")
            .description("path", "The path to the file to edit")
            .required("path")
            .param("type", "string")
            .description("type", "Operation type: replace, insert, delete, append, prepend")
            .param("old_text", "string")
            .description("old_text", "Text to find and replace")
            .param("new_text", "string")
            .description("new_text", "Replacement text")
            .param("content", "string")
            .description("content", "Content for insert/append/prepend")
            .param("line", "integer")
            .description("line", "Line number for insert")
            .param("start_line", "integer")
            .description("start_line", "Start line for range operations")
            .param("end_line", "integer")
            .description("end_line", "End line for range operations")
            .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        use std::fs;

        let path = &params.path;
        if path.trim().is_empty() {
            return Err("File path cannot be empty".to_string());
        }

        let edit_type = params.edit_type.as_deref().unwrap_or("replace");

        // Read current content
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

        let new_content = match edit_type {
            "replace" => {
                if let (Some(old), Some(new)) = (&params.old_text, &params.new_text) {
                    if !content.contains(old) {
                        return Err(format!("Text '{}' not found in file", old));
                    }
                    content.replace(old, new)
                } else {
                    return Err("replace operation requires old_text and new_text".to_string());
                }
            }
            "append" => {
                let add = params.content.as_deref().unwrap_or("");
                format!("{}{}", content, add)
            }
            "prepend" => {
                let add = params.content.as_deref().unwrap_or("");
                format!("{}{}", add, content)
            }
            "insert" => {
                let line_num = params.line.unwrap_or(1);
                let insert_content = params.content.as_deref().unwrap_or("");
                let mut lines: Vec<&str> = content.lines().collect();
                let insert_idx = (line_num.saturating_sub(1)).min(lines.len());
                lines.insert(insert_idx, insert_content);
                lines.join("\n")
            }
            "delete" => {
                let start = params.start_line.unwrap_or(1).saturating_sub(1);
                let end = params.end_line.unwrap_or(start + 1);
                let lines: Vec<&str> = content.lines().collect();
                let mut result: Vec<&str> = Vec::new();
                for (i, line) in lines.iter().enumerate() {
                    if i < start || i >= end {
                        result.push(line);
                    }
                }
                result.join("\n")
            }
            _ => return Err(format!("Unknown operation type: {}", edit_type)),
        };

        // Write new content
        fs::write(path, &new_content)
            .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

        Ok(FileEditResult {
            success: true,
            message: format!("Successfully edited '{}'", path),
            lines_changed: Some(new_content.lines().count()),
            backup_path: None,
            diff: None,
        })
    }
}

