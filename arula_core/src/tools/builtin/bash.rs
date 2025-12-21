//! Bash command execution tool
//!
//! This tool allows execution of shell commands on the host system.
//! Uses tokio::process for async command execution with streaming output.
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
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;

/// Parameters for the bash tool
#[derive(Debug, Deserialize)]
pub struct BashParams {
    /// The command to execute
    pub command: String,
    /// Optional timeout in seconds (default: 30, max: 300)
    pub timeout_seconds: Option<u64>,
}

/// Result from bash command execution
#[derive(Debug, Serialize, Clone)]
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

/// Bash execution tool with streaming support
pub struct BashTool;

impl BashTool {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

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
        .description("timeout_seconds", "Timeout in seconds for the command (default: 30, max: 300).")
        .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        execute_bash(&params.command, params.timeout_seconds).await
    }
}

/// Execute a bash command with optional timeout (no streaming)
pub async fn execute_bash(
    command: &str,
    timeout_seconds: Option<u64>,
) -> Result<BashResult, String> {
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

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn command '{}': {}", command, e))?;

    let timeout_secs = timeout_seconds.unwrap_or(30).min(300);
    let timeout_duration = Duration::from_secs(timeout_secs);

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
                Err(e) => Err(format!("Failed to execute command: {}", e)),
            }
        }
        _ = tokio::time::sleep(timeout_duration) => {
            Err(format!("Command '{}' timed out after {} seconds", command, timeout_secs))
        }
    }
}

/// Execute a bash command with streaming output via channel
/// 
/// Returns a channel receiver that yields output lines and a join handle for the result.
/// This is designed to work with iced's async runtime.
pub fn execute_bash_streaming_channel(
    command: String,
    timeout_seconds: Option<u64>,
) -> (mpsc::UnboundedReceiver<(String, bool)>, tokio::task::JoinHandle<Result<BashResult, String>>) {
    let (tx, rx) = mpsc::unbounded_channel();
    
    let handle = tokio::spawn(async move {
        execute_bash_streaming_inner(&command, timeout_seconds, tx).await
    });
    
    (rx, handle)
}

/// Execute bash with streaming - sends each line through the channel
async fn execute_bash_streaming_inner(
    command: &str,
    timeout_seconds: Option<u64>,
    tx: mpsc::UnboundedSender<(String, bool)>,
) -> Result<BashResult, String> {
    use tokio::time::Duration;

    if command.trim().is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = TokioCommand::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = TokioCommand::new("sh");
        c.arg("-c").arg(command);
        c
    };

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn command '{}': {}", command, e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();

    let timeout_secs = timeout_seconds.unwrap_or(30).min(300);
    let timeout_duration = Duration::from_secs(timeout_secs);

    let read_result = tokio::time::timeout(timeout_duration, async {
        loop {
            tokio::select! {
                biased;  // Check in order
                
                line = stdout_reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            let _ = tx.send((l.clone(), false));
                            stdout_lines.push(l);
                        }
                        Ok(None) => {
                            // stdout EOF - process might still be running
                        }
                        Err(e) => {
                            let err = format!("Error reading stdout: {}", e);
                            let _ = tx.send((err.clone(), true));
                            stderr_lines.push(err);
                        }
                    }
                }
                
                line = stderr_reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            let _ = tx.send((l.clone(), true));
                            stderr_lines.push(l);
                        }
                        Ok(None) => {
                            // stderr EOF
                        }
                        Err(e) => {
                            let err = format!("Error reading stderr: {}", e);
                            let _ = tx.send((err.clone(), true));
                            stderr_lines.push(err);
                        }
                    }
                }
                
                status = child.wait() => {
                    // Process exited - drain remaining output
                    while let Ok(Some(l)) = stdout_reader.next_line().await {
                        let _ = tx.send((l.clone(), false));
                        stdout_lines.push(l);
                    }
                    while let Ok(Some(l)) = stderr_reader.next_line().await {
                        let _ = tx.send((l.clone(), true));
                        stderr_lines.push(l);
                    }
                    
                    match status {
                        Ok(s) => {
                            let exit_code = s.code().unwrap_or(-1);
                            return Ok(BashResult {
                                stdout: stdout_lines.join("\n"),
                                stderr: stderr_lines.join("\n"),
                                exit_code,
                                success: s.success(),
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
        Err(_) => Err(format!(
            "Command '{}' timed out after {} seconds",
            command, timeout_secs
        )),
    }
}

/// Execute bash with streaming callback - compatible with the existing API
pub async fn execute_bash_streaming<F>(
    command: &str,
    timeout_seconds: Option<u64>,
    mut on_line: F,
) -> Result<BashResult, String>
where
    F: FnMut(String, bool) + Send + 'static,
{
    let (mut rx, handle) = execute_bash_streaming_channel(command.to_string(), timeout_seconds);
    
    // Process lines as they arrive
    while let Some((line, is_stderr)) = rx.recv().await {
        on_line(line, is_stderr);
    }
    
    // Get the final result
    handle.await.map_err(|e| format!("Task error: {}", e))?
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
                timeout_seconds: None,
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
                timeout_seconds: None,
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_streaming() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        let result = execute_bash_streaming(
            "echo line1; echo line2; echo line3",
            Some(10),
            move |_line, _is_stderr| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            },
        )
        .await
        .unwrap();

        assert!(result.success);
        assert!(count.load(Ordering::SeqCst) >= 1);
    }
}
