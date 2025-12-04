//! Prelude module for ARULA CLI
//!
//! This module re-exports commonly used types, traits, and macros
//! to simplify imports across the codebase.
//!
//! # Usage
//!
//! ```rust
//! use arula_cli::prelude::*;
//! ```

// Error handling
pub use crate::utils::error::{ApiError, ArulaError, ArulaResult, ToolError};
pub use crate::utils::error::{OptionExt, ResultExt};

// Debug utilities
pub use crate::utils::debug::{debug_print, is_debug_enabled, DebugTimer};
pub use crate::{debug, debug_block, debug_module};

// Model caching
pub use crate::api::models::{CachedModels, ModelCacheManager, ModelFetcher};

// Configuration
pub use crate::utils::config::Config;

// Chat types
pub use crate::utils::chat::{ChatMessage, MessageType};

// Agent types
pub use crate::api::agent::{ContentBlock, Tool, ToolResult, ToolSchema, ToolSchemaBuilder};

// Commonly used external crates
pub use anyhow::{bail, ensure, Context, Result};
pub use async_trait::async_trait;
