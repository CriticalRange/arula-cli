//! Modern tool implementations using the agent framework

use crate::agent::{Tool, ToolResult, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use futures::StreamExt;
use memmap2::{Mmap, MmapOptions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;
use tokio::process::Command as TokioCommand;

/// Parameters for the bash tool
#[derive(Debug, Deserialize)]
pub struct BashParams {
    pub command: String,
}

/// Result from bash command execution
#[derive(Debug, Serialize)]
pub struct BashResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

/// Modern bash execution tool
pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BashTool {
    type Params = BashParams;
    type Result = BashResult;

    fn name(&self) -> &str {
        "execute_bash"
    }

    fn description(&self) -> &str {
        "Execute bash shell commands and return the output. Use this when you need to run shell commands, check files, navigate directories, install packages, etc."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new(
            "execute_bash",
            "Execute bash shell commands and return the output",
        )
        .param("command", "string")
        .description("command", "The bash command to execute")
        .required("command")
        .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let command = &params.command;

        // Basic security checks
        if command.trim().is_empty() {
            return Err("Command cannot be empty".to_string());
        }

        let result = if cfg!(target_os = "windows") {
            TokioCommand::new("cmd")
                .args(["/C", command])
                .output()
                .await
        } else {
            TokioCommand::new("sh")
                .arg("-c")
                .arg(command)
                .output()
                .await
        };

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                let success = output.status.success();

                Ok(BashResult {
                    stdout,
                    stderr,
                    exit_code,
                    success,
                })
            }
            Err(e) => Err(format!("Failed to execute command '{}': {}", command, e)),
        }
    }
}

/// Parameters for the file read tool
#[derive(Debug, Deserialize)]
pub struct FileReadParams {
    pub path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

/// Result from file reading
#[derive(Debug, Serialize)]
pub struct FileReadResult {
    pub content: String,
    pub lines: usize,
    pub success: bool,
}

/// File reading tool
pub struct FileReadTool;

impl FileReadTool {
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

/// Parameters for the directory listing tool
#[derive(Debug, Deserialize)]
pub struct ListDirectoryParams {
    pub path: String,
    pub show_hidden: Option<bool>,
    pub recursive: Option<bool>,
}

/// Result from directory listing
#[derive(Debug, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub file_type: String, // "file", "directory", or "symlink"
    pub size: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ListDirectoryResult {
    pub entries: Vec<DirectoryEntry>,
    pub path: String,
    pub success: bool,
}

/// Directory listing tool
pub struct ListDirectoryTool;

impl ListDirectoryTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListDirectoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListDirectoryTool {
    type Params = ListDirectoryParams;
    type Result = ListDirectoryResult;

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
        let ListDirectoryParams {
            path,
            show_hidden,
            recursive,
        } = params;

        let show_hidden = show_hidden.unwrap_or(false);
        let recursive = recursive.unwrap_or(false);

        let mut entries = Vec::new();
        self.scan_directory(&path, show_hidden, recursive, &mut entries)?;

