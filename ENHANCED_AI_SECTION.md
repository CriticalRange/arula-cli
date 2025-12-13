# Enhanced AI Section for PROJECT.manifest

## Add This After "# AI ASSISTANCE NOTES" Section:

```markdown
# AI DEVELOPMENT GUIDE
## Project Context for AI Assistants
You're helping with ARULA CLI - an autonomous AI command-line tool. Key things to know:

### Core Architecture (30-second understanding)
- **Purpose**: AI CLI with chat interface, supports multiple providers
- **Architecture**: Async Rust with streaming, full-duplex terminal
- **Key Files**:
  - `arula_cli/src/main.rs` - Entry point (300 lines)
  - `arula_core/src/app.rs` - Core logic (83k lines, the beast)
  - `arula_core/src/api/agent_client.rs` - AI communication
  - `arula_cli/src/ui/tui_app.rs` - Terminal UI
- **Data Flow**: User input → TuiApp → App → AgentClient → AI provider

### Immediate Gotchas
1. **Don't run `cargo build` while app is running** - file locks
2. **All AI ops must be async** - using tokio channels
3. **Terminal state** - always restore after menus/raw mode
4. **Shared state** - use Arc<Mutex<>> not static variables
5. **Streaming responses** - don't buffer entire responses

### Common Patterns
```rust
// Adding new tool
#[async_trait]
impl Tool for MyTool {
    type Params = MyParams;
    type Result = MyResult;

    fn name(&self) -> &str { "my_tool" }
    async fn execute(&self, params: Self::Params) -> Result<Self::Result, String> {
        // Implementation
    }
}

// Adding new AI provider
impl Backend for MyProvider {
    async fn query(&self, prompt: &str, options: Option<AgentOptions>) -> ResponseStream {
        // Implementation
    }
}
```

### Making Changes
- **UI changes**: Update `arula_cli/src/ui/` components
- **Core logic**: Modify `arula_core/src/app.rs` or create new modules
- **Tools**: Add to `arula_core/src/tools/builtin/`
- **Providers**: Add to `arula_core/src/api/providers/`
- **Config**: Update `arula_core/src/utils/config.rs`

### Testing Your Changes
```bash
# Quick test
cargo check --package arula_cli

# Full test
cargo test

# With debug
ARULA_DEBUG=1 cargo run -- --debug
```

### When Users Say "It's broken"
1. Check terminal state (crossterm issues)
2. Verify configuration in `~/.config/arula/`
3. Look for async deadlocks in channels
4. Check AI provider API keys
5. Verify no file locks from previous runs

### Performance Tips
- Use `&str` not `String` for function parameters
- Prefer `Arc<str>` for shared strings
- Batch small operations
- Use `tokio::spawn` for independent tasks
- Watch out for blocking calls in async contexts

### Current State
- actively developing: ratatui TUI, MCP integration
- stable: core chat functionality, tool system
- needs work: test coverage, error handling
- known issues: YAML→JSON config migration
```

## Also Consider Adding:

### Module Size Indicators
```markdown
# MODULE COMPLEXITY
## Simple (<500 lines)
- Most tool implementations
- Configuration parsing
- Utility modules

## Medium (500-2000 lines)
- API client implementations
- UI components
- Provider backends

## Complex (>2000 lines)
- `arula_core/src/app.rs` - Main orchestration
- `arula_cli/src/ui/tui_app.rs` - Terminal UI
- `arula_core/src/tools/visioneer.rs` - Desktop automation
```

### Entry Point Summary
```markdown
# WHERE THINGS HAPPEN
## User Interaction
- CLI args → `arula_cli/src/main.rs`
- Chat input → `arula_cli/src/ui/input_handler.rs`
- Menu selection → `arula_cli/src/ui/menus/`

## Core Logic
- Message processing → `arula_core/src/app.rs:send_to_ai()`
- Response handling → `arula_core/src/app.rs:check_ai_response_nonblocking()`
- Tool execution → `arula_core/src/tools/tools.rs`

## AI Communication
- Provider interface → `arula_core/src/api/agent_client.rs`
- Streaming → `arula_core/src/api/models.rs`
- Tool calling → `arula_core/src/api/agent.rs`
```