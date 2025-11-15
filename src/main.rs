use anyhow::Result;
use clap::Parser;
use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType, EditMode};
use rustyline::history::DefaultHistory;

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

#[tokio::main]
async fn main() -> Result<()> {
    // Install color-eyre for better error reporting
    let _ = color_eyre::install();

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
    match app.initialize_api_client() {
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
    println!("{}", console::style("Tip: End line with \\ to continue on next line. Press Enter alone to send.").dim());
    println!();

    // Create rustyline editor with multi-line support
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .auto_add_history(true)
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
        // Process all pending AI responses before blocking on readline
        // Keep looping while we're waiting for a response
        while app.is_waiting_for_response() {
            match app.check_ai_response_nonblocking() {
                Some(response) => {
                    match response {
                        app::AiResponse::StreamStart => {
                            output.start_ai_message()?;
                        }
                        app::AiResponse::StreamChunk(chunk) => {
                            output.print_streaming_chunk(&chunk)?;
                        }
                        app::AiResponse::StreamEnd(_) => {
                            output.end_line()?;
                            // Execute tool calls if any from API response
                            if let Some(api_response) = app.get_pending_api_response() {
                                app.execute_tools_and_continue(&api_response).await?;
                            } else if let Some(tool_calls) = app.get_pending_tool_calls() {
                                app.execute_tools(tool_calls).await;
                            }
                            // Execute tool results if any
                            if let Some(tool_results) = app.get_pending_tool_results() {
                                for result in tool_results {
                                    if result.success {
                                        output.print_tool_call(&result.tool, "✅ Success")?;
                                    } else {
                                        output.print_tool_call(&result.tool, "❌ Failed")?;
                                    }
                                    output.print_tool_result(&result.output)?;
                                }

                                // For native OpenAI tool calls, we don't need to send results back to AI
                                // The tool results are already included in the response
                            }
                            // Execute legacy bash commands if any
                            if let Some(commands) = app.get_pending_bash_commands() {
                                for cmd in commands {
                                    output.print_system(&format!("Executing: {}", cmd))?;
                                    match app.execute_bash_command(&cmd).await {
                                        Ok(result) => {
                                            output.print_tool_result(&result)?;
                                        }
                                        Err(e) => {
                                            output.print_error(&format!("Command failed: {}", e))?;
                                        }
                                    }
                                }
                            }
                        }
                        app::AiResponse::Success { response, usage: _, tool_calls: _ } => {
                            output.print_ai_message(&response)?;
                        }
                        app::AiResponse::Error(error_msg) => {
                            output.print_error(&error_msg)?;
                        }
                        // New agent-based responses
                        app::AiResponse::AgentStreamStart => {
                            output.start_ai_message()?;
                        }
                        app::AiResponse::AgentStreamText(text) => {
                            output.print_streaming_chunk(&text)?;
                        }
                        app::AiResponse::AgentToolCall { id: _, name, arguments } => {
                            output.print_system(&format!("🔧 Tool call: {}({})", name, arguments))?;
                        }
                        app::AiResponse::AgentToolResult { tool_call_id, success, result } => {
                            let status = if success { "✅" } else { "❌" };
                            let result_text = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            output.print_system(&format!("{} Tool result: {}", status, tool_call_id))?;
                            output.print_tool_result(&result_text)?;
                        }
                        app::AiResponse::AgentStreamEnd => {
                            output.end_line()?;
                        }
                    }
                }
                None => {
                    // No response yet, wait a bit and try again
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            }
        }

        // Read user input with multi-line support
        let readline = read_multiline_input(&mut rl);
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
                    // Send to AI
                    app.send_to_ai(input).await?;
                    // Show loading indicator
                    output.print_system("⏳ Waiting for response...")?;
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

    Ok(())
}

fn read_multiline_input(rl: &mut Editor<(), DefaultHistory>) -> Result<String, ReadlineError> {
    use console::style;

    // Show prompt header
    println!("{}", style("┌─[You]").cyan().dim());

    let mut lines = Vec::new();
    let mut in_multiline = false;

    loop {
        // Use continuation prompt if we're in multi-line mode
        let prompt = if in_multiline {
            format!("{} ", style("│").cyan().dim())
        } else {
            format!("{} ", style("│").cyan().bold())
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                // If line ends with backslash, it's a continuation
                if line.trim_end().ends_with('\\') {
                    // Remove backslash and add line
                    let mut content = line.trim_end().to_string();
                    content.pop(); // Remove the backslash
                    lines.push(content);
                    in_multiline = true;
                    continue;
                }

                // Empty line behavior:
                // - If we're in multi-line mode (have content), finish
                // - If first line is empty, cancel
                if line.trim().is_empty() {
                    if in_multiline {
                        // Finish multi-line input
                        break;
                    } else {
                        // Empty first line - cancel
                        println!("{}", style("└─>").cyan().dim());
                        return Err(ReadlineError::Interrupted);
                    }
                }

                // Add current line and check if we should continue
                lines.push(line);

                // If not in multiline mode, this is a single line - finish
                if !in_multiline {
                    break;
                }
            }
            Err(e) => {
                println!("{}", style("└─>").cyan().dim());
                return Err(e);
            }
        }
    }

    println!("{}", style("└─>").cyan().dim());
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