        Ok(ListDirectoryResult {
            entries,
            path,
            success: true,
        })
    }
}

impl ListDirectoryTool {
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

/// Parameters for the file edit tool
#[derive(Debug, Deserialize)]
pub struct FileEditParams {
    pub path: String,
    pub operation: EditOperation,
}

/// AI operation format with type field
#[derive(Debug, Deserialize)]
struct AiOperation {
    #[serde(rename = "type")]
    operation_type: String,
    content: Option<String>,
    line_number: Option<usize>,
    line: Option<usize>,
    start_line: Option<usize>,
    end_line: Option<usize>,
    old_text: Option<String>,
    new_text: Option<String>,
    #[serde(alias = "old")]
    old_text_alt: Option<String>,
    #[serde(alias = "new")]
    new_text_alt: Option<String>,
}

/// File edit operations
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum EditOperation {
    // AI format with type field has highest priority
    #[serde(deserialize_with = "deserialize_ai_operation")]
    AiFormat(Box<AiOperation>),
    // Fallback to direct format
    Create {
        content: String,
    },
    Insert {
        line: usize,
        content: String,
    },
    Replace {
        start_line: usize,
        end_line: usize,
        content: String,
    },
    Delete {
        start_line: usize,
        end_line: usize,
    },
    // Original names
    ReplaceText {
        old_text: String,
        new_text: String,
    },
    InsertAt {
        line_number: usize,
        content: String,
    },
    DeleteRange {
        start_line: usize,
        end_line: usize,
    },
    Append {
        content: String,
    },
    Prepend {
        content: String,
    },
}

/// Deserialize AI format operation
fn deserialize_ai_operation<'de, D>(deserializer: D) -> Result<Box<AiOperation>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let ai_op = AiOperation::deserialize(deserializer)?;
    Ok(Box::new(ai_op))
}

impl From<AiOperation> for EditOperation {
    fn from(ai_op: AiOperation) -> Self {
        match ai_op.operation_type.as_str() {
            "create" => EditOperation::Create {
                content: ai_op.content.unwrap_or_default(),
            },
            "replace" => {
                if let (Some(start_line), Some(end_line)) = (ai_op.start_line, ai_op.end_line) {
                    let content = ai_op.content.unwrap_or_default();
                    EditOperation::Replace {
                        start_line,
                        end_line,
                        content,
                    }
                } else if let (Some(old_text), Some(new_text)) = (
                    ai_op.old_text.or(ai_op.old_text_alt),
                    ai_op.new_text.or(ai_op.new_text_alt),
                ) {
                    EditOperation::ReplaceText { old_text, new_text }
                } else {
                    let content = ai_op.content.unwrap_or_default();
                    EditOperation::Create { content }
                }
            }
            "insert" => {
                let line = ai_op.line_number.or(ai_op.line).unwrap_or(1);
                let content = ai_op.content.unwrap_or_default();
                EditOperation::Insert { line, content }
            }
            "delete" => {
                let start_line = ai_op.start_line.unwrap_or(1);
                let end_line = ai_op.end_line.unwrap_or(start_line);
                EditOperation::Delete {
                    start_line,
                    end_line,
                }
            }
            "append" => EditOperation::Append {
                content: ai_op.content.unwrap_or_default(),
            },
            _ => EditOperation::Append {
                content: ai_op.content.unwrap_or_default(),
            },
        }
    }
}

/// Result from file editing
#[derive(Debug, Serialize)]
pub struct FileEditResult {
    pub success: bool,
    pub message: String,
    pub lines_changed: Option<usize>,
    pub backup_path: Option<String>,
}

/// File editing tool using file-editor library
pub struct FileEditTool;

impl FileEditTool {
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
        "Edit file contents with various operations: replace, insert, delete, append, prepend, and text replacement. Supports line-specific operations and automatic backups."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new("edit_file", "Edit file contents with various operations")
            .param("path", "string")
            .description("path", "The path to the file to edit")
            .required("path")
            .param("operation", "object")
            .description("operation", "The edit operation to perform")
            .required("operation")
            .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        use std::fs;
        use std::path::Path;

        let FileEditParams { path, operation } = params;

        // Basic security check
        if path.trim().is_empty() {
            return Err("File path cannot be empty".to_string());
        }

        // Verify file exists for most operations (except prepend if file doesn't exist)
        if !Path::new(&path).exists()
            && !matches!(
                operation,
                EditOperation::Prepend { .. } | EditOperation::Replace { .. }
            )
        {
            return Err(format!("File '{}' does not exist", path));
        }

        // Create backup for safety
        let backup_path = if Path::new(&path).exists() {
            let backup = format!("{}.backup.{}", path, chrono::Utc::now().timestamp());
            if let Err(e) = fs::copy(&path, &backup) {
                eprintln!("Warning: Could not create backup: {}", e);
                None
            } else {
                Some(backup)
            }
        } else {
            None
        };

