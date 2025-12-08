//! Terminal Effects and Animations
//!
//! Provides beautiful visual effects for terminal applications including:
//! - Glowing text animations
//! - Typewriter effects with variable speed
//! - Smooth text transitions
//! - Rainbow and pulse effects

use super::colors::hsv_to_rgb;
use crossterm::{
    cursor, execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use tokio::time::sleep as tokio_sleep;

/// Beautiful terminal effects and animations
pub struct TerminalEffects;

impl TerminalEffects {
    /// Glowing text effect with pulsing intensity
    ///
    /// # Arguments
    /// * `text` - The text to make glow
    /// * `cycles` - Number of glow cycles to perform
    ///
    /// # Example
    /// ```rust
    /// use arula_cli::ui::effects::TerminalEffects;
    ///
    /// TerminalEffects::glowing_text("✨ Loading...", 5)?;
    /// ```
    pub fn glowing_text(text: &str, cycles: u32) -> io::Result<()> {
        let mut stdout = io::stdout();
        for cycle in 0..cycles {
            let phase = (cycle as f32) / (cycles as f32);
            let intensity = (phase * std::f32::consts::PI * 2.0).sin().abs() as u8;

            let color = Color::Rgb {
                r: intensity,
                g: intensity / 2,
                b: intensity,
            };

            // Use \r for better terminal compatibility
            execute!(stdout, SetForegroundColor(color))?;
            print!("\r\x1b[2K{}", text);
            stdout.flush()?;
            thread::sleep(Duration::from_millis(100));
        }

        // Clean up with normal color
        execute!(stdout, ResetColor)?;
        print!("\r\x1b[2K{}", text);
        stdout.flush()?;
        Ok(())
    }

    /// Typewriter effect with variable speed based on character type
    ///
    /// # Arguments
    /// * `text` - The text to type out
    /// * `base_delay` - Base delay between characters
    ///
    /// # Example
    /// ```rust
    /// use std::time::Duration;
    /// use arula_cli::ui::effects::TerminalEffects;
    ///
    /// TerminalEffects::typewriter_async("Hello, World!", Duration::from_millis(30)).await?;
    /// ```
    pub async fn typewriter_async(text: &str, base_delay: Duration) -> io::Result<()> {
        let chars: Vec<char> = text.chars().collect();
        for ch in chars.iter() {
            print!("{}", ch);
            io::stdout().flush()?;

            // Variable delay for organic feel
            let delay = if "*!.?".contains(*ch) {
                base_delay * 3
            } else if ch.is_whitespace() {
                base_delay * 2
            } else {
                base_delay
            };

            tokio_sleep(delay).await;
        }
        Ok(())
    }

    /// Synchronous version of typewriter effect
    pub fn typewriter(text: &str, base_delay: Duration) -> io::Result<()> {
        let chars: Vec<char> = text.chars().collect();
        for ch in chars.iter() {
            print!("{}", ch);
            io::stdout().flush()?;

            // Variable delay for organic feel
            let delay = if "*!.?".contains(*ch) {
                base_delay * 3
            } else if ch.is_whitespace() {
                base_delay * 2
            } else {
                base_delay
            };

            thread::sleep(delay);
        }
        Ok(())
    }

    /// Rainbow color transition effect
    ///
    /// # Arguments
    /// * `text` - The text to color with rainbow effect
    /// * `cycles` - Number of rainbow cycles
    /// * `speed_ms` - Speed of color change in milliseconds
    pub fn rainbow_text(text: &str, cycles: u32, speed_ms: u64) -> io::Result<()> {
        let mut stdout = io::stdout();
        let chars: Vec<char> = text.chars().collect();

        for cycle in 0..cycles {
            for (i, ch) in chars.iter().enumerate() {
                let phase = ((i + cycle as usize) as f32) / (chars.len() as f32);
                let hue = phase * 360.0;

                // Convert HSV to RGB (simplified)
                let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
                let color = Color::Rgb { r, g, b };

                execute!(stdout, SetForegroundColor(color), Print(ch))?;
                stdout.flush()?;
                thread::sleep(Duration::from_millis(speed_ms));
            }

            // Move to start for next cycle - use \r for terminal compatibility
            print!("\r\x1b[2K");
            stdout.flush()?;
        }

        execute!(stdout, ResetColor)?;
        print!("{}", text);
        stdout.flush()?;
        Ok(())
    }

    /// Fade in text effect
    ///
    /// # Arguments
    /// * `text` - The text to fade in
    /// * `steps` - Number of fade steps
    /// * `delay_ms` - Delay between steps
    pub fn fade_in_text(text: &str, steps: u32, delay_ms: u64) -> io::Result<()> {
        let mut stdout = io::stdout();

        for step in 0..=steps {
            let intensity = (step as f32) / (steps as f32);
            let color = Color::Rgb {
                r: (intensity * 205.0) as u8, // Light gray base
                g: (intensity * 209.0) as u8,
                b: (intensity * 196.0) as u8,
            };

            // Use \r for better terminal compatibility
            execute!(stdout, SetForegroundColor(color))?;
            print!("\r\x1b[2K{}", text);
            stdout.flush()?;
            thread::sleep(Duration::from_millis(delay_ms));
        }

        execute!(stdout, ResetColor)?;
        stdout.flush()?;
        Ok(())
    }

    /// Sliding text transition from right
    ///
    /// # Arguments
    /// * `text` - The text to slide in
    /// * `start_col` - Starting column (typically terminal width)
    /// * `target_col` - Target column (usually 0)
    /// * `delay_ms` - Delay between slide steps
    pub fn slide_in_from_right(
        text: &str,
        start_col: u16,
        target_col: u16,
        delay_ms: u64,
    ) -> io::Result<()> {
        let mut stdout = io::stdout();

        for col in (target_col..=start_col).rev() {
            execute!(
                stdout,
                cursor::MoveTo(col, 0),
                Clear(ClearType::CurrentLine),
                Print(text)
            )?;

            stdout.flush()?;
            thread::sleep(Duration::from_millis(delay_ms));
        }

        Ok(())
    }

    /// Pulse effect - text grows and shrinks
    ///
    /// # Arguments
    /// * `text` - The text to pulse
    /// * `cycles` - Number of pulse cycles
    /// * `min_intensity` - Minimum intensity (0.0 to 1.0)
    /// * `max_intensity` - Maximum intensity (0.0 to 1.0)
    pub fn pulse_text(
        text: &str,
        cycles: u32,
        min_intensity: f32,
        max_intensity: f32,
    ) -> io::Result<()> {
        let mut stdout = io::stdout();

        for cycle in 0..cycles {
            let phase = (cycle as f32) / (cycles as f32) * std::f32::consts::PI * 2.0;
            let intensity =
                min_intensity + (max_intensity - min_intensity) * ((phase).sin() * 0.5 + 0.5);

            let color = Color::Rgb {
                r: (intensity * 205.0) as u8, // Light gray base
                g: (intensity * 209.0) as u8,
                b: (intensity * 196.0) as u8,
            };

            // Use \r for better terminal compatibility
            execute!(stdout, SetForegroundColor(color))?;
            print!("\r\x1b[2K{}", text);
            stdout.flush()?;
            thread::sleep(Duration::from_millis(80));
        }

        execute!(stdout, ResetColor)?;
        print!("\r\x1b[2K{}", text);
        stdout.flush()?;
        Ok(())
    }
}

// NOTE: hsv_to_rgb is now in colors.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsv_to_rgb() {
        use super::super::colors::hsv_to_rgb;

        // Test pure red
        let (r, g, b) = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        // Test pure green
        let (r, g, b) = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        // Test pure blue
        let (r, g, b) = hsv_to_rgb(240.0, 1.0, 1.0);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 255);
    }

    #[test]
    fn test_glowing_text_creation() {
        // This test mainly ensures the function can be called
        // Visual testing would be needed for actual animation verification
        let result = TerminalEffects::glowing_text("Test", 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_typewriter_sync() {
        let result = TerminalEffects::typewriter("Test", Duration::from_millis(1));
        assert!(result.is_ok());
    }
}
