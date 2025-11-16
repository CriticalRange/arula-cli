use anyhow::Result;
use clap::Parser;
use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType, EditMode};
use rustyline::history::DefaultHistory;
use crossterm::{
    cursor::SetCursorStyle,
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyEvent},
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use std::io::Write;

#[derive(Parser)]
#[command(name = "arula")]
#[command(about = "ARULA CLI - Autonomous AI Interface with chat", long_about = None)]
struct Cli {
    /// Run in verbose mode
    #[arg(short, long)]
    verbose: bool,

    /// API endpoint to connect to
    #[arg(long, default_value = "http://localhost:8080")]
    endpoint: String,

    /// Enable debug mode
    #[arg(short, long)]
    debug: bool,
}

mod app;
mod chat;
mod config;
mod output;
mod api;
mod tool_call;
mod overlay_menu;
mod agent;
mod agent_client;
mod tools;

use app::App;
use output::OutputHandler;
use overlay_menu::OverlayMenu;

/// Guard to ensure cursor and terminal are properly restored when the program exits
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Restore default cursor style on exit
        let _ = restore_default_cursor();
        let _ = console::Term::stdout().show_cursor();
        // Disable raw mode
        let _ = disable_raw_mode();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install color-eyre for better error reporting
    let _ = color_eyre::install();

    // Create terminal guard to ensure terminal state is restored on exit
    let _terminal_guard = TerminalGuard;

    let cli = Cli::parse();

    if cli.verbose {
        println!("🚀 Starting ARULA CLI with endpoint: {}", cli.endpoint);
    }

    // Set debug environment variable if debug flag is enabled
    if cli.debug {
        std::env::set_var("ARULA_DEBUG", "1");
    }

    // Enable raw mode for keyboard input detection
    enable_raw_mode().unwrap();

    // Create output handler and app with debug flag
    let mut output = OutputHandler::new().with_debug(cli.debug);
    let mut app = App::new()?.with_debug(cli.debug);

    // Initialize AI client if configuration is valid
    match app.initialize_agent_client() {
        Ok(()) => {
            if cli.verbose {
                println!("✅ AI client initialized successfully");
            }
        }
        Err(e) => {
            if cli.verbose {
                println!("⚠️  AI client initialization failed: {}", e);
                println!("💡 You can configure AI settings in the application menu");
            }
        }
    }

    // Print banner
    output.print_banner()?;
    println!();
    println!("{}", console::style("💡 Tips:").cyan().bold());
    println!("{}", console::style("  • Type your message and press Enter to send").dim());
    println!("{}", console::style("  • Use Shift+Enter for new lines, Enter on empty line to finish").dim());
    println!("{}", console::style("  • Paste multi-line content, press Enter on empty line to finish").dim());
    println!("{}", console::style("  • End line with \\ to continue typing on next line").dim());
    println!("{}", console::style("  • Cursor changed to steady bar for better visibility").dim());
    println!();

    // Create rustyline editor with enhanced multi-line support
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .auto_add_history(true)
        .bracketed_paste(true)  // Enable bracketed paste mode
        .tab_stop(4)           // Set tab width for code blocks
        .build();

    let mut rl: Editor<(), DefaultHistory> = Editor::with_config(config)?;

    // Create overlay menu
    let mut menu = OverlayMenu::new();

    // Load history if exists
    let history_path = dirs::home_dir()
        .map(|p| p.join(".arula_history"))
        .unwrap_or_else(|| std::path::PathBuf::from(".arula_history"));

    let _ = rl.load_history(&history_path);

    // Main input loop
    loop {
        // Only process AI responses if we're currently waiting for one
        if app.is_waiting_for_response() {
            // Check for ESC key press to cancel
            if event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
                if let Ok(Event::Key(KeyEvent { code: KeyCode::Esc, .. })) = event::read() {
                    output.stop_spinner();
                    print!("\r\x1b[K"); // Clear spinner line
                    std::io::stdout().flush()?;
                    output.print_system("🛑 Request cancelled")?;
                    app.cancel_request();
                    continue;
                }
            }

            match app.check_ai_response_nonblocking() {
                Some(response) => {
                    match response {
                        app::AiResponse::AgentStreamStart => {
                            output.start_ai_message()?;
                        }
                        app::AiResponse::AgentStreamText(text) => {
                            output.print_streaming_chunk(&text)?;
                        }
                        app::AiResponse::AgentToolCall { id: _, name, arguments } => {
                            output.start_tool_execution(&name, &arguments)?;
                        }
                        app::AiResponse::AgentToolResult { tool_call_id: _, success, result } => {
                            let result_text = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            output.complete_tool_execution(&result_text, success)?;
                        }
                        app::AiResponse::AgentStreamEnd => {
                            // Stop spinner and cleanup
                            output.stop_spinner();
                            output.end_line()?;
                            // Show context usage for agent responses (no usage data available from agent system)
                            output.print_context_usage(None)?;
                        }
                    }
                }
                None => {
                    // No response yet, wait a bit and try again
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    continue; // Skip to next iteration without reading user input
                }
            }
        }

        // Only read user input if we're not waiting for a response
        let readline = read_multiline_input(&mut rl, &app);
        match readline {
            Ok(input) => {
                let input = input.trim();

                // Check for empty input
                if input.is_empty() {
                    continue;
                }

                // Check for special shortcuts
                if input == "m" || input == "menu" {
                    // Quick menu shortcut
                    if menu.show_main_menu(&mut app, &mut output)? {
                        break;
                    }
                    continue;
                }

                // Check for exit commands
                if input == "exit" || input == "quit" {
                    output.print_system("Goodbye! 👋")?;
                    break;
                }

                // Handle command
                if input.starts_with('/') {
                    // Handle CLI commands
                    handle_cli_command(input, &mut app, &mut output, &mut menu).await?;
                } else {
                    // User input is already shown in the prompt, don't duplicate

                    // Check if we're already waiting for a response
                    if !app.is_waiting_for_response() {
                        // Send to AI
                        app.send_to_ai(input).await?;

                        // Show AI loading spinner
                        print!("\n"); // Create space for spinner
                        std::io::stdout().flush()?;
                        output.start_spinner("Thinking...")?;
                        // Give spinner time to start animating
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    } else {
                        // Already waiting for a response, ignore this input
                        output.print_system("⚠️  Already processing a request, please wait...")?;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C - Show exit confirmation
                if menu.show_exit_confirmation(&mut output)? {
                    // Exit confirmed
                    break;
                }
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D
                output.print_system("Goodbye! 👋")?;
                break;
            }
            Err(err) => {
                output.print_error(&format!("Error: {:?}", err))?;
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history(&history_path);

    // Cursor will be automatically shown by CursorGuard's Drop implementation

    Ok(())
}

fn setup_bar_cursor() -> Result<(), Box<dyn std::error::Error>> {
    // Set cursor to a steady bar cursor (not blinking)
    std::io::stdout().execute(SetCursorStyle::SteadyBar)?;
    Ok(())
}

fn restore_default_cursor() -> Result<(), Box<dyn std::error::Error>> {
    // Restore cursor to default blinking line
    std::io::stdout().execute(SetCursorStyle::DefaultUserShape)?;
    Ok(())
}

fn read_multiline_input(rl: &mut Editor<(), DefaultHistory>, app: &App) -> Result<String, ReadlineError> {
    use console::style;

    // Setup bar cursor
    let _ = setup_bar_cursor();

    let mut lines = Vec::new();
    let mut in_multiline = false;

    loop {
        // Enhanced prompt with animated loading states
        let prompt = if lines.is_empty() {
            // Show animated loading state
            let is_loading = app.is_waiting_for_response() || app.has_pending_tool_calls();
            format!("{} ", style("▶").cyan())
        } else {
            format!("{} ", style("│  ").cyan().dim())
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                // Check if this is a bracketed paste (contains newlines)
                if line.contains('\n') {
                    let pasted_lines: Vec<&str> = line.lines().collect();
                    let total_lines = pasted_lines.len();

                    // Show preview with cleaner formatting
                    if total_lines > 0 {
                        println!("  {}", style(format!("📋 {} lines pasted", total_lines)).yellow());

                        // Show first few lines as preview
                        for preview_line in pasted_lines.iter().take(3) {
                            let truncated = if preview_line.len() > 60 {
                                format!("{}...", &preview_line[..60])
                            } else {
                                preview_line.to_string()
                            };
                            println!("  {} {}", style("│").cyan().dim(), style(truncated).dim());
                        }

                        if total_lines > 3 {
                            println!("  {} {}", style("│").cyan().dim(), style(format!("... and {} more lines", total_lines - 3)).dim());
                        }
                    }

                    // Add all pasted lines
                    lines.extend(pasted_lines.iter().map(|s| s.to_string()));
                    in_multiline = true;
                    continue;
                }

                // Line ends with backslash = manual continuation
                if line.trim_end().ends_with('\\') {
                    let mut content = line.trim_end().to_string();
                    content.pop(); // Remove backslash
                    lines.push(content);
                    in_multiline = true;
                    continue;
                }

                // Empty line behavior
                if line.trim().is_empty() {
                    if lines.is_empty() {
                        // Empty first line - cancel
                        return Err(ReadlineError::Interrupted);
                    } else if in_multiline {
                        // Empty line after paste/multiline - finish
                        break;
                    } else {
                        // Empty line on single input - cancel
                        return Err(ReadlineError::Interrupted);
                    }
                }

                // Add current line
                lines.push(line);

                // Auto-send if single line and not in multiline mode
                if !in_multiline {
                    break;
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    Ok(lines.join("\n"))
}

async fn handle_cli_command(
    input: &str,
    app: &mut App,
    output: &mut OutputHandler,
    menu: &mut OverlayMenu,
) -> Result<()> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let command = parts[0];

    match command {
        "/help" => {
            output.print_system("╔══════════════════════════════════════╗")?;
            output.print_system("║          ARULA HELP MENU             ║")?;
            output.print_system("╚══════════════════════════════════════╝")?;
            output.print_system("")?;
            output.print_system("📋 Commands:")?;
            output.print_system("  /help              - Show this help")?;
            output.print_system("  /menu              - Open interactive menu")?;
            output.print_system("  /clear             - Clear conversation history")?;
            output.print_system("  /history           - Show message history")?;
            output.print_system("  /summary           - Show conversation summary")?;
            output.print_system("  /config            - Show current configuration")?;
            output.print_system("  /model <name>      - Change AI model")?;
            output.print_system("  exit or quit       - Exit ARULA")?;
            output.print_system("")?;
            output.print_system("⌨️  Quick Shortcuts:")?;
            output.print_system("  m         - Open menu (type 'm' and press Enter)")?;
            output.print_system("  menu      - Open menu")?;
            output.print_system("  Ctrl+C    - Exit confirmation")?;
            output.print_system("  Ctrl+D    - Exit immediately")?;
            output.print_system("")?;
            output.print_system("💡 TIP: Just type 'm' to open the menu anytime!")?;
        }
        "/menu" => {
            // Show menu
            if menu.show_main_menu(app, output)? {
                // Exit requested
                std::process::exit(0);
            }
        }
        "/clear" => {
            app.clear_conversation();
            output.print_system("Conversation cleared")?;
        }
        "/history" => {
            let messages = app.get_message_history();
            output.print_message_history(&messages, 0)?;
        }
        "/summary" => {
            let messages = app.get_message_history();
            output.print_conversation_summary(&messages)?;
        }
        "/config" => {
            let config = app.get_config();
            output.print_system(&format!("Provider: {}", config.ai.provider))?;
            output.print_system(&format!("Model: {}", config.ai.model))?;
            output.print_system(&format!("API Key: {}", if config.ai.api_key.is_empty() { "Not set" } else { "Set" }))?;
        }
        "/model" => {
            if parts.len() < 2 {
                output.print_error("Usage: /model <name>")?;
            } else {
                let model = parts[1];
                app.set_model(model);
                output.print_system(&format!("Model changed to: {}", model))?;
            }
        }
        _ => {
            output.print_error(&format!("Unknown command: {}", command))?;
            output.print_system("Type /help for available commands")?;
        }
    }

    Ok(())
}