        // Perform the operation using std::fs for reliability
        let result = match operation {
            // Handle AI format by converting it first
            EditOperation::AiFormat(ai_op) => {
                let converted_op: EditOperation = (*ai_op).into();
                return self
                    .execute(FileEditParams {
                        path,
                        operation: converted_op,
                    })
                    .await;
            }

            // AI-friendly operations
            EditOperation::Create { content } => {
                fs::write(&path, &content).map_err(|e| format!("Failed to create file: {}", e))?;

                let lines = content.lines().count();
                Ok(FileEditResult {
                    success: true,
                    message: format!("File '{}' created successfully with {} lines", path, lines),
                    lines_changed: Some(lines),
                    backup_path,
                })
            }

            EditOperation::Insert { line, content } => {
                let file_content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

                let mut lines: Vec<&str> = file_content.lines().collect();
                if line > 0 && line <= lines.len() + 1 {
                    lines.insert(line - 1, &content);
                    let new_content = lines.join("\n");

                    fs::write(&path, new_content)
                        .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

                    Ok(FileEditResult {
                        success: true,
                        message: format!("Inserted content at line {} in file '{}'", line, path),
                        lines_changed: Some(1),
                        backup_path,
                    })
                } else {
                    Err(format!(
                        "Invalid line number: {}. File has {} lines",
                        line,
                        lines.len()
                    ))
                }
            }

            EditOperation::Replace {
                start_line,
                end_line,
                content,
            } => {
                let file_content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

                let mut lines: Vec<&str> = file_content.lines().collect();
                if start_line > 0 && end_line >= start_line && end_line <= lines.len() {
                    let new_lines: Vec<&str> = content.lines().collect();
                    lines.splice(start_line - 1..end_line, new_lines);
                    let new_content = lines.join("\n");

                    fs::write(&path, new_content)
                        .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

                    Ok(FileEditResult {
                        success: true,
                        message: format!(
                            "Replaced lines {} to {} in file '{}'",
                            start_line, end_line, path
                        ),
                        lines_changed: Some(end_line - start_line + 1),
                        backup_path,
                    })
                } else {
                    Err(format!(
                        "Invalid line range: {} to {}. File has {} lines",
                        start_line,
                        end_line,
                        lines.len()
                    ))
                }
            }

            EditOperation::Delete {
                start_line,
                end_line,
            } => {
                let file_content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

                let mut lines: Vec<&str> = file_content.lines().collect();
                if start_line > 0 && end_line >= start_line && end_line <= lines.len() {
                    let deleted_count = end_line - start_line + 1;
                    lines.drain(start_line - 1..end_line);
                    let new_content = lines.join("\n");

                    fs::write(&path, new_content)
                        .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

                    Ok(FileEditResult {
                        success: true,
                        message: format!(
                            "Deleted {} lines ({} to {}) from file '{}'",
                            deleted_count, start_line, end_line, path
                        ),
                        lines_changed: Some(deleted_count),
                        backup_path,
                    })
                } else {
                    Err(format!(
                        "Invalid line range: {} to {}. File has {} lines",
                        start_line,
                        end_line,
                        lines.len()
                    ))
                }
            }

            // Original operations
            EditOperation::ReplaceText { old_text, new_text } => {
                let file_content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

                let new_content = file_content.replace(&old_text, &new_text);

                fs::write(&path, new_content)
                    .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

                Ok(FileEditResult {
                    success: true,
                    message: format!(
                        "Replaced '{}' with '{}' in file '{}'",
                        old_text, new_text, path
                    ),
                    lines_changed: None,
                    backup_path,
                })
            }

            EditOperation::InsertAt {
                line_number,
                content,
            } => {
                let file_content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

                let mut lines: Vec<&str> = file_content.lines().collect();
                if line_number > 0 && line_number <= lines.len() + 1 {
                    lines.insert(line_number - 1, &content);
                    let new_content = lines.join("\n");

                    fs::write(&path, new_content)
                        .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

                    Ok(FileEditResult {
                        success: true,
                        message: format!(
                            "Inserted content at line {} in file '{}'",
                            line_number, path
                        ),
                        lines_changed: Some(1),
                        backup_path,
                    })
                } else {
                    Err(format!(
                        "Invalid line number: {}. File has {} lines",
                        line_number,
                        lines.len()
                    ))
                }
            }

            EditOperation::DeleteRange {
                start_line,
                end_line,
            } => {
                let file_content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

                let mut lines: Vec<&str> = file_content.lines().collect();
                if start_line > 0 && end_line >= start_line && end_line <= lines.len() {
                    let deleted_count = end_line - start_line + 1;
                    lines.drain(start_line - 1..end_line);
                    let new_content = lines.join("\n");

                    fs::write(&path, new_content)
                        .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

                    Ok(FileEditResult {
                        success: true,
                        message: format!(
                            "Deleted {} lines ({} to {}) from file '{}'",
                            deleted_count, start_line, end_line, path
                        ),
                        lines_changed: Some(deleted_count),
                        backup_path,
                    })
                } else {
                    Err(format!(
                        "Invalid line range: {} to {}. File has {} lines",
                        start_line,
                        end_line,
                        lines.len()
                    ))
                }
            }

            EditOperation::Append { content } => {
                // Read existing content and append to it
                let existing_content = if Path::new(&path).exists() {
                    fs::read_to_string(&path)
                        .map_err(|e| format!("Failed to read existing file '{}': {}", path, e))?
                } else {
                    String::new()
                };

                let new_content = format!("{}{}", existing_content, content);
                fs::write(&path, new_content)
                    .map_err(|e| format!("Failed to append content: {}", e))?;

                let lines_added = content.lines().count();
                Ok(FileEditResult {
                    success: true,
                    message: format!("Appended {} lines to file '{}'", lines_added, path),
                    lines_changed: Some(lines_added),
                    backup_path,
                })
            }

            EditOperation::Prepend { content } => {
                let existing_content = if Path::new(&path).exists() {
                    fs::read_to_string(&path)
                        .map_err(|e| format!("Failed to read existing file '{}': {}", path, e))?
                } else {
                    String::new()
                };

                let new_content = format!("{}{}", content, existing_content);
                fs::write(&path, new_content)
                    .map_err(|e| format!("Failed to prepend content: {}", e))?;

                let lines_added = content.lines().count();
                Ok(FileEditResult {
                    success: true,
                    message: format!("Prepended {} lines to file '{}'", lines_added, path),
                    lines_changed: Some(lines_added),
                    backup_path,
                })
            }
        };

