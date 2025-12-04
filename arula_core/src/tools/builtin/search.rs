//! File search tool
//!
//! This tool searches for patterns in files using regex or literal matching.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Parameters for the search tool
#[derive(Debug, Deserialize)]
pub struct SearchParams {
    /// The pattern to search for
    pub pattern: String,
    /// The directory or file path to search in
    pub path: Option<String>,
    /// Whether to use regex (default: false for literal search)
    pub regex: Option<bool>,
    /// Maximum number of results to return
    pub max_results: Option<usize>,
    /// File extensions to include (e.g., ["rs", "py"])
    pub extensions: Option<Vec<String>>,
}

/// A single match within a file
#[derive(Debug, Serialize)]
pub struct SearchMatch {
    /// Line number (1-indexed)
    pub line_number: usize,
    /// The matched line content
    pub line_content: String,
    /// Column where match starts (0-indexed)
    pub column: usize,
}

/// Matches found in a single file
#[derive(Debug, Serialize)]
pub struct FileMatch {
    /// Path to the file
    pub path: String,
    /// Matches found in this file
    pub matches: Vec<SearchMatch>,
}

/// Result from file search
#[derive(Debug, Serialize)]
pub struct SearchResult {
    /// Files containing matches
    pub files: Vec<FileMatch>,
    /// Total number of matches
    pub total_matches: usize,
    /// Number of files searched
    pub files_searched: usize,
    /// Whether the search was successful
    pub success: bool,
}

/// File search tool
///
/// Searches for patterns in files with support for:
/// - Literal string matching
/// - Regular expression matching
/// - File extension filtering
/// - Result limiting
pub struct SearchTool;

impl SearchTool {
    /// Create a new SearchTool instance
    pub fn new() -> Self {
        Self
    }

    fn search_file(
        &self,
        path: &Path,
        pattern: &str,
        use_regex: bool,
    ) -> Result<Vec<SearchMatch>, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;

        let mut matches = Vec::new();

        if use_regex {
            let re = regex::Regex::new(pattern)
                .map_err(|e| format!("Invalid regex: {}", e))?;

            for (line_num, line) in content.lines().enumerate() {
                if let Some(m) = re.find(line) {
                    matches.push(SearchMatch {
                        line_number: line_num + 1,
                        line_content: line.to_string(),
                        column: m.start(),
                    });
                }
            }
        } else {
            for (line_num, line) in content.lines().enumerate() {
                if let Some(pos) = line.find(pattern) {
                    matches.push(SearchMatch {
                        line_number: line_num + 1,
                        line_content: line.to_string(),
                        column: pos,
                    });
                }
            }
        }

        Ok(matches)
    }

    fn search_directory(
        &self,
        path: &Path,
        pattern: &str,
        use_regex: bool,
        extensions: &Option<Vec<String>>,
        results: &mut Vec<FileMatch>,
        files_searched: &mut usize,
        max_results: usize,
        total_matches: &mut usize,
    ) -> Result<(), String> {
        if *total_matches >= max_results {
            return Ok(());
        }

        if path.is_file() {
            // Check extension filter
            if let Some(exts) = extensions {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if !exts.iter().any(|e| e.to_lowercase() == ext_str) {
                        return Ok(());
                    }
                } else {
                    return Ok(());
                }
            }

            *files_searched += 1;
            if let Ok(matches) = self.search_file(path, pattern, use_regex) {
                if !matches.is_empty() {
                    *total_matches += matches.len();
                    results.push(FileMatch {
                        path: path.to_string_lossy().to_string(),
                        matches,
                    });
                }
            }
        } else if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    // Skip hidden files and common ignore patterns
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name == "node_modules" || name == "target" {
                        continue;
                    }
                    self.search_directory(
                        &entry_path,
                        pattern,
                        use_regex,
                        extensions,
                        results,
                        files_searched,
                        max_results,
                        total_matches,
                    )?;
                }
            }
        }

        Ok(())
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
        "Search for patterns in files. Supports literal and regex matching."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new("search_files", "Search for patterns in files")
            .param("pattern", "string")
            .description("pattern", "The pattern to search for")
            .required("pattern")
            .param("path", "string")
            .description("path", "Directory or file to search in (default: current directory)")
            .param("regex", "boolean")
            .description("regex", "Use regex matching (default: false)")
            .param("max_results", "integer")
            .description("max_results", "Maximum matches to return (default: 100)")
            .param("extensions", "array")
            .description("extensions", "File extensions to include, e.g. [\"rs\", \"py\"]")
            .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let SearchParams {
            pattern,
            path,
            regex,
            max_results,
            extensions,
        } = params;

        if pattern.is_empty() {
            return Err("Search pattern cannot be empty".to_string());
        }

        let search_path = path.unwrap_or_else(|| ".".to_string());
        let use_regex = regex.unwrap_or(false);
        let max_results = max_results.unwrap_or(100);

        let mut results = Vec::new();
        let mut files_searched = 0;
        let mut total_matches = 0;

        self.search_directory(
            Path::new(&search_path),
            &pattern,
            use_regex,
            &extensions,
            &mut results,
            &mut files_searched,
            max_results,
            &mut total_matches,
        )?;

        Ok(SearchResult {
            files: results,
            total_matches,
            files_searched,
            success: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_search_literal() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test.txt"), "hello world\nfoo bar\nhello again").unwrap();

        let tool = SearchTool::new();
        let result = tool.execute(SearchParams {
            pattern: "hello".to_string(),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            regex: Some(false),
            max_results: None,
            extensions: None,
        }).await.unwrap();

        assert!(result.success);
        assert_eq!(result.total_matches, 2);
    }

    #[tokio::test]
    async fn test_search_regex() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test.txt"), "hello123\nworld456\nhello789").unwrap();

        let tool = SearchTool::new();
        let result = tool.execute(SearchParams {
            pattern: r"hello\d+".to_string(),
            path: Some(temp_dir.path().to_string_lossy().to_string()),
            regex: Some(true),
            max_results: None,
            extensions: None,
        }).await.unwrap();

        assert!(result.success);
        assert_eq!(result.total_matches, 2);
    }
}

