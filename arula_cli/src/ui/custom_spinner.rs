//! ARULA Dots Orbit Spinner
//!
//! Features:
//! - Single character: Orbital dots animation
//! - Braille patterns creating atom-like orbit effect
//! - ARULA golden color with brightness pulsing
//! - Random direction for organic movement
//! - Smooth orbital rotation - very unique!

use crossterm::{
    cursor, execute,
    style::{Color, ResetColor, SetForegroundColor},
};
use fastrand;
use std::io::{self, Write};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

/// Dots Orbit frames - atom-like orbital animation
/// Using braille patterns to create rotating dots around center
const STAR_FRAMES: [&str; 12] = [
    "⢎⡰", // 0: Dots at position 1
    "⢎⡡", // 1: Dots rotating
    "⢎⡑", // 2: Dots rotating
    "⢎⠱", // 3: Dots rotating
    "⠎⡱", // 4: Dots at position 2
    "⢊⡱", // 5: Dots rotating
    "⢌⡱", // 6: Dots rotating
    "⢆⡱", // 7: Dots rotating
    "⢎⡰", // 8: Dots at position 3
    "⢎⡔", // 9: Dots rotating
    "⢎⡒", // 10: Dots rotating
    "⢎⡂", // 11: Dots rotating (cycle complete)
];

/// Transition effects for animations
#[derive(Clone)]
pub enum Transition {
    FadeOut,
    SlideUp,
    Pulse,
    Rainbow,
}

/// Commands for controlling the spinner
enum Cmd {
    SetMessage(String),
    StopOk(String),
    StopErr(String),
    TransitionTo {
        frames: Vec<String>,
        transition: Transition,
    },
    Shutdown,
}

/// Shared state for the spinner
struct SpinnerState {
    running: bool,
}

/// Random direction: +1 or -1
fn random_dir() -> i32 {
    if fastrand::bool() {
        1
    } else {
        -1
    }
}

/// Additional animation frame sets for transitions
const ARC_FRAMES: [&str; 8] = ["◜", "◠", "◝", "◞", "◡", "◟", "◜", "◠"];

const DOTS_ORBIT: [&str; 20] = [
    "⢀⠠", "⡀⢀", "⠄⡀", "⢄⠄", "⡄⢄", "⠌⡄", "⢌⠌", "⡌⢌", "⠎⡌", "⢎⠎", "⡎⢎", "⠱⡎", "⢱⠱", "⡱⢱", "⠹⡱", "⢹⠹",
    "⠼⢹", "⢼⠼", "⡼⢼", "⠧⡼",
];

/// ARULA Single-Character Star Pulse Spinner
pub struct CustomSpinner {
    tx: Sender<Cmd>,
    handle: Option<thread::JoinHandle<()>>,
    state: Arc<Mutex<SpinnerState>>,
}

impl Default for CustomSpinner {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomSpinner {
    /// Create a new spinner (not started)
    pub fn new() -> Self {
        Self {
            tx: mpsc::channel().0,
            handle: None,
            state: Arc::new(Mutex::new(SpinnerState { running: false })),
        }
    }

    /// Start the spinner with a message
    pub fn start(&mut self, message: &str) -> io::Result<()> {
        self.start_with_speed(message, 100) // 100ms for smooth orbital motion
    }

    /// Start the spinner above the current line (for persistent input)
    pub fn start_above(&mut self, message: &str) -> io::Result<()> {
        self.start(message)
    }

    /// Start the spinner with custom animation speed
    fn start_with_speed(&mut self, label: &str, speed_ms: u64) -> io::Result<()> {
        self.stop();

        let (tx, rx): (Sender<Cmd>, Receiver<Cmd>) = mpsc::channel();
        self.tx = tx;

        let state = Arc::new(Mutex::new(SpinnerState { running: true }));
        self.state = Arc::clone(&state);
        let state_clone = Arc::clone(&state);

        let label = label.to_string();

        let handle = thread::Builder::new()
            .name("arula-star-spinner".into())
            .spawn(move || {
                if let Err(e) = run_star_spinner(label, speed_ms, rx, state_clone) {
                    let _ = writeln!(io::stderr(), "spinner thread error: {:?}", e);
                }
            })?;

        self.handle = Some(handle);
        Ok(())
    }

    /// Update the spinner message while running
    pub fn set_message(&self, msg: &str) {
        let _ = self.tx.send(Cmd::SetMessage(msg.to_string()));
    }

    /// Transition to a different animation style with smooth effect
    pub fn transition_to(&self, new_frames: Vec<String>, transition: Transition) {
        let _ = self.tx.send(Cmd::TransitionTo {
            frames: new_frames,
            transition,
        });
    }

    /// Transition to arc animation
    pub fn transition_to_arc(&self) {
        let arc_frames: Vec<String> = ARC_FRAMES.iter().map(|&s| s.to_string()).collect();
        self.transition_to(arc_frames, Transition::FadeOut);
    }

