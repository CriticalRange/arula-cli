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

use arula_cli::ui::menus::main_menu::MainMenu;
use arula_cli::ui::output::OutputHandler;
use arula_cli::ui::response_display::ResponseDisplay;
use arula_cli::ui::ThinkingWidget;
use arula_core::api::agent::ToolResult;
use arula_core::app::AiResponse;
use arula_core::utils::changelog;
use arula_core::App;

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

/// Print changelog from remote git or local file
fn print_changelog() -> Result<()> {
    use changelog::Changelog;

    // Fetch changelog (tries remote first, falls back to local)
    let changelog = Changelog::fetch_from_remote().unwrap_or_else(|_| {
        Changelog::fetch_local()
            .unwrap_or_else(|_| Changelog::parse(&Changelog::default_changelog()))
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
        println!("{}", console::style("  • No recent changes").dim());
    } else {
        for change in changes {
            println!("{}", console::style(format!("  {}", change)).dim());
        }
    }

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

    // Initialize global logger
    if let Err(e) = arula_core::utils::logger::init_global_logger() {
        eprintln!("⚠️ Failed to initialize logger: {}", e);
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

    // Initialize response display
    let mut response_display = ResponseDisplay::new(OutputHandler::new());
    let mut main_menu = MainMenu::new();

    // Simple blocking input loop - no raw mode needed
    loop {
        // Print prompt
        print!("▶ ");
        io::stdout().flush()?;

        // Read line from stdin (blocking)
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF (Ctrl+D)
                println!();
                output.print_system("Goodbye! 👋")?;
                graceful_exit();
            }
            Ok(_) => {
                let input = input.trim().to_string();

                if input.is_empty() {
                    continue;
                }

                // Handle special commands
                if input == "exit" || input == "quit" {
                    output.print_system("Goodbye! 👋")?;
                    graceful_exit();
                }

                if input == "m" || input == "menu" {
                    match main_menu.show(&mut app, &mut output) {
                        Ok(_) => {}
                        Err(e) => output.print_error(&format!("Menu error: {}", e))?,
                    }
                    continue;
                }

                // Process AI request
                process_ai_request(&input, &mut app, &mut output, &mut response_display).await?;

                // Newline after AI response
                println!();
            }
            Err(e) => {
                output.print_error(&format!("Input error: {}", e))?;
            }
        }
    }
}

/// Process an AI request and stream the response
async fn process_ai_request(
    input: &str,
    app: &mut App,
    output: &mut OutputHandler,
    response_display: &mut ResponseDisplay,
) -> Result<()> {
    match app.send_to_ai(input).await {
        Ok(_) => {
            let mut response_text = String::new();
            let mut stream_started = false;
            let mut thinking_widget = ThinkingWidget::new();
            // Track tool calls by ID to get the tool name for results
            let mut pending_tools: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            // Poll for AI responses
            loop {
                // Update thinking animation if active
                if thinking_widget.is_active() {
                    let _ = thinking_widget.pulse();
                }

                if let Some(response) = app.check_ai_response_nonblocking() {
                    match response {
                        AiResponse::AgentStreamStart => {
                            response_text.clear();
                            stream_started = true;
                            // Initialize the markdown streamer for new response
                            let _ = output.start_ai_stream();
                        }
                        AiResponse::AgentThinkingStart => {
                            // Start the thinking widget with pulsing animation
                            let _ = thinking_widget.start();
                        }
                        AiResponse::AgentThinkingContent(content) => {
                            // Add content to thinking widget
                            let _ = thinking_widget.add_content(&content);
                        }
                        AiResponse::AgentThinkingEnd => {
                            // Finish thinking and display the thought
                            let _ = thinking_widget.finish();
                        }
                        AiResponse::AgentStreamText(chunk) => {
                            // If thinking is still active, finish it first
                            if thinking_widget.is_active() {
                                let _ = thinking_widget.finish();
                            }
                            response_text.push_str(&chunk);
                            // Use stream_chunk for proper markdown rendering
                            let _ = output.stream_chunk(&chunk);
                        }
                        AiResponse::AgentToolCall {
                            id,
                            name,
                            arguments,
                        } => {
                            // Finish thinking if active
                            if thinking_widget.is_active() {
                                let _ = thinking_widget.finish();
                            }
                            if stream_started && !response_text.is_empty() {
                                let _ = output.finalize_stream();
                                response_text.clear();
                            }
                            // Store tool name for when we get the result
                            pending_tools.insert(id.clone(), name.clone());
                            let _ =
                                response_display.display_tool_call_start(&id, &name, &arguments);
                        }
                        AiResponse::AgentToolResult {
                            tool_call_id,
                            success,
                            result,
                        } => {
                            // Get the actual tool name from our tracking map
                            let tool_name = pending_tools
                                .remove(&tool_call_id)
                                .unwrap_or_else(|| "Tool".to_string());
                            let tool_result = ToolResult {
                                success,
                                data: result.clone(),
                                error: None,
                            };
                            let _ = response_display.display_tool_result(
                                &tool_call_id,
                                &tool_name,
                                &tool_result,
                            );
                        }
                        AiResponse::AgentReasoningContent(content) => {
                            // Legacy reasoning content - show in thinking widget
                            if !thinking_widget.is_active() {
                                let _ = thinking_widget.start();
                            }
                            let _ = thinking_widget.add_content(&content);
                        }
                        AiResponse::AgentStreamEnd => {
                            // Finish thinking if still active
                            if thinking_widget.is_active() {
                                let _ = thinking_widget.finish();
                            }
                            if stream_started {
                                let _ = output.finalize_stream();
                            }
                            break;
                        }
                    }
                } else {
                    // Yield to allow other async tasks to run
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        }
        Err(e) => {
            output.print_error(&format!("❌ Error: {}", e))?;
            println!();
        }
    }
    Ok(())
}
