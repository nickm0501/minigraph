# AGENTS.md (mini-graph)

This repository is a small Rust (edition 2021) Axum + Tokio websocket server.
Agents working here should keep changes minimal, idiomatic, and clippy-clean.

## Implementation
- Whenever you come across any design decision, stop and list the viable options (with tradeoffs) before implementing.
- Whenever you find an invariant or edge case not covered by the current plan, stop and surface it so we can decide together.
- When deciding where domain methods live (e.g. on `AppState` vs a dedicated handle/newtype like `RoomsHandle`), stop and propose 2–3 viable options with tradeoffs (ergonomics, API surface area, coupling, testability) before implementing.
- When introducing cross-module dependencies (e.g. WAL reader -> rooms), proactively surface coupling/encapsulation options (direct handle, trait boundary, channel boundary) before implementing.
- When a function starts taking many related parameters (e.g. `client_id`, `current_room`, `tx`), consider encapsulating them into a small struct (e.g. `Session`) to reduce plumbing, clarify invariants, and make future refactors/extractions easier; propose this as a cleanup opportunity when it improves readability without obscuring control flow.
- After writing code, explain what changed and why (focus on tradeoffs and invariants).

## Source of Truth / Existing Rules

- `CLAUDE.md` exists and lists basic Cargo commands.
- Cursor rules: none found (no `.cursor/rules/` and no `.cursorrules`).
- Copilot rules: none found (no `.github/copilot-instructions.md`).

If you add any of the above rule files later, keep this document in sync.

---

## Quick Start

- Build: `cargo build`
- Run: `cargo run`
- Run server (then open): `http://localhost:3030`
- Lint: `cargo clippy --all-targets --all-features`
- Format: `cargo fmt`
- Test: `cargo test`

---

## Build / Run Commands

- Debug build: `cargo build`
- Release build: `cargo build --release`
- Run (debug): `cargo run`
- Run with backtraces: `RUST_BACKTRACE=1 cargo run`

Notes:
- The server binds `0.0.0.0:3030` in `src/main.rs`.
- Static files are served from `static/` (if present) via `tower-http`.

---

## Formatting / Linting

- Format (writes changes): `cargo fmt`
- Format check (CI-style): `cargo fmt -- --check`
- Clippy (recommended): `cargo clippy --all-targets --all-features`
- Clippy strict (optional locally): `cargo clippy --all-targets --all-features -- -D warnings`

Guideline:
- Prefer fixing clippy warnings instead of suppressing them.
- If you must allow a lint, keep it as narrow as possible and justify in code.

---

## Testing

### Run all tests

- All unit + integration tests: `cargo test`

### Run a single unit test (by substring)

- Name contains `join_room`: `cargo test join_room`

### Run a single test exactly

- Exact test name: `cargo test join_room -- --exact`

### Run a test in a specific module/path

- Example: `cargo test websocket::tests::handles_join -- --exact`

### Show `println!` output

- Do not capture stdout: `cargo test join_room -- --nocapture`

### Run integration tests (if/when `tests/` exists)

- Entire integration test file: `cargo test --test websocket`
- A single test inside it: `cargo test --test websocket handles_join -- --exact`

### Faster iteration

- One thread (reduces log interleaving): `cargo test -- --test-threads=1`

---

## Project Layout

Current modules:
- `src/main.rs`: HTTP router + server bootstrap
- `src/websocket.rs`: websocket upgrade and message loops
- `src/state.rs`: in-memory room membership + broadcast
- `src/types.rs`: wire types + error types

When adding features:
- Prefer adding new `src/<topic>.rs` modules over growing one file indefinitely.
- Keep `main.rs` focused on wiring (routes, state, configuration).

---

## Code Style (Rust)

### Formatting

- Always run `cargo fmt` before finalizing changes.
- Do not hand-format; let rustfmt decide.

### Imports

- Prefer explicit imports over glob imports.
- Group imports in this order (rustfmt generally enforces this):
  1. `std::...`
  2. external crates
  3. `crate::...`
- Use module-qualified names when it improves clarity (avoid overly-long `use` lists).

### Naming

- Modules/functions/vars: `snake_case`
- Types/traits/enums: `PascalCase`
- Constants/statics: `SCREAMING_SNAKE_CASE`
- Boolean-ish names should read well at call sites (`is_open`, `has_room`, `should_retry`).

### Types and ownership

- Prefer borrowing (`&str`, `&T`) over allocating (`String`, `Vec<T>`) unless ownership is needed.
- Avoid unnecessary `.clone()`; clone only at boundaries (e.g., storing in state, spawning tasks).
- Prefer `Option<T>` / `Result<T, E>` over sentinel values.

### Async / Tokio

- Avoid holding a mutex lock across `.await`.
- Keep critical sections small (lock, modify data, unlock).
- Prefer structured concurrency: spawn tasks only when you need independent lifetimes.

---

## Error Handling

General rules:
- Avoid `unwrap()` in server/runtime code paths.
- Use `expect("...")` only when a failure is truly impossible, and include a useful message.
- Prefer returning `Result<_, _>` and using `?` to propagate.

Error types:
- Prefer a small, explicit error enum for a module boundary (this repo has `WebSocketError`).
- When adding new error variants, keep messages stable and user-facing where appropriate.

Client-facing errors:
- Convert internal errors into a `ServerMessage::Error { message }` only at the websocket boundary.
- Do not leak internal details unless useful for debugging (balance security vs ergonomics).

---

## Serialization / Wire Protocol

- Messages are JSON using `serde` with `#[serde(tag = "type", rename_all = "snake_case")]`.
- Maintain backwards compatibility if clients might exist.
- When changing payload shapes, update both `ClientMessage` and `ServerMessage` carefully.

Guideline:
- Treat `src/types.rs` as API surface area: prefer additive changes.

---

## Logging

Current code uses `println!`.

Guidelines:
- Prefer consistent, grep-friendly prefixes (e.g. `[WS]`, `[STATE]`).
- Avoid noisy logs in hot loops unless behind a feature flag.

Optional future direction (if the project grows):
- Consider switching to `tracing` + `tracing-subscriber` for leveled logs.

---

## Concurrency / State

- `AppState` stores actor handles (e.g., `RoomsHandle`) and shared resources (e.g., `Arc<Metrics>`). Room membership is owned by the rooms actor.
- Do not keep per-client senders in multiple places unless ownership is clear.
- When broadcasting, handle failed senders by cleaning them up (current behavior).

Potential improvements (only if needed):
- Consider using `DashMap` or sharded locks if contention becomes an issue.

---

## Change Hygiene

- Keep PRs/patches focused; avoid drive-by refactors.
- Prefer small, testable functions over long match arms.
- Add unit tests alongside new logic when feasible.

---

## Useful Cargo Tips

- Expand macros (if needed): `cargo expand` (requires installing `cargo-expand`).
- Check features/targets: `cargo metadata`.
- Update deps carefully: `cargo update`.
