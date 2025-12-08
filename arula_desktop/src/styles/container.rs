use iced::widget::container;
use iced::{Background, Border, Color, Theme};
use crate::theme::PaletteColors;
use crate::constants::{CARD_BORDER_RADIUS, INPUT_BORDER_RADIUS};

/// Message bubble style for user messages.
pub fn user_bubble_style(palette: PaletteColors) -> impl Fn(&Theme) -> container::Style + Clone {
    move |_theme: &Theme| {
        container::Style {
            background: Some(Background::Color(Color { a: 0.2, ..palette.accent })),
            text_color: Some(palette.text),
            border: Border { 
                color: Color { a: 0.5, ..palette.accent }, 
                width: 1.0, 
                radius: CARD_BORDER_RADIUS.into() 
            },
            ..Default::default()
        }
    }
}

/// Message bubble style for AI messages.
pub fn ai_bubble_style(palette: PaletteColors, _is_streaming: bool) -> impl Fn(&Theme) -> container::Style + Clone {
    move |_theme: &Theme| {
        let opacity = 1.0;
        let text_opacity = 1.0;

        container::Style {
            // Apply opacity to background color
            background: Some(Background::Color(Color { 
                a: 0.4 * opacity, 
                ..palette.surface_raised 
            })),
            // Apply opacity to text color
            text_color: Some(Color { 
                a: text_opacity, 
                ..palette.text 
            }),
            border: Border { 
                color: Color::TRANSPARENT, 
                width: 0.0, 
                radius: CARD_BORDER_RADIUS.into() 
            },
            ..Default::default()
        }
    }
}

/// Chat input container with rounded border.
pub fn chat_input_container_style(palette: PaletteColors) -> impl Fn(&Theme) -> container::Style + Clone {
    move |_| {
        container::Style {
            background: Some(Background::Color(Color { a: 0.5, ..palette.surface_raised })),
            border: Border { 
                color: palette.border, 
                width: 1.0, 
                radius: INPUT_BORDER_RADIUS.into() 
            },
            ..Default::default()
        }
    }
}

/// Transparent container style.
pub fn transparent_style() -> impl Fn(&Theme) -> container::Style + Clone {
    move |_| container::Style { 
        background: None, 
        ..Default::default() 
    }
}

/// Cog/settings button container style with rounded corners.
pub fn cog_button_container_style(palette: PaletteColors, is_open: bool) -> impl Fn(&Theme) -> container::Style + Clone {
    move |_| {
        let base_bg = if is_open { 
            Background::Color(palette.background) 
        } else { 
            Background::Color(palette.surface_raised) 
        };
        let border_color = if is_open { palette.muted } else { palette.accent };
        
        container::Style {
            background: Some(base_bg),
            border: Border { 
                color: border_color, 
                width: 1.0, 
                radius: 12.0.into() 
            },
            ..Default::default()
        }
    }
}
