//! Advanced loading spinner animations for ARULA Desktop
//!
//! Provides multiple animated loading indicators using Iced canvas.

use iced::advanced::graphics::gradient;
use iced::widget::canvas::{self, Cache, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Theme};
use std::f32::consts::PI;

/// Types of loading animations
pub enum SpinnerType {
    /// Rotating ring with gradient
    Ring,
    /// Orbital dots
    Orbital,
    /// Pulsing circle
    Pulse,
    /// Morphing shapes
    Morph,
    /// Wave pattern
    Wave,
}

/// Animated loading spinner canvas
pub struct LoadingSpinner {
    state: SpinnerState,
    cache: Cache,
}

/// Animation state
pub struct SpinnerState {
    pub tick: f32,
    pub spinner_type: SpinnerType,
    pub size: f32,
    pub color: Color,
    pub accent_color: Color,
}

impl LoadingSpinner {
    /// Create a new loading spinner
    pub fn new(state: SpinnerState) -> Self {
        Self {
            state,
            cache: Cache::new(),
        }
    }

    /// Set the spinner type
    pub fn with_type(mut self, spinner_type: SpinnerType) -> Self {
        self.state.spinner_type = spinner_type;
        self
    }

    /// Set the spinner size
    pub fn with_size(mut self, size: f32) -> Self {
        self.state.size = size;
        self
    }

    /// Set the primary color
    pub fn with_color(mut self, color: Color) -> Self {
        self.state.color = color;
        self
    }

    /// Set the accent color
    pub fn with_accent_color(mut self, accent_color: Color) -> Self {
        self.state.accent_color = accent_color;
        self
    }

    /// Update animation state
    pub fn update(&mut self, delta: f32) {
        self.state.tick += delta;
    }
}

