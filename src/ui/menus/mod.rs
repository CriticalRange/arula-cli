//! Modular menu system for ARULA CLI
//!
//! This module provides a clean, organized approach to menu handling with
//! separate modules for different menu types and shared utilities.

pub mod common;
pub mod main_menu;
pub mod config_menu;
pub mod provider_menu;
pub mod model_selector;
pub mod api_key_selector;
pub mod exit_menu;
pub mod dialogs;

// Re-export commonly used types for convenience
pub use common::MenuResult;
pub use main_menu::MainMenu;
pub use config_menu::ConfigMenu;
pub use exit_menu::ExitMenu;