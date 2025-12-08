use iced::widget::text_input;
use iced::{Background, Border, Color, Theme};
use crate::theme::PaletteColors;

/// Creates a styled text input with accent border on focus.
pub fn input_style(palette: PaletteColors) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + Clone {
    move |_, status| {
        let is_focused = matches!(status, text_input::Status::Focused | text_input::Status::Hovered);
        let border_color = if is_focused { palette.accent } else { palette.border };
        text_input::Style {
            background: Background::Color(Color { a: 0.5, ..palette.surface_raised }),
            border: Border { 
                color: border_color, 
                width: 1.0, 
                radius: 8.0.into() 
            },
            icon: palette.muted,
            placeholder: palette.muted,
            value: palette.text,
            selection: palette.accent,
        }
    }
}

/// Creates a transparent input style for the chat input area.
pub fn chat_input_style(palette: PaletteColors) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + Clone {
    move |_, _status| {
        text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border { 
                color: Color::TRANSPARENT, 
                width: 0.0, 
                radius: 0.0.into() 
            },
            icon: palette.muted,
            placeholder: palette.muted,
            value: palette.text,
            selection: palette.accent,
        }
    }
}
