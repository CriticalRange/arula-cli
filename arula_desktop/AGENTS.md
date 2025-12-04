# Repository Guidelines

## Project Structure & Module Organization
- The workspace root `Cargo.toml` defines members `arula_core/`, `arula_cli/`, `arula_desktop/`; shared docs live in `docs/`, prompts in `ARULA.md`.
- Core logic in `arula_core/src/`: `api/` (agent, streaming, HTTP), `tools/` (builtin + MCP), `utils/` (config/logger/conversation/errors), orchestration in `app.rs`.
- CLI UI code lives in `arula_cli/src/ui/`; the desktop Iced entry point is `arula_desktop/src/main.rs`.
- Keep assets and crate-specific helpers alongside their crate; only generalize cross-crate UI pieces when justified.

## Build, Test, and Development Commands
- `cargo check -p arula_core -p arula_cli -p arula_desktop` — type-check all members.
- `cargo test -p arula_core` — run unit/integration tests for shared logic.
- `cargo run -p arula_cli -- --help` — inspect CLI flags and defaults.
- `cargo run -p arula_desktop` — launch the GUI against the current backend.
- `cargo fmt` then `cargo clippy --workspace --all-targets` — format and lint before review.

## Coding Style & Naming Conventions
- Rust 2021; 4-space indent; rely on `cargo fmt` defaults.
- Prefer descriptive `snake_case` for functions/vars and `CamelCase` for types; avoid abbreviations in public APIs.
- Place provider/API code in `arula_core::api`, UI-only code in the owning crate; keep modules cohesive and small.
- Add brief comments only for non-obvious flows (async streaming, tool routing, or state transitions).

## Testing Guidelines
- Use Rust's built-in test harness; favor table-driven cases for streaming/tooling flows.
- Name tests `feature_behaves_as_expected`; group by module and edge cases.
- Run `cargo test -p arula_core` locally; add focused unit tests near implementations when changing core behaviors.
- For UI changes, sanity-check manually via `cargo run -p arula_cli` or `cargo run -p arula_desktop`.

## Commit & Pull Request Guidelines
- Commits: imperative subject (e.g., `Add streaming bridge to desktop`), small scope, mention rationale when non-obvious.
- PRs: include summary, tests executed (`cargo check`, `cargo test`, `clippy`), screenshots for UI changes, linked issues, and call out breaking changes or config migrations.

## Security & Configuration Tips
- Load API keys and provider settings via `arula_core::utils::config::Config`; never hardcode secrets or tokens.
- Validate file paths before invoking tools; avoid unchecked shell execution in new code paths.
- Keep local environment files out of commits; rely on documented config loaders instead.
