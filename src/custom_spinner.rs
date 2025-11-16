use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Custom spinner that renders above the input line
pub struct CustomSpinner {
    handle: Option<thread::JoinHandle<()>>,
    running: Arc<Mutex<bool>>,
    message: Arc<Mutex<String>>,
}

impl CustomSpinner {
    pub fn new() -> Self {
        Self {
            handle: None,
            running: Arc::new(Mutex::new(false)),
            message: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Start the spinner with a message
    pub fn start(&mut self, message: &str) -> io::Result<()> {
        // Stop any existing spinner
        self.stop();

        *self.message.lock().unwrap() = message.to_string();
        *self.running.lock().unwrap() = true;

        let running = Arc::clone(&self.running);
        let message = Arc::clone(&self.message);

        let handle = thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut frame_idx = 0;

            while *running.lock().unwrap() {
                let msg = message.lock().unwrap().clone();
                let frame = frames[frame_idx];

                // Clear current line and print spinner
                let _ = execute!(
                    io::stdout(),
                    cursor::MoveToColumn(0),
                    terminal::Clear(ClearType::CurrentLine)
                );

                // Print spinner (stays on current line)
                print!("\x1b[36m{}\x1b[0m {}", frame, msg);
                let _ = io::stdout().flush();

                frame_idx = (frame_idx + 1) % frames.len();
                thread::sleep(Duration::from_millis(80));
            }

            // Clear spinner line when stopped
            let _ = execute!(
                io::stdout(),
                cursor::MoveToColumn(0),
                terminal::Clear(ClearType::CurrentLine)
            );
            let _ = io::stdout().flush();
        });

        self.handle = Some(handle);
        Ok(())
    }

    /// Stop the spinner
    pub fn stop(&mut self) {
        *self.running.lock().unwrap() = false;

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if spinner is running
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    /// Update the spinner message
    pub fn set_message(&self, message: &str) {
        *self.message.lock().unwrap() = message.to_string();
    }
}

impl Drop for CustomSpinner {
    fn drop(&mut self) {
        self.stop();
    }
}
