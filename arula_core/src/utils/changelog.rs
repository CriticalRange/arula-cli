//! Changelog fetcher and parser for ARULA CLI
//!
//! Fetches changelog from remote git repository and displays recent changes

use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum ChangelogType {
    Release,
    Custom,
    Development,
}

#[derive(Debug, Clone)]
pub struct ChangelogEntry {
    pub title: String,
    pub changes: Vec<String>,
}

pub struct Changelog {
    pub changelog_type: ChangelogType,
    pub entries: Vec<ChangelogEntry>,
}

impl Changelog {
    /// Fetch changelog from remote git repository
    pub fn fetch_from_remote() -> Result<Self> {
        // Try to fetch from origin/main
        let output = Command::new("git")
            .args(["show", "origin/main:CHANGELOG.md"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let content = String::from_utf8_lossy(&output.stdout).to_string();
                Ok(Self::parse(&content))
            }
            _ => {
                // Fallback to local CHANGELOG.md if git fetch fails
                Self::fetch_local()
            }
        }
    }

    /// Fetch changelog from local file
    pub fn fetch_local() -> Result<Self> {
        let content =
            std::fs::read_to_string("CHANGELOG.md").unwrap_or_else(|_| Self::default_changelog());
        Ok(Self::parse(&content))
    }

    /// Parse changelog content
    pub fn parse(content: &str) -> Self {
        let mut changelog_type = ChangelogType::Development;
        let mut entries = Vec::new();
        let mut current_entry: Option<ChangelogEntry> = None;
        let mut in_unreleased = false;

        for line in content.lines() {
            // Detect changelog type from header comment
            if line.contains("<!-- type:") {
                if line.contains("release") {
                    changelog_type = ChangelogType::Release;
                } else if line.contains("custom") {
                    changelog_type = ChangelogType::Custom;
                }
                continue;
            }

            // Skip until we find [Unreleased] section
            if line.contains("## [Unreleased]") {
                in_unreleased = true;
                continue;
            }

            // Stop at next version section
            if in_unreleased && line.starts_with("## [") && !line.contains("[Unreleased]") {
                break;
            }

            if !in_unreleased {
                continue;
            }

            // Parse section headers (### Added, ### Changed, etc.)
            if line.starts_with("### ") {
                // Save previous entry if exists
                if let Some(entry) = current_entry.take() {
                    if !entry.changes.is_empty() {
                        entries.push(entry);
                    }
                }

                let title = line.trim_start_matches("### ").trim().to_string();
                current_entry = Some(ChangelogEntry {
                    title,
                    changes: Vec::new(),
                });
                continue;
            }

            // Parse bullet points
            if let Some(ref mut entry) = current_entry {
                if line.starts_with("- ") {
                    let change = line.trim_start_matches("- ").trim().to_string();
                    if !change.is_empty() {
                        entry.changes.push(change);
                    }
                }
            }
        }

        // Add last entry
        if let Some(entry) = current_entry {
            if !entry.changes.is_empty() {
                entries.push(entry);
            }
        }

        Self {
            changelog_type,
            entries,
        }
    }

    /// Get default changelog when file doesn't exist
    pub fn default_changelog() -> String {
        r#"# Changelog
<!-- type: development -->

## [Unreleased]

### Added
- ARULA CLI development build
"#
        .to_string()
    }

    /// Get recent changes for display (limit to first N items)
    pub fn get_recent_changes(&self, max_items: usize) -> Vec<String> {
        let mut changes = Vec::new();
        let mut count = 0;

        for entry in &self.entries {
            if count >= max_items {
                break;
            }

            for change in &entry.changes {
                if count >= max_items {
                    break;
                }
                // Format with emoji based on section
                let emoji = match entry.title.as_str() {
                    "Added" => "✨",
                    "Changed" => "🔄",
                    "Fixed" => "🐛",
                    "Removed" => "🗑️",
                    "Security" => "🔒",
                    "Deprecated" => "⚠️",
                    _ => "•",
                };
                changes.push(format!("{} {}", emoji, change));
                count += 1;
            }
        }

        changes
    }

    /// Get changelog type label
    pub fn get_type_label(&self) -> &str {
        match self.changelog_type {
            ChangelogType::Release => "Release",
            ChangelogType::Custom => "Custom Build",
            ChangelogType::Development => "Development",
        }
    }

    /// Detect if this is a custom build by checking git remote
    pub fn detect_build_type() -> ChangelogType {
        // Check if git remote is the official repo
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let url = String::from_utf8_lossy(&output.stdout);

                // Check if it's the official ARULA repo
                if url.contains("arula-cli") || url.contains("official-arula-repo") {
                    return ChangelogType::Release;
                } else {
                    return ChangelogType::Custom;
                }
            }
        }

        ChangelogType::Development
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_changelog_type() {
        let content = r#"# Changelog
<!-- type: release -->

## [Unreleased]

### Added
- New feature
"#;
        let changelog = Changelog::parse(content);
        assert_eq!(changelog.changelog_type, ChangelogType::Release);
    }

    #[test]
    fn test_parse_custom_type() {
        let content = r#"# Changelog
<!-- type: custom -->

## [Unreleased]

### Changed
- Custom modification
"#;
        let changelog = Changelog::parse(content);
        assert_eq!(changelog.changelog_type, ChangelogType::Custom);
    }

    #[test]
    fn test_parse_entries() {
        let content = r#"# Changelog
<!-- type: release -->

## [Unreleased]

### Added
- Feature one
- Feature two

### Fixed
- Bug fix one

## [0.1.0] - 2025-01-01

### Added
- Initial release
"#;
        let changelog = Changelog::parse(content);
        assert_eq!(changelog.entries.len(), 2);
        assert_eq!(changelog.entries[0].title, "Added");
        assert_eq!(changelog.entries[0].changes.len(), 2);
        assert_eq!(changelog.entries[1].title, "Fixed");
        assert_eq!(changelog.entries[1].changes.len(), 1);
    }

    #[test]
    fn test_get_recent_changes() {
        let content = r#"# Changelog

## [Unreleased]

### Added
- Change 1
- Change 2
- Change 3

### Fixed
- Fix 1
- Fix 2
"#;
        let changelog = Changelog::parse(content);
        let recent = changelog.get_recent_changes(3);
        assert_eq!(recent.len(), 3);
        assert!(recent[0].contains("Change 1"));
    }
}
