//! Modern tool implementations using the agent framework
//!
//! This module provides the tool registry and re-exports all built-in tools
//! from the `builtin` submodule.
//!
//! # Architecture
//!
//! Tools are organized in the `builtin/` directory with one tool per file.
//! This module re-exports them and provides the registry creation functions.
//!
//! # Adding New Tools
//!
//! 1. Create a new module in `builtin/`
//! 2. Add it to `builtin/mod.rs`
//! 3. Re-export it here
//! 4. Register it in `create_basic_tool_registry()`

// Re-export all built-in tools from the organized builtin modules
// These are public API exports - not used internally but exposed for library consumers
#[allow(unused_imports)]
pub use crate::tools::builtin::{
    BashParams, BashResult, BashTool, DirectoryEntry, FileEditParams, FileEditResult, FileEditTool,
    FileReadParams, FileReadResult, FileReadTool, ListDirParams, ListDirResult, ListDirectoryTool,
    QuestionParams, QuestionResult, QuestionTool, SearchMatch, SearchParams, SearchResult,
    SearchTool, WebSearchParams, WebSearchResult, WebSearchResultItem, WebSearchTool,
    WriteFileParams, WriteFileResult, WriteFileTool,
};

// Re-export analyze_context tool
#[allow(unused_imports)]
pub use crate::tools::analyze_context::{
    AnalyzeContextParams, AnalyzeContextResult, AnalyzeContextTool,
};

// Re-export Visioneer tool from its own module
#[allow(unused_imports)]
pub use crate::tools::visioneer::{VisioneerParams, VisioneerResult, VisioneerTool};

/// Factory function to create a basic tool registry (without MCP discovery)
/// Used by AgentClient when a cached registry is already available
pub fn create_basic_tool_registry() -> crate::api::agent::ToolRegistry {
    use crate::api::agent::ToolRegistry;

    let mut registry = ToolRegistry::new();

    // Register the basic built-in tools
    registry.register(BashTool::new());
    registry.register(FileReadTool::new());
    registry.register(FileEditTool::new());
    registry.register(WriteFileTool::new());
    registry.register(ListDirectoryTool::new());
    registry.register(SearchTool::new());
    registry.register(WebSearchTool::new());
    registry.register(VisioneerTool::new());
    registry.register(QuestionTool::new());
    registry.register(AnalyzeContextTool::new());

    registry
}

/// Factory function to create a default tool registry with MCP discovery
/// This includes async MCP discovery and should only be called once at startup
pub async fn create_default_tool_registry_with_mcp(
    config: &crate::utils::config::Config,
) -> Result<crate::api::agent::ToolRegistry, String> {
    let mut registry = create_basic_tool_registry();

    // Initialize MCP tools if available
    if let Err(e) = initialize_mcp_tools(&mut registry, config).await {
        eprintln!("⚠️ Failed to initialize MCP tools: {}", e);
    }

    Ok(registry)
}

/// Factory function to create a default tool registry (backward compatibility)
/// MCP tools are initialized separately to avoid runtime conflicts
pub fn create_default_tool_registry(
    _config: &crate::utils::config::Config,
) -> crate::api::agent::ToolRegistry {
    create_basic_tool_registry()
}

/// Initialize MCP tools asynchronously and add them to the registry
pub async fn initialize_mcp_tools(
    registry: &mut crate::api::agent::ToolRegistry,
    config: &crate::utils::config::Config,
) -> Result<(), String> {
    use crate::tools::mcp_dynamic;

    mcp_dynamic::initialize_dynamic_mcp_tools(config).await?;
    mcp_dynamic::register_dynamic_mcp_tools(registry).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_basic_registry() {
        let registry = create_basic_tool_registry();
        let tools = registry.get_tools();

        // Should have all basic tools registered
        assert!(tools.contains(&"execute_bash"));
        assert!(tools.contains(&"read_file"));
        assert!(tools.contains(&"write_file"));
        assert!(tools.contains(&"edit_file"));
        assert!(tools.contains(&"list_directory"));
        assert!(tools.contains(&"search_files"));
        assert!(tools.contains(&"web_search"));
        assert!(tools.contains(&"visioneer"));
        assert!(tools.contains(&"ask_question"));
        assert!(tools.contains(&"analyze_context"));
    }
}
