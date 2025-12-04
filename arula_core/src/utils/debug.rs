//! Centralized debug and logging utilities for ARULA CLI
//!
//! This module eliminates duplicate `debug_print` functions across the codebase
//! by providing a single, efficient implementation with macros.
//!
//! # Usage
//!
//! ```rust
//! use arula_cli::{debug, debug_module};
//!
//! // Simple debug print
//! debug!("Processing message: {}", message);
//!
//! // Module-specific debug print
//! debug_module!("API", "Sending request to {}", endpoint);
//! ```
//!
//! # Environment Variables
//!
//! - `ARULA_DEBUG=1` - Enable debug output to console and log file

use std::sync::OnceLock;

/// Cached debug enabled state (checked once at startup for performance)
static DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

/// Check if debug mode is enabled
///
/// This function caches the result after the first call for performance.
/// The check looks for `ARULA_DEBUG=1` environment variable.
///
/// # Example
///
/// ```rust
/// if arula_cli::utils::debug::is_debug_enabled() {
///     println!("Debug mode is on!");
/// }
/// ```
#[inline]
pub fn is_debug_enabled() -> bool {
    *DEBUG_ENABLED.get_or_init(|| {
        std::env::var("ARULA_DEBUG")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false)
    })
}

/// Force re-check of debug enabled state
///
/// This is primarily useful for testing. In production, the debug state
/// is cached at first access for performance.
pub fn reset_debug_state() {
    // Note: OnceLock doesn't have a reset method, so this is a no-op
    // In tests, set ARULA_DEBUG before any debug calls
}

/// Debug print helper that checks ARULA_DEBUG environment variable
///
/// This is the function version for use when macros are not convenient.
/// Prefer the `debug!` macro in most cases.
#[inline]
pub fn debug_print(msg: &str) {
    if is_debug_enabled() {
        println!("🔧 DEBUG: {}", msg);
        crate::utils::logger::debug(msg);
    }
}

/// Debug print with module prefix
///
/// This is the function version for use when macros are not convenient.
/// Prefer the `debug_module!` macro in most cases.
#[inline]
pub fn debug_print_module(module: &str, msg: &str) {
    if is_debug_enabled() {
        println!("🔧 [{}] {}", module, msg);
        crate::utils::logger::debug(&format!("[{}] {}", module, msg));
    }
}

/// Log AI interaction details for debugging
///
/// This function logs detailed information about AI requests and responses
/// to help diagnose issues with AI communication.
///
/// # Arguments
///
/// * `request` - The user's request text
/// * `context` - The conversation context messages
/// * `response_start` - Optional first part of the AI response
pub fn log_ai_interaction(
    request: &str,
    context: &[crate::api::api::ChatMessage],
    response_start: Option<&str>,
) {
    if !is_debug_enabled() {
        return;
    }

    use chrono::Utc;
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let mut log_msg = format!("\n=== AI Interaction at {} ===\n", timestamp);
    log_msg.push_str(&format!("USER REQUEST: {}\n", request));
    log_msg.push_str(&format!("CONTEXT MESSAGES ({} total):\n", context.len()));

    for (i, msg) in context.iter().enumerate() {
        let content_preview = msg
            .content
            .as_ref()
            .map(|c| {
                if c.len() > 100 {
                    format!("{}...", &c[..100])
                } else {
                    c.clone()
                }
            })
            .unwrap_or_else(|| "(no content)".to_string());
        log_msg.push_str(&format!("  [{}]: {} -> {}\n", i, msg.role, content_preview));
    }

    if let Some(start) = response_start {
        log_msg.push_str(&format!("AI RESPONSE START: {}\n", start));
    }
    log_msg.push_str("=====================================\n");

    crate::utils::logger::info(&log_msg);
}

/// Log AI response chunk for streaming debugging
#[inline]
pub fn log_ai_response_chunk(chunk: &str) {
    if is_debug_enabled() {
        crate::utils::logger::debug(&format!("AI CHUNK: {}", chunk));
    }
}

/// Log AI response completion
pub fn log_ai_response_complete(final_response: &str) {
    if is_debug_enabled() {
        let preview = if final_response.len() > 500 {
            format!("{}...(truncated)", &final_response[..500])
        } else {
            final_response.to_string()
        };
        crate::utils::logger::info(&format!(
            "AI COMPLETE RESPONSE: {}\n=== END AI Interaction ===\n",
            preview
        ));
    }
}

