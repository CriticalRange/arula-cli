// Library exports for ARULA CLI components
#![allow(dead_code)]
#![allow(private_interfaces)]

pub mod agent;
pub mod agent_client;
pub mod api;
pub mod app;
// Temporarily disabled due to compilation issues
// pub mod app_testable;
pub mod chat;
pub mod config;
pub mod custom_spinner;
pub mod input_handler;
pub mod modern_input;
pub mod output;
pub mod overlay_menu;
pub mod test_utils;
pub mod tool_call;
pub mod tools;
pub mod visioneer;

// Testing infrastructure (only available during testing)
// Temporarily disabled due to compilation issues
// #[cfg(test)]
// pub mod testing;

// Re-export commonly used types
pub use app::App;
pub use output::OutputHandler;
pub use modern_input::ModernInputHandler;
