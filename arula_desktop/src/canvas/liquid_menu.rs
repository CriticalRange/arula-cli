use crate::animation::LiquidMenuState;
use crate::theme::PaletteColors;
use iced::mouse;
use iced::widget::canvas::{self, Geometry, Path};
use iced::{Color, Point, Rectangle, Theme};
use std::marker::PhantomData;

/// Canvas program for the settings menu backdrop.
/// Creates a gentle slide-up effect with soft gradient overlay.
pub struct LiquidMenuBackground<'a, Message> {
    pub state: &'a LiquidMenuState,
    pub palette: PaletteColors,
    pub _marker: PhantomData<Message>,
}

impl<'a, Message> LiquidMenuBackground<'a, Message> {
    pub fn new(state: &'a LiquidMenuState, palette: PaletteColors) -> Self {
        Self {
            state,
            palette,
            _marker: PhantomData,
        }
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
        let backdrop = self.state.cache.draw(renderer, bounds.size(), |frame| {
            let progress = self.state.spring.position;
            if progress < 0.01 {
                return;
            }

            // Eased progress for smoother animation
            let eased = ease_out_cubic(progress.min(1.0));
            
            // Main backdrop - slides up from bottom, fully covers screen
            let slide_offset = bounds.height * (1.0 - eased) * 0.08;
            let backdrop_height = bounds.height - slide_offset;
            
            // Background color - fully opaque to cover input bar
            let base_alpha = eased;
            let bg_color = Color {
                a: base_alpha,
                ..self.palette.background
            };
            
            // Draw main backdrop rectangle covering everything
            let backdrop_rect = Path::rectangle(
                Point::new(0.0, slide_offset),
                iced::Size::new(bounds.width, backdrop_height),
            );
            frame.fill(&backdrop_rect, bg_color);
            
            // Soft gradient at the top edge for a gentle "rise" feel
            let gradient_height = 50.0 * eased;
            for i in 0..12 {
                let t = i as f32 / 12.0;
                let y = slide_offset + t * gradient_height;
                let alpha = base_alpha * (1.0 - t * 0.15);
                
                let line = Path::rectangle(
                    Point::new(0.0, y),
                    iced::Size::new(bounds.width, gradient_height / 12.0),
                );
                frame.fill(&line, Color { a: alpha, ..self.palette.background });
            }
            
            // Subtle accent line at bottom (not glow, just a thin line)
            if eased > 0.5 {
                let line_alpha = (eased - 0.5) * 0.3;
                let bottom_line = Path::rectangle(
                    Point::new(0.0, bounds.height - 2.0),
                    iced::Size::new(bounds.width, 2.0),
                );
                frame.fill(&bottom_line, Color { a: line_alpha, ..self.palette.accent });
            }
        });
        vec![backdrop]
    }
}

/// Cubic ease-out for smooth deceleration
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}
