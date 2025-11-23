//! Model selection menu functionality for ARULA CLI
//! Extracted from original overlay_menu.rs for modular architecture

use crate::app::App;
use crate::ui::output::OutputHandler;
use crate::ui::menus::dialogs::Dialogs;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
    style::{SetForegroundColor, ResetColor, Print},
    cursor::MoveTo,
    ExecutableCommand,
    QueueableCommand,
};
use std::io::{stdout, Write};
use std::time::Duration;

/// Model selection menu handler
pub struct ModelSelector {
    dialogs: Dialogs,
}

impl ModelSelector {
    pub fn new() -> Self {
        Self {
            dialogs: Dialogs::new(),
        }
    }

    /// Show the model selector menu
    pub fn show_model_selector(&mut self, app: &mut App, output: &mut OutputHandler) -> Result<()> {
        // Clear screen once when entering submenu to avoid artifacts (like original overlay_menu.rs)
        stdout().execute(terminal::Clear(terminal::ClearType::All))?;

        let current_config = app.get_config();
        let provider = current_config.active_provider.clone();
        let current_model = current_config.get_model();

        // For custom provider, use text input instead of selector
        if provider.to_lowercase() == "custom" {
            if let Some(model) = self.show_text_input("Enter model name", &current_model, output)? {
                app.set_model(&model);
                output.print_system(&format!("✅ Model set to: {}", model))?;
            }
            return Ok(());
        }

        // For predefined providers, use dynamic fetching with caching
        let (mut models, is_loading): (Vec<String>, bool) = match provider.to_lowercase().as_str() {
            "z.ai coding plan" | "z.ai" | "zai" => {
                // Clear cache to simulate first-run behavior
                app.cache_zai_models(Vec::new());
                let (models, loading) = self.get_zai_models(app, output)?;
                (models, loading)
            }
            "openai" => {
                // Clear cache to simulate first-run behavior
                app.cache_openai_models(Vec::new());
                let (models, loading) = self.get_openai_models(app, output)?;
                (models, loading)
            }
            "anthropic" => {
                // Clear cache to simulate first-run behavior
                app.cache_anthropic_models(Vec::new());
                let (models, loading) = self.get_anthropic_models(app, output)?;
                (models, loading)
            }
            "ollama" => {
                // Clear cache to simulate first-run behavior
                app.cache_ollama_models(Vec::new());
                let (models, loading) = self.get_ollama_models(app, output)?;
                (models, loading)
            }
            "openrouter" => {
                // For OpenRouter, fetch models dynamically with caching
                // Force cache clear to simulate first-run behavior every time
                app.cache_openrouter_models(Vec::new());

                let (models, is_loading) = self.get_openrouter_models(app, output)?;

                // Always return tuple with loading state
                if is_loading {
                    (models, is_loading)
                } else {
                    // Models loaded very quickly, but we still want to show transition
                    (vec!["⚡ Loading models...".to_string()], true)
                }
            }
            _ => {
                // Fallback to text input for unknown providers
                if let Some(model) = self.show_text_input("Enter model name", &current_config.get_model(), output)? {
                    app.set_model(&model);
                    output.print_system(&format!("✅ Model set to: {}", model))?;
                }
                return Ok(());
            }
        };

        // Handle loading state consistently for all providers
        let final_models = if is_loading {
            models.clone()
        } else {
            // Models loaded quickly, but we still want to show transition
            vec!["⚡ Loading models...".to_string()]
        };

        // Handle empty models list
        if final_models.is_empty() {
            output.print_system(&format!("⚠️ No {} models available. Try selecting the provider again to fetch models.", provider))?;
            return Ok(());
        }

        let current_idx = final_models
            .iter()
            .position(|m| m == &current_model)
            .unwrap_or(0);

        // Clear any pending events in the buffer
        std::thread::sleep(Duration::from_millis(20));
        for _ in 0..3 {
            while event::poll(Duration::from_millis(0))? {
                let _ = event::read()?;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        // Create a temporary selection for model with search support
        let mut selected_idx = current_idx;
        let mut search_query = String::new();
        let mut all_models = final_models.clone();
        let mut loading_spinner = all_models.len() == 1 && (all_models[0].contains("Loading") || all_models[0].contains("⚡") || all_models[0].contains("Fetching"));
        let mut spinner_counter = 0;
        let mut needs_clear = false; // Track when to clear screen
        let mut last_selected_idx = selected_idx; // Track scrolling

        // State tracking for selective rendering - track actual render state, not calculations
        let mut last_rendered_state: Option<(Vec<String>, usize, String, bool)> = None;

        loop {
            // Always check cache until we have real models (not just "Fetching models...")
            let should_check_cache = loading_spinner ||
                (all_models.len() == 1 && (all_models[0].contains("Loading") || all_models[0].contains("⚡") || all_models[0].contains("Fetching"))) ||
                spinner_counter < 50; // Keep checking longer for real models to arrive

            if should_check_cache {
                spinner_counter += 1;

                // Re-evaluate loading spinner state in case models changed
                let was_loading = loading_spinner;
                loading_spinner = all_models.len() == 1 && (all_models[0].contains("Loading") || all_models[0].contains("⚡") || all_models[0].contains("Fetching"));
                if was_loading != loading_spinner {
                    // State changed - clear screen once to show new content
                    needs_clear = true;
                }

                // Shorter timeout after 10 seconds (100 iterations of 100ms)
                if spinner_counter > 100 {
                    all_models = vec!["⚠️ Loading taking too long - Press ESC or try a different provider".to_string()];
                    loading_spinner = false;
                    let _ = output.print_system("⚠️ Model loading timed out - try using a different provider");
                } else {
                    // Check cache every iteration for immediate response
                    let cached_models = match provider.to_lowercase().as_str() {
                        "openai" => app.get_cached_openai_models(),
                        "anthropic" => app.get_cached_anthropic_models(),
                        "ollama" => app.get_cached_ollama_models(),
                        "z.ai coding plan" | "z.ai" | "zai" => app.get_cached_zai_models(),
                        "openrouter" => app.get_cached_openrouter_models(),
                        _ => None,
                    };

                    match cached_models {
                        Some(models) => {
                            if models.is_empty() {
                                // Still empty, continue loading
                            } else if models.len() == 1 && (models[0].contains("Loading") || models[0].contains("timeout") || models[0].contains("Fetching") || models[0].contains("⚡")) {
                                // Still in loading state
                            } else {
                                // Real models loaded! Update immediately and clear screen once
                                if all_models != models {
                                    all_models = models;
                                    loading_spinner = false;
                                    needs_clear = true; // Clear once when models finish loading
                                }
                            }
                        }
                        None => {
                            // Cache is None, models not loaded yet
                        }
                    }

                    // Update loading text with spinning animation
                    if loading_spinner {
                        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                        let spinner = spinner_chars[(spinner_counter / 2) % spinner_chars.len()];
                        all_models = vec![format!("{} Fetching models...", spinner)];
                    }
                }
            }

            // Filter models based on search query
            let filtered_models: Vec<String> = if search_query.is_empty() {
                all_models.clone()
            } else {
                all_models.iter()
                    .filter(|model| model.to_lowercase().contains(&search_query.to_lowercase()))
                    .cloned()
                    .collect()
            };

            // Update selected_idx to be within bounds of filtered models
            if filtered_models.is_empty() {
                selected_idx = 0;
            } else if selected_idx >= filtered_models.len() {
                selected_idx = filtered_models.len() - 1;
            }

            // Create current render state tuple for comparison
            let current_state = (filtered_models.clone(), selected_idx, search_query.clone(), loading_spinner);

            // Check if search query changed (requires clear and full re-render)
            let search_changed = if let Some(ref last_state) = last_rendered_state {
                last_state.2 != search_query  // Compare search query (index 2)
            } else {
                false
            };

            // Only render if the state actually changed
            let should_render = if let Some(ref last_state) = last_rendered_state {
                // Compare the actual render state, not intermediate calculations
                last_state != &current_state || needs_clear
            } else {
                // First render
                true
            };

            if should_render {
                // Clear screen once if needed (after fetch completes, major changes, or search changed)
                if needs_clear || search_changed {
                    stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                    stdout().flush()?;
                    needs_clear = false;
                }

                // Render the full UI
                self.render_model_selector_with_search(&filtered_models, selected_idx, &search_query, loading_spinner)?;

                // Update last rendered state
                last_rendered_state = Some(current_state);
                last_selected_idx = selected_idx;
            }

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) => {
                        // Only handle key press events to avoid double-processing on Windows
                        if key_event.kind != KeyEventKind::Press {
                            continue;
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                if selected_idx > 0 && !filtered_models.is_empty() {
                                    selected_idx -= 1;
                                    if selected_idx != last_selected_idx {
                                        needs_clear = true; // Clear once when scrolling
                                        last_selected_idx = selected_idx;
                                    }
                                }
                            }
                            KeyCode::Down => {
                                if selected_idx + 1 < filtered_models.len() {
                                    selected_idx += 1;
                                    if selected_idx != last_selected_idx {
                                        needs_clear = true; // Clear once when scrolling
                                        last_selected_idx = selected_idx;
                                    }
                                }
                            }
                            KeyCode::PageUp => {
                                if selected_idx > 10 {
                                    selected_idx -= 10;
                                } else {
                                    selected_idx = 0;
                                }
                                if selected_idx != last_selected_idx {
                                    needs_clear = true; // Clear once when scrolling
                                    last_selected_idx = selected_idx;
                                }
                            }
                            KeyCode::PageDown => {
                                if !filtered_models.is_empty() && selected_idx + 10 < filtered_models.len() {
                                    selected_idx += 10;
                                } else if !filtered_models.is_empty() {
                                    selected_idx = filtered_models.len() - 1;
                                }
                                if selected_idx != last_selected_idx {
                                    needs_clear = true; // Clear once when scrolling
                                    last_selected_idx = selected_idx;
                                }
                            }
                            KeyCode::Home => {
                                selected_idx = 0;
                                if selected_idx != last_selected_idx {
                                    needs_clear = true; // Clear once when scrolling
                                    last_selected_idx = selected_idx;
                                }
                            }
                            KeyCode::End => {
                                if !filtered_models.is_empty() {
                                    selected_idx = filtered_models.len() - 1;
                                    if selected_idx != last_selected_idx {
                                        needs_clear = true; // Clear once when scrolling
                                        last_selected_idx = selected_idx;
                                    }
                                }
                            }
                            KeyCode::Enter => {
                                if !filtered_models.is_empty() {
                                    app.set_model(&filtered_models[selected_idx]);
                                    output.print_system(&format!(
                                        "✅ Model set to: {}",
                                        filtered_models[selected_idx]
                                    ))?;
                                }
                                // Clear screen before exiting
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                break;
                            }
                            KeyCode::Esc => {
                                // Clear screen before exiting
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                break;
                            }
                            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                                // Clear screen before exiting
                                stdout().execute(terminal::Clear(terminal::ClearType::All))?;
                                stdout().flush()?;
                                // Ctrl+C - close model selector (will show exit confirmation)
                                break;
                            }
                            KeyCode::Backspace => {
                                if !search_query.is_empty() {
                                    search_query.pop();
                                }
                            }
                            // Handle Ctrl+C BEFORE general character input
                            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                                if loading_spinner {
                                    // When loading, clear cache
                                    match provider.to_lowercase().as_str() {
                                        "openai" => { let _ = app.cache_openai_models(Vec::new()); },
                                        "anthropic" => { let _ = app.cache_anthropic_models(Vec::new()); },
                                        "ollama" => { let _ = app.cache_ollama_models(Vec::new()); },
                                        "z.ai coding plan" | "z.ai" | "zai" => { let _ = app.cache_zai_models(Vec::new()); },
                                        "openrouter" => { let _ = app.cache_openrouter_models(Vec::new()); },
                                        _ => {}
                                    }
                                    let _ = output.print_system("🗑️ Cache cleared");
                                    spinner_counter = 0;
                                } else {
                                    // When not loading, exit the menu
                                    break;
                                }
                            }
                            KeyCode::Char('r') if key_event.modifiers == KeyModifiers::CONTROL => {
                                // Always allow retry regardless of loading state
                                // Retry for the specific provider
                                match provider.to_lowercase().as_str() {
                                    "openai" => app.fetch_openai_models(),
                                    "anthropic" => app.fetch_anthropic_models(),
                                    "ollama" => app.fetch_ollama_models(),
                                    "z.ai coding plan" | "z.ai" | "zai" => app.fetch_zai_models(),
                                    "openrouter" => app.fetch_openrouter_models(),
                                    _ => {}
                                }
                                models = vec!["Fetching models...".to_string()];
                                loading_spinner = true;
                                spinner_counter = 0; // Reset timeout counter
                            }
                            // General character input for search - only if not a control character
                            KeyCode::Char(c) if c.is_ascii() && !c.is_control() => {
                                if !loading_spinner {
                                    search_query.push(c);
                                    // Reset selection when typing
                                    selected_idx = 0;
                                }
                            }
                            _ => {
                                // Ignore other keys
                                continue;
                            }
                        }
                    }
                    _ => {
                        // Ignore other event types
                        continue;
                    }
                }
            }
        }

        Ok(())
    }

    /// Show text input dialog (fallback for custom providers)
    fn show_text_input(&self, prompt: &str, default_value: &str, output: &mut OutputHandler) -> Result<Option<String>> {
        self.dialogs.input_dialog(prompt, Some(default_value), output)
    }

    /// Get OpenAI models with loading state
    fn get_openai_models(&self, app: &App, output: &mut OutputHandler) -> Result<(Vec<String>, bool)> {
        app.fetch_openai_models();
        Ok((vec!["Fetching models...".to_string()], true))
    }

    /// Get Anthropic models with loading state
    fn get_anthropic_models(&self, app: &App, output: &mut OutputHandler) -> Result<(Vec<String>, bool)> {
        app.fetch_anthropic_models();
        Ok((vec!["Fetching models...".to_string()], true))
    }

    /// Get Ollama models with loading state
    fn get_ollama_models(&self, app: &App, output: &mut OutputHandler) -> Result<(Vec<String>, bool)> {
        app.fetch_ollama_models();
        Ok((vec!["Fetching models...".to_string()], true))
    }

    /// Get Z.AI models with loading state
    fn get_zai_models(&self, app: &App, output: &mut OutputHandler) -> Result<(Vec<String>, bool)> {
        app.fetch_zai_models();
        Ok((vec!["Fetching models...".to_string()], true))
    }

    /// Get OpenRouter models with loading state
    fn get_openrouter_models(&self, app: &App, output: &mut OutputHandler) -> Result<(Vec<String>, bool)> {
        app.fetch_openrouter_models();
        Ok((vec!["Fetching models...".to_string()], true))
    }

    /// Draw modern box (copied from original overlay_menu.rs)
    fn draw_modern_box(&self, x: u16, y: u16, width: u16, height: u16, _title: &str) -> Result<()> {
        // Modern box with rounded corners using our color theme
        let top_left = "╭";
        let top_right = "╮";
        let bottom_left = "╰";
        let bottom_right = "╯";
        let horizontal = "─";
        let vertical = "│";

        // Validate dimensions to prevent overflow
        if width < 2 || height < 2 {
            return Ok(());
        }

        // Draw borders using our AI highlight color (steel blue)
        stdout().queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?;

        // Draw vertical borders
        for i in 0..height {
            stdout().queue(MoveTo(x, y + i))?.queue(Print(vertical))?;
            stdout().queue(MoveTo(x + width.saturating_sub(1), y + i))?.queue(Print(vertical))?;
        }

        // Top border
        stdout().queue(MoveTo(x, y))?.queue(Print(top_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(top_right))?;

        // Bottom border
        stdout().queue(MoveTo(x, y + height.saturating_sub(1)))?.queue(Print(bottom_left))?;
        for _i in 1..width.saturating_sub(1) {
            stdout().queue(Print(horizontal))?;
        }
        stdout().queue(Print(bottom_right))?;

        stdout().queue(ResetColor)?;
        Ok(())
    }

    /// Render model selector with search functionality
    fn render_model_selector_with_search(&self, models: &[String], selected_idx: usize, search_query: &str, loading: bool) -> Result<()> {
        let (cols, rows) = crossterm::terminal::size()?;

        // Calculate layout that fits within terminal height
        let total_models = models.len();

        // Reserve space for title (1), search (1), borders (2), navigation (1) = 5 lines total
        let available_height = rows.saturating_sub(6) as usize; // Leave extra padding
        let max_visible_models = available_height.max(1);

        // Use single column layout with proper width
        let menu_width = std::cmp::min(cols.saturating_sub(4), 60); // Good width for model names
        let menu_height = std::cmp::min(max_visible_models, total_models) + 6; // +6 for title, search, borders, navigation
        let menu_height_u16 = menu_height as u16;

        // Ensure menu fits in terminal
        let final_menu_height = if menu_height_u16 > rows.saturating_sub(4) {
            rows.saturating_sub(4) as usize
        } else {
            menu_height
        };

        let start_x = if cols > menu_width { cols.saturating_sub(menu_width) / 2 } else { 0 };
        let start_y = if rows > final_menu_height as u16 { rows.saturating_sub(final_menu_height as u16) / 2 } else { 0 };

        // Calculate viewport - ensure selected item is visible
        let actual_visible_models = std::cmp::min(max_visible_models, final_menu_height.saturating_sub(6));
        let viewport_start = if selected_idx >= actual_visible_models {
            selected_idx - actual_visible_models + 1
        } else {
            0
        };
        let viewport_end = std::cmp::min(viewport_start + actual_visible_models, total_models);

        // Display title with search hint
        let title = if search_query.is_empty() {
            format!("Select AI Model ({} models)", total_models)
        } else {
            format!("Select AI Model ({} of {} filtered)", models.len(), total_models)
        };
        self.draw_modern_box(start_x, start_y, menu_width, final_menu_height as u16, &title)?;

        // Show search input with error state detection
        let search_y = start_y + 1;

        // Check if models contain error messages
        let has_error = !models.is_empty() && (
            models[0].contains("error") ||
            models[0].contains("Error") ||
            models[0].contains("401") ||
            models[0].contains("403") ||
            models[0].contains("timeout") ||
            models[0].contains("failed") ||
            models[0].contains("Failed") ||
            models[0].contains("⚠️")
        );

        let search_text = if has_error {
            // Show error message from models[0]
            models[0].clone()
        } else if loading {
            "🔄 Fetching models...".to_string()
        } else if search_query.is_empty() {
            "🔍 Type to search models".to_string()
        } else {
            format!("🔍 {}", search_query)
        };

        // Print search text (pad with spaces to clear previous content)
        let search_width = menu_width.saturating_sub(4) as usize;
        let padded_search = format!("{:width$}", search_text, width = search_width);
        stdout().queue(MoveTo(start_x + 2, search_y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(&padded_search))?
              .queue(ResetColor)?;

        // Display models in viewport
        let max_text_width = menu_width.saturating_sub(6) as usize; // Leave space for prefix and padding

        if models.is_empty() {
            // Show a nice "no models found" message
            let y = start_y + 3;
            let no_results_msg = if search_query.is_empty() {
                "🔍 No models available"
            } else {
                "🔍 No models found with that name"
            };
            let msg_width = menu_width.saturating_sub(4) as usize;
            let padded_msg = format!("{:^width$}", no_results_msg, width = msg_width);
            stdout().queue(MoveTo(start_x + 2, y))?
                  .queue(SetForegroundColor(crossterm::style::Color::DarkGrey))?
                  .queue(Print(&padded_msg))?
                  .queue(ResetColor)?;
        } else {
            // Safe subtraction with saturating_sub to prevent overflow
            let items_to_show = viewport_end.saturating_sub(viewport_start);

            for (idx, model) in models.iter().enumerate().skip(viewport_start).take(items_to_show) {
                let y = start_y + 3 + (idx - viewport_start) as u16;

                // Truncate long model names to fit
                let display_text = if model.len() > max_text_width {
                    format!("{}...", &model[..max_text_width.saturating_sub(3)])
                } else {
                    model.clone()
                };

                let prefix = if idx == selected_idx { "▶ " } else { "  " };
                let text = format!("{}{}", prefix, display_text);

                // Pad with spaces to clear any previous content
                let text_width = menu_width.saturating_sub(4) as usize;
                let padded_text = format!("{:width$}", text, width = text_width);

                let color = if idx == selected_idx {
                    SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::PRIMARY_ANSI))
                } else {
                    SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::MISC_ANSI))
                };

                // Print the padded model text
                stdout().queue(MoveTo(start_x + 2, y))?
                      .queue(color)?
                      .queue(Print(&padded_text))?
                      .queue(ResetColor)?;
            }
        }

        // Show navigation hint (intercepting box border - left aligned)
        let nav_y = start_y + final_menu_height as u16 - 1;
        let nav_text = if models.is_empty() {
            "No results - Press ESC to go back".to_string()
        } else if viewport_start == 0 && viewport_end == total_models {
            // All models visible - show enter to select and ESC to go back
            "↑↓ Navigate • ↵ Select • ESC Back".to_string()
        } else {
            // Showing a subset - show position with enter and back options
            format!("↑↓ Navigate ({}-{} of {}) • ↵ Select • ESC Back",
                    viewport_start + 1, viewport_end, total_models)
        };

        // Print navigation text (left aligned with padding)
        let nav_x = start_x + 2;
        stdout().queue(MoveTo(nav_x, nav_y))?
              .queue(SetForegroundColor(crossterm::style::Color::AnsiValue(crate::utils::colors::AI_HIGHLIGHT_ANSI)))?
              .queue(Print(&nav_text))?
              .queue(ResetColor)?;

        stdout().flush()?;
        Ok(())
    }
}

impl Default for ModelSelector {
    fn default() -> Self {
        Self::new()
    }
}