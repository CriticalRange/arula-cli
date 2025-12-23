use super::palette::{palette_from_mode, PaletteColors, ThemeMode};
use iced::{theme, Theme};

/// Creates the custom Arula Neon theme with default palette.
pub fn app_theme() -> Theme {
    app_theme_with_mode(ThemeMode::default())
}

/// Creates the custom Arula Neon theme with a specific theme mode.
pub fn app_theme_with_mode(mode: ThemeMode) -> Theme {
    let p = palette_from_mode(mode);
    Theme::custom(
        format!("Arula Neon - {}", mode.name()),
        theme::Palette {
            background: p.background,
            text: p.text,
            primary: p.accent,
            success: p.success,
            danger: p.danger,
            warning: p.accent_soft, // Use accent_soft as warning color
        },
    )
}

/// Creates the custom Arula Neon theme with custom palette colors.
pub fn app_theme_with_palette(p: PaletteColors) -> Theme {
    Theme::custom(
        "Arula Neon".to_string(),
        theme::Palette {
            background: p.background,
            text: p.text,
            primary: p.accent,
            success: p.success,
            danger: p.danger,
            warning: p.accent_soft, // Use accent_soft as warning color
        },
    )
}
