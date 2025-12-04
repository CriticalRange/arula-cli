#![allow(dead_code)]
#![allow(private_interfaces)]

pub mod ui;

// Re-export core modules for backward compatibility
pub use arula_core::{
    api, app, prelude, tools, utils, App, StreamEvent, SessionConfig, SessionRunner, AgentBackend,
};
pub use ui::output::OutputHandler;
pub use ui::custom_spinner::CustomSpinner;
pub use api::api::Usage;
pub use utils::colors::{helpers, ColorTheme};
pub use utils::error::{ApiError, ArulaError, ArulaResult, OptionExt, ResultExt, ToolError};
pub use utils::debug::{debug_print, is_debug_enabled, DebugTimer};
pub use api::models::{CachedModels, ModelCacheManager, ModelFetcher};

pub mod config {
    pub use arula_core::utils::config::{AiConfig, Config, McpServerConfig, ProviderConfig};
}
