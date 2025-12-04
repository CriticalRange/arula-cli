//! Test MCP Integration Example
//!
//! This example tests the MCP tool discovery and integration system

use arula_cli::utils::config::{Config, McpServerConfig};
use arula_cli::tools::mcp_dynamic::{initialize_dynamic_mcp_tools, get_discovered_mcp_servers};
use arula_cli::tools::tools::create_default_tool_registry;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🔧 Testing MCP Integration...");

    // Create a test configuration with MCP servers
    let mut mcp_servers = HashMap::new();

    // Add a test MCP server configuration
    mcp_servers.insert("test_server".to_string(), McpServerConfig {
        url: "http://localhost:3000".to_string(),
        headers: HashMap::new(),
        timeout: Some(30),
        retries: Some(3),
    });

    let config = Config {
        active_provider: "test".to_string(),
        providers: HashMap::new(),
        mcp_servers,
        ai: None,
    };

    println!("✅ Test configuration created");

    // Test MCP tool discovery
    println!("🔍 Testing MCP tool discovery...");
    match initialize_dynamic_mcp_tools(&config).await {
        Ok(count) => {
            println!("✅ MCP discovery completed: {} servers processed", count);
        }
        Err(e) => {
            println!("⚠️  MCP discovery failed (expected for test servers): {}", e);
        }
    }

    // Test tool registry integration
    let tool_registry = create_default_tool_registry(&config);
    let available_tools = tool_registry.get_tools();

    for tool in available_tools {
        println!("  - {}", tool);
    }

    // Get discovered servers
    let discovered = get_discovered_mcp_servers().await;

    for server in discovered {
        println!("   Server: {} ({})", server.server_id, server.name);
        println!("    Tools: {}", server.tools.len());
        for tool in server.tools {
            println!("      - {}", tool.name);
        }
    }
    Ok(())
}