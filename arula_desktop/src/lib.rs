//! Arula Desktop - A modern AI assistant GUI built with Iced.

pub mod animation;
pub mod canvas;
pub mod config;
pub mod constants;
pub mod dispatcher;
pub mod session;
pub mod styles;
pub mod theme;

pub use animation::{LivingBackgroundState, LiquidMenuState, TiltCardState, SettingsMenuState, SettingsPage, TransitionDirection};
pub use config::{ConfigForm, collect_provider_options};
pub use constants::*;
pub use dispatcher::Dispatcher;
// Re-export UiEvent from core for convenience
pub use arula_core::UiEvent;
pub use session::{MessageEntry, Session};
pub use styles::*;
pub use theme::{PaletteColors, app_theme, palette};
