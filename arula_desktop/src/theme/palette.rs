use iced::Color;

/// Theme mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
    Black,
}

impl ThemeMode {
    pub fn name(&self) -> &'static str {
        match self {
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
            ThemeMode::Black => "Black",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "light" => Some(ThemeMode::Light),
            "dark" => Some(ThemeMode::Dark),
            "black" => Some(ThemeMode::Black),
            _ => None,
        }
    }

    pub fn all() -> Vec<&'static str> {
        vec!["Light", "Dark", "Black"]
    }
}

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
        Self::dark()
    }
}

impl PaletteColors {
    /// Light theme palette
    pub fn light() -> Self {
        Self {
            background: Color::from_rgb8(248, 250, 255),  // Very light blue
            surface: Color::from_rgb8(240, 244, 255),     // Light blue-gray
            surface_raised: Color::from_rgb8(255, 255, 255), // Pure white
            border: Color::from_rgb8(200, 210, 230),      // Light blue-gray border
            text: Color::from_rgb8(20, 30, 50),           // Dark blue-gray text
            muted: Color::from_rgb8(110, 120, 140),       // Medium gray
            accent: Color::from_rgb8(80, 120, 220),       // Blue accent (very distinct from dark purple)
            accent_soft: Color::from_rgb8(150, 180, 255), // Light blue
            success: Color::from_rgb8(40, 160, 80),       // Green
            danger: Color::from_rgb8(220, 60, 60),        // Red
            glow: Color::from_rgb8(100, 150, 255),        // Blue glow
        }
    }

    /// Dark theme palette
    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb8(10, 8, 14),      // Deep purple-black
            surface: Color::from_rgb8(18, 14, 24),        // Dark purple-gray
            surface_raised: Color::from_rgb8(26, 20, 34), // Raised purple-gray
            border: Color::from_rgb8(50, 40, 70),         // Purple border
            text: Color::from_rgb8(240, 235, 255),        // Off-white
            muted: Color::from_rgb8(150, 140, 180),       // Light purple-gray
            accent: Color::from_rgb8(180, 50, 255),       // Vibrant purple
            accent_soft: Color::from_rgb8(140, 40, 200),  // Soft purple
            success: Color::from_rgb8(100, 255, 140),     // Bright green
            danger: Color::from_rgb8(255, 100, 100),      // Bright red
            glow: Color::from_rgb8(200, 100, 255),        // Purple glow
        }
    }

    /// Black theme palette (darker than dark)
    pub fn black() -> Self {
        Self {
            background: Color::from_rgb8(0, 0, 0),        // Pure black (very distinct!)
            surface: Color::from_rgb8(5, 5, 8),           // Nearly black
            surface_raised: Color::from_rgb8(10, 10, 15), // Very dark gray
            border: Color::from_rgb8(25, 20, 35),         // Dark border
            text: Color::from_rgb8(250, 245, 255),        // Pure white text
            muted: Color::from_rgb8(140, 130, 170),       // Light purple-gray
            accent: Color::from_rgb8(200, 80, 255),       // Extra vibrant purple
            accent_soft: Color::from_rgb8(160, 60, 220),  // Soft purple
            success: Color::from_rgb8(120, 255, 150),     // Bright green
            danger: Color::from_rgb8(255, 110, 110),      // Bright red
            glow: Color::from_rgb8(220, 120, 255),        // Extra bright glow
        }
    }

    /// Create palette from theme mode
    pub fn from_theme_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Black => Self::black(),
        }
    }
}

/// Returns the default palette for the application.
pub fn palette() -> PaletteColors {
    PaletteColors::default()
}

/// Returns palette for a specific theme mode
pub fn palette_from_mode(mode: ThemeMode) -> PaletteColors {
    PaletteColors::from_theme_mode(mode)
}