impl<Message> canvas::Program<Message> for LoadingSpinner {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        vec![self.cache.draw(renderer, bounds.size(), |frame| {
            let center = frame.center();
            let time = self.state.tick;

            match self.state.spinner_type {
                SpinnerType::Ring => {
                    // Rotating ring with gradient
                    let segments = 12;
                    let inner_radius = self.state.size * 0.6;
                    let outer_radius = self.state.size * 0.8;

                    for i in 0..segments {
                        let angle = (i as f32 / segments as f32) * 2.0 * PI;
                        let next_angle = ((i + 1) as f32 / segments as f32) * 2.0 * PI;

                        // Rotate over time
                        let rotation = time * 2.0;
                        let start_angle = angle + rotation;
                        let end_angle = next_angle + rotation;

                        // Create arc segment
                        let start_point = Point::new(
                            center.x + start_angle.cos() * outer_radius,
                            center.y + start_angle.sin() * outer_radius,
                        );
                        let _end_point = Point::new(
                            center.x + end_angle.cos() * outer_radius,
                            center.y + end_angle.sin() * outer_radius,
                        );

                        let _inner_start = Point::new(
                            center.x + start_angle.cos() * inner_radius,
                            center.y + start_angle.sin() * inner_radius,
                        );
                        let inner_end = Point::new(
                            center.x + end_angle.cos() * inner_radius,
                            center.y + end_angle.sin() * inner_radius,
                        );

                        // Create path for arc segment
                        let path = Path::new(|builder| {
                            builder.move_to(start_point);
                            // Approximate arc with line segments
                            const SEGMENTS: usize = 10;
                            for i in 0..=SEGMENTS {
                                let t = i as f32 / SEGMENTS as f32;
                                let angle = start_angle + (end_angle - start_angle) * t;
                                let point = Point::new(
                                    center.x + angle.cos() * outer_radius,
                                    center.y + angle.sin() * outer_radius,
                                );
                                if i == 0 {
                                    builder.move_to(point);
                                } else {
                                    builder.line_to(point);
                                }
                            }
                            builder.line_to(inner_end);
                            // Approximate inner arc
                            for i in (0..=SEGMENTS).rev() {
                                let t = i as f32 / SEGMENTS as f32;
                                let angle = end_angle - (end_angle - start_angle) * t;
                                let point = Point::new(
                                    center.x + angle.cos() * inner_radius,
                                    center.y + angle.sin() * inner_radius,
                                );
                                builder.line_to(point);
                            }
                            builder.close();
                        });

                        // Fade segments
                        let fade = ((i as f32 + 1.0) / segments as f32).powf(2.0);
                        let alpha = 0.3 + (fade * 0.7);

                        frame.fill(
                            &path,
                            Color {
                                a: alpha,
                                ..self.state.color
                            },
                        );
                    }
                }
                SpinnerType::Orbital => {
                    // Orbital dots with trailing effect
                    let dots = 8;
                    let radius = self.state.size * 0.7;
                    let dot_size = self.state.size * 0.15;

                    for i in 0..dots {
                        let progress = i as f32 / dots as f32;
                        let angle = progress * 2.0 * PI + time * 3.0;

                        let x = center.x + angle.cos() * radius;
                        let y = center.y + angle.sin() * radius;

                        let dot_path = Path::circle(Point::new(x, y), dot_size);

                        // Gradient trail effect
                        let t = ((time * 3.0 + progress * 2.0 * PI) % (2.0 * PI)) / (2.0 * PI);
                        let alpha = 0.3 + (t.sin() * 0.5 + 0.5) * 0.7;

                        // Mix colors based on position
                        let color_factor = (angle.sin() + 1.0) * 0.5;
                        let color = Color {
                            r: self.state.color.r * (1.0 - color_factor)
                                + self.state.accent_color.r * color_factor,
                            g: self.state.color.g * (1.0 - color_factor)
                                + self.state.accent_color.g * color_factor,
                            b: self.state.color.b * (1.0 - color_factor)
                                + self.state.accent_color.b * color_factor,
                            a: alpha,
                        };

                        frame.fill(&dot_path, color);

                        // Add glow effect
                        let glow_path = Path::circle(Point::new(x, y), dot_size * 2.0);
                        frame.fill(
                            &glow_path,
                            Color {
                                a: alpha * 0.2,
                                ..color
                            },
                        );
                    }
                }
                SpinnerType::Pulse => {
                    // Pulsing concentric circles
                    let circles = 4;

                    for i in 0..circles {
                        let progress = (time * 2.0 + i as f32 * 0.5) % (circles as f32);
                        let t = progress / circles as f32;
                        let radius = self.state.size * (0.3 + t * 0.7);
                        let alpha = (1.0 - t) * 0.6;

                        let circle_path = Path::circle(center, radius);
                        frame.stroke(
                            &circle_path,
                            Stroke::default().with_width(2.0).with_color(Color {
                                a: alpha,
                                ..self.state.color
                            }),
                        );
                    }
                }
                SpinnerType::Morph => {
                    // Morphing between shapes
                    let _morph_progress = (time * 2.0) % 1.0;
                    let sides = 3 + ((time * 0.5) % 5.0) as usize; // 3 to 8 sides

                    let mut points = Vec::new();
                    for i in 0..sides {
                        let angle = (i as f32 / sides as f32) * 2.0 * PI - PI / 2.0;
                        let radius_factor = 1.0 + (angle * 3.0 + time * 4.0).sin() * 0.2;
                        let radius = self.state.size * radius_factor;

                        let x = center.x + angle.cos() * radius;
                        let y = center.y + angle.sin() * radius;
                        points.push(Point::new(x, y));
                    }

                    let path = Path::new(|builder| {
                        if let Some(first) = points.first() {
                            builder.move_to(*first);
                            for point in points.iter().skip(1) {
                                builder.line_to(*point);
                            }
                            builder.close();
                        }
                    });

                    // Gradient fill
                    let gradient = gradient::Linear::new(
                        Point::new(center.x - self.state.size, center.y - self.state.size),
                        Point::new(center.x + self.state.size, center.y + self.state.size),
                    )
                    .add_stop(
                        0.0,
                        Color {
                            a: 0.8,
                            ..self.state.color
                        },
                    )
                    .add_stop(
                        1.0,
                        Color {
                            a: 0.4,
                            ..self.state.accent_color
                        },
                    );

                    frame.fill(&path, gradient);
                }
                SpinnerType::Wave => {
                    // Wave pattern
                    let points = 40;
                    let wave_height = self.state.size * 0.3;

                    for i in 0..points {
                        let progress = i as f32 / points as f32;
                        let x = center.x - self.state.size + progress * self.state.size * 2.0;

                        // Multiple waves for complexity
                        let wave1 = (progress * PI * 4.0 + time * 3.0).sin() * wave_height;
                        let wave2 = (progress * PI * 6.0 - time * 2.0).cos() * wave_height * 0.5;
                        let y = center.y + wave1 + wave2;

                        let dot_size = self.state.size * 0.05;
                        let dot_path = Path::circle(Point::new(x, y), dot_size);

                        let alpha = 0.3 + ((progress + time) % 1.0) * 0.5;
                        frame.fill(
                            &dot_path,
                            Color {
                                a: alpha,
                                ..self.state.color
                            },
                        );
                    }
                }
            }
        })]
    }
}

/// Create a default spinner state
pub fn default_spinner_state(color: Color, accent_color: Color) -> SpinnerState {
    SpinnerState {
        tick: 0.0,
        spinner_type: SpinnerType::Orbital,
        size: 20.0,
        color,
        accent_color,
    }
}
