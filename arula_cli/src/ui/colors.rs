//! Color utilities for terminal effects
//!
//! This module provides shared color conversion and utility functions
//! for use across the ARULA CLI UI components.

/// Convert HSV color values to RGB
///
/// # Arguments
/// * `h` - Hue in degrees (0-360)
/// * `s` - Saturation (0.0-1.0)
/// * `v` - Value/brightness (0.0-1.0)
///
/// # Returns
/// A tuple of (r, g, b) values each in range 0-255
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r_prime, g_prime, b_prime) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let r = ((r_prime + m) * 255.0) as u8;
    let g = ((g_prime + m) * 255.0) as u8;
    let b = ((b_prime + m) * 255.0) as u8;

    (r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsv_to_rgb_red() {
        let (r, g, b) = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_hsv_to_rgb_green() {
        let (r, g, b) = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_hsv_to_rgb_blue() {
        let (r, g, b) = hsv_to_rgb(240.0, 1.0, 1.0);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 255);
    }

    #[test]
    fn test_hsv_to_rgb_white() {
        let (r, g, b) = hsv_to_rgb(0.0, 0.0, 1.0);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);
    }

    #[test]
    fn test_hsv_to_rgb_black() {
        let (r, g, b) = hsv_to_rgb(0.0, 0.0, 0.0);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }
}
