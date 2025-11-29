//! MCP (Model Context Protocol) Tool Integration
//!
//! This module provides MCP server integration for ARULA CLI,
//! allowing access to external MCP servers like Context7.

use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder};
use crate::utils::config::{Config, McpServerConfig};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

/// MCP tool parameters
#[derive(Debug, Deserialize)]
pub struct McpToolParams {
    pub server: String,
    pub action: String,
    pub parameters: Option<serde_json::Value>,
}

/// MCP tool result
#[derive(Debug, Serialize)]
pub struct McpToolResult {
    pub success: bool,
    pub result: serde_json::Value,
    pub server: String,
    pub action: String,
    pub message: String,
}

/// MCP Server client
#[derive(Clone)]
pub struct McpClient {
    config: McpServerConfig,
    client: reqwest::Client,
}

impl McpClient {
    pub fn new(config: McpServerConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout.unwrap_or(30)))
            .user_agent("arula-cli/1.0")
            .build()
            .expect("Failed to create MCP client");

        Self { config, client }
    }

    async fn call_mcp_method(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": Uuid::new_v4().to_string(),
            "method": method,
            "params": params
        });

        // Debug logging
        if std::env::var("ARULA_DEBUG").unwrap_or_default() == "1" {
            eprintln!("🔧 MCP Debug - Request URL: {}", self.config.url);
            eprintln!("🔧 MCP Debug - Request Body: {}", serde_json::to_string_pretty(&request_body).unwrap_or_default());
            eprintln!("🔧 MCP Debug - Headers: {:?}", self.config.headers);
        }

        let mut request = self.client
            .post(&self.config.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&request_body);

        // Add custom headers from config
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        let response = request.send().await
            .map_err(|e| anyhow::anyhow!("Failed to send MCP request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Failed to read error response".to_string());
            return Err(anyhow::anyhow!(
                "MCP server returned status: {}. Response: {}",
                status,
                error_text
            ));
        }

        let response_json: serde_json::Value = response.json().await
            .map_err(|e| anyhow::anyhow!("Failed to parse MCP response: {}", e))?;

        // Check for JSON-RPC error
        if let Some(error) = response_json.get("error") {
            return Err(anyhow::anyhow!(
                "MCP server error: {}",
                serde_json::to_string_pretty(error).unwrap_or_else(|_| "Unknown error".to_string())
            ));
        }

        Ok(response_json)
    }

    pub async fn initialize(&self) -> Result<serde_json::Value> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "roots": {
                    "listChanged": true
                },
                "sampling": {}
            },
            "clientInfo": {
                "name": "arula-cli",
                "version": "1.0.0"
            }
        });

        let response = self.call_mcp_method("initialize", params).await?;
        Ok(response.get("result").cloned().unwrap_or(json!({})))
    }

    pub async fn list_tools(&self) -> Result<Vec<String>> {
        let params = json!({});
        let response = self.call_mcp_method("tools/list", params).await?;

        if let Some(result) = response.get("result") {
            if let Some(tools) = result.get("tools") {
                if let Some(tools_array) = tools.as_array() {
                    let mut tool_names = Vec::new();
                    for tool in tools_array {
                        if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                            tool_names.push(name.to_string());
                        }
                    }
                    return Ok(tool_names);
                }
            }
        }

        Ok(vec![])
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let params = json!({
            "name": tool_name,
            "arguments": arguments
        });

        let response = self.call_mcp_method("tools/call", params).await?;

        if let Some(result) = response.get("result") {
            if let Some(content) = result.get("content") {
                return Ok(content.clone());
            }
        }

        Ok(json!({}))
    }
}

/// Global MCP Manager for managing MCP clients
pub struct McpManager {
    clients: RwLock<HashMap<String, McpClient>>,
    config: RwLock<Option<Config>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            config: RwLock::new(None),
        }
    }

    pub async fn update_config(&self, config: Config) {
        let mut config_guard = self.config.write().await;
        *config_guard = Some(config);

        // Update clients
        if let Some(ref config) = *config_guard {
            let mut clients_guard = self.clients.write().await;
            clients_guard.clear();

            for (server_id, server_config) in config.get_mcp_servers() {
                let client = McpClient::new(server_config.clone());

                // Initialize the client
                if let Err(e) = client.initialize().await {
                    eprintln!("Failed to initialize MCP server '{}': {}", server_id, e);
                    continue;
                }

                clients_guard.insert(server_id.clone(), client);
            }
        }
    }

    pub async fn get_client(&self, server_id: &str) -> Option<McpClient> {
        let clients = self.clients.read().await;
        clients.get(server_id).cloned()
    }

    async fn list_available_tools(&self) -> Result<HashMap<String, Vec<String>>> {
        let clients = self.clients.read().await;
        let mut server_tools = HashMap::new();

        for (server_id, client) in clients.iter() {
            match client.list_tools().await {
                Ok(tools) => {
                    server_tools.insert(server_id.clone(), tools);
                }
                Err(e) => {
                    eprintln!("Failed to list tools for server '{}': {}", server_id, e);
                    server_tools.insert(server_id.clone(), vec![]);
                }
            }
        }

        Ok(server_tools)
    }
}

