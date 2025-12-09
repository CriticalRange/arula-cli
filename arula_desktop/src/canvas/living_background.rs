use crate::animation::LivingBackgroundState;
use crate::theme::PaletteColors;
use iced::mouse;
use iced::widget::canvas::{self, Geometry, Path};
use iced::{Color, Rectangle, Theme};
use std::marker::PhantomData;

/// Canvas program for the animated living background with aurora beams and particles.
/// Canvas program for the animated living background with aurora beams and particles.
pub struct LivingBackground<'a, Message> {
    pub state: &'a LivingBackgroundState,
    pub palette: PaletteColors,
    pub opacity: f32,
    pub _marker: PhantomData<Message>,
}

impl<'a, Message> LivingBackground<'a, Message> {
    pub fn new(state: &'a LivingBackgroundState, palette: PaletteColors, opacity: f32) -> Self {
        Self {
            state,
            palette,
            opacity,
            _marker: PhantomData,
        }
    }
}

impl<'a, Message> canvas::Program<Message> for LivingBackground<'a, Message> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let living = self.state.cache.draw(renderer, bounds.size(), |frame| {
            let center = frame.center();
            // Use sway angle from state
            let rotation = self.state.sway_angle;
            let travel = self.state.travel;

            // Interpolate background color between Palette Background (Purple) and Neutral Gray
            let active_bg = self.palette.background;
            let disabled_bg = Color::from_rgb(0.1, 0.1, 0.1); // Dark Gray

            // self.opacity = 1.0 (Fully Active/Purple) -> 0.0 (Disabled/Gray)
            let r = active_bg.r * self.opacity + disabled_bg.r * (1.0 - self.opacity);
            let g = active_bg.g * self.opacity + disabled_bg.g * (1.0 - self.opacity);
            let b = active_bg.b * self.opacity + disabled_bg.b * (1.0 - self.opacity);
            let bg_color = Color::new(r, g, b, 1.0);

            // Fill background
            frame.fill_rectangle(
                iced::Point::ORIGIN,
                bounds.size(),
                canvas::Fill::from(bg_color),
            );

            // Optimization: If opacity is ~0, don't draw grid/features
            if self.opacity <= 0.01 {
                return;
            }

            // 3D Projection Parameters
            let fov = 250.0; // Field of view / focal length
            let visibility = 2000.0; // Max draw distance
            let grid_spacing = 100.0;
            let num_lines = 40; // How many horizontal lines to draw

            // Helper to project 3D point (x, y, z) to 2D screen space (x, y)
            let project = |x: f32, y: f32, z: f32| -> Option<iced::Point> {
                if z <= 1.0 {
                    return None;
                } // Behind camera
                let scale = fov / z;

                // Apply rotation
                let rx = x * rotation.cos() - y * rotation.sin();
                let ry = x * rotation.sin() + y * rotation.cos();

                Some(iced::Point::new(
                    center.x + rx * scale,
                    center.y + ry * scale,
                ))
            };

            frame.with_save(|frame| {
                // Draw multiple layers/planes for the grid (Floor and Ceiling)
                // y_level is the vertical distance from camera center
                for y_level in [-200.0, 200.0] {
                    // 1. Draw Longitudinal Lines (going into distance)
                    // These lines span from near to far Z
                    for i in -15..=15 {
                        let x_pos = i as f32 * grid_spacing;

                        // We draw this line segment by segment to apply fog correctly (gradient alpha)
                        // Or just draw one long line with start/end alpha.
                        // For simplicity/performance, we draw a single path but calculate alpha based on average depth.
                        // Actually, to get curves right with rotation, we need simple start/end points.

                        let z_start = 10.0;
                        let z_end = visibility;

                        if let (Some(p1), Some(p2)) = (
                            project(x_pos, y_level, z_start),
                            project(x_pos, y_level, z_end),
                        ) {
                            let stroke = canvas::Stroke {
                                style: canvas::Style::Solid(Color {
                                    a: 0.15 * self.opacity,
                                    ..self.palette.accent
                                }),
                                width: 1.0,
                                line_cap: canvas::LineCap::Round,
                                ..Default::default()
                            };
                            frame.stroke(&Path::line(p1, p2), stroke);
                        }
                    }

                    // 2. Draw Transverse Lines (moving towards camera)
                    // Infinite scrolling: offset Z by (travel % spacing)
                    let z_offset = travel % grid_spacing;

                    for i in 0..num_lines {
                        // Calculate Z depth for this line
                        // Draw from far to near? Order doesn't matter for lines much.
                        let z = visibility - (i as f32 * grid_spacing) - z_offset;

                        if z < 10.0 {
                            continue;
                        } // Too close/behind

                        // Calculate fade based on distance (Linear fog)
                        let alpha = (1.0 - (z / visibility)).max(0.0) * 0.3 * self.opacity; // Max opacity 0.3 * fade state
                        if alpha <= 0.01 {
                            continue;
                        }

                        let width = 1500.0; // Width of the grid plane

                        if let (Some(p1), Some(p2)) =
                            (project(-width, y_level, z), project(width, y_level, z))
                        {
                            let stroke = canvas::Stroke {
                                style: canvas::Style::Solid(Color {
                                    a: alpha,
                                    ..self.palette.accent
                                }),
                                width: 1.5, // Slightly thicker horizontal lines
                                line_cap: canvas::LineCap::Round,
                                ..Default::default()
                            };
                            frame.stroke(&Path::line(p1, p2), stroke);
                        }
                    }
                }
            });
        });
        vec![living]
    }
}
