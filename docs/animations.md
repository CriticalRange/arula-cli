# 🎨 ARULA CLI Animations

Beautiful terminal animations and effects for enhanced user experience.

## Features

### ✨ Terminal Effects

**Glowing Text** - Pulsing intensity animation
```rust
use arula_cli::ui::effects::TerminalEffects;

TerminalEffects::glowing_text("✨ Loading...", 5)?;
```

**Typewriter Effect** - Organic character-by-character typing
```rust
TerminalEffects::typewriter_async("Hello, World!", Duration::from_millis(40)).await?;
```

**Rainbow Text** - Smooth color transitions across the spectrum
```rust
TerminalEffects::rainbow_text("🌈 Beautiful Rainbow", 2, 100)?;
```

**Fade In Effect** - Gradual brightness increase
```rust
TerminalEffects::fade_in_text("🎭 Smooth Fade In", 20, 50)?;
```

**Pulse Effect** - Heartbeat-style intensity variation
```rust
TerminalEffects::pulse_text("💫 Pulse Animation", 10, 0.3, 1.0)?;
```

### 🔄 Enhanced Spinner

Your existing `CustomSpinner` now supports beautiful transitions:

```rust
use arula_cli::ui::custom_spinner::{CustomSpinner, Transition};

let mut spinner = CustomSpinner::new();

// Start with default orbital animation
spinner.start("Processing...")?;

// Transition to arc animation with fade
spinner.transition_to_arc();
spinner.set_message("Arc animation with fade");

// Transition to dots orbit with slide
spinner.transition_to_dots_orbit();
spinner.set_message("Dots orbit with slide");

// Custom transition with rainbow effect
let custom_frames = vec!["⢎⡡".to_string(), "⢎⡰".to_string(), "⢎⡑".to_string()];
spinner.transition_to(custom_frames, Transition::Rainbow);

spinner.finish_ok("✅ Success!");
```

### 🎭 Transition Effects

Available transition types:

- **`FadeOut`** - Smooth brightness fade transition
- **`SlideUp`** - Color slides from golden to white
- **`Pulse`** - Pulsing intensity during transition
- **`Rainbow`** - Full spectrum color cycle

## 🚀 Quick Demo

Run the animation demo to see all effects in action:

```bash
cargo run --example animation_demo
```

## 🎯 Usage Patterns

### Enhanced Loading Experience
```rust
async fn enhanced_loading() -> io::Result<()> {
    // Start with glowing welcome
    TerminalEffects::glowing_text("🚀 Starting process...", 3)?;

    // Typewriter for details
    TerminalEffects::typewriter_async(
        "Processing your request with beautiful animations...",
        Duration::from_millis(30)
    ).await?;

    // Rainbow during processing
    TerminalEffects::rainbow_text("Processing...", 1, 150)?;

    // Pulse when almost done
    TerminalEffects::pulse_text("Almost complete...", 5, 0.4, 0.9)?;

    // Final fade in success
    TerminalEffects::fade_in_text("✅ Success!", 15, 40)?;

    Ok(())
}
```

### Multi-Stage Spinner
```rust
async fn multi_stage_workflow() -> io::Result<()> {
    let mut spinner = CustomSpinner::new();

    // Stage 1: Initialize
    spinner.start("🔧 Initializing...")?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stage 2: Process (transition to arc)
    spinner.transition_to_arc();
    spinner.set_message("⚡ Processing data...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stage 3: Finalize (transition to dots)
    spinner.transition_to_dots_orbit();
    spinner.set_message("🎯 Finalizing...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Success
    spinner.finish_ok("🎉 All tasks completed!");

    Ok(())
}
```

## 🎨 Color Themes

The animations use ARULA's signature color palette:

- **Golden**: RGB(232, 197, 71) - Primary accent
- **Text**: RGB(205, 209, 196) - Readable gray
- **Success**: RGB(46, 204, 113) - Green checkmark
- **Error**: RGB(231, 76, 60) - Red X mark

## 🎛️ Customization

### Animation Speed
Adjust timing parameters for different experiences:

```rust
// Fast animations (snappy feel)
TerminalEffects::glowing_text("Quick load...", 3)?;

// Slow animations (relaxed feel)
TerminalEffects::fade_in_text("Gentle fade...", 30, 100)?;

// Medium pace (balanced)
TerminalEffects::typewriter_async("Standard speed", Duration::from_millis(50)).await?;
```

### Custom Frame Sets
Create your own spinner animations:

```rust
let custom_frames: Vec<String> = vec![
    "▹".to_string(), "▸".to_string(), "▾".to_string(), "▿".to_string()
];

spinner.transition_to(custom_frames, Transition::Pulse);
```

## 📚 API Reference

### `TerminalEffects`

- `glowing_text(text: &str, cycles: u32) -> io::Result<()>`
- `typewriter_async(text: &str, base_delay: Duration) -> io::Result<()>`
- `typewriter(text: &str, base_delay: Duration) -> io::Result<()>`
- `rainbow_text(text: &str, cycles: u32, speed_ms: u64) -> io::Result<()>`
- `fade_in_text(text: &str, steps: u32, delay_ms: u64) -> io::Result<()>`
- `pulse_text(text: &str, cycles: u32, min_intensity: f32, max_intensity: f32) -> io::Result<()>`

### `CustomSpinner`

- `new() -> Self`
- `start(message: &str) -> io::Result<()>`
- `set_message(msg: &str)`
- `transition_to(frames: Vec<String>, transition: Transition)`
- `transition_to_arc()`
- `transition_to_dots_orbit()`
- `finish_ok(final_message: &str)`
- `finish_err(final_message: &str)`
- `stop(&mut self)`

## 🎯 Best Practices

1. **Use appropriate speeds** - Fast for quick actions, slow for thoughtful processes
2. **Combine effects thoughtfully** - Don't overwhelm with too many animations at once
3. **Match animation to context** - Glowing for important moments, pulse for waiting, rainbow for celebrations
4. **Keep it readable** - Ensure text remains legible during animations
5. **Use meaningful transitions** - Different transitions for different types of state changes

## 🔧 Integration

These animations work seamlessly with ARULA's existing:

- **Reedline input system** - Animations don't interfere with typing
- **ExternalPrinter** - Concurrent AI output with animations
- **Menu system** - Enhanced visual feedback for navigation
- **Multi-provider AI** - Beautiful loading for different AI models

Your ARULA CLI now provides a gorgeous, professional terminal experience with smooth animations and beautiful visual effects! 🎉