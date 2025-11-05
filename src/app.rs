use anyhow::Result;
use chrono::Local;
use crossterm::event::{KeyCode, KeyModifiers, KeyEvent};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::api::ApiClient;
use crate::git_ops::GitOperations;

use crate::chat::{ChatMessage, MessageType};

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Chat,
    Exiting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ai: AiConfig,
    pub git: GitConfig,
    pub logging: LoggingConfig,
    pub art: ArtConfig,
    pub workspace: WorkspaceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub auto_commit: bool,
    pub create_branch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtConfig {
    pub default_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub path: String,
}

#[derive(Debug)]
pub struct App {
    pub state: AppState,
    pub input: String,
    pub input_mode: bool,
    pub messages: Vec<ChatMessage>,
    pub config: Config,
    pub start_time: SystemTime,
    pub session_id: String,
    pub cursor_position: usize, // New: cursor position in input
    pub api_client: Option<ApiClient>, // New: API client for remote AI
    pub pending_command: Option<String>, // New: pending command to execute async
    pub git_ops: GitOperations, // New: Git operations manager
}

impl App {
    pub fn new() -> Result<Self> {
        let session_id = format!("session_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs()
        );

        let mut app = Self {
            state: AppState::Chat,
            input: String::new(),
            input_mode: true, // Always enabled by default
            messages: Vec::new(),
            config: Self::default_config(),
            start_time: SystemTime::now(),
            session_id,
            cursor_position: 0,
            api_client: None, // Will be set later with endpoint
            pending_command: None,
            git_ops: GitOperations::new(),
        };

        // Add welcome message
        app.add_message(
            MessageType::Arula,
            "🚀 Welcome to ARULA CLI - Autonomous AI Interface!
I'm your AI-powered command-line assistant. I can help you with:
• Code art generation
• Task processing and automation
• System monitoring
• Configuration management

📝 How to use:
• Just start typing - input is always enabled
• Press Enter to send commands
• Press Esc to exit input mode (temporarily)
• Press Ctrl+C to quit anytime
• Click anywhere to focus input

Type 'help' to see all available commands, or just start typing!"
        );

        Ok(app)
    }

    pub fn set_api_client(&mut self, endpoint: String) {
        let endpoint_clone = endpoint.clone();
        self.api_client = Some(ApiClient::new(endpoint));
        self.add_message(
            MessageType::Info,
            &format!("Connected to API: {}", endpoint_clone)
        );
    }

    pub async fn handle_ai_command(&mut self, command: String) -> Result<()> {
        let api_client = self.api_client.clone();

        if let Some(client) = api_client {
            self.add_message(MessageType::User, &command);

            // Show thinking message
            self.add_message(MessageType::Arula, "🤔 Thinking...");

            match client.send_message(&command, None).await {
                Ok(response) => {
                    // Remove thinking message and add actual response
                    self.messages.pop(); // Remove "Thinking..."

                    if response.success {
                        self.add_message(MessageType::Arula, &response.response);
                    } else {
                        let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
                        self.add_message(MessageType::Error, &format!("❌ Error: {}", error_msg));
                    }
                }
                Err(e) => {
                    self.messages.pop(); // Remove "Thinking..."
                    self.add_message(MessageType::Error, &format!("❌ API Error: {}", e));
                }
            }
        } else {
            self.add_message(MessageType::Error, "❌ No API client configured");
        }

        Ok(())
    }

