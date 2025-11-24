// Library exports for ARULA CLI components
#![allow(dead_code)]
#![allow(private_interfaces)]

pub mod api;
pub mod app;
pub mod tools;
pub mod ui;
pub mod utils;

// Re-export commonly used types from their new locations
pub use app::App;
pub use utils::colors::{ColorTheme, helpers};
pub use ui::output::OutputHandler;
pub use ui::reedline_input::{ReedlineInput, AiState};
pub use api::api::Usage;
