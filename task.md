# ARULA Tasks

## Task: Implement `analyze_context` Tool

**Priority**: High  
**Status**: 📋 Planned  
**Estimated Effort**: 2-3 days  

---

### Overview

Implement a sophisticated codebase analysis tool that provides rapid project understanding for AI agents and developers. Instead of manually exploring files, this tool builds a comprehensive "project mental model" in seconds.

**Goal**: Get up to speed on unfamiliar codebases in seconds

---

### Core Analysis Modules

#### 1. Project Identity Detection
```
├── Language Detection (primary + secondary languages)
│   ├── By file extensions (.rs, .py, .ts, .go, etc.)
│   ├── By package manifests
│   └── By shebang lines (#!)
├── Framework Detection
│   ├── Web: React, Vue, Next.js, Django, FastAPI, Actix, etc.
│   ├── Desktop: Iced, Tauri, Electron, Qt
│   ├── Mobile: React Native, Flutter
│   └── CLI: Clap, Click, Commander
├── Build System Detection
│   ├── Cargo, npm/yarn/pnpm, pip/poetry, gradle, make
│   └── CI/CD configs (.github/workflows, .gitlab-ci.yml)
└── Project Type Classification
    ├── Library, Application, Monorepo, Plugin
    └── Microservice, Fullstack, CLI tool
```

#### 2. Architecture Pattern Recognition

| Pattern | Detection Signals |
|---------|-------------------|
| **MVC** | controllers/, models/, views/ directories |
| **Clean Architecture** | domain/, infrastructure/, application/ layers |
| **Hexagonal** | adapters/, ports/, core/ structure |
| **Modular Monolith** | modules/, features/ with isolated concerns |
| **Microservices** | Multiple Dockerfiles, docker-compose, separate services/ |
| **Plugin System** | plugins/, extensions/, addons/ directories |
| **Event-Driven** | events/, handlers/, message queues config |

#### 3. Dependency Graph Analysis
```rust
struct DependencyAnalysis {
    // Direct dependencies with versions
    dependencies: Vec<Dependency>,
    // Dev/test dependencies
    dev_dependencies: Vec<Dependency>,
    // Internal workspace/module dependencies
    internal_dependencies: Vec<InternalDep>,
    // Categorized by purpose
    categories: HashMap<Category, Vec<Dependency>>,
    // Security: outdated or vulnerable deps
    concerns: Vec<DependencyConcern>,
}

enum Category {
    WebFramework,
    Database,
    Serialization,
    Testing,
    Logging,
    Async,
    Crypto,
    UI,
}
```

#### 4. Entry Point Mapping
```
Entry Points Discovered:
├── Main Entry: src/main.rs (line 15: fn main())
├── Library Entry: src/lib.rs (exports 12 modules)
├── Binary Targets:
│   ├── arula_cli → src/bin/cli.rs
│   └── arula_desktop → src/main.rs
├── Test Entry: tests/integration_test.rs
├── Web Entry: src/routes/mod.rs (42 endpoints)
└── Config Entry: config/default.toml
```

#### 5. Smart Directory Analysis
```
Directory Structure (significance-annotated):
├── src/                     [Core Source - 156 files]
│   ├── api/                 [API Layer - HTTP/WebSocket handlers]
│   ├── core/                [Domain Logic - Business rules]
│   ├── utils/               [Utilities - Shared helpers]
│   └── models/              [Data Models - 23 structs]
├── tests/                   [Tests - 45% coverage estimate]
├── docs/                    [Documentation - 12 markdown files]
├── migrations/              [Database - 8 migrations]
└── .github/                 [CI/CD - 3 workflows]
```

#### 6. Documentation Intelligence
```rust
struct DocumentationAnalysis {
    // Main README quality score (0-100)
    readme_quality: u8,
    // Key topics extracted from READMEs
    key_topics: Vec<String>,
    // API documentation (rustdoc, jsdoc, etc.)
    api_docs_coverage: f32,
    // Architecture decision records
    adr_files: Vec<PathBuf>,
    // Contributing guidelines
    has_contributing: bool,
    // Changelog presence
    has_changelog: bool,
    // Inline comment density
    comment_ratio: f32,
}
```

#### 7. Code Quality Indicators
```
Quality Signals:
├── Testing
│   ├── Test files found: 45
│   ├── Test framework: cargo test + mock
│   └── Estimated coverage: ~60%
├── Linting/Formatting
│   ├── rustfmt.toml ✓
│   ├── clippy.toml ✓
│   └── .editorconfig ✓
├── Type Safety
│   ├── Strict mode: enabled
│   └── No unsafe blocks: 3 (reviewed)
└── Documentation
    ├── Public API documented: 78%
    └── README updated: 2 days ago
```

---

### Advanced Features

#### 8. Semantic Code Understanding
```rust
// Analyze public API surface
struct ApiSurface {
    // Exported functions/methods
    public_functions: Vec<FunctionSignature>,
    // Exported types/structs/enums
    public_types: Vec<TypeDefinition>,
    // Traits and implementations
    traits: Vec<TraitInfo>,
    // Most important modules (by import count)
    key_modules: Vec<ModuleImportance>,
}
```