        result
    }
}

/// Parameters for the write file tool
#[derive(Debug, Deserialize)]
pub struct WriteFileParams {
    pub path: String,
    pub content: String,
}

/// Result from writing a file
#[derive(Debug, Serialize)]
pub struct WriteFileResult {
    pub success: bool,
    pub message: String,
    pub bytes_written: Option<usize>,
    pub lines_written: Option<usize>,
}

/// Simple file writing tool that creates or overwrites files
pub struct WriteFileTool;

impl WriteFileTool {
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
        "Write content to a file. Creates the file if it doesn't exist or overwrites it if it does."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new(
            "write_file",
            "Write content to a file (creates or overwrites)",
        )
        .param("path", "string")
        .description("path", "The file path to write to")
        .required("path")
        .param("content", "string")
        .description("content", "The content to write to the file")
        .required("content")
        .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        use std::fs;
        use std::path::Path;

        let WriteFileParams { path, content } = params;

        // Basic security checks
        if path.trim().is_empty() {
            return Err("File path cannot be empty".to_string());
        }

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(&path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create directory '{}': {}", parent.display(), e)
                })?;
            }
        }

        // Check if file exists for reporting
        let file_existed = Path::new(&path).exists();

        // Write the file
        fs::write(&path, &content)
            .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

        let bytes_written = content.len();
        let lines_written = if content.is_empty() {
            0
        } else {
            content.lines().count()
        };

        let message = if file_existed {
            format!(
                "Successfully overwrote file '{}' ({} bytes, {} lines)",
                path, bytes_written, lines_written
            )
        } else {
            format!(
                "Successfully created file '{}' ({} bytes, {} lines)",
                path, bytes_written, lines_written
            )
        };

        Ok(WriteFileResult {
            success: true,
            message,
            bytes_written: Some(bytes_written),
            lines_written: Some(lines_written),
        })
    }
}

