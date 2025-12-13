//! Android command execution using Termux

use crate::platform::android::{AndroidContext, callbacks};
use anyhow::Result;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::process::Command as AsyncCommand;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Android command executor using Termux environment
pub struct AndroidCommandExecutor {
    ctx: AndroidContext,
    shell: Arc<Mutex<String>>,
}

impl AndroidCommandExecutor {
    pub fn new(ctx: AndroidContext) -> Self {
        Self {
            ctx,
            shell: Arc::new(Mutex::new("/data/data/com.termux/files/usr/bin/bash".to_string())),
        }
    }

    /// Execute a command synchronously
    pub async fn execute_sync(&self, command: &str, args: &[&str]) -> Result<CommandResult> {
        let shell = self.shell.lock().await;

        // Build command using Termux shell
        let mut cmd = AsyncCommand::new(&*shell);
        cmd.arg("-c")
            .arg(format!("{} {}", command, args.join(" ")))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn command: {}", e))?;

        let stdout = child.stdout.take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        let stderr = child.stderr.take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;

        let stdout_lines = Arc::new(Mutex::new(Vec::new()));
        let stderr_lines = Arc::new(Mutex::new(Vec::new()));

        // Read stdout
        let stdout_reader = BufReader::new(stdout);
        let stdout_clone = Arc::clone(&stdout_lines);
        tokio::spawn(async move {
            let mut lines = stdout_clone.lock().await;
            let mut reader = stdout_reader.lines();
            while let Some(line) = reader.next_line().await.map_err(|e| {
                log::error!("Error reading stdout: {}", e);
            })? {
                lines.push(line);
                // Send to callback
                callbacks::on_stream_chunk(&line);
            }
            Ok::<(), ()>(())
        });

        // Read stderr
        let stderr_reader = BufReader::new(stderr);
        let stderr_clone = Arc::clone(&stderr_lines);
        tokio::spawn(async move {
            let mut lines = stderr_clone.lock().await;
            let mut reader = stderr_reader.lines();
            while let Some(line) = reader.next_line().await.map_err(|e| {
                log::error!("Error reading stderr: {}", e);
            })? {
                lines.push(line);
                // Send error to callback
                callbacks::on_stream_chunk(&format!("[ERROR] {}", line));
            }
            Ok::<(), ()>(())
        });

        // Wait for command to complete
        let status = child.wait().await
            .map_err(|e| anyhow::anyhow!("Command execution error: {}", e))?;

        let stdout_output = {
            let lines = stdout_lines.lock().await;
            lines.join("\n")
        };

        let stderr_output = {
            let lines = stderr_lines.lock().await;
            lines.join("\n")
        };

        Ok(CommandResult {
            exit_code: status.code().unwrap_or(-1),
            stdout: stdout_output,
            stderr: stderr_output,
            success: status.success(),
        })
    }

    /// Execute command with streaming output
    pub async fn execute_streaming(&self, command: &str, args: &[&str]) -> Result<impl futures::Stream<Item = String>> {
        use futures::stream::{self, StreamExt};

        // Execute command and stream output
        let result = self.execute_sync(command, args).await?;

        // Convert output lines to stream
        let lines: Vec<String> = result.stdout.lines().chain(result.stderr.lines())
            .map(|s| s.to_string())
            .collect();

        Ok(stream::iter(lines))
    }

    /// Check if command exists
    pub async fn command_exists(&self, command: &str) -> bool {
        let shell = self.shell.lock().await;

        let output = AsyncCommand::new(&*shell)
            .arg("-c")
            .arg(format!("which {}", command))
            .output()
            .await;

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    /// Get environment variables
    pub async fn get_env_var(&self, key: &str) -> Option<String> {
        let shell = self.shell.lock().await;

        let output = AsyncCommand::new(&*shell)
            .arg("-c")
            .arg(format!("echo -n ${}", key))
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                Some(String::from_utf8_lossy(&o.stdout).to_string())
            }
            _ => None,
        }
    }

    /// Set environment variable
    pub async fn set_env_var(&self, key: &str, value: &str) -> Result<()> {
        let shell = self.shell.lock().await;

        let output = AsyncCommand::new(&*shell)
            .arg("-c")
            .arg(format!("export {}={}", key, value))
            .status()
            .await;

        match output {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Failed to set env var: {}", e)),
        }
    }

    /// Get current working directory
    pub async fn current_dir(&self) -> Result<String> {
        self.get_env_var("PWD")
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to get current directory"))
    }

    /// Change directory
    pub async fn change_dir(&self, path: &str) -> Result<()> {
        let shell = self.shell.lock().await;

        let output = AsyncCommand::new(&*shell)
            .arg("-c")
            .arg(format!("cd {}", path))
            .status()
            .await;

        match output {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => Err(anyhow::anyhow!("Failed to change directory to {}", path)),
            Err(e) => Err(anyhow::anyhow!("Command error: {}", e)),
        }
    }

    /// Execute Termux-specific API commands
    pub async fn execute_termux_api(&self, command: &str, args: &[&str]) -> Result<String> {
        let full_cmd = format!("termux-{} {}", command, args.join(" "));
        let result = self.execute_sync(&full_cmd, &[]).await?;

        if result.success {
            Ok(result.stdout)
        } else {
            Err(anyhow::anyhow!("Termux API command failed: {}", result.stderr))
        }
    }

    /// Get system information
    pub async fn get_system_info(&self) -> Result<SystemInfo> {
        let mut info = SystemInfo::default();

        // Get Android version
        if let Ok(version) = self.execute_termux_api("battery-status", &[]).await {
            info.android_version = "11".to_string(); // Would parse from actual output
        }

        // Get battery info
        if let Ok(battery) = self.execute_termux_api("battery-status", &[]).await {
            // Parse battery info
            info.battery_level = 85; // Would parse from actual output
        }

        // Get WiFi info
        if let Ok(wifi) = self.execute_termux_api("wifi-connectioninfo", &[]).await {
            info.wifi_connected = !wifi.is_empty();
        }

        Ok(info)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub android_version: String,
    pub battery_level: i32,
    pub wifi_connected: bool,
    pub storage_free: u64,
    pub storage_total: u64,
}