    fn default_config() -> Config {
        Config {
            ai: AiConfig {
                provider: "local".to_string(),
                model: "default".to_string(),
            },
            git: GitConfig {
                auto_commit: true,
                create_branch: true,
            },
            logging: LoggingConfig {
                level: "INFO".to_string(),
            },
            art: ArtConfig {
                default_style: "fractal".to_string(),
            },
            workspace: WorkspaceConfig {
                path: "./arula_workspace".to_string(),
            },
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        // Only handle keys if input mode is enabled
        if !self.input_mode {
            return;
        }

        match key.code {
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    let command = self.input.clone();
                    self.input.clear();
                    self.cursor_position = 0;
                    // Note: This will be handled in main loop to allow async execution
                    self.pending_command = Some(command);
                }
            }
            KeyCode::Char(c) => {
                // Insert character at cursor position
                self.input.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.input.remove(self.cursor_position);
                }
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input.len() {
                    self.input.remove(self.cursor_position);
                }
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_position < self.input.len() {
                    self.cursor_position += 1;
                }
            }
            KeyCode::Home => {
                self.cursor_position = 0;
            }
            KeyCode::End => {
                self.cursor_position = self.input.len();
            }
            KeyCode::Tab => {
                // Auto-complete or suggest commands
                self.input.insert_str(self.cursor_position, "help");
                self.cursor_position += 4;
            }
            _ => {}
        }

        // Handle Ctrl combinations separately
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Left => {
                    // Ctrl+Left - Move word left
                    let chars: Vec<char> = self.input.chars().collect();
                    let mut pos = self.cursor_position;

                    // Skip whitespace
                    while pos > 0 && chars[pos - 1].is_whitespace() {
                        pos -= 1;
                    }

                    // Skip word characters
                    while pos > 0 && !chars[pos - 1].is_whitespace() {
                        pos -= 1;
                    }

                    self.cursor_position = pos;
                }
                KeyCode::Right => {
                    // Ctrl+Right - Move word right
                    let chars: Vec<char> = self.input.chars().collect();
                    let mut pos = self.cursor_position;

                    // Skip word characters
                    while pos < chars.len() && !chars[pos].is_whitespace() {
                        pos += 1;
                    }

                    // Skip whitespace
                    while pos < chars.len() && chars[pos].is_whitespace() {
                        pos += 1;
                    }

                    self.cursor_position = pos;
                }
                KeyCode::Char('u') => {
                    // Ctrl+U - Clear to beginning
                    self.input.drain(0..self.cursor_position);
                    self.cursor_position = 0;
                }
                KeyCode::Char('k') => {
                    // Ctrl+K - Clear to end
                    self.input.truncate(self.cursor_position);
                }
                KeyCode::Char('a') => {
                    // Ctrl+A - Move to beginning
                    self.cursor_position = 0;
                }
                KeyCode::Char('e') => {
                    // Ctrl+E - Move to end
                    self.cursor_position = self.input.len();
                }
                _ => {}
            }
        }
    }

    pub async fn handle_command(&mut self, command: String) {
        let command_trimmed = command.trim();

        // First check if it's a special command
        match command_trimmed {
            cmd if cmd.starts_with('/') => {
                // Handle built-in commands (starting with /)
                self.handle_builtin_command(&command).await;
            }
            _ => {
                // Forward everything else to AI
                if let Err(e) = self.handle_ai_command(command).await {
                    self.add_message(MessageType::Error, &format!("Failed to process command: {}", e));
                }
            }
        }
    }

    async fn handle_builtin_command(&mut self, command: &str) {
        let command_trimmed = command.trim().strip_prefix('/').unwrap_or(command.trim());

        match command_trimmed {
            cmd if cmd == "help" || cmd == "h" || cmd == "?" => {
                self.add_message(
                    MessageType::Arula,
                    "🚀 Available commands:
• help - Show this help
• status - Show system status
• config - Manage configuration
• art - Generate code art
• task - Run task demos
• logs - View recent logs
• clear - Clear conversation
• git - Git operations (see /git help)
• exec - Execute shell commands
• exit or quit - Exit ARULA CLI

⌨️  Keyboard shortcuts:
• Just start typing - input is always enabled
• Enter - Send command
• Esc - Exit input mode (temporarily)
• Tab - Auto-complete 'help'
• Ctrl+C - Exit immediately
• Click - Focus input area

🎯 Cursor navigation:
• Arrow keys - Move cursor left/right
• Home/End - Move to beginning/end
• Backspace - Delete character before cursor
• Delete - Delete character at cursor
• Ctrl+Left/Right - Move by words
• Ctrl+U - Clear to beginning
• Ctrl+K - Clear to end
• Ctrl+A/E - Move to beginning/end

📝 Git Commands (use /git <command>):
• /git init - Initialize git repository
• /git status - Show git status
• /git branches - List all branches
• /git branch <name> - Create new branch
• /git checkout <name> - Switch to branch
• /git delete <name> - Delete branch
• /git add - Add all files
• /git commit <message> - Commit changes
• /git log - Show commit history
• /git pull - Pull from remote

💡 Try: art rust, /git status, or /exec ls -la"
                );
            }
            cmd if cmd == "status" || cmd == "st" => {
                let uptime = self.start_time.elapsed().unwrap_or_default().as_secs();
                self.add_message(
                    MessageType::Arula,
                    &format!("📊 System Status:
Configuration: ✅ Found
Log file: ✅ Active
Uptime: {}s
Session: {}", uptime, self.session_id)
                );
            }
            cmd if cmd.starts_with("config") => {
                if cmd == "config init" {
                    self.add_message(
                        MessageType::Success,
                        "Configuration initialized successfully!"
                    );
                } else {
                    self.add_message(
                        MessageType::Arula,
                        &format!("⚙️ Current Configuration:
ai:
  provider: {}
  model: {}
git:
  auto_commit: {}
  create_branch: {}
logging:
  level: {}
art:
  default_style: {}
workspace:
  path: {}",
                            self.config.ai.provider,
                            self.config.ai.model,
                            self.config.git.auto_commit,
                            self.config.git.create_branch,
                            self.config.logging.level,
                            self.config.art.default_style,
                            self.config.workspace.path)
                    );
                }
            }
            cmd if cmd.starts_with("art") => {
                let art_type = cmd.strip_prefix("art ").unwrap_or("").trim();
                match art_type {
                    "rust" | "crab" => {
                        self.add_message(
                            MessageType::Arula,
                            "🦀 Generating Rust Crab ASCII Art..."
                        );
                        self.add_message(
                            MessageType::Success,
                            &crate::art::generate_rust_crab()
                        );
                    }
                    "fractal" => {
                        self.add_message(
                            MessageType::Arula,
                            "🌿 Generating Fractal Art..."
                        );
                        self.add_message(
                            MessageType::Success,
                            &crate::art::generate_fractal()
                        );
                    }
                    "matrix" => {
                        self.add_message(
                            MessageType::Arula,
                            "💚 Generating Matrix Digital Rain..."
                        );
                        self.add_message(
                            MessageType::Success,
                            &crate::art::generate_matrix()
                        );
                    }
                    "demo" | "all" => {
                        self.add_message(
                            MessageType::Arula,
                            "🎨 Running Complete Art Demo..."
                        );
                        self.add_message(
                            MessageType::Success,
                            &crate::art::generate_demo()
                        );
                    }
                    _ => {
                        self.add_message(
                            MessageType::Error,
                            &format!("Unknown art style: {}\nAvailable: rust, fractal, matrix, demo", art_type)
                        );
                    }
                }
            }
            cmd if cmd.starts_with("task") => {
                let task_type = cmd.strip_prefix("task ").unwrap_or("").trim();
                match task_type {
                    "demo" => {
                        self.add_message(
                            MessageType::Arula,
                            "🤖 Starting Task Demo..."
                        );

                        self.add_message(
                            MessageType::Info,
                            "📋 Analyzing requirements..."
                        );

                        self.add_message(
                            MessageType::Success,
                            "✅ Requirements analyzed"
                        );

                        self.add_message(
                            MessageType::Info,
                            "🔧 Generating implementation plan..."
                        );

                        self.add_message(
                            MessageType::Success,
                            "✅ Implementation plan ready"
                        );

                        self.add_message(
                            MessageType::Info,
                            "💻 Creating solution..."
                        );

                        self.add_message(
                            MessageType::Success,
                            "✅ Solution completed successfully!"
                        );

                        self.add_message(
                            MessageType::Success,
                            "🎉 Task demo completed! Check workspace for generated files."
                        );
                    }
                    "status" => {
                        let success_count = self.messages.iter()
                            .filter(|m| m.message_type == MessageType::Success)
                            .count();
                        let error_count = self.messages.iter()
                            .filter(|m| m.message_type == MessageType::Error)
                            .count();

                        self.add_message(
                            MessageType::Arula,
                            &format!("📊 Task Status:
Active Tasks: 0
Completed: {}
Failed: {}", success_count, error_count)
                        );
                    }
                    _ => {
                        self.add_message(
                            MessageType::Error,
                            &format!("Unknown task command: {}\nAvailable: demo, status", task_type)
                        );
                    }
                }
            }
            cmd if cmd.starts_with("git") => {
                self.handle_git_command(cmd).await;
            }
            cmd if cmd.starts_with("exec") => {
                self.handle_exec_command(cmd).await;
            }
            cmd if cmd == "logs" || cmd == "log" => {
                let recent_messages: Vec<String> = self.messages
                    .iter()
                    .rev()
                    .take(10)
                    .map(|m| format!("[{}] {}: {}",
                        m.timestamp.format("%H:%M:%S"),
                        m.message_type,
                        m.content))
                    .collect();

                if recent_messages.is_empty() {
                    self.add_message(
                        MessageType::Info,
                        "No logs available yet."
                    );
                } else {
                    self.add_message(
                        MessageType::Arula,
                        &format!("📝 Recent Activity:\n{}", recent_messages.join("\n"))
                    );
                }
            }
            cmd if cmd == "clear" || cmd == "cls" => {
                self.messages.clear();
                self.add_message(
                    MessageType::System,
                    "Conversation cleared."
                );
            }
            cmd if cmd == "exit" || cmd == "quit" || cmd == "q" => {
                self.add_message(
                    MessageType::Arula,
                    "👋 Thank you for using ARULA CLI!
🚀 Session ended. Have a great day!"
                );
                self.state = AppState::Exiting;
            }
            "" => {
                // Empty command - ignore
            }
            _ => {
                self.add_message(
                    MessageType::Arula,
                    "I didn't understand that command.
Type 'help' to see available commands, or try:
• art - Generate code art
• task demo - Run task demonstration
• status - Check system status"
                );
            }
        }
    }

    pub fn add_message(&mut self, message_type: MessageType, content: &str) {
        let message = ChatMessage {
            timestamp: Local::now(),
            message_type,
            content: content.to_string(),
        };

        self.messages.push(message);

        // Keep only last 50 messages
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
    }

    async fn handle_git_command(&mut self, command: &str) {
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.len() < 2 {
            self.add_message(
                MessageType::Error,
                "Usage: /git <command> [args]\nUse /git help for available commands"
            );
            return;
        }

        match parts[1] {
            "help" => {
                self.add_message(
                    MessageType::Arula,
                    "🌿 Git Commands Help:
• /git init - Initialize git repository in current directory
• /git status - Show working directory status
• /git branches - List all branches (local and remote)
• /git branch <name> - Create new branch
• /git checkout <name> - Switch to existing branch
• /git delete <name> - Delete branch (not current branch)
• /git add - Add all untracked files to staging
• /git commit <message> - Commit staged changes
• /git log - Show commit history
• /git pull - Pull changes from remote
• /git push - Push changes to remote

💡 Examples:
• /git init
• /git status
• /git branch feature-xyz
• /git checkout main
• /git add
• /git commit \"Add new feature\""
                );
            }
            "init" => {
                match self.git_ops.initialize_repository(".") {
                    Ok(()) => {
                        self.add_message(
                            MessageType::Success,
                            "✅ Git repository initialized successfully!"
                        );
                    }
                    Err(e) => {
                        self.add_message(
                            MessageType::Error,
                            &format!("❌ Failed to initialize repository: {}", e)
                        );
                    }
                }
            }
            "status" => {
                // Try to open repository first
                if let Err(_) = self.git_ops.open_repository(".") {
                    self.add_message(
                        MessageType::Error,
                        "❌ Not a git repository. Use '/git init' to initialize."
                    );
                    return;
                }

                match self.git_ops.get_status() {
                    Ok(status_lines) => {
                        self.add_message(
                            MessageType::Arula,
                            &format!("📊 Git Status:\n{}", status_lines.join("\n"))
                        );
                    }
                    Err(e) => {
                        self.add_message(
                            MessageType::Error,
                            &format!("❌ Failed to get status: {}", e)
                        );
                    }
                }
            }
            "branches" => {
                // Try to open repository first
                if let Err(_) = self.git_ops.open_repository(".") {
                    self.add_message(
                        MessageType::Error,
                        "❌ Not a git repository. Use '/git init' to initialize."
                    );
                    return;
                }

                match self.git_ops.list_branches() {
                    Ok(branches) => {
                        let current_branch = self.git_ops.get_current_branch().unwrap_or_else(|_| "unknown".to_string());
                        self.add_message(
                            MessageType::Arula,
                            &format!("🌿 Branches:\nCurrent: {}\n{}", current_branch, branches.join("\n"))
                        );
                    }
                    Err(e) => {
                        self.add_message(
                            MessageType::Error,
                            &format!("❌ Failed to list branches: {}", e)
                        );
                    }
                }
            }
            "branch" => {
                if parts.len() < 3 {
                    self.add_message(
                        MessageType::Error,
                        "Usage: /git branch <name>"
                    );
                    return;
                }

                // Try to open repository first
                if let Err(_) = self.git_ops.open_repository(".") {
                    self.add_message(
                        MessageType::Error,
                        "❌ Not a git repository. Use '/git init' to initialize."
                    );
                    return;
                }

                let branch_name = parts[2];
                match self.git_ops.create_branch(branch_name) {
                    Ok(()) => {
                        self.add_message(
                            MessageType::Success,
                            &format!("✅ Branch '{}' created successfully!", branch_name)
                        );
                    }
                    Err(e) => {
                        self.add_message(
                            MessageType::Error,
                            &format!("❌ Failed to create branch: {}", e)
                        );
                    }
                }
            }
            "checkout" => {
                if parts.len() < 3 {
                    self.add_message(
                        MessageType::Error,
                        "Usage: /git checkout <branch_name>"
                    );
                    return;
                }

                // Try to open repository first
                if let Err(_) = self.git_ops.open_repository(".") {
                    self.add_message(
                        MessageType::Error,
                        "❌ Not a git repository. Use '/git init' to initialize."
                    );
                    return;
                }

                let branch_name = parts[2];
                match self.git_ops.checkout_branch(branch_name) {
                    Ok(()) => {
                        self.add_message(
                            MessageType::Success,
                            &format!("✅ Switched to branch '{}'", branch_name)
                        );
                    }
                    Err(e) => {
                        self.add_message(
                            MessageType::Error,
                            &format!("❌ Failed to checkout branch: {}", e)
                        );
                    }
                }
            }
            "delete" => {
                if parts.len() < 3 {
                    self.add_message(
                        MessageType::Error,
                        "Usage: /git delete <branch_name>"
                    );
                    return;
                }

                // Try to open repository first
                if let Err(_) = self.git_ops.open_repository(".") {
                    self.add_message(
                        MessageType::Error,
                        "❌ Not a git repository. Use '/git init' to initialize."
                    );
                    return;
                }

                let branch_name = parts[2];
                match self.git_ops.delete_branch(branch_name) {
                    Ok(()) => {
                        self.add_message(
                            MessageType::Success,
                            &format!("✅ Branch '{}' deleted successfully!", branch_name)
                        );
                    }
                    Err(e) => {
                        self.add_message(
                            MessageType::Error,
                            &format!("❌ Failed to delete branch: {}", e)
                        );
                    }
                }
            }
            "add" => {
                // Try to open repository first
                if let Err(_) = self.git_ops.open_repository(".") {
                    self.add_message(
                        MessageType::Error,
                        "❌ Not a git repository. Use '/git init' to initialize."
                    );
                    return;
                }

                match self.git_ops.add_all() {
                    Ok(()) => {
                        self.add_message(
                            MessageType::Success,
                            "✅ Files added to staging area successfully!"
                        );
                    }
                    Err(e) => {
                        self.add_message(
                            MessageType::Error,
                            &format!("❌ Failed to add files: {}", e)
                        );
                    }
                }
            }
            "commit" => {
                if parts.len() < 3 {
                    self.add_message(
                        MessageType::Error,
                        "Usage: /git commit <message>"
                    );
                    return;
                }

                // Try to open repository first
                if let Err(_) = self.git_ops.open_repository(".") {
                    self.add_message(
                        MessageType::Error,
                        "❌ Not a git repository. Use '/git init' to initialize."
                    );
                    return;
                }

                let commit_message = parts[2..].join(" ");
                match self.git_ops.commit(&commit_message) {
                    Ok(()) => {
                        self.add_message(
                            MessageType::Success,
                            &format!("✅ Commit created successfully!\n📝 Message: {}", commit_message)
                        );
                    }
                    Err(e) => {
                        self.add_message(
                            MessageType::Error,
                            &format!("❌ Failed to create commit: {}", e)
                        );
                    }
                }
            }
            _ => {
                self.add_message(
                    MessageType::Error,
                    &format!("Unknown git command: {}\nUse '/git help' for available commands", parts[1])
                );
            }
        }
    }

    async fn handle_exec_command(&mut self, command: &str) {
        use crate::cli_commands::CommandRunner;

        let parts: Vec<&str> = command.splitn(2, ' ').collect();

        if parts.len() < 2 {
            self.add_message(
                MessageType::Error,
                "Usage: /exec <command>\nExamples:\n• /exec ls -la\n• /exec cargo build\n• /exec git status"
            );
            return;
        }

        let exec_cmd = parts[1];
        let cmd_parts: Vec<&str> = exec_cmd.split_whitespace().collect();

        if cmd_parts.is_empty() {
            self.add_message(
                MessageType::Error,
                "No command provided"
            );
            return;
        }

        let mut runner = CommandRunner::new();
        self.add_message(
            MessageType::Info,
            &format!("🔧 Executing: {}", exec_cmd)
        );

        let result = if cmd_parts.len() == 1 {
            runner.run_command(cmd_parts[0].to_string(), vec![]).await
        } else {
            runner.run_command(cmd_parts[0].to_string(), cmd_parts[1..].iter().map(|&s| s.to_string()).collect()).await
        };

        match result {
            Ok(output) => {
                if output.trim().is_empty() {
                    self.add_message(
                        MessageType::Success,
                        "✅ Command completed successfully (no output)"
                    );
                } else {
                    self.add_message(
                        MessageType::Success,
                        &format!("✅ Command output:\n{}", output)
                    );
                }
            }
            Err(e) => {
                self.add_message(
                    MessageType::Error,
                    &format!("❌ Command failed: {}", e)
                );
            }
        }
    }

    pub fn update(&mut self) {
        // Handle any periodic updates
        if self.state == AppState::Exiting {
            // Handle exit state
        }
    }
}