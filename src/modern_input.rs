/// Modern input handler using rustyline for robust terminal input
use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::{
    completion::{Completer, FilenameCompleter},
    Editor,
    Helper,
    Context,
    hint::Hinter,
    highlight::{Highlighter},
    validate::{Validator, ValidationResult, ValidationContext},
    Result as RustylineResult,
    Config,
    history::FileHistory,
};
use std::borrow::Cow::{self, Borrowed, Owned};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Simple helper for rustyline with basic features
struct ArulaHelper {
    filename_completer: FilenameCompleter,
    commands: Vec<String>,
}

impl ArulaHelper {
    fn new() -> Self {
        let commands = vec![
            "help".to_string(),
            "menu".to_string(),
            "exit".to_string(),
            "quit".to_string(),
            "clear".to_string(),
            "history".to_string(),
            "/help".to_string(),
            "/menu".to_string(),
            "/config".to_string(),
            "/save".to_string(),
            "/load".to_string(),
            "/clear".to_string(),
            "/history".to_string(),
        ];

        Self {
            filename_completer: FilenameCompleter::new(),
            commands,
        }
    }
}

impl Completer for ArulaHelper {
    type Candidate = String;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> RustylineResult<(usize, Vec<String>)> {
        let mut candidates = Vec::new();

        // Command completion for lines starting with /
        if line.starts_with('/') || pos == 0 {
            let start = if line.starts_with('/') { 1 } else { 0 };
            let input = &line[start..pos];

            for cmd in &self.commands {
                if cmd.starts_with(input) {
                    candidates.push(cmd.clone());
                }
            }
        }

        Ok((0, candidates))
    }
}

impl Hinter for ArulaHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        // Only show hints when cursor is at end of line to avoid interfering with Enter
        if pos != line.len() {
            return None;
        }

        // Provide hints for common commands
        if line.is_empty() {
            return Some(" Type '/' for commands or start typing your message".to_string());
        }

        if line.starts_with('/') {
            let input = line.trim_start_matches('/');
            for cmd in &self.commands {
                if cmd.starts_with(input) && cmd.len() > input.len() {
                    let hint = &cmd[input.len()..];
                    return Some(hint.to_string());
                }
            }
        }

        None
    }
}

impl Highlighter for ArulaHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // Basic syntax highlighting for commands
        if line.starts_with('/') {
            // Highlight commands in cyan
            Owned(format!("\x1b[36m{}\x1b[0m", line))
        } else {
            // Regular text stays as-is
            Borrowed(line)
        }
    }

    fn highlight_char<'l>(&self, line: &'l str, _pos: usize, _forced: bool) -> bool {
        // Let the default highlighter handle character highlighting
        false
    }
}

impl Validator for ArulaHelper {
    fn validate(&self, ctx: &mut ValidationContext<'_>) -> RustylineResult<ValidationResult> {
        // Always accept input immediately - never wait for more lines
        Ok(ValidationResult::Valid(None))
    }
}

impl Helper for ArulaHelper {}

/// Async input handler that works with rustyline while maintaining real-time capabilities
pub struct ModernInputHandler {
    history: Arc<RwLock<VecDeque<String>>>,
    input_tx: Option<mpsc::UnboundedSender<String>>,
    prompt: String,
    max_history: usize,
    editor_config: Config,
}

impl ModernInputHandler {
    pub fn new(prompt: &str) -> Self {
        // Use default config for now - rustyline v14 API has changed
        let config = Config::default();

        Self {
            history: Arc::new(RwLock::new(VecDeque::new())),
            input_tx: None,
            prompt: prompt.to_string(),
            max_history: 1000,
            editor_config: config,
        }
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
    }

    /// Set input channel for async communication
    pub fn set_input_channel(&mut self, tx: mpsc::UnboundedSender<String>) {
        self.input_tx = Some(tx);
    }