    /// Transition to dots orbit animation
    pub fn transition_to_dots_orbit(&self) {
        let dots_frames: Vec<String> = DOTS_ORBIT.iter().map(|&s| s.to_string()).collect();
        self.transition_to(dots_frames, Transition::SlideUp);
    }

    /// Stop the spinner
    pub fn stop(&mut self) {
        if let Ok(mut s) = self.state.lock() {
            if !s.running {
                return;
            }
            s.running = false;
        }

        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    /// Finish with success message
    pub fn finish_ok(&mut self, final_message: &str) {
        let _ = self.tx.send(Cmd::StopOk(final_message.to_string()));
        self.shutdown_and_join();
    }

    /// Finish with error message
    pub fn finish_err(&mut self, final_message: &str) {
        let _ = self.tx.send(Cmd::StopErr(final_message.to_string()));
        self.shutdown_and_join();
    }

    /// Check if spinner is running
    pub fn is_running(&self) -> bool {
        self.state.lock().map(|s| s.running).unwrap_or(false)
    }

    /// Internal shutdown helper
    fn shutdown_and_join(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        if let Ok(mut s) = self.state.lock() {
            s.running = false;
        }
    }
}

impl Drop for CustomSpinner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Internal star pulse spinner loop
fn run_star_spinner(
    mut label: String,
    speed_ms: u64,
    rx: Receiver<Cmd>,
    _state: Arc<Mutex<SpinnerState>>,
) -> io::Result<()> {
    let mut index: i32 = 0;
    let mut stdout = io::stdout();

    // Start with star frames
    let mut current_frames: Vec<String> = STAR_FRAMES.iter().map(|&s| s.to_string()).collect();
    let mut transition_in_progress = false;
    let mut transition_type: Option<Transition> = None;
    let mut transition_frame_count = 0;

    execute!(stdout, cursor::SavePosition, cursor::Hide)?;

    let frame_duration = Duration::from_millis(speed_ms.max(10));
    let mut last_draw = Instant::now();
    let mut frame_count = 0;

    let mut finished = false;

    while !finished {
        // Process commands
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                Cmd::SetMessage(m) => label = m,
                Cmd::TransitionTo { frames, transition } => {
                    current_frames = frames;
                    transition_in_progress = true;
                    transition_type = Some(transition);
                    transition_frame_count = 0;
                }
                Cmd::StopOk(final_msg) => {
                    draw_final_with_transition(&label, &final_msg, false, transition_in_progress)?;
                    finished = true;
                    break;
                }
                Cmd::StopErr(final_msg) => {
                    draw_final_with_transition(&label, &final_msg, true, transition_in_progress)?;
                    finished = true;
                    break;
                }
                Cmd::Shutdown => {
                    finished = true;
                    break;
                }
            }
        }

        if finished {
            break;
        }

        if last_draw.elapsed() >= frame_duration {
            // Random direction for organic breathing
            index = (index + random_dir()).rem_euclid(current_frames.len() as i32);

            let frame = &current_frames[index as usize];

            if transition_in_progress {
                if let Some(ref transition) = transition_type {
                    draw_star_with_transition(
                        frame,
                        &label,
                        frame_count,
                        transition.clone(),
                        transition_frame_count,
                    )?;
                }
                transition_frame_count += 1;

                // End transition after some frames
                if transition_frame_count > 10 {
                    transition_in_progress = false;
                    transition_type = None;
                }
            } else {
                draw_star(frame, &label, frame_count)?;
            }

            last_draw = Instant::now();
            frame_count += 1;
        }

        thread::sleep(Duration::from_millis(8));
    }

    // Clean up: clear the spinner line completely
    let mut stdout = io::stdout();
    // Use \r for better terminal compatibility
    print!("\r\x1b[2K");
    execute!(stdout, ResetColor, cursor::Show)?;
    stdout.flush()?;
    Ok(())
}

/// Draw the single-character star with ARULA colors and brightness pulsing
fn draw_star(star: &str, label: &str, frame_count: u64) -> io::Result<()> {
    let mut stdout = io::stdout();

    // ARULA golden color
    let golden_r = 232u8;
    let golden_g = 197u8;
    let golden_b = 71u8;

    // Subtle brightness pulsing with sine wave
    let pulse_phase = (frame_count % 30) as f32 / 30.0;
    let brightness = 0.75 + (pulse_phase * std::f32::consts::PI * 2.0).sin() * 0.25;

    let pulsed_golden = Color::Rgb {
        r: ((golden_r as f32) * brightness) as u8,
        g: ((golden_g as f32) * brightness) as u8,
        b: ((golden_b as f32) * brightness) as u8,
    };

    // Light gray for text
    let text_color = Color::Rgb {
        r: 205,
        g: 209,
        b: 196,
    };

    // Use \r for better terminal compatibility
    execute!(stdout, cursor::SavePosition)?;
    print!("\r\x1b[2K");

    // Draw: [star] label
    execute!(stdout, SetForegroundColor(pulsed_golden))?;
    print!("{}", star);

    if !label.is_empty() {
        execute!(stdout, SetForegroundColor(text_color))?;
        print!(" {}", label);
    }

    execute!(stdout, ResetColor, cursor::RestorePosition)?;
    stdout.flush()?;
    Ok(())
}

