use std::f32::consts::PI;
use std::marker::PhantomData;
use iced::advanced::graphics::gradient;
use iced::mouse;
use iced::widget::canvas::{self, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Theme};
use crate::animation::TiltCardState;
use crate::theme::PaletteColors;

/// Canvas program for tilt-responsive cards with glare effect.
pub struct TiltCardCanvas<'a, Message> {
    pub state: &'a TiltCardState,
    pub base_color: Color,
    pub palette: PaletteColors,
    pub _marker: PhantomData<Message>,
}

impl<'a, Message> TiltCardCanvas<'a, Message> {
    pub fn new(state: &'a TiltCardState, base_color: Color, palette: PaletteColors) -> Self {
        Self { state, base_color, palette, _marker: PhantomData }
    }
}

impl<'a, Message> canvas::Program<Message> for TiltCardCanvas<'a, Message> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let card = self.state.cache.draw(renderer, bounds.size(), |frame| {
            let center = frame.center();
            let mouse_p = if self.state.is_hovered {
                self.state.mouse_position
            } else {
                center
            };

            let dx = (mouse_p.x - center.x) / (bounds.width / 2.0);
            let dy = (mouse_p.y - center.y) / (bounds.height / 2.0);

            // Draw card background
            let card_path = Path::rectangle(Point::ORIGIN, bounds.size());
            frame.fill(&card_path, self.base_color);

            // Draw border with pulse effect on hover
            let pulse = (self.state.hover_tick.sin() + 1.0) * 0.5;
            let border_alpha = if self.state.is_hovered {
                0.8 + (0.2 * pulse)
            } else {
                0.1
            };
            let border_color = if self.state.is_hovered {
                self.palette.accent
            } else {
                Color::WHITE
            };
            let stroke_width = if self.state.is_hovered { 2.0 } else { 1.0 };

            frame.stroke(
                &card_path,
                Stroke::default()
                    .with_color(Color { a: border_alpha, ..border_color })
                    .with_width(stroke_width),
            );

            // Draw glare effect on hover
            if self.state.is_hovered {
                let angle = dx * 0.5 + PI / 4.0;
                let glare_len = bounds.width * 1.5;
                let cx = center.x + (dx * bounds.width * 0.2);
                let cy = center.y + (dy * bounds.height * 0.2);

                let start = Point::new(
                    cx + (angle.cos() * glare_len * 0.5),
                    cy + (angle.sin() * glare_len * 0.5),
                );
                let end = Point::new(
                    cx - (angle.cos() * glare_len * 0.5),
                    cy - (angle.sin() * glare_len * 0.5),
                );

                let glare = gradient::Linear::new(start, end)
                    .add_stop(0.0, Color::TRANSPARENT)
                    .add_stop(0.5, Color { a: 0.1, ..Color::WHITE })
                    .add_stop(1.0, Color::TRANSPARENT);

                frame.fill(&card_path, glare);
            }
        });
        vec![card]
    }
}
