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
pub use crate::utils::error::{ArulaError, ArulaResult, ApiError, ToolError};
pub use crate::utils::error::{ResultExt, OptionExt};

// Debug utilities
pub use crate::utils::debug::{is_debug_enabled, debug_print, DebugTimer};
pub use crate::{debug, debug_module, debug_block};

// Model caching
pub use crate::api::models::{ModelCacheManager, ModelFetcher, CachedModels};

// Configuration
pub use crate::utils::config::Config;

// Chat types
pub use crate::utils::chat::{ChatMessage, MessageType};

// Agent types
pub use crate::api::agent::{Tool, ToolSchema, ToolSchemaBuilder, ToolResult, ContentBlock};

// Commonly used external crates
pub use anyhow::{Context, Result, bail, ensure};
pub use async_trait::async_trait;

