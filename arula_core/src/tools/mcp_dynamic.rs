//! Dynamic MCP Tool Discovery and Registration
//!
//! This module discovers MCP tools from configured servers and creates individual
//! tool wrappers that are registered directly in the tool registry.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use crate::tools::mcp::McpClient;
use crate::utils::config::{Config, McpServerConfig};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::sync::RwLock;

// Import MCP manager getter
use crate::tools::mcp::get_mcp_manager;

/// Represents a discovered MCP server
#[derive(Debug, Clone)]
pub struct DiscoveredMcpServer {
    pub server_id: String,
    pub name: String,
    pub tools: Vec<McpToolInfo>,
}

#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Server-based MCP tool wrapper
pub struct ServerMcpTool {
    pub server_info: DiscoveredMcpServer,
    pub tool_name: String,
}

impl ServerMcpTool {
    pub fn new(server_info: DiscoveredMcpServer) -> Self {
        // Create a clear tool name that identifies the server
        let tool_name = format!("mcp_{}", server_info.server_id);
        Self {
            server_info,
            tool_name,
        }
    }

    async fn call_mcp_tool(&self, tool_name: &str, parameters: Value) -> Result<Value, String> {
        let client = get_mcp_manager()
            .get_client(&self.server_info.server_id)
            .await
            .ok_or_else(|| format!("MCP server '{}' not available", self.server_info.server_id))?;

        let tool_params = if parameters.is_null()
            || parameters.as_object().map(|o| o.is_empty()).unwrap_or(true)
        {
            json!({})
        } else {
            parameters
        };

        match client
            .call_tool(
                tool_name,
                serde_json::from_value::<HashMap<String, Value>>(tool_params)
                    .unwrap_or_else(|_| HashMap::new()),
            )
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => Err(format!("MCP tool call failed: {}", e)),
        }
    }
}

#[async_trait]
impl Tool for ServerMcpTool {
    type Params = serde_json::Value;
    type Result = serde_json::Value;

    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.server_info.name
    }

    fn schema(&self) -> ToolSchema {
        // Create a dynamic description based on the actual server info
        let tool_names: Vec<String> = self
            .server_info
            .tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect();
        let tool_list = tool_names.join(", ");
        let description = format!(
            "Access tools from MCP server '{}'. Available tools: {}. Example usage: {{\"tool_name\": \"resolve-library-id\", \"parameters\": {{\"libraryName\": \"tokio\"}}}}",
            self.server_info.server_id,
            tool_list
        );

        let mut builder = ToolSchemaBuilder::new(&self.tool_name, &description);

        // Add parameters for tool name and arguments
        builder = builder
            .param("tool_name", "string")
            .description("tool_name", "The specific MCP tool name to call (required)")
            .required("tool_name");

        builder = builder
            .param("parameters", "object")
            .description(
                "parameters",
                "Parameters object for the MCP tool call (format varies by tool)",
            )
            .required("parameters");

        builder.build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        // Extract tool_name and parameters from the unified parameter structure
        let tool_name = params
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'tool_name' parameter")?;

        let parameters = params.get("parameters").cloned().unwrap_or(json!({}));

        self.call_mcp_tool(tool_name, parameters).await
    }
}

/// Dynamic MCP Tool Registry
pub struct DynamicMcpRegistry {
    discovered_servers: RwLock<Vec<DiscoveredMcpServer>>,
    config: RwLock<Option<Config>>,
}

impl Default for DynamicMcpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicMcpRegistry {
    pub fn new() -> Self {
        Self {
            discovered_servers: RwLock::new(Vec::new()),
            config: RwLock::new(None),
        }
    }

    pub async fn update_config(&self, config: Config) -> Result<usize, String> {
        // Initialize MCP manager
        crate::tools::mcp::McpTool::update_global_config(config.clone()).await;

        // Get all configured MCP servers
        let mcp_servers = config.get_mcp_servers();

        // Discover tools from all configured MCP servers
        let mut total_servers = 0;
        let mut all_servers = Vec::new();

        for (server_id, server_config) in mcp_servers {
            match self.discover_server_tools(server_id, server_config).await {
                Ok(server_info) => {
                    total_servers += 1;
                    all_servers.push(server_info);
                }
                Err(_e) => {
                    // Server discovery failed, skip this server
                }
            }
        }

        // Store discovered servers
        *self.discovered_servers.write().await = all_servers;

        Ok(total_servers)
    }

    async fn discover_server_tools(
        &self,
        server_id: &str,
        server_config: &McpServerConfig,
    ) -> Result<DiscoveredMcpServer, String> {
        let client = McpClient::new(server_config.clone());

        // Initialize the server
        client
            .initialize()
            .await
            .map_err(|e| format!("Failed to initialize MCP server: {}", e))?;

        // List available tools
        let tool_names = client
            .list_tools()
            .await
            .map_err(|e| format!("Failed to list MCP tools: {}", e))?;

        let mut server_tools = Vec::new();

        for tool_name in tool_names {
            server_tools.push(McpToolInfo {
                name: tool_name.clone(),
                description: format!("MCP tool: {}", tool_name),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            });
        }

        Ok(DiscoveredMcpServer {
            server_id: server_id.to_string(),
            name: format!("MCP Server: {}", server_id),
            tools: server_tools,
        })
    }

    pub async fn get_server_tools(&self) -> Vec<ServerMcpTool> {
        let servers = self.discovered_servers.read().await;
        servers.iter().cloned().map(ServerMcpTool::new).collect()
    }

    pub async fn get_discovered_servers(&self) -> Vec<DiscoveredMcpServer> {
        self.discovered_servers.read().await.clone()
    }
}

// Global dynamic MCP registry using OnceLock (Rust 1.70+)
static DYNAMIC_MCP_REGISTRY: std::sync::OnceLock<DynamicMcpRegistry> = std::sync::OnceLock::new();

/// Get the global dynamic MCP registry
fn get_dynamic_mcp_registry() -> &'static DynamicMcpRegistry {
    DYNAMIC_MCP_REGISTRY.get_or_init(|| DynamicMcpRegistry::new())
}

/// Initialize dynamic MCP tools and register them in the tool registry
pub async fn initialize_dynamic_mcp_tools(config: &Config) -> Result<usize, String> {
    get_dynamic_mcp_registry().update_config(config.clone()).await
}

/// Get all discovered MCP servers
pub async fn get_discovered_mcp_servers() -> Vec<DiscoveredMcpServer> {
    get_dynamic_mcp_registry().get_discovered_servers().await
}

/// Register dynamic MCP tools in the provided tool registry
pub async fn register_dynamic_mcp_tools(
    registry: &mut crate::api::agent::ToolRegistry,
) -> Result<usize, String> {
    let server_tools = get_dynamic_mcp_registry().get_server_tools().await;

    let mut registered_count = 0;
    for tool in server_tools {
        // Only register tools for servers that have actual tools discovered
        if !tool.server_info.tools.is_empty() {
            registry.register(tool);
            registered_count += 1;
        }
    }
    Ok(registered_count)
}
