//! Project analysis tool that builds a quick mental model of the repository.
//! Focuses on light-weight heuristics to avoid heavy I/O while still returning
//! a structured summary for the agent.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use async_trait::async_trait;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

/// Parameters for the analyze_context tool.
#[derive(Debug, Deserialize)]
pub struct AnalyzeContextParams {
    /// Root directory to analyze (defaults to current directory).
    pub root_path: Option<String>,
    /// Maximum number of files to scan (defaults to 500).
    pub max_files: Option<usize>,
    /// Include hidden files and common build directories.
    pub include_hidden: Option<bool>,
}

/// Entry point discovered in the project.
#[derive(Debug, Clone, Serialize)]
pub struct EntryPoint {
    pub path: String,
    pub kind: String,
    pub description: Option<String>,
}

/// Dependency highlights by ecosystem.
#[derive(Debug, Clone, Serialize, Default)]
pub struct DependencyHighlights {
    pub rust: Vec<String>,
    pub node: Vec<String>,
    pub python: Vec<String>,
}

/// Structured output for project analysis.
#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeContextResult {
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub build_systems: Vec<String>,
    pub project_type: Option<String>,
    pub entry_points: Vec<EntryPoint>,
    pub dependencies: DependencyHighlights,
    pub summary: String,
    pub cached: bool,
}

struct CacheEntry {
    signature: u64,
    captured_at: Instant,
    result: AnalyzeContextResult,
}

static ANALYSIS_CACHE: OnceCell<Mutex<HashMap<PathBuf, CacheEntry>>> = OnceCell::new();

fn cache() -> &'static Mutex<HashMap<PathBuf, CacheEntry>> {
    ANALYSIS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Analyze the repository structure and return a structured summary.
pub struct AnalyzeContextTool;

impl AnalyzeContextTool {
    pub fn new() -> Self {
        Self
    }

    fn should_skip_dir(name: &str) -> bool {
        matches!(
            name,
            ".git"
                | "target"
                | "node_modules"
                | ".idea"
                | ".vscode"
                | ".venv"
                | "__pycache__"
                | "dist"
                | "build"
                | "out"
        )
    }

