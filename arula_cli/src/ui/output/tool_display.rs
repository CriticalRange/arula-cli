//! Tool call and result display formatting
//!
//! Provides consistent, visually appealing display for tool calls
//! and their results in the terminal.

use console::style;
use serde_json::Value;

/// Icons for different tool types
pub mod icons {
    pub const BASH: &str = "⚙️";
    pub const FILE_READ: &str = "📖";
    pub const FILE_WRITE: &str = "✍️";
    pub const FILE_EDIT: &str = "✏️";
    pub const DIRECTORY: &str = "📂";
    pub const SEARCH: &str = "🔍";
    pub const WEB: &str = "🌐";
    pub const QUESTION: &str = "❓";
    pub const SUCCESS: &str = "✅";
    pub const ERROR: &str = "❌";
    pub const TOOL: &str = "🔧";
    pub const LOADING: &str = "⏳";
}

/// Get the appropriate icon for a tool
pub fn get_tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        "execute_bash" => icons::BASH,
        "read_file" => icons::FILE_READ,
        "write_file" => icons::FILE_WRITE,
        "edit_file" => icons::FILE_EDIT,
        "list_directory" => icons::DIRECTORY,
        "search_files" => icons::SEARCH,
        "web_search" => icons::WEB,
        "ask_question" => icons::QUESTION,
        _ => icons::TOOL,
    }
}

/// Format a tool call for display
///
/// Creates a visually appealing representation of a tool being called.
///
/// # Example Output
///
/// ```text
/// ┌─ 🔧 Tool Call ────────────────────────────┐
/// │  ⚙️ execute_bash                          │
/// │  Command: echo "Hello World"              │
/// └────────────────────────────────────────────┘
/// ```
pub fn format_tool_call_box(tool_name: &str, arguments: &str) -> String {
    let icon = get_tool_icon(tool_name);
    let args: Result<Value, _> = serde_json::from_str(arguments);

    // Build description based on tool type
    let description = match tool_name {
        "execute_bash" => {
            let cmd = args.as_ref()
                .ok()
                .and_then(|v| v.get("command"))
                .and_then(|c| c.as_str())
                .unwrap_or("unknown");
            let display_cmd = if cmd.len() > 60 {
                format!("{}...", &cmd[..57])
            } else {
                cmd.to_string()
            };
            format!("Running: {}", display_cmd)
        }
        "read_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            format!("Reading: {}", path)
        }
        "write_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            format!("Writing: {}", path)
        }
        "edit_file" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            format!("Editing: {}", path)
        }
        "list_directory" => {
            let path = args.as_ref()
                .ok()
                .and_then(|v| v.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            format!("Listing: {}", path)
        }
        "search_files" => {
            let query = args.as_ref()
                .ok()
                .and_then(|v| v.get("pattern").or(v.get("query")))
                .and_then(|q| q.as_str())
                .unwrap_or("unknown");
            format!("Searching: {}", query)
        }
        "web_search" => {
            let query = args.as_ref()
                .ok()
                .and_then(|v| v.get("query"))
                .and_then(|q| q.as_str())
                .unwrap_or("unknown");
            format!("Web search: {}", query)
        }
        _ => format!("Calling: {}", tool_name),
    };

    format!(
        "{} {} {}",
        style(icon).cyan(),
        style(tool_name).bold().cyan(),
        style(&description).dim()
    )
}

/// Format a tool result for display
///
/// Creates a summary of the tool execution result.
pub fn format_tool_result_box(tool_name: &str, result: &Value, success: bool) -> String {
    let status_icon = if success { icons::SUCCESS } else { icons::ERROR };
    let status_style = if success {
        style(status_icon).green()
    } else {
        style(status_icon).red()
    };

    let summary = summarize_result(tool_name, result);

    format!(
        "{} {}",
        status_style,
        style(&summary).dim()
    )
}

