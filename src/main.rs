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
use serde_json::Value;

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
mod changelog;
mod chat;
mod colors;
mod config;
mod custom_spinner;
mod input_handler;
mod modern_input;
mod output;
mod overlay_menu;
mod tool_call;
mod tool_progress;
mod tools;

use app::App;
use output::OutputHandler;
use overlay_menu::OverlayMenu;
use tool_progress::PersistentInput;

/// Properly exit the application with terminal cleanup
fn graceful_exit() -> ! {
    // Restore terminal state before exiting
    let _ = cleanup_terminal_and_exit();
    std::process::exit(0);
}

/// Clean up terminal state and return success
fn cleanup_terminal_and_exit() -> Result<()> {
    // Order matters: disable raw mode first, then restore cursor and terminal settings

    // First disable raw mode to return to normal terminal operation
    let _ = disable_raw_mode();

    // Move to beginning of line and ensure we're on a clean line for the shell prompt
    let _ = execute!(
        std::io::stdout(),
        cursor::MoveToColumn(0),
        terminal::Clear(terminal::ClearType::CurrentLine)
    );

    // Reset terminal to default state (this restores colors and attributes)
    let _ = execute!(std::io::stdout(), crossterm::style::ResetColor);

    // Restore cursor visibility and style to default user shape
    let _ = execute!(
        std::io::stdout(),
        cursor::Show,
        SetCursorStyle::DefaultUserShape
    );

    // Print a newline to ensure clean shell prompt separation
    let _ = println!();

    // Force flush all commands to terminal
    let _ = std::io::stdout().flush();

    // Additional backup using console library for maximum compatibility
    let _ = console::Term::stdout().show_cursor();

    Ok(())
}

/// Guard to ensure cursor and terminal are properly restored when the program exits
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Use the same cleanup function as graceful_exit
        let _ = cleanup_terminal_and_exit();
    }
}

/// Set up panic hook to clean terminal and show panic messages properly
fn setup_panic_hook() {
    std::panic::set_hook(Box::new(move |panic_info| {
        // Clean up terminal state first
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            cursor::Show,
            crossterm::style::ResetColor,
            terminal::Clear(terminal::ClearType::CurrentLine)
        );
        let _ = println!(); // Ensure we're on a clean line

        // Print panic information
        eprintln!("\n🚨 ARULA CLI PANIC:");
        eprintln!("Location: {}", panic_info.location().unwrap());

        if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("Message: {}", message);
        } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
            eprintln!("Message: {}", message);
        } else {
            eprintln!("Message: <unknown>");
        }

        eprintln!("\nPlease report this issue with the above information.");
        eprintln!("Terminal state has been restored.\n");
    }));
}