    /// Get input with async support using enhanced rustyline
    pub async fn get_input(&self) -> Result<Option<String>> {
        let prompt = format!("⚡{} ", self.prompt);
        let history = self.history.clone();
        let config = self.editor_config;

        // Run blocking readline in a separate thread
        let result = tokio::task::spawn_blocking(move || {
            // Create enhanced editor with all features
            let mut editor: rustyline::Editor<ArulaHelper, FileHistory> = match rustyline::Editor::new() {
                Ok(editor) => editor,
                Err(e) => {
                    eprintln!("Failed to create enhanced editor: {}", e);
                    return None;
                }
            };

            // Set helper with all features
            editor.set_helper(Some(ArulaHelper::new()));

            // Load history into editor
            let history_lines = history.blocking_read();
            for line in history_lines.iter() {
                let _ = editor.add_history_entry(line.clone());
            }

            let readline = editor.readline(&prompt);

            match readline {
                Ok(line) => {
                    if !line.trim().is_empty() {
                        // Add to in-memory history
                        let mut history = history.blocking_write();
                        history.push_back(line.clone());

                        // Limit history size
                        if history.len() > 1000 {
                            history.pop_front();
                        }
                    }
                    Some(line)
                }
                Err(ReadlineError::Interrupted) => {
                    Some("__CTRL_C__".to_string())
                }
                Err(ReadlineError::Eof) => {
                    Some("__CTRL_D__".to_string())
                }
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    None
                }
            }
        }).await.expect("Task join error");

        Ok(result)
    }

    /// Process input command (for non-async usage)
    pub fn process_input(&self, input: &str) -> Result<Option<String>> {
        if input.trim().is_empty() {
            return Ok(None);
        }

        // Handle special commands
        match input {
            "exit" | "quit" => Ok(Some("__EXIT__".to_string())),
            "/help" => Ok(Some("__HELP__".to_string())),
            "/menu" => Ok(Some("__MENU__".to_string())),
            _ => Ok(Some(input.to_string())),
        }
    }

    /// Add to history (for external calls)
    pub async fn add_to_history(&mut self, entry: String) {
        if entry.trim().is_empty() {
            return;
        }

        let mut history = self.history.write().await;
        if history.back() != Some(&entry) {
            history.push_back(entry.clone());
            if history.len() > self.max_history {
                history.pop_front();
            }
        }
    }

    /// Load history from file
    pub async fn load_history(&mut self, lines: Vec<String>) {
        let mut history = self.history.write().await;
        for line in lines {
            if !line.trim().is_empty() {
                history.push_back(line);
            }
        }
        if history.len() > self.max_history {
            let excess = history.len() - self.max_history;
            for _ in 0..excess {
                history.pop_front();
            }
        }
    }

    /// Get current history
    pub async fn get_history(&self) -> Vec<String> {
        self.history.read().await.iter().cloned().collect()
    }

    /// Save history to file
    pub async fn save_history(&self) -> Result<()> {
        let history_path = Self::get_history_path();
        let history_lines = self.get_history().await;
        let _ = std::fs::write(&history_path, history_lines.join("\n"));
        Ok(())
    }

    /// Get history file path
    fn get_history_path() -> PathBuf {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".arula_history");
        path
    }

    /// Clear current input buffer
    pub fn clear(&self) -> Result<()> {
        // This would require additional implementation for clearing
        // For now, just return success
        Ok(())
    }

    /// For compatibility with existing interface - not used in rustyline mode
    pub fn draw(&self) -> std::io::Result<()> {
        // Not applicable in rustyline mode
        Ok(())
    }

    /// Handle key events - not used in rustyline mode
    pub fn handle_key(&mut self, _key: crossterm::event::KeyEvent) -> std::io::Result<Option<String>> {
        // Not applicable in rustyline mode
        Ok(None)
    }
}

/// Helper functions for using enhanced rustyline in dialogs
pub mod dialogs {
    use super::*;

