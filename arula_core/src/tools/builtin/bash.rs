//! Bash command execution tool
//!
//! This tool allows execution of shell commands on the host system.
//!
//! # Security
//!
//! Commands are executed with the current user's permissions.
//! Basic validation prevents empty commands.
//!
//! # Cross-Platform Support
//!
//! - Windows: Uses `cmd /C`
//! - Unix/Linux/macOS: Uses `sh -c`

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command as TokioCommand;

/// Parameters for the bash tool
#[derive(Debug, Deserialize)]
pub struct BashParams {
    /// The command to execute
    pub command: String,
    /// Optional timeout in seconds (default: 30, max: 300)
    pub timeout_seconds: Option<u64>,
}

/// Result from bash command execution
#[derive(Debug, Serialize)]
pub struct BashResult {
    /// Standard output from the command
    pub stdout: String,
    /// Standard error from the command
    pub stderr: String,
    /// Exit code (0 typically indicates success)
    pub exit_code: i32,
    /// Whether the command succeeded (exit code 0)
    pub success: bool,
}

/// Modern bash execution tool
///
/// # Example
///
/// ```rust,ignore
/// let tool = BashTool::new();
/// let result = tool.execute(BashParams {
///     command: "echo Hello".to_string(),
/// }).await?;
/// assert!(result.success);
/// ```
pub struct BashTool;

impl BashTool {
    /// Create a new BashTool instance
    pub const fn new() -> Self {
        Self
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

// Provide a const instance for global use
pub const BASH_TOOL: BashTool = BashTool::new();

#[async_trait]
impl Tool for BashTool {
    type Params = BashParams;
    type Result = BashResult;

    fn name(&self) -> &str {
        "execute_bash"
    }

    fn description(&self) -> &str {
        "Execute bash shell commands and return the output. Use this when you need to run shell commands, check files, navigate directories, install packages, etc. You can specify a timeout_seconds parameter (default: 30, max: 300) for long-running commands."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new(
            "execute_bash",
            "Execute bash shell commands and return the output",
        )
        .param("command", "string")
        .description("command", "The bash command to execute")
        .required("command")
        .param("timeout_seconds", "integer")
        .description("timeout_seconds", "Timeout in seconds for the command (default: 30, max: 300). Use higher values for long-running commands like builds or downloads.")
        .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        use std::process::Stdio;
        use tokio::time::Duration;

        let command = &params.command;

        // Basic security checks
        if command.trim().is_empty() {
            return Err("Command cannot be empty".to_string());
        }

        // Build the command with stdin set to null to prevent blocking on user input
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = TokioCommand::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = TokioCommand::new("sh");
            c.arg("-c").arg(command);
            c
        };

        // Set stdin to null to prevent the process from waiting for user input
        cmd.stdin(Stdio::null());
        // Capture stdout and stderr
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Spawn the process
        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn command '{}': {}", command, e))?;

        // Get timeout from params (default: 30, max: 300 seconds)
        let timeout_secs = params.timeout_seconds.unwrap_or(30).min(300); // Cap at 5 minutes max
        let timeout_duration = Duration::from_secs(timeout_secs);

        // Use select! to race between command completion and timeout
        tokio::select! {
            result = child.wait_with_output() => {
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
            _ = tokio::time::sleep(timeout_duration) => {
                // Timeout occurred - kill the child process
                // Note: child was moved into the select! branch, so we can't kill it here
                // The process will be cleaned up when it's dropped
                Err(format!("Command '{}' timed out after {} seconds", command, timeout_secs))
            }
        }
    }
}

/// Execute a bash command with streaming output line-by-line
///
/// This function spawns the command and streams stdout/stderr lines as they arrive,
/// calling the provided callback for each line. This enables real-time display
/// in the UI rather than waiting for command completion.
pub async fn execute_bash_streaming<F>(
    command: &str,
    timeout_seconds: Option<u64>,
    mut on_line: F,
) -> Result<BashResult, String>
where
    F: FnMut(String, bool) + Send, // (line, is_stderr)
{
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::time::Duration;

    if command.trim().is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    // Build the command
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = TokioCommand::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = TokioCommand::new("sh");
        c.arg("-c").arg(command);
        c
    };

    // Set up for streaming
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Spawn the process
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn command '{}': {}", command, e))?;

    // Take ownership of stdout and stderr
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture stderr".to_string())?;

    // Create buffered readers
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    // Collected output for the final result
    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();

    // Get timeout
    let timeout_secs = timeout_seconds.unwrap_or(30).min(300);
    let timeout_duration = Duration::from_secs(timeout_secs);

    // Read lines concurrently with timeout
    let read_result = tokio::time::timeout(timeout_duration, async {
        loop {
            tokio::select! {
                // Read stdout line
                result = stdout_reader.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            on_line(line.clone(), false);
                            stdout_lines.push(line);
                        }
                        Ok(None) => {
                            // stdout closed, but stderr might still have data
                        }
                        Err(e) => {
                            let err_msg = format!("Error reading stdout: {}", e);
                            on_line(err_msg.clone(), true);
                            stderr_lines.push(err_msg);
                        }
                    }
                }
                // Read stderr line
                result = stderr_reader.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            on_line(line.clone(), true);
                            stderr_lines.push(line);
                        }
                        Ok(None) => {
                            // stderr closed
                        }
                        Err(e) => {
                            let err_msg = format!("Error reading stderr: {}", e);
                            on_line(err_msg.clone(), true);
                            stderr_lines.push(err_msg);
                        }
                    }
                }
                // Wait for child to exit
                result = child.wait() => {
                    match result {
                        Ok(status) => {
                            // Drain any remaining output
                            while let Ok(Some(line)) = stdout_reader.next_line().await {
                                on_line(line.clone(), false);
                                stdout_lines.push(line);
                            }
                            while let Ok(Some(line)) = stderr_reader.next_line().await {
                                on_line(line.clone(), true);
                                stderr_lines.push(line);
                            }

                            let exit_code = status.code().unwrap_or(-1);
                            return Ok(BashResult {
                                stdout: stdout_lines.join("\n"),
                                stderr: stderr_lines.join("\n"),
                                exit_code,
                                success: status.success(),
                            });
                        }
                        Err(e) => {
                            return Err(format!("Failed to wait for command: {}", e));
                        }
                    }
                }
            }
        }
    })
    .await;

    match read_result {
        Ok(result) => result,
        Err(_) => {
            // Timeout - try to kill the child
            let _ = child.kill().await;
            Err(format!(
                "Command '{}' timed out after {} seconds",
                command, timeout_secs
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_command() {
        let tool = BashTool::new();
        let result = tool
            .execute(BashParams {
                command: "echo hello".to_string(),
            })
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_empty_command_error() {
        let tool = BashTool::new();
        let result = tool
            .execute(BashParams {
                command: "   ".to_string(),
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[tokio::test]
    async fn test_failing_command() {
        let tool = BashTool::new();
        // This command should fail on both Windows and Unix
        let result = tool
            .execute(BashParams {
                command: "exit 1".to_string(),
            })
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
    }
}
