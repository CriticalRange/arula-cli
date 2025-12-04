use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Represents a tool call in JSON format from the AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub arguments: serde_json::Value,
}

/// Represents a tool call response
#[derive(Debug, Clone)]
pub struct ToolCallResult {
    pub tool: String,
    pub success: bool,
    pub output: String,
}

/// Bash tool parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashToolParams {
    pub command: String,
}

/// Execute bash command using duct
pub async fn execute_bash_tool(command: &str) -> Result<ToolCallResult> {
    use duct::cmd;

    match cmd!("bash", "-c", command).read() {
        Ok(output) => Ok(ToolCallResult {
            tool: "bash_tool".to_string(),
            success: true,
            output,
        }),
        Err(e) => Ok(ToolCallResult {
            tool: "bash_tool".to_string(),
            success: false,
            output: format!("Error: {}", e),
        }),
    }
}

/// Get tool schema for bash_tool
pub fn get_bash_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "bash_tool",
            "description": "Execute bash shell commands. Use this when you need to run shell commands, check files, navigate directories, install packages, etc.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute. Make sure the command is safe and appropriate."
                    }
                },
                "required": ["command"]
            }
        }
    })
}

/// Extract tool calls from AI message content
/// Supports multiple formats: ```json, ```bash, ```shell, or raw JSON
pub fn extract_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();
    let mut in_code_block = false;
    let mut current_code = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect start of code block (```json, ```bash, ```shell, etc.)
        if trimmed.starts_with("```") && !in_code_block {
            in_code_block = true;
            current_code.clear();
            continue;
        }

        // Detect end of code block
        if trimmed.starts_with("```") && in_code_block {
            in_code_block = false;

            // Try to parse as JSON first
            if let Ok(tool_call) = serde_json::from_str::<ToolCall>(current_code.trim()) {
                tool_calls.push(tool_call);
            } else if let Ok(tool_calls_array) =
                serde_json::from_str::<Vec<ToolCall>>(current_code.trim())
            {
                tool_calls.extend(tool_calls_array);
            } else {
                // If JSON parsing fails, treat it as a bash command
                let command = current_code.trim().to_string();
                if !command.is_empty() {
                    tool_calls.push(ToolCall {
                        tool: "bash".to_string(),
                        arguments: serde_json::json!({ "command": command }),
                    });
                }
            }

            current_code.clear();
            continue;
        }

        // Collect code block content
        if in_code_block {
            if !current_code.is_empty() {
                current_code.push('\n');
            }
            current_code.push_str(line);
        }
    }

    // Also try to find raw JSON objects in the text (fallback)
    if tool_calls.is_empty() {
        let mut in_json = false;
        let mut brace_count = 0;
        let mut current_json = String::new();

        for ch in content.chars() {
            if ch == '{' {
                if brace_count == 0 {
                    in_json = true;
                    current_json.clear();
                }
                brace_count += 1;
            }

            if in_json {
                current_json.push(ch);
            }

            if ch == '}' {
                brace_count -= 1;
                if brace_count == 0 && in_json {
                    in_json = false;

                    if let Ok(tool_call) = serde_json::from_str::<ToolCall>(current_json.trim()) {
                        tool_calls.push(tool_call);
                    }

                    current_json.clear();
                }
            }
        }
    }

    tool_calls
}

/// Detect if a string contains valid JSON
pub fn is_json(text: &str) -> bool {
    text.trim_start().starts_with('{') || text.trim_start().starts_with('[')
}

/// Extract bash commands from code blocks
pub fn extract_bash_commands(content: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut in_code_block = false;
    let mut current_code = String::new();
    let mut is_bash_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect start of code block
        if trimmed.starts_with("```") && !in_code_block {
            in_code_block = true;
            is_bash_block =
                trimmed.contains("bash") || trimmed.contains("sh") || trimmed.contains("shell");
            current_code.clear();
            continue;
        }

        // Detect end of code block
        if trimmed.starts_with("```") && in_code_block {
            in_code_block = false;

            if is_bash_block && !current_code.trim().is_empty() {
                commands.push(current_code.trim().to_string());
            }

            current_code.clear();
            is_bash_block = false;
            continue;
        }

        // Collect code block content
        if in_code_block && is_bash_block {
            if !current_code.is_empty() {
                current_code.push('\n');
            }
            current_code.push_str(line);
        }
    }

    commands
}

/// Pretty format JSON for display
pub fn format_json(json_str: &str) -> Result<String, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(json_str)?;
    serde_json::to_string_pretty(&value)
}