    fn collect_files(
        &self,
        root: &Path,
        max_files: usize,
        include_hidden: bool,
    ) -> Result<(Vec<PathBuf>, u64), String> {
        let mut files = Vec::new();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        let walker = WalkDir::new(root).follow_links(false).into_iter();
        for entry in walker {
            let entry = entry.map_err(|e| format!("Failed to walk directory: {e}"))?;
            let path = entry.path();
            let name = entry
                .file_name()
                .to_str()
                .unwrap_or_default()
                .to_string();

            if entry.file_type().is_dir() {
                if !include_hidden && name.starts_with('.') {
                    continue;
                }
                if Self::should_skip_dir(&name) {
                    continue;
                }
            } else if entry.file_type().is_file() {
                if !include_hidden && name.starts_with('.') {
                    continue;
                }
                files.push(path.to_path_buf());
                if files.len() >= max_files {
                    break;
                }
            }
        }

        // Build a lightweight signature from file paths + modified times.
        files.iter().for_each(|p| {
            p.hash(&mut hasher);
            if let Ok(meta) = fs::metadata(p) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(duration) = modified.elapsed() {
                        duration.as_secs().hash(&mut hasher);
                    }
                }
                meta.len().hash(&mut hasher);
            }
        });

        Ok((files, hasher.finish()))
    }

    fn detect_languages(&self, files: &[PathBuf]) -> Vec<String> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for path in files {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let lang = match ext.to_lowercase().as_str() {
                    "rs" => "rust",
                    "ts" | "tsx" => "typescript",
                    "js" | "jsx" => "javascript",
                    "py" => "python",
                    "go" => "go",
                    "rb" => "ruby",
                    "java" => "java",
                    "kt" => "kotlin",
                    "swift" => "swift",
                    "c" | "h" => "c",
                    "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
                    "md" => "markdown",
                    "toml" => "toml",
                    "json" => "json",
                    _ => continue,
                };
                *counts.entry(lang).or_default() += 1;
            }
        }

        let mut languages: Vec<_> = counts.into_iter().collect();
        languages.sort_by(|a, b| b.1.cmp(&a.1));
        languages.into_iter().map(|(lang, _)| lang.to_string()).collect()
    }

    fn load_toml(path: &Path) -> Option<toml::Value> {
        let data = fs::read_to_string(path).ok()?;
        toml::from_str(&data).ok()
    }

    fn load_json(path: &Path) -> Option<serde_json::Value> {
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn detect_build_systems(&self, root: &Path) -> Vec<String> {
        let mut systems = HashSet::new();
        if root.join("Cargo.toml").exists() {
            systems.insert("cargo".to_string());
        }
        if root.join("package.json").exists() {
            systems.insert("node".to_string());
        }
        if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
            systems.insert("python".to_string());
        }
        if root.join("Makefile").exists() {
            systems.insert("make".to_string());
        }
        systems.into_iter().collect()
    }

    fn detect_frameworks(
        &self,
        cargo: &Option<toml::Value>,
        package: &Option<serde_json::Value>,
        requirements: &Option<HashSet<String>>,
    ) -> Vec<String> {
        let mut frameworks = HashSet::new();

        if let Some(cargo_toml) = cargo {
            let deps = cargo_toml.get("dependencies").and_then(|d| d.as_table());
            let dep_has = |name: &str| deps.map(|t| t.contains_key(name)).unwrap_or(false);
            if dep_has("iced") {
                frameworks.insert("iced".to_string());
            }
            if dep_has("actix-web") || dep_has("actix") {
                frameworks.insert("actix".to_string());
            }
            if dep_has("tauri") {
                frameworks.insert("tauri".to_string());
            }
            if dep_has("clap") {
                frameworks.insert("clap".to_string());
            }
        }

        if let Some(pkg) = package {
            let deps = pkg
                .get("dependencies")
                .and_then(|d| d.as_object())
                .cloned()
                .unwrap_or_default();
            let dev = pkg
                .get("devDependencies")
                .and_then(|d| d.as_object())
                .cloned()
                .unwrap_or_default();
            let has = |key: &str| deps.contains_key(key) || dev.contains_key(key);
            if has("react") {
                frameworks.insert("react".to_string());
            }
            if has("next") {
                frameworks.insert("next.js".to_string());
            }
            if has("vue") {
                frameworks.insert("vue".to_string());
            }
            if has("vite") {
                frameworks.insert("vite".to_string());
            }
        }

        if let Some(reqs) = requirements {
            let has = |key: &str| reqs.contains(key);
            if has("django") {
                frameworks.insert("django".to_string());
            }
            if has("fastapi") {
                frameworks.insert("fastapi".to_string());
            }
            if has("flask") {
                frameworks.insert("flask".to_string());
            }
        }

        let mut list: Vec<_> = frameworks.into_iter().collect();
        list.sort();
        list
    }

    fn detect_requirements(&self, root: &Path) -> Option<HashSet<String>> {
        let req_path = root.join("requirements.txt");
        let mut set = HashSet::new();
        if req_path.exists() {
            if let Ok(data) = fs::read_to_string(&req_path) {
                for line in data.lines() {
                    let trimmed = line.split('#').next().unwrap_or("").trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let name = trimmed
                        .split(['=', '>', '<', ' '])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();
                    if !name.is_empty() {
                        set.insert(name);
                    }
                }
            }
        }
        if set.is_empty() {
            None
        } else {
            Some(set)
        }
    }

    fn detect_entry_points(&self, root: &Path, files: &[PathBuf]) -> Vec<EntryPoint> {
        let mut points = Vec::new();
        for path in files {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let kind = if name == "main.rs" {
                    Some("rust_bin")
                } else if path.starts_with(root.join("src/bin")) {
                    Some("rust_bin")
                } else if name == "main.py" || name == "manage.py" {
                    Some("python")
                } else if name == "index.tsx" || name == "index.ts" || name == "index.js" {
                    Some("node")
                } else {
                    None
                };
                if let Some(kind) = kind {
                    points.push(EntryPoint {
                        path: path.strip_prefix(root).unwrap_or(path).to_string_lossy().into(),
                        kind: kind.to_string(),
                        description: None,
                    });
                }
            }
        }

        // Cargo binaries listed in Cargo.toml
        if let Some(cargo) = Self::load_toml(&root.join("Cargo.toml")) {
            if let Some(bins) = cargo.get("bin").and_then(|b| b.as_array()) {
                for bin in bins {
                    if let Some(name) = bin.get("name").and_then(|n| n.as_str()) {
                        if let Some(path) = bin.get("path").and_then(|p| p.as_str()) {
                            points.push(EntryPoint {
                                path: path.to_string(),
                                kind: "rust_bin".to_string(),
                                description: Some(format!("binary target: {name}")),
                            });
                        }
                    }
                }
            }
        }

        points.sort_by(|a, b| a.path.cmp(&b.path));
        points
    }

    fn detect_dependencies(
        &self,
        cargo: &Option<toml::Value>,
        package: &Option<serde_json::Value>,
        requirements: &Option<HashSet<String>>,
    ) -> DependencyHighlights {
        let mut highlights = DependencyHighlights::default();

        if let Some(cargo_toml) = cargo {
            if let Some(deps) = cargo_toml.get("dependencies").and_then(|d| d.as_table()) {
                highlights
                    .rust
                    .extend(deps.keys().cloned().map(|k| k.to_string()));
            }
        }

        if let Some(pkg) = package {
            if let Some(deps) = pkg.get("dependencies").and_then(|d| d.as_object()) {
                highlights
                    .node
                    .extend(deps.keys().cloned().map(|k| k.to_string()));
            }
            if let Some(dev) = pkg.get("devDependencies").and_then(|d| d.as_object()) {
                highlights
                    .node
                    .extend(dev.keys().cloned().map(|k| k.to_string()));
            }
            highlights.node.sort();
            highlights.node.dedup();
        }

        if let Some(reqs) = requirements {
            let mut list: Vec<_> = reqs.iter().cloned().collect();
            list.sort();
            highlights.python = list;
        }

        highlights
    }

    fn detect_project_type(
        &self,
        root: &Path,
        frameworks: &[String],
        entry_points: &[EntryPoint],
    ) -> Option<String> {
        if root.join("Cargo.toml").exists() && root.join("package.json").exists() {
            return Some("fullstack".to_string());
        }
        if frameworks.iter().any(|f| f == "iced" || f == "tauri") {
            return Some("desktop".to_string());
        }
        if entry_points.iter().any(|e| e.kind == "rust_bin") {
            return Some("cli".to_string());
        }
        if frameworks.iter().any(|f| f == "react" || f == "next.js" || f == "vue") {
            return Some("web".to_string());
        }
        None
    }

    fn summarize(
        &self,
        languages: &[String],
        frameworks: &[String],
        build_systems: &[String],
        entry_points: &[EntryPoint],
    ) -> String {
        let mut parts = Vec::new();
        if !languages.is_empty() {
            parts.push(format!("Languages: {}", languages.join(", ")));
        }
        if !frameworks.is_empty() {
            parts.push(format!("Frameworks: {}", frameworks.join(", ")));
        }
        if !build_systems.is_empty() {
            parts.push(format!("Build: {}", build_systems.join(", ")));
        }
        if !entry_points.is_empty() {
            let eps: Vec<_> = entry_points
                .iter()
                .map(|e| format!("{} ({})", e.path, e.kind))
                .collect();
            parts.push(format!("Entry points: {}", eps.join(", ")));
        }
        if parts.is_empty() {
            "No notable project signals detected".to_string()
        } else {
            parts.join(" • ")
        }
    }
}

