//! Modern reedline-based input handler for ARULA CLI
//!
//! Features:
//! - Multi-line input with backslash continuation
//! - Emacs-style keybindings with full undo/redo
//! - Graphical columnar completion menu (Ctrl+Space)
//! - Inline history-based hints
//! - Context-aware syntax highlighting
//! - Dynamic prompt with AI status, token count, session ID
//! - Custom loading spinner integration
//! - Persistent history with immediate save
//! - Bracketed paste support with confirmations
//! - ESC-based menu system built into reedline

use anyhow::{Context, Result};
use crossterm::style::Stylize;
use nu_ansi_term::{Style as ReedlineStyle, Color as ReedlineColor};
use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultCompleter, DefaultHinter,
    EditCommand, Emacs, FileBackedHistory, KeyCode, KeyModifiers,
    Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus,
    Reedline, ReedlineEvent, ReedlineMenu, Signal, ValidationResult, Validator,
};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// AI processing state for dynamic prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiState {
    Ready,
    Thinking,
    Waiting,
}

/// Application state shared with the prompt
#[derive(Debug, Clone)]
pub struct AppState {
    pub ai_state: AiState,
    pub token_count: usize,
    pub token_limit: usize,
    pub session_id: String,
    pub esc_count: usize, // Track ESC presses for double-ESC menu trigger
    pub last_esc_time: std::time::Instant, // Track timing for ESC double-press
    pub last_signal_time: std::time::Instant, // Track timing for signal frequency
    pub ctrl_c_pending: bool, // Flag to indicate Ctrl+C was pressed (not ESC)
}

impl Default for AppState {
    fn default() -> Self {
        let now = std::time::Instant::now();
        Self {
            ai_state: AiState::Ready,
            token_count: 0,
            token_limit: 8192,
            session_id: "new".to_string(),
            esc_count: 0,
            last_esc_time: now,
            last_signal_time: now,
            ctrl_c_pending: false,
        }
    }
}

/// Custom ARULA prompt with dynamic status
pub struct ArulaPrompt {
    state: Arc<Mutex<AppState>>,
}

impl ArulaPrompt {
    pub fn new(state: Arc<Mutex<AppState>>) -> Self {
        Self { state }
    }

    fn get_ai_icon(&self, ai_state: AiState) -> &str {
        match ai_state {
            AiState::Ready => "⚡",
            AiState::Thinking => "🔧",
            AiState::Waiting => "⏳",
        }
    }

    fn get_token_display(&self, count: usize, limit: usize) -> String {
        let threshold = (limit as f64 * 0.9) as usize;
        if count >= limit {
            format!("[{}]", count).red().to_string()
        } else if count >= threshold {
            format!("[{}]", count).yellow().to_string()
        } else if count > 0 {
            format!("[{}]", count).dark_grey().to_string()
        } else {
            String::new()
        }
    }

    fn get_session_display(&self, session_id: &str) -> String {
        // Short format: s:a2f3
        if session_id.len() > 4 {
            format!("s:{}", &session_id[..4]).dark_grey().to_string()
        } else {
            format!("s:{}", session_id).dark_grey().to_string()
        }
    }
}

impl Prompt for ArulaPrompt {
    fn render_prompt_left(&self) -> Cow<str> {
        let state = self.state.lock().unwrap();

        let icon = self.get_ai_icon(state.ai_state);
        let token_display = self.get_token_display(state.token_count, state.token_limit);
        let session_display = self.get_session_display(&state.session_id);

        // Format: ⚡[234] s:a2f3 >
        let mut parts = vec![icon.to_string()];

        if !token_display.is_empty() {
            parts.push(token_display);
        }

        if !session_display.is_empty() {
            parts.push(session_display);
        }

        parts.push(">".to_string());

        Cow::Owned(parts.join(" "))
    }

