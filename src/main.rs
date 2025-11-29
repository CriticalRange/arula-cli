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
use ui::response_display::ResponseDisplay;
use ui::input_handler::InputHandler;
use ui::menus::main_menu::MainMenu;

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
    println!("💡 Type 'm' and press Enter to open menu, or 'exit'/'quit' to exit");

    // Initialize enhanced response display system
    let mut response_display = ResponseDisplay::new(OutputHandler::new());

    // Initialize input handler and menu system
    let mut input_handler = InputHandler::new("▶ ");
    let mut main_menu = MainMenu::new();

    // Enhanced input loop with menu support
    loop {
        // Use new input handler with menu detection
        match input_handler.read_input_with_menu_detection() {
            Ok(Some(input)) => {
                // Regular input received
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

                // Show user message and go directly to AI response
                output.print_user_message(&format!("You: {}", input))?;

                // Skip loading animation for more natural conversation flow
                // Let the AI response start immediately without artificial delays

                // Send to AI (simplified for now)
                println!("DEBUG: Sending to AI: {}", input);
                match app.send_to_ai(&input).await {
                    Ok(_) => {
                        println!("DEBUG: AI send successful, waiting for responses...");
                        // Process AI responses without separator for natural flow
                        // response_display.display_separator()?;

                        let mut response_count = 0;

                        // Continue polling for responses until stream ends
                        loop {
                            if let Some(response) = app.check_ai_response_nonblocking() {
                                response_count += 1;

                                // Enhanced debug logging - toggle with ARULA_DEBUG_RESPONSES=1
                                let debug_responses = std::env::var("ARULA_DEBUG_RESPONSES").unwrap_or_default() == "1";

                                let response_type = match &response {
                                    app::AiResponse::AgentStreamStart => "AgentStreamStart",
                                    app::AiResponse::AgentStreamText(chunk) => {
                                        if debug_responses {
                                            println!("DEBUG: AgentStreamText content: {:?}", chunk);
                                        }
                                        "AgentStreamText"
                                    }
                                    app::AiResponse::AgentToolCall { id, name, arguments } => {
                                        if debug_responses {
                                            println!("DEBUG: AgentToolCall - ID: {}, Name: {}, Args: {}", id, name, arguments);
                                        }
                                        "AgentToolCall"
                                    }
                                    app::AiResponse::AgentToolResult { tool_call_id, success, result } => {
                                        if debug_responses {
                                            println!("DEBUG: AgentToolResult - Tool: {}, Success: {}, Result: {:?}", tool_call_id, success, result);
                                        }
                                        "AgentToolResult"
                                    }
                                    app::AiResponse::AgentReasoningContent(reasoning) => {
                                        if debug_responses {
                                            println!("DEBUG: AgentReasoningContent: {:?}", reasoning);
                                        }
                                        "AgentReasoningContent"
                                    }
                                    app::AiResponse::AgentStreamEnd => "AgentStreamEnd",
                                };
                                println!("DEBUG: Got response #{} - Type: {}", response_count, response_type);
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
                            } else {
                                // No response available, sleep briefly and continue polling
                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }
                        }

                        println!("DEBUG: Total responses received: {}", response_count);
                        // Remove separator to maintain natural conversation flow
                        // response_display.display_separator()?;
                    }
                    Err(e) => {
                        println!("DEBUG: AI send error: {}", e);
                        output.print_error(&format!("❌ Error: {}", e))?;
                    }
                }
            }
            Ok(None) => {
                // Menu trigger detected (ESC twice or 'm')
                // Clear current line and show menu
                print!("\r");
                for _ in 0..80 {
                    print!(" ");
                }
                print!("\r");
                io::stdout().flush()?;

                // Show main menu
                match main_menu.show(&mut app, &mut output) {
                    Ok(_) => {
                        // Menu completed successfully
                    }
                    Err(e) => {
                        output.print_error(&format!("Menu error: {}", e))?;
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