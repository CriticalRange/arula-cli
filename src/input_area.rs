use anyhow::Result;
use rustyline::{
    completion::{Completer, FilenameCompleter},
    error::ReadlineError,
    highlight::Highlighter,
    hint::Hinter,
    history::FileHistory,
    validate::{ValidationContext, ValidationResult, Validator},
    Config, Context, Editor, ExternalPrinter, Helper,
};
use std::borrow::Cow::{self, Borrowed, Owned};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::mpsc;

/// Helper for rustyline with basic features
#[derive(Clone)]
pub struct ArulaHelper {
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

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
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
        
        // Also try filename completion
        let (file_start, file_candidates) = self.filename_completer.complete(line, pos, ctx)?;
        if !file_candidates.is_empty() {
             let candidates = file_candidates.into_iter().map(|pair| pair.replacement).collect();
             return Ok((file_start, candidates));
        }

        Ok((0, candidates))
    }
}

impl Hinter for ArulaHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        // Only show hints when cursor is at end of line
        if pos != line.len() {
            return None;
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
        if line.starts_with('/') {
            Owned(format!("\x1b[36m{}\x1b[0m", line))
        } else {
            Borrowed(line)
        }
    }

    fn highlight_char<'l>(&self, _line: &'l str, _pos: usize, _forced: bool) -> bool {
        false
    }
}

impl Validator for ArulaHelper {
    fn validate(&self, _ctx: &mut ValidationContext<'_>) -> rustyline::Result<ValidationResult> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Helper for ArulaHelper {}

pub struct InputArea {
    input_rx: mpsc::UnboundedReceiver<String>,
    printer: ExternalPrinter<ArulaHelper>,
    history_path: PathBuf,
    // We keep a handle to the history to save it later, though rustyline handles it internally mostly
    // But since the editor is moved to another thread, we might need a way to trigger save.
    // Actually, we can just save on every command in the background thread.
}

impl InputArea {
    pub fn new(prompt: &str) -> Result<Self> {
        let config = Config::builder()
            .auto_add_history(true)
            .build();

        let mut editor = Editor::with_config(config)?;
        editor.set_helper(Some(ArulaHelper::new()));

        // Load history
        let history_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".arula_history");
        let _ = editor.load_history(&history_path);

        let printer = editor.create_external_printer()?;
        let (tx, rx) = mpsc::unbounded_channel();
        let prompt = prompt.to_string();
        let history_path_clone = history_path.clone();

        // Spawn background thread for input loop
        thread::spawn(move || {
            loop {
                let readline = editor.readline(&format!("⚡{} ", prompt));
                match readline {
                    Ok(line) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let _ = editor.save_history(&history_path_clone);
                            if tx.send(line).is_err() {
                                break;
                            }
                        }
                    }
                    Err(ReadlineError::Interrupted) => {
                        if tx.send("__CTRL_C__".to_string()).is_err() {
                            break;
                        }
                    }
                    Err(ReadlineError::Eof) => {
                        if tx.send("__CTRL_D__".to_string()).is_err() {
                            break;
                        }
                        break;
                    }
                    Err(err) => {
                        eprintln!("Error: {:?}", err);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            input_rx: rx,
            printer,
            history_path,
        })
    }

    pub async fn next_input(&mut self) -> Option<String> {
        self.input_rx.recv().await
    }

    pub fn get_printer(&self) -> ExternalPrinter<ArulaHelper> {
        self.printer.clone()
    }
}
