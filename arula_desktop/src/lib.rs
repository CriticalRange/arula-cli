//! Arula Desktop - A modern AI assistant GUI built with Iced.

pub mod animation;
pub mod canvas;
pub mod config;
pub mod constants;
pub mod dispatcher;
pub mod session;
pub mod styles;
pub mod theme;

pub use animation::{
    LiquidMenuState, LivingBackgroundState, SettingsMenuState, SettingsPage, TiltCardState,
    TransitionDirection,
};
pub use config::{collect_provider_options, ConfigForm};
pub use constants::*;
pub use dispatcher::Dispatcher;
// Re-export UiEvent from core for convenience
pub use arula_core::UiEvent;
pub use session::{MessageEntry, Session};
pub use styles::*;
pub use theme::{app_theme, palette, PaletteColors};
