//! Utility modules for ARULA CLI
//!
//! Contains shared utilities, configuration management, data structures, and helper functions.

pub mod changelog;
pub mod chat;
pub mod colors;
pub mod config;
pub mod conversation;
pub mod debug;
pub mod error;
pub mod git_state;
pub mod logger;
pub mod tool_call;
pub mod tool_progress;

// Available exports via submodules:
// debug::{is_debug_enabled, debug_print, DebugTimer}
// error::{ArulaError, ArulaResult, ApiError, ToolError, ResultExt, OptionExt}