// Global MCP manager instance
lazy_static::lazy_static! {
    pub static ref MCP_MANAGER: McpManager = McpManager::new();
}

/// MCP Tool for accessing MCP servers
pub struct McpTool;

impl McpTool {
    pub fn new() -> Self {
        Self
    }

    pub async fn update_global_config(config: Config) {
        MCP_MANAGER.update_config(config).await;
    }

    async fn get_client(&self, server_id: &str) -> Option<McpClient> {
        MCP_MANAGER.get_client(server_id).await
    }

    async fn list_available_tools(&self) -> Result<HashMap<String, Vec<String>>> {
        MCP_MANAGER.list_available_tools().await
    }
}

#[async_trait]
impl Tool for McpTool {
    type Params = McpToolParams;
    type Result = McpToolResult;

    fn name(&self) -> &str {
        "mcp_call"
    }

    fn description(&self) -> &str {
        "Call a tool from a configured MCP server (Model Context Protocol)"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new(
            "mcp_call",
            "Call a tool from a configured MCP server (Model Context Protocol)"
        )
        .param("server", "string")
        .description("server", "The MCP server ID to use")
        .required("server")
        .param("action", "string")
        .description("action", "The action/tool name to call")
        .required("action")
        .param("parameters", "object")
        .description("parameters", "Parameters for the tool (optional)")
        .build()
    }

    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        let client = self.get_client(&params.server).await
            .ok_or_else(|| format!("MCP server '{}' not found or not initialized", params.server))?;

        // Convert parameters to HashMap
        let tool_args = if let Some(parameters) = params.parameters {
            if let Ok(map) = serde_json::from_value::<HashMap<String, serde_json::Value>>(parameters) {
                map
            } else {
                return Err("Failed to parse tool parameters".to_string());
            }
        } else {
            HashMap::new()
        };

        match client.call_tool(&params.action, tool_args).await {
            Ok(result) => Ok(McpToolResult {
                success: true,
                result,
                server: params.server,
                action: params.action,
                message: "Tool call successful".to_string(),
            }),
            Err(e) => Ok(McpToolResult {
                success: false,
                result: json!({}),
                server: params.server,
                action: params.action,
                message: format!("Tool call failed: {}", e),
            }),
        }
    }
}

/// MCP Discovery Tool - lists available MCP tools
pub struct McpDiscoveryTool;

impl McpDiscoveryTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for McpDiscoveryTool {
    type Params = serde_json::Value;
    type Result = serde_json::Value;

    fn name(&self) -> &str {
        "mcp_list_tools"
    }

    fn description(&self) -> &str {
        "List all available tools from configured MCP servers"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchemaBuilder::new(
            "mcp_list_tools",
            "List all available tools from configured MCP servers"
        )
        .build()
    }

    async fn execute(&self, _params: Self::Params) -> Result<Self::Result, String> {
        match MCP_MANAGER.list_available_tools().await {
            Ok(server_tools) => {
                // Format the results for better readability
                let mut formatted_servers = Vec::new();
                let mut tool_details = std::collections::HashMap::new();

                for (server_id, tools) in &server_tools {
                    formatted_servers.push(server_id.clone());

                    let tool_info: Vec<String> = tools.iter().map(|tool| {
                        format!("- {}", tool)
                    }).collect();

                    tool_details.insert(server_id.clone(), tool_info);
                }

                let result = json!({
                    "available_servers": formatted_servers,
                    "server_tools": server_tools.clone(),
                    "tool_details": tool_details,
                    "summary": format!("Found {} MCP servers with {} total tools",
                        formatted_servers.len(),
                        server_tools.values().map(|v| v.len()).sum::<usize>())
                });
                Ok(result)
            }
            Err(e) => {
                let error_result = json!({
                    "error": format!("Failed to list MCP tools: {}", e),
                    "available_servers": [],
                    "server_tools": {},
                    "tool_details": {},
                    "summary": "No MCP tools available due to connection errors"
                });
                Ok(error_result)
            }
        }
    }
}