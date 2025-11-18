#![allow(dead_code)]
#![allow(unreachable_code)]
#![allow(private_interfaces)]

use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::{self, SetCursorStyle},
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{self, enable_raw_mode, disable_raw_mode},
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
mod modern_input;
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
        // Properly restore terminal state on exit
        // Order matters: disable raw mode first, then restore cursor

        // First disable raw mode to return to normal terminal operation
        let _ = disable_raw_mode();

        // Reset terminal to default state (this restores colors and attributes)
        let _ = execute!(std::io::stdout(), crossterm::style::ResetColor);

        // Restore cursor visibility and style
        let _ = execute!(
            std::io::stdout(),
            cursor::Show,
            SetCursorStyle::DefaultUserShape
        );

        // Move cursor to beginning of line for clean shell prompt
        let _ = execute!(std::io::stdout(), cursor::MoveToColumn(0));

        // Force flush all commands to terminal
        let _ = std::io::stdout().flush();

        // Additional backup using console library for maximum compatibility
        let _ = console::Term::stdout().show_cursor();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install color-eyre for better error reporting
    let _ = color_eyre::install();

    // Show cursor initially
    let _ = console::Term::stdout().show_cursor();

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
        console::style("  • Cursor changed to blinking block for better visibility").dim()
    );
    println!();

    // NOW enable raw mode for keyboard input detection
    enable_raw_mode()?;

    // Set cursor to be visible and use steady bar style
    setup_bar_cursor()?;

    // Create input handler with prompt
    let prompt = if cfg!(windows) { "▶" } else { "▶" };
    let mut input_handler = input_handler::InputHandler::new(prompt);
    let mut custom_spinner = custom_spinner::CustomSpinner::new();

    // Create overlay menu
    let mut menu = OverlayMenu::new();

    // Main event loop
    'main_loop: loop {
        // If AI is processing, check for responses and allow cancellation
        if app.is_waiting_for_response() {
            // Handle AI responses and cancellation
            let _spinner_running = false;
            while app.is_waiting_for_response() {
                // Check for ESC to cancel (non-blocking check)
                if event::poll(std::time::Duration::from_millis(10))? {
                    if let Event::Key(key_event) = event::read()? {
                        if key_event.kind == KeyEventKind::Press && key_event.code == crossterm::event::KeyCode::Esc {
                            // ESC pressed, cancel AI request
                            custom_spinner.stop();
                            output.print_system("🛑 Request cancelled (ESC pressed)")?;
                            app.cancel_request();
                            break;
                        }
                    }
                }

                match app.check_ai_response_nonblocking() {
                    Some(response) => {
                        match response {
                            app::AiResponse::AgentStreamStart => {
                                if custom_spinner.is_running() {
                                    custom_spinner.stop();
                                }
                                custom_spinner.start("")?;
                                output.start_ai_message()?;
                            }
                            app::AiResponse::AgentStreamText(text) => {
                                let is_first_chunk = custom_spinner.is_running();
                                if is_first_chunk {
                                    custom_spinner.stop();
                                    execute!(
                                        std::io::stdout(),
                                        cursor::MoveToColumn(0),
                                        terminal::Clear(terminal::ClearType::CurrentLine)
                                    )?;
                                    print!(" ");
                                    std::io::stdout().flush()?;
                                    output.print_streaming_chunk(&text)?;
                                } else {
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
                                custom_spinner.stop();
                                output.stop_spinner();
                                output.end_line()?;
                                output.print_context_usage(None)?;
                                println!();
                                break; // Exit the AI response loop
                            }
                        }
                    }
                    None => {
                        if !custom_spinner.is_running() {
                            custom_spinner.start("")?;
                        }
                    }
                }
            }
            continue; // Continue to next iteration to get input
        }

        // Draw initial prompt
        input_handler.draw()?;

        // Input handling loop
        loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        match input_handler.handle_key(key_event)? {
                            Some(input) => {
                                // Handle special commands
                                if input == "__CTRL_C__" {
                                    // Ctrl+C pressed - show exit confirmation
                                    if menu.show_exit_confirmation(&mut output)? {
                                        output.print_system("Goodbye! 👋")?;
                                        std::process::exit(0);
                                    }
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else if input == "__CTRL_D__" {
                                    // Ctrl+D - EOF
                                    output.print_system("Goodbye! 👋")?;
                                    break;
                                } else if input == "__ESC__" {
                                    // ESC pressed, continue
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else if input == "m" || input == "menu" {
                                    // Menu shortcut
                                    if menu.show_main_menu(&mut app, &mut output)? {
                                        output.print_system("Goodbye! 👋")?;
                                        std::process::exit(0);
                                    }
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else if input.starts_with('/') {
                                    // Handle CLI commands
                                    handle_cli_command(&input, &mut app, &mut output, &mut menu).await?;
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else {
                                    // Handle empty input
                                    if input.trim().is_empty() {
                                        input_handler.clear()?;
                                        input_handler.draw()?;
                                        continue;
                                    }

                                    // Add to history
                                    input_handler.add_to_history(input.clone());

                                    // Handle exit commands
                                    if input == "exit" || input == "quit" {
                                        if menu.show_exit_confirmation(&mut output)? {
                                            output.print_system("Goodbye! 👋")?;
                                            std::process::exit(0);
                                        }
                                        input_handler.clear()?;
                                        input_handler.draw()?;
                                        continue 'main_loop;
                                    }

                                    // Send to AI
                                    if cli.verbose {
                                        output.print_system(&format!("DEBUG: About to call app.send_to_ai with input: '{}'", input))?;
                                    }
                                    match app.send_to_ai(&input).await {
                                        Ok(()) => {
                                            // AI request sent successfully
                                            if cli.verbose {
                                                output.print_system("DEBUG: AI request sent successfully")?;
                                            }
                                        }
                                        Err(e) => {
                                            // Handle AI client errors gracefully
                                            if cli.verbose {
                                                output.print_system(&format!("DEBUG: AI send failed with error: {}", e))?;
                                            }
                                            if e.to_string().contains("AI client not initialized") {
                                                output.print_error("AI client not configured. Use /config to set up AI settings.")?;
                                                output.print_system("💡 Try: /config or press 'm' for the configuration menu")?;
                                            } else {
                                                output.print_error(&format!("Failed to send to AI: {}", e))?;
                                            }
                                        }
                                    }

                                    // Clear input after sending
                                    input_handler.clear()?;
                                    break; // Exit input loop to go to AI response handling
                                }
                            }
                            None => {
                                // Continue handling input
                                input_handler.draw()?;
                            }
                        }
                    }
                }
            } else {
                // No event, continue
                continue;
            }
        }
    }

    // Cursor will be automatically shown by CursorGuard's Drop implementation

    Ok(())
}

fn setup_bar_cursor() -> Result<()> {
    // Set cursor to blinking block cursor for better visibility
    std::io::stdout().execute(SetCursorStyle::BlinkingBlock)?;
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
                output.print_system("Goodbye! 👋")?;
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
            // Open configuration menu directly
            if menu.show_config_menu(app, output)? {
                // Exit requested
                output.print_system("Goodbye! 👋")?;
                std::process::exit(0);
            }
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
