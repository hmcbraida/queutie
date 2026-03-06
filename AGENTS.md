# AGENTS.md

## Purpose
This guide is for coding agents operating in this repository.
Follow it to keep builds green, changes small, and behavior consistent.

## Repository Snapshot
- Language: Rust (edition 2024)
- Workspace root: `Cargo.toml`
- Workspace members:
  - `server`
  - `queutie_common`
  - `producer_cli`
  - `consumer_cli`
- Project intent: learning-oriented TCP pub/sub queue with shared wire protocol.

## Package Responsibilities
- `queutie_common`: packet definitions plus read/write framing logic.
- `server`: queue state, subscriber handling, TCP listener/connection handling.
- `producer_cli`: minimal publish client.
- `consumer_cli`: minimal subscribe client.

## Important Paths
- `Cargo.toml` (workspace members)
- `server/src/lib.rs`
- `server/src/server.rs`
- `server/src/queue.rs`
- `server/src/main.rs`
- `server/tests/integration_test.rs`
- `server/tests/blocking_subscriber_test.rs`
- `server/tests/support/mod.rs`
- `queutie_common/src/network.rs`
- `producer_cli/src/main.rs`
- `consumer_cli/src/main.rs`

## Command Conventions
- Run commands from repository root unless package-local behavior is required.
- Use workspace commands for full validation.
- Use package/test-function commands for quick iteration.

## Build Commands
- Build all crates: `cargo build --workspace`
- Build one crate:
  - `cargo build -p server`
  - `cargo build -p queutie_common`
  - `cargo build -p producer_cli`
  - `cargo build -p consumer_cli`
- Fast compile checks: `cargo check --workspace`
- Release build: `cargo build --workspace --release`

## Format and Lint Commands
- Apply formatting: `cargo fmt --all`
- Verify formatting only: `cargo fmt --all --check`
- Clippy (all targets): `cargo clippy --workspace --all-targets`
- Strict clippy (recommended before handoff):
  - `cargo clippy --workspace --all-targets -- -D warnings`

## Test Commands
- Run all tests: `cargo test --workspace`
- Run package tests:
  - `cargo test -p server`
  - `cargo test -p queutie_common`
  - `cargo test -p producer_cli`
  - `cargo test -p consumer_cli`
- Run one integration test file:
  - `cargo test -p server --test integration_test`
  - `cargo test -p server --test blocking_subscriber_test`
- Run one exact test function (preferred during iteration):
  - `cargo test -p server --test integration_test test_subscriber_receives_published_message -- --exact --nocapture`
  - `cargo test -p server --test blocking_subscriber_test test_publish_not_blocked_by_slow_subscriber -- --exact --nocapture`
- Run one exact unit test in `queutie_common`:
  - `cargo test -p queutie_common write_packet_handles_payload_larger_than_frame_size -- --exact --nocapture`
- List tests before targeting one:
  - `cargo test -p server -- --list`
  - `cargo test -p queutie_common -- --list`

## Local Run Commands
- Start server: `cargo run -p server`
- Start producer client: `cargo run -p producer_cli`
- Start consumer client: `cargo run -p consumer_cli`

## Code Style Guidelines

### Imports
- Group import blocks in this order:
  1) `std`
  2) external crates
  3) local crate modules (`crate::...`)
- Keep imports explicit and minimal.
- Avoid wildcard imports unless there is a clear readability win.

### Formatting and Layout
- Use rustfmt defaults; no custom rustfmt config is present.
- Keep multiline chains/matches easy to scan.
- Prefer trailing commas in multiline literals and calls.
- Keep functions focused; avoid long, mixed-responsibility blocks.

### Naming
- Types/traits/enums: `PascalCase`
- Functions/methods/modules/files: `snake_case`
- Constants/statics: `UPPER_SNAKE_CASE`
- Prefer intent-revealing names (`queue_name`, `packet_target`, `shared_state`).

### Types and API Shape
- Prefer explicit, concrete public signatures.
- Use generics only when behavior is truly type-parameterized.
- Reuse/introduce type aliases for complex shared types.
- Keep protocol-specific types in `queutie_common`.

### Ownership and Borrowing
- Prefer borrowing (`&str`, `&[u8]`) for read-only paths.
- Clone intentionally, mainly for thread handoff or ownership boundaries.
- Keep mutex guard lifetimes as short as possible.

### Error Handling
- Prefer `Result<T, E>` and propagate with `?` where possible.
- Avoid introducing new `unwrap()`/`expect()` in production logic.
- If panic is unavoidable, provide precise panic messages.
- Use small, focused error types for decode/parse failures.

### Concurrency and Locking
- Never hold `Mutex` guards across network I/O or long operations.
- Snapshot/copy needed state under lock, then release lock before writes.
- Keep spawned-thread bodies short and side-effect focused.
- Preserve existing behavior where slow subscribers do not block publish paths.

### Networking and Protocol
- Framing and packet encoding/decoding live in `queutie_common/src/network.rs`.
- Preserve wire compatibility unless doing an intentional protocol migration.
- Keep packet target/header size assumptions aligned between reader and writer.
- Add/update tests whenever framing or packet parsing behavior changes.

### Logging and Output
- Keep logs concise and operationally useful.
- Avoid noisy logging in hot paths unless intentionally debugging.

### Test Style
- Use descriptive behavior-driven test names.
- Prefer ephemeral ports (`127.0.0.1:0`) in networking tests.
- Keep timing-based assertions bounded and deterministic.
- Reuse helpers in `server/tests/support/mod.rs` where appropriate.
- Add regression coverage for concurrency and frame-size boundaries.

## Change Scope Guidance
- Keep protocol changes localized to `queutie_common` plus impacted callers/tests.
- Keep queue behavior changes in `server/src/queue.rs` with focused tests.
- Keep connection handling changes in `server/src/server.rs`.
- Avoid unrelated refactors in the same patch.

## Pre-Handoff Validation
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Also run the narrowest relevant single-test command for changed behavior.

## Cursor and Copilot Rules
- No Cursor rules currently found:
  - `.cursorrules` is missing.
  - `.cursor/rules/` is missing.
- No Copilot instruction file currently found:
  - `.github/copilot-instructions.md` is missing.
- If any of these files are added later, treat their directives as higher priority and merge them into this guide.

## Agent Working Notes
- Prefer minimal, targeted diffs.
- Preserve existing module boundaries and naming patterns.
- Add tests when fixing bugs or changing behavior.
- If uncertain, copy local conventions from the file you are editing.
