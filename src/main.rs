#![allow(dead_code)]
#![allow(unreachable_code)]
#![allow(private_interfaces)]

use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::{self, SetCursorStyle},
    event::{self, Event, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, enable_raw_mode, disable_raw_mode},
    ExecutableCommand,
    QueueableCommand,
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

// Module declarations for the organized folder structure
mod api;
mod app;
mod tools;
mod ui;
mod utils;

// Re-export for easier imports
use app::App;
use ui::output::OutputHandler;
use ui::menus::{MainMenu, ConfigMenu};
use ui::reedline_input::{ReedlineInput, AiState};
use ui::custom_spinner;

/// Compatibility wrapper for new menu system
fn show_main_menu(main_menu: &mut MainMenu, app: &mut App, output: &mut OutputHandler, config_menu: &mut ConfigMenu) -> Result<bool> {
    match main_menu.show(app, output)? {
        ui::menus::common::MenuResult::Exit => Ok(true),
        ui::menus::common::MenuResult::ClearChat => {
            app.clear_conversation();
            output.print_system("Conversation cleared")?;
            Ok(false)
        }
        ui::menus::common::MenuResult::BackToMain => {
            // This is the Settings selection - show config menu
            show_config_menu(config_menu, app, output)?;
            Ok(false)
        }
        ui::menus::common::MenuResult::Continue => Ok(false),
        _ => Ok(false),
    }
}

/// Compatibility wrapper for config menu
fn show_config_menu(config_menu: &mut ConfigMenu, app: &mut App, output: &mut OutputHandler) -> Result<bool> {
    match config_menu.show(app, output)? {
        ui::menus::common::MenuResult::Exit => Ok(true),
        _ => Ok(false), // BackToMain, ConfigurationUpdated -> don't exit
    }
}

