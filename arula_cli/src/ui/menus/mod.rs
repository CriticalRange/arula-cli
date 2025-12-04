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
pub mod conversation_menu;

// Re-export commonly used types for internal convenience
pub use config_menu::ConfigMenu;
pub use conversation_menu::ConversationMenu;