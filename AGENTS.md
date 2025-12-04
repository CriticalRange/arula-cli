# Repository Guidelines

## Project Structure & Module Organization
- Workspace root (`Cargo.toml`) with members: `arula_core/` (shared logic, agent/streaming/config), `arula_cli/` (terminal UI), `arula_desktop/` (Iced GUI scaffold). Shared assets/docs in `docs/`, prompts in `ARULA.md`.
- Core modules live in `arula_core/src/`: `api/` (agent, streaming, HTTP), `tools/` (builtin + MCP), `utils/` (config, logger, conversation, errors), `app.rs` (orchestration).
- CLI UI under `arula_cli/src/ui/` (menus, output, widgets). Desktop UI starts at `arula_desktop/src/main.rs`.

## Build, Test, and Development Commands
- `cargo check -p arula_core -p arula_cli -p arula_desktop` — type-check all crates.
- `cargo test -p arula_core` — run core unit/integration tests.
- `cargo run -p arula_cli -- --help` — launch CLI with flags help.
- `cargo run -p arula_desktop` — launch GUI (Iced) with current backend.
- Use `cargo fmt` and `cargo clippy --workspace --all-targets` before PRs.

## Coding Style & Naming Conventions
- Rust 2021 edition; enforce `cargo fmt` default style, 4-space indent.
- Prefer descriptive `snake_case` for functions/vars; `CamelCase` for types; avoid abbreviations in public APIs.
- Keep modules cohesive: API/provider logic in `arula_core::api`, UI-only code in respective crate.
- Add short comments only for non-obvious logic; avoid redundant narration.

## Testing Guidelines
- Use `cargo test -p arula_core` for core logic; add targeted unit tests near implementations.
- Test names: `module_behaves_as_expected` style; group by feature/edge case.
- For streaming/tooling flows, prefer table-driven tests and mock HTTP/tool registries where possible.

## Commit & Pull Request Guidelines
- Commits: clear imperative subject (e.g., `Add streaming bridge to desktop`), keep scope small; include rationale when non-obvious.
- PRs: include summary, testing performed (`cargo check`, `cargo test`, `clippy`), screenshots for UI changes, and link related issues. Call out breaking changes or config migrations.

## Security & Configuration Tips
- API keys and provider settings are read via `arula_core::utils::config::Config`; do not hardcode secrets. Use environment or config files in user dirs (see `Config::load_or_default`).
- Validate file paths before tool calls; avoid unchecked shell execution in new code paths.
