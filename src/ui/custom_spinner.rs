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
    terminal::{Clear, ClearType},
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
    "⢎⡰",  // 0: Dots at position 1
    "⢎⡡",  // 1: Dots rotating
    "⢎⡑",  // 2: Dots rotating
    "⢎⠱",  // 3: Dots rotating
    "⠎⡱",  // 4: Dots at position 2
    "⢊⡱",  // 5: Dots rotating
    "⢌⡱",  // 6: Dots rotating
    "⢆⡱",  // 7: Dots rotating
    "⢎⡰",  // 8: Dots at position 3
    "⢎⡔",  // 9: Dots rotating
    "⢎⡒",  // 10: Dots rotating
    "⢎⡂",  // 11: Dots rotating (cycle complete)
];

/// Commands for controlling the spinner
enum Cmd {
    SetMessage(String),
    StopOk(String),
    StopErr(String),
    Shutdown,
}

/// Shared state for the spinner
struct SpinnerState {
    running: bool,
}

/// Random direction: +1 or -1
fn random_dir() -> i32 {
    if fastrand::bool() { 1 } else { -1 }
}

/// ARULA Single-Character Star Pulse Spinner
pub struct CustomSpinner {
    tx: Sender<Cmd>,
    handle: Option<thread::JoinHandle<()>>,
    state: Arc<Mutex<SpinnerState>>,
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
                Cmd::StopOk(final_msg) => {
                    draw_final(&label, &final_msg, false)?;
                    finished = true;
                    break;
                }
                Cmd::StopErr(final_msg) => {
                    draw_final(&label, &final_msg, true)?;
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
            index = (index + random_dir()).rem_euclid(STAR_FRAMES.len() as i32);

            let star = STAR_FRAMES[index as usize];
            draw_star(star, &label, frame_count)?;

            last_draw = Instant::now();
            frame_count += 1;
        }

        thread::sleep(Duration::from_millis(8));
    }

    // Clean up: clear the spinner line completely
    let mut stdout = io::stdout();
    execute!(
        stdout,
        cursor::MoveToColumn(0),
        Clear(ClearType::CurrentLine),
        ResetColor,
        cursor::Show
    )?;
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
    let text_color = Color::Rgb { r: 205, g: 209, b: 196 };

    execute!(
        stdout,
        cursor::SavePosition,
        cursor::MoveToColumn(0),
        Clear(ClearType::CurrentLine),
    )?;

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

/// Draw final status message
fn draw_final(label: &str, final_msg: &str, is_err: bool) -> io::Result<()> {
    let mut stdout = io::stdout();

    let (status_symbol, status_color) = if is_err {
        ("✖", Color::Rgb { r: 231, g: 76, b: 60 })
    } else {
        ("✔", Color::Rgb { r: 46, g: 204, b: 113 })
    };

    let text_color = Color::Rgb { r: 205, g: 209, b: 196 };

    // Clear line and draw final message without saving position
    execute!(
        stdout,
        cursor::MoveToColumn(0),
        Clear(ClearType::CurrentLine),
    )?;

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
            assert!(frame.chars().count() == 2, "Frame '{}' should be 2 braille chars", frame);
        }
    }
}
