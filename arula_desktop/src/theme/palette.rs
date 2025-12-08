use iced::Color;

/// Core color palette for the Arula Neon theme.
#[derive(Debug, Clone, Copy)]
pub struct PaletteColors {
    pub background: Color,
    pub surface: Color,
    pub surface_raised: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub success: Color,
    pub danger: Color,
    pub glow: Color,
}

impl Default for PaletteColors {
    fn default() -> Self {
        Self {
            background: Color::from_rgb8(10, 8, 14),
            surface: Color::from_rgb8(18, 14, 24),
            surface_raised: Color::from_rgb8(26, 20, 34),
            border: Color::from_rgb8(50, 40, 70),
            text: Color::from_rgb8(240, 235, 255),
            muted: Color::from_rgb8(150, 140, 180),
            accent: Color::from_rgb8(180, 50, 255),      // Vibrant Purple
            accent_soft: Color::from_rgb8(140, 40, 200), // Soft Purple
            success: Color::from_rgb8(100, 255, 140),
            danger: Color::from_rgb8(255, 100, 100),
            glow: Color::from_rgb8(200, 100, 255),       // Bright Glow
        }
    }
}

/// Returns the default palette for the application.
pub fn palette() -> PaletteColors {
    PaletteColors::default()
}