/// Format tool result into human-readable text instead of raw JSON
fn format_tool_result(result: &Value) -> String {
    // Check if result has an "Ok" wrapper (common pattern)
    let empty_map = serde_json::Map::new();
    let result_clone = result.clone();
    let actual_result = if let Some(ok_obj) = result_clone.get("Ok").and_then(|v| v.as_object()) {
        ok_obj
    } else {
        result_clone.as_object().unwrap_or(&empty_map)
    };

    // Check if this is a file edit result with a diff
    if let (Some(success), Some(diff)) = (actual_result.get("success").and_then(|v| v.as_bool()),
                                                  actual_result.get("diff").and_then(|v| v.as_str())) {
        if success && !diff.is_empty() {
            return diff.to_string();
        }
    }

        // Check if this is a bash command result
        if let (Some(success), Some(stdout)) = (actual_result.get("success").and_then(|v| v.as_bool()),
                                                  actual_result.get("stdout").and_then(|v| v.as_str())) {
            if success {
                if let Some(stderr) = actual_result.get("stderr").and_then(|v| v.as_str()) {
                    if !stderr.is_empty() {
                        return format!("{}\nStderr:\n{}", stdout, stderr);
                    }
                }
                return stdout.to_string();
            }
        }

        // Check if this is a file read result
        if let (Some(success), Some(content)) = (actual_result.get("success").and_then(|v| v.as_bool()),
                                                    actual_result.get("content").and_then(|v| v.as_str())) {
            if success {
                return content.to_string();
            }
        }

        // Check if this is a web search result
        if let (Some(success), Some(results)) = (actual_result.get("success").and_then(|v| v.as_bool()),
                                                   actual_result.get("results").and_then(|v| v.as_array())) {
            if success {
                let mut output = String::new();
                for (i, result) in results.iter().enumerate() {
                    if let Some(title) = result.get("title").and_then(|v| v.as_str()) {
                        if let Some(url) = result.get("url").and_then(|v| v.as_str()) {
                            if let Some(snippet) = result.get("snippet").and_then(|v| v.as_str()) {
                                output.push_str(&format!("{}. {}\n   {}\n   {}\n\n",
                                    i + 1, title, url, snippet));
                            }
                        }
                    }
                }
                return output;
            }
        }

        // Check if this is a list directory result
        if let (Some(success), Some(files)) = (actual_result.get("success").and_then(|v| v.as_bool()),
                                                   actual_result.get("files").and_then(|v| v.as_array())) {
            if success {
                let mut output = String::new();
                for file in files {
                    if let Some(path) = file.as_str() {
                        output.push_str(&format!("{}\n", path));
                    }
                }
                return output;
            }
        }

        // Check for a message field for generic results
        if let Some(message) = actual_result.get("message").and_then(|v| v.as_str()) {
            if !message.is_empty() {
                return message.to_string();
            }
        }

        // Check for an error field
        if let Some(error) = actual_result.get("error").and_then(|v| v.as_str()) {
            if !error.is_empty() {
                return format!("Error: {}", error);
            }
        }

    // Fallback to pretty JSON if we can't format it specially
    serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install color-eyre for better error reporting
    let _ = color_eyre::install();

    // Set up panic hook to clean terminal and show errors
    setup_panic_hook();

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

    // Print real-time changelog
    print_changelog()?;
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

    // Create persistent input for typing during AI response
    let mut persistent_input = PersistentInput::new();
    let mut buffered_input = String::new(); // Input typed during AI response

    // Main event loop
    'main_loop: loop {
        // If AI is processing, check for responses and allow typing
        if app.is_waiting_for_response() {
            // Handle AI responses while allowing user input
            let _spinner_running = false;
            while app.is_waiting_for_response() {
                // Check for keyboard input (non-blocking)
                if event::poll(std::time::Duration::from_millis(10))? {
                    if let Event::Key(key_event) = event::read()? {
                        if key_event.kind == KeyEventKind::Press {
                            match key_event.code {
                                crossterm::event::KeyCode::Esc => {
                                    // ESC pressed, cancel AI request
                                    custom_spinner.stop();
                                    output.print_system("🛑 Request cancelled (ESC pressed)")?;
                                    app.cancel_request();
                                    break;
                                }
                                crossterm::event::KeyCode::Char(c) => {
                                    // Buffer input while AI is responding
                                    persistent_input.insert_char(c);
                                    persistent_input.render()?;
                                }
                                crossterm::event::KeyCode::Backspace => {
                                    persistent_input.backspace();
                                    persistent_input.render()?;
                                }
                                crossterm::event::KeyCode::Enter => {
                                    // Queue the input for processing after AI finishes
                                    if !persistent_input.get_input().is_empty() {
                                        buffered_input = persistent_input.take();
                                        // Clear the input line visually
                                        execute!(
                                            std::io::stdout(),
                                            cursor::MoveToColumn(0),
                                            terminal::Clear(terminal::ClearType::CurrentLine)
                                        )?;
                                        print!("{} ", console::style("▶").cyan());
                                        std::io::stdout().flush()?;
                                    }
                                }
                                crossterm::event::KeyCode::Left => {
                                    persistent_input.move_left();
                                    persistent_input.render()?;
                                }
                                crossterm::event::KeyCode::Right => {
                                    persistent_input.move_right();
                                    persistent_input.render()?;
                                }
                                _ => {}
                            }
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
                                // Move to beginning of current line and clear it (contains user's input)
                                execute!(
                                    std::io::stdout(),
                                    cursor::MoveToColumn(0),
                                    terminal::Clear(terminal::ClearType::CurrentLine)
                                )?;
                                std::io::stdout().flush()?;
                                // Start AI message output without prefix
                                output.start_ai_message()?;
                            }
                            app::AiResponse::AgentStreamText(text) => {
                                // Always stop spinner first if running
                                if custom_spinner.is_running() {
                                    custom_spinner.stop();
                                    // The spinner.stop() already clears its line, no need for additional clearing
                                    std::io::stdout().flush()?;
                                }

                                // Print the chunk without starting spinner immediately
                                // Note: print_streaming_chunk handles its own spinner logic
                                output.print_streaming_chunk(&text)?;
                            }
                            app::AiResponse::AgentToolCall {
                                id: _,
                                name,
                                arguments,
                            } => {
                                custom_spinner.stop();
                                // Clear only the current line (where spinner was)
                                execute!(
                                    std::io::stdout(),
                                    cursor::MoveToColumn(0),
                                    terminal::Clear(terminal::ClearType::CurrentLine)
                                )?;
                                output.start_tool_execution(&name, &arguments)?;
                                // Set up spinner above input prompt
                                print!("{} ", console::style("▶").cyan());
                                std::io::stdout().flush()?;
                                custom_spinner.start_above(&format!("Executing tool: {}", name))?;
                            }
                            app::AiResponse::AgentToolResult {
                                tool_call_id: _,
                                success,
                                result,
                            } => {
                                custom_spinner.stop();
                                // Clear only the current line (where spinner was)
                                execute!(
                                    std::io::stdout(),
                                    cursor::MoveToColumn(0),
                                    terminal::Clear(terminal::ClearType::CurrentLine)
                                )?;
                                let result_text = format_tool_result(&result);

                                // Check if this is a colored diff - if so, print it directly without box
                                if result_text.contains("\u{1b}[") &&
                                   (result_text.contains("\u{1b}[31m") || result_text.contains("\u{1b}[32m")) {
                                    // This is a colored diff, print directly
                                    println!("{}", result_text);
                                } else {
                                    // Regular tool result, use box formatting
                                    output.complete_tool_execution(&result_text, success)?;
                                }

                                // Restore spinner above input prompt
                                print!("{} ", console::style("▶").cyan());
                                std::io::stdout().flush()?;
                                custom_spinner.start_above("Processing results...")?;
                            }
                            app::AiResponse::AgentStreamEnd => {
                                // Stop spinner cleanly (it clears its own line)
                                custom_spinner.stop();
                                output.stop_spinner();

                                // Finish the AI message line
                                output.end_line()?;
                                output.print_context_usage(None)?;
                                
                                // Clear accumulated text to reset state for next response
                                output.clear_accumulated_text();

                                // Add exactly ONE blank line after AI response
                                println!();

                                // Transfer any typed input to the input handler
                                let typed_input = persistent_input.get_input().to_string();
                                persistent_input.clear();
                                if !typed_input.is_empty() {
                                    input_handler.set_input(&typed_input);
                                }

                                // Set up persistent input prompt for next message
                                print!("{} ", console::style("▶").cyan());
                                std::io::stdout().flush()?;

                                break; // Exit the AI response loop
                            }
                        }
                    }
                    None => {
                        // Start spinner immediately if not running
                        if !custom_spinner.is_running() {
                            custom_spinner.start_above("Generating response...")?;
                        }
                    }
                }
            }

            // Process buffered input if any
            if !buffered_input.is_empty() {
                let input = std::mem::take(&mut buffered_input);
                input_handler.add_to_history(input.clone());
                match app.send_to_ai(&input).await {
                    Ok(()) => {}
                    Err(e) => {
                        output.print_error(&format!("Failed to send to AI: {}", e))?;
                    }
                }
                continue 'main_loop;
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
                                        graceful_exit();
                                    }
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else if input == "__CTRL_D__" {
                                    // Ctrl+D - EOF (no message here, handled at end)
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
                                        graceful_exit();
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
                                            graceful_exit();
                                        }
                                        input_handler.clear()?;
                                        input_handler.draw()?;
                                        continue 'main_loop;
                                    }

                                    // Move to next line after user's input (which is already visible from input_handler)
                                    println!();

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

                                    // Clear input handler buffer (don't redraw, we'll set up our own layout)
                                    input_handler.set_input("");

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

    // Explicit cleanup before natural exit (in addition to TerminalGuard)
    output.print_system("Goodbye! 👋")?;

    // Ensure clean terminal state
    cleanup_terminal_and_exit()?;

    Ok(())
}