/// Original exit confirmation menu (Stay/Exit)
fn show_exit_confirmation(output: &mut OutputHandler) -> Result<bool> {
    use crossterm::{
        event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
        terminal,
        cursor::{Hide, MoveTo},
        style::{SetForegroundColor, SetBackgroundColor, ResetColor, Print, Color},
        ExecutableCommand,
    };
    use crate::utils::colors::{ColorTheme, helpers};
    use std::io::stdout;
    use std::time::Duration;

    let (cols, rows) = terminal::size()?;

    // Ensure minimum space for menu
    if cols < 30 || rows < 8 {
        output.print_system("Terminal too small for exit menu")?;
        return Ok(false);
    }

    // Setup terminal for exit menu
    terminal::enable_raw_mode()?;
    stdout().execute(terminal::Clear(terminal::ClearType::All))?;
    stdout().execute(MoveTo(0, 0))?;
    stdout().flush()?;

    let mut selected_index = 0; // 0 = Stay, 1 = Exit
    let options = vec![
        "Stay in ARULA CLI".to_string(),
        "Exit ARULA CLI".to_string(),
    ];

    loop {
        // Clear screen
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;

        let menu_width = 40.min(cols.saturating_sub(4));
        let menu_height = 8;
        let start_x = (cols - menu_width) / 2;
        let start_y = (rows - menu_height) / 2;

        // Draw modern box
        let top_left = "╭";
        let top_right = "╮";
        let bottom_left = "╰";
        let bottom_right = "╯";
        let horizontal = "─";
        let vertical = "│";

        // Top border
        stdout().execute(MoveTo(start_x, start_y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(top_left))?;
        for _ in 1..(menu_width - 1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(top_right))?;
        stdout().queue(ResetColor)?;

        // Title line
        stdout().execute(MoveTo(start_x, start_y + 1))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(vertical))?;
        let title = " Exit Confirmation ";
        let title_x = start_x + 1;
        for _ in 0..title.len().min(menu_width as usize - 2) {
            stdout().queue(Print(" "))?;
        }
        stdout().execute(MoveTo(title_x, start_y + 1))?
              .queue(Print(ColorTheme::primary().bold().apply_to(title)))?;
        stdout().execute(MoveTo(start_x + menu_width - 1, start_y + 1))?
              .queue(Print(vertical))?;
        stdout().queue(ResetColor)?;

        // Empty line
        stdout().execute(MoveTo(start_x, start_y + 2))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(vertical))?;
        for _ in 0..(menu_width - 2) {
            stdout().queue(Print(" "))?;
        }
        stdout().queue(Print(vertical))?;
        stdout().queue(ResetColor)?;

        // Options
        for (i, option) in options.iter().enumerate() {
            let y = start_y + 3 + i as u16;
            stdout().execute(MoveTo(start_x, y))?
                  .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
                  .queue(Print(vertical))?;

            if i == selected_index {
                // Selected item with background
                stdout().execute(MoveTo(start_x + 1, y))?;
                for _ in 0..(menu_width - 2) {
                    stdout().queue(SetBackgroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::BACKGROUND_ANSI)))?
                          .queue(Print(" "))?;
                }
                stdout().queue(ResetColor)?;

                let display_text = format!("▶ {}", option);
                stdout().execute(MoveTo(start_x + 3, y))?
                      .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::PRIMARY_ANSI)))?
                      .queue(Print(display_text))?
                      .queue(ResetColor)?;
            } else {
                // Unselected item
                for _ in 0..(menu_width - 2) {
                    stdout().queue(Print(" "))?;
                }
                stdout().execute(MoveTo(start_x + 3, y))?
                      .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI)))?
                      .queue(Print(option))?
                      .queue(ResetColor)?;
            }

            stdout().execute(MoveTo(start_x + menu_width - 1, y))?
                  .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
                  .queue(Print(vertical))?
                  .queue(ResetColor)?;
        }

        // Bottom border
        stdout().execute(MoveTo(start_x, start_y + 5))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(bottom_left))?;
        for _ in 1..(menu_width - 1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(bottom_right))?;
        stdout().queue(ResetColor)?;

        // Help text
        let help_text = "↑↓ Navigate • Enter Select • ESC Cancel";
        let help_x = start_x + (menu_width.saturating_sub(help_text.len() as u16)) / 2;
        stdout().execute(MoveTo(help_x, start_y + 6))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(help_text))?
              .queue(ResetColor)?;

        stdout().flush()?;

        // Handle input
        match event::read()? {
            Event::Key(key_event) => {
                if key_event.kind != KeyEventKind::Press {
                    continue;
                }

                match key_event.code {
                    KeyCode::Up => {
                        selected_index = if selected_index == 0 { options.len() - 1 } else { selected_index - 1 };
                    }
                    KeyCode::Down => {
                        selected_index = (selected_index + 1) % options.len();
                    }
                    KeyCode::Left => {
                        selected_index = if selected_index == 0 { options.len() - 1 } else { selected_index - 1 };
                    }
                    KeyCode::Right => {
                        selected_index = (selected_index + 1) % options.len();
                    }
                    KeyCode::Enter => {
                        // Restore terminal and return result
                        terminal::disable_raw_mode()?;
                        stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                        stdout().execute(MoveTo(0, 0))?;
                        stdout().flush()?;
                        return Ok(selected_index == 1); // true if Exit selected
                    }
                    KeyCode::Esc => {
                        // Restore terminal and return false (stay)
                        terminal::disable_raw_mode()?;
                        stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                        stdout().execute(MoveTo(0, 0))?;
                        stdout().flush()?;
                        return Ok(false);
                    }
                    KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                        // Ctrl+C in exit confirmation menu - exit immediately
                        terminal::disable_raw_mode()?;
                        stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                        stdout().execute(MoveTo(0, 0))?;
                        stdout().flush()?;
                        return Ok(true); // Exit immediately
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

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

    // Create reedline input handler (no raw mode needed - reedline handles it)
    let mut reedline_input = ReedlineInput::new()?;
    let mut custom_spinner = custom_spinner::CustomSpinner::new();

    // Create menus using the new modular system
    let mut main_menu = MainMenu::new();
    let mut config_menu = ConfigMenu::new();

    // Session ID for prompt
    let session_id = format!("{:x}", fastrand::u32(..));
    reedline_input.set_session_id(session_id);

    // Set initial AI state
    reedline_input.set_ai_state(AiState::Ready);

    // Main event loop
    'main_loop: loop {
        // If AI is processing, handle responses
        if app.is_waiting_for_response() {
            // Update prompt to show waiting state
            reedline_input.set_ai_state(AiState::Waiting);

            while app.is_waiting_for_response() {
                // Poll for AI responses (non-blocking with short timeout)
                std::thread::sleep(std::time::Duration::from_millis(10));

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
                                // Stop spinner cleanly
                                custom_spinner.stop();
                                output.stop_spinner();

                                // Finish the AI message line
                                output.end_line()?;
                                output.print_context_usage(None)?;

                                // Clear accumulated text to reset state for next response
                                output.clear_accumulated_text();

                                // Add blank line after AI response
                                println!();

                                // Reset AI state to ready for next input
                                reedline_input.set_ai_state(AiState::Ready);

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

            continue; // Continue to next iteration to get input
        }

        // Update AI state in prompt
        reedline_input.set_ai_state(AiState::Ready);

        // Get input from reedline
        match reedline_input.read_line()? {
            Some(input) => {
                // Handle special signals from reedline
                if input == "__ESC__" {
                    // Single ESC - cancel AI request if running, otherwise just clear
                    if app.is_waiting_for_response() {
                        custom_spinner.stop();
                        output.print_system("🛑 Request cancelled (ESC pressed)")?;
                        app.cancel_request();
                    }
                    continue 'main_loop;
                }

                if input == "__SHOW_MENU__" {
                    // Double ESC - show menu
                    if show_main_menu(&mut main_menu, &mut app, &mut output, &mut config_menu)? {
                        output.print_system("Goodbye! 👋")?;
                        graceful_exit();
                    }
                    continue 'main_loop;
                }

                if input == "__SHOW_EXIT_MENU__" {
                    // Ctrl+C - show exit menu
                    if show_exit_confirmation(&mut output)? {
                        output.print_system("Goodbye! 👋")?;
                        graceful_exit();
                    }
                    continue 'main_loop;
                }

                // Handle Ctrl+D (EOF)
                if input.is_empty() {
                    output.print_system("Goodbye! 👋")?;
                    break 'main_loop;
                }

                // Handle exit commands
                if input == "exit" || input == "quit" {
                    if show_exit_confirmation(&mut output)? {
                        output.print_system("Goodbye! 👋")?;
                        graceful_exit();
                    }
                    continue 'main_loop;
                }

                // Handle menu shortcuts
                if input == "m" || input == "menu" {
                    if show_main_menu(&mut main_menu, &mut app, &mut output, &mut config_menu)? {
                        output.print_system("Goodbye! 👋")?;
                        graceful_exit();
                    }
                    continue 'main_loop;
                }

                // Handle CLI commands (starting with /)
                if input.starts_with('/') {
                    handle_cli_command(&input, &mut app, &mut output, &mut main_menu, &mut config_menu).await?;
                    continue 'main_loop;
                }

                // Move to next line after user's input
                println!();

                // Update prompt to "thinking" state
                reedline_input.set_ai_state(AiState::Thinking);

                // Send to AI
                if cli.verbose {
                    output.print_system(&format!("DEBUG: Sending to AI: '{}'", input))?;
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
            }
            None => {
                // Ctrl+D - exit gracefully
                output.print_system("Goodbye! 👋")?;
                break 'main_loop;
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
    main_menu: &mut MainMenu,
    config_menu: &mut ConfigMenu,
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
            if show_main_menu(main_menu, app, output, config_menu)? {
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
            if show_config_menu(config_menu, app, output)? {
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
    use utils::changelog::Changelog;

    // Fetch changelog (tries remote first, falls back to local)
    let changelog = Changelog::fetch_from_remote().unwrap_or_else(|_| {
        Changelog::fetch_local().unwrap_or_else(|_| Changelog::parse(&Changelog::default_changelog()))
    });

    // Detect actual build type from git
    let build_type = Changelog::detect_build_type();
    let type_label = match build_type {
        utils::changelog::ChangelogType::Release => "📦 Release",
        utils::changelog::ChangelogType::Custom => "🔧 Custom Build",
        utils::changelog::ChangelogType::Development => "⚙️  Development",
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
