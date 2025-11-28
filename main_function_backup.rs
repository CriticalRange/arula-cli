async fn main() -> Result<()> {
    // Install color-eyre for better error reporting
    let _ = color_eyre::install();

    // Set up panic hook to clean terminal and show errors
    setup_panic_hook();

    // Show cursor initially
    let _ = console::Term::stdout().show_cursor();

    // Create terminal guard to ensure terminal state is restored on exit
    let _terminal_guard = TerminalGuard;

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

    // Initialize AI client if configuration is valid
    match app.initialize_agent_client() {
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

    // Print banner BEFORE enabling raw mode
    output.print_banner()?;
    println!();

    // Print real-time changelog
    print_changelog()?;
    println!();

    // NOW enable raw mode for keyboard input detection
    enable_raw_mode()?;

    // Set cursor to be visible and use steady bar style
    setup_bar_cursor()?;

    // Create input handler with prompt
    let prompt = if cfg!(windows) { "▶" } else { "▶" };
    let mut input_handler = input_handler::InputHandler::new(prompt);
    let mut custom_spinner = custom_spinner::CustomSpinner::new();

    // Create overlay menu
    let mut menu = OverlayMenu::new();

    // Create persistent input for typing during AI response
    let mut persistent_input = PersistentInput::new();
    let mut buffered_input = String::new(); // Input typed during AI response

    // Main event loop
    'main_loop: loop {
        // If AI is processing, check for responses and allow typing
        if app.is_waiting_for_response() {
            // Handle AI responses while allowing user input
            let _spinner_running = false;
            while app.is_waiting_for_response() {
                // Check for keyboard input (non-blocking)
                if event::poll(std::time::Duration::from_millis(10))? {
                    if let Event::Key(key_event) = event::read()? {
                        if key_event.kind == KeyEventKind::Press {
                            match key_event.code {
                                crossterm::event::KeyCode::Esc => {
                                    // ESC pressed, cancel AI request
                                    custom_spinner.stop();
                                    output.print_system("🛑 Request cancelled (ESC pressed)")?;
                                    app.cancel_request();
                                    break;
                                }
                                crossterm::event::KeyCode::Char(c) => {
                                    // Buffer input while AI is responding
                                    persistent_input.insert_char(c);
                                    persistent_input.render()?;
                                }
                                crossterm::event::KeyCode::Backspace => {
                                    persistent_input.backspace();
                                    persistent_input.render()?;
                                }
                                crossterm::event::KeyCode::Enter => {
                                    // Queue the input for processing after AI finishes
                                    if !persistent_input.get_input().is_empty() {
                                        buffered_input = persistent_input.take();
                                        // Clear the input line visually
                                        execute!(
                                            std::io::stdout(),
                                            cursor::MoveToColumn(0),
                                            terminal::Clear(terminal::ClearType::CurrentLine)
                                        )?;
                                        print!("{} ", console::style("▶").cyan());
                                        std::io::stdout().flush()?;
                                    }
                                }
                                crossterm::event::KeyCode::Left => {
                                    persistent_input.move_left();
                                    persistent_input.render()?;
                                }
                                crossterm::event::KeyCode::Right => {
                                    persistent_input.move_right();
                                    persistent_input.render()?;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                match app.check_ai_response_nonblocking() {
                    Some(response) => {
                        match response {
                            app::AiResponse::AgentStreamStart => {
                                if custom_spinner.is_running() {
                                    custom_spinner.stop();
                                }
                                // Move to beginning of current line and clear it (contains user's input)
                                execute!(
                                    std::io::stdout(),
                                    cursor::MoveToColumn(0),
                                    terminal::Clear(terminal::ClearType::CurrentLine)
                                )?;
                                std::io::stdout().flush()?;
                                // Start AI message output without prefix
                                output.start_ai_message()?;
                            }
                            app::AiResponse::AgentStreamText(text) => {
                                // Always stop spinner first if running
                                if custom_spinner.is_running() {
                                    custom_spinner.stop();
                                    // The spinner.stop() already clears its line, no need for additional clearing
                                    std::io::stdout().flush()?;
                                }

                                // Print the chunk without starting spinner immediately
                                // Note: print_streaming_chunk handles its own spinner logic
                                output.print_streaming_chunk(&text)?;
                            }
                            app::AiResponse::AgentToolCall {
                                id: _,
                                name,
                                arguments,
                            } => {
                                custom_spinner.stop();
                                // Clear only the current line (where spinner was)
                                execute!(
                                    std::io::stdout(),
                                    cursor::MoveToColumn(0),
                                    terminal::Clear(terminal::ClearType::CurrentLine)
                                )?;
                                output.start_tool_execution(&name, &arguments)?;
                                // Set up spinner above input prompt
                                print!("{} ", console::style("▶").cyan());
                                std::io::stdout().flush()?;
                                custom_spinner.start_above(&format!("Executing tool: {}", name))?;
                            }
                            app::AiResponse::AgentToolResult {
                                tool_call_id: _,
                                success,
                                result,
                            } => {
                                custom_spinner.stop();
                                // Clear only the current line (where spinner was)
                                execute!(
                                    std::io::stdout(),
                                    cursor::MoveToColumn(0),
                                    terminal::Clear(terminal::ClearType::CurrentLine)
                                )?;
                                let result_text = format_tool_result(&result);

                                // Check if this is a colored diff - if so, print it directly without box
                                if result_text.contains("\u{1b}[") &&
                                   (result_text.contains("\u{1b}[31m") || result_text.contains("\u{1b}[32m")) {
                                    // This is a colored diff, print directly
                                    println!("{}", result_text);
                                } else {
                                    // Regular tool result, use box formatting
                                    output.complete_tool_execution(&result_text, success)?;
                                }

                                // Restore spinner above input prompt
                                print!("{} ", console::style("▶").cyan());
                                std::io::stdout().flush()?;
                                custom_spinner.start_above("Processing results...")?;
                            }
                            app::AiResponse::AgentStreamEnd => {
                                // Stop spinner cleanly (it clears its own line)
                                custom_spinner.stop();
                                output.stop_spinner();

                                // Finish the AI message line
                                output.end_line()?;
                                output.print_context_usage(None)?;
                                
                                // Clear accumulated text to reset state for next response
                                output.clear_accumulated_text();

                                // Add exactly ONE blank line after AI response
                                println!();

                                // Transfer any typed input to the input handler
                                let typed_input = persistent_input.get_input().to_string();
                                persistent_input.clear();
                                if !typed_input.is_empty() {
                                    input_handler.set_input(&typed_input);
                                }

                                // Set up persistent input prompt for next message
                                print!("{} ", console::style("▶").cyan());
                                std::io::stdout().flush()?;

                                break; // Exit the AI response loop
                            }
                        }
                    }
                    None => {
                        // Start spinner immediately if not running
                        if !custom_spinner.is_running() {
                            custom_spinner.start_above("Generating response...")?;
                        }
                    }
                }
            }

            // Process buffered input if any
            if !buffered_input.is_empty() {
                let input = std::mem::take(&mut buffered_input);
                input_handler.add_to_history(input.clone());
                match app.send_to_ai(&input).await {
                    Ok(()) => {}
                    Err(e) => {
                        output.print_error(&format!("Failed to send to AI: {}", e))?;
                    }
                }
                continue 'main_loop;
            }

            continue; // Continue to next iteration to get input
        }

        // Draw initial prompt
        input_handler.draw()?;

        // Input handling loop
        loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        match input_handler.handle_key(key_event)? {
                            Some(input) => {
                                // Handle special commands
                                if input == "__CTRL_C__" {
                                    // Ctrl+C pressed - show exit confirmation
                                    if menu.show_exit_confirmation(&mut output)? {
                                        output.print_system("Goodbye! 👋")?;
                                        graceful_exit();
                                    }
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else if input == "__CTRL_D__" {
                                    // Ctrl+D - EOF (no message here, handled at end)
                                    break;
                                } else if input == "__ESC__" {
                                    // ESC pressed, continue
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else if input == "m" || input == "menu" {
                                    // Menu shortcut
                                    if menu.show_main_menu(&mut app, &mut output)? {
                                        output.print_system("Goodbye! 👋")?;
                                        graceful_exit();
                                    }
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else if input.starts_with('/') {
                                    // Handle CLI commands
                                    handle_cli_command(&input, &mut app, &mut output, &mut menu).await?;
                                    input_handler.clear()?;
                                    input_handler.draw()?;
                                    continue 'main_loop;
                                } else {
                                    // Handle empty input
                                    if input.trim().is_empty() {
                                        input_handler.clear()?;
                                        input_handler.draw()?;
                                        continue;
                                    }

                                    // Add to history
                                    input_handler.add_to_history(input.clone());

                                    // Handle exit commands
                                    if input == "exit" || input == "quit" {
                                        if menu.show_exit_confirmation(&mut output)? {
                                            output.print_system("Goodbye! 👋")?;
                                            graceful_exit();
                                        }
                                        input_handler.clear()?;
                                        input_handler.draw()?;
                                        continue 'main_loop;
                                    }

                                    // Move to next line after user's input (which is already visible from input_handler)
                                    println!();

                                    // Send to AI
                                    if cli.verbose {
                                        output.print_system(&format!("DEBUG: About to call app.send_to_ai with input: '{}'", input))?;
                                    }
                                    match app.send_to_ai(&input).await {
                                        Ok(()) => {
                                            // AI request sent successfully
                                            if cli.verbose {
                                                output.print_system("DEBUG: AI request sent successfully")?;
                                            }
                                        }
                                        Err(e) => {
                                            // Handle AI client errors gracefully
                                            if cli.verbose {
                                                output.print_system(&format!("DEBUG: AI send failed with error: {}", e))?;
                                            }
                                            if e.to_string().contains("AI client not initialized") {
                                                output.print_error("AI client not configured. Use /config to set up AI settings.")?;
                                                output.print_system("💡 Try: /config or press 'm' for the configuration menu")?;
                                            } else {
                                                output.print_error(&format!("Failed to send to AI: {}", e))?;
                                            }
                                        }
                                    }

                                    // Clear input handler buffer (don't redraw, we'll set up our own layout)
                                    input_handler.set_input("");

                                    break; // Exit input loop to go to AI response handling
                                }
                            }
                            None => {
                                // Continue handling input
                                input_handler.draw()?;
                            }
                        }
                    }
                }
            } else {
                // No event, continue
                continue;
            }
        }
    }

    // Explicit cleanup before natural exit (in addition to TerminalGuard)
    output.print_system("Goodbye! 👋")?;

    // Ensure clean terminal state
    cleanup_terminal_and_exit()?;

    Ok(())
}
