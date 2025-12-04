#![allow(dead_code)]
#![allow(private_interfaces)]

pub mod ui;

// Re-export core modules for backward compatibility
pub use api::api::Usage;
pub use api::models::{CachedModels, ModelCacheManager, ModelFetcher};
pub use arula_core::{
    api, app, prelude, tools, utils, AgentBackend, App, SessionConfig, SessionRunner, StreamEvent,
};
pub use ui::custom_spinner::CustomSpinner;
pub use ui::output::OutputHandler;
pub use utils::colors::{helpers, ColorTheme};
pub use utils::debug::{debug_print, is_debug_enabled, DebugTimer};
pub use utils::error::{ApiError, ArulaError, ArulaResult, OptionExt, ResultExt, ToolError};

pub mod config {
    pub use arula_core::utils::config::{AiConfig, Config, McpServerConfig, ProviderConfig};
}
