//! Animation Effects Demo for ARULA CLI
//!
//! This example demonstrates the beautiful animation effects including:
//! - Glowing text with pulsing intensity
//! - Typewriter effects with variable speed
//! - Rainbow color transitions
//! - Custom spinner transitions
//! - Fade and slide effects

use arula_cli::ui::{
    custom_spinner::{CustomSpinner, Transition},
    effects::TerminalEffects,
};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("🎨 ARULA CLI Animation Effects Demo\n");

    // Demo 1: Glowing Text Effect
    println!("✨ Demo 1: Glowing Text Effect");
    TerminalEffects::glowing_text("✨ Loading beautiful animations...", 5)?;
    thread::sleep(Duration::from_secs(1));
    println!("\n");

    // Demo 2: Typewriter Effect
    println!("✨ Demo 2: Typewriter Effect");
    TerminalEffects::typewriter_async(
        "Welcome to ARULA CLI - Autonomous AI with gorgeous animations!",
        Duration::from_millis(40),
    )
    .await?;
    println!("\n\n");

    // Demo 3: Rainbow Text Effect
    println!("✨ Demo 3: Rainbow Text Effect");
    TerminalEffects::rainbow_text("🌈 Beautiful Rainbow Animation", 2, 100)?;
    println!("\n");

    // Demo 4: Fade In Effect
    println!("✨ Demo 4: Fade In Effect");
    TerminalEffects::fade_in_text("🎭 Smooth Fade In Animation", 20, 50)?;
    println!("\n");

    // Demo 5: Pulse Effect
    println!("✨ Demo 5: Pulse Effect");
    TerminalEffects::pulse_text("💫 Heartbeat Pulse Animation", 10, 0.3, 1.0)?;
    println!("\n");

    // Demo 6: Custom Spinner Transitions
    println!("✨ Demo 6: Custom Spinner with Transitions");
    demo_spinner_transitions()?;

    // Demo 7: Combined Effects
    println!("✨ Demo 7: Combined Animation Effects");
    demo_combined_effects().await?;

    println!("\n🎉 Animation Demo Complete! Your ARULA CLI now has gorgeous animations! ✨");

    Ok(())
}

/// Demonstrate custom spinner with various transitions
fn demo_spinner_transitions() -> io::Result<()> {
    let mut spinner = CustomSpinner::new();

    // Start with default star animation
    println!("Starting with default orbital spinner...");
    spinner.start("Default orbital animation")?;
    thread::sleep(Duration::from_secs(2));

    // Transition to arc animation with fade effect
    println!("Transitioning to arc animation with fade...");
    spinner.transition_to_arc();
    spinner.set_message("Arc animation with fade transition");
    thread::sleep(Duration::from_secs(2));

    // Transition to dots orbit with slide effect
    println!("Transitioning to dots orbit with slide...");
    spinner.transition_to_dots_orbit();
    spinner.set_message("Dots orbit with slide transition");
    thread::sleep(Duration::from_secs(2));

    // Transition back to stars with rainbow effect
    println!("Transitioning back to stars with rainbow...");
    let star_frames: Vec<String> = ["⢎⡰", "⢎⡡", "⢎⡑", "⢎⠱"]
        .iter()
        .map(|&s| s.to_string())
        .collect();
    spinner.transition_to(star_frames, Transition::Rainbow);
    spinner.set_message("Stars with rainbow transition");
    thread::sleep(Duration::from_secs(2));

    // Finish with success
    spinner.finish_ok("All transitions completed successfully!");

    Ok(())
}

/// Demonstrate combined animation effects
async fn demo_combined_effects() -> io::Result<()> {
    // Start with a glowing welcome
    TerminalEffects::glowing_text("🚀 Starting combined effects demo...", 3)?;

    // Typewriter for the main message
    TerminalEffects::typewriter_async(
        "Let's create a stunning loading experience...",
        Duration::from_millis(30),
    )
    .await?;

    // Rainbow text for the process
    TerminalEffects::rainbow_text("Processing your beautiful animations", 1, 150)?;

    // Pulse during completion
    TerminalEffects::pulse_text("Almost done...", 5, 0.4, 0.9)?;

    // Final fade in
    TerminalEffects::fade_in_text("✨ Perfect! All animations working beautifully!", 15, 40)?;

    Ok(())
}
