# TODO: Architectural and Performance Improvements

## Performance Issues

### 1. Blocking subscriber notification while holding lock

**Location:** `server/src/server.rs:50-53`

**Problem:** `push_message_to_subscribers` is called while holding the queue mutex. Slow or unresponsive subscribers block all queue operations.

**Suggested fix:** Spawn a separate task/thread to handle subscriber notifications, or use a channel to hand off the notification work outside the lock.

---

### 2. Global lock contention

**Location:** `server/src/server.rs:9`

**Problem:** Single `Mutex<HashMap<String, MessageQueue>>` serializes all queue operations across the entire server.

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

### 6. `unwrap()` everywhere

**Location:** Multiple files (e.g., `server/src/server.rs:29`, `network.rs:29-30`)

**Problem:** No error handling; network/IO failures panic the server.

**Suggested fix:** Replace `unwrap()` with proper error handling (`Result` types, `?` operator, logging).

---

### 7. Integration test duplicates server logic

**Location:** `server/tests/integration_test.rs`

**Problem:** Test re-implements the server instead of using the `Server` struct.

**Suggested fix:** Refactor test to use `Server::new()` and `server.run()` with proper setup/teardown.

---

## Completed (or deferred)

- ~~Separate modules for queue, server, network~~ ✅ Done
- ~~Remove `on_publish` callback~~ ✅ Done
- ~~Remove `Consume` packet type~~ ✅ Done
