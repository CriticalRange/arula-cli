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
- Navigation icons in model selector footer (↵ Select, ← Back)
- Ctrl+C exit support in model selector menu
- Graceful "No models found" message when search returns empty results
- State-based rendering optimization to minimize screen updates
- Atomic line updates for flicker-free UI rendering

### Changed
- Configuration structure now supports multiple providers simultaneously
- Settings menu updated for better provider management
- Config API updated with new helper methods
- Model selector rendering now uses atomic string padding instead of character-by-character clearing
- Search functionality triggers full screen clear and re-render when query changes
- Settings navigation automatically skips non-editable fields when using arrow keys
- Non-editable API URL field now displays in gray color instead of showing error message

### Fixed
- Provider switching now preserves individual provider configurations
- API URL editing restricted to custom providers for safety
- Menu flickering and artifacts during navigation eliminated
- Overflow crash when search returns no results (attempt to subtract with overflow)
- Ctrl+C key detection now works correctly (match arm ordering fixed)
- Full viewport re-rendering on every arrow key press reduced to selective updates
- Search UI breaking and showing artifacts when filtering models

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
