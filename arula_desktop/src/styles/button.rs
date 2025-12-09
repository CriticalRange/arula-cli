use crate::constants::BUTTON_BORDER_RADIUS;
use crate::theme::PaletteColors;
use iced::widget::button;
use iced::{Background, Border, Color, Shadow, Theme, Vector};

/// Primary accent button style with glow on hover.
pub fn primary_button_style(
    palette: PaletteColors,
) -> impl Fn(&Theme, button::Status) -> button::Style + Clone {
    move |_, status| {
        let base = button::Style {
            background: Some(Background::Color(palette.accent)),
            text_color: palette.background,
            border: Border {
                color: palette.accent,
                width: 1.0,
                radius: BUTTON_BORDER_RADIUS.into(),
            },
            shadow: Shadow::default(),
        };
        match status {
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color {
                    a: 0.9,
                    ..palette.accent
                })),
                shadow: Shadow {
                    color: palette.accent,
                    blur_radius: 10.0,
                    offset: Vector::default(),
                },
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(palette.accent_soft)),
                ..base
            },
            _ => base,
        }
    }
}

/// Secondary/close button style with subtle border.
pub fn secondary_button_style(
    palette: PaletteColors,
) -> impl Fn(&Theme, button::Status) -> button::Style + Clone {
    move |_, _status| button::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: palette.text,
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: BUTTON_BORDER_RADIUS.into(),
        },
        shadow: Shadow::default(),
    }
}

/// Icon button style for menu toggle.
pub fn icon_button_style(
    palette: PaletteColors,
    is_open: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + Clone {
    move |_, status| {
        let base_bg = if is_open {
            Background::Color(palette.background)
        } else {
            Background::Color(palette.surface_raised)
        };
        let border_color = if is_open {
            palette.muted
        } else {
            palette.accent
        };
        let text_color = if is_open {
            palette.text
        } else {
            palette.accent
        };

        let base = button::Style {
            background: Some(base_bg),
            text_color,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: Shadow::default(),
        };

        match status {
            button::Status::Hovered => button::Style {
                border: Border {
                    color: palette.glow,
                    ..base.border
                },
                shadow: Shadow {
                    color: palette.glow,
                    blur_radius: 8.0,
                    offset: Vector::default(),
                },
                ..base
            },
            _ => base,
        }
    }
}

/// Send button style with asymmetric border radius.
pub fn send_button_style(
    palette: PaletteColors,
) -> impl Fn(&Theme, button::Status) -> button::Style + Clone {
    move |_, status| {
        let shadow = match status {
            button::Status::Hovered => Shadow {
                color: palette.accent,
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            _ => Shadow::default(),
        };

        button::Style {
            background: Some(Background::Color(palette.accent)),
            text_color: palette.background,
            border: Border {
                radius: iced::border::Radius {
                    top_left: 0.0,
                    top_right: 24.0,
                    bottom_right: 24.0,
                    bottom_left: 0.0,
                },
                ..Default::default()
            },
            shadow,
        }
    }
}

/// Cog/settings button style with rounded corners and glow on hover.
pub fn cog_button_container_style_button(
    palette: PaletteColors,
    is_open: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + Clone {
    move |_, status| {
        let base_bg = if is_open {
            Background::Color(palette.background)
        } else {
            Background::Color(palette.surface_raised)
        };
        let border_color = if is_open {
            palette.muted
        } else {
            palette.accent
        };
        let text_color = if is_open {
            palette.text
        } else {
            palette.accent
        };

        let base = button::Style {
            background: Some(base_bg),
            text_color,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: Shadow::default(),
        };

        match status {
            button::Status::Hovered => button::Style {
                border: Border {
                    color: palette.glow,
                    ..base.border
                },
                shadow: Shadow {
                    color: palette.glow,
                    blur_radius: 10.0,
                    offset: Vector::default(),
                },
                ..base
            },
            _ => base,
        }
    }
}
