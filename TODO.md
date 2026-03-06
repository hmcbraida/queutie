# TODO: Architectural and Performance Improvements

## Performance Issues

### 1. ~~Blocking subscriber notification while holding lock~~ ✅ Done

**Location:** `server/src/server.rs:50-53`

**Problem:** `push_message_to_subscribers` is called while holding the queue mutex. Slow or unresponsive subscribers block all queue operations.

**Status:** Fixed by moving subscriber notification outside the mutex and cleaning up failed subscribers after re-locking briefly.

**Suggested fix:** Spawn a separate task/thread to handle subscriber notifications, or use a channel to hand off the notification work outside the lock.

---

### 2. ~~Global lock contention~~ ✅ Done

**Location:** `server/src/server.rs:9`

**Problem:** Single `Mutex<HashMap<String, MessageQueue>>` serializes all queue operations across the entire server.

**Status:** Fixed by storing each queue behind its own mutex (`HashMap<String, Arc<Mutex<MessageQueue>>>`) so map locking is short-lived and queue operations no longer serialize globally.

**Suggested fix:** Use finer-grained locking - either per-queue locks (RwLock per queue) or sharding across multiple HashMaps.

---

### 3. Thread-per-connection

**Location:** `server/src/server.rs:31-37`

**Problem:** Spawning a new `thread::spawn` for each connection is expensive at scale.

**Suggested fix:** Use a thread pool (e.g., `rayon` or custom worker threads) or migrate to async (`tokio`).

---

## Architectural Issues

### 4. `maintain_subscription` is a no-op

**Location:** `server/src/server.rs:65-69`

**Problem:** Just loops sleeping. Doesn't receive messages or do anything useful. Works only because push happens at publish time.

**Suggested fix:** Either remove this function or implement proper subscription handling (e.g., listening for disconnect, sending keepalives).

---

### 5. No backpressure

**Location:** `server/src/queue.rs:31`

**Problem:** `VecDeque` has unbounded capacity. Memory grows indefinitely if producers outpace subscribers.

**Suggested fix:** Add bounded queue with backpressure (return error or block when full) or implement message TTL/eviction.

---

### 6. ~~`unwrap()` everywhere~~ ✅ Done

**Location:** Multiple files (e.g., `server/src/server.rs:29`, `network.rs:29-30`)

**Problem:** No error handling; network/IO failures panic the server.

**Status:** Fixed by introducing typed error enums (`NetworkError`, `ServerError`), returning `Result` from packet read/write paths, and replacing runtime panics/unwraps in production code with propagation and connection-level logging.

**Suggested fix:** Replace `unwrap()` with proper error handling (`Result` types, `?` operator, logging).

---

### 7. ~~Integration test duplicates server logic~~ ✅ Done

**Location:** `server/tests/integration_test.rs`

**Problem:** Test re-implements the server instead of using the `Server` struct.

**Status:** Fixed by starting the real `Server` in integration tests and extracting shared test helpers into `server/tests/support/mod.rs`.

**Suggested fix:** Refactor test to use `Server::new()` and `server.run()` with proper setup/teardown.

---

## Completed (or deferred)

- ~~Blocking subscriber notification while holding lock~~ ✅ Done
- ~~Global lock contention~~ ✅ Done
- ~~Fix packet writes for payloads > 1024 bytes~~ ✅ Done
- ~~Separate modules for queue, server, network~~ ✅ Done
- ~~Remove `on_publish` callback~~ ✅ Done
- ~~Remove `Consume` packet type~~ ✅ Done
- ~~Integration test duplicates server logic~~ ✅ Done
- ~~`unwrap()` everywhere~~ ✅ Done