    fn render_prompt_right(&self) -> Cow<str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<str> {
        Cow::Borrowed(" ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<str> {
        Cow::Borrowed("│ ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };

        Cow::Owned(format!(
            "({}reverse-search: {}) ",
            prefix, history_search.term
        ))
    }
}

/// Multi-line validator - continues on trailing backslash
pub struct MultilineValidator;

impl Validator for MultilineValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if line.trim_end().ends_with('\\') {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

/// Smart completer that provides history-based suggestions
pub struct ArulaCompleter {
    default_completer: DefaultCompleter,
}

impl ArulaCompleter {
    pub fn new() -> Self {
        Self {
            default_completer: DefaultCompleter::default(),
        }
    }
}

impl reedline::Completer for ArulaCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<reedline::Suggestion> {
        // Use default completer for now
        // TODO: Add context-aware completion based on AI state
        self.default_completer.complete(line, pos)
    }
}

/// Custom hinter with smart thresholds (3 chars for commands, 8 for text)
pub struct ArulaHinter {
    default_hinter: DefaultHinter,
}

impl ArulaHinter {
    pub fn new() -> Self {
        Self {
            default_hinter: DefaultHinter::default().with_style(
                ReedlineStyle::new().dimmed(),
            ),
        }
    }

    fn should_show_hint(&self, line: &str) -> bool {
        let trimmed = line.trim();

        // Commands (starting with /) - show after 3 chars
        if trimmed.starts_with('/') {
            return trimmed.len() >= 3;
        }

        // Regular text - show after 8 chars
        trimmed.len() >= 8
    }
}

impl reedline::Hinter for ArulaHinter {
    fn handle(
        &mut self,
        line: &str,
        pos: usize,
        history: &dyn reedline::History,
        use_ansi_coloring: bool,
        cwd: &str,
    ) -> String {
        if !self.should_show_hint(line) {
            return String::new();
        }

        self.default_hinter.handle(line, pos, history, use_ansi_coloring, cwd)
    }

    fn complete_hint(&self) -> String {
        self.default_hinter.complete_hint()
    }

    fn next_hint_token(&self) -> String {
        self.default_hinter.next_hint_token()
    }
}

/// Syntax highlighter with context-aware coloring
pub struct ArulaSyntaxHighlighter;

impl reedline::Highlighter for ArulaSyntaxHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> reedline::StyledText {
        let mut styled = reedline::StyledText::new();

        // Simple highlighting for now - just color commands
        if line.trim_start().starts_with('/') {
            styled.push((
                ReedlineStyle::new().fg(ReedlineColor::Cyan),
                line.to_string(),
            ));
        } else {
            styled.push((
                ReedlineStyle::new(),
                line.to_string(),
            ));
        }

        styled
    }
}

/// Main reedline input handler
pub struct ReedlineInput {
    editor: Reedline,
    prompt: ArulaPrompt,
    state: Arc<Mutex<AppState>>,
    history_path: PathBuf,
}

impl ReedlineInput {
    pub fn new() -> Result<Self> {
        // Set up history
        let history_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".arula_history");

        let history = Box::new(
            FileBackedHistory::with_file(1000, history_path.clone())
                .context("Failed to create history")?,
        );

        // Create app state
        let state = Arc::new(Mutex::new(AppState::default()));

        // Create custom keybindings based on Emacs
        let mut keybindings = default_emacs_keybindings();

        // Add custom keybindings
        // Ctrl+Space for completion menu
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char(' '),
            ReedlineEvent::Menu("completion_menu".to_string()),
        );

