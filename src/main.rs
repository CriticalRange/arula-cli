use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    cursor::MoveTo,
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io::{self, stdout, IsTerminal, Write};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, Ordering};

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
mod git_ops;
mod cli_commands;
mod progress;
mod conversation;

use app::App;
use layout::Layout;
use ui_components::Theme;

// Global flag for cleanup
static CLEANUP_DONE: AtomicBool = AtomicBool::new(false);

fn cleanup_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    if CLEANUP_DONE.swap(true, Ordering::SeqCst) {
        return Ok(()); // Already cleaned up
    }

    // Ensure we're back to normal terminal state
    let _ = disable_raw_mode();

    // Clear screen and move cursor to top-left for clean exit
    let _ = execute!(
        terminal.backend_mut(),
        Clear(ClearType::All),
        MoveTo(0, 0)
    );

    // Execute cleanup commands (NO LeaveAlternateScreen since we didn't enter it)
    let result = execute!(
        terminal.backend_mut(),
        crossterm::event::DisableBracketedPaste,
        crossterm::event::DisableFocusChange
    );

    let _ = terminal.show_cursor();
    let _ = io::stdout().flush();

    result.map_err(|e| anyhow::anyhow!("Failed to cleanup terminal: {}", e))
}

#[tokio::main]
async fn main() -> Result<()> {
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

    // Store original terminal state for restoration
    let mut stdout = stdout();

    // Clear screen first for clean start (native scrollback still works)
    execute!(
        stdout,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )?;

    // Enable raw mode first
    enable_raw_mode()?;

    // Enable terminal features WITHOUT alternate screen for native scrollback
    execute!(
        stdout,
        crossterm::event::EnableBracketedPaste,
        crossterm::event::EnableFocusChange
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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

    let mut layout = Layout::new(Theme::Cyberpunk);

    // Run app with proper cleanup
    let res = run_app(&mut terminal, &mut app, &mut layout).await;

    // Always cleanup, even if an error occurred
    if let Err(cleanup_err) = cleanup_terminal(terminal) {
        eprintln!("Warning: Failed to cleanup terminal properly: {}", cleanup_err);
    }

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

        // Draw UI
        let messages = app.messages.clone();
        terminal.draw(|f| layout.render(f, app, &messages))?;

        // Check if scrolled to bottom after rendering, re-enable auto-scroll
        {
            let total_lines = app.messages.len() * 2; // Approximation: each message + blank line
            let visible_lines = 20; // Approximation of visible area
            app.check_if_at_bottom(total_lines, visible_lines);
        }

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

