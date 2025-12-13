//! Project manifest system for AI quick understanding
//!
//! This module creates and maintains a PROJECT.manifest file that provides
//! AI with a single-file overview of the entire project.

use crate::api::agent_client::AgentClient;
use crate::utils::config::Config;
use anyhow::Result;
use std::fs;
use std::path::Path;
use std::fmt::Write;

pub mod example;
pub mod fragments;
pub mod pipeline;
pub mod report_generator;

pub use example::*;
pub use fragments::*;
pub use pipeline::*;
pub use report_generator::*;

/// Project manifest system orchestrator
#[derive(Clone)]
pub struct ProjectManifestSystem {
    agent_client: AgentClient,
    config: Config,
}

impl ProjectManifestSystem {
    pub fn new(agent_client: AgentClient, config: Config) -> Self {
        Self { agent_client, config }
    }

    /// Create or update PROJECT.manifest for quick AI understanding
    pub async fn create_or_update_manifest(
        &self,
        project_path: &str,
        project_description: Option<&str>
    ) -> Result<ProjectManifest> {
        // Check if manifest already exists
        let manifest_path = Path::new(project_path).join("PROJECT.manifest");

        if manifest_path.exists() {
            // Load existing manifest
            let content = fs::read_to_string(&manifest_path)?;
            let mut manifest = self.parse_manifest(&content)?;

            // Update it with new understanding
            manifest.touch();
            self.enhance_manifest(&mut manifest, project_path, project_description).await?;

            // Save updated manifest
            self.save_manifest(&manifest, &manifest_path)?;
            Ok(manifest)
        } else {
            // Create new manifest
            let pipeline = ProjectLearningPipeline::new(self.agent_client.clone());
            let initial_desc = project_description.unwrap_or("New project");

            // Learn about the project
            let context = pipeline.learn_context(initial_desc).await?;

            // Create a basic manifest from context
            let manifest = ProjectManifest {
                version: "1.0".to_string(),
                metadata: ProjectMetadata {
                    name: extract_project_name(&context.purpose),
                    project_type: "auto".to_string(),
                    language: "auto".to_string(),
                    framework: "auto".to_string(),
                    created: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                    last_updated: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                },
                essence: ProjectEssence {
                    purpose: context.purpose.clone(),
                    architecture: context.problem_domain.clone(),
                    key_technologies: vec![],
                },
                structure: ProjectStructure::default(),
                patterns: ProjectPatterns::default(),
                dependencies: ProjectDependencies::default(),
                workflow: ProjectWorkflow {
                    run_command: "auto".to_string(),
                    test_command: "auto".to_string(),
                    build_command: "auto".to_string(),
                },
                decision_log: vec![],
                todo_future: TodoFuture {
                    immediate: vec![],
                    considered: vec![],
                },
                ai_notes: AIAssistanceNotes::default(),
            };
            let manifest_content = self.format_manifest_simple(&manifest)?;

            // Save the manifest
            fs::write(&manifest_path, manifest_content)?;
            Ok(manifest)
        }
    }

    /// Get quick project summary from manifest
    pub fn get_quick_summary(&self, project_path: &str) -> Result<String> {
        let manifest_path = Path::new(project_path).join("PROJECT.manifest");

        if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path)?;
            let manifest = self.parse_manifest(&content)?;

            Ok(format!(
                "Project: {}\nPurpose: {}\nArchitecture: {}\nTechnologies: {}",
                manifest.metadata.name,
                manifest.essence.purpose,
                manifest.essence.architecture,
                manifest.essence.key_technologies.join(", ")
            ))
        } else {
            Err(anyhow::anyhow!("No PROJECT.manifest found"))
        }
    }

    /// Enhance existing manifest with current project state
    async fn enhance_manifest(
        &self,
        manifest: &mut ProjectManifest,
        project_path: &str,
        project_description: Option<&str>
    ) -> Result<()> {
        let pipeline = ProjectLearningPipeline::new(self.agent_client.clone());

        // Assess current state
        let current_state = pipeline.assess_current_state(project_path).await?;

        // Update manifest with new information
        if !current_state.existing_code.is_empty() {
            manifest.structure.key_files = current_state.existing_code
                .iter()
                .map(|f| (f.clone(), "Detected file".to_string()))
                .collect();
        }

        if !current_state.dependencies.is_empty() {
            manifest.dependencies.external_libraries = current_state.dependencies
                .iter()
                .map(|d| (d.clone(), "Detected dependency".to_string()))
                .collect();
        }

        if let Some(desc) = project_description {
            manifest.essence.purpose = desc.to_string();
        }

        Ok(())
    }

    /// Parse manifest from text
    fn parse_manifest(&self, content: &str) -> Result<ProjectManifest> {
        // Simple parsing - in production would use a proper parser
        let mut manifest = ProjectManifest::default();

        // Extract basic information
        for line in content.lines() {
            if line.starts_with("name: ") {
                manifest.metadata.name = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.starts_with("purpose: ") {
                manifest.essence.purpose = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.starts_with("architecture: ") {
                manifest.essence.architecture = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.starts_with("key_technologies: ") {
                let tech_str = line.split(':').nth(1).unwrap_or("");
                manifest.essence.key_technologies = tech_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
            }
        }

        Ok(manifest)
    }

    /// Save manifest to file
    fn save_manifest(&self, manifest: &ProjectManifest, path: &Path) -> Result<()> {
        let generator = ManifestGenerator::new();
        let content = generator.format_manifest(manifest)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Generate analysis report from project understanding
    pub fn generate_analysis_report(&self, understanding: &ProjectUnderstanding) -> Result<AnalysisReport> {
        let generator = ManifestGenerator::new();
        generator.generate(understanding)
    }

    /// Learn about a project through discovery and analysis
    pub async fn learn_about_project(
        &self,
        initial_understanding: &str,
        project_path: &str
    ) -> Result<ProjectUnderstanding> {
        let pipeline = ProjectLearningPipeline::new(self.agent_client.clone());

        // Execute learning pipeline steps
        let context = pipeline.learn_context(initial_understanding).await?;
        let architecture = pipeline.discover_architecture(&context).await?;
        let requirements = pipeline.identify_requirements(&context, &architecture).await?;
        let current_state = pipeline.assess_current_state(project_path).await?;

        // Assemble project understanding
        let understanding = ProjectUnderstanding {
            context,
            architecture,
            requirements,
            current_state,
        };

        Ok(understanding)
    }

    /// Simple format manifest without using the generator
    fn format_manifest_simple(&self, manifest: &ProjectManifest) -> Result<String> {
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

        Ok(output)
    }
}

/// Extract project name from purpose description
fn extract_project_name(purpose: &str) -> String {
    let words: Vec<&str> = purpose.split_whitespace().collect();
    if words.len() >= 3 {
        format!("{}-{}-{}", words[0], words[1], words[2])
    } else {
        "untitled-project".to_string()
    }
}

// Maintain backward compatibility with old names
pub type InitSystem = ProjectManifestSystem;
pub type ProjectLearningSystem = ProjectManifestSystem;
pub type InitPipeline = ProjectLearningPipeline;
pub type ProjectBlueprint = ProjectManifest;
pub type AnalysisReport = ManifestContent;
pub type SbpFiles = ManifestContent;
pub type SbpAssembler = ManifestGenerator;