// Library exports for ARULA CLI components
#![allow(dead_code)]
#![allow(private_interfaces)]

pub mod api;
pub mod app;
pub mod tools;
pub mod ui;
pub mod utils;

// Add missing modules for tests and benchmarks
pub use utils::chat;
pub use utils::conversation;
pub use utils::tool_call;
pub use api::agent;
pub use tools::visioneer;

// Re-export commonly used types from their new locations
pub use app::App;
pub use utils::colors::{ColorTheme, helpers};
pub use ui::output::OutputHandler;
pub use ui::custom_spinner::CustomSpinner;
pub use api::api::Usage;
