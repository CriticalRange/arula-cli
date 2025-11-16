use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::{self, MoveDown, MoveUp, SetCursorStyle},
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{self, disable_raw_mode, enable_raw_mode, ClearType},
    ExecutableCommand,
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

mod agent;
mod agent_client;
mod api;
mod app;
mod chat;
mod config;
mod custom_spinner;
mod input_handler;
mod output;
mod overlay_menu;
mod tool_call;
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

    // Print banner BEFORE enabling raw mode
    output.print_banner()?;
    println!();
    println!("{}", console::style("💡 Tips:").cyan().bold());
    println!(
        "{}",
        console::style("  • Type your message and press Enter to send").dim()
    );
    println!(
        "{}",
        console::style("  • Use Shift+Enter for new lines, Enter on empty line to finish").dim()
    );
    println!(
        "{}",
        console::style("  • Paste multi-line content, press Enter on empty line to finish").dim()
    );
    println!(
        "{}",
        console::style("  • End line with \\ to continue typing on next line").dim()
    );
    println!(
        "{}",
        console::style("  • Cursor changed to steady bar for better visibility").dim()
    );
    println!();

    // NOW enable raw mode for keyboard input detection
    enable_raw_mode()?;

    // Create custom input handler and spinner
    let prompt = if cfg!(windows) { "▶ " } else { "▶ " };
    let mut input_handler = input_handler::InputHandler::new(prompt);
    let mut custom_spinner = custom_spinner::CustomSpinner::new();

    // Create overlay menu
    let mut menu = OverlayMenu::new();

    // Load history if exists
    let history_path = dirs::home_dir()
        .map(|p| p.join(".arula_history"))
        .unwrap_or_else(|| std::path::PathBuf::from(".arula_history"));

    if let Ok(contents) = std::fs::read_to_string(&history_path) {
        let lines: Vec<String> = contents.lines().map(|s| s.to_string()).collect();
        input_handler.load_history(lines);
    }

    // Print initial prompt on a new line
    input_handler.draw()?;

    // Main event loop
    loop {
        // Process AI responses if waiting
        while app.is_waiting_for_response() {
            match app.check_ai_response_nonblocking() {
                Some(response) => {
                    match response {
                        app::AiResponse::AgentStreamStart => {
                            // Stop the spinner (it's on the wrong line)
                            if custom_spinner.is_running() {
                                custom_spinner.stop();
                            }

                            // Add blank line for spacing between user message and AI response
                            println!();

                            // Restart spinner on the new line
                            custom_spinner.start("Thinking...")?;
                            output.start_ai_message()?;
                        }
                        app::AiResponse::AgentStreamText(text) => {
                            // Stop spinner (it clears its line) - only on first chunk
                            let is_first_chunk = custom_spinner.is_running();
                            if is_first_chunk {
                                custom_spinner.stop();
                                // Explicitly clear the line and reset cursor
                                execute!(
                                    std::io::stdout(),
                                    cursor::MoveToColumn(0),
                                    terminal::Clear(terminal::ClearType::CurrentLine)
                                )?;
                                // Add left margin (one space) and print first chunk with markdown
                                print!(" ");
                                std::io::stdout().flush()?;
                                output.print_streaming_chunk(&text)?;
                            } else {
                                // Print subsequent chunks with markdown rendering
                                output.print_streaming_chunk(&text)?;
                            }
                        }
                        app::AiResponse::AgentToolCall {
                            id: _,
                            name,
                            arguments,
                        } => {
                            custom_spinner.stop();
                            output.start_tool_execution(&name, &arguments)?;
                        }
                        app::AiResponse::AgentToolResult {
                            tool_call_id: _,
                            success,
                            result,
                        } => {
                            let result_text = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            output.complete_tool_execution(&result_text, success)?;
                        }
                        app::AiResponse::AgentStreamEnd => {
                            // Stop spinner and cleanup
                            custom_spinner.stop();
                            output.stop_spinner();
                            output.end_line()?;
                            output.print_context_usage(None)?;
                            // Redraw input prompt after response completes
                            println!();
                            input_handler.draw()?;
                        }
                    }
                }
                None => {
                    // No response yet, start spinner if not running
                    if !custom_spinner.is_running() {
                        custom_spinner.start("Thinking...")?;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            }
        }

        // Poll for keyboard events (non-blocking)
        if event::poll(std::time::Duration::from_millis(10))? {
            if let Event::Key(key_event) = event::read()? {
                // Only handle key press events, ignore key release
                if key_event.kind != event::KeyEventKind::Press {
                    continue;
                }

                match input_handler.handle_key(key_event)? {
                    Some(input) => {
                        // Handle special signals
                        if input == "__CTRL_C__" {
                            // Ctrl+C pressed
                            if app.is_waiting_for_response() {
                                custom_spinner.stop();
                                output.print_system("🛑 Request cancelled")?;
                                app.cancel_request();
                                input_handler.draw()?;
                            } else {
                                // Show exit confirmation
                                custom_spinner.stop();
                                disable_raw_mode()?;
                                if menu.show_exit_confirmation(&mut output)? {
                                    break;
                                }
                                enable_raw_mode()?;
                                input_handler.draw()?;
                            }
                        } else if input == "__ESC__" {
                            // ESC pressed - cancel request if waiting
                            if app.is_waiting_for_response() {
                                custom_spinner.stop();
                                output.print_system("🛑 Request cancelled")?;
                                app.cancel_request();
                                input_handler.draw()?;
                            }
                        } else if input == "__CTRL_D__" {
                            // Ctrl+D - EOF
                            output.print_system("Goodbye! 👋")?;
                            break;
                        } else {
                            // Normal input submitted
                            let input = input.trim();

                            // Check for empty input
                            if input.is_empty() {
                                input_handler.draw()?;
                                continue;
                            }

                            // Add to history
                            input_handler.add_to_history(input.to_string());

                            // Check for special shortcuts
                            if input == "m" || input == "menu" {
                                custom_spinner.stop();
                                disable_raw_mode()?;
                                if menu.show_main_menu(&mut app, &mut output)? {
                                    break;
                                }
                                enable_raw_mode()?;
                                input_handler.draw()?;
                                continue;
                            }

                            // Check for exit commands
                            if input == "exit" || input == "quit" {
                                output.print_system("Goodbye! 👋")?;
                                break;
                            }

                            // Handle command
                            if input.starts_with('/') {
                                handle_cli_command(input, &mut app, &mut output, &mut menu).await?;
                                input_handler.draw()?;
                            } else {
                                // Check if we're already waiting for a response
                                if !app.is_waiting_for_response() {
                                    // Input handler added a newline, but we want minimal spacing
                                    // The AI response will start on the next line
                                    app.send_to_ai(input).await?;
                                    // Spinner will start in the response loop
                                } else {
                                    output.print_system(
                                        "⚠️  Already processing a request, please wait...",
                                    )?;
                                    input_handler.draw()?;
                                }
                            }
                        }
                    }
                    None => {
                        // Key handled, no input submitted yet
                    }
                }
            }
        }
    }

    // Save history
    let history_lines = input_handler.get_history().join("\n");
    let _ = std::fs::write(&history_path, history_lines);

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
            output.print_system(&format!(
                "API Key: {}",
                if config.ai.api_key.is_empty() {
                    "Not set"
                } else {
                    "Set"
                }
            ))?;
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