fn setup_bar_cursor() -> Result<()> {
    // Set cursor to blinking block cursor for better visibility
    // Some terminals don't support cursor style changes, so we handle it gracefully
    match std::io::stdout().execute(SetCursorStyle::BlinkingBlock) {
        Ok(_) => {
            if std::env::var("ARULA_DEBUG").is_ok() {
                println!("🔧 DEBUG: ✅ Cursor style set to blinking block");
            }
        }
        Err(e) => {
            if std::env::var("ARULA_DEBUG").is_ok() {
                println!("🔧 DEBUG: ⚠️ Could not set cursor style (this is normal): {}", e);
            }
            // Don't fail the whole application for cursor style issues
        }
    }
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
                graceful_exit();
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
                graceful_exit();
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

/// Print changelog from remote git or local file
fn print_changelog() -> Result<()> {
    use changelog::Changelog;

    // Fetch changelog (tries remote first, falls back to local)
    let changelog = Changelog::fetch_from_remote().unwrap_or_else(|_| {
        Changelog::fetch_local().unwrap_or_else(|_| Changelog::parse(&Changelog::default_changelog()))
    });

    // Detect actual build type from git
    let build_type = Changelog::detect_build_type();
    let type_label = match build_type {
        changelog::ChangelogType::Release => "📦 Release",
        changelog::ChangelogType::Custom => "🔧 Custom Build",
        changelog::ChangelogType::Development => "⚙️  Development",
    };

    // Print header
    println!(
        "{} {}",
        console::style("📋 What's New").cyan().bold(),
        console::style(format!("({})", type_label)).dim()
    );

    // Get recent changes (limit to 5)
    let changes = changelog.get_recent_changes(5);

    if changes.is_empty() {
        println!(
            "{}",
            console::style("  • No recent changes").dim()
        );
    } else {
        for change in changes {
            println!(
                "{}",
                console::style(format!("  {}", change)).dim()
            );
        }
    }

    Ok(())
}