    /// Get input with validation using enhanced rustyline
    pub fn get_validated_input(prompt: &str) -> Result<String> {
        let mut editor: rustyline::Editor<ArulaHelper, FileHistory> = Editor::new()?;
        editor.set_helper(Some(ArulaHelper::new()));
        let line = editor.readline(prompt)?;
        Ok(line)
    }

    /// Get input with default value
    pub fn get_input_with_default(prompt: &str, default: &str) -> Result<String> {
        let mut editor: rustyline::Editor<ArulaHelper, FileHistory> = Editor::new()?;
        editor.set_helper(Some(ArulaHelper::new()));
        let line = if default.is_empty() {
            editor.readline(prompt)?
        } else {
            editor.readline_with_initial(prompt, (default, ""))?
        };
        Ok(line)
    }

    /// Get multiline input with enhanced features
    pub fn get_multiline_input(prompt: &str) -> Result<Vec<String>> {
        let mut editor: rustyline::Editor<ArulaHelper, FileHistory> = Editor::new()?;
        editor.set_helper(Some(ArulaHelper::new()));
        let mut lines = Vec::new();

        loop {
            let current_prompt = if lines.is_empty() {
                prompt
            } else if lines.len() == 1 {
                "┗▶ "
            } else {
                "  > "
            };

            let line = editor.readline(current_prompt)?;

            // Support multiple exit commands
            match line.trim() {
                "exit" | "done" | "." => break,
                _ if line.trim().is_empty() && lines.len() > 0 => break,
                _ => lines.push(line),
            }
        }

        Ok(lines)
    }

    /// Get password input (no echo)
    pub fn get_password(prompt: &str) -> Result<String> {
        // Simple password editor without helper features
        let mut editor: rustyline::Editor<(), FileHistory> = Editor::new()?;
        let line = editor.readline(prompt)?;
        Ok(line)
    }

    /// Get input with autocomplete for specific options
    pub fn get_autocomplete_input(prompt: &str, options: Vec<String>) -> Result<String> {
        struct AutoCompleteHelper {
            options: Vec<String>,
        }

        impl AutoCompleteHelper {
            fn new(options: Vec<String>) -> Self {
                Self { options }
            }
        }

        impl Completer for AutoCompleteHelper {
            type Candidate = String;

            fn complete(&self, line: &str, _pos: usize, _ctx: &Context<'_>) -> RustylineResult<(usize, Vec<String>)> {
                let mut candidates = Vec::new();
                for option in &self.options {
                    if option.starts_with(line) {
                        candidates.push(option.clone());
                    }
                }
                Ok((0, candidates))
            }
        }

        impl Hinter for AutoCompleteHelper {
            type Hint = String;

            fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
                // Only show hints when cursor is at end of line
                if pos != line.len() {
                    return None;
                }

                for option in &self.options {
                    if option.starts_with(line) && option.len() > line.len() {
                        return Some(option[line.len()..].to_string());
                    }
                }
                None
            }
        }

        impl Highlighter for AutoCompleteHelper {
            fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
                Borrowed(line)
            }

            fn highlight_char<'l>(&self, _line: &'l str, _pos: usize, _forced: bool) -> bool {
                false
            }
        }

        impl Validator for AutoCompleteHelper {
            fn validate(&self, ctx: &mut ValidationContext<'_>) -> RustylineResult<ValidationResult> {
                // Always accept input immediately
                Ok(ValidationResult::Valid(None))
            }
        }

        impl Helper for AutoCompleteHelper {}

        let mut editor: rustyline::Editor<AutoCompleteHelper, FileHistory> = Editor::new()?;
        editor.set_helper(Some(AutoCompleteHelper::new(options)));
        let line = editor.readline(prompt)?;
        Ok(line)
    }
}

impl Drop for ModernInputHandler {
    fn drop(&mut self) {
        // Note: We can't use async in Drop, so we'll save synchronously if possible
        let history_path = Self::get_history_path();
        if let Ok(history) = self.history.try_read() {
            let history_lines: Vec<String> = history.iter().cloned().collect();
            let _ = std::fs::write(&history_path, history_lines.join("\n"));
        }
    }
}