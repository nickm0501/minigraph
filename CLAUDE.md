# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Quick Commands

**Building and Running:**
- `cargo build` - Build the project
- `cargo run` - Build and run the project
- `cargo build --release` - Build optimized release binary

**Testing and Linting:**
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run a specific test
- `cargo clippy` - Run the Rust linter for common mistakes and style issues
- `cargo fmt` - Format code according to Rust style guidelines
- `cargo fmt -- --check` - Check formatting without modifying files

**Dependency and Project Management:**
- `cargo add <crate>` - Add a new dependency
- `cargo update` - Update dependencies to latest compatible versions
- `cargo doc --open` - Generate and open documentation for the project and dependencies

## Project Architecture

This is a Rust binary application project. The structure is minimal and grows organically as features are added:

- `src/main.rs` - Entry point for the application
- As the project grows, additional modules should be organized by functionality (e.g., `src/graph.rs`, `src/cli.rs`)
- Tests can be placed in the same file as the code they test (using `#[cfg(test)]` modules) or in a `tests/` directory for integration tests

## Notes

- Edition: The project uses Rust edition 2021 (though Cargo.toml currently specifies 2024, which should be corrected if not intentional)
- No external dependencies are currently configured; add them as needed using `cargo add`