#### 9. Hot Spots Detection
Identify areas of the codebase that are:
- **Complex** - High cyclomatic complexity
- **Frequently Changed** - From git history
- **Heavily Imported** - Central modules
- **Large Files** - May need refactoring

```
Hot Spots Identified:
┌─────────────────────────────┬────────┬─────────┬──────────┐
│ File                        │ Lines  │ Changes │ Imports  │
├─────────────────────────────┼────────┼─────────┼──────────┤
│ src/api/agent_client.rs     │ 734    │ 45      │ 23       │
│ src/main.rs                 │ 3024   │ 89      │ 31       │
│ src/tools/tools.rs          │ 542    │ 34      │ 18       │
└─────────────────────────────┴────────┴─────────┴──────────┘
```

#### 10. Project Relationship Graph
```
Module Dependency Graph:
                    ┌──────────┐
                    │  main.rs │
                    └────┬─────┘
           ┌─────────────┼─────────────┐
           ▼             ▼             ▼
      ┌────────┐   ┌──────────┐  ┌──────────┐
      │  api   │   │  tools   │  │  config  │
      └────┬───┘   └────┬─────┘  └──────────┘
           │            │
           ▼            ▼
      ┌────────────────────┐
      │       utils        │
      └────────────────────┘
```

#### 11. Quick Start Generation
Based on analysis, generate a "Quick Start" guide:
```markdown
## Quick Start for This Project

1. **Prerequisites**: Rust 1.70+, Node.js 18+
2. **Setup**: `cargo build && npm install`
3. **Run**: `cargo run -p arula_desktop`
4. **Test**: `cargo test`

### Key Files to Start With:
- `src/main.rs` - Application entry
- `arula_core/src/api/` - Core API logic
- `README.md` - Project overview
```

---

### Output Format Options

#### Option 1: Structured JSON (for AI consumption)
```json
{
  "project_name": "arula",
  "languages": ["rust", "toml"],
  "framework": "iced",
  "architecture": "modular_monolith",
  "entry_points": [...],
  "key_modules": [...],
  "dependencies": {...},
  "hot_spots": [...],
  "quality_score": 85,
  "quick_start": "..."
}
```

#### Option 2: Markdown Report (human-readable)
Beautiful formatted report with sections, tables, and diagrams.

#### Option 3: Condensed Summary (for system prompts)
```
Project: arula (Rust/Iced desktop app)
Structure: 3 crates (core, cli, desktop)
Key: src/api/agent_client.rs, src/tools/
Focus: AI assistant with tool execution
Start: cargo run -p arula_desktop
```

---

### Implementation Plan

#### Phase 1: Core Structure
- [ ] Create `arula_core/src/tools/analyze_context.rs`
- [ ] Define `ProjectAnalysis` struct with all analysis fields
- [ ] Implement basic directory scanning
- [ ] Add language detection by file extension

#### Phase 2: Manifest Parsing
- [ ] Parse `Cargo.toml` for Rust projects
- [ ] Parse `package.json` for Node.js projects
- [ ] Parse `requirements.txt`/`pyproject.toml` for Python
- [ ] Extract dependencies and categorize them

#### Phase 3: Pattern Detection
- [ ] Implement architecture pattern recognition
- [ ] Add entry point detection
- [ ] Build module relationship graph

#### Phase 4: Quality Analysis
- [ ] Documentation coverage estimation
- [ ] Test file detection
- [ ] Hot spots identification (file size + git history)

#### Phase 5: Output & Integration
- [ ] JSON output format
- [ ] Markdown report generation
- [ ] Condensed summary for AI context
- [ ] Integrate with tool registry

---

### Caching Strategy
```rust
struct ContextCache {
    // Hash of relevant files for invalidation
    content_hash: String,
    // Cached analysis
    analysis: ProjectAnalysis,
    // When cached
    cached_at: DateTime<Utc>,
    // TTL before re-analysis
    ttl: Duration,
}
```

---

### Use Cases

1. **New Project Onboarding** - "Analyze this codebase and tell me what it does"
2. **Pre-Task Context** - AI runs this before major code changes
3. **Documentation Generation** - Generate README from analysis
4. **Refactoring Planning** - Identify hot spots to improve
5. **Dependency Audit** - Security and update checks
6. **Architecture Review** - Identify patterns and anti-patterns

---

### Files to Create/Modify

- `arula_core/src/tools/analyze_context.rs` - Main implementation
- `arula_core/src/tools/mod.rs` - Register new tool
- `arula_core/src/tools/tools.rs` - Add to tool list

---

### Acceptance Criteria

- [ ] Tool can analyze any Rust project and return structured output
- [ ] Tool can analyze Node.js, Python projects (basic support)
- [ ] Output includes: languages, frameworks, entry points, key modules
- [ ] Hot spots detection works with git history
- [ ] Caching prevents redundant re-analysis
- [ ] Integration with existing tool system complete
