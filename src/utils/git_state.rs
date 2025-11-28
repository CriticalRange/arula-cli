//! Git state tracking and restoration utilities
//!
//! This module provides functionality to track and restore git state,
//! particularly useful for AI agent interactions that may change branches.

use std::path::Path;
use std::fs;
use anyhow::Result;
use tokio::process::Command as TokioCommand;

/// Tracks git state for restoration
pub struct GitStateTracker {
    original_branch: Option<String>,
    working_directory: String,
}

impl GitStateTracker {
    /// Create a new git state tracker for the given directory
    pub fn new<P: AsRef<Path>>(working_dir: P) -> Self {
        Self {
            original_branch: None,
            working_directory: working_dir.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Save the current git branch to both memory and disk
    pub async fn save_current_branch(&mut self) -> Result<()> {
        // Check if we're in a git repository
        let is_git_repo = self.check_is_git_repo().await?;
        if !is_git_repo {
            return Ok(()); // Not a git repo, nothing to save
        }

        // Get current branch
        let output = TokioCommand::new("git")
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .current_dir(&self.working_directory)
            .output()
            .await;

        match output {
            Ok(result) if result.status.success() => {
                let branch = String::from_utf8_lossy(&result.stdout).trim().to_string();
                if !branch.is_empty() && branch != "HEAD" {
                    self.original_branch = Some(branch.clone());

                    // Also save to disk for crash recovery
                    self.save_branch_to_disk(&branch).await?;
                }
            }
            Ok(result) => {
                let stderr = String::from_utf8_lossy(&result.stderr);
                eprintln!("⚠️ GitState: Failed to get current branch: {}", stderr);
            }
            Err(e) => {
                eprintln!("⚠️ GitState: Error running git command: {}", e);
            }
        }

        Ok(())
    }

    /// Save branch name to a temporary file for crash recovery
    async fn save_branch_to_disk(&self, branch: &str) -> Result<()> {
        if let Ok(home_dir) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            let state_file = std::path::Path::new(&home_dir)
                .join(".arula")
                .join("git_state.json");

            if let Some(parent) = state_file.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let state_data = serde_json::json!({
                "original_branch": branch,
                "working_directory": self.working_directory,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            if let Ok(content) = serde_json::to_string_pretty(&state_data) {
                if let Err(e) = fs::write(&state_file, content) {
                    eprintln!("⚠️ GitState: Failed to save branch state to disk: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Load branch from disk for crash recovery
    pub async fn load_branch_from_disk(&mut self) -> Result<Option<String>> {
        if let Ok(home_dir) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            let state_file = std::path::Path::new(&home_dir)
                .join(".arula")
                .join("git_state.json");

            if state_file.exists() {
                if let Ok(content) = fs::read_to_string(&state_file) {
                    if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(branch) = state.get("original_branch").and_then(|v| v.as_str()) {
                            if let Some(working_dir) = state.get("working_directory").and_then(|v| v.as_str()) {
                                // Only use if working directory matches or if we can't verify
                                if working_dir == self.working_directory {
                                    self.original_branch = Some(branch.to_string());
                                    eprintln!("🔧 GitState: Loaded saved branch from disk: {:?}", branch);
                                    return Ok(Some(branch.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// Restore the original git branch
    pub async fn restore_original_branch(&self) -> Result<()> {
        if let Some(ref original_branch) = self.original_branch {
            eprintln!("🔧 GitState: Restoring branch to '{}'", original_branch);

            // Get current branch to check if we need to switch
            let current_branch = self.get_current_branch().await?;

            if current_branch.as_ref() != Some(original_branch) {
                // Switch back to original branch
                let output = TokioCommand::new("git")
                    .arg("checkout")
                    .arg(original_branch)
                    .current_dir(&self.working_directory)
                    .output()
                    .await;

                match output {
                    Ok(result) if result.status.success() => {
                        eprintln!("✅ GitState: Successfully restored branch to '{}'", original_branch);
                    }
                    Ok(result) => {
                        let stderr = String::from_utf8_lossy(&result.stderr);
                        eprintln!("❌ GitState: Failed to restore branch '{}': {}", original_branch, stderr);
                    }
                    Err(e) => {
                        eprintln!("❌ GitState: Error restoring branch '{}': {}", original_branch, e);
                    }
                }
            } else {
                eprintln!("✅ GitState: Already on correct branch '{}'", original_branch);
            }
        }

        // Clean up saved state file after successful restoration
        self.cleanup_saved_state().await?;

        Ok(())
    }

    /// Clean up the saved state file after restoration
    async fn cleanup_saved_state(&self) -> Result<()> {
        if let Ok(home_dir) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            let state_file = std::path::Path::new(&home_dir)
                .join(".arula")
                .join("git_state.json");

            if state_file.exists() {
                if let Err(e) = fs::remove_file(&state_file) {
                    eprintln!("⚠️ GitState: Failed to cleanup saved state file: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Check if the current directory is a git repository
    async fn check_is_git_repo(&self) -> Result<bool> {
        let output = TokioCommand::new("git")
            .arg("rev-parse")
            .arg("--git-dir")
            .current_dir(&self.working_directory)
            .output()
            .await;

        match output {
            Ok(result) => Ok(result.status.success()),
            Err(_) => Ok(false),
        }
    }

    /// Get the current git branch
    async fn get_current_branch(&self) -> Result<Option<String>> {
        let output = TokioCommand::new("git")
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .current_dir(&self.working_directory)
            .output()
            .await;

        match output {
            Ok(result) if result.status.success() => {
                let branch = String::from_utf8_lossy(&result.stdout).trim().to_string();
                if !branch.is_empty() && branch != "HEAD" {
                    Ok(Some(branch))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Get the saved original branch
    pub fn get_saved_branch(&self) -> Option<&str> {
        self.original_branch.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_git_state_tracker_new_repo() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut tracker = GitStateTracker::new(temp_dir.path());

        // Should not fail even if not a git repo
        assert!(tracker.save_current_branch().await.is_ok());
        assert!(tracker.restore_original_branch().await.is_ok());

        Ok(())
    }
}