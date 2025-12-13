# Suggested Improvements for PROJECT.manifest

## Current Strengths:
- Excellent overview of project structure
- Good capture of key technologies and patterns
- Comprehensive workflow commands
- Valuable decision log and gotchas
- Clear organization with sections

## Suggested Improvements:

### 1. Add Quick Reference Section at Top
```markdown
# QUICK REFERENCE (First 30 seconds)
- What it is: Autonomous AI CLI tool with chat interface
- Where to start: `arula_cli/src/main.rs` or `arula_core/src/app.rs`
- Key pattern: Async streaming with full-duplex terminal
- Main dependencies: tokio, reedline, reqwest, serde
- Test command: `cargo test`
```

### 2. Code Navigation Hints
```markdown
# CODE NAVIGATION
## Learning Path
1. Start: `arula_cli/src/main.rs` - CLI entry point
2. Core: `arula_core/src/app.rs` - Main application logic
3. AI: `arula_core/src/api/agent_client.rs` - AI communication
4. UI: `arula_cli/src/ui/tui_app.rs` - Terminal interface
5. Tools: `arula_core/src/tools/tools.rs` - Tool system

## Important Relationships
- Backend trait → Provider implementations (OpenAI, Anthropic, etc.)
- Tool trait → Built-in tools and MCP integration
- TuiApp → App → AgentClient (data flow)
- ExternalPrinter → Reedline (concurrent output)
```

### 3. Debugging & Development Tips
```markdown
# DEVELOPMENT NOTES
## Debug Mode
- Enable: `ARULA_DEBUG=1 cargo run -- --debug`
- Logs: `.arula/debug.log`
- Common issues: Terminal state, file locks, async deadlocks

## Key Architectural Decisions
- Single App instance with Arc<Mutex<>> for shared state
- Streaming responses via tokio::sync::mpsc channels
- Tools run in isolated subprocesses for safety
- Configuration migration: YAML → JSON (in progress)

## Performance Notes
- Use `cargo build --release` for production
- Streaming prevents memory issues with long responses
- Tool execution has timeout protection
- Terminal operations should be atomic
```

### 4. AI Interaction Patterns
```markdown
# AI INTERACTION PATTERNS
## Adding New Features
1. If modifying AI behavior: Update `agent_client.rs` or create new tool
2. If adding UI: Update `tui_app.rs` and maintain state consistency
3. If adding provider: Implement Backend trait in `api/`
4. ALWAYS update tests and documentation

## Common Modifications
- Add tool: Implement Tool trait, register in tools.rs
- Add command: Update main.rs args and TUI menu
- Modify streaming: Check app.rs for channel usage
- Update config: Maintain backward compatibility in config.rs
```

### 5. File Size & Complexity Indicators
```markdown
# FILE COMPLEXITY METRICS
## Large Files (>1000 lines)
- arula_core/src/app.rs (~83k lines) - Main orchestration
- arula_core/src/tools/visioneer.rs - Windows desktop automation
- arula_cli/src/ui/tui_app.rs - Terminal UI implementation

## Critical Files
- Single source of truth for each major feature
- Avoid duplicating logic between arula_cli and arula_desktop
- Core logic belongs in arula_core
```

### 6. Testing Strategy Notes
```markdown
# TESTING GUIDELINES
## Current Coverage
- Unit tests in most modules
- Integration tests for tools
- MockAll for external dependencies
- Wiremock for HTTP API testing

## Testing Challenges
- Async code requires test harness setup
- Terminal UI tests need mock terminal
- AI interactions require mocking or test endpoints
- File operations may need temp directories

## Test Categories
- `config_benchmarks`: Configuration performance
- `chat_benchmarks`: Message processing
- `tools_benchmarks`: Tool execution speed
```

### 7. Version Compatibility
```markdown
# COMPATIBILITY NOTES
## Rust Version
- Requires 1.70+ for async features
- Tested on stable channel

## Platform Support
- Primary: Linux, macOS, Windows
- Desktop automation: Windows only
- Vision tool: Requires Tesseract OCR

## API Compatibility
- Configuration format migrating YAML → JSON
- Backend trait stable for new providers
- Tool interface evolving with MCP integration
```

## Implementation Priority:
1. **Quick Reference** - Most important for rapid understanding
2. **Code Navigation** - Helps AI find relevant code quickly
3. **Development Notes** - Reduces debugging time
4. **AI Interaction Patterns** - Guides modifications
5. **Complexity Metrics** - Helps estimate effort
6. **Testing Guidelines** - Ensures quality
7. **Compatibility Notes** - Prevents issues