/// Summarize a tool result in human-readable format
pub fn summarize_result(tool_name: &str, result: &Value) -> String {
    // Check for error wrapper
    if let Some(err) = result.get("Err") {
        return if let Some(err_str) = err.as_str() {
            format!("Error: {}", truncate_string(err_str, 80))
        } else {
            format!("Error: {}", truncate_json(err, 80))
        };
    }

    // Extract Ok result if present
    let inner = result.get("Ok").unwrap_or(result);

    match tool_name {
        "execute_bash" => {
            let exit_code = inner.get("exit_code").and_then(|c| c.as_i64()).unwrap_or(0);
            let stdout = inner.get("stdout").and_then(|s| s.as_str()).unwrap_or("");

            if exit_code == 0 {
                if stdout.trim().is_empty() {
                    "Command succeeded (no output)".to_string()
                } else {
                    let preview = truncate_string(stdout.lines().next().unwrap_or(""), 60);
                    format!("Success: {}", preview)
                }
            } else {
                format!("Failed (exit code {})", exit_code)
            }
        }
        "read_file" => {
            let lines = inner.get("lines").and_then(|l| l.as_u64()).unwrap_or(0);
            format!("Read {} lines", lines)
        }
        "write_file" => {
            let bytes = inner.get("bytes_written").and_then(|b| b.as_u64()).unwrap_or(0);
            format!("Wrote {} bytes", bytes)
        }
        "list_directory" => {
            if let Some(entries) = inner.get("entries").and_then(|e| e.as_array()) {
                let files = entries.iter()
                    .filter(|e| e.get("file_type").and_then(|t| t.as_str()) == Some("file"))
                    .count();
                let dirs = entries.iter()
                    .filter(|e| e.get("file_type").and_then(|t| t.as_str()) == Some("directory"))
                    .count();
                format!("Found {} files, {} directories", files, dirs)
            } else {
                "Listed directory".to_string()
            }
        }
        "search_files" => {
            let matches = inner.get("total_matches").and_then(|m| m.as_u64()).unwrap_or(0);
            let files = inner.get("files_searched").and_then(|f| f.as_u64()).unwrap_or(0);
            format!("Found {} matches in {} files", matches, files)
        }
        "web_search" => {
            let count = inner.get("result_count").and_then(|c| c.as_u64()).unwrap_or(0);
            format!("Found {} results", count)
        }
        _ => {
            if inner.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
                "Success".to_string()
            } else {
                truncate_json(inner, 80)
            }
        }
    }
}

/// Truncate a string to max length with ellipsis
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Truncate a JSON value to a string representation
fn truncate_json(value: &Value, max_len: usize) -> String {
    let s = value.to_string();
    truncate_string(&s, max_len)
}

/// Format a detailed tool result box (full output)
pub fn format_detailed_result(tool_name: &str, result: &Value, success: bool) -> String {
    let mut output = String::new();

    let status = if success {
        style("✅ Success").green().to_string()
    } else {
        style("❌ Failed").red().to_string()
    };

    output.push_str(&format!(
        "{}\n",
        style(format!("┌─ {} Result ─────────────────────────┐", tool_name)).dim()
    ));
    output.push_str(&format!("{} {}\n", style("│").dim(), status));

    // Format result content
    let inner = result.get("Ok").unwrap_or(result);
    let formatted = serde_json::to_string_pretty(inner)
        .unwrap_or_else(|_| inner.to_string());

    for line in formatted.lines().take(20) {
        output.push_str(&format!("{} {}\n", style("│").dim(), line));
    }

    if formatted.lines().count() > 20 {
        output.push_str(&format!("{} ... (truncated)\n", style("│").dim()));
    }

    output.push_str(&format!(
        "{}\n",
        style("└────────────────────────────────────────────┘").dim()
    ));

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_tool_icon() {
        assert_eq!(get_tool_icon("execute_bash"), icons::BASH);
        assert_eq!(get_tool_icon("read_file"), icons::FILE_READ);
        assert_eq!(get_tool_icon("unknown_tool"), icons::TOOL);
    }

    #[test]
    fn test_format_tool_call_box() {
        let result = format_tool_call_box("execute_bash", r#"{"command": "ls -la"}"#);
        assert!(result.contains("execute_bash"));
        assert!(result.contains("ls -la"));
    }

    #[test]
    fn test_summarize_result_bash_success() {
        let result = json!({
            "Ok": {
                "stdout": "Hello World\n",
                "stderr": "",
                "exit_code": 0
            }
        });
        let summary = summarize_result("execute_bash", &result);
        assert!(summary.contains("Success"));
    }

    #[test]
    fn test_summarize_result_bash_failure() {
        let result = json!({
            "Ok": {
                "stdout": "",
                "stderr": "Error",
                "exit_code": 1
            }
        });
        let summary = summarize_result("execute_bash", &result);
        assert!(summary.contains("Failed"));
    }

    #[test]
    fn test_summarize_result_error() {
        let result = json!({
            "Err": "File not found"
        });
        let summary = summarize_result("read_file", &result);
        assert!(summary.contains("Error"));
        assert!(summary.contains("not found"));
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("short", 10), "short");
        assert_eq!(truncate_string("this is a long string", 10), "this is...");
    }
}

