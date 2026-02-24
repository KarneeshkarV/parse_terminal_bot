# Agent Guidelines for parse_terminal_bot

Rust-based terminal parsing bot with Axum HTTP/WebSocket server and vanilla JS frontend.

## Build Commands

```bash
# Build debug
cargo build

# Build release (used by run.sh)
cargo build --release

# Run with config
cargo run -- config.toml

# Launch in tmux (production)
./run.sh
```

## Lint/Test Commands

```bash
# Check code
cargo check

# Run clippy
cargo clippy --all-targets --all-features

# Run all tests
cargo test

# Run single test
cargo test <test_name>

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check
```

## Code Style Guidelines

### Imports
- Group: std lib → external crates → local modules
- Local modules use `crate::` prefix
- One blank line between import groups

### Error Handling
- Use `anyhow::Result` for application-level errors
- Use `anyhow::anyhow!("msg")` for custom error messages
- Propagate errors with `?` operator
- Log errors with `tracing::error!` before exiting

### Types & Naming
- Type aliases in snake_case with types module: `pub type PaneId = String;`
- Struct fields aligned when related (see types.rs)
- Serde attributes: `#[serde(tag = "...", rename_all = "snake_case")]`
- Use `#[derive(Debug, Clone)]` for most types

### Async & Concurrency
- Use `tokio::spawn` for background tasks
- Channels: `tokio::sync::broadcast` for shutdown signals
- Use `tokio::select!` for waiting multiple futures
- Clone Arc-like types before moving into async blocks

### Logging
- Use `tracing` macros: `info!`, `error!`, `warn!`, `debug!`
- Initialize subscriber in main with `EnvFilter`
- Log key lifecycle events (startup, shutdown, errors)

### Module Structure
- `mod.rs` in each subdirectory re-exports public items
- Keep modules small and focused
- Public API exposed through module boundaries

### Formatting
- Visual alignment for struct fields (see types.rs examples)
- Section comments with ── borders in main.rs
- Max line length: ~100 characters
- Trailing commas in multi-line structs/enums

### Configuration
- Use `config.toml` in project root
- Serde derive for config structs
- Validate config early, exit with error message on failure

### Testing
- Unit tests in same file as code under `#[cfg(test)]`
- Integration tests in `tests/` directory
- Use `tokio::test` for async tests

## Frontend (vanilla JS)

- ES6 modules with import/export
- No build step required
- Files in `frontend/` served statically
- WebSocket client in `ws_client.js`

## Dependencies

Key crates: tokio, axum, serde, tracing, anyhow, vte, chrono, uuid
See Cargo.toml for full list and versions.