/// Parameters for the search tool
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub path: Option<String>,
    pub file_pattern: Option<String>,
    pub case_sensitive: Option<bool>,
    pub max_results: Option<usize>,
}

/// Search result entry
#[derive(Debug, Clone, Serialize)]
pub struct SearchMatch {
    pub file: String,
    pub line_number: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}

/// Result from search operation
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub total_matches: usize,
    pub files_searched: usize,
    pub success: bool,
}

/// Fast parallel search tool with gitignore support
pub struct SearchTool;

impl SearchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SearchTool {
    type Params = SearchParams;
    type Result = SearchResult;

    fn name(&self) -> &str {
        "search_files"
    }

    fn description(&self) -> &str {
        "Search for text patterns in files using parallel walker with gitignore support. Fast and efficient for searching large codebases. Supports file pattern filtering, case-sensitive options, and provides detailed match results."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new(
            "search_files",
            "Search for text patterns in files"
        )
        .param("query", "string")
        .description("query", "The text pattern to search for. Can be a simple string or part of a larger expression.")
        .required("query")
        .param("path", "string")
        .description("path", "The directory path to search in. Use '.' for current directory (default).")
        .param("file_pattern", "string")
        .description("file_pattern", "File pattern to match (e.g., '*.rs', '*.py', '*.txt', '*.md'). Searches all files if not specified.")
        .param("case_sensitive", "boolean")
        .description("case_sensitive", "Whether the search should be case sensitive (default: false). Useful for searching code with specific capitalization.")
        .param("max_results", "integer")
        .description("max_results", "Maximum number of results to return (default: 100). Helps prevent overwhelming output in large codebases.")
        .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        use ignore::WalkBuilder;
        use std::path::Path;
        use std::sync::{Arc, Mutex};

        let SearchParams {
            query,
            path,
            file_pattern,
            case_sensitive,
            max_results,
        } = params;

        if query.trim().is_empty() {
            return Err("Search query cannot be empty. Please provide a non-empty text pattern to search for in files.".to_string());
        }

        let search_path = path.as_deref().unwrap_or(".");
        let case_sensitive = case_sensitive.unwrap_or(false);
        let max_results = max_results.unwrap_or(100);

        // Build glob matcher if pattern is provided
        let glob_matcher = if let Some(ref pattern) = file_pattern {
            use globset::{Glob, GlobSetBuilder};
            // Create a proper glob pattern for file inclusion
            let glob = Glob::new(pattern)
                .map_err(|e| format!("Invalid file pattern '{}': {}. Common patterns: '*.rs', '*.py', '*.txt', '*.md'", pattern, e))?;
            let mut builder = GlobSetBuilder::new();
            builder.add(glob);
            Some(
                builder
                    .build()
                    .map_err(|e| format!("Failed to process file pattern '{}': {}", pattern, e))?,
            )
        } else {
            None
        };

        // Validate path exists
        if !Path::new(search_path).exists() {
            return Err(format!("Search path '{}' does not exist or is not accessible. Please provide a valid directory path.", search_path));
        }

        // Shared state for collecting results
        let matches = Arc::new(Mutex::new(Vec::new()));
        let files_searched = Arc::new(Mutex::new(0usize));

        // Build the parallel walker with gitignore support
        let walker = WalkBuilder::new(search_path)
            .hidden(false) // Don't skip hidden files by default
            .git_ignore(true) // Respect .gitignore
            .git_global(true) // Respect global gitignore
            .git_exclude(true) // Respect .git/info/exclude
            .require_git(false) // Work even without git repo
            .follow_links(false) // Don't follow symlinks
            .threads(num_cpus::get())
            .build_parallel();

        // Clone Arcs for the closure
        let matches_clone = Arc::clone(&matches);
        let files_searched_clone = Arc::clone(&files_searched);
        let query_clone = query.clone();
        let glob_matcher_clone = glob_matcher.clone();

        // Walk files in parallel
        walker.run(|| {
            let matches = Arc::clone(&matches_clone);
            let files_searched = Arc::clone(&files_searched_clone);
            let query = query_clone.clone();
            let glob_matcher = glob_matcher_clone.clone();

            Box::new(move |result| {
                use ignore::WalkState;

                // Check if we've hit the max results limit
                {
                    let current_matches = matches.lock().unwrap();
                    if current_matches.len() >= max_results {
                        return WalkState::Quit;
                    }
                }

                let entry = match result {
                    Ok(entry) => entry,
                    Err(_) => return WalkState::Continue,
                };

                // Only process files
                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    return WalkState::Continue;
                }

                let path = entry.path();

                // Apply glob pattern filter if specified
                if let Some(ref matcher) = glob_matcher {
                    if !matcher.is_match(path) {
                        return WalkState::Continue;
                    }
                }

                // Check if file is binary before trying to read it
                // We'll use a simple heuristic: try to read first 8KB and check for null bytes
                if let Ok(sample) = std::fs::read(path) {
                    // Take first 8KB or entire file if smaller
                    let check_size = std::cmp::min(sample.len(), 8192);
                    let sample_slice = &sample[..check_size];

                    // If we find null bytes, it's likely binary
                    if sample_slice.contains(&0) {
                        return WalkState::Continue;
                    }
                }

                // Increment files searched counter
                {
                    let mut count = files_searched.lock().unwrap();
                    *count += 1;
                }

                // Read and search file contents
                // We already read the file above, but read_to_string is safer for UTF-8
                if let Ok(content) = std::fs::read_to_string(path) {
                    let file_path = path.to_string_lossy().to_string();

                    for (line_num, line) in content.lines().enumerate() {
                        let search_line = if case_sensitive {
                            line.to_string()
                        } else {
                            line.to_lowercase()
                        };

                        let search_query = if case_sensitive {
                            query.clone()
                        } else {
                            query.to_lowercase()
                        };

                        if let Some(pos) = search_line.find(&search_query) {
                            let match_result = SearchMatch {
                                file: file_path.clone(),
                                line_number: line_num + 1,
                                line_content: line.to_string(),
                                match_start: pos,
                                match_end: pos + query.len(),
                            };

                            let mut current_matches = matches.lock().unwrap();
                            current_matches.push(match_result);

                            // Check if we've reached the limit
                            if current_matches.len() >= max_results {
                                return WalkState::Quit;
                            }
                        }
                    }
                }

                WalkState::Continue
            })
        });

