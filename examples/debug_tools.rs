//! Debug tool listing to see what's actually registered

use arula_cli::tools::tools::create_default_tool_registry;
use arula_cli::utils::config::Config;
use arula_cli::tools::mcp_dynamic;

#[tokio::main]
async fn main() {
    println!("🔧 Debug: Checking what tools are actually registered...");

    let config = Config::default();

    // Create basic tool registry
    let mut registry = create_default_tool_registry(&config);

    println!("📋 Basic tools:");
    for tool_name in registry.get_tools() {
        println!("  - {}", tool_name);
    }

    // Initialize MCP tools
    println!("\n🔧 Initializing MCP tools...");
    match mcp_dynamic::initialize_dynamic_mcp_tools(&config).await {
        Ok(count) => {
            println!("✅ Discovered {} MCP servers", count);

            if let Err(e) = mcp_dynamic::register_dynamic_mcp_tools(&mut registry).await {
                println!("❌ Failed to register MCP tools: {}", e);
            } else {
                println!("✅ Registered MCP tools");
            }
        }
        Err(e) => {
            println!("❌ Failed to initialize MCP tools: {}", e);
        }
    }

    println!("\n📋 All tools after MCP initialization:");
    for tool_name in registry.get_tools() {
        println!("  - {}", tool_name);
    }

    println!("\n🔧 OpenAI tools format:");
    for tool in registry.get_openai_tools() {
        let name = tool.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("unknown");
        let description = tool.get("function").and_then(|f| f.get("description")).and_then(|d| d.as_str()).unwrap_or("no description");
        println!("  - {}: {}", name, description);
    }
}