/// Log tool execution for debugging
pub fn log_tool_execution(tool_name: &str, args: &str, result: Option<&str>) {
    if !is_debug_enabled() {
        return;
    }

    let args_preview = if args.len() > 200 {
        format!("{}...", &args[..200])
    } else {
        args.to_string()
    };

    debug_print_module(
        "TOOL",
        &format!("Executing '{}' with args: {}", tool_name, args_preview),
    );

    if let Some(res) = result {
        let result_preview = if res.len() > 200 {
            format!("{}...", &res[..200])
        } else {
            res.to_string()
        };
        debug_print_module("TOOL", &format!("Result: {}", result_preview));
    }
}

/// Performance timing helper for debugging slow operations
pub struct DebugTimer {
    name: String,
    start: std::time::Instant,
    enabled: bool,
}

impl DebugTimer {
    /// Create a new debug timer
    ///
    /// If debug mode is disabled, the timer is a no-op.
    pub fn new(name: impl Into<String>) -> Self {
        let enabled = is_debug_enabled();
        Self {
            name: name.into(),
            start: std::time::Instant::now(),
            enabled,
        }
    }

    /// Log an intermediate checkpoint
    pub fn checkpoint(&self, label: &str) {
        if self.enabled {
            let elapsed = self.start.elapsed();
            debug_print_module(
                "PERF",
                &format!("{} - {}: {:?}", self.name, label, elapsed),
            );
        }
    }

    /// Complete the timer and log the total duration
    pub fn finish(self) {
        if self.enabled {
            let elapsed = self.start.elapsed();
            debug_print_module("PERF", &format!("{} completed in {:?}", self.name, elapsed));
        }
    }
}

impl Drop for DebugTimer {
    fn drop(&mut self) {
        // Don't log on drop - use finish() explicitly
    }
}

/// Debug print macro - use this instead of direct function calls
///
/// # Examples
///
/// ```rust
/// debug!("Simple message");
/// debug!("Formatted: {} and {}", value1, value2);
/// ```
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::utils::debug::is_debug_enabled() {
            let msg = format!($($arg)*);
            println!("🔧 DEBUG: {}", msg);
            $crate::utils::logger::debug(&msg);
        }
    };
}

/// Debug print macro with module prefix
///
/// # Examples
///
/// ```rust
/// debug_module!("API", "Sending request to {}", endpoint);
/// debug_module!("TOOL", "Executing: {}", tool_name);
/// ```
#[macro_export]
macro_rules! debug_module {
    ($module:expr, $($arg:tt)*) => {
        if $crate::utils::debug::is_debug_enabled() {
            let msg = format!($($arg)*);
            println!("🔧 [{}] {}", $module, msg);
            $crate::utils::logger::debug(&format!("[{}] {}", $module, msg));
        }
    };
}

/// Conditional debug block - only executes if debug is enabled
///
/// # Examples
///
/// ```rust
/// debug_block! {
///     let expensive_debug_info = compute_debug_info();
///     println!("Debug info: {:?}", expensive_debug_info);
/// }
/// ```
#[macro_export]
macro_rules! debug_block {
    ($($body:tt)*) => {
        if $crate::utils::debug::is_debug_enabled() {
            $($body)*
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_debug_enabled_default() {
        // Note: This test may be affected by ARULA_DEBUG being set in the environment
        // In a clean environment, debug should be disabled by default
        let _ = is_debug_enabled(); // Just ensure it doesn't panic
    }

    #[test]
    fn test_debug_print_no_panic() {
        // Should not panic regardless of debug state
        debug_print("Test message");
        debug_print_module("TEST", "Module message");
    }

    #[test]
    fn test_debug_timer() {
        let timer = DebugTimer::new("test_operation");
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.checkpoint("after sleep");
        timer.finish();
    }

    #[test]
    fn test_log_tool_execution_no_panic() {
        log_tool_execution("test_tool", r#"{"arg": "value"}"#, Some("success"));
        log_tool_execution("test_tool", r#"{"arg": "value"}"#, None);
    }

    #[test]
    fn test_macros_compile() {
        // These should compile without errors
        debug!("Simple message");
        debug!("Formatted: {}", 42);
        debug_module!("TEST", "Module message");
        debug_module!("TEST", "Formatted: {}", "value");
        debug_block! {
            let _x = 1 + 1;
        }
    }
}

