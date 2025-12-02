use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Debug,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Clone)]
pub struct Logger {
    log_file_path: PathBuf,
    file_handle: Arc<Mutex<Option<std::fs::File>>>,
}

impl Logger {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let arula_dir = PathBuf::from(".arula");
        let logs_dir = arula_dir.join("logs");
        let log_file_path = logs_dir.join("latest.log");

        // Create directories if they don't exist
        fs::create_dir_all(&logs_dir)?;

        let file_handle = Arc::new(Mutex::new(None));

        let logger = Self {
            log_file_path: log_file_path.clone(),
            file_handle,
        };

        // Open the log file immediately
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)?;

        *logger.file_handle.lock().unwrap() = Some(file);

        Ok(logger)
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        let timestamp: DateTime<Utc> = Utc::now();
        let formatted_timestamp = timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC");

        let log_line = format!("[{}] [{}] {}\n", formatted_timestamp, level, message);

        if let Ok(mut file_guard) = self.file_handle.lock() {
            if let Some(ref mut file) = *file_guard {
                let _ = file.write_all(log_line.as_bytes());
                let _ = file.flush();
            }
        }
    }

    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    pub fn warn(&self, message: &str) {
        self.log(LogLevel::Warn, message);
    }

    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!("Failed to initialize logger: {}", e);
            // Create a dummy logger that doesn't write anywhere
            Self {
                log_file_path: PathBuf::from(".arula/logs/latest.log"),
                file_handle: Arc::new(Mutex::new(None)),
            }
        })
    }
}

// Global static logger instance using OnceLock for Rust 2024 compatibility
static GLOBAL_LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn init_global_logger() -> Result<(), Box<dyn std::error::Error>> {
    let logger = Logger::new()?;
    GLOBAL_LOGGER.set(logger).map_err(|_| "Logger already initialized")?;
    Ok(())
}

pub fn get_global_logger() -> Option<&'static Logger> {
    GLOBAL_LOGGER.get()
}

// Convenience functions for global logging
pub fn log(level: LogLevel, message: &str) {
    if let Some(logger) = get_global_logger() {
        logger.log(level, message);
    }
}

pub fn info(message: &str) {
    log(LogLevel::Info, message);
}

pub fn debug(message: &str) {
    log(LogLevel::Debug, message);
}

pub fn warn(message: &str) {
    log(LogLevel::Warn, message);
}

pub fn error(message: &str) {
    log(LogLevel::Error, message);
}
