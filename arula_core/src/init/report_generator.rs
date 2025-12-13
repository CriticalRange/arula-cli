//! Project manifest generator
//!
//! Creates a single PROJECT.manifest file that provides AI with quick project understanding.

use crate::init::fragments::*;
use anyhow::Result;
use std::fmt::Write;
use std::collections::HashMap;

/// Generates PROJECT.manifest from project understanding
pub struct ManifestGenerator;

impl ManifestGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Convert understanding to manifest content
    pub fn generate(&self, understanding: &ProjectUnderstanding) -> Result<ManifestContent> {
        // For now, create a basic manifest from the understanding
        // In a real implementation, this would be more sophisticated
        let manifest = ProjectManifest {
            version: "1.0".to_string(),
            metadata: ProjectMetadata {
                name: extract_project_name(&understanding.context.purpose),
                project_type: "auto".to_string(),
                language: "auto".to_string(),
                framework: "auto".to_string(),
                created: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                last_updated: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            },
            essence: ProjectEssence {
                purpose: understanding.context.purpose.clone(),
                architecture: format!(
                    "Components: {}, Technologies: {}",
                    understanding.architecture.components.len(),
                    understanding.architecture.technologies.len()
                ),
                key_technologies: understanding.architecture.technologies.clone(),
            },
            structure: ProjectStructure {
                core_components: understanding.architecture.components
                    .iter()
                    .map(|c| (c.clone(), "Core component".to_string()))
                    .collect(),
                key_files: understanding.current_state.existing_code
                    .iter()
                    .map(|f| (f.clone(), "Key file".to_string()))
                    .collect(),
                entry_points: HashMap::new(),
            },
            patterns: ProjectPatterns {
                naming: NamingConventions {
                    files: "auto".to_string(),
                    functions: "auto".to_string(),
                    variables: "auto".to_string(),
                },
                architecture_patterns: understanding.architecture.patterns
                    .iter()
                    .map(|p| (p.clone(), "Used throughout".to_string()))
                    .collect(),
            },
            dependencies: ProjectDependencies {
                external_libraries: understanding.current_state.dependencies
                    .iter()
                    .map(|d| (d.clone(), "Dependency".to_string()))
                    .collect(),
                system_requirements: vec![],
            },
            workflow: ProjectWorkflow {
                run_command: "auto".to_string(),
                test_command: "auto".to_string(),
                build_command: "auto".to_string(),
            },
            decision_log: vec![],
            todo_future: TodoFuture {
                immediate: understanding.requirements.functional.clone(),
                considered: vec![],
            },
            ai_notes: AIAssistanceNotes {
                common_tasks: vec![],
                gotchas: understanding.current_state.pain_points
                    .iter()
                    .map(|p| (p.clone(), "Known issue".to_string()))
                    .collect(),
                recent_changes: understanding.current_state.recent_changes.clone(),
            },
        };

        let content = self.format_manifest(&manifest)?;

        Ok(ManifestContent {
            content,
            file_path: "PROJECT.manifest".to_string(),
        })
    }

    /// Format manifest as text
    pub fn format_manifest(&self, manifest: &ProjectManifest) -> Result<String> {
        let mut output = String::new();

        // Header
        writeln!(output, "PROJECT_MANIFEST v{}", manifest.version)?;
        writeln!(output)?;

        // Metadata
        writeln!(output, "# METADATA")?;
        writeln!(output, "name: {}", manifest.metadata.name)?;
        writeln!(output, "type: {}", manifest.metadata.project_type)?;
        writeln!(output, "language: {}", manifest.metadata.language)?;
        writeln!(output, "framework: {}", manifest.metadata.framework)?;
        writeln!(output, "created: {}", manifest.metadata.created)?;
        writeln!(output, "last_updated: {}", manifest.metadata.last_updated)?;
        writeln!(output)?;

        // Essence
        writeln!(output, "# ESSENCE (TL;DR for AI)")?;
        writeln!(output, "purpose: {}", manifest.essence.purpose)?;
        writeln!(output, "architecture: {}", manifest.essence.architecture)?;
        writeln!(output, "key_technologies: {}", manifest.essence.key_technologies.join(", "))?;
        writeln!(output)?;

        // Structure
        if !manifest.structure.core_components.is_empty() || !manifest.structure.key_files.is_empty() {
            writeln!(output, "# STRUCTURE")?;
            if !manifest.structure.core_components.is_empty() {
                writeln!(output, "## Core Components")?;
                for (name, desc) in &manifest.structure.core_components {
                    writeln!(output, "- {}: {}", name, desc)?;
                }
            }
            if !manifest.structure.key_files.is_empty() {
                writeln!(output, "## Key Files")?;
                for (path, purpose) in &manifest.structure.key_files {
                    writeln!(output, "- {}: {}", path, purpose)?;
                }
            }
            writeln!(output)?;
        }

        // AI Notes
        if !manifest.ai_notes.gotchas.is_empty() || !manifest.ai_notes.recent_changes.is_empty() {
            writeln!(output, "# AI ASSISTANCE NOTES")?;
            if !manifest.ai_notes.gotchas.is_empty() {
                writeln!(output, "## Gotchas")?;
                for (pitfall, avoidance) in &manifest.ai_notes.gotchas {
                    writeln!(output, "- {}: {}", pitfall, avoidance)?;
                }
            }
            if !manifest.ai_notes.recent_changes.is_empty() {
                writeln!(output, "## Recent Changes")?;
                for change in &manifest.ai_notes.recent_changes {
                    writeln!(output, "- {}", change)?;
                }
            }
            writeln!(output)?;
        }

        // TODO
        if !manifest.todo_future.immediate.is_empty() {
            writeln!(output, "# TODO & FUTURE")?;
            writeln!(output, "## Immediate")?;
            for task in &manifest.todo_future.immediate {
                writeln!(output, "- {}", task)?;
            }
            writeln!(output)?;
        }

        Ok(output)
    }
}

/// Extract project name from purpose description
fn extract_project_name(purpose: &str) -> String {
    // Simple extraction - in real implementation would be more sophisticated
    let words: Vec<&str> = purpose.split_whitespace().collect();
    if words.len() >= 3 {
        format!("{}-{}-{}", words[0], words[1], words[2])
    } else {
        "untitled-project".to_string()
    }
}