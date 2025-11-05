# 🚀 ARULA CLI

A modern autonomous AI CLI built with Rust and Ratatui, featuring a professional chat-style interface for autonomous task processing and code generation.

## Features

- 🤖 **Chat Interface**: Modern terminal UI with real-time chat interaction
- 🎨 **Code Art Generation**: Multiple ASCII art styles (Rust crab, fractals, matrix rain)
- ⚙️ **Configuration Management**: YAML-based configuration system
- 📊 **Task Processing**: Simulated AI task execution with progress tracking
- 📝 **Logging**: Comprehensive activity logging with timestamps
- 🎯 **Professional UI**: Clean, responsive terminal interface

## Installation

### Prerequisites
- Rust 1.70+ (install from https://rustup.rs/)
- Terminal with UTF-8 support

### Build from Source
```bash
git clone <repository>
cd arula
cargo build --release
```

### Run
```bash
# Development mode
cargo run

# Release mode
./target/release/arula-cli
```

## Usage

### Interactive Chat Mode
Simply run the CLI to enter chat mode:

```bash
./arula-cli
```

Then type commands like:
- `help` - Show available commands
- `art rust` - Generate Rust crab ASCII art
- `task demo` - Run task demonstration
- `status` - Check system status
- `exit` - Exit the application

### Available Commands

#### 🎨 Art Generation
- `art rust` - Generate Rust crab ASCII art
- `art fractal` - Generate fractal patterns
- `art matrix` - Generate Matrix digital rain effect
- `art demo` - Show all art styles

#### 🤖 Task Processing
- `task demo` - Run complete task demonstration
- `task status` - Show task statistics

#### ⚙️ Configuration
- `config` - Show current configuration
- `config init` - Initialize default configuration

#### 📊 System
- `status` - Show system status and statistics
- `logs` - View recent activity logs
- `clear` - Clear conversation history

#### 🚪 Navigation
- `help` - Show help information
- `exit` / `quit` / `q` - Exit application

## Architecture

### Core Components

- **`main.rs`**: Application entry point and terminal setup
- **`app.rs`**: Main application state and command handling
- **`chat.rs`**: Chat message types and data structures
- **`art.rs`**: ASCII art generation functions
- **`config.rs`**: Configuration management

### Dependencies

- **ratatui**: Modern terminal UI framework
- **crossterm**: Cross-platform terminal handling
- **tokio**: Async runtime
- **serde**: Serialization/deserialization
- **chrono**: Date/time handling
- **anyhow**: Error handling

## Technical Features

- **Async Architecture**: Built on Tokio for responsive UI
- **Event Handling**: Proper keyboard and terminal event processing
- **State Management**: Clean application state with message history
- **Error Handling**: Comprehensive error management with anyhow
- **Configuration**: YAML-based configuration system
- **Modular Design**: Clean separation of concerns

## Development

### Project Structure
```
arula/
├── src/
│   ├── main.rs      # Application entry point
│   ├── app.rs       # Main application state
│   ├── chat.rs      # Chat message types
│   ├── art.rs       # Art generation
│   └── config.rs    # Configuration management
├── Cargo.toml       # Dependencies
├── target/          # Compiled binaries
└── README.md        # This file
```

### Building
```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Check code
cargo check
cargo clippy
```

## Performance

- **Startup**: < 100ms (optimized build)
- **Memory**: < 10MB baseline
- **CPU**: Minimal impact during idle
- **Responsive**: 60Hz UI refresh rate

## License

MIT License - see LICENSE file for details

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Future Enhancements

- [ ] Real AI API integration (OpenAI, Anthropic)
- [ ] Git operations and branch management
- [ ] Advanced configuration editor
- [ ] Plugin system for custom commands
- [ ] Multiple workspace support
- [ ] Theme customization
- [ ] Mouse interaction support