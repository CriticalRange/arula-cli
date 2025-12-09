use super::palette::palette;
use iced::{theme, Theme};

/// Creates the custom Arula Neon theme.
pub fn app_theme() -> Theme {
    let p = palette();
    Theme::custom(
        "Arula Neon".to_string(),
        theme::Palette {
            background: p.background,
            text: p.text,
            primary: p.accent,
            success: p.success,
            danger: p.danger,
        },
    )
}