/// Draw the single-character star with transition effects
fn draw_star_with_transition(
    star: &str,
    label: &str,
    _frame_count: u64,
    transition: Transition,
    transition_frame: u32,
) -> io::Result<()> {
    let mut stdout = io::stdout();

    // ARULA golden color base
    let golden_r = 232u8;
    let golden_g = 197u8;
    let golden_b = 71u8;

    let (r, g, b) = match transition {
        Transition::FadeOut => {
            let fade_factor = (10 - transition_frame.min(10)) as f32 / 10.0;
            let new_r = (golden_r as f32 * fade_factor) as u8;
            let new_g = (golden_g as f32 * fade_factor) as u8;
            let new_b = (golden_b as f32 * fade_factor) as u8;
            (new_r, new_g, new_b)
        }
        Transition::Pulse => {
            let pulse_phase = (transition_frame as f32 / 10.0) * std::f32::consts::PI;
            let pulse_factor = pulse_phase.sin() * 0.5 + 0.5;
            let new_r = (golden_r as f32 * pulse_factor) as u8;
            let new_g = (golden_g as f32 * pulse_factor) as u8;
            let new_b = (golden_b as f32 * pulse_factor) as u8;
            (new_r, new_g, new_b)
        }
        Transition::Rainbow => {
            let hue = (transition_frame as f32 / 10.0) * 360.0;
            hsv_to_rgb(hue, 1.0, 1.0)
        }
        Transition::SlideUp => {
            let slide_factor = (transition_frame.min(10) as f32) / 10.0;
            let new_r = ((255 - golden_r) as f32 * slide_factor + golden_r as f32) as u8;
            let new_g = ((255 - golden_g) as f32 * slide_factor + golden_g as f32) as u8;
            let new_b = ((255 - golden_b) as f32 * slide_factor + golden_b as f32) as u8;
            (new_r, new_g, new_b)
        }
    };

    let pulsed_golden = Color::Rgb { r, g, b };
    let text_color = Color::Rgb {
        r: 205,
        g: 209,
        b: 196,
    };

    // Use \r for better terminal compatibility
    execute!(stdout, cursor::SavePosition)?;
    print!("\r\x1b[2K");

    // Draw: [star] label with transition effect
    execute!(stdout, SetForegroundColor(pulsed_golden))?;
    print!("{}", star);

    if !label.is_empty() {
        execute!(stdout, SetForegroundColor(text_color))?;
        print!(" {}", label);
    }

    execute!(stdout, ResetColor, cursor::RestorePosition)?;
    stdout.flush()?;
    Ok(())
}

/// Convert HSV to RGB for rainbow effects
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
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

/// Draw final status message with optional transition effect
fn draw_final_with_transition(
    label: &str,
    final_msg: &str,
    is_err: bool,
    _has_transition: bool,
) -> io::Result<()> {
    // For now, just use the regular final drawing
    // Could enhance this with transition animations if desired
    draw_final(label, final_msg, is_err)
}

/// Draw final status message
fn draw_final(label: &str, final_msg: &str, is_err: bool) -> io::Result<()> {
    let mut stdout = io::stdout();

    let (status_symbol, status_color) = if is_err {
        (
            "✖",
            Color::Rgb {
                r: 231,
                g: 76,
                b: 60,
            },
        )
    } else {
        (
            "✔",
            Color::Rgb {
                r: 46,
                g: 204,
                b: 113,
            },
        )
    };

    let text_color = Color::Rgb {
        r: 205,
        g: 209,
        b: 196,
    };

    // Clear line and draw final message - use \r for better terminal compatibility
    print!("\r\x1b[2K");

    execute!(stdout, SetForegroundColor(status_color))?;
    print!("{} ", status_symbol);

    execute!(stdout, SetForegroundColor(text_color))?;
    if !final_msg.is_empty() {
        print!("{}", final_msg);
    } else {
        print!("{}", label);
    }

    execute!(stdout, ResetColor)?;
    stdout.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_count() {
        assert_eq!(STAR_FRAMES.len(), 12);
    }

    #[test]
    fn test_random_dir() {
        for _ in 0..100 {
            let dir = random_dir();
            assert!(dir == 1 || dir == -1);
        }
    }

    #[test]
    fn test_spinner_creation() {
        let spinner = CustomSpinner::new();
        assert!(!spinner.is_running());
    }

    #[test]
    fn test_index_wrapping() {
        let len = STAR_FRAMES.len() as i32;
        assert_eq!((0i32 + 1i32).rem_euclid(len), 1);
        assert_eq!((11i32 + 1i32).rem_euclid(len), 0);
        assert_eq!((0i32 - 1i32).rem_euclid(len), 11);
    }

    #[test]
    fn test_all_frames_braille() {
        // Ensure all frames are valid braille patterns (2 chars combined into 1 visual)
        for frame in STAR_FRAMES.iter() {
            assert!(
                frame.chars().count() == 2,
                "Frame '{}' should be 2 braille chars",
                frame
            );
        }
    }
}
