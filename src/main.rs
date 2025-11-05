use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
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
mod git_ops;
mod cli_commands;

use app::App;
use layout::Layout;
use ui_components::Theme;

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

    // Setup terminal with proper keyboard handling
    enable_raw_mode()?;
    let mut stdout = stdout();

    // Enable all terminal features for better input handling, especially for Termux
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        crossterm::event::EnableBracketedPaste,
        crossterm::event::EnableFocusChange
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and layout
    let mut app = App::new()?;
    app.set_api_client(cli.endpoint.clone());
    let mut layout = Layout::new(Theme::Cyberpunk);

    // Run app
    let res = run_app(&mut terminal, &mut app, &mut layout).await;

    // Restore terminal properly
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        crossterm::event::DisableBracketedPaste,
        crossterm::event::DisableFocusChange
    )?;
    terminal.show_cursor()?;

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
    let tick_rate = Duration::from_millis(250);

    loop {

        // Draw UI
        terminal.draw(|f| layout.render(f, &app, &app.messages, &app.input, app.input_mode))?;

        // Handle events with shorter timeout for better responsiveness
        let timeout = Duration::from_millis(50); // Very responsive to input

        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    // Handle Ctrl+C for exit
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        app.state = crate::app::AppState::Exiting;
                        return Ok(());
                    }

                    match key.code {
                        KeyCode::Esc => {
                            // Handle escape key differently based on context
                            if app.input_mode {
                                app.input_mode = false;
                            } else if layout.settings_mode {
                                layout.toggle_settings_mode(); // Exit settings mode
                            } else {
                                layout.toggle_settings_mode(); // Enter settings mode
                            }
                        }
                        _ => {
                            // Only handle other keys if we're not in settings mode
                            if !layout.settings_mode {
                                app.handle_key_event(key);
                            }
                        }
                    }
                }
                Event::Mouse(_) => {
                    // Mouse click - always enable input mode (more aggressive for Termux)
                    if !layout.settings_mode {
                        app.input_mode = true;
                    }
                }
                Event::FocusGained => {
                    // Terminal gained focus - enable input mode
                    if !layout.settings_mode {
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
            return Ok(());
        }
    }
}

