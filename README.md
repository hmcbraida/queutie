# Queutie

Queutie is a small Rust workspace for experimenting with a TCP publish/subscribe
queue server and a shared wire protocol.

The project currently focuses on learning-oriented infrastructure, not
production-ready durability or scaling.

## Current functionality (limited)

What works today:

- TCP server that accepts `Publish` and `Subscribe` packets.
- In-memory queue state keyed by queue name.
- Fan-out of published message bytes to current subscribers.
- Shared packet framing and serialization in `queutie_common`.
- Basic producer CLI that publishes a hard-coded message.
- Integration/concurrency tests for key behavior and regressions.

Important limitations right now:

- Queue data is in-memory only (no disk persistence).
- Server uses thread-per-connection and a global mutexed state map.
- Very limited protocol surface (`Publish` and `Subscribe` only).
- Minimal client ergonomics (producer sends fixed queue/message).
- Error handling still has several `unwrap()`/panic paths.
- No authentication, authorization, encryption, or backpressure controls.

## Project structure

```text
queutie/
├── Cargo.toml                  # workspace definition
├── AGENTS.md                   # coding-agent guide
├── server/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs              # crate exports
│   │   ├── main.rs             # server binary entrypoint
│   │   ├── queue.rs            # queue and subscriber abstractions
│   │   └── server.rs           # TCP listener + connection handling
│   └── tests/
│       ├── integration_test.rs
│       └── blocking_subscriber_test.rs
├── queutie_common/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── network.rs          # packet/frame encode/decode
└── producer_cli/
    ├── Cargo.toml
    └── src/main.rs             # simple publish client
```

## Quick start

From the repository root:

```bash
# 1) Run the server
cargo run -p server

# 2) In another terminal, publish one message
cargo run -p producer_cli
```

The default setup publishes `"hello world"` to `test_queue` on
`127.0.0.1:3001`.

## Build, lint, and test

```bash
# Build all workspace crates
cargo build --workspace

# Format + lint
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings

# Run all tests
cargo test --workspace
```

Run one specific test (useful during iteration):

```bash
cargo test -p server --test blocking_subscriber_test test_publish_not_blocked_by_slow_subscriber -- --exact --nocapture
```

## Notes

- `AGENTS.md` contains repository-specific instructions for coding agents.
- `docs/protocol-design.md` documents the current wire protocol and system behavior.
- `TODO.md` tracks known architectural/performance follow-ups.
