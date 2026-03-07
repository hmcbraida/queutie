# Protocol Design (Current)

This document describes the current Queutie wire protocol and server behavior.
It is a design snapshot of what is implemented now, including known constraints.

## Goals and scope

- Keep transport simple: raw TCP with a small custom framing format.
- Support publish/subscribe plus server ACK/NACK responses.
- Keep shared protocol code in `queutie_common/src/network.rs`.

Out of scope today:

- Authentication/authorization
- Encryption/TLS negotiation
- Message acknowledgements or retries
- Flow control beyond fixed queue caps and persistence

## Components

- `queutie_common`: packet/frame encode + decode.
- `server`: listener, queue state, subscription fan-out.
- `producer_cli`: basic publisher client.

## Packet model

`Packet` is represented as:

- `header.packet_type`: operation selector
  - `0` => `Publish`
  - `1` => `Subscribe`
  - `2` => `QueueFull` (server -> producer rejection when queue cap is reached)
  - `3` => `PublishAck` (server -> producer acceptance after enqueue)
- `header.packet_target`: queue name
- `header.packet_id`: client-generated correlation id (`u64`)
- `body`: payload bytes (`Vec<u8>`)

Constants from implementation:

- `PACKET_HEADER_SIZE = 32` bytes
- `PACKET_TARGET_SIZE = 16` bytes

Current layout of packet header bytes before framing:

- Byte `0`: packet type (`0..=3`)
- Bytes `1..16` region: queue target bytes (NUL padded by default)
- Bytes `17..24`: packet id (`u64`, big-endian)
- Remaining header bytes up to 32: reserved/zero-filled

Read-side target extraction trims trailing `\0` during decode in
`decode_packet_header`, so callers receive a normalized queue name.

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
4. Write all frames, then flush once.

Large payloads are supported by multi-frame segmentation.
This is covered by a unit test for payload size `4097` bytes.

## Read path

`read_packet` behavior:

1. Read full frames in a loop.
2. Append `frame_body[..payload_len]` to a packet buffer.
3. Stop when final-frame flag is set.
4. Split first 32 bytes as packet header and decode fields.

Decode/encode paths are `Result`-based and return `NetworkError` variants for
malformed packets (invalid frame length, unknown packet type, short header,
invalid UTF-8 target, oversized target).

## Server behavior

Connection handling in `server/src/server.rs`:

- Accept connection, decode one packet.
- Use decoded packet target directly as queue name.

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
- Backpressure is drop-based only; there is no producer retry/ack semantics.

## Testing coverage (protocol-related)

- `queutie_common/src/network.rs` unit test:
  - `write_packet_handles_payload_larger_than_frame_size`
  - `write_packet_rejects_target_longer_than_protocol_limit`
  - `packet_type_rejects_unknown_discriminant`
  - `read_packet_rejects_payload_smaller_than_protocol_header`
- `server/tests/integration_test.rs`:
  - publish/subscribe message delivery behavior
- `server/tests/blocking_subscriber_test.rs`:
  - regression for publish behavior with slow subscribers

## TODO

1. Introduce protocol versioning and compatibility checks.
2. Add bounded queues/backpressure semantics.
3. Consider async I/O or worker pool instead of thread-per-connection.
