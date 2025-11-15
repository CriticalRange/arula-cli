# CLAUDE.md

ARULA CLI - Autonomous AI command-line interface with native terminal scrollback.

## Development Commands

```bash
cargo build && cargo run         # Build and run
cargo clippy && cargo fmt        # Code quality
cargo test                       # Run tests
```

## Architecture

**Core Flow**: `main()` → rustyline readline loop → `app.send_to_ai()` / `app.check_ai_response_nonblocking()`

**Key Modules**:
- `app.rs`: Application state and AI message handling (~260 lines)
- `main.rs`: Rustyline input loop, command handling, AI response processing
- `api.rs`: AI client with streaming support
- `output.rs`: Colored terminal output to stdout
- `overlay_menu.rs`: Crossterm-based overlay menu system
- `tool_call.rs`: Bash command extraction from AI responses

**AI Streaming**: Uses `tokio::sync::mpsc::unbounded_channel()` for non-blocking responses.

**Terminal Design**: No alternate screen - all output flows to native scrollback buffer.

## Design Principles

**Core Principles Followed:**

1. **Single Responsibility Principle (SRP)**
   - Each module has one clear purpose
   - `output.rs` handles display, `app.rs` handles logic, `overlay_menu.rs` handles menus
   - Successfully reduced `app.rs` from 2058 lines to ~260 lines

2. **Don't Repeat Yourself (DRY)**
   - Extract common patterns into reusable functions
   - `OutputHandler` centralizes all terminal output formatting

3. **KISS Principle**
   - Keep code simple and straightforward
   - Replaced complex ratatui TUI with simple rustyline readline loop
   - Direct stdout printing instead of render buffers

4. **Command-Query-Separation (CQS)**
   - Commands perform actions: `send_to_ai()`, `execute_bash_command()`
   - Queries return data: `get_config()`, `check_ai_response_nonblocking()`

5. **Encapsulation**
   - `OutputHandler` encapsulates colored output
   - `OverlayMenu` encapsulates menu state and rendering
   - `ApiClient` encapsulates API communication

## Implementation Patterns

**AI Streaming**:
```rust
let (tx, rx) = mpsc::unbounded_channel();
self.ai_response_rx = Some(rx);
tokio::spawn(async move {
    match api_client.send_message_stream(&msg, Some(message_history)).await {
        Ok(mut stream_rx) => {
            let _ = tx.send(AiResponse::StreamStart);
            while let Some(response) = stream_rx.recv().await {
                match response {
                    StreamingResponse::Chunk(chunk) => {
                        let _ = tx.send(AiResponse::StreamChunk(chunk));
                    }
                    StreamingResponse::End(_) => {
                        let _ = tx.send(AiResponse::StreamEnd);
                        break;
                    }
                }
            }
        }
    }
});
```

**Non-blocking Response Check**:
```rust
// In main loop
if let Some(response) = app.check_ai_response_nonblocking() {
    match response {
        AiResponse::StreamStart => output.start_ai_message()?,
        AiResponse::StreamChunk(chunk) => output.print_streaming_chunk(&chunk)?,
        AiResponse::StreamEnd => output.end_line()?,
    }
}
```

**Overlay Menu Pattern**:
```rust
// Clear screen, enable raw mode, show menu, restore
execute!(stdout(), terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0))?;
terminal::enable_raw_mode()?;
let result = self.run_main_menu(app, output);
terminal::disable_raw_mode()?;
execute!(stdout(), terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0))?;
```

**Adding Menu Options**:
1. Add option to `options` vector in `run_main_menu()`
2. Add match arm in `KeyCode::Enter` handler
3. Implement the option's logic (may call other methods)
4. Use `show_confirm_dialog()` for confirmations

## Terminal Notes

- Termux: `export TERM=xterm-256color`
- Native scrollback enabled - no alternate screen
- Menu shortcuts: `m`, `menu`, or `/menu`
- Ctrl+C shows exit confirmation (double press to exit)
- All output uses console for consistent styling

## Key Libraries

- **rustyline**: Readline-style input with history
- **crossterm**: Terminal manipulation (raw mode, cursor, styling)
- **console**: Colored output with rich styling options
- **dialoguer**: Interactive prompts (used in configuration menu)
- **tokio**: Async runtime for AI streaming
- **async-openai**: OpenAI API client with streaming

## TODO: Future Enhancements

### UI/UX Improvements
- [ ] Progress indicators with spinners (using `indicatif` or console built-in)
- [ ] Formatted code blocks with syntax highlighting
- [ ] Syntax highlighting for AI responses (using `syntect`)
- [ ] Better markdown rendering (using `termimad` or `comrak`)
- [ ] Multi-line input support (Shift+Enter for new line, Enter to send)
- [ ] Enhanced input prompt with status indicators
- [ ] Token count display
- [ ] Message history browser

### Features
- [ ] Save/load conversation sessions
- [ ] Export conversations to markdown
- [ ] Custom system prompts
- [ ] Function/tool calling support
- [ ] Image input support (for vision models)
