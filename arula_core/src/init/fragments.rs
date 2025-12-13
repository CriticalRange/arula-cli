//! Project manifest fragment definitions
//!
//! Data structures for generating a single AI-readable PROJECT.manifest file
//! that provides quick project understanding.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Project metadata for the manifest
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub project_type: String,
    pub language: String,
    pub framework: String,
    pub created: String,
    pub last_updated: String,
}

/// Project essence - the TL;DR for AI
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectEssence {
    pub purpose: String,
    pub architecture: String,
    pub key_technologies: Vec<String>,
}

/// Project structure information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectStructure {
    pub core_components: Vec<(String, String)>, // (name, description)
    pub key_files: Vec<(String, String)>,        // (path, purpose)
    pub entry_points: HashMap<String, String>,   // (type, path)
}

/// Patterns and conventions used in the project
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectPatterns {
    pub naming: NamingConventions,
    pub architecture_patterns: Vec<(String, String)>, // (pattern, where_used)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NamingConventions {
    pub files: String,
    pub functions: String,
    pub variables: String,
}

/// Dependencies and requirements
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectDependencies {
    pub external_libraries: Vec<(String, String)>, // (library, purpose)
    pub system_requirements: Vec<(String, String)>, // (requirement, details)
}

/// Workflow information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectWorkflow {
    pub run_command: String,
    pub test_command: String,
    pub build_command: String,
}

/// Decision log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    pub date: String,
    pub title: String,
    pub context: String,
    pub result: String,
}

/// AI assistance notes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AIAssistanceNotes {
    pub common_tasks: Vec<(String, String)>, // (task, approach)
    pub gotchas: Vec<(String, String)>,      // (pitfall, avoidance)
    pub recent_changes: Vec<String>,         // (change descriptions with dates embedded)
}

/// Complete project manifest
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectManifest {
    pub version: String,
    pub metadata: ProjectMetadata,
    pub essence: ProjectEssence,
    pub structure: ProjectStructure,
    pub patterns: ProjectPatterns,
    pub dependencies: ProjectDependencies,
    pub workflow: ProjectWorkflow,
    pub decision_log: Vec<DecisionEntry>,
    pub todo_future: TodoFuture,
    pub ai_notes: AIAssistanceNotes,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoFuture {
    pub immediate: Vec<String>,
    pub considered: Vec<(String, String)>, // (feature, note)
}

/// Generated manifest content
#[derive(Debug, Clone)]
pub struct ManifestContent {
    pub content: String,
    pub file_path: String,
}

// Backward compatibility types for pipeline
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectContext {
    pub purpose: String,
    pub problem_domain: String,
    pub user_goals: Vec<String>,
    pub business_value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchitectureFragment {
    pub patterns: Vec<String>,
    pub components: Vec<String>,
    pub technologies: Vec<String>,
    pub integrations: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequirementsFragment {
    pub functional: Vec<String>,
    pub non_functional: Vec<String>,
    pub constraints: std::collections::HashMap<String, String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurrentStateFragment {
    pub existing_code: Vec<String>,
    pub dependencies: Vec<String>,
    pub pain_points: Vec<String>,
    pub recent_changes: Vec<String>,
}

/// Backward compatibility type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUnderstanding {
    pub context: ProjectContext,
    pub architecture: ArchitectureFragment,
    pub requirements: RequirementsFragment,
    pub current_state: CurrentStateFragment,
}

impl ProjectManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.essence.purpose.is_empty() {
            return Err("Project purpose is required in the manifest".to_string());
        }
        if self.metadata.name.is_empty() {
            return Err("Project name is required in the manifest".to_string());
        }
        Ok(())
    }

    /// Update last_updated timestamp
    pub fn touch(&mut self) {
        use chrono::Utc;
        self.metadata.last_updated = Utc::now().format("%Y-%m-%d").to_string();
    }

    /// Add a recent change entry
    pub fn add_recent_change(&mut self, change: &str) {
        use chrono::Utc;
        let date = Utc::now().format("%Y-%m-%d").to_string();
        self.ai_notes.recent_changes.insert(0, format!("[{}] {}", date, change));

        // Keep only last 10 changes
        self.ai_notes.recent_changes.truncate(10);
    }

    /// Add a decision to the log
    pub fn add_decision(&mut self, title: String, context: String, result: String) {
        use chrono::Utc;
        let entry = DecisionEntry {
            date: Utc::now().format("%Y-%m-%d").to_string(),
            title,
            context,
            result,
        };
        self.decision_log.insert(0, entry);

        // Keep only last 20 decisions
        self.decision_log.truncate(20);
    }
}