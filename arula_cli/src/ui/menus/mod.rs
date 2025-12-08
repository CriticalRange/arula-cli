//! Modular menu system for ARULA CLI
//!
//! This module provides a clean, organized approach to menu handling with
//! separate modules for different menu types and shared utilities.

pub mod api_key_selector;
pub mod common;
pub mod config_menu;
pub mod conversation_menu;
pub mod dialogs;
pub mod exit_menu;
pub mod main_menu;
pub mod model_selector;
pub mod provider_menu;

// Re-export commonly used types for internal convenience
pub use config_menu::ConfigMenu;
pub use conversation_menu::ConversationMenu;

// Re-export shared drawing functions for use by all menu modules
pub use common::{draw_menu_item, draw_modern_box, draw_selected_item, draw_unselected_item};
