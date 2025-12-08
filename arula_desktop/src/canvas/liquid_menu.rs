use iced::mouse;
use iced::widget::canvas::{self, Geometry, Path};
use iced::{Color, Point, Rectangle, Theme};
use crate::animation::LiquidMenuState;
use crate::theme::PaletteColors;
use std::marker::PhantomData;

/// Canvas program for the liquid expanding menu background.
pub struct LiquidMenuBackground<'a, Message> {
    pub state: &'a LiquidMenuState,
    pub palette: PaletteColors,
    pub _marker: PhantomData<Message>,
}

impl<'a, Message> LiquidMenuBackground<'a, Message> {
    pub fn new(state: &'a LiquidMenuState, palette: PaletteColors) -> Self {
        Self { state, palette, _marker: PhantomData }
    }
}

impl<'a, Message> canvas::Program<Message> for LiquidMenuBackground<'a, Message> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let liquid = self.state.cache.draw(renderer, bounds.size(), |frame| {
            let progress = self.state.spring.position;
            if progress < 0.01 {
                return;
            }

            // Anchor point at bottom left
            let anchor = Point::new(40.0, bounds.height - 40.0);
            let max_radius = bounds.width.max(bounds.height) * 1.8;
            let current_radius = max_radius * progress;

            let color = Color {
                a: 0.98 * progress.min(1.0),
                ..self.palette.background
            };
            let circle = Path::circle(anchor, current_radius);
            frame.fill(&circle, color);
        });
        vec![liquid]
    }
}