#[async_trait]
impl Tool for AnalyzeContextTool {
    type Params = AnalyzeContextParams;
    type Result = AnalyzeContextResult;

    fn name(&self) -> &str {
        "analyze_context"
    }

    fn description(&self) -> &str {
        "Analyze the current repository to summarize languages, frameworks, build systems, and entry points."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new(
            "analyze_context",
            "Analyze the repository and return a structured overview",
        )
        .param("root_path", "string")
        .description("root_path", "Root directory to analyze (default: current directory)")
        .param("max_files", "integer")
        .description("max_files", "Maximum number of files to scan (default: 500)")
        .param("include_hidden", "boolean")
        .description("include_hidden", "Include hidden files and build outputs (default: false)")
        .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let root = PathBuf::from(params.root_path.unwrap_or_else(|| ".".to_string()));
        let max_files = params.max_files.unwrap_or(500).max(10);
        let include_hidden = params.include_hidden.unwrap_or(false);

        if !root.exists() {
            return Err(format!("Path does not exist: {}", root.display()));
        }

        let (files, signature) = self.collect_files(&root, max_files, include_hidden)?;

        // Check cache
        if let Ok(guard) = cache().lock() {
            if let Some(entry) = guard.get(&root) {
                if entry.signature == signature && entry.captured_at.elapsed() < Duration::from_secs(120) {
                    let mut cached_result = entry.result.clone();
                    cached_result.cached = true;
                    return Ok(cached_result);
                }
            }
        }

        let cargo = Self::load_toml(&root.join("Cargo.toml"));
        let package = Self::load_json(&root.join("package.json"));
        let requirements = self.detect_requirements(&root);

        let languages = self.detect_languages(&files);
        let build_systems = self.detect_build_systems(&root);
        let frameworks = self.detect_frameworks(&cargo, &package, &requirements);
        let entry_points = self.detect_entry_points(&root, &files);
        let dependencies = self.detect_dependencies(&cargo, &package, &requirements);
        let project_type = self.detect_project_type(&root, &frameworks, &entry_points);
        let summary = self.summarize(&languages, &frameworks, &build_systems, &entry_points);

        let result = AnalyzeContextResult {
            languages,
            frameworks,
            build_systems,
            project_type,
            entry_points,
            dependencies,
            summary,
            cached: false,
        };

        if let Ok(mut guard) = cache().lock() {
            guard.insert(
                root.clone(),
                CacheEntry {
                    signature,
                    captured_at: Instant::now(),
                    result: result.clone(),
                },
            );
        }

        Ok(result)
    }
}

impl Default for AnalyzeContextTool {
    fn default() -> Self {
        Self::new()
    }
}
