# CLAUDE.md

ARULA CLI - Autonomous AI command-line interface with TUI.

## Development Commands

```bash
cargo build && cargo run         # Build and run
cargo clippy && cargo fmt        # Code quality
cargo test                       # Run tests
```

## Architecture

**Core Flow**: `main()` → event loop → `app.handle_key_event()` / `app.check_ai_response()`

**Key Modules**:
- `app.rs`: Application state (too large, needs refactoring)
- `main.rs`: Event loop, terminal handling
- `api.rs`: AI client with streaming
- `layout.rs`: TUI rendering

**AI Streaming**: Uses `tokio::sync::mpsc::unbounded_channel()` for non-blocking responses.

## Design Principles

**Core Principles to Follow:**

1. **Single Responsibility Principle (SRP)**
   - Each module has one clear purpose
   - Break down large files (app.rs is 2058 lines)
   - One reason to change per module

2. **Don't Repeat Yourself (DRY)**
   - Extract common patterns into reusable functions
   - Avoid duplicated code across modules

3. **KISS Principle**
   - Keep code simple and straightforward
   - Async event loop is a good example

4. **Command-Query-Separation (CQS)**
   - Commands perform actions, queries return data
   - `handle_ai_command()` is command, state checks are queries

5. **Encapsulation**
   - Bundle data with methods that operate on it
   - `ConversationManager` encapsulates chat persistence
   - `GitOperations` encapsulates repository state

6. **Open/Closed Principle (OCP)**
   - Open for extension, closed for modification
   - Use traits for AI providers to extend easily

7. **Dependency Inversion Principle (DIP)**
   - Depend on abstractions, not concretions
   - Abstract `ApiClient` behind trait for testing

## Implementation Patterns

**AI Streaming**:
```rust
let (tx, rx) = mpsc::unbounded_channel();
self.ai_response_rx = Some(rx);
tokio::spawn(async move {
    api_client.send_message_streaming(prompt, message_history, tx).await;
});
```


**Adding Menu Options**:
1. Add variant to `MenuOption` enum
2. Add to `App::menu_options()`
3. Add display text in `App::option_display()`
4. Handle in `App::handle_menu_navigation()`

## Terminal Notes

- Termux: `export TERM=xterm-256color`
- Width < 50: auto vertical layout
- Mouse support enabled