# Changelog
<!-- type: release -->

All notable changes to ARULA CLI will be documented in this file.

## [Unreleased]

### Added
- Multi-provider configuration support - switch between AI providers without losing settings
- Custom provider support with full control over model, API URL, and API key
- Automatic config migration from legacy single-provider format
- Provider-specific settings persistence
- Real-time changelog display in startup banner

### Changed
- Configuration structure now supports multiple providers simultaneously
- Settings menu updated for better provider management
- Config API updated with new helper methods

### Fixed
- Provider switching now preserves individual provider configurations
- API URL editing restricted to custom providers for safety

## [0.1.0] - 2025-01-22

### Added
- Initial release of ARULA CLI
- Support for OpenAI, Anthropic, Ollama, Z.AI, and OpenRouter providers
- Interactive configuration menu
- Multi-line input with Shift+Enter
- Visioneer desktop automation tool
- File operations (read, write, edit, search)
- Bash command execution
- Syntax highlighting for code blocks
- Progress indicators and spinners
