#![allow(dead_code)]
#![allow(unreachable_code)]
#![allow(private_interfaces)]

use anyhow::Result;
use clap::Parser;
use std::io::{self, Write};

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

// Legacy input handling modules
mod input_handler;

// Re-export for easier imports
use app::App;
use ui::output::OutputHandler;
use ui::custom_spinner;
use ui::response_display::{ResponseDisplay, LoadingType};

fn graceful_exit() -> ! {
    std::process::exit(0);
}

fn cleanup_terminal_and_exit() -> Result<()> {
    Ok(())
}

fn graceful_exit_with_app(_app: &mut App) -> ! {
    graceful_exit();
}

fn show_exit_confirmation(_output: &mut OutputHandler) -> Result<bool> {
    Ok(true)
}

fn print_changelog() -> Result<()> {
    println!("📋 ARULA CLI - Custom Build");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
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

    // Initialize app components
    if let Err(e) = app.initialize_git_state().await {
        eprintln!("⚠️ Failed to initialize git state tracking: {}", e);
    }

    if let Err(e) = app.initialize_tool_registry().await {
        eprintln!("⚠️ Failed to initialize tool registry: {}", e);
    }

    match app.initialize_agent_client() {
        Ok(()) => {
            if cli.verbose {
                println!("✅ AI client initialized successfully");
            }
        }
        Err(e) => {
            if cli.verbose {
                println!("⚠️ AI client initialization failed: {}", e);
                println!("💡 You can configure AI settings in the application menu");
            }
        }
    }

    // Print banner
    output.print_banner()?;
    println!();

    // Print changelog
    print_changelog()?;
    println!();

    println!("✅ ARULA CLI started successfully!");
    println!("📝 Type your message and press Enter to send to AI");
    println!("🔄 You can now type while AI is responding - concurrent input enabled!");
    println!("💡 Type 'exit' or 'quit' to exit");

    // Initialize enhanced response display system
    let mut response_display = ResponseDisplay::new(OutputHandler::new());

    // Simple enhanced input loop for now
    loop {
        print!("▶ ");
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF
                break;
            }
            Ok(_) => {
                let input = input.trim();

                if input.is_empty() {
                    continue;
                }

                if input == "exit" || input == "quit" {
                    if show_exit_confirmation(&mut output)? {
                        output.print_system("Goodbye! 👋")?;
                        graceful_exit();
                    }
                    continue;
                }

                if input.starts_with('/') {
                    output.print_system(&format!("🔧 Command '{}' recognized but not implemented yet", input))?;
                    continue;
                }

                // Show enhanced loading animation
                output.print_user_message(&format!("You: {}", input))?;

                // Display thinking loading animation
                let _ = response_display.display_loading_animation(
                    LoadingType::Thinking,
                    "AI is thinking..."
                );

                // Send to AI (simplified for now)
                match app.send_to_ai(input).await {
                    Ok(_) => {
                        // Process AI responses with enhanced display
                        response_display.display_separator()?;

                        while let Some(response) = app.check_ai_response_nonblocking() {
                            match response {
                                app::AiResponse::AgentStreamStart => {
                                    // Finalize any pending thinking content before starting stream
                                    let _ = response_display.finalize_thinking_content();
                                    output.start_ai_message()?;
                                }
                                app::AiResponse::AgentStreamText(chunk) => {
                                    let _ = response_display.display_stream_text(&chunk);
                                }
                                app::AiResponse::AgentToolCall { id, name, arguments } => {
                                    // Finalize any pending thinking content before showing tool call
                                    let _ = response_display.finalize_thinking_content();
                                    let _ = response_display.display_tool_call_start(&id, &name, &arguments);
                                }
                                app::AiResponse::AgentToolResult { tool_call_id, success, result } => {
                                    // Create a mock ToolResult for display
                                    let tool_result = crate::api::agent::ToolResult {
                                        success,
                                        data: result.clone(),
                                        error: None,
                                    };
                                    let _ = response_display.display_tool_result(&tool_call_id, "Tool", &tool_result);
                                }
                                app::AiResponse::AgentReasoningContent(reasoning) => {
                                    let _ = response_display.display_thinking_content(&reasoning);
                                }
                                app::AiResponse::AgentStreamEnd => {
                                    // Finalize thinking content before ending
                                    let _ = response_display.finalize_thinking_content();
                                    output.end_line()?;
                                    break;
                                }
                                _ => {}
                            }
                        }

                        response_display.display_separator()?;
                    }
                    Err(e) => {
                        output.print_error(&format!("❌ Error: {}", e))?;
                    }
                }
            }
            Err(e) => {
                output.print_error(&format!("Input error: {}", e))?;
            }
        }
    }

    graceful_exit()
}