/// Demo showcasing the new inquire-based input system
/// Run with: cargo run --example inquire_demo

use arula_cli::inquire_input::{InquireInputHandler, StyledInputBuilder, themes};

fn main() -> anyhow::Result<()> {
    println!("🎨 ARULA Inquire Input Demo\n");

    // Set global config for all inquire prompts
    InquireInputHandler::set_global_config();

    // Example 1: Basic input
    println!("📝 Example 1: Basic Input");
    let handler = InquireInputHandler::new("▶ Enter your name");
    let name = handler.get_input()?;
    println!("✓ Hello, {}!\n", name);

    // Example 2: Input with default value
    println!("📝 Example 2: Input with Default");
    let handler = InquireInputHandler::new("▶ Choose AI model");
    let model = handler.get_input_with_default("claude-3-5-sonnet-20241022")?;
    println!("✓ Selected model: {}\n", model);

    // Example 3: Input with placeholder
    println!("📝 Example 3: Input with Placeholder");
    let handler = InquireInputHandler::new("▶ Enter API endpoint");
    let endpoint = handler.get_input_with_placeholder("https://api.anthropic.com")?;
    println!("✓ Endpoint: {}\n", endpoint);

    // Example 4: Input with help message
    println!("📝 Example 4: Input with Help");
    let handler = InquireInputHandler::new("▶ Enter your API key");
    let key = handler.get_input_with_help("Your API key will be stored securely")?;
    println!("✓ API key saved (length: {})\n", key.len());

    // Example 5: Input with validation
    println!("📝 Example 5: Input with Validation");
    use inquire::validator::Validation;
    let handler = InquireInputHandler::new("▶ Enter a number");
    let number = handler.get_input_with_validation(|input| {
        input.parse::<i32>()
            .map(|_| Validation::Valid)
            .map_err(|_| "Please enter a valid number".to_string())
    })?;
    println!("✓ Valid number: {}\n", number);

    // Example 6: Input with autocomplete
    println!("📝 Example 6: Input with Autocomplete");
    let commands = vec![
        "/help".to_string(),
        "/menu".to_string(),
        "/clear".to_string(),
        "/history".to_string(),
        "/config".to_string(),
    ];
    let handler = InquireInputHandler::new("▶ Type a command");
    let command = handler.get_input_with_suggestions(commands)?;
    println!("✓ Command: {}\n", command);

    // Example 7: Using the builder pattern
    println!("📝 Example 7: Builder Pattern");
    let message = StyledInputBuilder::new("▶ Enter your message")
        .with_placeholder("Type something...")
        .with_help("Press ESC to cancel")
        .with_validator(|input| {
            if input.trim().is_empty() {
                Err("Message cannot be empty".to_string())
            } else {
                Ok(Validation::Valid)
            }
        })
        .prompt()?;
    println!("✓ Message: {}\n", message);

    // Example 8: Using different themes
    println!("📝 Example 8: Custom Themes");

    println!("  Error theme:");
    let error_config = themes::error_theme();
    let error_input = inquire::Text::new("▶ Enter error message")
        .with_render_config(error_config)
        .prompt()?;
    println!("  ✗ Error: {}\n", error_input);

    println!("  Success theme:");
    let success_config = themes::success_theme();
    let success_input = inquire::Text::new("▶ Enter success message")
        .with_render_config(success_config)
        .prompt()?;
    println!("  ✓ Success: {}\n", success_input);

    println!("  Info theme:");
    let info_config = themes::info_theme();
    let info_input = inquire::Text::new("▶ Enter info message")
        .with_render_config(info_config)
        .prompt()?;
    println!("  ℹ Info: {}\n", info_input);

    println!("🎉 Demo completed!");

    Ok(())
}