        // ESC triggers CtrlC signal for double-ESC handling
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Esc,
            ReedlineEvent::CtrlC,
        );

        // Bind Ctrl+C to a different signal type - let's try EndOfFile
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('c'),
            ReedlineEvent::UntilFound(vec![ReedlineEvent::CtrlD]), // Try CtrlD signal
        );

        // Create edit mode with keybindings
        let edit_mode = Box::new(Emacs::new(keybindings));

        // Create columnar completion menu
        let completion_menu = Box::new(
            ColumnarMenu::default()
                .with_columns(4)
                .with_column_width(None)
                .with_column_padding(2),
        );

        // Create validator for multi-line support
        let validator = Box::new(MultilineValidator);

        // Create completer
        let completer = Box::new(ArulaCompleter::new());

        // Create hinter
        let hinter = Box::new(ArulaHinter::new());

        // Create highlighter
        let highlighter = Box::new(ArulaSyntaxHighlighter);

        // Build reedline editor
        let editor = Reedline::create()
            .with_history(history)
            .with_edit_mode(edit_mode)
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_validator(validator)
            .with_completer(completer)
            .with_hinter(hinter)
            .with_highlighter(highlighter)
            .with_quick_completions(true)
            .with_partial_completions(true)
            .use_bracketed_paste(true); // Enable bracketed paste

        let prompt = ArulaPrompt::new(state.clone());

        Ok(Self {
            editor,
            prompt,
            state,
            history_path,
        })
    }

    /// Update AI state (for dynamic prompt)
    pub fn set_ai_state(&mut self, state: AiState) {
        if let Ok(mut app_state) = self.state.lock() {
            app_state.ai_state = state;
        }
    }

    /// Update token count (for dynamic prompt)
    pub fn set_token_count(&mut self, count: usize) {
        if let Ok(mut app_state) = self.state.lock() {
            app_state.token_count = count;
        }
    }

    /// Update session ID (for dynamic prompt)
    pub fn set_session_id(&mut self, id: String) {
        if let Ok(mut app_state) = self.state.lock() {
            app_state.session_id = id;
        }
    }

    /// Update token limit (for warnings)
    pub fn set_token_limit(&mut self, limit: usize) {
        if let Ok(mut app_state) = self.state.lock() {
            app_state.token_limit = limit;
        }
    }

    /// Check if should show token warning
    pub fn should_warn_tokens(&self) -> bool {
        if let Ok(state) = self.state.lock() {
            state.token_count >= state.token_limit
        } else {
            false
        }
    }

    /// Check if ESC was pressed and handle double-ESC logic
    /// Returns: None (continue), Some(true) (show menu), Some(false) (cancel/clear)
    pub fn check_esc_state(&mut self) -> Option<bool> {
        let state = self.state.lock().unwrap();
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(state.last_esc_time);

        // If more than 500ms passed, this can't be a double-ESC
        if elapsed.as_millis() > 500 {
            None
        } else if state.esc_count >= 2 {
            // Second ESC within 500ms - show menu
            Some(true)
        } else {
            None
        }
    }

    /// Track ESC press
    pub fn track_esc(&mut self) {
        let mut state = self.state.lock().unwrap();
        let now = std::time::Instant::now();
        state.last_esc_time = now;
        state.last_signal_time = now;
        state.esc_count += 1;
    }

    /// Reset ESC counter
    pub fn reset_esc(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.esc_count = 0;
    }

    /// Get input from user
    pub fn read_line(&mut self) -> Result<Option<String>> {
        loop {
            let sig = self.editor.read_line(&self.prompt)?;

            match sig {
                Signal::Success(buffer) => {
                    // Check for ESC Menu event
                    if buffer == "__ESC_EVENT__" {
                        // Track ESC press and handle double ESC logic
                        let now = std::time::Instant::now();
                        let should_show_menu = {
                            let state = self.state.lock().unwrap();
                            let elapsed = now.duration_since(state.last_esc_time);

                            // If less than 500ms since last ESC, show menu (double ESC)
                            if elapsed.as_millis() <= 500 && state.esc_count >= 1 {
                                true
                            } else {
                                false
                            }
                        };

                        self.track_esc();

                        if should_show_menu {
                            self.reset_esc();
                            return Ok(Some("__SHOW_MENU__".to_string()));
                        } else {
                            continue; // Single ESC, continue
                        }
                    }

                    // Reset ESC counter on successful input
                    self.reset_esc();

                    // Check for empty input
                    if buffer.trim().is_empty() {
                        continue; // Block empty messages
                    }

                    // Check token limit warning
                    if self.should_warn_tokens() {
                        eprintln!("\n⚠️  Warning: Message size ({} tokens) at/exceeds limit ({})",
                            self.state.lock().unwrap().token_count,
                            self.state.lock().unwrap().token_limit);
                        print!("Send anyway? (y/n): ");
                        std::io::Write::flush(&mut std::io::stdout())?;

                        let mut response = String::new();
                        std::io::stdin().read_line(&mut response)?;

                        if !response.trim().eq_ignore_ascii_case("y") {
                            continue;
                        }
                    }

                    // Process multi-line: remove trailing backslashes and join
                    let processed = buffer
                        .lines()
                        .map(|line| line.trim_end_matches('\\').trim_end())
                        .collect::<Vec<_>>()
                        .join(" ");

                    return Ok(Some(processed));
                }
                Signal::CtrlC => {
                    // ESC signal - handle double ESC logic
                    let now = std::time::Instant::now();
                    let should_show_main_menu = {
                        let state = self.state.lock().unwrap();
                        let elapsed_since_last_esc = now.duration_since(state.last_esc_time);

                        // If recent ESC activity and count >= 1, treat as double ESC
                        elapsed_since_last_esc.as_millis() <= 500 && state.esc_count >= 1
                    };

                    if should_show_main_menu {
                        // Double ESC - show main menu
                        self.reset_esc();
                        return Ok(Some("__SHOW_MENU__".to_string()));
                    }

                    // Track this ESC press
                    self.track_esc();

                    // First ESC - just clear input and continue
                    continue;
                }
                                Signal::CtrlD => {
                    // CtrlD signal - could be actual CtrlD or Ctrl+C (mapped via UntilFound)
                    // Since we use UntilFound to map Ctrl+C to CtrlD, treat as Ctrl+C
                    return Ok(Some("__SHOW_EXIT_MENU__".to_string()));
                }
            }
        }
    }

    /// Save history (called on graceful shutdown)
    pub fn save_history(&mut self) -> Result<()> {
        // Reedline's FileBackedHistory auto-saves on each entry
        // No manual save needed
        Ok(())
    }
}

impl Drop for ReedlineInput {
    fn drop(&mut self) {
        let _ = self.save_history();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert_eq!(state.ai_state, AiState::Ready);
        assert_eq!(state.token_count, 0);
        assert_eq!(state.token_limit, 8192);
    }

    #[test]
    fn test_multiline_validator() {
        let validator = MultilineValidator;

        assert_eq!(
            validator.validate("hello world\\"),
            ValidationResult::Incomplete
        );

        assert_eq!(
            validator.validate("hello world"),
            ValidationResult::Complete
        );
    }

    #[test]
    fn test_hinter_thresholds() {
        let hinter = ArulaHinter::new();

        // Command - should show after 3 chars
        assert!(!hinter.should_show_hint("/me"));
        assert!(hinter.should_show_hint("/menu"));

        // Text - should show after 8 chars
        assert!(!hinter.should_show_hint("hello"));
        assert!(hinter.should_show_hint("hello world!"));
    }
}
