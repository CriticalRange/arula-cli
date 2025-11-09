use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, EnableMouseCapture, DisableMouseCapture, Event, KeyCode, KeyModifiers, KeyEventKind, MouseEventKind},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io::{self, stdout, IsTerminal};
use std::time::{Duration, Instant};

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
mod art;
mod config;
mod ui_components;
mod layout;
mod api;
mod cli_commands;
mod progress;
mod conversation;
mod tool_call;
mod widgets;

use app::App;
use layout::Layout;
use ui_components::Theme;


#[tokio::main]
async fn main() -> Result<()> {
    // Install color-eyre for better error reporting (optional)
    let _ = color_eyre::install();

    let cli = Cli::parse();

    if cli.verbose {
        println!("🚀 Starting ARULA CLI with endpoint: {}", cli.endpoint);
    }

    // Check if we're in a proper terminal
    if !stdout().is_terminal() {
        eprintln!("⚠️  Terminal Error: ARULA CLI requires a proper terminal environment to run.");
        eprintln!();
        eprintln!("This application needs:");
        eprintln!("• A real terminal (not a pipe or redirected output)");
        eprintln!("• Interactive terminal support");
        eprintln!("• Proper TTY capabilities");
        eprintln!();
        eprintln!("For Termux users:");
        eprintln!("  export TERM=xterm-256color");
        eprintln!("  pkg install xterm-repo && pkg install xterm");
        eprintln!();
        eprintln!("To run ARULA CLI:");
        eprintln!("  cargo run                    # In a real terminal");
        eprintln!("  ./target/release/arula-cli   # After building release");
        eprintln!();
        eprintln!("❌ Cannot continue without proper terminal support.");
        std::process::exit(1);
    }

    // Initialize terminal with modern ratatui approach
    let mut terminal = ratatui::init();
    terminal.clear()?;

    // Enable mouse capture for scroll wheel support
    crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;

    // Create app and layout
    let mut app = App::new()?;

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

    let mut layout = Layout::default();
layout.set_theme(Theme::Cyberpunk);

    // Run app with proper cleanup
    let res = run_app(&mut terminal, &mut app, &mut layout).await;

    // Always cleanup using modern ratatui approach, even if an error occurred
    let _ = ratatui::restore();

    // Disable mouse capture
    let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    layout: &mut Layout,
) -> Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(100); // Faster updates for smoother animation

    loop {
        // Check for AI responses (non-blocking)
        app.check_ai_response();

        // Auto-scroll to bottom when flag is set (new messages or streaming updates)
        if app.should_scroll_to_bottom {
            layout.scroll_to_bottom();
            app.should_scroll_to_bottom = false;  // Reset flag after scrolling
        }

        // Execute bash commands from AI response when flag is set
        if app.should_execute_bash {
            app.should_execute_bash = false;  // Reset flag first
            app.execute_ai_bash_commands().await;
        }

        // Draw UI
        let messages = app.messages.clone();
        terminal.draw(|f| layout.render(f, app, &messages))?;

        
        // Handle events with shorter timeout for better responsiveness
        let timeout = Duration::from_millis(50); // Very responsive to input

        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    // Only handle key press events, ignore key release
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Handle Ctrl+C for exit
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        // Check if already in exit confirmation
                        if matches!(app.state, crate::app::AppState::Menu(crate::app::MenuType::ExitConfirmation)) {
                            // Graceful exit: first clear popup and textarea, then exit
                            app.state = crate::app::AppState::Chat; // Clear menu popup
                            app.show_input = false; // Hide input textarea
                            let messages = app.messages.clone();
                            terminal.draw(|f| layout.render(f, app, &messages))?; // Render chat without popup/textarea
                            std::thread::sleep(Duration::from_millis(100)); // Brief pause for visual feedback
                            app.state = crate::app::AppState::Exiting;
                            return Ok(());
                        } else {
                            // Show exit confirmation
                            app.state = crate::app::AppState::Menu(crate::app::MenuType::ExitConfirmation);
                            app.menu_selected = 0;
                        }
                        continue;
                    }

                    // Check if we're in menu mode
                    if matches!(app.state, crate::app::AppState::Menu(_)) {
                        app.handle_menu_navigation(key);
                    } else {
                        match key.code {
                            KeyCode::Esc => {
                                // Open main menu
                                app.state = crate::app::AppState::Menu(crate::app::MenuType::Main);
                            }
                            KeyCode::PageUp => {
                                // Scroll chat up
                                for _ in 0..5 {
                                    layout.scroll_state.scroll_up();
                                }
                            }
                            KeyCode::PageDown => {
                                // Scroll chat down
                                for _ in 0..5 {
                                    layout.scroll_state.scroll_down();
                                }
                            }
                            KeyCode::Up => {
                                // If Ctrl is held, scroll chat up
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    layout.scroll_state.scroll_up();
                                } else {
                                    app.handle_key_event(key);
                                }
                            }
                            KeyCode::Down => {
                                // If Ctrl is held, scroll chat down
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    layout.scroll_state.scroll_down();
                                } else {
                                    app.handle_key_event(key);
                                }
                            }
                            KeyCode::Home => {
                                // Scroll to top
                                layout.scroll_state.scroll_to_top();
                            }
                            KeyCode::End => {
                                // Scroll to bottom
                                layout.scroll_state.scroll_to_bottom();
                            }
                            _ => {
                                app.handle_key_event(key);
                            }
                        }
                    }
                }
                Event::FocusGained => {
                    // Terminal gained focus - enable input mode
                    if app.state == crate::app::AppState::Chat {
                        app.input_mode = true;
                    }
                }
                Event::FocusLost => {
                    // Terminal lost focus - you might want to disable input mode here
                    // For now, keep it enabled for better UX
                }
                Event::Resize(width, height) => {
                    // Handle terminal resize
                    app.handle_terminal_resize(width, height);
                }
                Event::Mouse(mouse_event) => {
                    // Handle mouse events for scrolling
                    if matches!(app.state, crate::app::AppState::Chat) {
                        match mouse_event.kind {
                            MouseEventKind::ScrollDown => {
                                // Scroll down - scroll multiple lines for better UX
                                for _ in 0..3 {
                                    layout.scroll_state.scroll_down();
                                }
                            }
                            MouseEventKind::ScrollUp => {
                                // Scroll up - scroll multiple lines for better UX
                                for _ in 0..3 {
                                    layout.scroll_state.scroll_up();
                                }
                            }
                            _ => {
                                // Handle other mouse events if needed
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Handle pending async commands
        if let Some(command) = app.pending_command.take() {
            app.handle_command(command).await;
        }

        if last_tick.elapsed() >= tick_rate {
            app.update();
            last_tick = Instant::now();
        }

        // Check if app should exit
        if app.state == crate::app::AppState::Exiting {
            // Graceful exit: first clear popup and textarea, then exit
            app.state = crate::app::AppState::Chat; // Clear menu state
            app.show_input = false; // Hide input textarea
            let messages = app.messages.clone();
            terminal.draw(|f| layout.render(f, app, &messages))?; // Render chat without popup/textarea
            std::thread::sleep(Duration::from_millis(100)); // Brief pause for visual feedback
            return Ok(());
        }
    }
}

