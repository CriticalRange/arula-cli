#![allow(dead_code)]
#![allow(unreachable_code)]
#![allow(private_interfaces)]

use anyhow::Result;
use clap::Parser;

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

use arula_cli::ui::output::OutputHandler;
use arula_cli::ui::tui_app::TuiApp;
use arula_core::utils::changelog::{Changelog, ChangelogType};
use arula_core::App;

/// Print changelog from remote git or local file
fn print_changelog() -> Result<()> {
    // Fetch changelog (tries remote first, falls back to local)
    let changelog = Changelog::fetch_from_remote().unwrap_or_else(|_| {
        Changelog::fetch_local()
            .unwrap_or_else(|_| Changelog::parse(&Changelog::default_changelog()))
    });

    // Detect actual build type from git
    let build_type = Changelog::detect_build_type();
    let type_label = match build_type {
        ChangelogType::Release => "📦 Release",
        ChangelogType::Custom => "🔧 Custom Build",
        ChangelogType::Development => "⚙️  Development",
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
            println!("  {}", change);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set debug environment variable if debug flag is enabled
    if cli.debug {
        unsafe {
            std::env::set_var("ARULA_DEBUG", "1");
        }
    }

    // Initialize global logger
    if let Err(e) = arula_core::utils::logger::init_global_logger() {
        eprintln!("⚠️ Failed to initialize logger: {}", e);
    }

    // Create app with debug flag
    let mut app = App::new()?.with_debug(cli.debug);

    // Initialize app components
    let _ = app.initialize_git_state().await;
    let _ = app.initialize_tool_registry().await;
    let _ = app.initialize_agent_client();

    // Print banner and changelog BEFORE entering TUI
    let output = OutputHandler::new();
    output.print_banner()?;
    println!();
    print_changelog()?;
    println!();

    // Run TUI
    let mut tui = TuiApp::new(app)?;
    tui.run().await?;

    Ok(())
}
