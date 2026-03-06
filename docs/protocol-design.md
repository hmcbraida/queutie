# Protocol Design (Current)

This document describes the current Queutie wire protocol and server behavior.
It is a design snapshot of what is implemented now, including known constraints.

## Goals and scope

- Keep transport simple: raw TCP with a small custom framing format.
- Support two operations: publish a message, subscribe to a queue.
- Keep shared protocol code in `queutie_common/src/network.rs`.

Out of scope today:

- Authentication/authorization
- Encryption/TLS negotiation
- Message acknowledgements or retries
- Backpressure, flow control, and persistence

## Components

- `queutie_common`: packet/frame encode + decode.
- `server`: listener, queue state, subscription fan-out.
- `producer_cli`: basic publisher client.

## Packet model

`Packet` is represented as:

- `header.packet_type`: operation selector
  - `0` => `Publish`
  - `1` => `Subscribe`
- `header.packet_target`: queue name
- `body`: payload bytes (`Vec<u8>`)

Constants from implementation:

- `PACKET_HEADER_SIZE = 32` bytes
- `PACKET_TARGET_SIZE = 16` bytes

Current layout of packet header bytes before framing:

- Byte `0`: packet type (`0`/`1`)
- Bytes `1..16` region: queue target bytes (NUL padded by default)
- Remaining header bytes up to 32: reserved/zero-filled

Note: read-side target extraction currently uses `packet_data[1..PACKET_TARGET_SIZE]`,
then trims trailing `\0` at call sites.

## Frame model

Packets are transmitted as one or more fixed-size frames.

- `FRAME_HEADER_LENGTH = 4`
- `FRAME_BODY_LENGTH = 1024`

Frame header bytes:

- Byte `0`: final-frame flag (`0x01` for last frame, `0x00` otherwise)
- Bytes `1..=2`: payload byte count for this frame (`u16`, big-endian)
- Byte `3`: currently unused/reserved

Frame body:

- 1024-byte buffer
- only first `payload_len` bytes are meaningful

## Write path

`write_packet` behavior:

1. Build packet bytes (`32-byte` header + message body).
2. Split bytes into chunks of up to `1024`.
3. For each chunk, create a frame with final flag and length.
4. Write frame header and body to stream, then flush.

Large payloads are supported by multi-frame segmentation.
This is covered by a unit test for payload size `4097` bytes.

## Read path

`read_packet` behavior:

1. Read full frames in a loop.
2. Append `frame_body[..payload_len]` to a packet buffer.
3. Stop when final-frame flag is set.
4. Split first 32 bytes as packet header and decode fields.

Current error behavior is panic-heavy (`unwrap()` and `panic!`), not `Result` based.

## Server behavior

Connection handling in `server/src/server.rs`:

- Accept connection, decode one packet.
- Determine queue name from packet target, trimming trailing NUL bytes.

On `Publish`:

1. Acquire state lock and push message into queue.
2. Clone subscriber handles while locked.
3. Release lock and send payload to subscribers.
4. Re-lock briefly to remove failed subscribers.

On `Subscribe`:

1. Add `TcpSubscriber` to queue while holding lock.
2. Keep connection open with `maintain_subscription` loop.

## Current limitations and implications

- State is in-memory only; restart loses all queued messages/subscribers.
- One global `Mutex<HashMap<...>>` can become a contention point.
- Thread-per-connection model may not scale under high concurrency.
- No protocol version field or negotiation.
- Queue target width is constrained by fixed header encoding.
- Several production paths still rely on `unwrap()`.

## Testing coverage (protocol-related)

- `queutie_common/src/network.rs` unit test:
  - `write_packet_handles_payload_larger_than_frame_size`
- `server/tests/integration_test.rs`:
  - publish/subscribe message delivery behavior
- `server/tests/blocking_subscriber_test.rs`:
  - regression for publish behavior with slow subscribers

## Suggested evolution path

1. Move protocol read/write to `Result`-returning APIs.
2. Define explicit packet header schema and document byte offsets.
3. Introduce protocol versioning and compatibility checks.
4. Add bounded queues/backpressure semantics.
5. Consider async I/O or worker pool instead of thread-per-connection.
