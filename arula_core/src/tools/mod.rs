//! Tools module for ARULA CLI
//!
//! Contains built-in tool implementations for AI agent interactions.
//!
//! # Module Structure
//!
//! - `builtin` - Organized built-in tools (new modular structure)
//! - `tools` - Legacy tools file (being migrated to builtin/)
//! - `visioneer` - Vision/screenshot capabilities
//! - `mcp` - Model Context Protocol client
//! - `mcp_dynamic` - Dynamic MCP tool loading

pub mod builtin;
pub mod tools;
pub mod visioneer;
pub mod mcp;
pub mod mcp_dynamic;

// Builtin tools available via:
// builtin::{BashTool, FileReadTool, WriteFileTool, FileEditTool, etc.}