        // Collect final results
        let final_matches = match Arc::try_unwrap(matches) {
            Ok(mutex) => mutex.into_inner().unwrap(),
            Err(arc) => arc.lock().unwrap().clone(),
        };

        let total_matches = final_matches.len();
        let files_count = match Arc::try_unwrap(files_searched) {
            Ok(mutex) => mutex.into_inner().unwrap(),
            Err(arc) => *arc.lock().unwrap(),
        };

        // Provide helpful message when no matches found
        let success = if final_matches.is_empty() && files_count > 0 {
            // No matches found, but files were searched - this is still a successful operation
            true
        } else if files_count == 0 {
            // No files were searched - likely due to file pattern filtering or path issues
            if file_pattern.is_some() {
                return Err(format!("No files matched the file pattern '{}'. Try using a different pattern like '*.rs', '*.py', '*.txt' or remove the pattern to search all files.", file_pattern.unwrap()));
            } else {
                return Err("No searchable files found in the specified directory. The directory might be empty or contain only binary files.".to_string());
            }
        } else {
            true
        };

        Ok(SearchResult {
            matches: final_matches,
            total_matches,
            files_searched: files_count,
            success,
        })
    }
}

/// Factory function to create a default tool registry
pub fn create_default_tool_registry() -> crate::agent::ToolRegistry {
    use crate::agent::ToolRegistry;

    let mut registry = ToolRegistry::new();

    // Register the basic tools
    registry.register(BashTool::new());
    registry.register(FileReadTool::new());
    registry.register(FileEditTool::new());
    registry.register(WriteFileTool::new());
    registry.register(ListDirectoryTool::new());
    registry.register(SearchTool::new());

    registry
}
