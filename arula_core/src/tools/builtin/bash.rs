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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_command() {
        let tool = BashTool::new();
        let result = tool.execute(BashParams {
            command: "echo hello".to_string(),
        }).await.unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_empty_command_error() {
        let tool = BashTool::new();
        let result = tool.execute(BashParams {
            command: "   ".to_string(),
        }).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[tokio::test]
    async fn test_failing_command() {
        let tool = BashTool::new();
        // This command should fail on both Windows and Unix
        let result = tool.execute(BashParams {
            command: "exit 1".to_string(),
        }).await.unwrap();

        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
    }
}

