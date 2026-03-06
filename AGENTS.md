# AGENTS.md

## Purpose
This document is for coding agents working in this repository.
It defines how to build, lint, test, and contribute code consistently.

## Repository Overview
- Language: Rust (edition 2024)
- Workspace root: `Cargo.toml`
- Workspace members:
  - `server`
  - `queutie_common`
  - `producer_cli`
- High-level responsibilities:
  - `queutie_common`: shared packet/network protocol
  - `server`: queue server library + binary
  - `producer_cli`: simple publisher CLI

## Project Layout
- `Cargo.toml` (workspace)
- `server/src/lib.rs`
- `server/src/server.rs`
- `server/src/queue.rs`
- `server/src/main.rs`
- `server/tests/integration_test.rs`
- `server/tests/blocking_subscriber_test.rs`
- `queutie_common/src/network.rs`
- `producer_cli/src/main.rs`

## Tooling Assumptions
- Use Cargo commands from repository root unless package-specific behavior is needed.
- Prefer workspace-level checks when validating broad changes.
- Prefer package-level test runs for faster iteration.

## Build Commands
- Build everything:
  - `cargo build --workspace`
- Build one package:
  - `cargo build -p server`
  - `cargo build -p queutie_common`
  - `cargo build -p producer_cli`
- Fast type-check without full codegen:
  - `cargo check --workspace`
- Release build:
  - `cargo build --workspace --release`

## Format / Lint Commands
- Format all crates:
  - `cargo fmt --all`
- Check formatting only:
  - `cargo fmt --all --check`
- Run clippy for all targets:
  - `cargo clippy --workspace --all-targets`
- Strict clippy (recommended before PR):
  - `cargo clippy --workspace --all-targets -- -D warnings`

## Test Commands
- Run all tests:
  - `cargo test --workspace`
- Run tests for one package:
  - `cargo test -p server`
  - `cargo test -p queutie_common`
- Run one integration test file:
  - `cargo test -p server --test integration_test`
  - `cargo test -p server --test blocking_subscriber_test`
- Run one exact test function (recommended pattern):
  - `cargo test -p server --test blocking_subscriber_test test_publish_not_blocked_by_slow_subscriber -- --exact --nocapture`
  - `cargo test -p server --test integration_test test_subscriber_receives_published_message -- --exact --nocapture`
- Run one unit test in `queutie_common`:
  - `cargo test -p queutie_common write_packet_handles_payload_larger_than_frame_size -- --exact --nocapture`
- List available tests:
  - `cargo test -p server -- --list`
  - `cargo test -p queutie_common -- --list`

## Local Dev Run Commands
- Run server binary:
  - `cargo run -p server`
- Run producer CLI:
  - `cargo run -p producer_cli`

## Coding Conventions

### Imports
- Group imports by origin in this order:
  1) `std`
  2) external crates
  3) local crate (`crate::...`)
- Keep imports explicit and minimal.
- Avoid wildcard imports unless there is a strong reason.

### Formatting
- Use rustfmt defaults (no repo-specific rustfmt config exists).
- Keep line breaks readable around long builder/match chains.
- Prefer trailing commas in multiline literals and function calls.

### Naming
- Types/traits/enums: `PascalCase` (`MessageQueue`, `PacketType`).
- Functions/methods/modules/files: `snake_case`.
- Constants: `UPPER_SNAKE_CASE`.
- Use descriptive names (`queue_name`, `failed_subscribers`, `packet_target`).

### Types and API Design
- Prefer concrete, explicit types at public boundaries.
- Use generics when behavior is clearly type-parameterized (existing pattern: `MessageQueue<S: Subscriber>`).
- Prefer type aliases for complex shared types (existing pattern: `SharedState`).
- Keep shared protocol structures in `queutie_common`.

### Ownership and Borrowing
- Prefer borrowing (`&[u8]`, `&str`) for read-only operations.
- Clone deliberately and only when needed for thread handoff or ownership transfer.
- Keep lock lifetimes short; drop locks before potentially blocking operations.

### Error Handling
- Prefer returning `Result<T, E>` from fallible logic.
- Use `?` to propagate errors when the caller can decide recovery.
- Avoid introducing new `unwrap()`/`expect()` in production paths.
- If panic is truly intended, include a precise panic message.
- For decode/parse errors, define focused error types (existing pattern: `StringDecodeError`).

### Concurrency and Locks
- Do not hold `Mutex` guards across I/O or long-running work.
- Copy/extract required state while locked, then release lock before network writes.
- Use `Arc<Mutex<...>>` sparingly; prefer finer-grained locking if touching shared state design.
- Keep spawned thread bodies small and side-effect focused.

### Networking / Protocol
- Protocol framing logic lives in `queutie_common/src/network.rs`.
- Preserve frame/header constants compatibility unless doing a deliberate protocol migration.
- Keep packet target size and framing assumptions consistent between reader/writer.
- Add/adjust tests when packet framing behavior changes.

### Logging / Output
- Use concise operational logs.
- Avoid noisy per-byte/per-frame logs in hot paths unless behind debug controls.

### Test Style
- Use descriptive test names that state behavior.
- Prefer ephemeral ports (`127.0.0.1:0`) in network tests.
- Keep timing-based tests resilient: bounded waits, clear timeout failure messages.
- Reuse helper functions (`publish`, `subscribe`, `start_server`) in integration tests.
- Add regression tests for concurrency and frame-size edge cases.

## Change Scope Guidelines
- Keep protocol changes isolated to `queutie_common` + impacted call sites/tests.
- Keep queue behavior changes in `server/src/queue.rs` with focused tests.
- Keep connection handling changes in `server/src/server.rs`.
- Avoid unrelated refactors in the same patch.

## Validation Checklist (Before Finishing)
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- If change is localized, also run the narrowest relevant single-test command.

## Cursor / Copilot Rules
- No Cursor rules were found:
  - `.cursorrules` missing
  - `.cursor/rules/` missing
- No Copilot instructions were found:
  - `.github/copilot-instructions.md` missing
- If these files are added later, merge their directives into this document and treat them as higher-priority repo policy.

## Notes for Agents
- Prefer minimal, targeted diffs.
- Preserve module boundaries and existing naming patterns.
- Add tests with bug fixes and behavior changes.
- When uncertain, follow existing code in the touched module.
