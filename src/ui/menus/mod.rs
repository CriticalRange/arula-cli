//! Modular menu system for ARULA CLI
//!
//! This module provides a clean, organized approach to menu handling with
//! separate modules for different menu types and shared utilities.

pub mod common;
pub mod main_menu;
pub mod config_menu;
pub mod provider_menu;
pub mod model_selector;
pub mod dialogs;

// Re-export commonly used types for convenience
pub use common::{MenuResult, MenuAction};
pub use main_menu::MainMenu;
pub use config_menu::ConfigMenu;
pub use provider_menu::ProviderMenu;
pub use model_selector::ModelSelector;
pub use dialogs::Dialogs;