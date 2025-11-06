use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub struct ProgressHelper {
    spinner: Option<ProgressBar>,
}

impl ProgressHelper {
    pub fn new() -> Self {
        Self {
            spinner: None,
        }
    }

    pub async fn with_progress<F, T>(&mut self, message: &str, operation: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        // Create and configure spinner
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        spinner.set_message(message.to_string());
        spinner.enable_steady_tick(Duration::from_millis(80));

        self.spinner = Some(spinner.clone());

        // Execute the operation
        let result = operation();

        // Finish the spinner based on result
        match &result {
            Ok(_) => {
                spinner.finish_with_message(format!("✅ {}", message));
            }
            Err(e) => {
                spinner.finish_with_message(format!("❌ {} - Error: {}", message, e));
            }
        }

        self.spinner = None;
        result
    }

    pub fn finish(&mut self) {
        if let Some(spinner) = self.spinner.take() {
            spinner.finish_and_clear();
        }
    }
}

impl Default for ProgressHelper {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ProgressHelper {
    fn drop(&mut self) {
        self.finish();
    }
}
